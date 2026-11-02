//! STEP / SAT import via `cadrum` (statically-linked, headless OpenCASCADE).
//!
//! STEP is the primary family-authoring format: manufacturers ship STEP, not
//! RVT families. We read the B-rep and tessellate it to a triangle mesh for
//! display, with whole-shape parametric scaling on top (see the family
//! authoring contract). SAT is the ACIS text format; cadrum reads STEP, so
//! SAT would need a separate ACIS reader and is out of scope for this slice.

use openbim_document::Mesh;

/// Geometric parameters extracted from an imported STEP mesh (metres), shown as
/// the family's parameter table: the bounding box, an enclosed-volume estimate,
/// the centroid and the mesh resolution. These become the family instance
/// parameters (Width / Depth / Height / Volume) and drive quantity take-off.
#[derive(Clone, Debug, PartialEq)]
pub struct StepParams {
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    pub volume: f64,
    pub centroid: [f64; 3],
    pub vertices: usize,
    pub triangles: usize,
}

/// Extract the parameter table from a mesh (metres). The volume is the sum of
/// the signed tetrahedron volumes from the origin over outward-wound triangles.
pub fn mesh_params(m: &Mesh) -> StepParams {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    let mut vol = 0.0;
    for face in &m.faces {
        if face.len() < 3 { continue; }
        for &i in face {
            let p = m.vertices[i];
            for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); }
        }
        // signed volume of the fan of triangles of this face over the origin
        for k in 1..face.len().saturating_sub(1) {
            let (a, b, c) = (m.vertices[face[0]], m.vertices[face[k]], m.vertices[face[k + 1]]);
            vol += a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0]);
        }
    }
    let vol = (vol / 6.0).abs();
    let n = m.vertices.len().max(1) as f64;
    let mut c = [0.0; 3];
    for v in &m.vertices { for k in 0..3 { c[k] += v[k]; } }
    let centroid = [c[0] / n, c[1] / n, c[2] / n];
    StepParams { width: hi[0] - lo[0], depth: hi[1] - lo[1], height: hi[2] - lo[2], volume: vol, centroid, vertices: m.vertices.len(), triangles: m.faces.len() }
}

/// Read a STEP file and tessellate its solids to a triangle mesh.
///
/// The mesh is in the STEP's own units (millimetres for most manufacturers);
/// callers scale to metres when placing it.
pub fn import_step(path: &str) -> Result<Mesh, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
    let mut cursor = std::io::Cursor::new(bytes);
    let solids = cadrum::Solid::read_step(&mut cursor).map_err(|e| format!("read step: {e}"))?;
    if solids.is_empty() {
        return Err("no solids in STEP file".to_string());
    }
    let mesh = cadrum::Solid::mesh(solids.iter(), cadrum::Tessellation::default())
        .map_err(|e| format!("mesh: {e}"))?;
    let vertices = mesh.vertices.iter().map(|v| [v.x, v.y, v.z]).collect();
    let faces = mesh
        .indices
        .chunks(3)
        .map(|t| vec![t[0], t[1], t[2]])
        .collect();
    Ok(Mesh { vertices, faces })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_roundtrip_meshes_a_box() {
        // cadrum can also write STEP, so make a known box, write it, re-read it.
        let solid = cadrum::Solid::cube([0.0, 0.0, 0.0].into(), [100.0, 200.0, 300.0].into());
        let dir = std::env::temp_dir().join("openbim_step_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("box.stp");
        let mut file = std::fs::File::create(&path).unwrap();
        cadrum::Solid::write_step([&solid], &mut file).unwrap();
        drop(file);

        let mesh = import_step(path.to_str().unwrap()).expect("import");
        assert!(!mesh.vertices.is_empty(), "mesh has vertices");
        assert!(mesh.faces.len() >= 12, "a box has >= 12 triangles");
        // the box spans 0..100 x, 0..200 y, 0..300 z
        let max = mesh
            .vertices
            .iter()
            .map(|v| v[0].max(v[1]).max(v[2]))
            .fold(0.0, f64::max);
        assert!((max - 300.0).abs() < 1e-6, "box reaches z=300, got {max}");
    }

    #[test]
    fn mesh_params_reports_the_bounding_box_and_volume() {
        // a 2 x 3 x 4 box, in metres, from two triangles of one face is enough for the box;
        // use a full box here so the volume is meaningful
        let m = Mesh {
            vertices: vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 3.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 4.0], [2.0, 0.0, 4.0], [2.0, 3.0, 4.0], [0.0, 3.0, 4.0]],
            faces: vec![vec![0, 3, 2], vec![0, 2, 1], vec![4, 5, 6], vec![4, 6, 7], vec![0, 1, 5], vec![0, 5, 4], vec![1, 2, 6], vec![1, 6, 5], vec![2, 3, 7], vec![2, 7, 6], vec![3, 0, 4], vec![3, 4, 7]],
        };
        let p = mesh_params(&m);
        assert!((p.width - 2.0).abs() < 1e-9 && (p.depth - 3.0).abs() < 1e-9 && (p.height - 4.0).abs() < 1e-9, "box {p:?}");
        assert!((p.volume - 24.0).abs() < 1e-6, "volume 2*3*4=24, got {}", p.volume);
    }
}
