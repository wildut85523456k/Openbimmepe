//! Wall types (layer sets) and the joins between wall elements.
//!
//! A drawn wall is one element however many segments it has. Joins are written between two different elements:
//! the end of one wall meeting the end of another (an L or a mitre), or the end of one meeting the side of another
//! (a T). Corners inside one chain belong to that one element and need no relation.
use openbim_geom::WallChain;

type P = [f64; 2];

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub material: String,
    pub thickness: f64,
}

/// A wall type: its layers from the right of the direction of travel to the left (so, on a closed wall drawn
/// counter-clockwise, from the outside to the inside). The thicknesses add up to the thickness of the walls of this type.
#[derive(Debug, Clone, PartialEq)]
pub struct WallType {
    pub name: String,
    pub layers: Vec<Layer>,
}

impl WallType {
    pub fn thickness(&self) -> f64 {
        self.layers.iter().map(|l| l.thickness).sum()
    }
    pub fn check(&self) -> Result<(), String> {
        if self.layers.is_empty() {
            return Err("a wall type needs at least one layer".into());
        }
        if self.layers.len() > 64 {
            return Err("more than 64 layers".into());
        }
        for l in &self.layers {
            if !(l.thickness.is_finite() && l.thickness > 0.0) {
                return Err(format!("layer '{}' has no thickness above 0", l.material));
            }
            if l.material.trim().is_empty() {
                return Err("a layer has no material name".into());
            }
        }
        Ok(())
    }
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
    let mut out: Vec<WallJoin> = Vec::new();
    let ends = |c: &WallChain| -> Vec<(JoinEnd, P)> {
        if c.closed || c.points.len() < 2 {
            vec![]
        } else {
            vec![(JoinEnd::Start, c.points[0]), (JoinEnd::End, *c.points.last().expect("points"))]
        }
    };
    for (i, wa) in walls.iter().enumerate() {
        for (ea, pa) in ends(wa) {
            for (j, wb) in walls.iter().enumerate() {
                if i == j || !same_level(wa, wb) {
                    continue;
                }
                // end to end: write each pair once, from the lower index
                let mut found: Option<JoinEnd> = None;
                for (eb, pb) in ends(wb) {
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
