//! Wall penetrations: where a duct, pipe, conduit or cable tray run crosses a wall, an opening is needed.
//!
//! A `Run` is a straight piece of service (3D start and end, a size and a margin). For every wall segment
//! it crosses, the opening is the exact bounding rectangle, in the wall's own frame (along the wall, up),
//! of the run's cross-section carried through the wall thickness, so a skewed crossing gets a wider
//! opening than a square one. Openings that overlap (or lie within `merge_gap`) in the same wall segment
//! become one `Penetration`, because several services often share one opening and one firestop.
//! Each penetration remembers its runs, systems and the fire rating of the wall it passes, so the register can
//! be listed, filtered to fire partitions and exported. No standard values live here: ratings, margins and
//! gaps all come from the author.
use openbim_family::mep::up_for;
use openbim_family::{Domain, System};
use openbim_geom::WallChain;

pub const MAX_RUNS: usize = 1_000_000;

/// Cross-section of a run (metres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RunSize {
    Round { diameter: f64 },
    /// width is across, height is along the run's "up" (default +Z, or +Y for a vertical run), `roll` turns it (degrees)
    Rect { width: f64, height: f64, roll: f64 },
}

/// A straight run of duct, pipe, conduit or cable tray.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub label: String,
    pub domain: Domain,
    pub system: System,
    pub a: [f64; 3],
    pub b: [f64; 3],
    pub size: RunSize,
    /// space added all round the run (insulation, sleeve, clearance), metres
    pub margin: f64,
}

impl Run {
    pub fn round(label: &str, domain: Domain, system: System, a: [f64; 3], b: [f64; 3], diameter: f64) -> Run {
        Run { label: label.into(), domain, system, a, b, size: RunSize::Round { diameter }, margin: 0.0 }
    }
    pub fn rect(label: &str, domain: Domain, system: System, a: [f64; 3], b: [f64; 3], width: f64, height: f64) -> Run {
        Run { label: label.into(), domain, system, a, b, size: RunSize::Rect { width, height, roll: 0.0 }, margin: 0.0 }
    }
    pub fn with_margin(mut self, m: f64) -> Run {
        self.margin = m;
        self
    }
}

/// Why a run could not be turned into penetrations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunProblem {
    /// a number is not finite, the run has no length, or a size or margin is not allowed
    BadData,
    /// the run lies inside the thickness of a wall instead of crossing it
    InsideWall,
    /// the run starts or ends inside a wall
    EndsInWall,
}

impl RunProblem {
    pub fn text(self) -> &'static str {
        match self {
            RunProblem::BadData => "the run has bad data (not finite, no length, or a size or margin that is not allowed)",
            RunProblem::InsideWall => "the run lies inside a wall",
            RunProblem::EndsInWall => "the run starts or ends inside a wall",
        }
    }
}

/// Which penetration: wall chain, segment, and the number along the segment (from its start).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PenetrationId {
    pub chain: usize,
    pub segment: usize,
    pub seq: usize,
}

impl std::fmt::Display for PenetrationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "W{}-S{}-P{}", self.chain, self.segment, self.seq + 1)
    }
}

/// One opening in one wall segment, with everything that passes through it.
#[derive(Debug, Clone, PartialEq)]
pub struct Penetration {
    pub id: PenetrationId,
    /// indices into `Model::runs`, ascending
    pub runs: Vec<usize>,
    /// distinct systems through the opening
    pub systems: Vec<System>,
    /// distinct domains through the opening
    pub domains: Vec<Domain>,
    /// along the wall centreline from the segment start
    pub offset: f64,
    pub width: f64,
    /// height of the opening bottom above the wall base
    pub sill: f64,
    pub height: f64,
    /// centre of the opening on the wall centreline, world coordinates
    pub centre: [f64; 3],
    /// fire rating of the wall in minutes (None = not a fire partition)
    pub fire_rating_min: Option<u32>,
    /// false when the opening could not be cut (it conflicts with another opening or leaves the wall)
    pub cut: bool,
}

impl Penetration {
    pub fn in_fire_partition(&self) -> bool {
        self.fire_rating_min.is_some_and(|m| m > 0)
    }
    /// Opening area in m2 (the clear area a firestop must fill, before the services are deducted).
    pub fn area(&self) -> f64 {
        self.width * self.height
    }
}

/// A run's problem, with the run it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunIssue {
    pub run: usize,
    pub problem: RunProblem,
}

const EPS: f64 = 1e-9;

/// per chain, per segment: the (run, opening rectangle) hits
type Hits = Vec<Vec<Vec<(usize, [f64; 4])>>>;

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let n = dot(a, a).sqrt();
    [a[0] / n, a[1] / n, a[2] / n]
}

fn check_run(r: &Run) -> bool {
    let finite = r.a.iter().chain(r.b.iter()).all(|v| v.is_finite() && v.abs() <= 1.0e6);
    let d = sub(r.b, r.a);
    let size_ok = match r.size {
        RunSize::Round { diameter } => diameter.is_finite() && diameter > 0.0 && diameter <= 100.0,
        RunSize::Rect { width, height, roll } => width.is_finite() && height.is_finite() && roll.is_finite() && width > 0.0 && height > 0.0 && width <= 100.0 && height <= 100.0,
    };
    finite && dot(d, d).sqrt() > 1e-6 && size_ok && r.margin.is_finite() && (0.0..=10.0).contains(&r.margin)
}

/// The opening rectangle (offset, width, sill, height) of a run through segment `i`, or what is wrong.
enum Cross {
    None,
    Problem(RunProblem),
    Opening { rect: [f64; 4] },
}

fn cross_segment(r: &Run, c: &WallChain, i: usize) -> Cross {
    let n = c.points.len();
    let (pa, pb) = (c.points[i], c.points[(i + 1) % n]);
    let (dx, dy) = (pb[0] - pa[0], pb[1] - pa[1]);
    let len = (dx * dx + dy * dy).sqrt();
    let uw = [dx / len, dy / len, 0.0];
    let nw = [-dy / len, dx / len, 0.0];
    let zw = [0.0, 0.0, 1.0];
    let org = [pa[0], pa[1], c.base_z];
    let frame = |v: [f64; 3]| [dot(v, uw), dot(v, nw), dot(v, zw)];
    let p = frame(sub(r.a, org));
    let q = frame(sub(r.b, org));
    let l = dot(sub(r.b, r.a), sub(r.b, r.a)).sqrt();
    let d = unit(sub(q, p));
    let half = c.thickness / 2.0;
    if d[1].abs() < EPS {
        // parallel to the wall plane: only a problem when it lies in the wall
        if p[1].abs() < half - EPS {
            let (u0, u1) = (p[0].min(q[0]), p[0].max(q[0]));
            let (z0, z1) = (p[2].min(q[2]), p[2].max(q[2]));
            if u1 > EPS && u0 < len - EPS && z1 > EPS && z0 < c.height - EPS {
                return Cross::Problem(RunProblem::InsideWall);
            }
        }
        return Cross::None;
    }
    let s_of = |v: f64| (v - p[1]) / d[1];
    let (s1, s2) = (s_of(-half), s_of(half));
    let (s_lo, s_hi) = (s1.min(s2), s1.max(s2));
    if s_hi <= EPS || s_lo >= l - EPS {
        return Cross::None; // wholly before, after, or only touching a face
    }
    // the position where it passes the middle of the wall decides whether this segment is the one
    let s_mid = s_of(0.0);
    let mid = [p[0] + s_mid * d[0], 0.0, p[2] + s_mid * d[2]];
    if mid[0] < -EPS || mid[0] > len + EPS || mid[2] < -EPS || mid[2] > c.height + EPS {
        // beside or over/under this wall segment; but if it ends inside the slab it is still a problem
        return Cross::None;
    }
    if s_lo < -EPS || s_hi > l + EPS {
        return Cross::Problem(RunProblem::EndsInWall);
    }
    // cross-section axes (perpendicular to the run) in the wall frame
    let dw = [d[0] * uw[0] + d[1] * nw[0], d[0] * uw[1] + d[1] * nw[1], d[2]];
    let ext = |e: [f64; 3]| -> f64 {
        // g = e - N (e.d) / (N.d): the direction in which the section on a wall face moves with the run's cross-section
        let k = dot(e, d) / d[1];
        let g = [e[0], e[1] - k, e[2]];
        let gp = {
            let t = dot(g, d);
            [g[0] - t * d[0], g[1] - t * d[1], g[2] - t * d[2]]
        };
        match r.size {
            RunSize::Round { diameter } => (diameter / 2.0 + r.margin) * dot(gp, gp).sqrt(),
            RunSize::Rect { width, height, roll } => {
                let up = up_for(dw, roll);
                let hd = frame(up);
                let wd = unit(cross(d, hd));
                (width / 2.0 + r.margin) * dot(gp, wd).abs() + (height / 2.0 + r.margin) * dot(gp, hd).abs()
            }
        }
    };
    let (eu, ez) = (ext([1.0, 0.0, 0.0]), ext([0.0, 0.0, 1.0]));
    let pt = |v: f64| {
        let s = s_of(v);
        [p[0] + s * d[0], v, p[2] + s * d[2]]
    };
    let (c1, c2) = (pt(-half), pt(half));
    let u0 = (c1[0] - eu).min(c2[0] - eu);
    let u1 = (c1[0] + eu).max(c2[0] + eu);
    let z0 = (c1[2] - ez).min(c2[2] - ez);
    let z1 = (c1[2] + ez).max(c2[2] + ez);
    Cross::Opening { rect: [u0, u1 - u0, z0, z1 - z0] }
}

/// All penetrations of `runs` through `chains`, and the runs that have a problem. Merging uses `merge_gap`.
/// The result depends only on its inputs, in a fixed order: by chain, segment, then position along the segment.
pub fn find(chains: &[WallChain], runs: &[Run], merge_gap: f64) -> (Vec<Penetration>, Vec<RunIssue>) {
    let mut issues = Vec::new();
    let mut found: Hits = chains.iter().map(|c| vec![Vec::new(); if c.validate().is_ok() { c.segment_count() } else { 0 }]).collect();
    let gap = if merge_gap.is_finite() && merge_gap > 0.0 { merge_gap } else { 0.0 };
    for (ri, r) in runs.iter().enumerate() {
        if !check_run(r) {
            issues.push(RunIssue { run: ri, problem: RunProblem::BadData });
            continue;
        }
        let mut problem = None;
        let mut hits: Vec<(usize, usize, [f64; 4])> = Vec::new();
        for (ci, c) in chains.iter().enumerate() {
            for si in 0..found[ci].len() {
                match cross_segment(r, c, si) {
                    Cross::None => {}
                    Cross::Problem(p) => problem = problem.or(Some(p)),
                    Cross::Opening { rect, .. } => hits.push((ci, si, rect)),
                }
            }
        }
        match problem {
            Some(p) => issues.push(RunIssue { run: ri, problem: p }),
            None => {
                for (ci, si, rect) in hits {
                    found[ci][si].push((ri, rect));
                }
            }
        }
    }
    let mut out = Vec::new();
    for (ci, segs) in found.iter().enumerate() {
        for (si, list) in segs.iter().enumerate() {
            // merge rectangles that touch, overlap or lie within `gap`, until nothing more merges
            let mut groups: Vec<(Vec<usize>, [f64; 4])> = list.iter().map(|(r, rect)| (vec![*r], [rect[0], rect[0] + rect[1], rect[2], rect[2] + rect[3]])).collect();
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
            let c = &chains[ci];
            let n = c.points.len();
            let (pa, pb) = (c.points[si], c.points[(si + 1) % n]);
            let len = ((pb[0] - pa[0]).powi(2) + (pb[1] - pa[1]).powi(2)).sqrt();
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
                let um = (b[0] + b[1]) / 2.0;
                let centre = [pa[0] + (pb[0] - pa[0]) * um / len, pa[1] + (pb[1] - pa[1]) * um / len, c.base_z + (b[2] + b[3]) / 2.0];
                out.push(Penetration {
                    id: PenetrationId { chain: ci, segment: si, seq },
                    runs: rs,
                    systems,
                    domains,
                    offset: b[0],
                    width: b[1] - b[0],
                    sill: b[2],
                    height: b[3] - b[2],
                    centre,
                    fire_rating_min: c.fire_rating_min,
                    cut: true,
                });
            }
        }
    }
    issues.sort_by_key(|i| i.run);
    (out, issues)
}

/// The register as text for a schedule: one row per penetration, comma separated, metres and minutes.
pub fn register_csv(pens: &[Penetration], runs: &[Run]) -> String {
    let mut s = String::from("id,wall,segment,offset_m,width_m,sill_m,height_m,x_m,y_m,z_m,fire_rating_min,fire_partition,cut,runs,systems\n");
    for p in pens {
        let labels: Vec<String> = p.runs.iter().map(|&r| {
            let t = runs[r].label.replace([',', '\n', '\r', '"'], " ");
            if t.starts_with(['=', '+', '-', '@']) { format!(" {t}") } else { t } // never read as a formula by a spreadsheet
        }).collect();
        let systems: Vec<String> = p.systems.iter().map(|x| format!("{x:?}")).collect();
        s.push_str(&format!(
            "{},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{},{},{},{},{}\n",
            p.id, p.id.chain, p.id.segment, p.offset, p.width, p.sill, p.height, p.centre[0], p.centre[1], p.centre[2],
            p.fire_rating_min.map(|m| m.to_string()).unwrap_or_default(), p.in_fire_partition(), p.cut,
            labels.join(" | "), systems.join(" | ")
        ));
    }
    s
}
