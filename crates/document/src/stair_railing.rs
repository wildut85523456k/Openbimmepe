//! The railing of a stair, as Revit puts one on a stair when it is made: along the edge of each run, over the landing
//! where the stair turns, rising with the runs and level over the landing. The railing points at the first run of the
//! stair, so it goes with it; its heights are not stored but read from the stair, so that it stays on the steps.
use crate::{Category, Document, ElementId, Geometry};

type P = [f64; 2];

/// Which sides of a stair get a railing, seen climbing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RailingSides {
    None,
    Left,
    Right,
    #[default]
    Both,
}

impl RailingSides {
    pub const ALL: [(RailingSides, &'static str); 4] = [(RailingSides::Both, "Both sides"), (RailingSides::Left, "Left side"), (RailingSides::Right, "Right side"), (RailingSides::None, "None")];
}

/// The edge of a run on its left or right, as the run is climbed.
fn edge(run: (P, P), width: f64, left: bool) -> (P, P) {
    let (dx, dy) = (run.1[0] - run.0[0], run.1[1] - run.0[1]);
    let l = dx.hypot(dy).max(1e-12);
    let s = if left { 1.0 } else { -1.0 };
    let off = [-dy / l * s * width / 2.0, dx / l * s * width / 2.0];
    ([run.0[0] + off[0], run.0[1] + off[1]], [run.1[0] + off[0], run.1[1] + off[1]])
}

fn seg_dist(p: P, a: P, b: P) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let l2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if l2 > 0.0 { (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (a[0] + ab[0] * t - p[0]).hypot(a[1] + ab[1] * t - p[1])
}

/// The corners to pass walking the edge of the landing `ring` from `a` to `b`, the short way that does not cross the last
/// edge of the ring (the side the runs come in at, where there is the stair and no railing).
fn route(ring: &[P], a: P, b: P) -> Vec<P> {
    let n = ring.len();
    let on = |p: P| (0..n).find(|&i| seg_dist(p, ring[i], ring[(i + 1) % n]) < 1e-6);
    let (Some(ia), Some(ib)) = (on(a), on(b)) else { return Vec::new() };
    if ia == ib { return Vec::new(); }
    let length = |via: &[P]| { let mut pts = vec![a]; pts.extend_from_slice(via); pts.push(b); pts.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum::<f64>() };
    let (mut forward, mut backward) = (Some(Vec::new()), Some(Vec::new()));
    let mut cur = ia;
    while cur != ib {
        if cur == n - 1 { forward = None; break; }
        forward.as_mut().unwrap().push(ring[(cur + 1) % n]);
        cur = (cur + 1) % n;
    }
    let mut cur = ia;
    while cur != ib {
        if cur == n - 1 { backward = None; break; }
        backward.as_mut().unwrap().push(ring[cur]);
        cur = (cur + n - 1) % n;
    }
    match (forward, backward) {
        (Some(f), Some(b2)) => if length(&f) <= length(&b2) { f } else { b2 },
        (Some(f), None) => f,
        (None, Some(b2)) => b2,
        (None, None) => Vec::new(),
    }
}

/// The path in plan of the railing along the left or the right side of a stair of one or two `runs` (the start and the
/// end of each, `width` wide), with the `landing` outline between two runs.
pub fn rail_path(runs: &[(P, P)], landing: Option<&[P]>, width: f64, left: bool) -> Vec<P> {
    let mut pts: Vec<P> = Vec::new();
    for (k, run) in runs.iter().enumerate() {
        let (a, b) = edge(*run, width, left);
        if k > 0 {
            if let (Some(ring), Some(last)) = (landing, pts.last().copied()) { pts.extend(route(ring, last, a)); }
        }
        pts.push(a);
        pts.push(b);
    }
    pts.dedup_by(|q, p| (q[0] - p[0]).hypot(q[1] - p[1]) < 1e-9);
    pts
}

fn inside(ring: &[P], p: P) -> bool {
    let n = ring.len();
    if (0..n).any(|i| seg_dist(p, ring[i], ring[(i + 1) % n]) < 1e-6) { return true; }
    let mut c = false;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] { c = !c; }
    }
    c
}

impl Document {
    /// The height of the stair `root` (all its runs and its landing) at the plan point `p`, where a railing stands: along a
    /// run it climbs from the base to the top over the length of the treads, over the landing it is the landing's top.
    pub fn stair_surface_z(&self, root: ElementId, p: P) -> Option<f64> {
        let mut found: Option<f64> = None;
        for id in self.stair_parts(root) {
            let Some(e) = self.get(id) else { continue };
            let z = match e.geometry {
                Some(Geometry::Linear { start, end, level }) => {
                    let (dx, dy) = (end[0] - start[0], end[1] - start[1]);
                    let len = dx.hypot(dy);
                    if len < 1e-9 { continue; }
                    let (d, rel) = ([dx / len, dy / len], [p[0] - start[0], p[1] - start[1]]);
                    let (x, lat) = (rel[0] * d[0] + rel[1] * d[1], -rel[0] * d[1] + rel[1] * d[0]);
                    if x < -1e-6 || x > len + 1e-6 || lat.abs() > e.width_m / 2.0 + 0.05 { continue; }
                    self.get(level)?.elevation_m + e.elevation_m + e.height_m * x.clamp(0.0, len) / len
                }
                Some(Geometry::Area { level }) if inside(&e.outline, p) => self.get(level)?.elevation_m + e.elevation_m,
                _ => continue,
            };
            found = Some(found.map_or(z, |f: f64| f.min(z)));
        }
        found
    }

    /// The path of a railing with its heights: a railing on a stair has the heights of the stair under each point, plus
    /// its own base offset; any other stands at its offset above its level.
    pub fn railing_path3(&self, id: ElementId) -> Option<Vec<[f64; 3]>> {
        let e = self.get(id).filter(|e| e.category == Category::Railing)?;
        let Some(Geometry::Area { level }) = e.geometry else { return None };
        let flat = self.get(level)?.elevation_m + e.elevation_m;
        let host = e.parent.filter(|h| self.get(*h).is_some_and(|h| h.category == Category::Stair));
        Some(e.outline.iter().map(|p| [p[0], p[1], host.and_then(|h| self.stair_surface_z(h, *p)).map_or(flat, |z| z + e.elevation_m)]).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Document, StairShape, StairSpec, Xf};

    fn spec() -> StairSpec { StairSpec { start: [1.0, 1.0], toward: [1.0, 9.0], width_m: 1.0, rise_m: 3.0, risers: 18, tread_m: 0.28, base_offset_m: 0.0 } }

    fn railings(d: &Document) -> Vec<ElementId> { d.elements().filter(|e| e.category == Category::Railing).map(|e| e.id).collect() }

    #[test]
    fn a_stair_gets_its_railing_on_the_sides_asked_for_climbing_with_the_runs_and_level_over_the_landing() {
        for shape in [StairShape::Straight, StairShape::L { left: true }, StairShape::L { left: false }, StairShape::U { left: true }, StairShape::U { left: false }] {
            let mut d = Document::spatial_sample();
            let level = ElementId(10);
            let level_z = d.get(level).unwrap().elevation_m;
            let id = d.place_stair_with_railing(level, &spec(), shape, RailingSides::Both).unwrap();
            let rails = railings(&d);
            assert_eq!(rails.len(), 2, "{shape:?}");
            assert!(rails.iter().all(|r| d.get(*r).unwrap().parent == Some(id)));
            // the parts of the stair are still its runs and landing; a railing is not one of them
            assert_eq!(d.stair_parts(id).len(), if shape == StairShape::Straight { 1 } else { 3 });
            // the first run goes +y from x = 1, so its left edge is at x = 0.5 and its right edge at x = 1.5
            let lefts: Vec<bool> = rails.iter().map(|r| d.get(*r).unwrap().outline[0][0] < 1.0).collect();
            assert_eq!(lefts.iter().filter(|l| **l).count(), 1, "one on each side: {shape:?}");
            for r in &rails {
                let p3 = d.railing_path3(*r).unwrap();
                // it starts on the first run at the edge of the stair, at the foot of the stair, and ends at the top of it
                assert!(((p3[0][0] - 1.0).abs() - 0.5).abs() < 1e-9 && (p3[0][1] - 1.0).abs() < 1e-9, "{shape:?} {p3:?}");
                assert!((p3[0][2] - level_z).abs() < 1e-9, "{shape:?}: starts at the foot {p3:?}");
                assert!((p3.last().unwrap()[2] - (level_z + 3.0)).abs() < 1e-9, "{shape:?}: ends at the top {p3:?}");
                // never going down, and level (the height of the landing) wherever the plan point is on the landing alone
                assert!(p3.windows(2).all(|w| w[1][2] >= w[0][2] - 1e-9), "{shape:?} {p3:?}");
                if shape != StairShape::Straight {
                    assert!(p3.iter().any(|q| (q[2] - (level_z + 1.5)).abs() < 1e-9), "{shape:?}: a vertex at the landing height");
                    let flat: Vec<_> = p3.windows(2).filter(|w| (w[0][2] - (level_z + 1.5)).abs() < 1e-9 && (w[1][2] - (level_z + 1.5)).abs() < 1e-9).collect();
                    assert!(!flat.is_empty() || matches!(shape, StairShape::L { .. }), "{shape:?}");
                }
                // the rail climbs along the run at the slope of the stair's line from foot to top
                let w = &p3[..2];
                let run = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
                let n = if shape == StairShape::Straight { 18.0 } else { 9.0 };
                assert!((run - (n - 1.0) * 0.28).abs() < 1e-9 && (w[1][2] - w[0][2] - n * 3.0 / 18.0).abs() < 1e-9, "{shape:?} {w:?}");
                // a closed box for every part, the top rail 1100 mm over the base at the top
                let m = d.mesh(*r).unwrap();
                assert_eq!(m.faces.len() % 6, 0);
                let top = m.vertices.iter().map(|v| v[2]).fold(f64::MIN, f64::max);
                assert!((top - (level_z + 3.0 + 1.1)).abs() < 0.1, "{shape:?}: {top}");
            }
            // one undo takes the stair and its railings away
            let mut probe = d.clone();
            probe.undo().unwrap();
            assert!(probe.elements().all(|e| !matches!(e.category, Category::Stair | Category::Railing)), "{shape:?}");
            // moved with the stair by selecting the first run only; the railing keeps its heights
            let before: Vec<_> = rails.iter().map(|r| d.railing_path3(*r).unwrap()).collect();
            let (ops, _) = (d.transform_commands(&[id], Xf::Move([2.0, 0.5])).unwrap(), ());
            d.transact(d.revision(), &ops).unwrap();
            for (r, b) in rails.iter().zip(&before) {
                let a = d.railing_path3(*r).unwrap();
                assert!(a.iter().zip(b).all(|(p, q)| (p[0] - q[0] - 2.0).abs() < 1e-9 && (p[1] - q[1] - 0.5).abs() < 1e-9 && (p[2] - q[2]).abs() < 1e-9), "{shape:?}");
            }
            // turned and mirrored: still on the steps
            for xf in [Xf::Rotate([0.0, 0.0], 90f64.to_radians()), Xf::Mirror([0.0, 0.0], [1.0, 0.0])] {
                let ops = d.transform_commands(&[id], xf).unwrap();
                d.transact(d.revision(), &ops).unwrap();
                for r in &rails {
                    let p3 = d.railing_path3(*r).unwrap();
                    // (a mirror may turn the path round: the ends are still the foot and the top)
                    let (lo, hi) = p3.iter().fold((f64::MAX, f64::MIN), |(l, h), q| (l.min(q[2]), h.max(q[2])));
                    assert!((lo - level_z).abs() < 1e-9 && (hi - (level_z + 3.0)).abs() < 1e-9 && ((p3[0][2] - level_z).abs() < 1e-9) != ((p3.last().unwrap()[2] - level_z).abs() < 1e-9), "{shape:?} {xf:?} {p3:?}");
                }
            }
            // saved natively and read back
            let (els, site) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
            let mut back = Document::new();
            back.transact(back.revision(), &crate::native::restore_commands(els, site)).unwrap();
            for r in &rails { assert_eq!(back.mesh(*r).unwrap().vertices, d.mesh(*r).unwrap().vertices, "{shape:?}"); }
            // copied with the stair
            let (ops, _) = d.copy_commands(&[id], Xf::Move([6.0, 0.0])).unwrap();
            d.transact(d.revision(), &ops).unwrap();
            assert_eq!(railings(&d).len(), 4, "{shape:?}");
            // deleted with it
            let ops = d.delete_commands(&[id]).unwrap();
            d.transact(d.revision(), &ops).unwrap();
            assert_eq!(railings(&d).len(), 2, "{shape:?}");
        }
        // one side, or none
        let mut d = Document::spatial_sample();
        d.place_stair_with_railing(ElementId(10), &spec(), StairShape::Straight, RailingSides::Left).unwrap();
        assert_eq!(railings(&d).len(), 1);
        assert!(d.get(railings(&d)[0]).unwrap().outline[0][0] < 1.0, "the left of a stair climbing +y is -x");
        d.place_stair_with_railing(ElementId(10), &StairSpec { start: [4.0, 1.0], toward: [4.0, 9.0], ..spec() }, StairShape::Straight, RailingSides::None).unwrap();
        assert_eq!(railings(&d).len(), 1);
        // a railing cannot point at a thing that is not a stair
        let rid = railings(&d)[0];
        assert!(d.transact(d.revision(), &[Command::SetParent { id: rid, parent: Some(ElementId(10)) }]).is_err());
    }

    #[test]
    fn the_rail_goes_round_the_landing_by_the_outside_and_straight_across_the_inside() {
        // L turning left: the left rail has a corner where the runs meet, the right rail goes round the landing
        let lay = crate::stair::layout(&spec(), StairShape::L { left: true }).unwrap();
        let runs = [(lay.run1[0], lay.run1[1]), (lay.run2[0], lay.run2[1])];
        let inside = rail_path(&runs, Some(&lay.landing), 1.0, true);
        let outside = rail_path(&runs, Some(&lay.landing), 1.0, false);
        assert_eq!(inside.len(), 3, "{inside:?}");
        assert_eq!(outside.len(), 5, "{outside:?}");
        // U: the rail on the inside crosses the gap between the runs, the one on the outside goes round the landing
        let lay = crate::stair::layout(&spec(), StairShape::U { left: true }).unwrap();
        let runs = [(lay.run1[0], lay.run1[1]), (lay.run2[0], lay.run2[1])];
        assert_eq!(rail_path(&runs, Some(&lay.landing), 1.0, true).len(), 4);
        assert_eq!(rail_path(&runs, Some(&lay.landing), 1.0, false).len(), 6);
    }
}
