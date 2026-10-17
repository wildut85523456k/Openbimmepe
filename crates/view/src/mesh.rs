//! Triangle meshes of the model, one per element, for a 3D viewport.
//!
//! Every mesh is split into parts: runs of triangles that share a face kind (top, bottom, side, or for walls the
//! exterior and interior face) and a material, so the viewport can colour them and the pick buffer can name the
//! element. `Batch` joins the meshes into one set of buffers with an element index per triangle.
//!
//! `MeshCache` keeps the meshes between edits. It decides what to rebuild by comparing the inputs of each element
//! with the ones it meshed last time, so it does not matter how the scene was changed: only elements whose inputs
//! differ are meshed again, and meshing is done in parallel. Output order is walls, slabs, columns, family items,
//! each in scene order, and does not depend on the number of threads.

use crate::{slab_holes, Element, Kind, ViewError};
use openbim_family::Instance;
use openbim_geom::{Mesh, WallChain};
use openbim_ifc::{Frame, Scene};
use openbim_place::floors::Slab;
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

type P = [f64; 2];

/// What a run of triangles is. Walls have an exterior face (left of the direction of travel, as in the wall type)
/// and an interior face; their ends and the sides of openings are `Edge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FaceKind {
    Top,
    Bottom,
    Side,
    Exterior,
    Interior,
    Edge,
}

/// A run of triangles of one mesh with one face kind and one material.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub face: FaceKind,
    pub material: Option<String>,
    pub tris: Range<usize>,
}

/// The mesh of one element. Triangles face outward and are grouped by `parts`, which cover all of them.
#[derive(Debug, Clone)]
pub struct ElementMesh {
    pub element: Element,
    pub mesh: Mesh,
    pub parts: Vec<Part>,
}

// ------------------------------------------------------------------ geometry

fn area2(r: &[P]) -> f64 {
    (0..r.len()).map(|k| { let (a, b) = (r[k], r[(k + 1) % r.len()]); a[0] * b[1] - b[0] * a[1] }).sum()
}

fn oriented(r: &[P], ccw: bool) -> Vec<P> {
    let mut v = r.to_vec();
    if (area2(&v) > 0.0) != ccw { v.reverse(); }
    v
}

/// A prism between heights z0 and z1 on `outer`, with vertical holes. None if the outline cannot be triangulated.
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

fn normal(m: &Mesh, t: [u32; 3]) -> [f64; 3] {
    let (a, b, c) = (m.verts[t[0] as usize], m.verts[t[1] as usize], m.verts[t[2] as usize]);
    let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l < 1e-12 { [0.0; 3] } else { [n[0] / l, n[1] / l, n[2] / l] }
}

fn vertical_kind(n: [f64; 3]) -> FaceKind {
    if n[2] > 0.5 { FaceKind::Top } else if n[2] < -0.5 { FaceKind::Bottom } else { FaceKind::Side }
}

/// Reorder the triangles of `mesh` so that those with the same (face kind, material) are one run.
/// `tag[i]` is the kind and the material of triangle i. Within a run the order is kept.
fn group(mesh: Mesh, tag: Vec<(FaceKind, Option<&str>)>) -> (Mesh, Vec<Part>) {
    let mut mats: Vec<Option<&str>> = Vec::new();
    let mut runs: BTreeMap<(FaceKind, usize), Vec<[u32; 3]>> = BTreeMap::new();
    for (t, (face, mat)) in mesh.tris.iter().zip(&tag) {
        let k = match mats.iter().position(|m| m == mat) { Some(k) => k, None => { mats.push(*mat); mats.len() - 1 } };
        runs.entry((*face, k)).or_default().push(*t);
    }
    let mut tris = Vec::with_capacity(mesh.tris.len());
    let mut parts = Vec::new();
    for ((face, k), v) in runs {
        let start = tris.len();
        tris.extend(v);
        parts.push(Part { face, material: mats[k].map(str::to_string), tris: start..tris.len() });
    }
    (Mesh { verts: mesh.verts, tris }, parts)
}

fn plain(mesh: Mesh) -> (Mesh, Vec<Part>) {
    let tag = mesh.tris.iter().map(|&t| (vertical_kind(normal(&mesh, t)), None)).collect();
    group(mesh, tag)
}

/// Exterior and interior material of a wall, and the material of its edges.
type WallMaterials = [Option<String>; 3];

fn wall_materials(scene: &Scene, wall_type: Option<usize>) -> WallMaterials {
    let Some(t) = wall_type.and_then(|k| scene.wall_types.get(k)) else { return [None, None, None] };
    let (c0, _) = t.core_range();
    [t.layers.first().map(|l| l.material.clone()), t.layers.last().map(|l| l.material.clone()), t.layers.get(c0).map(|l| l.material.clone())]
}

fn wall_mesh(chain: &WallChain, mats: &WallMaterials) -> Result<(Mesh, Vec<Part>), openbim_geom::GeomError> {
    let mesh = chain.chain_mesh()?;
    let n = chain.points.len();
    let segs = if chain.closed { n } else { n.saturating_sub(1) };
    let mut tag: Vec<(FaceKind, Option<&str>)> = Vec::with_capacity(mesh.tris.len());
    for &t in &mesh.tris {
        let nm = normal(&mesh, t);
        let kind = if nm[2].abs() > 0.5 {
            if nm[2] > 0.0 { FaceKind::Top } else { FaceKind::Bottom }
        } else {
            // the face belongs to the segment whose centreline is nearest to its centre
            let c = [0, 1].map(|k| (mesh.verts[t[0] as usize][k] + mesh.verts[t[1] as usize][k] + mesh.verts[t[2] as usize][k]) / 3.0);
            let mut best = (f64::MAX, [0.0, 0.0]);
            for i in 0..segs {
                let (a, b) = (chain.points[i], chain.points[(i + 1) % n]);
                let d = [b[0] - a[0], b[1] - a[1]];
                let l2 = d[0] * d[0] + d[1] * d[1];
                if l2 <= 0.0 { continue; }
                let s = (((c[0] - a[0]) * d[0] + (c[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0);
                let dist = (a[0] + d[0] * s - c[0]).hypot(a[1] + d[1] * s - c[1]);
                if dist < best.0 - 1e-9 { best = (dist, [-d[1] / l2.sqrt(), d[0] / l2.sqrt()]); }
            }
            let side = nm[0] * best.1[0] + nm[1] * best.1[1];
            if side > 0.7 { FaceKind::Exterior } else if side < -0.7 { FaceKind::Interior } else { FaceKind::Edge }
        };
        let mat = match kind { FaceKind::Exterior => &mats[0], FaceKind::Interior => &mats[1], _ => &mats[2] };
        tag.push((kind, mat.as_deref()));
    }
    Ok(group(mesh, tag))
}

fn family_mesh(frame: &Frame, inst: &Instance) -> Mesh {
    let mut out = Mesh::default();
    for (k, (_, bx)) in inst.solids.iter().enumerate() {
        let xf = inst.xf.get(k).copied().flatten();
        let ring: Vec<P> = [[bx.min[0], bx.min[1]], [bx.max[0], bx.min[1]], [bx.max[0], bx.max[1]], [bx.min[0], bx.max[1]]].iter().map(|&q| {
            let q = match xf { Some(t) => t.apply(q), None => q };
            let w = frame.point([q[0], q[1], 0.0]);
            [w[0], w[1]]
        }).collect();
        let Some(m) = prism(&ring, &[], frame.origin[2] + bx.min[2], frame.origin[2] + bx.max[2]) else { continue };
        let base = out.verts.len() as u32;
        out.verts.extend(m.verts);
        out.tris.extend(m.tris.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
    }
    out
}

/// Meshes of the solids of a family item, each triangle tagged with the material of its solid.
fn family_parts(frame: &Frame, inst: &Instance) -> (Mesh, Vec<Part>) {
    let mut mesh = Mesh::default();
    let mut mat_of_tri: Vec<Option<&str>> = Vec::new();
    for (k, _) in inst.solids.iter().enumerate() {
        let one = Instance { solids: vec![inst.solids[k]], xf: vec![inst.xf.get(k).copied().flatten()], ..inst.clone() };
        let m = family_mesh(frame, &one);
        let mat = inst.materials.get(k).copied().flatten().and_then(|i| inst.strings.get(i as usize)).map(|s| s.as_str());
        let base = mesh.verts.len() as u32;
        mesh.verts.extend(m.verts);
        for t in m.tris { mesh.tris.push([t[0] + base, t[1] + base, t[2] + base]); mat_of_tri.push(mat); }
    }
    let tag = mesh.tris.iter().zip(&mat_of_tri).map(|(&t, &m)| (vertical_kind(normal(&mesh, t)), m)).collect();
    group(mesh, tag)
}

// ------------------------------------------------------------------ inputs and cache

/// The inputs of one element's mesh, borrowed from the scene.
enum Src<'a> {
    Wall(&'a WallChain, WallMaterials),
    Slab(&'a Slab, Vec<Vec<P>>),
    Column(&'a [P], f64, f64),
    Family(&'a Frame, &'a Instance),
}

/// The same inputs, kept by the cache.
enum Key {
    Wall(WallChain, WallMaterials),
    Slab(Slab, Vec<Vec<P>>),
    Column(Vec<P>, f64, f64),
    Family(Frame, Instance),
}

impl Src<'_> {
    fn same(&self, k: &Key) -> bool {
        match (self, k) {
            (Src::Wall(a, m), Key::Wall(b, n)) => *a == b && m == n,
            (Src::Slab(a, h), Key::Slab(b, g)) => *a == b && h == g,
            (Src::Column(a, z, t), Key::Column(b, y, u)) => *a == b.as_slice() && z == y && t == u,
            (Src::Family(a, i), Key::Family(b, j)) => *a == b && *i == j,
            _ => false,
        }
    }
    fn own(&self) -> Key {
        match self {
            Src::Wall(c, m) => Key::Wall((*c).clone(), m.clone()),
            Src::Slab(s, h) => Key::Slab((*s).clone(), h.clone()),
            Src::Column(f, a, b) => Key::Column(f.to_vec(), *a, *b),
            Src::Family(f, i) => Key::Family(**f, (*i).clone()),
        }
    }
    fn build(&self, el: Element) -> Result<ElementMesh, ViewError> {
        let bad = || ViewError::Mesh { id: el.id };
        let (mesh, parts) = match self {
            Src::Wall(c, m) => wall_mesh(c, m).map_err(|err| ViewError::Wall { id: el.id, err })?,
            Src::Slab(s, holes) => plain(prism(&s.outline, holes, s.top - s.thickness, s.top).ok_or_else(bad)?),
            Src::Column(f, z0, z1) => plain(prism(f, &[], *z0, *z1).ok_or_else(bad)?),
            Src::Family(f, i) => family_parts(f, i),
        };
        Ok(ElementMesh { element: el, mesh, parts })
    }
}

fn sources(scene: &Scene) -> Vec<(Element, Src<'_>)> {
    let mut v = Vec::new();
    for w in &scene.walls { v.push((Element { kind: Kind::Wall, id: w.id }, Src::Wall(&w.chain, wall_materials(scene, w.wall_type)))); }
    for (i, s) in scene.slabs.iter().enumerate() { v.push((Element { kind: Kind::Slab, id: s.id }, Src::Slab(&s.slab, slab_holes(scene, i, &s.slab)))); }
    for c in &scene.columns { v.push((Element { kind: Kind::Column, id: c.id }, Src::Column(&c.footprint, c.base_z, c.base_z + c.height))); }
    for it in &scene.items { v.push((Element { kind: Kind::Family, id: it.id }, Src::Family(&it.frame, &it.instance))); }
    v
}

/// Meshes of every wall, slab, column and family solid in the scene.
pub fn scene_meshes(scene: &Scene) -> Result<Vec<ElementMesh>, ViewError> {
    sources(scene).par_iter().map(|(el, s)| s.build(*el)).collect::<Vec<_>>().into_iter().collect()
}

/// What `MeshCache::update` found.
#[derive(Debug, Clone)]
pub struct Update {
    /// every element's mesh, in scene order; unchanged ones are the same `Arc` as before
    pub meshes: Vec<Arc<ElementMesh>>,
    /// elements meshed again (new or changed)
    pub rebuilt: Vec<Element>,
    /// elements that were in the cache and are no longer in the scene
    pub removed: Vec<Element>,
}

/// Meshes kept between edits; see the module notes.
#[derive(Default)]
pub struct MeshCache {
    entries: HashMap<Element, (Key, Arc<ElementMesh>)>,
}

impl MeshCache {
    pub fn new() -> MeshCache { MeshCache::default() }
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    /// Bring the cache to the state of `scene`, meshing only what changed. On an error the cache is left as it was.
    pub fn update(&mut self, scene: &Scene) -> Result<Update, ViewError> {
        let src = sources(scene);
        let built: Vec<Option<Result<ElementMesh, ViewError>>> = src.par_iter().map(|(el, s)| {
            if self.entries.get(el).is_some_and(|(k, _)| s.same(k)) { None } else { Some(s.build(*el)) }
        }).collect();
        let mut fresh = Vec::new();
        for b in built.iter().flatten() { if let Err(e) = b { return Err(e.clone()); } }
        let mut meshes = Vec::with_capacity(src.len());
        let mut rebuilt = Vec::new();
        for ((el, s), b) in src.iter().zip(built) {
            match b {
                None => meshes.push(self.entries[el].1.clone()),
                Some(r) => {
                    let m = Arc::new(r.expect("errors returned above"));
                    meshes.push(m.clone());
                    fresh.push((*el, s.own(), m));
                    rebuilt.push(*el);
                }
            }
        }
        let now: HashSet<Element> = src.iter().map(|(e, _)| *e).collect();
        let removed: Vec<Element> = { let mut r: Vec<Element> = self.entries.keys().filter(|e| !now.contains(e)).copied().collect(); r.sort_by_key(|e| (e.id, format!("{:?}", e.kind))); r };
        for e in &removed { self.entries.remove(e); }
        for (el, k, m) in fresh { self.entries.insert(el, (k, m)); }
        Ok(Update { meshes, rebuilt, removed })
    }
}

// ------------------------------------------------------------------ batch for the GPU

/// All meshes joined into one set of buffers, in single precision relative to `origin`.
#[derive(Debug, Clone, Default)]
pub struct Batch {
    /// position of (0, 0, 0) of `verts` in model coordinates
    pub origin: [f64; 3],
    pub verts: Vec<[f32; 3]>,
    pub tris: Vec<[u32; 3]>,
    /// for each triangle: index into `elements`
    pub tri_element: Vec<u32>,
    pub tri_face: Vec<FaceKind>,
    /// for each triangle: index into `materials`, or `u32::MAX` for none
    pub tri_material: Vec<u32>,
    pub elements: Vec<Element>,
    /// material names, in order of first use
    pub materials: Vec<String>,
}

impl Batch {
    pub fn new(meshes: &[Arc<ElementMesh>]) -> Batch {
        let mut origin = [f64::MAX; 3];
        for m in meshes { for v in &m.mesh.verts { for k in 0..3 { origin[k] = origin[k].min(v[k]); } } }
        if origin[0] == f64::MAX { origin = [0.0; 3]; }
        let mut b = Batch { origin, ..Batch::default() };
        let mut mat_index: HashMap<&str, u32> = HashMap::new();
        for (ei, m) in meshes.iter().enumerate() {
            let base = b.verts.len() as u32;
            b.verts.extend(m.mesh.verts.iter().map(|v| [(v[0] - origin[0]) as f32, (v[1] - origin[1]) as f32, (v[2] - origin[2]) as f32]));
            b.elements.push(m.element);
            for part in &m.parts {
                let mi = match &part.material {
                    None => u32::MAX,
                    Some(name) => *mat_index.entry(name.as_str()).or_insert_with(|| { b.materials.push(name.clone()); b.materials.len() as u32 - 1 }),
                };
                for t in &m.mesh.tris[part.tris.clone()] {
                    b.tris.push([t[0] + base, t[1] + base, t[2] + base]);
                    b.tri_element.push(ei as u32);
                    b.tri_face.push(part.face);
                    b.tri_material.push(mi);
                }
            }
        }
        b
    }
}
