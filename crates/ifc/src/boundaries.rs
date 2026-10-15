//! Second-level space boundaries: for every room, the faces of walls, slabs, doors and windows that bound it, each
//! with its own surface. Worked out from the model, not drawn. Coordinates are in the frame of the storey the room
//! is on (x, y as in the plan, z from the storey's elevation), which is the frame the room is written in.
use crate::{Scene, RoomItem};

type P = [f64; 2];
type V = [f64; 3];

/// Where a boundary face lies, and what it is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Wall(usize),
    Slab(usize),
    /// a door or window: index into `Scene::items`
    Item(usize),
}

/// A planar face (rings are open: the last point is not repeated): the plane (origin, normal pointing out of the room, direction of the face's u axis) and the
/// outline and holes in the plane's own (u, v) coordinates, with v = normal x u.
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    pub origin: V,
    pub axis: V,
    pub refdir: V,
    pub outer: Vec<P>,
    pub inner: Vec<Vec<P>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Boundary {
    pub target: Target,
    pub surface: Surface,
    /// index in the same list of the boundary this one sits in (a door in its wall's face)
    pub parent: Option<usize>,
}

impl Surface {
    /// A point of the face in the room's frame, from its (u, v) coordinates.
    pub fn point(&self, p: P) -> V {
        let v = cross3(self.axis, self.refdir);
        [self.origin[0] + p[0] * self.refdir[0] + p[1] * v[0], self.origin[1] + p[0] * self.refdir[1] + p[1] * v[1], self.origin[2] + p[0] * self.refdir[2] + p[1] * v[2]]
    }

    /// Area of the face: the outline less the holes.
    pub fn area(&self) -> f64 {
        let a = |r: &[P]| {
            let n = r.len();
            (0..n).map(|i| r[i][0] * r[(i + 1) % n][1] - r[(i + 1) % n][0] * r[i][1]).sum::<f64>().abs() / 2.0
        };
        a(&self.outer) - self.inner.iter().map(|h| a(h)).sum::<f64>()
    }
}

const TOL: f64 = 0.01;

fn dot2(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn signed_area(r: &[P]) -> f64 {
    let n = r.len();
    (0..n).map(|i| r[i][0] * r[(i + 1) % n][1] - r[(i + 1) % n][0] * r[i][1]).sum::<f64>() / 2.0
}
fn oriented(r: &[P], ccw: bool) -> Vec<P> {
    let mut v = r.to_vec();
    if (signed_area(&v) > 0.0) != ccw {
        v.reverse();
    }
    v
}
fn cross3(a: V, b: V) -> V {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn on_boundary(poly: &[P], p: P, tol: f64) -> bool {
    let n = poly.len();
    (0..n).any(|i| {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let t = if l2 == 0.0 { 0.0 } else { (dot2([p[0] - a[0], p[1] - a[1]], [dx, dy]) / l2).clamp(0.0, 1.0) };
        (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy) <= tol
    })
}
fn inside(poly: &[P], p: P) -> bool {
    let n = poly.len();
    let mut c = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
    }
    c
}
/// all corners of `inner` lie in `outer` or on its edge
fn covers(outer: &[P], inner: &[P]) -> bool {
    inner.iter().all(|&p| inside(outer, p) || on_boundary(outer, p, TOL))
}

/// Coordinates of a point in the plane's (u, v) frame.
fn uv(origin: V, refdir: V, vdir: V, p: V) -> P {
    let d = [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]];
    [d[0] * refdir[0] + d[1] * refdir[1] + d[2] * refdir[2], d[0] * vdir[0] + d[1] * vdir[1] + d[2] * vdir[2]]
}
fn rect(u0: f64, v0: f64, u1: f64, v1: f64) -> Vec<P> {
    vec![[u0, v0], [u1, v0], [u1, v1], [u0, v1]]
}

/// The boundaries of one room, and what could not be worked out.
pub fn room_boundaries(scene: &Scene, room: &RoomItem) -> (Vec<Boundary>, Vec<String>) {
    let r = &room.room;
    let mut out: Vec<Boundary> = Vec::new();
    let mut warn: Vec<String> = Vec::new();
    let Some(level) = scene.levels.all().get(r.level) else { return (out, warn) };
    let elev = level.elevation;
    let (z0, z1) = (elev, elev + r.height);
    let outline = oriented(&r.outline, true);
    let holes: Vec<Vec<P>> = r.holes.iter().map(|h| oriented(h, false)).collect();

    // ---- walls, with their doors and windows
    for ring in std::iter::once(&outline).chain(holes.iter()) {
        let n = ring.len();
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            let len = (b[0] - a[0]).hypot(b[1] - a[1]);
            if len < 1e-9 {
                continue;
            }
            let d = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
            let nout = [d[1], -d[0]]; // the right of the edge points out of the room
            for (wi, w) in scene.walls.iter().enumerate() {
                let c = &w.chain;
                if c.tops.iter().any(|t| t.is_some()) || c.base_z + c.height <= z0 + 1e-6 || c.base_z >= z1 - 1e-6 {
                    continue;
                }
                let Ok(plans) = c.plans() else { continue };
                let np = c.points.len();
                for (si, p) in plans.iter().enumerate() {
                    let (c0, c1) = (c.points[si], c.points[(si + 1) % np]);
                    // the wall has to stand outside the room, beside the edge
                    let mid = [(c0[0] + c1[0]) / 2.0, (c0[1] + c1[1]) / 2.0];
                    if dot2([mid[0] - a[0], mid[1] - a[1]], nout) <= 0.0 {
                        continue;
                    }
                    for (f0, f1) in [(p.l0, p.l1), (p.r0, p.r1)] {
                        let fl = (f1[0] - f0[0]).hypot(f1[1] - f0[1]);
                        if fl < 1e-9 {
                            continue;
                        }
                        let parallel = ((f1[0] - f0[0]) * d[1] - (f1[1] - f0[1]) * d[0]).abs() / fl <= 1e-6;
                        let apart = dot2([f0[0] - a[0], f0[1] - a[1]], nout).abs();
                        if !parallel || apart > TOL {
                            continue;
                        }
                        let (s0, s1) = (dot2([f0[0] - a[0], f0[1] - a[1]], d), dot2([f1[0] - a[0], f1[1] - a[1]], d));
                        let (lo, hi) = (s0.min(s1).max(0.0), s0.max(s1).min(len));
                        if hi - lo < 1e-6 {
                            continue;
                        }
                        let (za, zb) = (z0.max(c.base_z), z1.min(c.base_z + c.height));
                        let origin = [a[0] + d[0] * lo, a[1] + d[1] * lo, za - elev];
                        let axis = [nout[0], nout[1], 0.0];
                        let refdir = [d[0], d[1], 0.0];
                        let mut wall = Surface { origin, axis, refdir, outer: rect(0.0, 0.0, hi - lo, zb - za), inner: vec![] };
                        let wall_idx = out.len();
                        let mut kids: Vec<Boundary> = Vec::new();
                        let u = [(c1[0] - c0[0]) / (c1[0] - c0[0]).hypot(c1[1] - c0[1]), (c1[1] - c0[1]) / (c1[0] - c0[0]).hypot(c1[1] - c0[1])];
                        if let Some(ops) = c.openings.get(si) {
                            for o in ops {
                                let pa = dot2([c0[0] + u[0] * o.offset - a[0], c0[1] + u[1] * o.offset - a[1]], d);
                                let pb = dot2([c0[0] + u[0] * (o.offset + o.width) - a[0], c0[1] + u[1] * (o.offset + o.width) - a[1]], d);
                                let (ua, ub) = (pa.min(pb).max(lo) - lo, pa.max(pb).min(hi) - lo);
                                let (va, vb) = ((c.base_z + o.sill).max(za) - za, (c.base_z + o.sill + o.height).min(zb) - za);
                                if ub - ua < 1e-6 || vb - va < 1e-6 {
                                    continue;
                                }
                                wall.inner.push(rect(ua, va, ub, vb));
                                // a door or window that fills this opening has a boundary of its own
                                for (k, it) in scene.items.iter().enumerate() {
                                    let Some((hw, hs, centre)) = it.host else { continue };
                                    let Some([iw, ih, sill]) = it.instance.opening else { continue };
                                    if hw == wi && hs == si && (centre - iw / 2.0 - o.offset).abs() < 1e-6 && (iw - o.width).abs() < 1e-6 && (ih - o.height).abs() < 1e-6 && (sill - o.sill).abs() < 1e-6 {
                                        let face = Surface { origin, axis, refdir, outer: rect(ua, va, ub, vb), inner: vec![] };
                                        kids.push(Boundary { target: Target::Item(k), surface: face, parent: Some(wall_idx) });
                                    }
                                }
                            }
                        }
                        out.push(Boundary { target: Target::Wall(wi), surface: wall, parent: None });
                        out.extend(kids);
                    }
                }
            }
        }
    }

    // ---- floor and ceiling
    let ring_uv = |ring: &[P], origin: V, refdir: V, vdir: V, z: f64| -> Vec<P> { ring.iter().map(|p| uv(origin, refdir, vdir, [p[0], p[1], z])).collect() };
    let mut ceiling: Option<(usize, f64)> = None;
    for (si, item) in scene.slabs.iter().enumerate() {
        let s = &item.slab;
        let covered = covers(&s.outline, &outline);
        let bottom = s.top - s.thickness;
        if !covered {
            continue;
        }
        if (s.top - z0).abs() <= TOL {
            // the floor: faces down out of the room
            let (origin, axis, refdir) = ([0.0, 0.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]);
            let vdir = cross3(axis, refdir);
            let surface = Surface { origin, axis, refdir, outer: ring_uv(&outline, origin, refdir, vdir, 0.0), inner: holes.iter().map(|h| ring_uv(h, origin, refdir, vdir, 0.0)).collect() };
            out.push(Boundary { target: Target::Slab(si), surface, parent: None });
        } else if bottom > z0 + 0.5 * r.height && ceiling.is_none_or(|(_, b)| bottom < b) {
            ceiling = Some((si, bottom));
        }
    }
    match ceiling {
        Some((si, bottom)) if (bottom - z1).abs() <= 0.3 => {
            let (origin, axis, refdir) = ([0.0, 0.0, bottom - elev], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
            let vdir = cross3(axis, refdir);
            let z = bottom - elev;
            let surface = Surface { origin, axis, refdir, outer: ring_uv(&outline, origin, refdir, vdir, z), inner: holes.iter().map(|h| ring_uv(h, origin, refdir, vdir, z)).collect() };
            out.push(Boundary { target: Target::Slab(si), surface, parent: None });
        }
        Some((si, bottom)) => warn.push(format!("room {}: the slab {} above is {:.2} m from the top of the room, so no ceiling boundary was made", r.name, scene.slabs[si].slab.name, bottom - z1)),
        None => {}
    }
    (out, warn)
}
