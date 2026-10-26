//! Associative plan dimensions (D-DIM). A dimension's two points can each keep a reference to the wall or grid they
//! were picked on, as Revit's dimensions keep references to the faces and lines they measure. The reference is the
//! place on that element: how far along its line (a fraction, so a point at an end stays at the end when the wall is
//! stretched) and how far to its left. After every change the dimension is regenerated from its references in the
//! same transaction, so it follows the wall when the wall moves and undo restores both together.
use crate::{Category, Element, ElementId, Geometry};
use std::collections::BTreeMap;

type P = [f64; 2];

/// The place on a wall or grid that one end of a dimension refers to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DimRef {
    pub element: ElementId,
    /// along the element's line from its start, as a fraction of its length
    pub t: f64,
    /// to the left of the line (seen from start to end), metres
    pub offset: f64,
}

/// The line a reference is measured on: a wall's axis or a grid's line.
fn line_of(e: &Element) -> Option<(P, P)> {
    match e.geometry {
        Some(Geometry::Linear { start, end, .. }) if e.category == Category::Wall => Some((start, end)),
        Some(Geometry::Grid { start, end }) if e.bulge == 0.0 => Some((start, end)),
        _ => None,
    }
}

/// Where the reference is now, or None when its element is gone or no longer a straight wall or grid.
pub fn point_of(r: &DimRef, elements: &BTreeMap<ElementId, Element>) -> Option<P> {
    let (s, e) = line_of(elements.get(&r.element)?)?;
    let (dx, dy) = (e[0] - s[0], e[1] - s[1]);
    let len = dx.hypot(dy);
    if len < 1e-9 { return None; }
    let n = [-dy / len, dx / len];
    Some([s[0] + dx * r.t + n[0] * r.offset, s[1] + dy * r.t + n[1] * r.offset])
}

/// The reference for a point picked on `level`: the wall whose footprint holds it (on its face, its axis or its end),
/// else a straight grid it lies on. None when the point is on neither, and the dimension end stays where it was put.
pub fn reference_at(elements: &BTreeMap<ElementId, Element>, level: ElementId, p: P) -> Option<DimRef> {
    const TOL: f64 = 1e-3;
    let fit = |e: &Element, reach: f64| -> Option<(DimRef, f64)> {
        let (s, t) = line_of(e)?;
        let (dx, dy) = (t[0] - s[0], t[1] - s[1]);
        let len = dx.hypot(dy);
        if len < 1e-9 { return None; }
        let (u, n) = ([dx / len, dy / len], [-dy / len, dx / len]);
        let along = (p[0] - s[0]) * u[0] + (p[1] - s[1]) * u[1];
        let left = (p[0] - s[0]) * n[0] + (p[1] - s[1]) * n[1];
        if along < -TOL || along > len + TOL || left.abs() > reach + TOL { return None; }
        Some((DimRef { element: e.id, t: along / len, offset: left }, left.abs()))
    };
    let wall = elements.values()
        .filter(|e| e.category == Category::Wall && matches!(e.geometry, Some(Geometry::Linear { level: l, .. }) if l == level))
        .filter_map(|e| fit(e, e.width_m / 2.0))
        .min_by(|a, b| a.1.total_cmp(&b.1));
    wall.or_else(|| elements.values().filter(|e| e.category == Category::Grid).filter_map(|e| fit(e, 0.0)).min_by(|a, b| a.1.total_cmp(&b.1)))
        .map(|(r, _)| r)
}

/// Puts every associative dimension's ends where their references are now. An end whose element is gone keeps its
/// last place and loses the reference (it is no longer associative), as the dimension would otherwise be lost.
pub(crate) fn regenerate(elements: &mut BTreeMap<ElementId, Element>, revision: u64) {
    let updates: Vec<(ElementId, Geometry)> = elements.values().filter_map(|e| match e.geometry {
        Some(Geometry::Dimension { a, b, at, level, refs }) if refs.iter().any(Option::is_some) => {
            let place = |end: P, r: Option<DimRef>| match r.and_then(|r| point_of(&r, elements).map(|p| (p, r))) {
                Some((p, r)) => (p, Some(r)),
                None => (end, None),
            };
            let ((na, ra), (nb, rb)) = (place(a, refs[0]), place(b, refs[1]));
            // the dimension line keeps its distance from the measured line: it moves with the first end
            let nat = [at[0] + na[0] - a[0], at[1] + na[1] - a[1]];
            let g = Geometry::Dimension { a: na, b: nb, at: nat, level, refs: [ra, rb] };
            (Some(g) != e.geometry).then_some((e.id, g))
        }
        _ => None,
    }).collect();
    for (id, g) in updates {
        if let Some(e) = elements.get_mut(&id) { e.geometry = Some(g); e.revision = revision; }
    }
}

/// A reference written in a record: `element:t:offset`, or `-` for none.
pub(crate) fn ref_text(r: &Option<DimRef>) -> String {
    match r { Some(r) => format!("{}:{:?}:{:?}", r.element.0, r.t, r.offset), None => "-".into() }
}

pub(crate) fn ref_parse(s: &str) -> Result<Option<DimRef>, String> {
    if s == "-" { return Ok(None); }
    let p: Vec<&str> = s.split(':').collect();
    let [id, t, off] = p.as_slice() else { return Err(format!("bad dimension reference '{s}'")) };
    let f = |v: &str| v.parse::<f64>().ok().filter(|x| x.is_finite()).ok_or_else(|| format!("bad dimension reference '{s}'"));
    Ok(Some(DimRef { element: ElementId(id.parse().map_err(|_| format!("bad dimension reference '{s}'"))?), t: f(t)?, offset: f(off)? }))
}
