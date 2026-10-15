//! Automatic routing: the best orthogonal path between two equipment connectors.
//!
//! The search runs on a grid whose spacing is the author's `pitch`. Moves go along the three axes only. A path
//! starts leaving the first connector the way it faces and ends entering the second the way it faces, so the
//! result can be joined to both pieces of equipment in the connectivity graph. The cost of a path is
//!
//! * its length (vertical moves count `vertical_factor` times, because risers cost more),
//! * `bend_cost` for each bend, and
//! * for each soft zone it crosses (a wall, a floor): the zone's own cost once for each crossing, plus the length
//!   run inside the zone times `inside_factor`, so a pipe never runs along inside a wall.
//!
//! Hard zones (equipment bodies, existing runs) are never entered. Every zone is widened all round by the half
//! size of the run, the routing preferences' margin and the author's `clearance`, because the search follows
//! the centreline. A bend may only be made once the straight length since the last bend is at least `min_leg`,
//! the length two elbows and the shortest straight piece between them need, so the path always turns into a
//! valid route; the result is built with the same drawing code as a route drawn by hand and refused if that
//! code refuses it. The search is exact on the grid (A* with an admissible heuristic) and does the same thing
//! every time. Nothing here is a standard value: every number is the author's.
use super::catalogue::FittingClass;
use super::{conn_run_size, dist, ref_dim, route_inner, same_size, transition_len, Opts, Route, RouteError, RoutingPrefs};
use crate::snaps::{SnapSettings, Snaps};
use openbim_family::mep::Connector;
use openbim_geom::WallChain;
use openbim_place::penetrations::{Run, RunSize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

const MAX_NODES: f64 = 40_000_000.0;
const MAX_ZONES: usize = 100_000;
/// zones are taken as open: touching a face is not entering
const SHRINK: f64 = 1e-7;

#[derive(Clone, Debug, PartialEq)]
pub enum AutoError {
    BadInput(&'static str),
    /// a connector does not face along one of the coordinate axes (or along the vertical when that is off)
    NotAxisAligned,
    /// no path exists on this grid
    NoPath,
    /// the grid is too big, or the search ran past its limit
    TooLarge,
    Route(RouteError),
}

impl std::fmt::Display for AutoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for AutoError {}

#[derive(Clone, Debug, PartialEq)]
enum Shape {
    Hard { lo: [f64; 3], hi: [f64; 3] },
    /// convex prism: `nx * x + ny * y >= c` for every plane, between two heights
    Soft { planes: Vec<[f64; 3]>, z0: f64, z1: f64, cost: f64 },
}

/// Something the path must avoid (hard) or may cross at a price (soft).
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub label: String,
    shape: Shape,
}

fn fin3(v: [f64; 3]) -> bool {
    v.iter().all(|x| x.is_finite() && x.abs() <= 1.0e6)
}

impl Zone {
    /// A box that is never entered.
    pub fn hard(label: &str, lo: [f64; 3], hi: [f64; 3]) -> Result<Zone, AutoError> {
        if !fin3(lo) || !fin3(hi) || (0..3).any(|i| lo[i] >= hi[i]) {
            return Err(AutoError::BadInput("a hard zone needs finite corners with the low corner below the high one"));
        }
        Ok(Zone { label: label.into(), shape: Shape::Hard { lo, hi } })
    }

    /// A convex prism (plan outline between two heights) that may be crossed at `cost` (metres of extra length
    /// for each crossing). The outline may run either way round.
    pub fn soft(label: &str, ring: &[[f64; 2]], z0: f64, z1: f64, cost: f64) -> Result<Zone, AutoError> {
        let n = ring.len();
        if !(3..=64).contains(&n) || ring.iter().any(|p| !fin3([p[0], p[1], 0.0])) || !fin3([z0, z1, cost]) || z0 >= z1 || cost < 0.0 {
            return Err(AutoError::BadInput("a soft zone needs 3 to 64 finite plan points, two heights in order and a cost of 0 or more"));
        }
        let area2: f64 = (0..n).map(|i| ring[i][0] * ring[(i + 1) % n][1] - ring[(i + 1) % n][0] * ring[i][1]).sum();
        if area2.abs() < 1e-12 {
            return Err(AutoError::BadInput("the outline of a soft zone has no area"));
        }
        let pts: Vec<[f64; 2]> = if area2 > 0.0 { ring.to_vec() } else { ring.iter().rev().copied().collect() };
        let mut planes = Vec::with_capacity(n);
        for i in 0..n {
            let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let l = d[0].hypot(d[1]);
            if l < 1e-9 {
                return Err(AutoError::BadInput("the outline of a soft zone has a zero-length edge"));
            }
            let e = [c[0] - b[0], c[1] - b[1]];
            if d[0] * e[1] - d[1] * e[0] < -1e-9 * l * e[0].hypot(e[1]).max(1.0) {
                return Err(AutoError::BadInput("the outline of a soft zone must be convex"));
            }
            let nrm = [-d[1] / l, d[0] / l];
            planes.push([nrm[0], nrm[1], nrm[0] * a[0] + nrm[1] * a[1]]);
        }
        Ok(Zone { label: label.into(), shape: Shape::Soft { planes, z0, z1, cost } })
    }

    /// The box a straight run takes up (its section and margin all round), never entered.
    pub fn from_run(run: &Run) -> Result<Zone, AutoError> {
        let half = ref_dim(run.size) / 2.0 + run.margin;
        if !fin3(run.a) || !fin3(run.b) || !half.is_finite() || half < 0.0 {
            return Err(AutoError::BadInput("a run has a number that is not finite"));
        }
        let lo = [0, 1, 2].map(|i| run.a[i].min(run.b[i]) - half);
        let hi = [0, 1, 2].map(|i| run.a[i].max(run.b[i]) + half);
        Zone::hard(&run.label, lo, hi)
    }

    /// One soft zone for each segment of a wall chain: `fire_cost` when the chain is a fire partition,
    /// otherwise `plain_cost`. Openings are not looked at: crossing a wall costs the same wherever it is.
    pub fn walls(chain: &WallChain, plain_cost: f64, fire_cost: f64) -> Result<Vec<Zone>, AutoError> {
        let plans = chain.plans().map_err(|_| AutoError::BadInput("the wall chain is not valid"))?;
        let cost = if chain.fire_rating_min.is_some() { fire_cost } else { plain_cost };
        let mut out = Vec::with_capacity(plans.len());
        for (i, p) in plans.iter().enumerate() {
            out.push(Zone::soft(&format!("wall segment {i}"), &[p.l0, p.l1, p.r1, p.r0], chain.base_z, chain.base_z + chain.height, cost)?);
        }
        Ok(out)
    }
}

/// The author's tuning of the search.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoOptions {
    /// grid spacing, metres
    pub pitch: f64,
    /// extra length-equivalent for each bend, metres
    pub bend_cost: f64,
    /// vertical moves count this many times their length (1 or more)
    pub vertical_factor: f64,
    /// length run inside a soft zone counts this many times (1 or more)
    pub inside_factor: f64,
    /// space kept free all round the run, added to the routing preferences' margin, metres
    pub clearance: f64,
    /// shortest straight length between two bends; None = two bend radii plus the shortest straight piece
    pub min_leg: Option<f64>,
    /// may the path go up and down
    pub allow_vertical: bool,
    /// how far outside the box of the two connectors the search may go, metres
    pub search_margin: f64,
    /// the search stops with `TooLarge` after this many steps
    pub max_expansions: usize,
}

impl Default for AutoOptions {
    fn default() -> Self {
        AutoOptions { pitch: 0.1, bend_cost: 1.0, vertical_factor: 1.5, inside_factor: 10.0, clearance: 0.05, min_leg: None, allow_vertical: true, search_margin: 2.0, max_expansions: 1_000_000 }
    }
}

impl AutoOptions {
    pub fn check(&self) -> Result<(), AutoError> {
        let ok = |v: f64, lo: f64, hi: f64| v.is_finite() && v >= lo && v <= hi;
        if !ok(self.pitch, 0.005, 10.0) || !ok(self.bend_cost, 0.0, 1000.0) || !ok(self.vertical_factor, 1.0, 1000.0) || !ok(self.inside_factor, 1.0, 1000.0) || !ok(self.clearance, 0.0, 10.0) || !ok(self.search_margin, 0.0, 1000.0) {
            return Err(AutoError::BadInput("an automatic routing option is not a finite number in range"));
        }
        if self.min_leg.is_some_and(|m| !ok(m, 1e-6, 1000.0)) || self.max_expansions == 0 {
            return Err(AutoError::BadInput("the shortest leg must be above 0 and the expansion limit at least 1"));
        }
        Ok(())
    }
}

/// One path to find: from one connector to another in one size, avoiding the zones. `from_own` and `to_own`
/// name zones (by position in `zones`) that belong to the equipment at each end: they are not looked at while
/// the path runs straight out of (or into) its own connector, and are avoided everywhere else.
#[derive(Clone, Copy, Debug)]
pub struct AutoRequest<'a> {
    pub from: &'a Connector,
    pub to: &'a Connector,
    pub size: RunSize,
    pub prefs: &'a RoutingPrefs,
    pub zones: &'a [Zone],
    pub from_own: &'a [usize],
    pub to_own: &'a [usize],
    pub opts: &'a AutoOptions,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AutoRoute {
    /// the drawn route, from the first connector to the second
    pub route: Route,
    /// the first connector, each bend, the second connector
    pub waypoints: Vec<[f64; 3]>,
    /// the cost of the path as the search counted it
    pub cost: f64,
    /// the label of every soft zone entered, one for each crossing, in order along the path
    pub crossed: Vec<String>,
    pub expansions: usize,
}

fn axis_dir(d: [f64; 3]) -> Option<usize> {
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if !l.is_finite() || l < 1e-9 {
        return None;
    }
    for ax in 0..3 {
        let v = d[ax] / l;
        let others = (0..3).filter(|&j| j != ax).all(|j| (d[j] / l).abs() < 1e-6);
        if v.abs() > 1.0 - 1e-6 && others {
            return Some(ax * 2 + usize::from(v < 0.0));
        }
    }
    None
}

fn dir_vec(d: usize) -> [f64; 3] {
    let mut v = [0.0; 3];
    v[d / 2] = if d.is_multiple_of(2) { 1.0 } else { -1.0 };
    v
}

/// Grid lines along one axis: the start's coordinate plus whole pitches, and the end's coordinate itself.
fn axis_lines(a: f64, b: f64, pitch: f64, margin: f64, fixed: bool) -> Vec<f64> {
    if fixed {
        return vec![a];
    }
    let (lo, hi) = (a.min(b) - margin, a.max(b) + margin);
    let k0 = ((lo - a) / pitch).floor() as i64;
    let k1 = ((hi - a) / pitch).ceil() as i64;
    let mut v: Vec<f64> = (k0..=k1).map(|k| a + k as f64 * pitch).collect();
    if !v.iter().any(|x| (x - b).abs() < 1e-9) {
        v.push(b);
    }
    v.sort_by(f64::total_cmp);
    v
}

fn find_line(v: &[f64], x: f64) -> usize {
    v.iter().position(|y| (y - x).abs() < 1e-9).unwrap_or(0)
}

fn on_ray(p: [f64; 3], q: [f64; 3], o: [f64; 3], dir: usize) -> bool {
    let ax = dir / 2;
    let sgn = if dir.is_multiple_of(2) { 1.0 } else { -1.0 };
    (0..3).filter(|&j| j != ax).all(|j| (p[j] - o[j]).abs() < 1e-9 && (q[j] - o[j]).abs() < 1e-9) && sgn * (p[ax] - o[ax]) > -1e-9 && sgn * (q[ax] - o[ax]) > -1e-9
}

/// The part of the segment p->q (as t from 0 to 1) inside a box shrunk by SHRINK, or None.
fn clip_box(p: [f64; 3], q: [f64; 3], lo: [f64; 3], hi: [f64; 3]) -> Option<(f64, f64)> {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for i in 0..3 {
        let (a, b) = (lo[i] + SHRINK, hi[i] - SHRINK);
        let d = q[i] - p[i];
        if d.abs() < 1e-12 {
            if p[i] < a || p[i] > b {
                return None;
            }
        } else {
            let (u, v) = ((a - p[i]) / d, (b - p[i]) / d);
            let (u, v) = if u < v { (u, v) } else { (v, u) };
            t0 = t0.max(u);
            t1 = t1.min(v);
            if t0 > t1 {
                return None;
            }
        }
    }
    Some((t0, t1))
}

/// The same for a convex prism widened by `r` all round.
fn clip_prism(p: [f64; 3], q: [f64; 3], planes: &[[f64; 3]], z0: f64, z1: f64, r: f64) -> Option<(f64, f64)> {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let mut limit = |s0: f64, s1: f64| -> bool {
        // inside while s >= 0; s is linear in t
        let d = s1 - s0;
        if d.abs() < 1e-12 {
            return s0 >= 0.0;
        }
        let t = -s0 / d;
        if d > 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        t0 <= t1
    };
    for pl in planes {
        let c = pl[2] - r + SHRINK;
        if !limit(pl[0] * p[0] + pl[1] * p[1] - c, pl[0] * q[0] + pl[1] * q[1] - c) {
            return None;
        }
    }
    if !limit(p[2] - (z0 - r + SHRINK), q[2] - (z0 - r + SHRINK)) || !limit((z1 + r - SHRINK) - p[2], (z1 + r - SHRINK) - q[2]) {
        return None;
    }
    Some((t0, t1))
}

#[derive(Clone, Copy)]
struct Item {
    f: f64,
    g: f64,
    key: u64,
}
impl PartialEq for Item {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Item {
    // a min-heap on f, then on the state number, so equal costs always come out in the same order
    fn cmp(&self, o: &Self) -> Ordering {
        o.f.total_cmp(&self.f).then(o.key.cmp(&self.key))
    }
}

/// Find the best path for one request.
pub fn auto_route(rq: &AutoRequest) -> Result<AutoRoute, AutoError> {
    let o = rq.opts;
    o.check()?;
    let prefs = rq.prefs;
    prefs.check().map_err(AutoError::Route)?;
    let (from, to) = (rq.from, rq.to);
    for c in [from, to] {
        if c.domain != prefs.domain || !c.system.compatible(prefs.system) {
            return Err(AutoError::Route(RouteError::ConnectorMismatch));
        }
    }
    let csz1 = conn_run_size(from).map_err(AutoError::Route)?;
    let csz2 = conn_run_size(to).map_err(AutoError::Route)?;
    let dim = ref_dim(rq.size);
    if !dim.is_finite() || dim <= 0.0 || dim > 100.0 {
        return Err(AutoError::BadInput("the run size is not a number in range"));
    }
    let (a, b) = (from.pos, to.pos);
    if !fin3(a) || !fin3(b) {
        return Err(AutoError::BadInput("a connector position is not a finite number in range"));
    }
    let d1 = axis_dir(from.dir).ok_or(AutoError::NotAxisAligned)?;
    let d2 = axis_dir(to.dir).ok_or(AutoError::NotAxisAligned)?;
    if !o.allow_vertical {
        if d1 / 2 == 2 || d2 / 2 == 2 {
            return Err(AutoError::NotAxisAligned);
        }
        if (a[2] - b[2]).abs() > 1e-9 {
            return Err(AutoError::BadInput("with vertical moves off both connectors must be at one height"));
        }
    }
    if rq.zones.len() > MAX_ZONES || rq.from_own.iter().chain(rq.to_own).any(|&i| i >= rq.zones.len()) {
        return Err(AutoError::BadInput("too many zones, or an own-zone number is out of range"));
    }
    if (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt() < 1e-6 {
        return Err(AutoError::BadInput("the two connectors are at one point"));
    }
    let stub = prefs.connector_stub.max(1e-3);
    let r = dim / 2.0 + prefs.margin + o.clearance;
    let bend_r = prefs.bend_radius_factor * dim;
    // a connector of another size than the pipe gets a transition just after its stub, which the first leg must hold
    let lead_transition = if same_size(csz1, rq.size) {
        0.0
    } else {
        let ty = prefs.effective_catalogue().pick_transition(prefs.domain, prefs.system, csz1, rq.size).map_err(|_| AutoError::Route(RouteError::NoFitting { class: FittingClass::Transition, vertex: 1 }))?.clone();
        transition_len(&ty, csz1, rq.size).ok_or(AutoError::Route(RouteError::MixedTransition { vertex: 1 }))?
    };
    let min_leg = o.min_leg.unwrap_or_else(|| (2.0 * bend_r + prefs.min_straight).max(stub + bend_r + prefs.min_straight + lead_transition));
    let kf = (min_leg / o.pitch - 1e-9).ceil().max(1.0);
    if kf > 1000.0 {
        return Err(AutoError::BadInput("the shortest leg is more than 1000 pitches"));
    }
    let k = kf as usize;

    // the grid
    let lines: Vec<Vec<f64>> = (0..3).map(|ax| axis_lines(a[ax], b[ax], o.pitch, o.search_margin, ax == 2 && !o.allow_vertical)).collect();
    let n = [lines[0].len(), lines[1].len(), lines[2].len()];
    if (n[0] as f64) * (n[1] as f64) * (n[2] as f64) > MAX_NODES {
        return Err(AutoError::TooLarge);
    }
    let start = [find_line(&lines[0], a[0]), find_line(&lines[1], a[1]), find_line(&lines[2], a[2])];
    let goal = [find_line(&lines[0], b[0]), find_line(&lines[1], b[1]), find_line(&lines[2], b[2])];
    let bounds_lo = [lines[0][0] - r, lines[1][0] - r, lines[2][0] - r];
    let bounds_hi = [lines[0][n[0] - 1] + r, lines[1][n[1] - 1] + r, lines[2][n[2] - 1] + r];

    // the zones that can matter here, with whether each is one of the two ends' own
    struct Z<'a> {
        shape: &'a Shape,
        from_own: bool,
        to_own: bool,
        label: &'a str,
    }
    let mut zs: Vec<Z> = Vec::new();
    for (i, z) in rq.zones.iter().enumerate() {
        let touches = match &z.shape {
            Shape::Hard { lo, hi } => (0..3).all(|j| lo[j] - r <= bounds_hi[j] && hi[j] + r >= bounds_lo[j]),
            Shape::Soft { .. } => true,
        };
        if touches {
            zs.push(Z { shape: &z.shape, from_own: rq.from_own.contains(&i), to_own: rq.to_own.contains(&i), label: &z.label });
        }
    }
    let pos = |ix: [usize; 3]| [lines[0][ix[0]], lines[1][ix[1]], lines[2][ix[2]]];
    let blocked = |p: [f64; 3], q: [f64; 3]| -> bool {
        zs.iter().any(|z| match z.shape {
            Shape::Hard { lo, hi } => {
                if (z.from_own && on_ray(p, q, a, d1)) || (z.to_own && on_ray(p, q, b, d2)) {
                    return false;
                }
                clip_box(p, q, [0, 1, 2].map(|i| lo[i] - r), [0, 1, 2].map(|i| hi[i] + r)).is_some()
            }
            Shape::Soft { .. } => false,
        })
    };
    let inside_soft = |p: [f64; 3]| -> bool {
        zs.iter().any(|z| match z.shape {
            Shape::Soft { planes, z0, z1, .. } => clip_prism(p, p, planes, *z0, *z1, r).is_some(),
            Shape::Hard { .. } => false,
        })
    };
    // extra cost of the soft zones along p->q, and the labels entered
    let soft_cost = |p: [f64; 3], q: [f64; 3], entered: &mut Vec<String>| -> f64 {
        let len = (0..3).map(|i| (q[i] - p[i]).powi(2)).sum::<f64>().sqrt();
        let mut extra = 0.0;
        for z in &zs {
            if let Shape::Soft { planes, z0, z1, cost } = z.shape {
                if (z.from_own && on_ray(p, q, a, d1)) || (z.to_own && on_ray(p, q, b, d2)) {
                    continue;
                }
                if let Some((t0, t1)) = clip_prism(p, q, planes, *z0, *z1, r) {
                    let inside = (t1 - t0) * len;
                    if inside > 1e-9 {
                        extra += inside * (o.inside_factor - 1.0);
                        if clip_prism(p, p, planes, *z0, *z1, r).is_none() {
                            extra += cost;
                            entered.push(z.label.to_string());
                        }
                    }
                }
            }
        }
        extra
    };

    let goal_dir = d2 ^ 1;
    let node_id = |ix: [usize; 3]| ((ix[2] * n[1] + ix[1]) * n[0] + ix[0]) as u64;
    // the state also says whether the path has bent yet: one that has not is a straight line between the two stubs,
    // which have to fit in it side by side
    let key_of = |node: u64, d: usize, run: usize, bent: bool| (((node * 6 + d as u64) * (k as u64 + 1)) + run as u64) * 2 + u64::from(bent);
    let decode = |key: u64| -> ([usize; 3], usize, usize) {
        let key = key / 2;
        let run = (key % (k as u64 + 1)) as usize;
        let rest = key / (k as u64 + 1);
        let d = (rest % 6) as usize;
        let node = (rest / 6) as usize;
        ([node % n[0], (node / n[0]) % n[1], node / (n[0] * n[1])], d, run)
    };
    // the least a path from here can cost: the distance, and a bend unless the goal is dead ahead the way it is entered
    let heur = |ix: [usize; 3], d: usize| {
        let p = pos(ix);
        let disp = [b[0] - p[0], b[1] - p[1], b[2] - p[2]];
        let ax = d / 2;
        let sgn = if d.is_multiple_of(2) { 1.0 } else { -1.0 };
        let ahead = d == goal_dir && (0..3).filter(|&j| j != ax).all(|j| disp[j].abs() < 1e-9) && sgn * disp[ax] > -1e-9;
        disp[0].abs() + disp[1].abs() + o.vertical_factor * disp[2].abs() + if ahead { 0.0 } else { o.bend_cost }
    };
    // a cheap look first: can the goal be reached at all, leaving direction and bends aside?
    {
        let mut seen = vec![false; n[0] * n[1] * n[2]];
        let at = |ix: [usize; 3]| (ix[2] * n[1] + ix[1]) * n[0] + ix[0];
        let mut queue = std::collections::VecDeque::from([start]);
        seen[at(start)] = true;
        let mut reached = start == goal;
        while let Some(ix) = queue.pop_front() {
            if reached {
                break;
            }
            for nd in 0..6 {
                let ax = nd / 2;
                let mut jx = ix;
                if nd % 2 == 0 {
                    if jx[ax] + 1 >= n[ax] {
                        continue;
                    }
                    jx[ax] += 1;
                } else {
                    if jx[ax] == 0 {
                        continue;
                    }
                    jx[ax] -= 1;
                }
                if seen[at(jx)] || blocked(pos(ix), pos(jx)) {
                    continue;
                }
                seen[at(jx)] = true;
                if jx == goal {
                    reached = true;
                    break;
                }
                queue.push_back(jx);
            }
        }
        if !reached {
            return Err(AutoError::NoPath);
        }
    }

    let start_key = key_of(node_id(start), d1, 0, false);
    let straight_fits = dist(a, b) > 2.0 * stub + 1e-9;
    let goal_node = node_id(goal);
    let mut best: HashMap<u64, (f64, u64)> = HashMap::new();
    best.insert(start_key, (0.0, u64::MAX));
    let mut heap = BinaryHeap::new();
    heap.push(Item { f: heur(start, d1), g: 0.0, key: start_key });
    let mut expansions = 0usize;
    let mut found: Option<(u64, f64)> = None;
    let mut sink = Vec::new();
    while let Some(Item { g, key, .. }) = heap.pop() {
        if best.get(&key).is_some_and(|e| g > e.0 + 1e-12) {
            continue;
        }
        expansions += 1;
        if expansions > o.max_expansions {
            return Err(AutoError::TooLarge);
        }
        let (ix, d, run) = decode(key);
        if node_id(ix) == goal_node && d == goal_dir && run == k && (key % 2 == 1 || straight_fits) {
            found = Some((key, g));
            break;
        }
        let p = pos(ix);
        let can_turn = run >= k && !inside_soft(p);
        for nd in 0..6 {
            if nd != d && (nd == d ^ 1 || !can_turn) {
                continue;
            }
            let ax = nd / 2;
            let mut jx = ix;
            if nd % 2 == 0 {
                if jx[ax] + 1 >= n[ax] {
                    continue;
                }
                jx[ax] += 1;
            } else {
                if jx[ax] == 0 {
                    continue;
                }
                jx[ax] -= 1;
            }
            let q = pos(jx);
            if blocked(p, q) {
                continue;
            }
            let step = (q[ax] - p[ax]).abs();
            sink.clear();
            let mut c = step * if ax == 2 { o.vertical_factor } else { 1.0 } + soft_cost(p, q, &mut sink);
            if nd != d {
                c += o.bend_cost;
            }
            let gained = (step / o.pitch + 1e-9).floor() as usize;
            let nrun = if nd == d { (run + gained).min(k) } else { gained.min(k) };
            let nkey = key_of(node_id(jx), nd, nrun, key % 2 == 1 || nd != d);
            let ng = g + c;
            let better = best.get(&nkey).is_none_or(|e| ng < e.0 - 1e-12);
            if better {
                best.insert(nkey, (ng, key));
                heap.push(Item { f: ng + heur(jx, nd), g: ng, key: nkey });
            }
        }
    }
    let (end_key, cost) = found.ok_or(AutoError::NoPath)?;

    // walk back: the nodes of the path, then the bends
    let mut keys = vec![end_key];
    while let Some(&(_, parent)) = best.get(keys.last().unwrap_or(&end_key)) {
        if parent == u64::MAX {
            break;
        }
        keys.push(parent);
    }
    keys.reverse();
    let mut waypoints = vec![a];
    let mut crossed = Vec::new();
    for w in keys.windows(2) {
        let (ix0, d0, _) = decode(w[0]);
        let (ix1, d1b, _) = decode(w[1]);
        let (p, q) = (pos(ix0), pos(ix1));
        soft_cost(p, q, &mut crossed);
        if d1b != d0 {
            waypoints.push(p);
        }
    }
    waypoints.push(b);

    // build the route: the connector stubs, the bends, the stub at the far end
    let sv = dir_vec(d1);
    let ev = dir_vec(d2);
    let mut pts = vec![a, [0, 1, 2].map(|i| a[i] + sv[i] * stub)];
    pts.extend_from_slice(&waypoints[1..waypoints.len() - 1]);
    pts.push([0, 1, 2].map(|i| b[i] + ev[i] * stub));
    pts.push(b);
    let mut sizes = vec![csz1];
    sizes.extend(std::iter::repeat_n(rq.size, pts.len() - 3));
    sizes.push(csz2);
    let off = Snaps::new(SnapSettings::off());
    let route = route_inner(&pts, &sizes, prefs, &off, &Opts { lead_start: 0.0, keep_first: true }).map_err(AutoError::Route)?;
    Ok(AutoRoute { route, waypoints, cost, crossed, expansions })
}

/// Find many independent paths at the same time, one thread for each core. The paths do not see each other;
/// the result for each request is exactly what `auto_route` gives for it alone.
pub fn auto_route_many(reqs: &[AutoRequest]) -> Vec<Result<AutoRoute, AutoError>> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(reqs.len().max(1));
    let mut out: Vec<Option<Result<AutoRoute, AutoError>>> = (0..reqs.len()).map(|_| None).collect();
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let mut mine = Vec::new();
                    let mut i = t;
                    while i < reqs.len() {
                        mine.push((i, auto_route(&reqs[i])));
                        i += threads;
                    }
                    mine
                })
            })
            .collect();
        for h in handles {
            if let Ok(done) = h.join() {
                for (i, r) in done {
                    out[i] = Some(r);
                }
            }
        }
    });
    out.into_iter().map(|r| r.unwrap_or(Err(AutoError::BadInput("a routing thread stopped")))).collect()
}

/// Find paths one after another; each path avoids the runs of the ones found before it.
pub fn auto_route_sequence(reqs: &[AutoRequest]) -> Vec<Result<AutoRoute, AutoError>> {
    let mut out = Vec::with_capacity(reqs.len());
    let mut done: Vec<Zone> = Vec::new();
    for rq in reqs {
        let mut zones = rq.zones.to_vec();
        zones.extend(done.iter().cloned());
        let r = auto_route(&AutoRequest { zones: &zones, ..*rq });
        if let Ok(ar) = &r {
            for run in &ar.route.runs {
                if let Ok(z) = Zone::from_run(run) {
                    done.push(z);
                }
            }
        }
        out.push(r);
    }
    out
}
