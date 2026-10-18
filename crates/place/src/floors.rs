//! Floor penetrations: where a service run crosses a floor slab (a riser), an opening is needed unless the run
//! goes through a declared shaft, which is already a void through the floors.
//!
//! The opening is the exact bounding rectangle, along X and Y, of the run's section carried through the slab
//! thickness, so a leaning riser gets a longer opening than a plumb one. Openings that touch or overlap (or lie
//! within `merge_gap`) in one slab are merged. A merged opening that lies inside a shaft void that cuts the slab
//! is recorded as being in that shaft; one that does not must lie inside the slab outline or it cannot be cut.
use crate::penetrations::{Run, RunIssue, RunProblem, RunSize};
use openbim_arch::spaces::Shaft;
use openbim_arch::{ArchError, Levels};
use openbim_family::mep::up_for;
use openbim_family::{Domain, System};

type P = [f64; 2];

pub const MAX_SLABS: usize = 100_000;
pub const MAX_SHAFTS: usize = 100_000;

/// A floor slab: the top surface height, the thickness, the plan outline and the fire rating of the floor.
#[derive(Debug, Clone, PartialEq)]
pub struct Slab {
    pub name: String,
    /// height of the top surface
    pub top: f64,
    pub thickness: f64,
    /// plan outline, a simple polygon in any direction
    pub outline: Vec<P>,
    /// fire resistance of the floor in minutes, as the author sets it; None = not a fire separation
    pub fire_rating_min: Option<u32>,
}

/// A vertical void in plan (from a shaft) between two heights. It cuts every slab whose top is strictly between.
#[derive(Debug, Clone, PartialEq)]
pub struct ShaftVoid {
    pub name: String,
    pub outline: Vec<P>,
    pub z_bottom: f64,
    pub z_top: f64,
}

impl ShaftVoid {
    pub fn from_shaft(s: &Shaft, levels: &Levels) -> Result<ShaftVoid, ArchError> {
        s.check(levels)?;
        Ok(ShaftVoid { name: s.name.clone(), outline: s.outline.clone(), z_bottom: levels.all()[s.from].elevation, z_top: levels.all()[s.to].elevation })
    }
    fn cuts(&self, slab: &Slab) -> bool {
        slab.top > self.z_bottom + 1e-6 && slab.top < self.z_top - 1e-6
    }
}

/// A void cut in one slab of a scene (`slab` indexes the scene's slabs): an outline in plan that goes through its whole
/// thickness. It may reach the outline of the slab.
#[derive(Debug, Clone, PartialEq)]
pub struct SlabVoid {
    pub slab: usize,
    pub name: String,
    pub outline: Vec<P>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlabError {
    Name,
    Outline,
    Thickness,
    Top,
    Fire,
    TooMany,
}

impl Slab {
    pub fn check(&self) -> Result<(), SlabError> {
        if self.name.is_empty() || self.name.chars().count() > 64 || self.name.chars().any(|c| c.is_control()) {
            return Err(SlabError::Name);
        }
        if !(self.thickness.is_finite() && self.thickness > 0.0 && self.thickness <= 5.0) {
            return Err(SlabError::Thickness);
        }
        if !(self.top.is_finite() && self.top.abs() <= 1.0e5) {
            return Err(SlabError::Top);
        }
        check_ring(&self.outline).map_err(|_| SlabError::Outline)
    }
}

fn check_ring(r: &[P]) -> Result<(), ()> {
    if r.len() < 3 || r.len() > 1000 || r.iter().any(|p| !p[0].is_finite() || !p[1].is_finite() || p[0].abs() > 1.0e6 || p[1].abs() > 1.0e6) {
        return Err(());
    }
    if area(r).abs() < 1e-6 {
        return Err(());
    }
    let n = r.len();
    for i in 0..n {
        for j in (i + 2)..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            if crosses(r[i], r[(i + 1) % n], r[j], r[(j + 1) % n]) {
                return Err(());
            }
        }
    }
    Ok(())
}

fn area(r: &[P]) -> f64 {
    let n = r.len();
    (0..n).map(|i| r[i][0] * r[(i + 1) % n][1] - r[(i + 1) % n][0] * r[i][1]).sum::<f64>() / 2.0
}
fn orient(a: P, b: P, c: P) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
/// Proper crossing only: touching ends do not count.
fn crosses(a: P, b: P, c: P, d: P) -> bool {
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    o1 != 0.0 && o2 != 0.0 && o3 != 0.0 && o4 != 0.0 && ((o1 > 0.0) != (o2 > 0.0)) && ((o3 > 0.0) != (o4 > 0.0))
}
fn point_in(r: &[P], q: P) -> bool {
    let n = r.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        if (r[i][1] > q[1]) != (r[j][1] > q[1]) && q[0] < (r[j][0] - r[i][0]) * (q[1] - r[i][1]) / (r[j][1] - r[i][1]) + r[i][0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}
/// Does the rectangle lie inside the polygon (corners inside, no polygon edge crossing a rectangle edge, no
/// polygon corner inside the rectangle)?
pub fn rect_in_ring(r: &[P], x0: f64, x1: f64, y0: f64, y1: f64) -> bool {
    let c = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    if !c.iter().all(|q| point_in(r, *q)) {
        return false;
    }
    let n = r.len();
    for i in 0..n {
        let (a, b) = (r[i], r[(i + 1) % n]);
        if (0..4).any(|k| crosses(a, b, c[k], c[(k + 1) % 4])) {
            return false;
        }
        // a polygon corner strictly inside the rectangle means the outline dips into it
        if a[0] > x0 && a[0] < x1 && a[1] > y0 && a[1] < y1 {
            return false;
        }
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloorPenId {
    pub slab: usize,
    pub seq: usize,
}

impl std::fmt::Display for FloorPenId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "F{}-P{}", self.slab, self.seq + 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloorOpeningKind {
    /// an opening of its own is cut in the slab
    Cut,
    /// the run is inside this shaft (index into the shaft voids), which is already a void through the slab
    InShaft(usize),
}

/// One opening in one slab, with everything that passes through it.
#[derive(Debug, Clone, PartialEq)]
pub struct FloorPenetration {
    pub id: FloorPenId,
    pub runs: Vec<usize>,
    pub systems: Vec<System>,
    pub domains: Vec<Domain>,
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
    /// centre of the opening at the middle of the slab thickness
    pub centre: [f64; 3],
    pub fire_rating_min: Option<u32>,
    pub kind: FloorOpeningKind,
    /// false when an opening of its own would not lie inside the slab outline (so it cannot be cut)
    pub fits: bool,
}

impl FloorPenetration {
    pub fn in_fire_partition(&self) -> bool {
        self.fire_rating_min.is_some_and(|m| m > 0)
    }
    pub fn size_x(&self) -> f64 {
        self.x1 - self.x0
    }
    pub fn size_y(&self) -> f64 {
        self.y1 - self.y0
    }
    pub fn area(&self) -> f64 {
        self.size_x() * self.size_y()
    }
}

/// The floor openings as text for a schedule: one row per opening, comma separated, metres and minutes.
pub fn floor_register_csv(pens: &[FloorPenetration], runs: &[Run]) -> String {
    let mut s = String::from("id,slab,x0_m,x1_m,y0_m,y1_m,size_x_m,size_y_m,x_m,y_m,z_m,fire_rating_min,fire_partition,kind,fits,runs,systems\n");
    for p in pens {
        let labels: Vec<String> = p.runs.iter().map(|&r| crate::penetrations::csv_safe(&runs[r].label)).collect();
        let systems: Vec<String> = p.systems.iter().map(|x| format!("{x:?}")).collect();
        let kind = match p.kind {
            FloorOpeningKind::Cut => "cut".to_string(),
            FloorOpeningKind::InShaft(i) => format!("shaft {i}"),
        };
        s.push_str(&format!(
            "{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{},{},{},{},{},{}\n",
            p.id, p.id.slab, p.x0, p.x1, p.y0, p.y1, p.size_x(), p.size_y(), p.centre[0], p.centre[1], p.centre[2],
            p.fire_rating_min.map(|m| m.to_string()).unwrap_or_default(), p.in_fire_partition(), kind, p.fits, labels.join(" | "), systems.join(" | ")
        ));
    }
    s
}

const EPS: f64 = 1e-9;

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let n = dot(a, a).sqrt();
    [a[0] / n, a[1] / n, a[2] / n]
}

enum Cross {
    None,
    Problem(RunProblem),
    Rect([f64; 4]),
}

/// Half extent, along unit vector `e`, of the run's section on a plane with normal Z (see wall penetrations).
fn half_extent(r: &Run, d: [f64; 3], e: [f64; 3]) -> f64 {
    let k = dot(e, d) / d[2];
    let g = [e[0], e[1], e[2] - k];
    let t = dot(g, d);
    let gp = [g[0] - t * d[0], g[1] - t * d[1], g[2] - t * d[2]];
    match r.size {
        RunSize::Round { diameter } => (diameter / 2.0 + r.margin) * dot(gp, gp).sqrt(),
        RunSize::Rect { width, height, roll } => {
            let up = up_for(d, roll);
            let wd = unit(cross3(d, up));
            (width / 2.0 + r.margin) * dot(gp, wd).abs() + (height / 2.0 + r.margin) * dot(gp, up).abs()
        }
    }
}

fn cross_slab(r: &Run, s: &Slab) -> Cross {
    let l = dot(sub(r.b, r.a), sub(r.b, r.a)).sqrt();
    let d = unit(sub(r.b, r.a));
    let (top, bot) = (s.top, s.top - s.thickness);
    if d[2].abs() < EPS {
        // level run: only a problem when it lies in the slab thickness over the slab
        if r.a[2] > bot + EPS && r.a[2] < top - EPS {
            let (x0, x1) = (r.a[0].min(r.b[0]), r.a[0].max(r.b[0]));
            let (y0, y1) = (r.a[1].min(r.b[1]), r.a[1].max(r.b[1]));
            let (sx0, sx1) = s.outline.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p[0]), a.1.max(p[0])));
            let (sy0, sy1) = s.outline.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p[1]), a.1.max(p[1])));
            if x1 > sx0 + EPS && x0 < sx1 - EPS && y1 > sy0 + EPS && y0 < sy1 - EPS {
                return Cross::Problem(RunProblem::InsideSlab);
            }
        }
        return Cross::None;
    }
    let s_of = |z: f64| (z - r.a[2]) / d[2];
    let (s1, s2) = (s_of(top), s_of(bot));
    let (lo, hi) = (s1.min(s2), s1.max(s2));
    if hi <= EPS || lo >= l - EPS {
        return Cross::None;
    }
    let mid = {
        let sm = s_of((top + bot) / 2.0);
        [r.a[0] + sm * d[0], r.a[1] + sm * d[1]]
    };
    if !point_in(&s.outline, mid) {
        return Cross::None; // passes beside the slab
    }
    if lo < -EPS || hi > l + EPS {
        return Cross::Problem(RunProblem::EndsInSlab);
    }
    let (ex, ey) = (half_extent(r, d, [1.0, 0.0, 0.0]), half_extent(r, d, [0.0, 1.0, 0.0]));
    let at = |z: f64| {
        let sz = s_of(z);
        [r.a[0] + sz * d[0], r.a[1] + sz * d[1]]
    };
    let (c1, c2) = (at(top), at(bot));
    Cross::Rect([(c1[0] - ex).min(c2[0] - ex), (c1[0] + ex).max(c2[0] + ex), (c1[1] - ey).min(c2[1] - ey), (c1[1] + ey).max(c2[1] + ey)])
}

/// All floor penetrations of `runs` through `slabs`, and the runs that have a problem. A run that is in a shaft
/// that cuts the slab is recorded as in the shaft. Order: by slab, then x, then y.
pub fn find_floor(slabs: &[Slab], shafts: &[ShaftVoid], runs: &[Run], merge_gap: f64) -> (Vec<FloorPenetration>, Vec<RunIssue>) {
    let mut issues = Vec::new();
    let mut found: Vec<Vec<(usize, [f64; 4])>> = vec![Vec::new(); slabs.len()];
    let gap = if merge_gap.is_finite() && merge_gap > 0.0 { merge_gap } else { 0.0 };
    for (ri, r) in runs.iter().enumerate() {
        // bad run data is reported by the wall pass; here it is simply skipped
        let finite = r.a.iter().chain(r.b.iter()).all(|v| v.is_finite() && v.abs() <= 1.0e6);
        let l = dot(sub(r.b, r.a), sub(r.b, r.a)).sqrt();
        let size_ok = match r.size {
            RunSize::Round { diameter } => diameter.is_finite() && diameter > 0.0 && diameter <= 100.0,
            RunSize::Rect { width, height, roll } => width.is_finite() && height.is_finite() && roll.is_finite() && width > 0.0 && height > 0.0 && width <= 100.0 && height <= 100.0,
        };
        if !(finite && l > 1e-6 && size_ok && r.margin.is_finite() && (0.0..=10.0).contains(&r.margin)) {
            continue;
        }
        let mut problem = None;
        let mut hits = Vec::new();
        for (si, s) in slabs.iter().enumerate() {
            match cross_slab(r, s) {
                Cross::None => {}
                Cross::Problem(p) => problem = problem.or(Some(p)),
                Cross::Rect(rect) => hits.push((si, rect)),
            }
        }
        match problem {
            Some(p) => issues.push(RunIssue { run: ri, problem: p }),
            None => {
                for (si, rect) in hits {
                    found[si].push((ri, rect));
                }
            }
        }
    }
    let mut out = Vec::new();
    for (si, list) in found.iter().enumerate() {
        let mut groups: Vec<(Vec<usize>, [f64; 4])> = list.iter().map(|(r, b)| (vec![*r], *b)).collect();
        loop {
            let mut merged = false;
            'scan: for a in 0..groups.len() {
                for b in a + 1..groups.len() {
                    let (ra, rb) = (groups[a].1, groups[b].1);
                    if ra[0] <= rb[1] + gap + EPS && rb[0] <= ra[1] + gap + EPS && ra[2] <= rb[3] + gap + EPS && rb[2] <= ra[3] + gap + EPS {
                        let g = groups.remove(b);
                        groups[a].0.extend(g.0);
                        groups[a].1 = [ra[0].min(rb[0]), ra[1].max(rb[1]), ra[2].min(rb[2]), ra[3].max(rb[3])];
                        merged = true;
                        break 'scan;
                    }
                }
            }
            if !merged {
                break;
            }
        }
        groups.sort_by(|x, y| x.1[0].total_cmp(&y.1[0]).then(x.1[2].total_cmp(&y.1[2])));
        let s = &slabs[si];
        for (seq, (mut rs, b)) in groups.into_iter().enumerate() {
            rs.sort_unstable();
            rs.dedup();
            let mut systems: Vec<System> = Vec::new();
            let mut domains: Vec<Domain> = Vec::new();
            for &r in &rs {
                if !systems.contains(&runs[r].system) {
                    systems.push(runs[r].system);
                }
                if !domains.contains(&runs[r].domain) {
                    domains.push(runs[r].domain);
                }
            }
            let shaft = shafts.iter().position(|v| v.cuts(s) && rect_in_ring(&v.outline, b[0], b[1], b[2], b[3]));
            let kind = match shaft {
                Some(k) => FloorOpeningKind::InShaft(k),
                None => FloorOpeningKind::Cut,
            };
            let fits = shaft.is_some() || rect_in_ring(&s.outline, b[0], b[1], b[2], b[3]);
            out.push(FloorPenetration {
                id: FloorPenId { slab: si, seq },
                runs: rs,
                systems,
                domains,
                x0: b[0],
                x1: b[1],
                y0: b[2],
                y1: b[3],
                centre: [(b[0] + b[1]) / 2.0, (b[2] + b[3]) / 2.0, s.top - s.thickness / 2.0],
                fire_rating_min: s.fire_rating_min,
                kind,
                fits,
            });
        }
    }
    issues.sort_by_key(|i| i.run);
    (out, issues)
}
