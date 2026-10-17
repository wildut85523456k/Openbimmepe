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
use openbim_route::{Fitting, FittingKind};
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

/// Rings of a profile with the outer one counter-clockwise and the holes clockwise, whatever they were.
fn profile_rings(f: &Form) -> Vec<Vec<P>> {
    std::iter::once(oriented(&loop_points(&f.outer), true)).chain(f.holes.iter().map(|h| oriented(&loop_points(h), false))).collect()
}

fn rotate_vec(v: V3, axis: V3, t: f64) -> V3 {
    let (s, c) = (t.sin(), t.cos());
    let cr = cross3(axis, v);
    let d = axis[0] * v[0] + axis[1] * v[1] + axis[2] * v[2];
    [v[0] * c + cr[0] * s + axis[0] * d * (1.0 - c), v[1] * c + cr[1] * s + axis[1] * d * (1.0 - c), v[2] * c + cr[2] * s + axis[2] * d * (1.0 - c)]
}
fn dot3(a: V3, b: V3) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }

/// A solid made by carrying a profile through `steps` positions. `rings[r][j][k]` is point k of ring r at position j
/// (ring 0 the outer boundary, counter-clockwise; the others holes, clockwise); `out(r, k, j)` is a vector that points
/// out of the solid at the side face between point k and k + 1 of ring r at position j; `wrap` joins the last position
/// to the first (a full turn); otherwise the ends are closed with caps, triangulated from `profile` (the 2D rings),
/// where `cap_out(is_end, centre)` says which way a cap faces.
fn skin(rings: &[Vec<Vec<V3>>], profile: &[Vec<P>], out: &dyn Fn(usize, usize, usize) -> V3, wrap: bool, cap_out: &dyn Fn(bool, V3) -> V3) -> Option<Mesh> {
    let steps = rings[0].len();
    let mut mesh = Mesh::default();
    let mut base = Vec::new();
    for r in rings {
        base.push(mesh.verts.len() as u32);
        for step in r { mesh.verts.extend(step.iter().copied()); }
    }
    let vid = |r: usize, j: usize, k: usize| base[r] + (j * rings[r][0].len() + k) as u32;
    let segs = if wrap { steps } else { steps - 1 };
    for (r, ring) in rings.iter().enumerate() {
        let n = ring[0].len();
        for j in 0..segs {
            let j2 = (j + 1) % steps;
            for k in 0..n {
                let k2 = (k + 1) % n;
                let (a, b, c, d) = (vid(r, j, k), vid(r, j, k2), vid(r, j2, k2), vid(r, j2, k));
                let (pa, pb, pd) = (mesh.verts[a as usize], mesh.verts[b as usize], mesh.verts[d as usize]);
                let nrm = cross3(sub3(pb, pa), sub3(pd, pa));
                let flip = dot3(nrm, out(r, k, j)) < 0.0;
                let quad = if flip { [[a, d, c], [a, c, b]] } else { [[a, b, c], [a, c, d]] };
                mesh.tris.extend(quad);
            }
        }
    }
    if !wrap {
        let pts: Vec<P> = profile.iter().flatten().copied().collect();
        let mut starts = Vec::new();
        let mut n = profile[0].len();
        for h in &profile[1..] { starts.push(n); n += h.len(); }
        let flat: Vec<f64> = pts.iter().flat_map(|p| [p[0], p[1]]).collect();
        let idx = earcutr::earcut(&flat, &starts, 2).ok()?;
        if idx.len() % 3 != 0 || idx.iter().any(|&k| k >= pts.len()) { return None; }
        let locate = |flat_k: usize, j: usize| { let (mut r, mut k) = (0, flat_k); while k >= profile[r].len() { k -= profile[r].len(); r += 1; } vid(r, j, k) };
        for is_end in [false, true] {
            let j = if is_end { steps - 1 } else { 0 };
            for t in idx.chunks(3) {
                let v = [locate(t[0], j), locate(t[1], j), locate(t[2], j)];
                let p = v.map(|i| mesh.verts[i as usize]);
                let centre = [0, 1, 2].map(|c| (p[0][c] + p[1][c] + p[2][c]) / 3.0);
                let nrm = cross3(sub3(p[1], p[0]), sub3(p[2], p[0]));
                let want = cap_out(is_end, centre);
                let flip = dot3(nrm, want) < 0.0;
                mesh.tris.push(if flip { [v[0], v[2], v[1]] } else { v });
            }
        }
    }
    Some(mesh)
}

/// A revolve, in the form's own frame.
fn revolve_local(f: &Form) -> Option<Mesh> {
    let FormGeom::Revolve { plane, offset, about_v, at, angle } = f.geom else { return None };
    let (ud, vd): (V3, V3) = match plane { Axis::Z => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), Axis::X => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), Axis::Y => ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]) };
    let to3 = |u: f64, v: f64| match plane { Axis::Z => [u, v, offset], Axis::X => [offset, u, v], Axis::Y => [v, offset, u] };
    let (axis_pt, axis) = if about_v { (to3(at, 0.0), vd) } else { (to3(0.0, at), ud) };
    let full = angle >= 360.0 - 1e-9;
    let steps = ((angle / 7.5).ceil() as usize).clamp(2, 96);
    let n = if full { steps } else { steps + 1 };
    let profile = profile_rings(f);
    let at_step = |p: V3, j: usize| -> V3 {
        let t = angle.to_radians() * j as f64 / steps as f64;
        let r = rotate_vec(sub3(p, axis_pt), axis, t);
        [axis_pt[0] + r[0], axis_pt[1] + r[1], axis_pt[2] + r[2]]
    };
    let rings: Vec<Vec<Vec<V3>>> = profile.iter().map(|ring| (0..n).map(|j| ring.iter().map(|q| at_step(to3(q[0], q[1]), j)).collect()).collect()).collect();
    let out = |r: usize, k: usize, j: usize| -> V3 {
        let ring = &profile[r];
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        let right = [b[1] - a[1], -(b[0] - a[0])];
        let w = [ud[0] * right[0] + vd[0] * right[1], ud[1] * right[0] + vd[1] * right[1], ud[2] * right[0] + vd[2] * right[1]];
        rotate_vec(w, axis, angle.to_radians() * j as f64 / steps as f64)
    };
    let cap_out = |is_end: bool, c: V3| -> V3 {
        let t = cross3(axis, sub3(c, axis_pt));
        if is_end { t } else { [-t[0], -t[1], -t[2]] }
    };
    skin(&rings, &profile, &out, full, &cap_out)
}

/// A sweep, in the form's own frame: the profile (u to the left of travel, v up) carried along the plan path at height z,
/// mitred at the bends.
fn sweep_local(f: &Form) -> Option<Mesh> {
    let FormGeom::Sweep { z, ref path } = f.geom else { return None };
    if path.len() < 2 { return None; }
    let nrm = |a: P, b: P| { let d = [b[0] - a[0], b[1] - a[1]]; let l = d[0].hypot(d[1]); if l == 0.0 { None } else { Some([-d[1] / l, d[0] / l]) } };
    let lefts: Vec<P> = path.windows(2).map(|w| nrm(w[0], w[1])).collect::<Option<_>>()?;
    let n = path.len();
    let mitre: Vec<P> = (0..n).map(|i| {
        if i == 0 { lefts[0] } else if i == n - 1 { lefts[n - 2] } else {
            let (a, b) = (lefts[i - 1], lefts[i]);
            let k = 1.0 + a[0] * b[0] + a[1] * b[1];
            [(a[0] + b[0]) / k, (a[1] + b[1]) / k]
        }
    }).collect();
    let profile = profile_rings(f);
    let rings: Vec<Vec<Vec<V3>>> = profile.iter().map(|ring| (0..n).map(|i| ring.iter().map(|q| [path[i][0] + q[0] * mitre[i][0], path[i][1] + q[0] * mitre[i][1], z + q[1]]).collect()).collect()).collect();
    let out = |r: usize, k: usize, j: usize| -> V3 {
        let ring = &profile[r];
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        let right = [b[1] - a[1], -(b[0] - a[0])];
        let l = lefts[j];
        [l[0] * right[0], l[1] * right[0], right[1]]
    };
    let dirs: Vec<V3> = lefts.iter().map(|l| [l[1], -l[0], 0.0]).collect(); // travel = left turned right
    let cap_out = move |is_end: bool, _c: V3| -> V3 {
        if is_end { dirs[dirs.len() - 1] } else { let d = dirs[0]; [-d[0], -d[1], -d[2]] }
    };
    skin(&rings, &profile, &out, false, &cap_out)
}

/// A form in the model: an extrusion, a revolve or a sweep.
fn form_mesh(frame: &Frame, f: &Form) -> Option<Mesh> {
    let mut m = match f.geom {
        FormGeom::Extrusion { plane, from, to } => {
            let outer = loop_points(&f.outer);
            let holes: Vec<Vec<P>> = f.holes.iter().map(loop_points).collect();
            let mut m = prism(&outer, &holes, 0.0, 1.0)?;
            for v in &mut m.verts {
                let (u, w, t) = (v[0], v[1], v[2]);
                let off = from + t * (to - from);
                *v = match plane { Axis::Z => [u, w, off], Axis::X => [off, u, w], Axis::Y => [w, off, u] };
            }
            if m.volume() < 0.0 { for t in &mut m.tris { t.swap(1, 2); } } // the plane may be left-handed, or `to` below `from`
            m
        }
        FormGeom::Revolve { .. } => revolve_local(f)?,
        FormGeom::Sweep { .. } => sweep_local(f)?,
    };
    for v in &mut m.verts {
        let xy = f.xf.apply([v[0], v[1]]);
        let g = frame.point([xy[0], xy[1], 0.0]);
        *v = [g[0], g[1], frame.origin[2] + f.dz + v[2]];
    }
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
            None => warnings.push("a form of this family could not be meshed".to_string()),
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

/// An elbow as a bend: the section carried round the arc of radius `f.radius` between the tangent points of its two
/// legs. None when the fitting does not describe a consistent arc, so that the caller draws stubs instead.
fn elbow_bend(f: &Fitting) -> Option<Mesh> {
    if !matches!(f.kind, FittingKind::Elbow { .. }) || f.ports.len() != 2 || f.radius <= 1e-9 || f.trim.len() != 2 || f.trim.iter().any(|t| t.is_nan() || *t <= 1e-9) { return None; }
    let (u0, u1) = (unit3(f.ports[0]), unit3(f.ports[1]));
    let travel = [-u0[0], -u0[1], -u0[2]]; // direction of flow into the fitting along leg 0
    let c = dot3(travel, u1).clamp(-1.0, 1.0);
    let bend = c.acos();
    if !(1e-3..=std::f64::consts::PI - 1e-3).contains(&bend) { return None; }
    let axis = unit3(cross3(travel, u1));
    let t0 = [f.at[0] + u0[0] * f.trim[0], f.at[1] + u0[1] * f.trim[0], f.at[2] + u0[2] * f.trim[0]];
    // the centre of the arc is toward the inside of the bend, square to the first leg
    let inside = unit3(cross3(axis, travel));
    let centre = [t0[0] + inside[0] * f.radius, t0[1] + inside[1] * f.radius, t0[2] + inside[2] * f.radius];
    // the arc has to end where the second leg's tangent point is
    let end = {
        let r = rotate_vec(sub3(t0, centre), axis, bend);
        [centre[0] + r[0], centre[1] + r[1], centre[2] + r[2]]
    };
    let t1 = [f.at[0] + u1[0] * f.trim[1], f.at[1] + u1[1] * f.trim[1], f.at[2] + u1[2] * f.trim[1]];
    if (0..3).any(|k| (end[k] - t1[k]).abs() > 1e-6) { return None; }
    // section frame at the start: x and y as for a straight piece along `travel`
    let size = f.port_sizes[0];
    let (x0, ring): (V3, Vec<P>) = match size {
        RunSize::Round { diameter } => (perp(travel), loop_points(&Loop::Circle { c: [0.0, 0.0], r: diameter / 2.0 })),
        RunSize::Rect { width, height, roll } => (unit3(cross3(up_for(travel, roll), travel)), vec![[-width / 2.0, -height / 2.0], [width / 2.0, -height / 2.0], [width / 2.0, height / 2.0], [-width / 2.0, height / 2.0]]),
    };
    let y0 = cross3(travel, x0);
    let steps = ((bend.to_degrees() / 7.5).ceil() as usize).clamp(2, 48);
    let ang = |j: usize| bend * j as f64 / steps as f64;
    let rings = vec![(0..=steps).map(|j| {
        let (x, y) = (rotate_vec(x0, axis, ang(j)), rotate_vec(y0, axis, ang(j)));
        let o = rotate_vec(sub3(t0, centre), axis, ang(j));
        let o = [centre[0] + o[0], centre[1] + o[1], centre[2] + o[2]];
        ring.iter().map(|q| [o[0] + x[0] * q[0] + y[0] * q[1], o[1] + x[1] * q[0] + y[1] * q[1], o[2] + x[2] * q[0] + y[2] * q[1]]).collect()
    }).collect::<Vec<Vec<V3>>>()];
    let out = |_r: usize, k: usize, j: usize| -> V3 {
        let (x, y) = (rotate_vec(x0, axis, ang(j)), rotate_vec(y0, axis, ang(j)));
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        let right = [b[1] - a[1], -(b[0] - a[0])]; // for a counter-clockwise ring this points out
        [x[0] * right[0] + y[0] * right[1], x[1] * right[0] + y[1] * right[1], x[2] * right[0] + y[2] * right[1]]
    };
    let end_dir = rotate_vec(travel, axis, bend);
    let cap_out = move |is_end: bool, _p: V3| -> V3 { if is_end { end_dir } else { [-travel[0], -travel[1], -travel[2]] } };
    skin(&rings, std::slice::from_ref(&ring), &out, false, &cap_out)
}

/// A fitting: an elbow with a consistent bend radius is a bend; anything else is the straight stubs it takes from
/// each leg, as in the IFC export.
fn fitting_mesh(f: &Fitting) -> Option<Mesh> {
    if let Some(m) = elbow_bend(f) { return Some(m); }
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
