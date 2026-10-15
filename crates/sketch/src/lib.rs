//! Sketch-and-lock: a small 2D geometric constraint solver (S2h).
//!
//! A sketch has points, lines (between two points) and circles (a centre point and a radius). Constraints hold them
//! in place: locks (fixed point), relations (horizontal, parallel, tangent, ...) and dimensions (distance, angle,
//! radius). The solver moves the points the least it can from where they were drawn until every constraint holds
//! (Levenberg-Marquardt on exact derivatives), then reports how many degrees of freedom are left, how many
//! constraints repeat another, and which points are fully fixed.

#![forbid(unsafe_code)]

use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

pub const MAX_POINTS: usize = 200;
pub const MAX_LINES: usize = 400;
pub const MAX_CIRCLES: usize = 50;
pub const MAX_CONSTRAINTS: usize = 800;
pub const MAX_VALUE: f64 = 1.0e6;
/// every constraint is met when each residual is within this
pub const TOLERANCE: f64 = 1.0e-10;
const MAX_ITER: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    /// index of the centre point
    pub centre: usize,
    pub radius: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Constraint {
    /// lock a point at a place
    Fixed { p: usize, x: f64, y: f64 },
    Coincident { a: usize, b: usize },
    /// two points at the same height / the same x
    Horizontal { a: usize, b: usize },
    Vertical { a: usize, b: usize },
    /// lines by index
    Parallel { l1: usize, l2: usize },
    Perpendicular { l1: usize, l2: usize },
    EqualLength { l1: usize, l2: usize },
    /// straight-line distance between two points
    Distance { a: usize, b: usize, d: f64 },
    /// signed x / y offset from a to b
    DistanceX { a: usize, b: usize, d: f64 },
    DistanceY { a: usize, b: usize, d: f64 },
    /// the point is on the (infinite) line through the line's two points
    PointOnLine { p: usize, l: usize },
    Midpoint { p: usize, l: usize },
    /// angle from line l1 to line l2 in degrees, counter-clockwise, taken modulo 180
    Angle { l1: usize, l2: usize, degrees: f64 },
    Radius { c: usize, r: f64 },
    EqualRadius { c1: usize, c2: usize },
    PointOnCircle { p: usize, c: usize },
    /// the (infinite) line touches the circle
    Tangent { l: usize, c: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub enum SketchError {
    TooMany(&'static str),
    BadIndex { constraint: usize, what: &'static str },
    BadValue { constraint: usize, what: &'static str },
    NotFinite(&'static str),
    /// a line whose two ends are the same point
    DegenerateLine(usize),
}

impl fmt::Display for SketchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SketchError::*;
        match self {
            TooMany(w) => write!(f, "too many {w}"),
            BadIndex { constraint, what } => write!(f, "constraint {constraint}: no such {what}"),
            BadValue { constraint, what } => write!(f, "constraint {constraint}: {what}"),
            NotFinite(w) => write!(f, "{w} is not a finite number within 1 000 000"),
            DegenerateLine(l) => write!(f, "line {l} starts and ends at the same point"),
        }
    }
}
impl std::error::Error for SketchError {}

#[derive(Clone, Debug, Default)]
pub struct Sketch {
    pub points: Vec<[f64; 2]>,
    pub lines: Vec<(usize, usize)>,
    pub circles: Vec<Circle>,
    pub constraints: Vec<Constraint>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// every constraint holds
    Solved,
    /// the constraints cannot all hold (they conflict) or no solution was found near the drawing
    Unsolved { residual: f64 },
}

#[derive(Clone, Debug)]
pub struct Solution {
    pub points: Vec<[f64; 2]>,
    pub radii: Vec<f64>,
    pub status: Status,
    /// movements left (coordinates and radii not pinned down); 0 means fully constrained
    pub dof: usize,
    /// equations (counting a lock or a midpoint as two) that repeat others
    pub redundant: usize,
    /// for each point, whether both coordinates are fully determined
    pub fixed: Vec<bool>,
    pub iterations: usize,
}

// ---------- forward-mode derivatives ----------

const N: usize = 8;

#[derive(Clone, Copy)]
struct D {
    v: f64,
    g: [f64; N],
}
impl D {
    fn var(v: f64, i: usize) -> D { let mut g = [0.0; N]; g[i] = 1.0; D { v, g } }
    fn con(v: f64) -> D { D { v, g: [0.0; N] } }
    fn sqrt(self) -> D {
        let s = self.v.max(0.0).sqrt();
        let k = if s > 1e-150 { 0.5 / s } else { 0.0 };
        D { v: s, g: self.g.map(|x| x * k) }
    }
}
impl Add for D { type Output = D; fn add(self, o: D) -> D { let mut g = self.g; for (a, b) in g.iter_mut().zip(o.g) { *a += b; } D { v: self.v + o.v, g } } }
impl Sub for D { type Output = D; fn sub(self, o: D) -> D { let mut g = self.g; for (a, b) in g.iter_mut().zip(o.g) { *a -= b; } D { v: self.v - o.v, g } } }
impl Neg for D { type Output = D; fn neg(self) -> D { D { v: -self.v, g: self.g.map(|x| -x) } } }
impl Mul for D {
    type Output = D;
    fn mul(self, o: D) -> D {
        let mut g = [0.0; N];
        for (k, x) in g.iter_mut().enumerate() { *x = self.g[k] * o.v + self.v * o.g[k]; }
        D { v: self.v * o.v, g }
    }
}

type Eval = Box<dyn Fn(&[D]) -> D>;
/// one scalar equation: the variables it reads (global numbers) and its residual
struct Row {
    vars: Vec<usize>,
    f: Eval,
}

fn cross(u: [D; 2], w: [D; 2]) -> D { u[0] * w[1] - u[1] * w[0] }
fn dot(u: [D; 2], w: [D; 2]) -> D { u[0] * w[0] + u[1] * w[1] }
fn diff(a: [D; 2], b: [D; 2]) -> [D; 2] { [b[0] - a[0], b[1] - a[1]] }
fn len(u: [D; 2]) -> D { dot(u, u).sqrt() }

impl Sketch {
    pub fn point(&mut self, x: f64, y: f64) -> usize { self.points.push([x, y]); self.points.len() - 1 }
    pub fn line(&mut self, a: usize, b: usize) -> usize { self.lines.push((a, b)); self.lines.len() - 1 }
    pub fn circle(&mut self, centre: usize, radius: f64) -> usize { self.circles.push(Circle { centre, radius }); self.circles.len() - 1 }
    pub fn constrain(&mut self, c: Constraint) -> &mut Self { self.constraints.push(c); self }

    fn validate(&self) -> Result<(), SketchError> {
        if self.points.len() > MAX_POINTS { return Err(SketchError::TooMany("points")); }
        if self.lines.len() > MAX_LINES { return Err(SketchError::TooMany("lines")); }
        if self.circles.len() > MAX_CIRCLES { return Err(SketchError::TooMany("circles")); }
        if self.constraints.len() > MAX_CONSTRAINTS { return Err(SketchError::TooMany("constraints")); }
        let fin = |v: f64| v.is_finite() && v.abs() <= MAX_VALUE;
        for p in &self.points { if !fin(p[0]) || !fin(p[1]) { return Err(SketchError::NotFinite("a point")); } }
        for (i, &(a, b)) in self.lines.iter().enumerate() {
            if a >= self.points.len() || b >= self.points.len() { return Err(SketchError::BadIndex { constraint: i, what: "point of a line" }); }
            if a == b { return Err(SketchError::DegenerateLine(i)); }
        }
        for c in &self.circles {
            if c.centre >= self.points.len() { return Err(SketchError::BadIndex { constraint: 0, what: "centre point" }); }
            if !fin(c.radius) || c.radius <= 0.0 { return Err(SketchError::NotFinite("a radius")); }
        }
        for (k, c) in self.constraints.iter().enumerate() {
            let pt = |p: usize| if p < self.points.len() { Ok(()) } else { Err(SketchError::BadIndex { constraint: k, what: "point" }) };
            let ln = |l: usize| if l < self.lines.len() { Ok(()) } else { Err(SketchError::BadIndex { constraint: k, what: "line" }) };
            let ci = |c: usize| if c < self.circles.len() { Ok(()) } else { Err(SketchError::BadIndex { constraint: k, what: "circle" }) };
            let val = |v: f64, positive: bool| {
                if !fin(v) { Err(SketchError::BadValue { constraint: k, what: "a value is not a finite number within 1 000 000" }) }
                else if positive && v <= 0.0 { Err(SketchError::BadValue { constraint: k, what: "the value must be above zero" }) }
                else { Ok(()) }
            };
            use Constraint::*;
            match *c {
                Fixed { p, x, y } => { pt(p)?; val(x, false)?; val(y, false)?; }
                Coincident { a, b } | Horizontal { a, b } | Vertical { a, b } => { pt(a)?; pt(b)?; }
                Parallel { l1, l2 } | Perpendicular { l1, l2 } | EqualLength { l1, l2 } => { ln(l1)?; ln(l2)?; }
                Distance { a, b, d } => { pt(a)?; pt(b)?; val(d, true)?; }
                DistanceX { a, b, d } | DistanceY { a, b, d } => { pt(a)?; pt(b)?; val(d, false)?; }
                PointOnLine { p, l } | Midpoint { p, l } => { pt(p)?; ln(l)?; }
                Angle { l1, l2, degrees } => { ln(l1)?; ln(l2)?; val(degrees, false)?; }
                Radius { c, r } => { ci(c)?; val(r, true)?; }
                EqualRadius { c1, c2 } => { ci(c1)?; ci(c2)?; }
                PointOnCircle { p, c } => { pt(p)?; ci(c)?; }
                Tangent { l, c } => { ln(l)?; ci(c)?; }
            }
        }
        Ok(())
    }

    fn rows(&self) -> Vec<Row> {
        let np = self.points.len();
        let xy = |p: usize| [2 * p, 2 * p + 1];
        let rv = |c: usize| 2 * np + c;
        let mut rows = Vec::new();
        let mut add = |vars: Vec<usize>, f: Eval| rows.push(Row { vars, f });
        use Constraint::*;
        for c in &self.constraints {
            match *c {
                Fixed { p, x, y } => {
                    add(vec![xy(p)[0]], Box::new(move |v| v[0] - D::con(x)));
                    add(vec![xy(p)[1]], Box::new(move |v| v[0] - D::con(y)));
                }
                Coincident { a, b } => {
                    add(vec![xy(a)[0], xy(b)[0]], Box::new(|v| v[0] - v[1]));
                    add(vec![xy(a)[1], xy(b)[1]], Box::new(|v| v[0] - v[1]));
                }
                Horizontal { a, b } => add(vec![xy(a)[1], xy(b)[1]], Box::new(|v| v[0] - v[1])),
                Vertical { a, b } => add(vec![xy(a)[0], xy(b)[0]], Box::new(|v| v[0] - v[1])),
                Parallel { l1, l2 } | Perpendicular { l1, l2 } | EqualLength { l1, l2 } => {
                    let (a, b) = self.lines[l1];
                    let (c2, d) = self.lines[l2];
                    let vars = vec![xy(a)[0], xy(a)[1], xy(b)[0], xy(b)[1], xy(c2)[0], xy(c2)[1], xy(d)[0], xy(d)[1]];
                    let kind = match *c { Parallel { .. } => 0, Perpendicular { .. } => 1, _ => 2 };
                    add(vars, Box::new(move |v| {
                        let u = diff([v[0], v[1]], [v[2], v[3]]);
                        let w = diff([v[4], v[5]], [v[6], v[7]]);
                        match kind { 0 => cross(u, w), 1 => dot(u, w), _ => len(u) - len(w) }
                    }));
                }
                Distance { a, b, d } => add(vec![xy(a)[0], xy(a)[1], xy(b)[0], xy(b)[1]], Box::new(move |v| len(diff([v[0], v[1]], [v[2], v[3]])) - D::con(d))),
                DistanceX { a, b, d } => add(vec![xy(a)[0], xy(b)[0]], Box::new(move |v| v[1] - v[0] - D::con(d))),
                DistanceY { a, b, d } => add(vec![xy(a)[1], xy(b)[1]], Box::new(move |v| v[1] - v[0] - D::con(d))),
                PointOnLine { p, l } => {
                    let (a, b) = self.lines[l];
                    add(vec![xy(p)[0], xy(p)[1], xy(a)[0], xy(a)[1], xy(b)[0], xy(b)[1]], Box::new(|v| cross(diff([v[2], v[3]], [v[4], v[5]]), diff([v[2], v[3]], [v[0], v[1]]))));
                }
                Midpoint { p, l } => {
                    let (a, b) = self.lines[l];
                    for k in 0..2 {
                        add(vec![xy(p)[k], xy(a)[k], xy(b)[k]], Box::new(|v| v[0] + v[0] - v[1] - v[2]));
                    }
                }
                Angle { l1, l2, degrees } => {
                    let (a, b) = self.lines[l1];
                    let (c2, d) = self.lines[l2];
                    let (s, co) = degrees.to_radians().sin_cos();
                    add(vec![xy(a)[0], xy(a)[1], xy(b)[0], xy(b)[1], xy(c2)[0], xy(c2)[1], xy(d)[0], xy(d)[1]], Box::new(move |v| {
                        let u = diff([v[0], v[1]], [v[2], v[3]]);
                        let w = diff([v[4], v[5]], [v[6], v[7]]);
                        cross(u, w) * D::con(co) - dot(u, w) * D::con(s)
                    }));
                }
                Radius { c, r } => add(vec![rv(c)], Box::new(move |v| v[0] - D::con(r))),
                EqualRadius { c1, c2 } => add(vec![rv(c1), rv(c2)], Box::new(|v| v[0] - v[1])),
                PointOnCircle { p, c } => {
                    let cc = self.circles[c].centre;
                    add(vec![xy(p)[0], xy(p)[1], xy(cc)[0], xy(cc)[1], rv(c)], Box::new(|v| len(diff([v[0], v[1]], [v[2], v[3]])) - v[4]));
                }
                Tangent { l, c } => {
                    let (a, b) = self.lines[l];
                    let cc = self.circles[c].centre;
                    add(vec![xy(a)[0], xy(a)[1], xy(b)[0], xy(b)[1], xy(cc)[0], xy(cc)[1], rv(c)], Box::new(|v| {
                        let u = diff([v[0], v[1]], [v[2], v[3]]);
                        let x = cross(u, diff([v[0], v[1]], [v[4], v[5]]));
                        x * x - v[6] * v[6] * dot(u, u)
                    }));
                }
            }
        }
        rows
    }

    /// Residuals and the Jacobian (rows = equations, columns = coordinates then radii).
    fn evaluate(rows: &[Row], x: &[f64]) -> (Vec<f64>, Vec<Vec<f64>>) {
        let n = x.len();
        let mut r = Vec::with_capacity(rows.len());
        let mut j = Vec::with_capacity(rows.len());
        for row in rows {
            let args: Vec<D> = row.vars.iter().enumerate().map(|(i, &g)| D::var(x[g], i)).collect();
            let out = (row.f)(&args);
            r.push(out.v);
            let mut line = vec![0.0; n];
            for (i, &g) in row.vars.iter().enumerate() { line[g] += out.g[i]; }
            j.push(line);
        }
        (r, j)
    }

    /// Every equation's residual and its exact derivatives for the given coordinates (all points as x, y pairs, then
    /// the radii). Zero residuals mean the constraints hold.
    pub fn residuals(&self, x: &[f64]) -> Result<(Vec<f64>, Vec<Vec<f64>>), SketchError> {
        self.validate()?;
        if x.len() != 2 * self.points.len() + self.circles.len() { return Err(SketchError::NotFinite("the coordinate list")); }
        Ok(Self::evaluate(&self.rows(), x))
    }

    pub fn solve(&self) -> Result<Solution, SketchError> {
        self.validate()?;
        let np = self.points.len();
        let n = 2 * np + self.circles.len();
        let mut x: Vec<f64> = self.points.iter().flat_map(|p| [p[0], p[1]]).chain(self.circles.iter().map(|c| c.radius)).collect();
        let rows = self.rows();
        let norm2 = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>();
        let inf = |r: &[f64]| r.iter().fold(0.0f64, |a, v| a.max(v.abs()));
        let (mut r, mut jac) = Self::evaluate(&rows, &x);
        let mut lambda = 1e-3;
        let mut iterations = 0;
        while inf(&r) > TOLERANCE && iterations < MAX_ITER && !rows.is_empty() {
            iterations += 1;
            // (JtJ + lambda I) d = -Jt r
            let mut a = vec![vec![0.0; n]; n];
            let mut b = vec![0.0; n];
            for (row, ri) in jac.iter().zip(&r) {
                for (i, &ji) in row.iter().enumerate() {
                    if ji == 0.0 { continue; }
                    b[i] -= ji * ri;
                    for (k, &jk) in row.iter().enumerate() { if jk != 0.0 { a[i][k] += ji * jk; } }
                }
            }
            let mut stepped = false;
            for _ in 0..30 {
                let mut m = a.clone();
                for (i, row) in m.iter_mut().enumerate() { row[i] += lambda * (1.0 + a[i][i]); }
                let Some(d) = solve_linear(m, b.clone()) else { lambda *= 10.0; continue };
                let trial: Vec<f64> = x.iter().zip(&d).map(|(p, q)| p + q).collect();
                let (tr, tj) = Self::evaluate(&rows, &trial);
                if norm2(&tr) < norm2(&r) {
                    x = trial; r = tr; jac = tj;
                    lambda = (lambda / 3.0).max(1e-15);
                    stepped = true;
                    break;
                }
                lambda *= 4.0;
            }
            if !stepped { break; }
        }
        let mut residual = inf(&r);
        let (rank, fixed_cols) = analyse(&jac, n);
        if residual <= TOLERANCE && rank < n && !rows.is_empty() {
            // free motions are left: settle on the answer closest to the drawing, not just any answer
            let x0: Vec<f64> = self.points.iter().flat_map(|p| [p[0], p[1]]).chain(self.circles.iter().map(|c| c.radius)).collect();
            if let Some(better) = Self::closest(&rows, &x0, &x) { x = better; let (nr, _) = Self::evaluate(&rows, &x); residual = inf(&nr); }
        }
        let fixed = (0..np).map(|p| fixed_cols[2 * p] && fixed_cols[2 * p + 1]).collect();
        Ok(Solution {
            points: (0..np).map(|p| [x[2 * p], x[2 * p + 1]]).collect(),
            radii: x[2 * np..].to_vec(),
            status: if residual <= TOLERANCE { Status::Solved } else { Status::Unsolved { residual } },
            dof: n - rank,
            redundant: rows.len() - rank,
            fixed,
            iterations,
        })
    }
    /// Move along the constraints towards the drawing `x0`: repeatedly step to the nearest point of the linearised
    /// constraints. Returns None if that does not end on a valid answer.
    fn closest(rows: &[Row], x0: &[f64], start: &[f64]) -> Option<Vec<f64>> {
        let m = rows.len();
        let mut x = start.to_vec();
        for _ in 0..40 {
            let (r, jac) = Self::evaluate(rows, &x);
            // delta = (x0 - x) + Jt mu with J delta = -r  =>  (J Jt + eps I) mu = -(r + J (x0 - x))
            let back: Vec<f64> = x0.iter().zip(&x).map(|(a, b)| a - b).collect();
            let mut a = vec![vec![0.0; m]; m];
            let mut rhs = vec![0.0; m];
            for i in 0..m {
                rhs[i] = -(r[i] + jac[i].iter().zip(&back).map(|(p, q)| p * q).sum::<f64>());
                for k in i..m {
                    let v: f64 = jac[i].iter().zip(&jac[k]).map(|(p, q)| p * q).sum();
                    a[i][k] = v;
                    a[k][i] = v;
                }
            }
            let scale = (0..m).map(|i| a[i][i]).fold(0.0f64, f64::max).max(1e-300);
            for (i, row) in a.iter_mut().enumerate() { row[i] += 1e-12 * scale; }
            let mu = solve_linear(a, rhs)?;
            let mut step = back;
            for (i, m_i) in mu.iter().enumerate() { for (st, j) in step.iter_mut().zip(&jac[i]) { *st += m_i * j; } }
            let size = step.iter().fold(0.0f64, |a, v| a.max(v.abs()));
            for (xi, st) in x.iter_mut().zip(&step) { *xi += st; }
            if size < 1e-13 { break; }
        }
        let (r, _) = Self::evaluate(rows, &x);
        r.iter().all(|v| v.abs() <= TOLERANCE).then_some(x)
    }

}

/// Solve a x = b by Gaussian elimination with partial pivoting; None if singular.
fn solve_linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[p][c].abs() < 1e-300 { return None; }
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..n {
            let f = a[r][c] / a[c][c];
            if f == 0.0 { continue; }
            let prow = a[c].clone();
            for (x, p) in a[r][c..].iter_mut().zip(&prow[c..]) { *x -= f * p; }
            let v = b[c];
            b[r] -= f * v;
        }
    }
    let mut x = vec![0.0; n];
    for c in (0..n).rev() {
        let s: f64 = (c + 1..n).map(|k| a[c][k] * x[k]).sum();
        x[c] = (b[c] - s) / a[c][c];
        if !x[c].is_finite() { return None; }
    }
    Some(x)
}

/// Rank of the Jacobian and, for every column, whether it is fully determined (the null space has nothing in it).
fn analyse(jac: &[Vec<f64>], n: usize) -> (usize, Vec<bool>) {
    let mut a: Vec<Vec<f64>> = jac.to_vec();
    let m = a.len();
    let scale = a.iter().flatten().fold(0.0f64, |s, v| s.max(v.abs()));
    let tol = 1e-9 * scale.max(1e-300);
    let mut col_of_row: Vec<usize> = Vec::new();
    let mut used = vec![false; n];
    let mut rank = 0;
    while rank < m {
        // largest remaining entry
        let mut best = (0.0, rank, 0);
        for (i, row) in a.iter().enumerate().skip(rank) {
            for (c, &v) in row.iter().enumerate() { if !used[c] && v.abs() > best.0 { best = (v.abs(), i, c); } }
        }
        if best.0 <= tol { break; }
        let (_, pr, pc) = best;
        a.swap(rank, pr);
        let pv = a[rank][pc];
        for v in a[rank].iter_mut() { *v /= pv; }
        let prow = a[rank].clone();
        for (i, row) in a.iter_mut().enumerate() {
            if i == rank { continue; }
            let f = row[pc];
            if f != 0.0 { for (k, v) in row.iter_mut().enumerate() { *v -= f * prow[k]; } }
        }
        used[pc] = true;
        col_of_row.push(pc);
        rank += 1;
    }
    let mut fixed = vec![false; n];
    for (r, &pc) in col_of_row.iter().enumerate() {
        fixed[pc] = (0..n).all(|c| used[c] || a[r][c].abs() <= tol * 1e3);
    }
    (rank, fixed)
}
