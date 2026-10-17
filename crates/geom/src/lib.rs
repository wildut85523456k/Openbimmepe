//! Spike S1 / S1b: wall plans with mitred joins and T-junction stops, polygonal openings,
//! sloped (profile) wall tops, one closed shell per wall, parallel evaluation.
//!
//! Units are metres. Plan coordinates are (x, y); z is up.
//! A wall segment is described by a plan quad (left and right edges) and an elevation profile
//! in (s, z): s along the segment from 0 to its length, z up from the base. The solid is the
//! profile swept across the plan quad, so every face is planar and the shell is closed.
//! All public entry points validate their input and return `Err` instead of panicking.
#![forbid(unsafe_code)]

use rayon::prelude::*;
use std::collections::HashMap;

pub const MAX_WALLS: usize = 10_000_000;
pub const MAX_OPENINGS_PER_WALL: usize = 1_000;
pub const MAX_PROFILE_VERTS: usize = 4_096;
const EPS: f64 = 1e-9;
/// Openings must stay at least this far from the wall ends and top.
const MARGIN: f64 = 1e-6;

#[derive(Debug, Clone, PartialEq)]
pub enum GeomError {
    NotFinite(&'static str),
    NonPositive(&'static str),
    TooFewPoints,
    ZeroLengthSegment(usize),
    Parallel(usize),
    /// segment `0` turns straight back over the previous one
    FoldedBack(usize),
    /// segments `0` and `1` touch or cross (not counting neighbours meeting at their shared point)
    SelfIntersecting(usize, usize),
    TooMany(&'static str),
    OpeningOutside { wall: usize, opening: usize },
    OpeningsOverlap { wall: usize },
    BadOpening { wall: usize, opening: usize, why: &'static str },
    BadTop { wall: usize, why: &'static str },
    BadStop(&'static str),
    WallTooShort(usize),
    TriangulationFailed { wall: usize },
}

type P = [f64; 2];

/// A rectangular opening through a wall. `offset` is measured along the wall centreline from its start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opening {
    pub offset: f64,
    pub width: f64,
    pub sill: f64,
    pub height: f64,
}

/// A line that a free wall end is extended to (for T-junctions: the face of the host wall).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StopLine {
    pub point: P,
    pub dir: P,
}

/// A chain of walls along a polyline centreline, closed or open, with one thickness.
#[derive(Debug, Clone, PartialEq)]
pub struct WallChain {
    pub points: Vec<P>,
    pub closed: bool,
    pub thickness: f64,
    pub height: f64,
    pub base_z: f64,
    /// rectangular openings; openings[i] belongs to segment i
    pub openings: Vec<Vec<Opening>>,
    /// polygonal openings in (s, z) of segment i, any simple polygon (arches, circles as polylines)
    pub poly_openings: Vec<Vec<Vec<P>>>,
    /// optional top profile per segment: points (s, z above base), s from 0 to the segment length.
    /// None = flat top at `height`.
    pub tops: Vec<Option<Vec<P>>>,
    /// open chains only: extend the first / last end to this line instead of a square end
    pub start_stop: Option<StopLine>,
    pub end_stop: Option<StopLine>,
    /// fire resistance of the whole chain in minutes, as the author sets it; None = not a fire partition
    pub fire_rating_min: Option<u32>,
}

impl WallChain {
    pub fn new(points: Vec<P>, closed: bool, thickness: f64, height: f64) -> Self {
        WallChain { points, closed, thickness, height, base_z: 0.0, openings: vec![], poly_openings: vec![],
                    tops: vec![], start_stop: None, end_stop: None, fire_rating_min: None }
    }
}

/// Plan quad of one wall segment: left edge l0->l1, right edge r0->r1 (left = left of travel direction).
#[derive(Debug, Clone, Copy)]
pub struct SegmentPlan {
    pub l0: P,
    pub l1: P,
    pub r0: P,
    pub r1: P,
    /// centreline length of the segment
    pub length: f64,
    /// position of each face corner along the centreline (0 = segment start); mitres and stops move these
    pub s_l0: f64,
    pub s_l1: f64,
    pub s_r0: f64,
    pub s_r1: f64,
}

#[derive(Debug, Default, Clone)]
pub struct Mesh {
    pub verts: Vec<[f64; 3]>,
    pub tris: Vec<[u32; 3]>,
}

impl Mesh {
    /// Signed volume by the divergence theorem. Positive for outward-facing triangles.
    pub fn volume(&self) -> f64 {
        self.tris.iter().map(|t| {
            let (a, b, c) = (self.verts[t[0] as usize], self.verts[t[1] as usize], self.verts[t[2] as usize]);
            dot3(a, cross3(b, c)) / 6.0
        }).sum()
    }

    /// True when every directed edge appears exactly once and its reverse exactly once
    /// (a closed, consistently oriented, edge-manifold surface). Vertices must already be welded.
    pub fn is_closed_manifold(&self) -> bool {
        let mut m: HashMap<(u32, u32), u32> = HashMap::with_capacity(self.tris.len() * 3);
        for t in &self.tris {
            if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] { return false; }
            for k in 0..3 { *m.entry((t[k], t[(k + 1) % 3])).or_insert(0) += 1; }
        }
        m.iter().all(|(&(a, b), &n)| n == 1 && m.get(&(b, a)) == Some(&1))
    }

    /// V - E + F for the welded surface (0 for a ring-shaped wall, 2 for a box).
    pub fn euler_characteristic(&self) -> i64 {
        let mut edges = std::collections::HashSet::new();
        let mut used = std::collections::HashSet::new();
        for t in &self.tris {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                edges.insert((a.min(b), a.max(b)));
                used.insert(a);
            }
        }
        used.len() as i64 - edges.len() as i64 + self.tris.len() as i64
    }
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn fin(v: f64, what: &'static str) -> Result<f64, GeomError> {
    if v.is_finite() { Ok(v) } else { Err(GeomError::NotFinite(what)) }
}
fn sub(a: P, b: P) -> P { [a[0] - b[0], a[1] - b[1]] }
fn add(a: P, b: P) -> P { [a[0] + b[0], a[1] + b[1]] }
fn mul(a: P, s: f64) -> P { [a[0] * s, a[1] * s] }
fn len(a: P) -> f64 { (a[0] * a[0] + a[1] * a[1]).sqrt() }
/// Do two segments share at least one point?
fn seg_touch(a: P, b: P, c: P, d: P) -> bool {
    let o = |p: P, q: P, r: P| cross2(sub(q, p), sub(r, p));
    let (o1, o2, o3, o4) = (o(a, b, c), o(a, b, d), o(c, d, a), o(c, d, b));
    if ((o1 > 0.0 && o2 < 0.0) || (o1 < 0.0 && o2 > 0.0)) && ((o3 > 0.0 && o4 < 0.0) || (o3 < 0.0 && o4 > 0.0)) { return true; }
    let on = |p: P, q: P, r: P| o(p, q, r) == 0.0 && r[0] >= p[0].min(q[0]) && r[0] <= p[0].max(q[0]) && r[1] >= p[1].min(q[1]) && r[1] <= p[1].max(q[1]);
    on(a, b, c) || on(a, b, d) || on(c, d, a) || on(c, d, b)
}
fn cross2(a: P, b: P) -> f64 { a[0] * b[1] - a[1] * b[0] }

/// Intersection of lines p + t*d and q + s*e.
fn intersect(p: P, d: P, q: P, e: P) -> Option<P> {
    let den = cross2(d, e);
    if den.abs() < 1e-9 * len(d) * len(e) { return None; }
    let t = cross2(sub(q, p), e) / den;
    Some(add(p, mul(d, t)))
}

fn ring_area(r: &[P]) -> f64 {
    let n = r.len();
    0.5 * (0..n).map(|i| cross2(r[i], r[(i + 1) % n])).sum::<f64>()
}
/// Proper intersection test of segments ab and cd (touching endpoints do not count).
fn segs_cross(a: P, b: P, c: P, d: P) -> bool {
    let (d1, d2) = (cross2(sub(b, a), sub(c, a)), cross2(sub(b, a), sub(d, a)));
    let (d3, d4) = (cross2(sub(d, c), sub(a, c)), cross2(sub(d, c), sub(b, c)));
    ((d1 > EPS && d2 < -EPS) || (d1 < -EPS && d2 > EPS)) && ((d3 > EPS && d4 < -EPS) || (d3 < -EPS && d4 > EPS))
}

/// Elevation profile of one segment: an outer ring (CCW) and holes (CW), all in (s, z).
struct Profile {
    outer: Vec<P>,
    holes: Vec<Vec<P>>,
    /// validated top polyline (s from 0 to the centreline length)
    top: Vec<P>,
    /// extent of the faces along the wall: smin <= smid0 < smid1 <= smax; the full thickness exists in [smid0, smid1]
    smin: f64,
    smid0: f64,
    smid1: f64,
    smax: f64,
}

/// Top height at s, extended beyond the ends along the end slopes.
fn top_ext(top: &[P], s: f64) -> f64 {
    let n = top.len();
    let k = top.partition_point(|p| p[0] <= s).clamp(1, n - 1);
    let (a, b) = (top[k - 1], top[k]);
    a[1] + (b[1] - a[1]) * (s - a[0]) / (b[0] - a[0])
}

/// Signed integral of the extended top over [a, b].
fn top_integral(top: &[P], a: f64, b: f64) -> f64 {
    if a == b { return 0.0; }
    if b < a { return -top_integral(top, b, a); }
    let mut cuts = vec![a];
    cuts.extend(top.iter().map(|p| p[0]).filter(|&x| x > a && x < b));
    cuts.push(b);
    cuts.windows(2).map(|w| 0.5 * (top_ext(top, w[0]) + top_ext(top, w[1])) * (w[1] - w[0])).sum()
}

/// Integral over [a, b] of top(s) * weight(s) where weight ramps linearly from `w_a` at a to `w_b` at b.
fn top_weighted_integral(top: &[P], a: f64, b: f64, w_a: f64, w_b: f64) -> f64 {
    if b <= a { return 0.0; }
    let mut cuts = vec![a];
    cuts.extend(top.iter().map(|p| p[0]).filter(|&x| x > a && x < b));
    cuts.push(b);
    let w = |s: f64| w_a + (w_b - w_a) * (s - a) / (b - a);
    cuts.windows(2).map(|c| {
        let (x0, x1) = (c[0], c[1]);
        let xm = 0.5 * (x0 + x1);
        // product of two linear functions: Simpson's rule is exact
        (x1 - x0) / 6.0 * (top_ext(top, x0) * w(x0) + 4.0 * top_ext(top, xm) * w(xm) + top_ext(top, x1) * w(x1))
    }).sum()
}

impl WallChain {
    pub fn segment_count(&self) -> usize {
        if self.closed { self.points.len() } else { self.points.len() - 1 }
    }

    fn seg(&self, i: usize) -> (P, P) { (self.points[i], self.points[(i + 1) % self.points.len()]) }

    /// A chain may not fold back over itself or touch or cross itself away from its own corners.
    fn check_simple(&self) -> Result<(), GeomError> {
        let n = self.segment_count();
        let segs: Vec<(P, P)> = (0..n).map(|i| self.seg(i)).collect();
        // neighbours: meeting at the shared corner is fine, running back along each other is not
        for i in 0..n {
            let j = (i + 1) % n;
            if !self.closed && j == 0 { continue; }
            let (a, b) = segs[i];
            let (c, d) = segs[j];
            let (u, v) = (sub(b, a), sub(d, c));
            if cross2(u, v) == 0.0 && u[0] * v[0] + u[1] * v[1] < 0.0 { return Err(GeomError::FoldedBack(j)); }
        }
        // everything else: sweep along x
        let mut order: Vec<usize> = (0..n).collect();
        let (lo, hi) = (|k: usize| segs[k].0[0].min(segs[k].1[0]), |k: usize| segs[k].0[0].max(segs[k].1[0]));
        order.sort_by(|&x, &y| lo(x).total_cmp(&lo(y)));
        for (oi, &i) in order.iter().enumerate() {
            let (ylo, yhi) = (segs[i].0[1].min(segs[i].1[1]), segs[i].0[1].max(segs[i].1[1]));
            for &j in &order[oi + 1..] {
                if lo(j) > hi(i) { break; }
                let (ylo2, yhi2) = (segs[j].0[1].min(segs[j].1[1]), segs[j].0[1].max(segs[j].1[1]));
                if ylo2 > yhi || ylo > yhi2 { continue; }
                let (x, y) = (i.min(j), i.max(j));
                let neighbours = y == x + 1 || (self.closed && x == 0 && y == n - 1);
                if neighbours { continue; }
                if seg_touch(segs[x].0, segs[x].1, segs[y].0, segs[y].1) { return Err(GeomError::SelfIntersecting(x, y)); }
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), GeomError> {
        if self.points.len() < 2 || (self.closed && self.points.len() < 3) { return Err(GeomError::TooFewPoints); }
        if self.points.len() > MAX_WALLS { return Err(GeomError::TooMany("points")); }
        for p in &self.points { fin(p[0], "x")?; fin(p[1], "y")?; }
        fin(self.thickness, "thickness")?; fin(self.height, "height")?; fin(self.base_z, "base_z")?;
        if self.thickness <= 0.0 { return Err(GeomError::NonPositive("thickness")); }
        if self.height <= 0.0 { return Err(GeomError::NonPositive("height")); }
        let n = self.segment_count();
        if self.openings.len() > n || self.poly_openings.len() > n || self.tops.len() > n {
            return Err(GeomError::TooMany("per-segment lists"));
        }
        for i in 0..n {
            let (a, b) = self.seg(i);
            if len(sub(b, a)) < EPS { return Err(GeomError::ZeroLengthSegment(i)); }
        }
        self.check_simple()?;
        for s in [&self.start_stop, &self.end_stop].into_iter().flatten() {
            fin(s.point[0], "stop")?; fin(s.point[1], "stop")?; fin(s.dir[0], "stop")?; fin(s.dir[1], "stop")?;
            if len(s.dir) < EPS { return Err(GeomError::BadStop("zero direction")); }
            if self.closed { return Err(GeomError::BadStop("stops only apply to open chains")); }
        }
        Ok(())
    }

    /// Stop line along the face of host segment `host_seg` of `self` that is nearest to `p`.
    /// Use it as `start_stop` / `end_stop` of a branch wall that ends on this wall.
    pub fn face_stop_for(&self, host_seg: usize, p: P) -> Result<StopLine, GeomError> {
        let plans = self.plans()?;
        let pl = plans.get(host_seg).ok_or(GeomError::BadStop("no such host segment"))?;
        let (a, b) = self.seg(host_seg);
        let d = sub(b, a);
        let side = cross2(d, sub(p, a)); // > 0: p is on the left of the host
        let on = if side >= 0.0 { pl.l0 } else { pl.r0 };
        Ok(StopLine { point: on, dir: d })
    }

    /// Mitred plan quad of every segment.
    pub fn plans(&self) -> Result<Vec<SegmentPlan>, GeomError> {
        self.validate()?;
        let n = self.segment_count();
        let h = self.thickness / 2.0;
        let mut dir = Vec::with_capacity(n);
        let mut nrm = Vec::with_capacity(n);
        for i in 0..n {
            let (a, b) = self.seg(i);
            let d = sub(b, a);
            let u = mul(d, 1.0 / len(d));
            dir.push(u);
            nrm.push([-u[1], u[0]]);
        }
        let m = self.points.len();
        let mut jl = vec![[0.0; 2]; m];
        let mut jr = vec![[0.0; 2]; m];
        for v in 0..m {
            let prev = if v == 0 { if self.closed { Some(n - 1) } else { None } } else { Some(v - 1) };
            let next = if v < n { Some(v) } else { None };
            let p = self.points[v];
            match (prev, next) {
                (Some(a), Some(b)) => {
                    let lp = intersect(add(p, mul(nrm[a], h)), dir[a], add(p, mul(nrm[b], h)), dir[b]);
                    let rp = intersect(sub(p, mul(nrm[a], h)), dir[a], sub(p, mul(nrm[b], h)), dir[b]);
                    match (lp, rp) {
                        (Some(l), Some(r)) => {
                            if len(sub(l, p)) > 10.0 * h || len(sub(r, p)) > 10.0 * h { return Err(GeomError::Parallel(v)); }
                            jl[v] = l; jr[v] = r;
                        }
                        _ => { jl[v] = add(p, mul(nrm[a], h)); jr[v] = sub(p, mul(nrm[a], h)); }
                    }
                }
                (Some(a), None) => {
                    jl[v] = add(p, mul(nrm[a], h)); jr[v] = sub(p, mul(nrm[a], h));
                    if let Some(st) = &self.end_stop { self.apply_stop(&mut jl[v], &mut jr[v], p, nrm[a], dir[a], st)?; }
                }
                (None, Some(b)) => {
                    jl[v] = add(p, mul(nrm[b], h)); jr[v] = sub(p, mul(nrm[b], h));
                    if let Some(st) = &self.start_stop { self.apply_stop(&mut jl[v], &mut jr[v], p, nrm[b], dir[b], st)?; }
                }
                (None, None) => return Err(GeomError::TooFewPoints),
            }
        }
        Ok((0..n).map(|i| {
            let j = (i + 1) % m;
            let (a, b) = self.seg(i);
            let u = dir[i];
            let along = |p: P| (p[0] - a[0]) * u[0] + (p[1] - a[1]) * u[1];
            SegmentPlan { l0: jl[i], l1: jl[j], r0: jr[i], r1: jr[j], length: len(sub(b, a)),
                          s_l0: along(jl[i]), s_l1: along(jl[j]), s_r0: along(jr[i]), s_r1: along(jr[j]) }
        }).collect())
    }

    /// Move both side points of a free end onto the stop line, along the wall direction.
    fn apply_stop(&self, l: &mut P, r: &mut P, p: P, nrm: P, dir: P, st: &StopLine) -> Result<(), GeomError> {
        let h = self.thickness / 2.0;
        let nl = intersect(add(p, mul(nrm, h)), dir, st.point, st.dir).ok_or(GeomError::BadStop("stop is parallel to the wall"))?;
        let nr = intersect(sub(p, mul(nrm, h)), dir, st.point, st.dir).ok_or(GeomError::BadStop("stop is parallel to the wall"))?;
        // a stop may only move the end by a bounded amount (the host cannot be far away)
        if len(sub(nl, *l)) > 10.0 * self.thickness + 1.0 || len(sub(nr, *r)) > 10.0 * self.thickness + 1.0 {
            return Err(GeomError::BadStop("stop is too far from the wall end"));
        }
        *l = nl; *r = nr;
        Ok(())
    }

    /// Build the elevation profile (outer ring + holes) of segment i.
    fn profile(&self, i: usize, plan: &SegmentPlan) -> Result<Profile, GeomError> {
        let l = plan.length;
        let (smin, smax) = (plan.s_l0.min(plan.s_r0), plan.s_l1.max(plan.s_r1));
        let (smid0, smid1) = (plan.s_l0.max(plan.s_r0), plan.s_l1.min(plan.s_r1));
        if (smid1 - smid0).is_nan() || smid1 - smid0 <= 4.0 * MARGIN { return Err(GeomError::WallTooShort(i)); }
        // --- top boundary, left to right
        let top: Vec<P> = match self.tops.get(i).and_then(|t| t.as_ref()) {
            None => vec![[0.0, self.height], [l, self.height]],
            Some(t) => {
                if t.len() < 2 || t.len() > MAX_PROFILE_VERTS { return Err(GeomError::BadTop { wall: i, why: "need 2 or more points" }); }
                for p in t { fin(p[0], "top s")?; fin(p[1], "top z")?; if p[1] <= EPS { return Err(GeomError::BadTop { wall: i, why: "top must be above the base" }); } }
                if t[0][0].abs() > 1e-6 || (t[t.len() - 1][0] - l).abs() > 1e-6 { return Err(GeomError::BadTop { wall: i, why: "must run from s = 0 to the segment length" }); }
                if t.windows(2).any(|w| w[1][0] <= w[0][0]) { return Err(GeomError::BadTop { wall: i, why: "s must increase" }); }
                let mut t = t.clone();
                t[0][0] = 0.0; let k = t.len() - 1; t[k][0] = l;
                t
            }
        };
        if smid0 >= top[1][0] - MARGIN || smid1 <= top[top.len() - 2][0] + MARGIN {
            return Err(GeomError::BadTop { wall: i, why: "wall end (mitre or stop) reaches the first or last top point" });
        }
        for s in [smin, smax] {
            if top_ext(&top, s) <= EPS { return Err(GeomError::BadTop { wall: i, why: "extended top falls below the base at a wall end" }); }
        }
        let top_at = |s: f64| -> f64 {
            let k = top.partition_point(|p| p[0] <= s).clamp(1, top.len() - 1);
            let (a, b) = (top[k - 1], top[k]);
            a[1] + (b[1] - a[1]) * ((s - a[0]) / (b[0] - a[0])).clamp(0.0, 1.0)
        };

        // --- openings as polygons in (s, z)
        let mut polys: Vec<Vec<P>> = Vec::new();
        let rects: &[Opening] = self.openings.get(i).map(|v| v.as_slice()).unwrap_or(&[]);
        let others: &[Vec<P>] = self.poly_openings.get(i).map(|v| v.as_slice()).unwrap_or(&[]);
        if rects.len() + others.len() > MAX_OPENINGS_PER_WALL { return Err(GeomError::TooMany("openings")); }
        for (k, o) in rects.iter().enumerate() {
            fin(o.offset, "offset")?; fin(o.width, "width")?; fin(o.sill, "sill")?; fin(o.height, "opening height")?;
            if o.width <= 0.0 || o.height <= 0.0 || o.sill < 0.0 { return Err(GeomError::NonPositive("opening size")); }
            if o.offset < smid0.max(0.0) || o.offset + o.width > smid1.min(l) || o.sill + o.height > self.height + EPS {
                return Err(GeomError::OpeningOutside { wall: i, opening: k });
            }
            polys.push(vec![[o.offset, o.sill], [o.offset + o.width, o.sill], [o.offset + o.width, o.sill + o.height], [o.offset, o.sill + o.height]]);
        }
        for (k, p) in others.iter().enumerate() {
            let k = rects.len() + k;
            if p.len() < 3 || p.len() > MAX_PROFILE_VERTS { return Err(GeomError::BadOpening { wall: i, opening: k, why: "need 3 or more points" }); }
            for q in p { fin(q[0], "opening s")?; fin(q[1], "opening z")?; }
            polys.push(p.clone());
        }
        for (k, p) in polys.iter_mut().enumerate() {
            let a = ring_area(p);
            if a.abs() < EPS { return Err(GeomError::BadOpening { wall: i, opening: k, why: "zero area" }); }
            if a < 0.0 { p.reverse(); } // make CCW
            let n = p.len();
            for u in 0..n {
                for v in (u + 2)..n {
                    if u == 0 && v == n - 1 { continue; }
                    if segs_cross(p[u], p[(u + 1) % n], p[v], p[(v + 1) % n]) {
                        return Err(GeomError::BadOpening { wall: i, opening: k, why: "polygon crosses itself" });
                    }
                }
            }
            for q in p.iter_mut() {
                if q[1] < -EPS { return Err(GeomError::OpeningOutside { wall: i, opening: k }); }
                if q[1].abs() <= EPS { q[1] = 0.0; } // snap to the base
                if q[0] < smid0.max(0.0) + MARGIN || q[0] > smid1.min(l) - MARGIN { return Err(GeomError::OpeningOutside { wall: i, opening: k }); }
                if q[1] > top_at(q[0]) - MARGIN { return Err(GeomError::OpeningOutside { wall: i, opening: k }); }
            }
        }
        // openings may not overlap: bounding boxes in s must be disjoint, or a real edge crossing is an error
        for a in 0..polys.len() {
            for b in (a + 1)..polys.len() {
                let (pa, pb) = (&polys[a], &polys[b]);
                let (a0, a1) = bounds_s(pa);
                let (b0, b1) = bounds_s(pb);
                if a1 <= b0 + EPS || b1 <= a0 + EPS { continue; }
                // overlapping in s: allow only if the shapes truly do not touch (stacked windows etc.)
                let (az0, az1) = bounds_z(pa);
                let (bz0, bz1) = bounds_z(pb);
                if az1 < bz0 - EPS || bz1 < az0 - EPS { continue; }
                // otherwise do an exact edge test; contained shapes (hole in hole) are also an error
                let (na, nb) = (pa.len(), pb.len());
                let mut bad = false;
                'e: for u in 0..na { for v in 0..nb {
                    if segs_cross(pa[u], pa[(u + 1) % na], pb[v], pb[(v + 1) % nb]) { bad = true; break 'e; }
                } }
                if bad || point_in(pa, pb[0]) || point_in(pb, pa[0]) { return Err(GeomError::OpeningsOverlap { wall: i }); }
            }
        }

        // --- outer ring: bottom left to right with bottom-touching openings merged in as notches
        let mut bottoms: Vec<Vec<P>> = Vec::new();
        let mut holes: Vec<Vec<P>> = Vec::new();
        for (k, p) in polys.into_iter().enumerate() {
            if p.iter().any(|q| q[1] == 0.0) {
                let on: Vec<usize> = (0..p.len()).filter(|&u| p[u][1] == 0.0).collect();
                if on.len() != 2 { return Err(GeomError::BadOpening { wall: i, opening: k, why: "must touch the base along exactly one edge" }); }
                let n = p.len();
                // CCW polygon: bottom edge runs left to right, vertex bl followed by br
                let (bl, br) = if p[on[0]][0] < p[on[1]][0] { (on[0], on[1]) } else { (on[1], on[0]) };
                if (bl + 1) % n != br { return Err(GeomError::BadOpening { wall: i, opening: k, why: "base edge is not a polygon edge" }); }
                // notch path from bl back around to br (reverse of the CCW order)
                let mut path = Vec::with_capacity(n);
                let mut u = bl;
                loop { path.push(p[u]); if u == br { break; } u = (u + n - 1) % n; }
                bottoms.push(path);
            } else {
                let mut h = p; h.reverse(); // holes run CW
                holes.push(h);
            }
        }
        bottoms.sort_by(|a, b| a[0][0].total_cmp(&b[0][0]));
        for w in bottoms.windows(2) {
            if w[0][w[0].len() - 1][0] >= w[1][0][0] - EPS { return Err(GeomError::OpeningsOverlap { wall: i }); }
        }
        let mut outer: Vec<P> = vec![[0.0, 0.0]];
        for b in bottoms { outer.extend(b); }
        outer.push([l, 0.0]);
        for p in top.iter().rev() { outer.push(*p); }
        if outer.len() + holes.iter().map(|h| h.len()).sum::<usize>() > MAX_PROFILE_VERTS { return Err(GeomError::TooMany("profile vertices")); }
        // holes must not cross the outer ring
        let no = outer.len();
        for h in &holes {
            for u in 0..h.len() { for v in 0..no {
                if segs_cross(h[u], h[(u + 1) % h.len()], outer[v], outer[(v + 1) % no]) {
                    return Err(GeomError::OpeningOutside { wall: i, opening: 0 });
                }
            } }
        }
        Ok(Profile { outer, holes, top, smin, smid0, smid1, smax })
    }

    /// Exact volume of segment i, computed without any triangulation:
    /// thickness x (profile area over the full-thickness zone) plus the two wedge-shaped end zones where a
    /// mitre or stop cuts the end at an angle. Flat-topped walls with a few rectangular openings use an
    /// allocation-free path; everything else (and every invalid input) goes through the full profile code.
    pub fn segment_volume_exact(&self, i: usize, plan: &SegmentPlan) -> Result<f64, GeomError> {
        if let Some(v) = self.fast_rect_volume(i, plan) { return Ok(v); }
        self.segment_volume_profile(i, plan)
    }

    /// The reference implementation: builds the full elevation profile first. Public so tests can compare.
    #[doc(hidden)]
    pub fn segment_volume_profile(&self, i: usize, plan: &SegmentPlan) -> Result<f64, GeomError> {
        let pr = self.profile(i, plan)?;
        let l = plan.length;
        let mut ring = ring_area(&pr.outer);
        for h in &pr.holes { ring += ring_area(h); }
        // the ring covers [0, l]; the full-thickness zone is [smid0, smid1]
        let mid = ring + top_integral(&pr.top, pr.smid0, 0.0) + top_integral(&pr.top, l, pr.smid1);
        let w0 = top_weighted_integral(&pr.top, pr.smin, pr.smid0, 0.0, 1.0);
        let w1 = top_weighted_integral(&pr.top, pr.smid1, pr.smax, 1.0, 0.0);
        Ok(self.thickness * (mid + w0 + w1))
    }

    /// None whenever anything is unusual, so the full code path decides (and reports errors).
    fn fast_rect_volume(&self, i: usize, plan: &SegmentPlan) -> Option<f64> {
        if self.poly_openings.get(i).is_some_and(|v| !v.is_empty()) { return None; }
        if self.tops.get(i).is_some_and(|t| t.is_some()) { return None; }
        let ops: &[Opening] = self.openings.get(i).map(|v| v.as_slice()).unwrap_or(&[]);
        if ops.len() > 8 { return None; }
        let h = self.height;
        let (smin, smax) = (plan.s_l0.min(plan.s_r0), plan.s_l1.max(plan.s_r1));
        let (smid0, smid1) = (plan.s_l0.max(plan.s_r0), plan.s_l1.min(plan.s_r1));
        if (smid1 - smid0).is_nan() || smid1 - smid0 <= 4.0 * MARGIN { return None; }
        let mut cut = 0.0;
        for (k, o) in ops.iter().enumerate() {
            if !(o.offset.is_finite() && o.width.is_finite() && o.sill.is_finite() && o.height.is_finite()) { return None; }
            if o.width <= 0.0 || o.height <= 0.0 || o.sill < 0.0 { return None; }
            if o.offset < smid0.max(0.0) + MARGIN || o.offset + o.width > smid1.min(plan.length) - MARGIN || o.sill + o.height > h - MARGIN { return None; }
            for p in &ops[..k] {
                if p.offset < o.offset + o.width + EPS && o.offset < p.offset + p.width + EPS { return None; }
            }
            cut += o.width * o.height;
        }
        // flat top: wedges are triangles of height h
        Some(self.thickness * (h * (smid1 - smid0) - cut + 0.5 * h * (smid0 - smin) + 0.5 * h * (smax - smid1)))
    }

    /// Closed triangle shell of segment i. Boundary faces are returned as quads in `quads`
    /// so a chain can drop the faces where neighbouring walls touch.
    fn segment_shell(&self, i: usize, plan: &SegmentPlan, out: &mut Shell) -> Result<(), GeomError> {
        let pr = self.profile(i, plan)?;
        let l = plan.length;
        let mut pts: Vec<P> = pr.outer.clone();
        let mut hole_starts = Vec::new();
        for h in &pr.holes { hole_starts.push(pts.len()); pts.extend(h.iter().copied()); }
        let flat: Vec<f64> = pts.iter().flat_map(|p| [p[0], p[1]]).collect();
        let idx = earcutr::earcut(&flat, &hole_starts, 2).map_err(|_| GeomError::TriangulationFailed { wall: i })?;
        if idx.len() % 3 != 0 || idx.iter().any(|&k| k >= pts.len()) { return Err(GeomError::TriangulationFailed { wall: i }); }
        // the triangulation must cover exactly the profile area
        let want = ring_area(&pr.outer) + pr.holes.iter().map(|h| ring_area(h)).sum::<f64>();
        let got: f64 = idx.chunks(3).map(|t| 0.5 * cross2(sub(pts[t[1]], pts[t[0]]), sub(pts[t[2]], pts[t[0]])).abs()).sum();
        if (got - want).abs() > 1e-7 * want.abs().max(1.0) { return Err(GeomError::TriangulationFailed { wall: i }); }

        let base = out.verts.len() as u32;
        let (a0, _) = self.seg(i);
        let (sl0, sl1, sr0, sr1) = (plan.s_l0, plan.s_l1, plan.s_r0, plan.s_r1);
        // unit direction and left normal of this segment
        let (b0, b1) = self.seg(i);
        let d = sub(b1, b0);
        let u = mul(d, 1.0 / len(d));
        let nl = [-u[1], u[0]];
        let hh = self.thickness / 2.0;
        for p in &pts {
            // ring vertices at s = 0 / s = l are the wall ends: place them where the left and right faces really end
            // and keep the end of a sloped top on the same roof plane
            let (sl, zl, sr, zr) = if p[0] == 0.0 {
                let z = |s: f64| if p[1] > 0.0 { top_ext(&pr.top, s) } else { 0.0 };
                (sl0, z(sl0), sr0, z(sr0))
            } else if p[0] == l {
                let z = |s: f64| if p[1] > 0.0 { top_ext(&pr.top, s) } else { 0.0 };
                (sl1, z(sl1), sr1, z(sr1))
            } else {
                (p[0], p[1], p[0], p[1])
            };
            let lp = add(add(a0, mul(u, sl)), mul(nl, hh));
            let rp = sub(add(a0, mul(u, sr)), mul(nl, hh));
            out.verts.push([lp[0], lp[1], self.base_z + zl]);
            out.verts.push([rp[0], rp[1], self.base_z + zr]);
        }
        let li = |k: usize| base + 2 * k as u32;
        let ri = |k: usize| base + 2 * k as u32 + 1;
        // side faces: a triangle that is CCW in (s, z) faces the right; the left face is reversed
        for t in idx.chunks(3) {
            let (a, b, c) = (t[0], t[1], t[2]);
            let ccw = cross2(sub(pts[b], pts[a]), sub(pts[c], pts[a])) > 0.0;
            let (b, c) = if ccw { (b, c) } else { (c, b) };
            out.tris.push([ri(a), ri(b), ri(c)]);
            out.tris.push([li(a), li(c), li(b)]);
        }
        // boundary quads across the thickness, outward normal
        let mut start = 0;
        let ring_lens: Vec<usize> = std::iter::once(pr.outer.len()).chain(pr.holes.iter().map(|h| h.len())).collect();
        for n in ring_lens {
            for k in 0..n {
                let (u, v) = (start + k, start + (k + 1) % n);
                out.quads.push([li(u), li(v), ri(v), ri(u)]);
            }
            start += n;
        }
        Ok(())
    }

    /// Triangle mesh of segment i (one closed shell).
    pub fn segment_mesh(&self, i: usize, plan: &SegmentPlan) -> Result<Mesh, GeomError> {
        let mut sh = Shell::default();
        self.segment_shell(i, plan, &mut sh)?;
        Ok(sh.into_mesh())
    }

    /// Whole chain as one welded shell: faces where neighbouring segments touch are removed.
    pub fn chain_mesh(&self) -> Result<Mesh, GeomError> {
        let plans = self.plans()?;
        let mut sh = Shell::default();
        for (i, p) in plans.iter().enumerate() { self.segment_shell(i, p, &mut sh)?; }
        Ok(sh.weld_and_merge())
    }
}

fn bounds_s(p: &[P]) -> (f64, f64) {
    p.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q[0]), b.max(q[0])))
}
fn bounds_z(p: &[P]) -> (f64, f64) {
    p.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q[1]), b.max(q[1])))
}
/// Point-in-polygon (even-odd).
fn point_in(poly: &[P], q: P) -> bool {
    let n = poly.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]) + a[0] { inside = !inside; }
    }
    inside
}

#[derive(Default)]
struct Shell {
    verts: Vec<[f64; 3]>,
    tris: Vec<[u32; 3]>,
    quads: Vec<[u32; 4]>,
}

impl Shell {
    fn push_quad(tris: &mut Vec<[u32; 3]>, q: [u32; 4]) {
        tris.push([q[0], q[1], q[2]]);
        tris.push([q[0], q[2], q[3]]);
    }

    fn into_mesh(self) -> Mesh {
        let mut tris = self.tris;
        for q in self.quads { Self::push_quad(&mut tris, q); }
        Mesh { verts: self.verts, tris }
    }

    /// Weld vertices that coincide (to 1e-7 m), drop quads shared by two walls (opposite orientation), and
    /// return the welded mesh.
    fn weld_and_merge(self) -> Mesh {
        let key = |v: [f64; 3]| -> (i64, i64, i64) { ((v[0] * 1e7).round() as i64, (v[1] * 1e7).round() as i64, (v[2] * 1e7).round() as i64) };
        let mut map: HashMap<(i64, i64, i64), u32> = HashMap::new();
        let mut verts: Vec<[f64; 3]> = Vec::new();
        let remap: Vec<u32> = self.verts.iter().map(|&v| {
            *map.entry(key(v)).or_insert_with(|| { verts.push(v); verts.len() as u32 - 1 })
        }).collect();
        let tris: Vec<[u32; 3]> = self.tris.iter().map(|t| [remap[t[0] as usize], remap[t[1] as usize], remap[t[2] as usize]]).collect();
        // quads: a quad whose vertex set equals another's (reverse winding) is an internal face
        let quads: Vec<[u32; 4]> = self.quads.iter().map(|q| [remap[q[0] as usize], remap[q[1] as usize], remap[q[2] as usize], remap[q[3] as usize]]).collect();
        let canon = |q: &[u32; 4]| { let mut s = *q; s.sort_unstable(); s };
        let mut count: HashMap<[u32; 4], u32> = HashMap::new();
        for q in &quads { *count.entry(canon(q)).or_insert(0) += 1; }
        let mut out_tris = tris;
        for q in &quads {
            if count[&canon(q)] == 1 { Self::push_quad(&mut out_tris, *q); }
        }
        // drop degenerate triangles produced by welding
        out_tris.retain(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2]);
        Mesh { verts, tris: out_tris }
    }
}

/// Chains per parallel task. Fixed, so the order of additions (and the result) never depends on thread count.
const CHUNK: usize = 2048;

fn sum_chunk<F: Fn(&WallChain) -> Result<f64, GeomError>>(chunk: &[WallChain], f: &F) -> Result<f64, GeomError> {
    let mut s = 0.0;
    for c in chunk { s += f(c)?; }
    Ok(s)
}

fn total_with<F: Fn(&WallChain) -> Result<f64, GeomError> + Sync>(chains: &[WallChain], parallel: bool, f: F) -> Result<f64, GeomError> {
    if chains.len() > MAX_WALLS { return Err(GeomError::TooMany("chains")); }
    let parts: Result<Vec<f64>, GeomError> = if parallel {
        chains.par_chunks(CHUNK).map(|ch| sum_chunk(ch, &f)).collect()
    } else {
        chains.chunks(CHUNK).map(|ch| sum_chunk(ch, &f)).collect()
    };
    Ok(parts?.iter().sum())
}

/// The total of per-chain volumes added the way `total_volume_parallel` adds them (in chunks of 2048 chains, then the
/// chunk sums), so a caller that keeps the volume of each chain gets the same bits as the function that computes them all.
pub fn combine_chain_volumes(volumes: &[f64]) -> f64 {
    let parts: Vec<f64> = volumes.chunks(CHUNK).map(|ch| { let mut s = 0.0; for v in ch { s += v; } s }).collect();
    parts.iter().sum()
}

/// Volume of one chain from the volumes of its segments: added in segment order, as `total_volume_serial` does.
pub fn chain_volume_from_segments(segments: &[f64]) -> f64 {
    let mut s = 0.0;
    for v in segments { s += v; }
    s
}

/// Total exact volume of all chains, evaluated in parallel (same result as the serial version, bit for bit).
pub fn total_volume_parallel(chains: &[WallChain]) -> Result<f64, GeomError> { total_with(chains, true, chain_volume_par) }

pub fn total_volume_serial(chains: &[WallChain]) -> Result<f64, GeomError> { total_with(chains, false, chain_volume) }

fn chain_volume(c: &WallChain) -> Result<f64, GeomError> {
    let plans = c.plans()?;
    let mut s = 0.0;
    for (i, p) in plans.iter().enumerate() { s += c.segment_volume_exact(i, p)?; }
    Ok(s)
}

/// Chains with more segments than this share their segments among the threads.
const LONG_CHAIN: usize = 512;

/// Same value as `chain_volume`, bit for bit: the segments are evaluated in parallel on a long chain, but added in
/// segment order.
fn chain_volume_par(c: &WallChain) -> Result<f64, GeomError> {
    let plans = c.plans()?;
    if plans.len() <= LONG_CHAIN {
        let mut s = 0.0;
        for (i, p) in plans.iter().enumerate() { s += c.segment_volume_exact(i, p)?; }
        return Ok(s);
    }
    let parts: Result<Vec<f64>, GeomError> = plans.par_iter().enumerate().map(|(i, p)| c.segment_volume_exact(i, p)).collect();
    let mut s = 0.0;
    for v in parts? { s += v; }
    Ok(s)
}

/// Total mesh volume of all chains in parallel (builds every triangle).
pub fn total_mesh_volume_parallel(chains: &[WallChain]) -> Result<f64, GeomError> {
    total_with(chains, true, |c| Ok(c.chain_mesh()?.volume()))
}
