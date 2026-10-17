//! Triangle meshes of what a family instance builds: its boxes, extrusions, revolves, sweeps and blends, with the
//! voids cut out. Everything is in the family's own frame (the turn `xf` and the shift `dz` of nested families are
//! already applied); the caller moves the points into the model.
//!
//! Triangles face outward. Cutting a void leaves the surface with T-junctions (a vertex of one triangle on the
//! edge of another), so the result is not watertight in the strict sense, but its volume and its look are right.

use crate::forms::{lerp_loop, Form, FormGeom, Loop};
use crate::{Axis, Instance};

type V = [f64; 3];
type P = [f64; 2];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TriMesh {
    pub verts: Vec<V>,
    pub tris: Vec<[u32; 3]>,
}

impl TriMesh {
    /// Signed volume by the divergence theorem; positive for outward-facing triangles.
    pub fn volume(&self) -> f64 {
        self.tris.iter().map(|t| {
            let (a, b, c) = (self.verts[t[0] as usize], self.verts[t[1] as usize], self.verts[t[2] as usize]);
            dot(a, cross(b, c)) / 6.0
        }).sum()
    }
    pub fn append(&mut self, o: TriMesh) {
        let base = self.verts.len() as u32;
        self.verts.extend(o.verts);
        self.tris.extend(o.tris.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
    }
    fn flip(&mut self) { for t in &mut self.tris { t.swap(1, 2); } }
    pub fn bounds(&self) -> Option<(V, V)> {
        let first = *self.verts.first()?;
        Some(self.verts.iter().fold((first, first), |(lo, hi), v| ([lo[0].min(v[0]), lo[1].min(v[1]), lo[2].min(v[2])], [hi[0].max(v[0]), hi[1].max(v[1]), hi[2].max(v[2])])))
    }
}

pub(crate) fn sub(a: V, b: V) -> V { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
pub(crate) fn dot(a: V, b: V) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
pub(crate) fn cross(a: V, b: V) -> V { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }

const CIRCLE_SIDES: usize = 32;

fn area2(r: &[P]) -> f64 {
    (0..r.len()).map(|k| { let (a, b) = (r[k], r[(k + 1) % r.len()]); a[0] * b[1] - b[0] * a[1] }).sum()
}

/// The ring `r` turned to run counter-clockwise (`ccw`) or clockwise.
pub fn oriented(r: &[P], ccw: bool) -> Vec<P> {
    let mut v = r.to_vec();
    if (area2(&v) > 0.0) != ccw { v.reverse(); }
    v
}

/// The corners of a loop; a circle is a 32-gon.
pub fn loop_points(l: &Loop) -> Vec<P> {
    match l {
        Loop::Poly(p) => p.clone(),
        Loop::Circle { c, r } => (0..CIRCLE_SIDES).map(|k| { let a = std::f64::consts::TAU * k as f64 / CIRCLE_SIDES as f64; [c[0] + r * a.cos(), c[1] + r * a.sin()] }).collect(),
    }
}

/// A prism between heights z0 and z1 on `outer`, with vertical holes. None if the outline cannot be triangulated.
pub fn prism(outer: &[P], holes: &[Vec<P>], z0: f64, z1: f64) -> Option<TriMesh> {
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
    let mut mesh = TriMesh::default();
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

pub fn rotate_vec(v: V, axis: V, t: f64) -> V {
    let (s, c) = (t.sin(), t.cos());
    let cr = cross(axis, v);
    let d = dot(axis, v);
    [v[0] * c + cr[0] * s + axis[0] * d * (1.0 - c), v[1] * c + cr[1] * s + axis[1] * d * (1.0 - c), v[2] * c + cr[2] * s + axis[2] * d * (1.0 - c)]
}

/// A solid made by carrying a profile through positions. `rings[r][j][k]` is point k of ring r at position j (ring 0
/// the outer boundary, counter-clockwise; the others holes, clockwise); `out(r, k, j)` is a vector that points out of
/// the solid at the side face between points k and k + 1 of ring r at position j; `wrap` joins the last position to
/// the first (a full turn); otherwise the ends are closed with caps triangulated from `profile` (the 2D rings), and
/// `cap_out(is_end, centre)` says which way a cap faces.
pub fn skin(rings: &[Vec<Vec<V>>], profile: &[Vec<P>], out: &dyn Fn(usize, usize, usize) -> V, wrap: bool, cap_out: &dyn Fn(bool, V) -> V) -> Option<TriMesh> {
    let steps = rings[0].len();
    let mut mesh = TriMesh::default();
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
                let nrm = cross(sub(pb, pa), sub(pd, pa));
                let flip = dot(nrm, out(r, k, j)) < 0.0;
                mesh.tris.extend(if flip { [[a, d, c], [a, c, b]] } else { [[a, b, c], [a, c, d]] });
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
                let nrm = cross(sub(p[1], p[0]), sub(p[2], p[0]));
                let flip = dot(nrm, cap_out(is_end, centre)) < 0.0;
                mesh.tris.push(if flip { [v[0], v[2], v[1]] } else { v });
            }
        }
    }
    Some(mesh)
}

/// Rings of a profile with the outer one counter-clockwise and the holes clockwise, whatever they were.
fn profile_rings(f: &Form) -> Vec<Vec<P>> {
    std::iter::once(oriented(&loop_points(&f.outer), true)).chain(f.holes.iter().map(|h| oriented(&loop_points(h), false))).collect()
}

fn plane_axes(plane: Axis) -> (V, V, V) {
    // u direction, v direction, normal
    match plane { Axis::Z => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), Axis::X => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]), Axis::Y => ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]) }
}
fn map3(plane: Axis, offset: f64, u: f64, v: f64) -> V {
    match plane { Axis::Z => [u, v, offset], Axis::X => [offset, u, v], Axis::Y => [v, offset, u] }
}
/// A vector pointing out of the profile's edge a to b in the plane, as a vector in space.
fn edge_out(plane: Axis, a: P, b: P) -> V {
    let (ud, vd, _) = plane_axes(plane);
    let right = [b[1] - a[1], -(b[0] - a[0])];
    [ud[0] * right[0] + vd[0] * right[1], ud[1] * right[0] + vd[1] * right[1], ud[2] * right[0] + vd[2] * right[1]]
}

fn revolve_local(f: &Form) -> Option<TriMesh> {
    let FormGeom::Revolve { plane, offset, about_v, at, angle } = f.geom else { return None };
    let (ud, vd, _) = plane_axes(plane);
    let (axis_pt, axis) = if about_v { (map3(plane, offset, at, 0.0), vd) } else { (map3(plane, offset, 0.0, at), ud) };
    let full = angle >= 360.0 - 1e-9;
    let steps = ((angle / 7.5).ceil() as usize).clamp(2, 96);
    let n = if full { steps } else { steps + 1 };
    let profile = profile_rings(f);
    let at_step = |p: V, j: usize| -> V {
        let r = rotate_vec(sub(p, axis_pt), axis, angle.to_radians() * j as f64 / steps as f64);
        [axis_pt[0] + r[0], axis_pt[1] + r[1], axis_pt[2] + r[2]]
    };
    let rings: Vec<Vec<Vec<V>>> = profile.iter().map(|ring| (0..n).map(|j| ring.iter().map(|q| at_step(map3(plane, offset, q[0], q[1]), j)).collect()).collect()).collect();
    let out = |r: usize, k: usize, j: usize| -> V {
        let ring = &profile[r];
        rotate_vec(edge_out(plane, ring[k], ring[(k + 1) % ring.len()]), axis, angle.to_radians() * j as f64 / steps as f64)
    };
    let cap_out = |is_end: bool, c: V| -> V {
        let t = cross(axis, sub(c, axis_pt));
        if is_end { t } else { [-t[0], -t[1], -t[2]] }
    };
    skin(&rings, &profile, &out, full, &cap_out)
}

/// The mitre vectors at each vertex of a path and the left normal of each segment.
fn path_frames(path: &[P]) -> Option<(Vec<P>, Vec<P>)> {
    let nrm = |a: P, b: P| { let d = [b[0] - a[0], b[1] - a[1]]; let l = d[0].hypot(d[1]); if l == 0.0 { None } else { Some([-d[1] / l, d[0] / l]) } };
    let lefts: Vec<P> = path.windows(2).map(|w| nrm(w[0], w[1])).collect::<Option<_>>()?;
    let n = path.len();
    let mitre = (0..n).map(|i| {
        if i == 0 { lefts[0] } else if i == n - 1 { lefts[n - 2] } else {
            let (a, b) = (lefts[i - 1], lefts[i]);
            let k = 1.0 + a[0] * b[0] + a[1] * b[1];
            [(a[0] + b[0]) / k, (a[1] + b[1]) / k]
        }
    }).collect();
    Some((mitre, lefts))
}

/// A sweep or a swept blend: the profile (u to the left of travel, v up) carried along the plan path at height z, mitred
/// at the bends; for a swept blend the profile changes into `end` in proportion to the distance along the path.
fn sweep_local(f: &Form) -> Option<TriMesh> {
    let (z, path, end) = match &f.geom { FormGeom::Sweep { z, path } => (*z, path, None), FormGeom::SweptBlend { z, path, end } => (*z, path, Some(end)), _ => return None };
    if path.len() < 2 { return None; }
    let (mitre, lefts) = path_frames(path)?;
    let n = path.len();
    let total: f64 = path.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum();
    let mut frac = vec![0.0];
    for w in path.windows(2) { frac.push(frac[frac.len() - 1] + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]) / total); }
    // the profile at every vertex
    let at = |i: usize| -> Vec<Vec<P>> {
        match end {
            None => profile_rings(f),
            Some(e) => vec![oriented(&loop_points(&lerp_loop(&f.outer, e, frac[i])), true)],
        }
    };
    let profiles: Vec<Vec<Vec<P>>> = (0..n).map(at).collect();
    let rings: Vec<Vec<Vec<V>>> = (0..profiles[0].len()).map(|r| (0..n).map(|i| profiles[i][r].iter().map(|q| [path[i][0] + q[0] * mitre[i][0], path[i][1] + q[0] * mitre[i][1], z + q[1]]).collect()).collect()).collect();
    let profile = &profiles[0];
    let out = |r: usize, k: usize, j: usize| -> V {
        let ring = &profiles[j][r];
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        let right = [b[1] - a[1], -(b[0] - a[0])];
        let l = lefts[j];
        [l[0] * right[0], l[1] * right[0], right[1]]
    };
    let dirs: Vec<V> = lefts.iter().map(|l| [l[1], -l[0], 0.0]).collect(); // travel = left turned right
    let cap_out = move |is_end: bool, _c: V| -> V { if is_end { dirs[dirs.len() - 1] } else { let d = dirs[0]; [-d[0], -d[1], -d[2]] } };
    skin(&rings, profile, &out, false, &cap_out)
}

/// A blend: the profile at `from` changes into the top profile at `to`, point for point.
fn blend_local(f: &Form) -> Option<TriMesh> {
    let FormGeom::Blend { plane, from, to, ref top } = f.geom else { return None };
    let (_, _, n) = plane_axes(plane);
    let bottom = oriented(&loop_points(&f.outer), true);
    let upper = oriented(&loop_points(top), true);
    if bottom.len() != upper.len() { return None; }
    let ring = |pts: &[P], w: f64| -> Vec<V> { pts.iter().map(|q| map3(plane, w, q[0], q[1])).collect() };
    let rings = vec![vec![ring(&bottom, from), ring(&upper, to)]];
    let out = |_r: usize, k: usize, j: usize| -> V { let p = if j == 0 { &bottom } else { &upper }; edge_out(plane, p[k], p[(k + 1) % p.len()]) };
    let cap_out = move |is_end: bool, _c: V| -> V { if is_end { n } else { [-n[0], -n[1], -n[2]] } };
    skin(&rings, std::slice::from_ref(&bottom), &out, false, &cap_out)
}

fn extrusion_local(f: &Form) -> Option<TriMesh> {
    let FormGeom::Extrusion { plane, from, to } = f.geom else { return None };
    let outer = loop_points(&f.outer);
    let holes: Vec<Vec<P>> = f.holes.iter().map(loop_points).collect();
    let mut m = prism(&outer, &holes, 0.0, 1.0)?;
    for v in &mut m.verts {
        *v = map3(plane, from + v[2] * (to - from), v[0], v[1]);
    }
    if m.volume() < 0.0 { m.flip(); } // the plane may be left-handed
    Some(m)
}

impl Form {
    /// The surface of the form in the family's frame (after `xf` and `dz`). None if it cannot be triangulated.
    pub fn mesh(&self) -> Option<TriMesh> {
        let mut m = match self.geom {
            FormGeom::Extrusion { .. } => extrusion_local(self)?,
            FormGeom::Revolve { .. } => revolve_local(self)?,
            FormGeom::Sweep { .. } | FormGeom::SweptBlend { .. } => sweep_local(self)?,
            FormGeom::Blend { .. } => blend_local(self)?,
        };
        for v in &mut m.verts {
            let xy = self.xf.apply([v[0], v[1]]);
            *v = [xy[0], xy[1], v[2] + self.dz];
        }
        Some(m)
    }
}

/// One solid piece of an instance and the material it has.
#[derive(Debug, Clone)]
pub struct Piece {
    pub mesh: TriMesh,
    /// index into `Instance::strings`
    pub material: Option<u32>,
}

/// Everything an instance builds, voids already cut out, and what could not be drawn.
#[derive(Debug, Clone, Default)]
pub struct Body {
    pub pieces: Vec<Piece>,
    pub warnings: Vec<String>,
}

impl Instance {
    /// The meshes of the boxes and forms, in the family's frame. Void forms are cut out of every solid piece they touch.
    pub fn body(&self) -> Body {
        let mut body = Body::default();
        let mut voids: Vec<TriMesh> = Vec::new();
        for (k, f) in self.forms.iter().enumerate() {
            match f.mesh() {
                Some(m) if f.void => voids.push(m),
                Some(m) => body.pieces.push(Piece { mesh: m, material: self.form_materials.get(k).copied().flatten() }),
                None => body.warnings.push("a form of this family could not be meshed".into()),
            }
        }
        // boxes come first in the order of the instance
        let mut pieces = Vec::new();
        for (k, (_, bx)) in self.solids.iter().enumerate() {
            let xf = self.xf.get(k).copied().flatten();
            let ring: Vec<P> = [[bx.min[0], bx.min[1]], [bx.max[0], bx.min[1]], [bx.max[0], bx.max[1]], [bx.min[0], bx.max[1]]].iter().map(|&q| match xf { Some(t) => t.apply(q), None => q }).collect();
            if let Some(m) = prism(&ring, &[], bx.min[2], bx.max[2]) {
                pieces.push(Piece { mesh: m, material: self.materials.get(k).copied().flatten() });
            }
        }
        pieces.append(&mut body.pieces);
        if !voids.is_empty() {
            for p in &mut pieces {
                for v in &voids {
                    match crate::csg::difference(&p.mesh, v) {
                        Some(m) => p.mesh = m,
                        None => { body.warnings.push("a void could not be cut (the surfaces are too complex)".into()); break; }
                    }
                }
            }
            pieces.retain(|p| !p.mesh.tris.is_empty());
        }
        body.pieces = pieces;
        body.warnings.dedup();
        body
    }
}
