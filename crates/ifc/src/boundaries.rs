//! Second-level space boundaries: for every room, the faces of walls, slabs, doors and windows that bound it, each
//! with its own surface. Worked out from the model, not drawn. Coordinates are in the frame of the storey the room
//! is on (x, y as in the plan, z from the storey's elevation), which is the frame the room is written in.
use crate::{RoomItem, Scene, SlabRole};

type P = [f64; 2];
type V = [f64; 3];

/// Where a boundary face lies, and what it is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Wall(usize),
    Slab(usize),
    /// a column: index into `Scene::columns`
    Column(usize),
    /// a door or window: index into `Scene::items`
    Item(usize),
    /// a room separation line: index into `Scene::separation_lines`; the boundary is virtual
    Virtual(usize),
}

/// Where a room's boundary lies in a wall, as in Revit's Area and Volume Computations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoundaryLocation {
    /// the faces of the wall (the default)
    #[default]
    WallFinish,
    /// the centre line of the wall
    WallCentre,
    /// the faces of the wall's core
    WallCore,
    /// the centre line of the wall's core
    WallCoreCentre,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomSettings {
    pub boundary_location: BoundaryLocation,
    /// compute volumes as well as areas: the floor and ceiling are boundaries of the room
    pub volumes: bool,
    /// what the floor and ceiling of a room are
    pub volume_basis: VolumeBasis,
}

/// Where a room's volume stops at the bottom and the top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VolumeBasis {
    /// a floor finish and a false ceiling, where the room has them, else the slabs (the default)
    #[default]
    Finished,
    /// the slabs only: from the structural floor to the structural slab above, over any finish or false ceiling
    SlabToSlab,
}

impl Default for RoomSettings {
    fn default() -> Self {
        RoomSettings { boundary_location: BoundaryLocation::WallFinish, volumes: true, volume_basis: VolumeBasis::Finished }
    }
}

/// A line that bounds rooms where there is no wall (Revit's room separation line), on one level.
#[derive(Debug, Clone, PartialEq)]
pub struct SeparationLine {
    pub id: u64,
    pub level: usize,
    pub a: [f64; 2],
    pub b: [f64; 2],
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
    let ext = room_extent(scene, room);
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
            // the parts of the side that something bounds, along the side from its start
            let mut covered: Vec<(f64, f64)> = Vec::new();
            for (wi, w) in scene.walls.iter().enumerate() {
                let c = &w.chain;
                if !w.room_bounding || c.tops.iter().any(|t| t.is_some()) || c.base_z + c.height <= z0 + 1e-6 || c.base_z >= z1 - 1e-6 {
                    continue;
                }
                let Ok(plans) = c.plans() else { continue };
                let np = c.points.len();
                for (si, p) in plans.iter().enumerate() {
                    let (c0, c1) = (c.points[si], c.points[(si + 1) % np]);
                    // the wall has to stand outside the room, beside the edge
                    let mid = [(c0[0] + c1[0]) / 2.0, (c0[1] + c1[1]) / 2.0];
                    if dot2([mid[0] - a[0], mid[1] - a[1]], nout) < -TOL {
                        continue;
                    }
                    for (f0, f1) in wall_lines(scene, wi, c, p, c0, c1) {
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
                        covered.push((lo, hi));
                        let (za, zb) = (ext.floor_z.max(c.base_z), ext.ceiling_z.min(c.base_z + c.height));
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
            // the faces of columns that a wall is attached to, where they look into the room
            for (ci, col) in scene.columns.iter().enumerate() {
                if !col.room_bounding || col.base_z + col.height <= z0 + 1e-6 || col.base_z >= z1 - 1e-6 || !attached_to_wall(scene, col) {
                    continue;
                }
                let fp = oriented(&col.footprint, true);
                let m = fp.len();
                for k in 0..m {
                    let (f0, f1) = (fp[k], fp[(k + 1) % m]);
                    let fl = (f1[0] - f0[0]).hypot(f1[1] - f0[1]);
                    if fl < 1e-9 {
                        continue;
                    }
                    // the column is on the outer side of the room's edge: its face looks back along the edge's outward normal
                    let face_n = [(f1[1] - f0[1]) / fl, -(f1[0] - f0[0]) / fl];
                    let faces_room = dot2(face_n, nout) < -1.0 + 1e-6;
                    let apart = dot2([f0[0] - a[0], f0[1] - a[1]], nout).abs();
                    if !faces_room || apart > TOL {
                        continue;
                    }
                    let (s0, s1) = (dot2([f0[0] - a[0], f0[1] - a[1]], d), dot2([f1[0] - a[0], f1[1] - a[1]], d));
                    let (lo, hi) = (s0.min(s1).max(0.0), s0.max(s1).min(len));
                    if hi - lo < 1e-6 {
                        continue;
                    }
                    covered.push((lo, hi));
                    let (za, zb) = (ext.floor_z.max(col.base_z), ext.ceiling_z.min(col.base_z + col.height));
                    let surface = Surface { origin: [a[0] + d[0] * lo, a[1] + d[1] * lo, za - elev], axis: [nout[0], nout[1], 0.0], refdir: [d[0], d[1], 0.0], outer: rect(0.0, 0.0, hi - lo, zb - za), inner: vec![] };
                    out.push(Boundary { target: Target::Column(ci), surface, parent: None });
                }
            }
            // room separation lines on this side bound it where there is no wall
            for (li, sl) in scene.separation_lines.iter().enumerate().filter(|(_, l)| l.level == r.level) {
                let sd = [sl.b[0] - sl.a[0], sl.b[1] - sl.a[1]];
                let sl_len = sd.iter().map(|x| x * x).sum::<f64>().sqrt();
                if sl_len < 1e-9 {
                    continue;
                }
                let sdn = [sd[0] / sl_len, sd[1] / sl_len];
                let parallel = (sdn[0] * d[1] - sdn[1] * d[0]).abs() <= 1e-6;
                if !parallel || dot2([sl.a[0] - a[0], sl.a[1] - a[1]], nout).abs() > TOL {
                    continue;
                }
                let (s0, s1) = (dot2([sl.a[0] - a[0], sl.a[1] - a[1]], d), dot2([sl.b[0] - a[0], sl.b[1] - a[1]], d));
                let (lo, hi) = (s0.min(s1).max(0.0), s0.max(s1).min(len));
                if hi - lo < 1e-6 {
                    continue;
                }
                covered.push((lo, hi));
                let surface = Surface { origin: [a[0] + d[0] * lo, a[1] + d[1] * lo, 0.0], axis: [nout[0], nout[1], 0.0], refdir: [d[0], d[1], 0.0], outer: rect(0.0, 0.0, hi - lo, r.height), inner: vec![] };
                out.push(Boundary { target: Target::Virtual(li), surface, parent: None });
            }
            // what nothing bounds is said, since a room has no side without a wall or a separation line
            covered.sort_by(|x, y| x.0.total_cmp(&y.0));
            let mut reach = 0.0;
            let mut gaps = 0.0;
            for (lo, hi) in &covered {
                if *lo > reach + 1e-6 {
                    gaps += lo - reach;
                }
                reach = reach.max(*hi);
            }
            if len > reach + 1e-6 {
                gaps += len - reach;
            }
            if gaps > 1e-6 {
                warn.push(format!("room {}: {:.2} m of the side from ({:.2}, {:.2}) to ({:.2}, {:.2}) is bounded by no wall and no separation line", r.name, gaps, a[0], a[1], b[0], b[1]));
            }
        }
    }

    // ---- floor and ceiling
    let ring_uv = |ring: &[P], origin: V, refdir: V, vdir: V, z: f64| -> Vec<P> { ring.iter().map(|p| uv(origin, refdir, vdir, [p[0], p[1], z])).collect() };
    if let Some(si) = ext.floor {
        // the floor faces down out of the room, at the top of its finish
        let z = ext.floor_z - elev;
        let (origin, axis, refdir) = ([0.0, 0.0, z], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]);
        let vdir = cross3(axis, refdir);
        let surface = Surface { origin, axis, refdir, outer: ring_uv(&outline, origin, refdir, vdir, z), inner: holes.iter().map(|h| ring_uv(h, origin, refdir, vdir, z)).collect() };
        out.push(Boundary { target: Target::Slab(si), surface, parent: None });
    }
    if let Some(si) = ext.ceiling {
        let s = &scene.slabs[si].slab;
        let bottom = s.top - s.thickness;
        if ext.ceiling_is_false || (bottom - z1).abs() <= 0.3 {
            let z = bottom - elev;
            let (origin, axis, refdir) = ([0.0, 0.0, z], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
            let vdir = cross3(axis, refdir);
            let surface = Surface { origin, axis, refdir, outer: ring_uv(&outline, origin, refdir, vdir, z), inner: holes.iter().map(|h| ring_uv(h, origin, refdir, vdir, z)).collect() };
            out.push(Boundary { target: Target::Slab(si), surface, parent: None });
        } else {
            warn.push(format!("room {}: the slab {} above is {:.2} m from the top of the room, so no ceiling boundary was made", r.name, s.name, bottom - z1));
        }
    }
    (out, warn)
}

/// The lines of a wall segment that can be a room's boundary, as the two end points of each: the faces of the wall,
/// its centre line, or the faces or centre line of its core, as the project's setting says.
fn wall_lines(scene: &Scene, wi: usize, c: &openbim_geom::WallChain, p: &openbim_geom::SegmentPlan, c0: P, c1: P) -> Vec<(P, P)> {
    let t = c.thickness;
    let (before, core) = match scene.walls[wi].wall_type.and_then(|ti| scene.wall_types.get(ti)) {
        Some(wt) if (wt.thickness() - t).abs() < 1e-9 => {
            let (a, b) = wt.core_range();
            (wt.layers[..a].iter().map(|l| l.thickness).sum::<f64>(), wt.layers[a..=b].iter().map(|l| l.thickness).sum::<f64>())
        }
        _ => (0.0, t),
    };
    // offsets to the left of the centre line; the exterior (first layer) is on the left
    let offsets: Vec<f64> = match scene.room_settings.boundary_location {
        BoundaryLocation::WallFinish => return vec![(p.l0, p.l1), (p.r0, p.r1)],
        BoundaryLocation::WallCentre => vec![0.0],
        BoundaryLocation::WallCore => vec![t / 2.0 - before, t / 2.0 - before - core],
        BoundaryLocation::WallCoreCentre => vec![t / 2.0 - before - core / 2.0],
    };
    let len = (c1[0] - c0[0]).hypot(c1[1] - c0[1]);
    if len < 1e-9 {
        return vec![];
    }
    let (u, n) = ([(c1[0] - c0[0]) / len, (c1[1] - c0[1]) / len], [-(c1[1] - c0[1]) / len, (c1[0] - c0[0]) / len]);
    offsets
        .into_iter()
        .map(|o| {
            // the ends of a line between the faces lie between the ends of the faces (mitres and stops are straight across)
            let w = (o + t / 2.0) / t;
            let (s0, s1) = (p.s_r0 + (p.s_l0 - p.s_r0) * w, p.s_r1 + (p.s_l1 - p.s_r1) * w);
            ([c0[0] + u[0] * s0 + n[0] * o, c0[1] + u[1] * s0 + n[1] * o], [c0[0] + u[0] * s1 + n[0] * o, c0[1] + u[1] * s1 + n[1] * o])
        })
        .collect()
}

/// The heights a room's volume lies between, and the slabs and coverings that make them.
#[derive(Debug, Clone, PartialEq)]
pub struct Extent {
    /// the floor of the room as its boundary: the top of a floor finish, else the level
    pub floor_z: f64,
    /// the ceiling: the underside of a false ceiling, else the room's top
    pub ceiling_z: f64,
    /// the level and the room's own top: the slab-to-slab extent
    pub slab_floor_z: f64,
    pub slab_ceiling_z: f64,
    /// index in `Scene::slabs` of what bounds the floor and the ceiling, when something does
    pub floor: Option<usize>,
    pub ceiling: Option<usize>,
    /// the floor or ceiling is a finish or a false ceiling
    pub floor_is_finish: bool,
    pub ceiling_is_false: bool,
}

/// Find the floor and the ceiling of a room. A floor finish standing on the level and a false ceiling under the
/// room's top are the boundaries when the basis is `Finished`; the structural slabs otherwise.
pub fn room_extent(scene: &Scene, r: &RoomItem) -> Extent {
    let room = &r.room;
    let elev = scene.levels.all().get(room.level).map_or(0.0, |l| l.elevation);
    let (z0, z1) = (elev, elev + room.height);
    let outline = oriented(&room.outline, true);
    let mut e = Extent { floor_z: z0, ceiling_z: z1, slab_floor_z: z0, slab_ceiling_z: z1, floor: None, ceiling: None, floor_is_finish: false, ceiling_is_false: false };
    if !scene.room_settings.volumes {
        return e;
    }
    let finished = scene.room_settings.volume_basis == VolumeBasis::Finished;
    let mut ceil_struct: Option<(usize, f64)> = None;
    for (si, item) in scene.slabs.iter().enumerate() {
        let s = &item.slab;
        if !item.room_bounding || !covers(&s.outline, &outline) {
            continue;
        }
        let bottom = s.top - s.thickness;
        match item.role {
            SlabRole::Structure => {
                if (s.top - z0).abs() <= TOL {
                    if e.floor.is_none() {
                        e.floor = Some(si);
                    }
                } else if bottom > z0 + 0.5 * room.height && ceil_struct.is_none_or(|(_, b)| bottom < b) {
                    ceil_struct = Some((si, bottom));
                }
            }
            // the highest finish on the level
            SlabRole::FloorFinish if finished && (bottom - z0).abs() <= TOL && s.top - z0 < 0.5 * room.height && (!e.floor_is_finish || s.top > e.floor_z) => {
                e.floor = Some(si);
                e.floor_z = s.top;
                e.floor_is_finish = true;
            }
            // the lowest false ceiling
            SlabRole::Ceiling if finished && bottom > z0 + 0.5 * room.height && bottom <= z1 + 0.3 && (!e.ceiling_is_false || bottom < e.ceiling_z) => {
                e.ceiling = Some(si);
                e.ceiling_z = bottom;
                e.ceiling_is_false = true;
            }
            _ => {}
        }
    }
    if !e.ceiling_is_false {
        if let Some((si, _)) = ceil_struct {
            e.ceiling = Some(si);
        }
    }
    e
}

/// The volumes of a room from its area: what the room holds, and what lies above its false ceiling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomVolumes {
    /// from the floor to the ceiling as the volume basis says (the volume of the room)
    pub volume: f64,
    /// from the structural floor to the slab: the room and any floor build-up and ceiling void
    pub slab_to_slab: f64,
    /// the part of slab-to-slab above a false ceiling (zero without one)
    pub ceiling_void: f64,
    /// the part below the floor finish (zero without one)
    pub floor_build_up: f64,
}

pub fn room_volumes(scene: &Scene, r: &RoomItem) -> RoomVolumes {
    let e = room_extent(scene, r);
    let a = r.room.area();
    let (vol, s2s) = (a * (e.ceiling_z - e.floor_z), a * (e.slab_ceiling_z - e.slab_floor_z));
    RoomVolumes { volume: vol, slab_to_slab: s2s, ceiling_void: a * (e.slab_ceiling_z - e.ceiling_z), floor_build_up: a * (e.floor_z - e.slab_floor_z) }
}

/// Air changes per hour of a room: an air flow in m³/s over a volume in m³. `with_ceiling_void` counts the space
/// above a false ceiling as part of the room, as it is for a plenum that the room's air passes through.
pub fn air_changes_per_hour(flow_m3_per_s: f64, v: &RoomVolumes, with_ceiling_void: bool) -> Option<f64> {
    let vol = if with_ceiling_void { v.volume + v.ceiling_void } else { v.volume };
    (flow_m3_per_s.is_finite() && vol > 1e-9).then(|| flow_m3_per_s * 3600.0 / vol)
}

/// Is a wall attached to the column: does any wall segment's plan touch the column's footprint (within the tolerance)
/// at a height the two share?
fn attached_to_wall(scene: &Scene, col: &crate::ColumnItem) -> bool {
    let fp = oriented(&col.footprint, true);
    scene.walls.iter().any(|w| {
        let c = &w.chain;
        if c.base_z + c.height <= col.base_z + 1e-6 || c.base_z >= col.base_z + col.height - 1e-6 {
            return false;
        }
        let Ok(plans) = c.plans() else { return false };
        plans.iter().any(|p| near_polygons(&fp, &[p.l0, p.l1, p.r1, p.r0], TOL))
    })
}

/// Do two convex polygons come within `tol` of each other? (No axis of either polygon's edges separates them by more.)
fn near_polygons(a: &[P], b: &[P], tol: f64) -> bool {
    let axes = |poly: &[P]| -> Vec<P> {
        (0..poly.len())
            .filter_map(|i| {
                let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
                let l = (q[0] - p[0]).hypot(q[1] - p[1]);
                (l > 1e-12).then(|| [-(q[1] - p[1]) / l, (q[0] - p[0]) / l])
            })
            .collect()
    };
    let span = |poly: &[P], ax: P| -> (f64, f64) {
        poly.iter().map(|p| dot2(*p, ax)).fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| (lo.min(x), hi.max(x)))
    };
    axes(a).into_iter().chain(axes(b)).all(|ax| {
        let ((alo, ahi), (blo, bhi)) = (span(a, ax), span(b, ax));
        alo <= bhi + tol && blo <= ahi + tol
    })
}
