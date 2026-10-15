//! S2g: forms with a profile — extrusions, revolves and sweeps.
//!
//! A form is described by a profile (a polygon or a circle, with optional holes) drawn in a work plane, and what
//! happens to it:
//! * extrusion: pushed along the plane's normal from one offset to another;
//! * revolve: turned about a line in the plane, through an angle up to a full turn;
//! * sweep: carried along a path of straight pieces in plan, with mitred corners.
//!
//! Profile coordinates (u, v) of a work plane with normal Z are (x, y); with normal X they are (y, z); with normal Y
//! they are (z, x). Every number is a formula. Lengths are metres, angles degrees.
//! The solved form gives exact volume and bounds; no mesh is made here.
use crate::expr::{Expr, Want, ANG, LEN, NUM};
use crate::{Axis, FamilyError, Xf};

pub const MAX_FORMS: usize = 200;
pub const MAX_POINTS: usize = 256;
pub const MAX_HOLES: usize = 16;
pub const MAX_PATH: usize = 64;
/// Most forms in one placement, nested families included.
pub const MAX_FORMS_TOTAL: usize = 5000;
pub const MAX_COORD: f64 = 100.0;
/// Smallest profile area (square metres).
pub const MIN_AREA: f64 = 1e-8;
/// Sharpest corner of a sweep path (degrees of turn).
pub const MAX_TURN: f64 = 150.0;

#[derive(Debug, Clone)]
pub enum ProfileDef {
    Polygon(Vec<(String, String)>),
    Circle { c: (String, String), r: String },
    /// a polygon held by locks and dimensions; every number is a formula, so family parameters drive it
    Sketch(SketchDef),
}

/// A constraint whose numbers are formulas.
#[derive(Debug, Clone)]
pub enum SketchCon {
    /// relations that carry no number (parallel, horizontal, ...)
    Rel(openbim_sketch::Constraint),
    Fixed { p: usize, x: String, y: String },
    Distance { a: usize, b: usize, d: String },
    DistanceX { a: usize, b: usize, d: String },
    DistanceY { a: usize, b: usize, d: String },
    Angle { l1: usize, l2: usize, degrees: String },
}

/// Points as first drawn (formulas), the first `loop_len` of which form the outline in order, then constraints.
/// Lines 0..loop_len are the outline edges (edge k runs from point k to point k+1); extra construction lines follow.
#[derive(Debug, Clone)]
pub struct SketchDef {
    pub points: Vec<(String, String)>,
    pub loop_len: usize,
    pub lines: Vec<(usize, usize)>,
    pub constraints: Vec<SketchCon>,
}

impl SketchDef {
    /// All the points are the outline.
    pub fn new(points: &[(&str, &str)]) -> Self {
        SketchDef { points: points.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), loop_len: points.len(), lines: vec![], constraints: vec![] }
    }
    /// Add a construction point that is not part of the outline.
    pub fn extra_point(mut self, x: &str, y: &str) -> Self { self.points.push((x.into(), y.into())); self }
    pub fn line(mut self, a: usize, b: usize) -> Self { self.lines.push((a, b)); self }
    pub fn rel(mut self, c: openbim_sketch::Constraint) -> Self { self.constraints.push(SketchCon::Rel(c)); self }
    pub fn fixed(mut self, p: usize, x: &str, y: &str) -> Self { self.constraints.push(SketchCon::Fixed { p, x: x.into(), y: y.into() }); self }
    pub fn distance(mut self, a: usize, b: usize, d: &str) -> Self { self.constraints.push(SketchCon::Distance { a, b, d: d.into() }); self }
    pub fn distance_x(mut self, a: usize, b: usize, d: &str) -> Self { self.constraints.push(SketchCon::DistanceX { a, b, d: d.into() }); self }
    pub fn distance_y(mut self, a: usize, b: usize, d: &str) -> Self { self.constraints.push(SketchCon::DistanceY { a, b, d: d.into() }); self }
    pub fn angle(mut self, l1: usize, l2: usize, degrees: &str) -> Self { self.constraints.push(SketchCon::Angle { l1, l2, degrees: degrees.into() }); self }
}

impl ProfileDef {
    pub fn polygon(pts: &[(&str, &str)]) -> Self { ProfileDef::Polygon(pts.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()) }
    pub fn rect(u0: &str, v0: &str, u1: &str, v1: &str) -> Self { Self::polygon(&[(u0, v0), (u1, v0), (u1, v1), (u0, v1)]) }
    pub fn circle(cu: &str, cv: &str, r: &str) -> Self { ProfileDef::Circle { c: (cu.into(), cv.into()), r: r.into() } }
    pub fn sketch(s: SketchDef) -> Self { ProfileDef::Sketch(s) }
}

#[derive(Debug, Clone)]
pub enum FormKind {
    /// the profile is pushed along the plane's normal between the coordinates `from` and `to`
    Extrusion { plane: Axis, from: String, to: String },
    /// the axis is a line in the plane: parallel to v at u = `at` (`about_v`), or parallel to u at v = `at`
    Revolve { plane: Axis, offset: String, about_v: bool, at: String, angle: String },
    /// the path is in plan (x, y) at height `z`; the profile is drawn in the plane across the path: u to the left of
    /// the direction of travel, v up, with the path point as origin
    Sweep { z: String, path: Vec<(String, String)> },
}

#[derive(Debug, Clone)]
pub struct FormDef {
    pub name: String,
    pub kind: FormKind,
    pub outer: ProfileDef,
    pub holes: Vec<ProfileDef>,
    pub material: Option<String>,
    pub visible: Option<String>,
}

impl FormDef {
    pub fn extrusion(name: &str, plane: Axis, from: &str, to: &str, outer: ProfileDef) -> Self {
        FormDef { name: name.into(), kind: FormKind::Extrusion { plane, from: from.into(), to: to.into() }, outer, holes: vec![], material: None, visible: None }
    }
    pub fn revolve(name: &str, plane: Axis, offset: &str, about_v: bool, at: &str, angle: &str, outer: ProfileDef) -> Self {
        FormDef { name: name.into(), kind: FormKind::Revolve { plane, offset: offset.into(), about_v, at: at.into(), angle: angle.into() }, outer, holes: vec![], material: None, visible: None }
    }
    pub fn sweep(name: &str, z: &str, path: &[(&str, &str)], outer: ProfileDef) -> Self {
        FormDef { name: name.into(), kind: FormKind::Sweep { z: z.into(), path: path.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect() }, outer, holes: vec![], material: None, visible: None }
    }
    pub fn hole(mut self, p: ProfileDef) -> Self { self.holes.push(p); self }
    pub fn material(mut self, value_name: &str) -> Self { self.material = Some(value_name.into()); self }
    pub fn visible_if(mut self, f: &str) -> Self { self.visible = Some(f.into()); self }
}

// ---------- compiled ----------

pub(crate) type Parse<'a> = &'a dyn Fn(&str, &str, bool, Want) -> Result<Expr, FamilyError>;

pub(crate) enum CCon {
    Rel(openbim_sketch::Constraint),
    Fixed(usize, Expr, Expr),
    Distance(usize, usize, Expr),
    DistanceX(usize, usize, Expr),
    DistanceY(usize, usize, Expr),
    Angle(usize, usize, Expr),
}
pub(crate) struct CSketch {
    points: Vec<[Expr; 2]>,
    loop_len: usize,
    lines: Vec<(usize, usize)>,
    cons: Vec<CCon>,
}
pub(crate) enum CLoop {
    Poly(Vec<[Expr; 2]>),
    Circle([Expr; 2], Expr),
    Sketch(CSketch),
}
pub(crate) enum CKind {
    Extrusion { plane: Axis, from: Expr, to: Expr },
    Revolve { plane: Axis, offset: Expr, about_v: bool, at: Expr, angle: Expr },
    Sweep { z: Expr, path: Vec<[Expr; 2]> },
}
pub(crate) struct CForm {
    pub name: String,
    kind: CKind,
    outer: CLoop,
    holes: Vec<CLoop>,
    pub material: Option<usize>,
    visible: Option<Expr>,
}

fn bad(name: &str, why: &'static str) -> FamilyError { FamilyError::Form { name: name.into(), why } }

fn compile_loop(name: &str, what: &str, p: &ProfileDef, parse: Parse) -> Result<CLoop, FamilyError> {
    let pe = |part: String, s: &str| parse(&format!("form {name}.{what}.{part}"), s, false, LEN);
    let pa = |part: String, s: &str| parse(&format!("form {name}.{what}.{part}"), s, false, ANG);
    match p {
        ProfileDef::Polygon(pts) => {
            if pts.len() < 3 { return Err(bad(name, "a polygon profile needs at least 3 points")); }
            if pts.len() > MAX_POINTS { return Err(FamilyError::TooMany("points in a profile")); }
            let mut out = Vec::with_capacity(pts.len());
            for (i, (u, v)) in pts.iter().enumerate() { out.push([pe(format!("u{i}"), u)?, pe(format!("v{i}"), v)?]); }
            Ok(CLoop::Poly(out))
        }
        ProfileDef::Sketch(sk) => {
            if sk.loop_len < 3 || sk.loop_len > sk.points.len() { return Err(bad(name, "a sketch outline needs at least 3 of its points")); }
            if sk.points.len() > openbim_sketch::MAX_POINTS { return Err(FamilyError::TooMany("points in a profile")); }
            if sk.constraints.len() > openbim_sketch::MAX_CONSTRAINTS { return Err(FamilyError::TooMany("sketch constraints")); }
            if sk.loop_len + sk.lines.len() > openbim_sketch::MAX_LINES { return Err(FamilyError::TooMany("lines in a sketch")); }
            let mut points = Vec::with_capacity(sk.points.len());
            for (i, (u, v)) in sk.points.iter().enumerate() { points.push([pe(format!("u{i}"), u)?, pe(format!("v{i}"), v)?]); }
            let mut cons = Vec::with_capacity(sk.constraints.len());
            for (k, c) in sk.constraints.iter().enumerate() {
                let e = |part: &str, s: &str| pe(format!("c{k}.{part}"), s);
                let ea = |part: &str, s: &str| pa(format!("c{k}.{part}"), s);
                cons.push(match c {
                    SketchCon::Rel(r) => CCon::Rel(*r),
                    SketchCon::Fixed { p, x, y } => CCon::Fixed(*p, e("x", x)?, e("y", y)?),
                    SketchCon::Distance { a, b, d } => CCon::Distance(*a, *b, e("d", d)?),
                    SketchCon::DistanceX { a, b, d } => CCon::DistanceX(*a, *b, e("d", d)?),
                    SketchCon::DistanceY { a, b, d } => CCon::DistanceY(*a, *b, e("d", d)?),
                    SketchCon::Angle { l1, l2, degrees } => CCon::Angle(*l1, *l2, ea("degrees", degrees)?),
                });
            }
            Ok(CLoop::Sketch(CSketch { points, loop_len: sk.loop_len, lines: sk.lines.clone(), cons }))
        }
        ProfileDef::Circle { c, r } => Ok(CLoop::Circle([pe("cu".into(), &c.0)?, pe("cv".into(), &c.1)?], pe("r".into(), r)?)),
    }
}

pub(crate) fn compile(forms: &[FormDef], parse: Parse, material_slot: &dyn Fn(&str, &str) -> Result<usize, FamilyError>) -> Result<Vec<CForm>, FamilyError> {
    if forms.len() > MAX_FORMS { return Err(FamilyError::TooMany("forms")); }
    let mut out: Vec<CForm> = Vec::new();
    for f in forms {
        if !crate::valid_name(&f.name) { return Err(FamilyError::BadName(f.name.clone())); }
        if out.iter().any(|o| o.name == f.name) { return Err(FamilyError::DuplicateName(f.name.clone())); }
        let pe = |part: &str, s: &str| parse(&format!("form {}.{}", f.name, part), s, false, LEN);
        let pa = |part: &str, s: &str| parse(&format!("form {}.{}", f.name, part), s, false, ANG);
        let pn = |part: &str, s: &str| parse(&format!("form {}.{}", f.name, part), s, false, NUM);
        let kind = match &f.kind {
            FormKind::Extrusion { plane, from, to } => CKind::Extrusion { plane: *plane, from: pe("from", from)?, to: pe("to", to)? },
            FormKind::Revolve { plane, offset, about_v, at, angle } => CKind::Revolve { plane: *plane, offset: pe("offset", offset)?, about_v: *about_v, at: pe("at", at)?, angle: pa("angle", angle)? },
            FormKind::Sweep { z, path } => {
                if path.len() < 2 { return Err(bad(&f.name, "a sweep path needs at least 2 points")); }
                if path.len() > MAX_PATH { return Err(FamilyError::TooMany("points in a sweep path")); }
                let mut p = Vec::new();
                for (i, (x, y)) in path.iter().enumerate() { p.push([pe(&format!("path.x{i}"), x)?, pe(&format!("path.y{i}"), y)?]); }
                CKind::Sweep { z: pe("z", z)?, path: p }
            }
        };
        if f.holes.len() > MAX_HOLES { return Err(FamilyError::TooMany("holes in a profile")); }
        let outer = compile_loop(&f.name, "outer", &f.outer, parse)?;
        let mut holes = Vec::new();
        for (i, h) in f.holes.iter().enumerate() { holes.push(compile_loop(&f.name, &format!("hole{i}"), h, parse)?); }
        let material = match &f.material { Some(m) => Some(material_slot(&f.name, m)?), None => None };
        let visible = match &f.visible { Some(v) => Some(pn("visible", v)?), None => None };
        out.push(CForm { name: f.name.clone(), kind, outer, holes, material, visible });
    }
    Ok(out)
}

// ---------- solved ----------

#[derive(Debug, Clone, PartialEq)]
pub enum Loop {
    /// counter-clockwise, no repeated first point
    Poly(Vec<[f64; 2]>),
    Circle { c: [f64; 2], r: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FormGeom {
    Extrusion { plane: Axis, from: f64, to: f64 },
    Revolve { plane: Axis, offset: f64, about_v: bool, at: f64, angle: f64 },
    Sweep { z: f64, path: Vec<[f64; 2]> },
}

/// A solved form. `xf` and `dz` move it into the family's frame when it came from a nested family.
#[derive(Debug, Clone, PartialEq)]
pub struct Form {
    /// index into the family's form names (`Inner::form_name`)
    pub index: usize,
    pub geom: FormGeom,
    pub outer: Loop,
    pub holes: Vec<Loop>,
    pub xf: Xf,
    pub dz: f64,
}

type P2 = [f64; 2];

fn sub(a: P2, b: P2) -> P2 { [a[0] - b[0], a[1] - b[1]] }
fn cross(a: P2, b: P2) -> f64 { a[0] * b[1] - a[1] * b[0] }
fn dot(a: P2, b: P2) -> f64 { a[0] * b[0] + a[1] * b[1] }

fn signed_area(p: &[P2]) -> f64 {
    let n = p.len();
    (0..n).map(|i| cross(p[i], p[(i + 1) % n])).sum::<f64>() / 2.0
}

/// area and first moments (integral of u dA, integral of v dA) of a loop
fn moments(l: &Loop) -> (f64, f64, f64) {
    match l {
        Loop::Circle { c, r } => {
            let a = std::f64::consts::PI * r * r;
            (a, a * c[0], a * c[1])
        }
        Loop::Poly(p) => {
            let n = p.len();
            let (mut a, mut mu, mut mv) = (0.0, 0.0, 0.0);
            for i in 0..n {
                let (p0, p1) = (p[i], p[(i + 1) % n]);
                let w = cross(p0, p1);
                a += w;
                mu += (p0[0] + p1[0]) * w;
                mv += (p0[1] + p1[1]) * w;
            }
            (a / 2.0, mu / 6.0, mv / 6.0)
        }
    }
}

/// Net area and first moments of outer minus holes.
pub fn net_moments(outer: &Loop, holes: &[Loop]) -> (f64, f64, f64) {
    let (mut a, mut mu, mut mv) = moments(outer);
    for h in holes { let m = moments(h); a -= m.0; mu -= m.1; mv -= m.2; }
    (a, mu, mv)
}

// segment tests
fn seg_dist_pt(a: P2, b: P2, p: P2) -> f64 {
    let ab = sub(b, a);
    let l2 = dot(ab, ab);
    let t = if l2 == 0.0 { 0.0 } else { (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0) };
    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    let d = sub(p, q);
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}
fn orient(a: P2, b: P2, c: P2) -> f64 { cross(sub(b, a), sub(c, a)) }
/// segments share at least a point
fn segs_touch(a: P2, b: P2, c: P2, d: P2) -> bool {
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    if ((o1 > 0.0 && o2 < 0.0) || (o1 < 0.0 && o2 > 0.0)) && ((o3 > 0.0 && o4 < 0.0) || (o3 < 0.0 && o4 > 0.0)) { return true; }
    let on = |p: P2, q: P2, r: P2| orient(p, q, r) == 0.0 && r[0] >= p[0].min(q[0]) && r[0] <= p[0].max(q[0]) && r[1] >= p[1].min(q[1]) && r[1] <= p[1].max(q[1]);
    on(a, b, c) || on(a, b, d) || on(c, d, a) || on(c, d, b)
}
fn point_in_poly(p: &[P2], q: P2) -> bool {
    let n = p.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]) + a[0] { inside = !inside; }
    }
    inside
}
fn poly_simple(p: &[P2]) -> bool {
    let n = p.len();
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        for j in i + 1..n {
            let (c, d) = (p[j], p[(j + 1) % n]);
            let adjacent = j == i + 1 || (i == 0 && j == n - 1);
            if adjacent { continue; }
            if segs_touch(a, b, c, d) { return false; }
        }
    }
    true
}

fn loops_disjoint(a: &Loop, b: &Loop) -> bool {
    match (a, b) {
        (Loop::Circle { c: c1, r: r1 }, Loop::Circle { c: c2, r: r2 }) => { let d = sub(*c1, *c2); (d[0] * d[0] + d[1] * d[1]).sqrt() > r1 + r2 }
        (Loop::Circle { c, r }, Loop::Poly(p)) | (Loop::Poly(p), Loop::Circle { c, r }) => {
            let n = p.len();
            (0..n).all(|i| seg_dist_pt(p[i], p[(i + 1) % n], *c) > *r) && !point_in_poly(p, *c)
        }
        (Loop::Poly(p), Loop::Poly(q)) => {
            let (n, m) = (p.len(), q.len());
            for i in 0..n { for j in 0..m { if segs_touch(p[i], p[(i + 1) % n], q[j], q[(j + 1) % m]) { return false; } } }
            !point_in_poly(p, q[0]) && !point_in_poly(q, p[0])
        }
    }
}
/// `inner` lies strictly inside `outer`
fn loop_inside(inner: &Loop, outer: &Loop) -> bool {
    match (inner, outer) {
        (Loop::Circle { c: c1, r: r1 }, Loop::Circle { c: c2, r: r2 }) => { let d = sub(*c1, *c2); (d[0] * d[0] + d[1] * d[1]).sqrt() + r1 < *r2 }
        (Loop::Poly(p), Loop::Circle { c, r }) => p.iter().all(|q| { let d = sub(*q, *c); (d[0] * d[0] + d[1] * d[1]).sqrt() < *r }),
        (Loop::Circle { c, r }, Loop::Poly(q)) => { let n = q.len(); point_in_poly(q, *c) && (0..n).all(|i| seg_dist_pt(q[i], q[(i + 1) % n], *c) > *r) }
        (Loop::Poly(p), Loop::Poly(q)) => {
            let (n, m) = (p.len(), q.len());
            for i in 0..n { for j in 0..m { if segs_touch(p[i], p[(i + 1) % n], q[j], q[(j + 1) % m]) { return false; } } }
            point_in_poly(q, p[0])
        }
    }
}

fn solve_loop(name: &str, c: &CLoop, vals: &[f64]) -> Result<Loop, FamilyError> {
    let ev = |e: &Expr, part: &str| -> Result<f64, FamilyError> {
        let v = e.eval(vals).map_err(|err| FamilyError::Formula { name: format!("form {name}.{part}"), err })?;
        if v.abs() > MAX_COORD { return Err(bad(name, "a coordinate is not a finite number within 100 m")); }
        Ok(v)
    };
    let raw = |e: &Expr, part: &str| -> Result<f64, FamilyError> { e.eval(vals).map_err(|err| FamilyError::Formula { name: format!("form {name}.{part}"), err }) };
    match c {
        CLoop::Circle(ce, r) => {
            let (cu, cv, rr) = (ev(&ce[0], "cu")?, ev(&ce[1], "cv")?, ev(r, "r")?);
            if rr < 0.0 || std::f64::consts::PI * rr * rr < MIN_AREA { return Err(bad(name, "the circle is too small")); }
            Ok(Loop::Circle { c: [cu, cv], r: rr })
        }
        CLoop::Poly(_) | CLoop::Sketch(_) => {
            let mut p: Vec<P2> = Vec::new();
            match c {
                CLoop::Poly(pts) => { for q in pts { p.push([ev(&q[0], "u")?, ev(&q[1], "v")?]); } }
                CLoop::Sketch(cs) => p = solve_sketch(name, cs, &ev, &raw)?,
                CLoop::Circle(..) => {}
            }
            if !poly_simple(&p) { return Err(bad(name, "the profile crosses or touches itself")); }
            let a = signed_area(&p);
            if a.abs() < MIN_AREA { return Err(bad(name, "the profile has no area")); }
            if a < 0.0 { p.reverse(); }
            Ok(Loop::Poly(p))
        }
    }
}

/// Evaluate the sketch's formulas, let the solver settle it, and return its outline.
fn solve_sketch(name: &str, cs: &CSketch, ev: &dyn Fn(&Expr, &str) -> Result<f64, FamilyError>, raw: &dyn Fn(&Expr, &str) -> Result<f64, FamilyError>) -> Result<Vec<P2>, FamilyError> {
    use openbim_sketch::{Constraint, Sketch, SketchError, Status};
    let mut sk = Sketch::default();
    for q in &cs.points { sk.point(ev(&q[0], "u")?, ev(&q[1], "v")?); }
    for k in 0..cs.loop_len { sk.line(k, (k + 1) % cs.loop_len); }
    for &(a, b) in &cs.lines { sk.line(a, b); }
    for c in &cs.cons {
        sk.constrain(match c {
            CCon::Rel(r) => *r,
            CCon::Fixed(p, x, y) => Constraint::Fixed { p: *p, x: ev(x, "x")?, y: ev(y, "y")? },
            CCon::Distance(a, b, d) => Constraint::Distance { a: *a, b: *b, d: ev(d, "d")? },
            CCon::DistanceX(a, b, d) => Constraint::DistanceX { a: *a, b: *b, d: ev(d, "d")? },
            CCon::DistanceY(a, b, d) => Constraint::DistanceY { a: *a, b: *b, d: ev(d, "d")? },
            CCon::Angle(l1, l2, d) => Constraint::Angle { l1: *l1, l2: *l2, degrees: raw(d, "degrees")? },
        });
    }
    let sol = sk.solve().map_err(|e| bad(name, match e {
        SketchError::TooMany(_) => "the sketch has too many parts",
        SketchError::BadIndex { .. } | SketchError::DegenerateLine(_) => "a sketch constraint or line refers to a point, line or circle that is not there",
        SketchError::BadValue { .. } | SketchError::NotFinite(_) => "a sketch dimension is not a usable number",
    }))?;
    if !matches!(sol.status, Status::Solved) { return Err(bad(name, "the sketch constraints conflict with each other")); }
    let out: Vec<P2> = sol.points[..cs.loop_len].to_vec();
    if out.iter().any(|q| q[0].abs() > MAX_COORD || q[1].abs() > MAX_COORD) { return Err(bad(name, "the sketch ends outside 100 m")); }
    Ok(out)
}

fn solve_profile(name: &str, outer: &CLoop, holes: &[CLoop], vals: &[f64]) -> Result<(Loop, Vec<Loop>), FamilyError> {
    let o = solve_loop(name, outer, vals)?;
    let mut hs: Vec<Loop> = Vec::new();
    for h in holes {
        let hl = solve_loop(name, h, vals)?;
        if !loop_inside(&hl, &o) { return Err(bad(name, "a hole must lie inside the outer profile without touching it")); }
        if hs.iter().any(|x| !loops_disjoint(x, &hl)) { return Err(bad(name, "holes must not touch or overlap each other")); }
        hs.push(hl);
    }
    Ok((o, hs))
}

fn poly_pts(l: &Loop) -> Vec<P2> {
    match l { Loop::Poly(p) => p.clone(), Loop::Circle { c, r } => vec![[c[0] - r, c[1] - r], [c[0] + r, c[1] + r]] }
}

/// every point of the loop satisfies `f`
fn loop_all(l: &Loop, f: &dyn Fn(P2) -> bool) -> bool { poly_pts(l).into_iter().all(f) }

fn path_turns(path: &[P2]) -> Vec<f64> {
    (1..path.len() - 1).map(|i| { let (a, b) = (sub(path[i], path[i - 1]), sub(path[i + 1], path[i])); cross(a, b).atan2(dot(a, b)) }).collect()
}

pub(crate) fn solve_forms(forms: &[CForm], vals: &[f64]) -> Result<Vec<Form>, FamilyError> {
    let mut out = Vec::new();
    for (index, f) in forms.iter().enumerate() {
        if let Some(v) = &f.visible {
            let x = v.eval(vals).map_err(|err| FamilyError::Formula { name: format!("form {}.visible", f.name), err })?;
            if x == 0.0 { continue; }
        }
        let name = f.name.as_str();
        let ev = |e: &Expr, part: &str| -> Result<f64, FamilyError> {
            let v = e.eval(vals).map_err(|err| FamilyError::Formula { name: format!("form {name}.{part}"), err })?;
            if v.abs() > MAX_COORD { return Err(bad(name, "a value is not a finite number within 100 m")); }
            Ok(v)
        };
        let (outer, holes) = solve_profile(name, &f.outer, &f.holes, vals)?;
        let geom = match &f.kind {
            CKind::Extrusion { plane, from, to } => {
                let (from, to) = (ev(from, "from")?, ev(to, "to")?);
                if to - from <= crate::MIN_SIZE { return Err(bad(name, "the extrusion needs a positive length")); }
                FormGeom::Extrusion { plane: *plane, from, to }
            }
            CKind::Revolve { plane, offset, about_v, at, angle } => {
                let (offset, at) = (ev(offset, "offset")?, ev(at, "at")?);
                let angle = angle.eval(vals).map_err(|err| FamilyError::Formula { name: format!("form {name}.angle"), err })?;
                if !(angle > 0.0 && angle <= 360.0) { return Err(bad(name, "the angle must be above 0 and at most 360 degrees")); }
                let k = if *about_v { 0 } else { 1 };
                let left = loop_all(&outer, &|p| p[k] >= at);
                let right = loop_all(&outer, &|p| p[k] <= at);
                if !(left || right) { return Err(bad(name, "the profile crosses the axis of revolution")); }
                FormGeom::Revolve { plane: *plane, offset, about_v: *about_v, at, angle }
            }
            CKind::Sweep { z, path } => {
                let z = ev(z, "z")?;
                let mut p = Vec::with_capacity(path.len());
                for q in path { p.push([ev(&q[0], "path.x")?, ev(&q[1], "path.y")?]); }
                let len: Vec<f64> = (1..p.len()).map(|i| { let d = sub(p[i], p[i - 1]); (d[0] * d[0] + d[1] * d[1]).sqrt() }).collect();
                if len.iter().any(|l| *l < crate::MIN_SIZE) { return Err(bad(name, "two path points coincide")); }
                let turns = path_turns(&p);
                if turns.iter().any(|t| t.abs().to_degrees() > MAX_TURN) { return Err(bad(name, "a path corner turns more than 150 degrees")); }
                // the mitre cuts at both ends of a piece must not pass each other
                let umax = poly_pts(&outer).iter().map(|q| q[0].abs()).fold(0.0, f64::max);
                let lead = |i: usize| if i == 0 || i == p.len() - 1 { 0.0 } else { umax * (turns[i - 1].abs() / 2.0).tan() };
                for (i, li) in len.iter().enumerate() { if lead(i) + lead(i + 1) >= *li { return Err(bad(name, "the profile is too wide for a corner of the path")); } }
                // the path may not cross itself
                for i in 0..len.len() { for j in i + 2..len.len() { if segs_touch(p[i], p[i + 1], p[j], p[j + 1]) { return Err(bad(name, "the path crosses itself")); } } }
                FormGeom::Sweep { z, path: p }
            }
        };
        out.push(Form { index, geom, outer, holes, xf: Xf::IDENTITY, dz: 0.0 });
    }
    Ok(out)
}

// ---------- measures ----------

fn map3(plane: Axis, offset: f64, u: f64, v: f64) -> [f64; 3] {
    match plane { Axis::Z => [u, v, offset], Axis::X => [offset, u, v], Axis::Y => [v, offset, u] }
}

impl Form {
    /// Net profile area (outer minus holes).
    pub fn profile_area(&self) -> f64 { net_moments(&self.outer, &self.holes).0 }

    /// Exact volume in cubic metres.
    pub fn volume(&self) -> f64 {
        let (a, mu, mv) = net_moments(&self.outer, &self.holes);
        match &self.geom {
            FormGeom::Extrusion { from, to, .. } => a * (to - from),
            FormGeom::Revolve { about_v, at, angle, .. } => {
                // Pappus: turning angle times the first moment about the axis
                let m = if *about_v { mu - at * a } else { mv - at * a };
                angle.to_radians() * m.abs()
            }
            FormGeom::Sweep { path, .. } => {
                let total: f64 = (1..path.len()).map(|i| { let d = sub(path[i], path[i - 1]); (d[0] * d[0] + d[1] * d[1]).sqrt() }).sum();
                let t: f64 = path_turns(path).iter().map(|th| (th / 2.0).tan()).sum();
                a * total - 2.0 * mu * t
            }
        }
    }

    /// Exact bounds in the form's own frame (before `xf` and `dz`): (min, max).
    pub fn bounds_local(&self) -> ([f64; 3], [f64; 3]) {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        let mut add = |p: [f64; 3]| { for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } };
        let (ul, vl) = match &self.outer {
            Loop::Circle { c, r } => ((c[0] - r, c[0] + r), (c[1] - r, c[1] + r)),
            Loop::Poly(p) => (
                (p.iter().map(|q| q[0]).fold(f64::INFINITY, f64::min), p.iter().map(|q| q[0]).fold(f64::NEG_INFINITY, f64::max)),
                (p.iter().map(|q| q[1]).fold(f64::INFINITY, f64::min), p.iter().map(|q| q[1]).fold(f64::NEG_INFINITY, f64::max)),
            ),
        };
        match &self.geom {
            FormGeom::Extrusion { plane, from, to } => {
                for w in [*from, *to] { for u in [ul.0, ul.1] { for v in [vl.0, vl.1] { add(map3(*plane, w, u, v)); } } }
            }
            FormGeom::Revolve { plane, offset, about_v, at, angle } => {
                // every corner of the profile box, turned to the end angles and to each quarter turn in between
                let mut angles = vec![0.0, *angle];
                let mut q = 90.0;
                while q < *angle { angles.push(q); q += 90.0; }
                let to_p = |u: f64, v: f64| map3(*plane, *offset, u, v);
                let (axis_pt, axis_dir) = if *about_v { (to_p(*at, 0.0), unit_of(*plane, false)) } else { (to_p(0.0, *at), unit_of(*plane, true)) };
                // Each coordinate of a turned point is plus or minus its radial distance times a cosine or sine of the
                // angle, or plus or minus its position along the axis. The turn angle only matters at 0, at the end and
                // at quarter turns, and the profile only matters at its corners (polygon) or at the axis-parallel
                // extremes (circle), so those are the only points to try.
                let mut pts: Vec<[f64; 3]> = Vec::new();
                match &self.outer {
                    Loop::Poly(p) => for q in p { pts.push(to_p(q[0], q[1])); },
                    Loop::Circle { c, r } => {
                        // the extreme radial and axial values of a circle are on its axis-parallel extremes
                        for (du, dv) in [(*r, 0.0), (-*r, 0.0), (0.0, *r), (0.0, -*r)] { pts.push(to_p(c[0] + du, c[1] + dv)); }
                    }
                }
                for p in pts {
                    for a in &angles { add(rotate_about(p, axis_pt, axis_dir, a.to_radians())); }
                }
            }
            FormGeom::Sweep { z, path } => {
                let n = path.len();
                let dirs: Vec<P2> = (1..n).map(|i| { let d = sub(path[i], path[i - 1]); let l = (d[0] * d[0] + d[1] * d[1]).sqrt(); [d[0] / l, d[1] / l] }).collect();
                for i in 0..n {
                    let m = if i == 0 { left(dirs[0]) } else if i == n - 1 { left(dirs[n - 2]) } else {
                        let (na, nb) = (left(dirs[i - 1]), left(dirs[i]));
                        let k = 1.0 + dot(na, nb);
                        [(na[0] + nb[0]) / k, (na[1] + nb[1]) / k]
                    };
                    for u in [ul.0, ul.1] { for v in [vl.0, vl.1] { add([path[i][0] + u * m[0], path[i][1] + u * m[1], z + v]); } }
                }
            }
        }
        (lo, hi)
    }
}

fn left(d: P2) -> P2 { [-d[1], d[0]] }

/// 3D unit vector of the profile's v axis (`u_dir` false) or u axis (`u_dir` true) in a work plane
fn unit_of(plane: Axis, u_dir: bool) -> [f64; 3] {
    let (u, v) = match plane { Axis::Z => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), Axis::X => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), Axis::Y => ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]) };
    if u_dir { u } else { v }
}

fn rotate_about(p: [f64; 3], o: [f64; 3], a: [f64; 3], t: f64) -> [f64; 3] {
    let v = [p[0] - o[0], p[1] - o[1], p[2] - o[2]];
    let (s, c) = (t.sin(), t.cos());
    let cr = [a[1] * v[2] - a[2] * v[1], a[2] * v[0] - a[0] * v[2], a[0] * v[1] - a[1] * v[0]];
    let d = a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
    // exact values at quarter turns, so bounds are not blurred by rounding
    let (s, c) = (snap(s), snap(c));
    [o[0] + v[0] * c + cr[0] * s + a[0] * d * (1.0 - c), o[1] + v[1] * c + cr[1] * s + a[1] * d * (1.0 - c), o[2] + v[2] * c + cr[2] * s + a[2] * d * (1.0 - c)]
}
fn snap(x: f64) -> f64 { if x.abs() < 1e-15 { 0.0 } else if (x - 1.0).abs() < 1e-15 { 1.0 } else if (x + 1.0).abs() < 1e-15 { -1.0 } else { x } }

impl Form {
    /// Bounds in the family's frame (after `xf` and `dz`): (min, max, exact). The bounds are exact when the form is
    /// not turned or is turned by a multiple of 90 degrees; otherwise they are the box around the turned local box.
    pub fn bounds(&self) -> ([f64; 3], [f64; 3], bool) {
        let (lo, hi) = self.bounds_local();
        let mut bl = [f64::INFINITY; 2];
        let mut bh = [f64::NEG_INFINITY; 2];
        for x in [lo[0], hi[0]] {
            for y in [lo[1], hi[1]] {
                let p = self.xf.apply([x, y]);
                for k in 0..2 { bl[k] = bl[k].min(p[k]); bh[k] = bh[k].max(p[k]); }
            }
        }
        let exact = self.xf.angle.rem_euclid(90.0) == 0.0;
        ([bl[0], bl[1], lo[2] + self.dz], [bh[0], bh[1], hi[2] + self.dz], exact)
    }
}
