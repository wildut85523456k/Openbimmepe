//! Plan views. A plan view is what you see looking down on a level with a horizontal cut through the model:
//! walls and columns the cut passes through are *cut* (filled, heavy outline, with gaps where doors and windows
//! are at the cut height), things below the cut are seen from above, things above it up to the view top are
//! drawn as *overhead*, and things below the view bottom down to the view depth are *beyond*. Family plan
//! symbols (door swings, fixtures) are drawn where the family is placed.
//!
//! The result is plain geometry in metres (x, y; y up), classified so the interface chooses line weights and
//! colours; `svg` is one way to draw it. The same range rules as Revit's View Range are used; no values are
//! shipped, so the caller gives all four heights.
//!
//! The model is not changed, results do not depend on the thread count, and bad data gives an error naming the wall.
#![forbid(unsafe_code)]

use openbim_family::PlanShape;
use openbim_geom::{GeomError, SegmentPlan, WallChain};
use openbim_ifc::{FamilyItem, Scene};
use rayon::prelude::*;

type P = [f64; 2];
const EPS: f64 = 1e-9;

/// The four heights of a view, in metres *above the level* (negative = below it). They must satisfy
/// `depth <= bottom <= cut <= top`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewRange {
    pub top: f64,
    pub cut: f64,
    pub bottom: f64,
    pub depth: f64,
}

impl ViewRange {
    pub fn new(top: f64, cut: f64, bottom: f64, depth: f64) -> Result<ViewRange, ViewError> {
        if ![top, cut, bottom, depth].iter().all(|v| v.is_finite()) { return Err(ViewError::BadRange("a height is not a number")); }
        if !(depth <= bottom && bottom <= cut && cut <= top) { return Err(ViewError::BadRange("heights must satisfy depth <= bottom <= cut <= top")); }
        Ok(ViewRange { top, cut, bottom, depth })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewError {
    NoSuchLevel(usize),
    BadRange(&'static str),
    /// the wall element with this number cannot be drawn
    Wall { id: u64, err: GeomError },
}

impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewError::NoSuchLevel(i) => write!(f, "no level {i}"),
            ViewError::BadRange(why) => write!(f, "view range: {why}"),
            ViewError::Wall { id, err } => write!(f, "wall {id}: {err:?}"),
        }
    }
}
impl std::error::Error for ViewError {}

/// How an element meets the view range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// the cut plane passes through it
    Cut,
    /// entirely below the cut plane, above the view bottom
    Below,
    /// entirely above the cut plane, up to the view top
    Overhead,
    /// below the view bottom, down to the view depth
    Beyond,
    /// a family's plan symbol
    Symbol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Wall,
    Column,
    /// a placed family (door, window, fixture, equipment)
    Family,
}

/// The element a shape belongs to: its kind and its element number in the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Element {
    pub kind: Kind,
    pub id: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// a closed region to fill (the cut part of a wall or column)
    Fill(Vec<P>),
    Line(P, P),
    /// counter-clockwise from `a0` to `a1`, degrees; the sweep is `a1 - a0` taken into (0, 360]
    Arc { c: P, r: f64, a0: f64, a1: f64 },
    Circle { c: P, r: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub element: Element,
    pub class: Class,
    pub shape: Shape,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanView {
    pub level: usize,
    pub level_name: String,
    /// elevation of the level; the range heights are added to it
    pub elevation: f64,
    pub range: ViewRange,
    pub items: Vec<Item>,
}

impl PlanView {
    /// Smallest box around everything, `[min_x, min_y, max_x, max_y]`; None when the view is empty.
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut b: Option<[f64; 4]> = None;
        let mut add = |p: P| {
            b = Some(match b { None => [p[0], p[1], p[0], p[1]], Some(o) => [o[0].min(p[0]), o[1].min(p[1]), o[2].max(p[0]), o[3].max(p[1])] });
        };
        for it in &self.items {
            match &it.shape {
                Shape::Fill(poly) => poly.iter().for_each(|&p| add(p)),
                Shape::Line(a, c) => { add(*a); add(*c); }
                Shape::Arc { c, r, .. } | Shape::Circle { c, r } => { add([c[0] - r, c[1] - r]); add([c[0] + r, c[1] + r]); }
            }
        }
        b
    }
}

/// Build the plan view of level `level` of `scene`.
pub fn plan_view(scene: &Scene, level: usize, range: ViewRange) -> Result<PlanView, ViewError> {
    let lv = scene.levels.all().get(level).ok_or(ViewError::NoSuchLevel(level))?;
    let r = ViewRange::new(range.top, range.cut, range.bottom, range.depth)?;
    let abs = Abs { top: lv.elevation + r.top, cut: lv.elevation + r.cut, bottom: lv.elevation + r.bottom, depth: lv.elevation + r.depth };
    let walls: Vec<Result<Vec<Item>, ViewError>> = scene.walls.par_iter().map(|w| wall_items(w.id, &w.chain, &abs)).collect();
    let mut items = Vec::new();
    for w in walls { items.extend(w?); }
    let cols: Vec<Vec<Item>> = scene.columns.par_iter().map(|c| column_items(c.id, &c.footprint, c.base_z, c.base_z + c.height, &abs)).collect();
    items.extend(cols.into_iter().flatten());
    let fams: Vec<Vec<Item>> = scene.items.par_iter().map(|it| family_items(it, &abs)).collect();
    items.extend(fams.into_iter().flatten());
    Ok(PlanView { level, level_name: lv.name.clone(), elevation: lv.elevation, range: r, items })
}

/// The range in absolute heights.
struct Abs {
    top: f64,
    cut: f64,
    bottom: f64,
    depth: f64,
}

/// Where an element spanning `z0..z1` meets the range; None when it is not in view.
fn classify(z0: f64, z1: f64, r: &Abs) -> Option<Class> {
    if z0 <= r.cut && r.cut < z1 { return Some(Class::Cut); }
    if z1 <= r.cut { // below the cut
        if z1 > r.bottom { return Some(Class::Below); }
        if z1 > r.depth { return Some(Class::Beyond); }
        return None;
    }
    // above the cut
    if z0 <= r.top { Some(Class::Overhead) } else { None }
}

// ------------------------------------------------------------------ intervals along a wall segment

type Spans = Vec<(f64, f64)>;

/// `set` without `(a, b)`; `set` is sorted and disjoint.
fn subtract(set: Spans, a: f64, b: f64) -> Spans {
    let mut out = Vec::with_capacity(set.len() + 1);
    for (s, e) in set {
        if b <= s || a >= e { out.push((s, e)); continue; }
        if a > s { out.push((s, a)); }
        if b < e { out.push((b, e)); }
    }
    out
}

/// The parts of `0..len` where the top profile (s, z above base) is above `z`.
fn above_profile(profile: &[P], z: f64, len: f64) -> Spans {
    let mut out: Spans = Vec::new();
    let mut push = |a: f64, b: f64| {
        let (a, b) = (a.max(0.0), b.min(len));
        if b - a <= EPS { return; }
        match out.last_mut() { Some(l) if a <= l.1 + EPS => l.1 = l.1.max(b), _ => out.push((a, b)) }
    };
    for w in profile.windows(2) {
        let (p, q) = (w[0], w[1]);
        let (hp, hq) = (p[1] > z, q[1] > z);
        match (hp, hq) {
            (true, true) => push(p[0], q[0]),
            (false, false) => {}
            _ => {
                let t = (z - p[1]) / (q[1] - p[1]);
                let sx = p[0] + t * (q[0] - p[0]);
                if hp { push(p[0], sx) } else { push(sx, q[0]) }
            }
        }
    }
    out
}

/// The parts of the s axis inside a polygon given in (s, z), at height `z` (even-odd rule).
fn polygon_at(poly: &[P], z: f64) -> Spans {
    let mut xs: Vec<f64> = Vec::new();
    for k in 0..poly.len() {
        let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
        if (p[1] <= z) != (q[1] <= z) { xs.push(p[0] + (z - p[1]) / (q[1] - p[1]) * (q[0] - p[0])); }
    }
    xs.sort_by(|a, b| a.total_cmp(b));
    xs.chunks_exact(2).map(|c| (c[0], c[1])).collect()
}

// ------------------------------------------------------------------ walls

fn wall_items(id: u64, c: &WallChain, r: &Abs) -> Result<Vec<Item>, ViewError> {
    let top_profile = c.tops.iter().flatten().flat_map(|p| p.iter().map(|q| q[1])).fold(c.height, f64::max);
    let Some(class) = classify(c.base_z, c.base_z + top_profile, r) else { return Ok(Vec::new()) };
    let plans = c.plans().map_err(|err| ViewError::Wall { id, err })?;
    let n = plans.len();
    let el = Element { kind: Kind::Wall, id };
    let mut out = Vec::new();
    for (i, p) in plans.iter().enumerate() {
        let (u, left_s, right_s) = ([(p.l1[0] - p.l0[0]), (p.l1[1] - p.l0[1])], p.s_l0, p.s_r0);
        let ul = u[0].hypot(u[1]);
        let u = [u[0] / ul, u[1] / ul];
        let lat = |s: f64| [p.l0[0] + u[0] * (s - left_s), p.l0[1] + u[1] * (s - left_s)];
        let rat = |s: f64| [p.r0[0] + u[0] * (s - right_s), p.r0[1] + u[1] * (s - right_s)];
        if class == Class::Cut {
            let zrel = r.cut - c.base_z;
            let mut keep: Spans = vec![(0.0, p.length)];
            if let Some(Some(profile)) = c.tops.get(i) { keep = above_profile(profile, zrel, p.length); }
            for o in c.openings.get(i).into_iter().flatten() {
                if o.sill <= zrel && zrel < o.sill + o.height { keep = subtract(keep, o.offset, o.offset + o.width); }
            }
            for poly in c.poly_openings.get(i).into_iter().flatten() {
                for (a, b) in polygon_at(poly, zrel) { keep = subtract(keep, a, b); }
            }
            for (s0, s1) in keep {
                if s1 - s0 <= EPS { continue; }
                let at_start = s0 <= EPS;
                let at_end = s1 >= p.length - EPS;
                let (l0, r0) = if at_start { (p.l0, p.r0) } else { (lat(s0), rat(s0)) };
                let (l1, r1) = if at_end { (p.l1, p.r1) } else { (lat(s1), rat(s1)) };
                out.push(Item { element: el, class, shape: Shape::Fill(vec![l0, l1, r1, r0]) });
                // a mitre between two segments of the same chain is not drawn as a line
                let joint_start = at_start && (c.closed || i > 0);
                let joint_end = at_end && (c.closed || i + 1 < n);
                out.push(Item { element: el, class, shape: Shape::Line(l0, l1) });
                out.push(Item { element: el, class, shape: Shape::Line(r0, r1) });
                if !joint_start { out.push(Item { element: el, class, shape: Shape::Line(r0, l0) }); }
                if !joint_end { out.push(Item { element: el, class, shape: Shape::Line(l1, r1) }); }
            }
        } else {
            outline(&mut out, el, class, p, c.closed, i, n);
        }
    }
    Ok(out)
}

/// The edges of a whole segment quad, without the mitre lines between segments of one chain.
fn outline(out: &mut Vec<Item>, el: Element, class: Class, p: &SegmentPlan, closed: bool, i: usize, n: usize) {
    out.push(Item { element: el, class, shape: Shape::Line(p.l0, p.l1) });
    out.push(Item { element: el, class, shape: Shape::Line(p.r0, p.r1) });
    if closed || i > 0 { } else { out.push(Item { element: el, class, shape: Shape::Line(p.r0, p.l0) }); }
    if closed || i + 1 < n { } else { out.push(Item { element: el, class, shape: Shape::Line(p.l1, p.r1) }); }
}

// ------------------------------------------------------------------ columns

fn column_items(id: u64, footprint: &[P], z0: f64, z1: f64, r: &Abs) -> Vec<Item> {
    let Some(class) = classify(z0, z1, r) else { return Vec::new() };
    let el = Element { kind: Kind::Column, id };
    let mut out = Vec::new();
    if class == Class::Cut { out.push(Item { element: el, class, shape: Shape::Fill(footprint.to_vec()) }); }
    for k in 0..footprint.len() {
        out.push(Item { element: el, class, shape: Shape::Line(footprint[k], footprint[(k + 1) % footprint.len()]) });
    }
    out
}

// ------------------------------------------------------------------ family symbols

fn family_items(it: &FamilyItem, r: &Abs) -> Vec<Item> {
    let z = it.frame.origin[2];
    if it.instance.plan.is_empty() || z < r.bottom || z > r.top { return Vec::new(); }
    let (o, u) = (it.frame.origin, it.frame.x);
    let v = [-u[1], u[0]];
    let at = |p: P| [o[0] + u[0] * p[0] + v[0] * p[1], o[1] + u[1] * p[0] + v[1] * p[1]];
    let turn = u[1].atan2(u[0]).to_degrees();
    let el = Element { kind: Kind::Family, id: it.id };
    it.instance.plan.iter().map(|s| Item {
        element: el,
        class: Class::Symbol,
        shape: match *s {
            PlanShape::Line([x1, y1, x2, y2]) => Shape::Line(at([x1, y1]), at([x2, y2])),
            PlanShape::Circle { c, r } => Shape::Circle { c: at(c), r },
            PlanShape::Arc { c, r, a0, a1 } => Shape::Arc { c: at(c), r, a0: a0 + turn, a1: a1 + turn },
        },
    }).collect()
}

// ------------------------------------------------------------------ drawing

/// Draw a plan view as an SVG page, one unit = one metre, y up. Line weights are in metres on paper at 1:1, so a
/// caller who wants a scale applies it to the whole page. Cut parts are filled grey with a heavy outline, parts
/// below the cut are thin, overhead parts dashed, beyond parts light grey, family symbols thin.
pub fn svg(v: &PlanView) -> String {
    use std::fmt::Write;
    let [x0, y0, x1, y1] = v.bounds().unwrap_or([0.0, 0.0, 1.0, 1.0]);
    let m = 0.5;
    let mut s = String::new();
    let _ = write!(s, "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\">", x0 - m, -(y1 + m), x1 - x0 + 2.0 * m, y1 - y0 + 2.0 * m);
    let _ = write!(s, "<title>{}</title><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"white\"/><g transform=\"scale(1,-1)\" fill=\"none\" stroke=\"black\" stroke-linecap=\"round\" stroke-linejoin=\"round\">", esc(&v.level_name), x0 - m, -(y1 + m), x1 - x0 + 2.0 * m, y1 - y0 + 2.0 * m);
    let style = |c: Class| match c {
        Class::Cut => "stroke=\"black\" stroke-width=\"0.05\"",
        Class::Below => "stroke=\"black\" stroke-width=\"0.02\"",
        Class::Overhead => "stroke=\"black\" stroke-width=\"0.02\" stroke-dasharray=\"0.15 0.1\"",
        Class::Beyond => "stroke=\"#999\" stroke-width=\"0.015\"",
        Class::Symbol => "stroke=\"black\" stroke-width=\"0.02\"",
    };
    for it in &v.items {
        let st = style(it.class);
        match &it.shape {
            Shape::Fill(poly) => {
                let pts: Vec<String> = poly.iter().map(|p| format!("{},{}", p[0], p[1])).collect();
                let _ = write!(s, "<polygon points=\"{}\" fill=\"#cfcfcf\" stroke=\"none\"/>", pts.join(" "));
            }
            Shape::Line(a, b) => { let _ = write!(s, "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" {st}/>", a[0], a[1], b[0], b[1]); }
            Shape::Circle { c, r } => { let _ = write!(s, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" {st}/>", c[0], c[1], r); }
            Shape::Arc { c, r, a0, a1 } => {
                let sweep = (a1 - a0).rem_euclid(360.0);
                let sweep = if sweep == 0.0 { 360.0 } else { sweep };
                let (p, q) = (point_on(*c, *r, *a0), point_on(*c, *r, a0 + sweep));
                let _ = write!(s, "<path d=\"M{},{} A{r},{r} 0 {} 1 {},{}\" {st}/>", p[0], p[1], u8::from(sweep > 180.0), q[0], q[1]);
            }
        }
    }
    s.push_str("</g></svg>");
    s
}

fn point_on(c: P, r: f64, deg: f64) -> P {
    let (s, co) = deg.to_radians().sin_cos();
    [c[0] + r * co, c[1] + r * s]
}

fn esc(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
