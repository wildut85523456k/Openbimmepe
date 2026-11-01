//! STEP / SAT import via `cadrum` (statically-linked, headless OpenCASCADE).
//!
//! STEP is the primary family-authoring format: manufacturers ship STEP, not
//! RVT families. We read the B-rep and tessellate it to a triangle mesh for
//! display, with whole-shape parametric scaling on top (see the family
//! authoring contract). SAT is the ACIS text format; cadrum reads STEP, so
//! SAT would need a separate ACIS reader and is out of scope for this slice.

use openbim_document::Mesh;

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
}
