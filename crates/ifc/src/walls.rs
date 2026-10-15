//! Wall types (layer sets) and the joins between wall elements.
//!
//! A drawn wall is one element however many segments it has. Joins are written between two different elements:
//! the end of one wall meeting the end of another (an L or a mitre), or the end of one meeting the side of another
//! (a T). Corners inside one chain belong to that one element and need no relation.
use openbim_geom::WallChain;

type P = [f64; 2];

/// What a layer is for, as in Revit's compound walls. The number is the layer's join priority: where two walls join,
/// the layer with the lower number runs through the other wall's layers (structure first, finishes last).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Function {
    Structure,
    Substrate,
    ThermalAir,
    Membrane,
    Finish1,
    Finish2,
}

impl Function {
    pub fn priority(self) -> u8 {
        match self {
            Function::Structure => 1,
            Function::Substrate => 2,
            Function::ThermalAir => 3,
            Function::Membrane => 4,
            Function::Finish1 => 5,
            Function::Finish2 => 6,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Function::Structure => "Structure",
            Function::Substrate => "Substrate",
            Function::ThermalAir => "Thermal/Air Layer",
            Function::Membrane => "Membrane Layer",
            Function::Finish1 => "Finish 1",
            Function::Finish2 => "Finish 2",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub material: String,
    pub thickness: f64,
    pub function: Function,
    /// the layer turns round the ends of the wall and the edges of its openings
    pub wraps: bool,
}

impl Layer {
    /// A structure layer that does not wrap.
    pub fn new(material: &str, thickness: f64) -> Layer {
        Layer { material: material.into(), thickness, function: Function::Structure, wraps: false }
    }
    pub fn with(material: &str, thickness: f64, function: Function) -> Layer {
        Layer { material: material.into(), thickness, function, wraps: false }
    }
}

/// Where the line the author draws runs in the thickness of the wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationLine {
    WallCentreline,
    CoreCentreline,
    FinishFaceExterior,
    FinishFaceInterior,
    CoreFaceExterior,
    CoreFaceInterior,
}

/// A wall type: its layers from the exterior face to the interior face, as in Revit's compound walls. The exterior
/// is on the left of the direction of travel (walls are drawn clockwise, so that the outside is on the outside).
/// The thicknesses add up to the thickness of the walls of this type. The core is the run of layers between the
/// core boundaries (first and last layer, counted from 0); without one the whole wall is the core.
#[derive(Debug, Clone, PartialEq)]
pub struct WallType {
    pub name: String,
    pub layers: Vec<Layer>,
    pub core: Option<(usize, usize)>,
}

impl WallType {
    pub fn thickness(&self) -> f64 {
        self.layers.iter().map(|l| l.thickness).sum()
    }
    /// The first and last layer of the core.
    pub fn core_range(&self) -> (usize, usize) {
        self.core.unwrap_or((0, self.layers.len().saturating_sub(1)))
    }
    pub fn check(&self) -> Result<(), String> {
        if self.layers.is_empty() {
            return Err("a wall type needs at least one layer".into());
        }
        if self.layers.len() > 64 {
            return Err("more than 64 layers".into());
        }
        for l in &self.layers {
            if !l.thickness.is_finite() || l.thickness < 0.0 || (l.thickness == 0.0 && l.function != Function::Membrane) {
                return Err(format!("layer '{}' has no thickness above 0 (only a membrane layer may have none)", l.material));
            }
            if l.material.trim().is_empty() {
                return Err("a layer has no material name".into());
            }
        }
        let (a, b) = self.core_range();
        if a > b || b >= self.layers.len() {
            return Err("the core boundaries are not on layers of the wall".into());
        }
        let core = &self.layers[a..=b];
        if core.iter().any(|l| l.function == Function::Membrane) {
            return Err("a membrane layer cannot be in the core".into());
        }
        if !core.iter().any(|l| l.function == Function::Structure) {
            return Err("the core needs a structure layer".into());
        }
        if self.thickness() <= 0.0 {
            return Err("the wall has no thickness".into());
        }
        Ok(())
    }
    /// Distance from the exterior face to the core's exterior face, and the thickness of the core.
    fn core_position(&self) -> (f64, f64) {
        let (a, b) = self.core_range();
        let before: f64 = self.layers[..a].iter().map(|l| l.thickness).sum();
        let core: f64 = self.layers[a..=b].iter().map(|l| l.thickness).sum();
        (before, core)
    }
    /// How far the body's centre line lies to the left of the line the author draws, for a location line:
    /// the drawn line is `pos` from the exterior face, the centre is half the thickness from it.
    pub fn centreline_shift(&self, loc: LocationLine) -> f64 {
        let t = self.thickness();
        let (before, core) = self.core_position();
        let pos = match loc {
            LocationLine::WallCentreline => t / 2.0,
            LocationLine::CoreCentreline => before + core / 2.0,
            LocationLine::FinishFaceExterior => 0.0,
            LocationLine::FinishFaceInterior => t,
            LocationLine::CoreFaceExterior => before,
            LocationLine::CoreFaceInterior => before + core,
        };
        pos - t / 2.0
    }
}

/// A wall of this type drawn along a location line: the chain's centre line is the drawn path moved sideways to
/// where the wall's centre is. Corners are mitred; a corner too sharp to mitre is refused.
pub fn wall_on_location_line(points: &[P], closed: bool, t: &WallType, loc: LocationLine, height: f64) -> Result<WallChain, String> {
    t.check()?;
    let path = offset_path(points, closed, t.centreline_shift(loc))?;
    Ok(WallChain::new(path, closed, t.thickness(), height))
}

/// The path moved `d` to the left of its direction of travel (right when negative), corners mitred.
pub fn offset_path(points: &[P], closed: bool, d: f64) -> Result<Vec<P>, String> {
    let n = points.len();
    if n < 2 || (closed && n < 3) {
        return Err("a path needs two points (three when closed)".into());
    }
    if !d.is_finite() || points.iter().any(|p| !p[0].is_finite() || !p[1].is_finite()) {
        return Err("a number is not finite".into());
    }
    let segs = if closed { n } else { n - 1 };
    let mut dirs: Vec<P> = Vec::new();
    for i in 0..segs {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let l = (b[0] - a[0]).hypot(b[1] - a[1]);
        if l < 1e-9 {
            return Err(format!("segment {i} has no length"));
        }
        dirs.push([(b[0] - a[0]) / l, (b[1] - a[1]) / l]);
    }
    let left = |u: P| [-u[1], u[0]];
    let mut out = Vec::with_capacity(n);
    for (k, &p) in points.iter().enumerate() {
        let before = if k == 0 { if closed { Some(segs - 1) } else { None } } else { Some(k - 1) };
        let after = if k < segs { Some(k) } else { None };
        match (before, after) {
            (Some(a), Some(b)) => {
                let (u, v) = (dirs[a], dirs[b]);
                let cross = u[0] * v[1] - u[1] * v[0];
                let dot = u[0] * v[0] + u[1] * v[1];
                if cross.abs() < 1e-9 {
                    if dot < 0.0 {
                        return Err(format!("the path doubles back at point {k}"));
                    }
                    let nl = left(u);
                    out.push([p[0] + d * nl[0], p[1] + d * nl[1]]);
                } else {
                    // the corner of the two offset lines: p + d * (n_u + n_v) / (1 + u.v)
                    let (nu, nv) = (left(u), left(v));
                    let k2 = d / (1.0 + dot);
                    if 1.0 + dot < 1e-3 {
                        return Err(format!("the corner at point {k} is too sharp to mitre"));
                    }
                    out.push([p[0] + k2 * (nu[0] + nv[0]), p[1] + k2 * (nu[1] + nv[1])]);
                }
            }
            (None, Some(b)) => {
                let nl = left(dirs[b]);
                out.push([p[0] + d * nl[0], p[1] + d * nl[1]]);
            }
            (Some(a), None) => {
                let nl = left(dirs[a]);
                out.push([p[0] + d * nl[0], p[1] + d * nl[1]]);
            }
            (None, None) => unreachable!("a path has segments"),
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinEnd {
    Start,
    End,
    /// along the wall (the wall being met by a T)
    Path,
}

impl JoinEnd {
    pub(crate) fn ifc(self) -> &'static str {
        match self {
            JoinEnd::Start => ".ATSTART.",
            JoinEnd::End => ".ATEND.",
            JoinEnd::Path => ".ATPATH.",
        }
    }
}

/// `a` (at `a_end`) meets `b` (at `b_end`). Indices are into `Scene::walls`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallJoin {
    pub a: usize,
    pub a_end: JoinEnd,
    pub b: usize,
    pub b_end: JoinEnd,
}

fn dist_point_segment(p: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 == 0.0 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) };
    (p[0] - (a[0] + t * dx)).hypot(p[1] - (a[1] + t * dy))
}

fn path_distance(c: &WallChain, p: P) -> f64 {
    let n = c.points.len();
    let segs = if c.closed { n } else { n - 1 };
    (0..segs).map(|i| dist_point_segment(p, c.points[i], c.points[(i + 1) % n])).fold(f64::INFINITY, f64::min)
}

fn same_level(a: &WallChain, b: &WallChain) -> bool {
    // the two walls stand on the same storey span (they overlap in height)
    a.base_z < b.base_z + b.height - 1e-9 && b.base_z < a.base_z + a.height - 1e-9
}

/// The free ends of open walls that touch another wall. An end counts as touching when it is within `tol` of the
/// other wall's end point (an L: End/Start), or within half that wall's thickness plus `tol` of its centre line (a T).
/// Closed walls have no ends; a wall is not joined to itself.
pub fn find_joins(walls: &[&WallChain], tol: f64) -> Vec<WallJoin> {
    find_joins_except(walls, tol, &[])
}

/// As [`find_joins`], but the listed wall ends are not joined to anything: Revit's "Disallow Join" at a wall end.
/// A wall's end that is disallowed neither joins another wall nor is joined by one at that end.
pub fn find_joins_except(walls: &[&WallChain], tol: f64, disallowed: &[(usize, JoinEnd)]) -> Vec<WallJoin> {
    let mut out: Vec<WallJoin> = Vec::new();
    let all_ends = |c: &WallChain| -> Vec<(JoinEnd, P)> {
        if c.closed || c.points.len() < 2 {
            vec![]
        } else {
            vec![(JoinEnd::Start, c.points[0]), (JoinEnd::End, *c.points.last().expect("points"))]
        }
    };
    let ends = |wi: usize, c: &WallChain| -> Vec<(JoinEnd, P)> { all_ends(c).into_iter().filter(|(e, _)| !disallowed.contains(&(wi, *e))).collect() };
    for (i, wa) in walls.iter().enumerate() {
        for (ea, pa) in ends(i, wa) {
            for (j, wb) in walls.iter().enumerate() {
                if i == j || !same_level(wa, wb) {
                    continue;
                }
                // an end that lands on an end that is not to be joined is left alone
                if all_ends(wb).iter().any(|(e, pb)| disallowed.contains(&(j, *e)) && (pa[0] - pb[0]).hypot(pa[1] - pb[1]) <= tol + 0.5 * wa.thickness.max(wb.thickness)) {
                    continue;
                }
                // end to end: write each pair once, from the lower index
                let mut found: Option<JoinEnd> = None;
                for (eb, pb) in ends(j, wb) {
                    if (pa[0] - pb[0]).hypot(pa[1] - pb[1]) <= tol + 0.5 * wa.thickness.max(wb.thickness) {
                        found = Some(eb);
                    }
                }
                if let Some(eb) = found {
                    if i < j {
                        out.push(WallJoin { a: i, a_end: ea, b: j, b_end: eb });
                    }
                    continue;
                }
                if path_distance(wb, pa) <= 0.5 * wb.thickness + tol {
                    out.push(WallJoin { a: i, a_end: ea, b: j, b_end: JoinEnd::Path });
                }
            }
        }
    }
    out.sort_by_key(|j| (j.a, j.b, j.a_end as u8, j.b_end as u8));
    out.dedup();
    out
}
