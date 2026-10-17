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
use openbim_family::{forms::Loop, mep::up_for, Axis, Form, FormGeom, Instance, System};
use openbim_place::penetrations::{Run, RunSize};
use openbim_route::Fitting;
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
    /// what could not be drawn of this element (for instance a revolve in a family)
    pub warnings: Vec<String>,
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

const CIRCLE_SIDES: usize = 32;

fn loop_points(l: &Loop) -> Vec<P> {
    match l {
        Loop::Poly(p) => p.clone(),
        Loop::Circle { c, r } => (0..CIRCLE_SIDES).map(|k| { let a = std::f64::consts::TAU * k as f64 / CIRCLE_SIDES as f64; [c[0] + r * a.cos(), c[1] + r * a.sin()] }).collect(),
    }
}

/// An extrusion form in the model. None for revolves and sweeps, which are not drawn yet.
fn form_mesh(frame: &Frame, f: &Form) -> Option<Mesh> {
    let FormGeom::Extrusion { plane, from, to } = f.geom else { return None };
    let outer = loop_points(&f.outer);
    let holes: Vec<Vec<P>> = f.holes.iter().map(loop_points).collect();
    let mut m = prism(&outer, &holes, 0.0, 1.0)?;
    for v in &mut m.verts {
        let (u, w, t) = (v[0], v[1], v[2]);
        let off = from + t * (to - from);
        let q = match plane { Axis::Z => [u, w, off], Axis::X => [off, u, w], Axis::Y => [w, off, u] };
        let xy = f.xf.apply([q[0], q[1]]);
        let g = frame.point([xy[0], xy[1], 0.0]);
        *v = [g[0], g[1], frame.origin[2] + f.dz + q[2]];
    }
    if m.volume() < 0.0 { for t in &mut m.tris { t.swap(1, 2); } } // the plane may be left-handed, or `to` below `from`
    Some(m)
}

/// Meshes of the solids and extrusions of a family item, each triangle tagged with its material.
fn family_parts(frame: &Frame, inst: &Instance) -> (Mesh, Vec<Part>, Vec<String>) {
    let mut mesh = Mesh::default();
    let mut mat_of_tri: Vec<Option<&str>> = Vec::new();
    let mut warnings = Vec::new();
    fn add<'a>(m: Mesh, mat: Option<&'a str>, mesh: &mut Mesh, mats: &mut Vec<Option<&'a str>>) {
        let base = mesh.verts.len() as u32;
        mesh.verts.extend(m.verts);
        for t in m.tris { mesh.tris.push([t[0] + base, t[1] + base, t[2] + base]); mats.push(mat); }
    }
    for (k, _) in inst.solids.iter().enumerate() {
        let one = Instance { solids: vec![inst.solids[k]], xf: vec![inst.xf.get(k).copied().flatten()], ..inst.clone() };
        let mat = inst.materials.get(k).copied().flatten().and_then(|i| inst.strings.get(i as usize)).map(|s| s.as_str());
        add(family_mesh(frame, &one), mat, &mut mesh, &mut mat_of_tri);
    }
    for (k, f) in inst.forms.iter().enumerate() {
        let mat = inst.form_materials.get(k).copied().flatten().and_then(|i| inst.strings.get(i as usize)).map(|s| s.as_str());
        match form_mesh(frame, f) {
            Some(m) => add(m, mat, &mut mesh, &mut mat_of_tri),
            None => warnings.push(match f.geom { FormGeom::Revolve { .. } => "a revolve is not drawn in 3D yet", _ => "a sweep is not drawn in 3D yet" }.to_string()),
        }
    }
    warnings.dedup();
    let tag = mesh.tris.iter().zip(&mat_of_tri).map(|(&t, &m)| (vertical_kind(normal(&mesh, t)), m)).collect();
    let (mesh, parts) = group(mesh, tag);
    (mesh, parts, warnings)
}

// ------------------------------------------------------------------ service networks

type V3 = [f64; 3];
fn sub3(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn cross3(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn unit3(a: V3) -> V3 { let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt(); if l == 0.0 { a } else { [a[0] / l, a[1] / l, a[2] / l] } }

/// A direction square to `d` (the same rule as the IFC export uses for round sections).
fn perp(d: V3) -> V3 {
    let h = if d[0].abs() <= d[1].abs() && d[0].abs() <= d[2].abs() { [1.0, 0.0, 0.0] } else if d[1].abs() <= d[2].abs() { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
    unit3(cross3(d, h))
}

/// A straight piece with the section `size` from `at` along the unit vector `d` for `len` metres: a cylinder
/// (an n-gon prism) or a box. Triangles face outward.
fn straight_piece(size: RunSize, at: V3, d: V3, len: f64) -> Option<Mesh> {
    if len.is_nan() || len <= 1e-9 { return None; }
    let (x, ring): (V3, Vec<[f64; 2]>) = match size {
        RunSize::Round { diameter } => (perp(d), loop_points(&Loop::Circle { c: [0.0, 0.0], r: diameter / 2.0 })),
        RunSize::Rect { width, height, roll } => {
            let (w, h) = (width / 2.0, height / 2.0);
            (unit3(cross3(up_for(d, roll), d)), vec![[-w, -h], [w, -h], [w, h], [-w, h]])
        }
    };
    let y = cross3(d, x);
    let n = ring.len() as u32;
    let mut m = Mesh::default();
    for k in 0..2 {
        let o = [at[0] + d[0] * len * k as f64, at[1] + d[1] * len * k as f64, at[2] + d[2] * len * k as f64];
        for q in &ring { m.verts.push([o[0] + x[0] * q[0] + y[0] * q[1], o[1] + x[1] * q[0] + y[1] * q[1], o[2] + x[2] * q[0] + y[2] * q[1]]); }
    }
    for i in 0..n {
        let j = (i + 1) % n;
        m.tris.push([i, j, j + n]);
        m.tris.push([i, j + n, i + n]);
    }
    for i in 1..n - 1 {
        m.tris.push([n, n + i, n + i + 1]); // far end, facing along d
        m.tris.push([0, i + 1, i]); // near end
    }
    if m.volume() < 0.0 { for t in &mut m.tris { t.swap(1, 2); } }
    Some(m)
}

fn run_mesh(r: &Run) -> Option<Mesh> {
    let d = sub3(r.b, r.a);
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    straight_piece(r.size, r.a, unit3(d), len)
}

/// A fitting is drawn as the short stubs it takes from each leg, as in the IFC export.
fn fitting_mesh(f: &Fitting) -> Option<Mesh> {
    let mut out = Mesh::default();
    for k in 0..f.ports.len() {
        let Some(m) = straight_piece(f.port_sizes[k], f.at, unit3(f.ports[k]), f.trim[k]) else { continue };
        let base = out.verts.len() as u32;
        out.verts.extend(m.verts);
        out.tris.extend(m.tris.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
    }
    if out.tris.is_empty() { None } else { Some(out) }
}

/// One part, one material: the system the piece belongs to.
fn one_part(mesh: Mesh, system: System) -> (Mesh, Vec<Part>) {
    let n = mesh.tris.len();
    (mesh, vec![Part { face: FaceKind::Side, material: Some(system.ifc_system().to_string()), tris: 0..n }])
}

// ------------------------------------------------------------------ inputs and cache

/// The inputs of one element's mesh, borrowed from the scene.
enum Src<'a> {
    Wall(&'a WallChain, WallMaterials),
    Slab(&'a Slab, Vec<Vec<P>>),
    Column(&'a [P], f64, f64),
    Family(&'a Frame, &'a Instance),
    Run(&'a Run),
    Fitting(&'a Fitting, System),
}

/// The same inputs, kept by the cache.
enum Key {
    Wall(WallChain, WallMaterials),
    Slab(Slab, Vec<Vec<P>>),
    Column(Vec<P>, f64, f64),
    Family(Frame, Instance),
    Run(Run),
    Fitting(Fitting, System),
}

impl Src<'_> {
    fn same(&self, k: &Key) -> bool {
        match (self, k) {
            (Src::Wall(a, m), Key::Wall(b, n)) => *a == b && m == n,
            (Src::Slab(a, h), Key::Slab(b, g)) => *a == b && h == g,
            (Src::Column(a, z, t), Key::Column(b, y, u)) => *a == b.as_slice() && z == y && t == u,
            (Src::Family(a, i), Key::Family(b, j)) => *a == b && *i == j,
            (Src::Run(a), Key::Run(b)) => *a == b,
            (Src::Fitting(a, s), Key::Fitting(b, t)) => *a == b && s == t,
            _ => false,
        }
    }
    fn own(&self) -> Key {
        match self {
            Src::Wall(c, m) => Key::Wall((*c).clone(), m.clone()),
            Src::Slab(s, h) => Key::Slab((*s).clone(), h.clone()),
            Src::Column(f, a, b) => Key::Column(f.to_vec(), *a, *b),
            Src::Family(f, i) => Key::Family(**f, (*i).clone()),
            Src::Run(r) => Key::Run((*r).clone()),
            Src::Fitting(f, s) => Key::Fitting((*f).clone(), *s),
        }
    }
    fn build(&self, el: Element) -> Result<ElementMesh, ViewError> {
        let bad = || ViewError::Mesh { id: el.id };
        let mut warnings = Vec::new();
        let (mesh, parts) = match self {
            Src::Wall(c, m) => wall_mesh(c, m).map_err(|err| ViewError::Wall { id: el.id, err })?,
            Src::Slab(s, holes) => plain(prism(&s.outline, holes, s.top - s.thickness, s.top).ok_or_else(bad)?),
            Src::Column(f, z0, z1) => plain(prism(f, &[], *z0, *z1).ok_or_else(bad)?),
            Src::Family(f, i) => { let (m, p, w) = family_parts(f, i); warnings = w; (m, p) }
            Src::Run(r) => one_part(run_mesh(r).ok_or_else(bad)?, r.system),
            Src::Fitting(f, s) => match fitting_mesh(f) { Some(m) => one_part(m, *s), None => (Mesh::default(), Vec::new()) },
        };
        Ok(ElementMesh { element: el, mesh, parts, warnings })
    }
}

/// Every element with its inputs. The number beside the element is the network it belongs to (0 for everything that is
/// not a service piece): piece numbers are only unique within one network.
fn sources(scene: &Scene) -> Vec<(Element, u32, Src<'_>)> {
    let mut v = Vec::new();
    for w in &scene.walls { v.push((Element { kind: Kind::Wall, id: w.id }, 0, Src::Wall(&w.chain, wall_materials(scene, w.wall_type)))); }
    for (i, s) in scene.slabs.iter().enumerate() { v.push((Element { kind: Kind::Slab, id: s.id }, 0, Src::Slab(&s.slab, slab_holes(scene, i, &s.slab)))); }
    for c in &scene.columns { v.push((Element { kind: Kind::Column, id: c.id }, 0, Src::Column(&c.footprint, c.base_z, c.base_z + c.height))); }
    for it in &scene.items { v.push((Element { kind: Kind::Family, id: it.id }, 0, Src::Family(&it.frame, &it.instance))); }
    for (ni, net) in scene.networks.iter().enumerate() {
        for sg in &net.segments { v.push((Element { kind: Kind::Segment, id: sg.id }, ni as u32, Src::Run(&sg.run))); }
        for nf in &net.fittings { v.push((Element { kind: Kind::Fitting, id: nf.id }, ni as u32, Src::Fitting(&nf.fitting, nf.system))); }
    }
    v
}

/// Meshes of every wall, slab, column and family solid in the scene.
pub fn scene_meshes(scene: &Scene) -> Result<Vec<ElementMesh>, ViewError> {
    sources(scene).par_iter().map(|(el, _, s)| s.build(*el)).collect::<Vec<_>>().into_iter().collect()
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
    entries: HashMap<(Element, u32), (Key, Arc<ElementMesh>)>,
}

impl MeshCache {
    pub fn new() -> MeshCache { MeshCache::default() }
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    /// Bring the cache to the state of `scene`, meshing only what changed. On an error the cache is left as it was.
    pub fn update(&mut self, scene: &Scene) -> Result<Update, ViewError> {
        let src = sources(scene);
        let built: Vec<Option<Result<ElementMesh, ViewError>>> = src.par_iter().map(|(el, own, s)| {
            if self.entries.get(&(*el, *own)).is_some_and(|(k, _)| s.same(k)) { None } else { Some(s.build(*el)) }
        }).collect();
        let mut fresh = Vec::new();
        for b in built.iter().flatten() { if let Err(e) = b { return Err(e.clone()); } }
        let mut meshes = Vec::with_capacity(src.len());
        let mut rebuilt = Vec::new();
        for ((el, own, s), b) in src.iter().zip(built) {
            match b {
                None => meshes.push(self.entries[&(*el, *own)].1.clone()),
                Some(r) => {
                    let m = Arc::new(r.expect("errors returned above"));
                    meshes.push(m.clone());
                    fresh.push(((*el, *own), s.own(), m));
                    rebuilt.push(*el);
                }
            }
        }
        let now: HashSet<(Element, u32)> = src.iter().map(|(e, o, _)| (*e, *o)).collect();
        let mut gone: Vec<(Element, u32)> = self.entries.keys().filter(|e| !now.contains(e)).copied().collect();
        gone.sort_by_key(|(e, o)| (*o, e.id, format!("{:?}", e.kind)));
        for e in &gone { self.entries.remove(e); }
        let removed: Vec<Element> = gone.into_iter().map(|(e, _)| e).collect();
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
