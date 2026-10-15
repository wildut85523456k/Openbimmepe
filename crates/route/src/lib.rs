//! MEP routing. A path drawn point by point becomes straight runs joined by fittings.
//!
//! * Angle snaps (see `snaps`) are applied while drawing, relative to the previous point.
//! * Every bend gets an elbow of the angle drawn, if the routing preferences have that angle; the straight
//!   runs are shortened by the tangent length `R tan(theta / 2)` so the centreline stays one smooth path.
//! * A size change in a straight line gets a transition of the length the preferences ask for.
//! * A branch starting on a run gets a tee (the run is split) or a tap (the run is left whole).
//! * A route can start at an equipment connector, leaving it straight for a stub, resized if needed.
//!
//! The straight runs are `openbim_place::penetrations::Run`, so walls crossed by a route get their openings
//! in the penetration register. No standard values live here: radii, angles, stubs and margins are the
//! author's routing preferences.
pub mod snaps;

use openbim_family::mep::{up_for, Connector, Domain, Size, System};
use openbim_place::penetrations::{Run, RunSize};
use snaps::{SnapError, Snaps};

pub const MAX_POINTS: usize = 1000;
/// Two angles closer than this (degrees) are the same angle.
pub const ANGLE_TOL: f64 = 0.01;
const EPS: f64 = 1e-9;

/// A set of angles in degrees (from 0 to 180) that a fitting exists for.
#[derive(Clone, Debug, PartialEq)]
pub enum AngleSet {
    /// any angle (a flexible or custom fitting)
    Any,
    List(Vec<f64>),
}

impl AngleSet {
    pub fn allows(&self, deg: f64) -> bool {
        match self {
            AngleSet::Any => true,
            AngleSet::List(v) => v.iter().any(|a| (a - deg).abs() <= ANGLE_TOL),
        }
    }
}

/// Whether a branch joins a run with a tee (the run is split) or a tap (the run stays whole).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Junction {
    Tee,
    Tap,
}

/// The routing preferences for one system: what fittings exist and how big they are. All numbers are the
/// author's; nothing is assumed.
#[derive(Clone, Debug, PartialEq)]
pub struct RoutingPrefs {
    pub domain: Domain,
    pub system: System,
    pub label: String,
    /// angles of the elbows that exist
    pub elbow_angles: AngleSet,
    /// bend radius = this x the larger section dimension
    pub bend_radius_factor: f64,
    /// shortest straight run between two fittings, metres
    pub min_straight: f64,
    pub junction: Junction,
    /// angles (to the run, 0 to 180) a branch may leave at; an angle and its supplement are both accepted
    pub tee_angles: AngleSet,
    /// half the length of a tee along the run = this x the larger section dimension of the run
    pub tee_half_factor: f64,
    /// length of a tee branch = this x the larger section dimension of the run
    pub branch_stub_factor: f64,
    /// how far from a run a branch may start and still be taken as starting on it, metres
    pub pick_tolerance: f64,
    /// largest included angle of a transition, degrees (more than 0, less than 180)
    pub transition_angle_deg: f64,
    /// shortest transition, metres
    pub min_transition_len: f64,
    /// straight length leaving an equipment connector, metres
    pub connector_stub: f64,
    /// space added all round the run (insulation, clearance), metres; used for wall openings
    pub margin: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RouteError {
    Snap(SnapError),
    BadPrefs(&'static str),
    BadInput(&'static str),
    /// no elbow of this angle exists in the preferences
    BendNotAvailable { vertex: usize, angle: f64 },
    /// the path turns back on itself
    Reversal { vertex: usize },
    /// the straight run between fittings is too short
    SegmentTooShort { segment: usize },
    /// the size changes at a bend; put the change on a straight length
    SizeChangeAtBend { vertex: usize },
    /// a transition between a round and a rectangular section is not made here
    MixedTransition { vertex: usize },
    /// the branch does not start on the run
    NotOnRun,
    TeeAngleNotAvailable { angle: f64 },
    TeeTooCloseToEnd,
    /// the connector does not belong with these preferences (domain or system), or cannot take a run
    ConnectorMismatch,
}

impl std::fmt::Display for RouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RouteError {}

#[derive(Clone, Debug, PartialEq)]
pub enum FittingKind {
    Elbow { angle: f64 },
    Transition,
    Tee { branch_angle: f64 },
    Tap { branch_angle: f64 },
}

/// A fitting: where the centrelines meet, the ports (unit directions leaving the fitting) and their sizes.
#[derive(Clone, Debug, PartialEq)]
pub struct Fitting {
    pub kind: FittingKind,
    pub at: [f64; 3],
    pub ports: Vec<[f64; 3]>,
    pub port_sizes: Vec<RunSize>,
    /// bend radius for an elbow, 0 otherwise
    pub radius: f64,
    /// length along each port that the fitting takes from the straight run
    pub trim: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    /// the drawn points after snapping (collinear points of one size are merged)
    pub points: Vec<[f64; 3]>,
    pub runs: Vec<Run>,
    pub fittings: Vec<Fitting>,
}

impl Route {
    /// Length of the centreline: straight runs plus the arcs of the elbows plus the other fittings' lengths.
    pub fn centreline_length(&self) -> f64 {
        let straight: f64 = self.runs.iter().map(|r| dist(r.a, r.b)).sum();
        let fit: f64 = self
            .fittings
            .iter()
            .map(|f| match &f.kind {
                FittingKind::Elbow { angle } => f.radius * angle.to_radians(),
                FittingKind::Transition => f.trim.iter().sum::<f64>(),
                _ => 0.0,
            })
            .sum();
        straight + fit
    }
    /// The same route walked from the other end.
    pub fn reversed(&self) -> Route {
        let mut r = self.clone();
        r.points.reverse();
        r.runs.reverse();
        for run in &mut r.runs {
            std::mem::swap(&mut run.a, &mut run.b);
        }
        r
    }
}

// ---------- vector helpers ----------

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add_s(a: [f64; 3], d: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] + d[0] * s, a[1] + d[1] * s, a[2] + d[2] * s]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    len(sub(a, b))
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let n = len(a);
    [a[0] / n, a[1] / n, a[2] / n]
}
fn neg(a: [f64; 3]) -> [f64; 3] {
    [-a[0], -a[1], -a[2]]
}
/// sine of an angle in degrees, exact at multiples of 90
fn sin_deg(x: f64) -> f64 {
    let r = x.rem_euclid(360.0);
    if r % 90.0 == 0.0 {
        return match (r / 90.0) as u8 {
            0 | 2 => 0.0,
            1 => 1.0,
            _ => -1.0,
        };
    }
    r.to_radians().sin()
}
fn cos_deg(x: f64) -> f64 {
    sin_deg(x.rem_euclid(360.0) + 90.0)
}
fn angle_deg(a: [f64; 3], b: [f64; 3]) -> f64 {
    dot(a, b).clamp(-1.0, 1.0).acos().to_degrees()
}

/// The larger section dimension, the reference for radii and fitting lengths.
pub fn ref_dim(s: RunSize) -> f64 {
    match s {
        RunSize::Round { diameter } => diameter,
        RunSize::Rect { width, height, .. } => width.max(height),
    }
}

fn size_ok(s: RunSize) -> bool {
    match s {
        RunSize::Round { diameter } => diameter.is_finite() && diameter > 0.0 && diameter <= 100.0,
        RunSize::Rect { width, height, roll } => width.is_finite() && height.is_finite() && roll.is_finite() && width > 0.0 && height > 0.0 && width <= 100.0 && height <= 100.0,
    }
}

fn same_size(a: RunSize, b: RunSize) -> bool {
    match (a, b) {
        (RunSize::Round { diameter: x }, RunSize::Round { diameter: y }) => (x - y).abs() <= EPS,
        (RunSize::Rect { width: w1, height: h1, .. }, RunSize::Rect { width: w2, height: h2, .. }) => (w1 - w2).abs() <= EPS && (h1 - h2).abs() <= EPS,
        _ => false,
    }
}

impl RoutingPrefs {
    pub fn check(&self) -> Result<(), RouteError> {
        let bad = |w: &'static str| Err(RouteError::BadPrefs(w));
        let pos = |v: f64| v.is_finite() && v > 0.0 && v <= 1000.0;
        let nonneg = |v: f64| v.is_finite() && (0.0..=1000.0).contains(&v);
        if self.domain == Domain::Electrical {
            return bad("an electrical domain has no routed runs");
        }
        if !self.system.domain_ok(self.domain) {
            return bad("the system does not belong to this domain");
        }
        if let AngleSet::List(v) = &self.elbow_angles {
            if v.is_empty() || v.len() > 64 || v.iter().any(|a| !(a.is_finite() && *a > 0.0 && *a < 180.0)) {
                return bad("elbow angles must be 1 to 64 angles between 0 and 180");
            }
        }
        if let AngleSet::List(v) = &self.tee_angles {
            if v.is_empty() || v.len() > 64 || v.iter().any(|a| !(a.is_finite() && *a > 0.0 && *a < 180.0)) {
                return bad("tee angles must be 1 to 64 angles between 0 and 180");
            }
        }
        if !pos(self.bend_radius_factor) {
            return bad("bend radius factor must be more than 0");
        }
        if !nonneg(self.min_straight) || !nonneg(self.pick_tolerance) || !nonneg(self.min_transition_len) || !nonneg(self.connector_stub) || !nonneg(self.tee_half_factor) || !nonneg(self.branch_stub_factor) {
            return bad("lengths and factors cannot be negative or infinite");
        }
        if !(self.transition_angle_deg.is_finite() && self.transition_angle_deg > 0.0 && self.transition_angle_deg < 180.0) {
            return bad("transition angle must be more than 0 and less than 180 degrees");
        }
        if !(self.margin.is_finite() && (0.0..=10.0).contains(&self.margin)) {
            return bad("margin must be from 0 to 10 metres");
        }
        Ok(())
    }

    fn run(&self, idx: usize, a: [f64; 3], b: [f64; 3], size: RunSize) -> Run {
        Run { label: format!("{}-{}", self.label, idx + 1), domain: self.domain, system: self.system, a, b, size, margin: self.margin }
    }

    fn transition_len(&self, from: RunSize, to: RunSize) -> Option<f64> {
        let delta = match (from, to) {
            (RunSize::Round { diameter: a }, RunSize::Round { diameter: b }) => (a - b).abs(),
            (RunSize::Rect { width: w1, height: h1, .. }, RunSize::Rect { width: w2, height: h2, .. }) => (w1 - w2).abs().max((h1 - h2).abs()),
            _ => return None,
        };
        Some(self.min_transition_len.max(delta / (2.0 * (self.transition_angle_deg.to_radians() / 2.0).tan())))
    }
}

// ---------- drawing ----------

struct Opts {
    /// length taken off the start of the first run (for a tee branch)
    lead_start: f64,
    /// the first leg is exactly as given, not snapped (for a connector stub)
    keep_first: bool,
}

/// Snap the drawn points one by one, each relative to the one before.
fn snap_points(points: &[[f64; 3]], snaps: &Snaps, keep_first: bool) -> Result<Vec<[f64; 3]>, RouteError> {
    let s = snaps.active();
    s.check().map_err(RouteError::Snap)?;
    let mut out = vec![points[0]];
    let mut last_az = 0.0f64;
    for i in 1..points.len() {
        let prev = out[i - 1];
        let v = sub(points[i], prev);
        let l = len(v);
        if l.is_nan() || l <= 1e-6 {
            return Err(RouteError::BadInput("two points coincide"));
        }
        let h = (v[0] * v[0] + v[1] * v[1]).sqrt();
        let el = v[2].atan2(h).to_degrees();
        let az = if h > 1e-9 { v[1].atan2(v[0]).to_degrees() } else { last_az };
        let (az2, el2) = if keep_first && i == 1 { (az, el) } else { (s.plan.snap_plan(az), s.vertical.snap_vertical(el)) };
        last_az = az2;
        let (ce, se) = (cos_deg(el2), sin_deg(el2));
        let d = [ce * cos_deg(az2), ce * sin_deg(az2), se];
        out.push(add_s(prev, d, l));
    }
    Ok(out)
}

fn route_inner(points: &[[f64; 3]], sizes: &[RunSize], prefs: &RoutingPrefs, snaps: &Snaps, o: &Opts) -> Result<Route, RouteError> {
    prefs.check()?;
    if points.len() < 2 || points.len() > MAX_POINTS {
        return Err(RouteError::BadInput("a route needs 2 to 1000 points"));
    }
    if sizes.len() != points.len() - 1 {
        return Err(RouteError::BadInput("one size is needed for each straight leg"));
    }
    if points.iter().any(|p| p.iter().any(|v| !v.is_finite() || v.abs() > 1.0e6)) || !sizes.iter().all(|s| size_ok(*s)) {
        return Err(RouteError::BadInput("a point or size is not a finite number within range"));
    }
    let snapped = snap_points(points, snaps, o.keep_first)?;
    // merge legs that continue straight on in the same size
    let mut pts = vec![snapped[0]];
    let mut szs: Vec<RunSize> = Vec::new();
    for i in 0..snapped.len() - 1 {
        let d = unit(sub(snapped[i + 1], snapped[i]));
        if let Some(&sz) = szs.last() {
            let pd = unit(sub(pts[pts.len() - 1], pts[pts.len() - 2]));
            if dot(pd, d) > 1.0 - 1e-12 && same_size(sz, sizes[i]) {
                let n = pts.len();
                pts[n - 1] = snapped[i + 1];
                continue;
            }
        }
        pts.push(snapped[i + 1]);
        szs.push(sizes[i]);
    }
    let n = pts.len();
    let dirs: Vec<[f64; 3]> = (0..n - 1).map(|i| unit(sub(pts[i + 1], pts[i]))).collect();
    let seg_len: Vec<f64> = (0..n - 1).map(|i| dist(pts[i], pts[i + 1])).collect();
    let mut trim_s = vec![0.0; n - 1];
    let mut trim_e = vec![0.0; n - 1];
    trim_s[0] = o.lead_start;
    let mut fittings = Vec::new();
    for v in 1..n - 1 {
        let (din, dout) = (dirs[v - 1], dirs[v]);
        let theta = angle_deg(din, dout);
        if theta > 179.99 {
            return Err(RouteError::Reversal { vertex: v });
        }
        if theta < 1e-4 {
            // straight on, so the size must differ (equal sizes were merged): a transition
            let l = prefs.transition_len(szs[v - 1], szs[v]).ok_or(RouteError::MixedTransition { vertex: v })?;
            trim_s[v] += l;
            fittings.push(Fitting { kind: FittingKind::Transition, at: pts[v], ports: vec![neg(din), dout], port_sizes: vec![szs[v - 1], szs[v]], radius: 0.0, trim: vec![0.0, l] });
            continue;
        }
        if !same_size(szs[v - 1], szs[v]) {
            return Err(RouteError::SizeChangeAtBend { vertex: v });
        }
        if !prefs.elbow_angles.allows(theta) {
            return Err(RouteError::BendNotAvailable { vertex: v, angle: theta });
        }
        let r = prefs.bend_radius_factor * ref_dim(szs[v]);
        let t = r * (theta.to_radians() / 2.0).tan();
        trim_e[v - 1] += t;
        trim_s[v] += t;
        fittings.push(Fitting { kind: FittingKind::Elbow { angle: theta }, at: pts[v], ports: vec![neg(din), dout], port_sizes: vec![szs[v], szs[v]], radius: r, trim: vec![t, t] });
    }
    let mut runs = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        let left = seg_len[i] - trim_s[i] - trim_e[i];
        if left < prefs.min_straight - EPS || left < -EPS {
            return Err(RouteError::SegmentTooShort { segment: i });
        }
        runs.push(prefs.run(i, add_s(pts[i], dirs[i], trim_s[i]), add_s(pts[i + 1], dirs[i], -trim_e[i]), szs[i]));
    }
    Ok(Route { points: pts, runs, fittings })
}

/// Draw a route through `points` (before snapping), one size per leg. Elbows, transitions and the
/// shortened straight runs follow from the preferences and the snaps in force.
pub fn route_path(points: &[[f64; 3]], sizes: &[RunSize], prefs: &RoutingPrefs, snaps: &Snaps) -> Result<Route, RouteError> {
    route_inner(points, sizes, prefs, snaps, &Opts { lead_start: 0.0, keep_first: false })
}

// ---------- branches ----------

/// A branch joined to a run.
#[derive(Clone, Debug, PartialEq)]
pub struct Branch {
    /// the run after the junction: two pieces for a tee, the whole run for a tap
    pub main: Vec<Run>,
    pub fitting: Fitting,
    /// the branch, starting at the fitting
    pub route: Route,
}

/// Join a branch to `main`. The branch starts at `branch_points[0]`, which must lie within the pick
/// tolerance of the run (a tee on a drop: the start is dropped onto the run). `branch_sizes` is per leg.
pub fn branch_from_run(main: &Run, branch_points: &[[f64; 3]], branch_sizes: &[RunSize], prefs: &RoutingPrefs, snaps: &Snaps) -> Result<Branch, RouteError> {
    prefs.check()?;
    if branch_points.len() < 2 || branch_sizes.len() != branch_points.len() - 1 || branch_points.iter().any(|p| p.iter().any(|v| !v.is_finite())) {
        return Err(RouteError::BadInput("a branch needs at least 2 points and one size per leg"));
    }
    let ml = dist(main.a, main.b);
    if ml.is_nan() || ml <= 1e-6 {
        return Err(RouteError::BadInput("the run has no length"));
    }
    let dm = unit(sub(main.b, main.a));
    let s = dot(sub(branch_points[0], main.a), dm);
    let foot = add_s(main.a, dm, s);
    if dist(foot, branch_points[0]) > prefs.pick_tolerance + EPS || s < -EPS || s > ml + EPS {
        return Err(RouteError::NotOnRun);
    }
    // snap the branch from the foot, to know which way it really leaves
    let mut pts = branch_points.to_vec();
    pts[0] = foot;
    let snapped = snap_points(&pts, snaps, false)?;
    let d0 = unit(sub(snapped[1], snapped[0]));
    let alpha = angle_deg(dm, d0);
    if !(1.0..=179.0).contains(&alpha) {
        return Err(RouteError::BadInput("the branch runs along the run"));
    }
    if !(prefs.tee_angles.allows(alpha) || prefs.tee_angles.allows(180.0 - alpha)) {
        return Err(RouteError::TeeAngleNotAvailable { angle: alpha });
    }
    let half = prefs.tee_half_factor * ref_dim(main.size);
    let stub = prefs.branch_stub_factor * ref_dim(main.size);
    let route = route_inner(&pts, branch_sizes, prefs, snaps, &Opts { lead_start: stub, keep_first: false })?;
    let (mut runs, fitting) = match prefs.junction {
        Junction::Tee => {
            if s - half < prefs.min_straight - EPS || ml - s - half < prefs.min_straight - EPS {
                return Err(RouteError::TeeTooCloseToEnd);
            }
            let mut r1 = main.clone();
            r1.b = add_s(foot, dm, -half);
            let mut r2 = main.clone();
            r2.a = add_s(foot, dm, half);
            r1.label = format!("{}.1", main.label);
            r2.label = format!("{}.2", main.label);
            let f = Fitting {
                kind: FittingKind::Tee { branch_angle: alpha },
                at: foot,
                ports: vec![neg(dm), dm, d0],
                port_sizes: vec![main.size, main.size, branch_sizes[0]],
                radius: 0.0,
                trim: vec![half, half, stub],
            };
            (vec![r1, r2], f)
        }
        Junction::Tap => {
            let f = Fitting { kind: FittingKind::Tap { branch_angle: alpha }, at: foot, ports: vec![d0], port_sizes: vec![branch_sizes[0]], radius: 0.0, trim: vec![stub] };
            (vec![main.clone()], f)
        }
    };
    runs.shrink_to_fit();
    Ok(Branch { main: runs, fitting, route })
}

// ---------- equipment ----------

/// Start a route at an equipment connector (in model coordinates). It leaves the connector straight, along
/// the way the connector faces, for the connector stub at the connector's own size; the rest of the path
/// follows in `size`, with a transition where the sizes differ. `points` are the points after the connector.
pub fn route_from_connector(conn: &Connector, points: &[[f64; 3]], size: RunSize, prefs: &RoutingPrefs, snaps: &Snaps) -> Result<Route, RouteError> {
    prefs.check()?;
    if conn.domain != prefs.domain || !conn.system.compatible(prefs.system) {
        return Err(RouteError::ConnectorMismatch);
    }
    let csize = match conn.size {
        Size::Round { diameter } => RunSize::Round { diameter },
        Size::Rect { width, height } => {
            // roll the section so its height lies along the connector's own up
            let base = up_for(conn.dir, 0.0);
            let w = cross(conn.dir, base);
            let roll = dot(conn.up, w).atan2(dot(conn.up, base)).to_degrees();
            RunSize::Rect { width, height, roll }
        }
        Size::Electrical { .. } => return Err(RouteError::ConnectorMismatch),
    };
    if points.is_empty() {
        return Err(RouteError::BadInput("a route needs at least one point after the connector"));
    }
    let d = unit(conn.dir);
    let mut pts = vec![conn.pos, add_s(conn.pos, d, prefs.connector_stub.max(1e-3))];
    pts.extend_from_slice(points);
    let mut sizes = vec![csize];
    sizes.extend(std::iter::repeat_n(size, points.len()));
    // the stub leg is exactly the connector's direction; the first point after it is snapped from the stub end
    route_inner(&pts, &sizes, prefs, snaps, &Opts { lead_start: 0.0, keep_first: true })
}
