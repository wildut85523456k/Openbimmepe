//! Triangle meshes of the model, one per element, for a 3D viewport. Output is in element order (walls, slabs,
//! columns, family items) and does not depend on the number of threads.

use crate::{slab_holes, Element, Kind, ViewError};
use openbim_geom::Mesh;
use openbim_ifc::Scene;
use rayon::prelude::*;

type P = [f64; 2];

/// The mesh of one element. Triangles face outward.
#[derive(Debug, Clone)]
pub struct ElementMesh {
    pub element: Element,
    pub mesh: Mesh,
}

fn area2(r: &[P]) -> f64 {
    (0..r.len()).map(|k| { let (a, b) = (r[k], r[(k + 1) % r.len()]); a[0] * b[1] - b[0] * a[1] }).sum()
}

fn oriented(r: &[P], ccw: bool) -> Vec<P> {
    let mut v = r.to_vec();
    if (area2(&v) > 0.0) != ccw { v.reverse(); }
    v
}

/// A prism between heights z0 and z1 on `outer`, with vertical holes. Returns None if the outline cannot be triangulated.
pub(crate) fn prism(outer: &[P], holes: &[Vec<P>], z0: f64, z1: f64) -> Option<Mesh> {
    if outer.len() < 3 || z1 <= z0 { return None; }
    let rings: Vec<Vec<P>> = std::iter::once(oriented(outer, true)).chain(holes.iter().filter(|h| h.len() >= 3).map(|h| oriented(h, false))).collect();
    let pts: Vec<P> = rings.iter().flatten().copied().collect();
    let mut starts = Vec::new();
    let mut n = rings[0].len();
    for h in &rings[1..] { starts.push(n); n += h.len(); }
    let flat: Vec<f64> = pts.iter().flat_map(|p| [p[0], p[1]]).collect();
    let idx = earcutr::earcut(&flat, &starts, 2).ok()?;
    if idx.len() % 3 != 0 || idx.iter().any(|&k| k >= pts.len()) { return None; }
    let m = pts.len() as u32;
    let mut mesh = Mesh::default();
    for &z in &[z0, z1] { mesh.verts.extend(pts.iter().map(|p| [p[0], p[1], z])) }
    for t in idx.chunks(3) {
        let (a, b, c) = (t[0] as u32, t[1] as u32, t[2] as u32);
        let ccw = (pts[t[1]][0] - pts[t[0]][0]) * (pts[t[2]][1] - pts[t[0]][1]) - (pts[t[1]][1] - pts[t[0]][1]) * (pts[t[2]][0] - pts[t[0]][0]) > 0.0;
        let (b, c) = if ccw { (b, c) } else { (c, b) };
        mesh.tris.push([a + m, b + m, c + m]);
        mesh.tris.push([a, c, b]);
    }
    let mut start = 0u32;
    for r in &rings {
        let k = r.len() as u32;
        for i in 0..k {
            let (u, v) = (start + i, start + (i + 1) % k);
            mesh.tris.push([u, v, v + m]);
            mesh.tris.push([u, v + m, u + m]);
        }
        start += k;
    }
    Some(mesh)
}

fn bad(el: Element) -> ViewError { ViewError::Mesh { id: el.id } }

/// Meshes of every wall, slab, column and family solid in the scene.
pub fn scene_meshes(scene: &Scene) -> Result<Vec<ElementMesh>, ViewError> {
    let walls: Vec<Result<ElementMesh, ViewError>> = scene.walls.par_iter().map(|w| {
        let mesh = w.chain.chain_mesh().map_err(|err| ViewError::Wall { id: w.id, err })?;
        Ok(ElementMesh { element: Element { kind: Kind::Wall, id: w.id }, mesh })
    }).collect();
    let slabs: Vec<Result<ElementMesh, ViewError>> = scene.slabs.par_iter().enumerate().map(|(i, s)| {
        let el = Element { kind: Kind::Slab, id: s.id };
        let holes = slab_holes(scene, i, &s.slab);
        Ok(ElementMesh { element: el, mesh: prism(&s.slab.outline, &holes, s.slab.top - s.slab.thickness, s.slab.top).ok_or(bad(el))? })
    }).collect();
    let columns: Vec<Result<ElementMesh, ViewError>> = scene.columns.par_iter().map(|c| {
        let el = Element { kind: Kind::Column, id: c.id };
        Ok(ElementMesh { element: el, mesh: prism(&c.footprint, &[], c.base_z, c.base_z + c.height).ok_or(bad(el))? })
    }).collect();
    let items: Vec<Result<ElementMesh, ViewError>> = scene.items.par_iter().map(|it| {
        let el = Element { kind: Kind::Family, id: it.id };
        let mut mesh = Mesh::default();
        for (k, (_, bx)) in it.instance.solids.iter().enumerate() {
            let xf = it.instance.xf.get(k).copied().flatten();
            let ring: Vec<P> = [[bx.min[0], bx.min[1]], [bx.max[0], bx.min[1]], [bx.max[0], bx.max[1]], [bx.min[0], bx.max[1]]].iter().map(|&q| {
                let q = match xf { Some(t) => t.apply(q), None => q };
                let w = it.frame.point([q[0], q[1], 0.0]);
                [w[0], w[1]]
            }).collect();
            let z0 = it.frame.origin[2] + bx.min[2];
            let z1 = it.frame.origin[2] + bx.max[2];
            let Some(m) = prism(&ring, &[], z0, z1) else { continue };
            let base = mesh.verts.len() as u32;
            mesh.verts.extend(m.verts);
            mesh.tris.extend(m.tris.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
        }
        Ok(ElementMesh { element: el, mesh })
    }).collect();
    walls.into_iter().chain(slabs).chain(columns).chain(items).collect()
}
