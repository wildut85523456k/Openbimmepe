//! Straight stairs, as Revit's Stair by Component with one Run: `count` risers climb `height_m` between the base
//! (the level plus `elevation_m`) and the top, over `count - 1` treads along the line from `start` to `end`. The stair
//! is a solid stepped block `width_m` wide (a monolithic stair without a soffit), and the top riser is the last
//! thing of the run: the floor above continues from it.
use super::{Category, Command, Document, EditError, ElementId, Geometry, Mesh};

type P = [f64; 2];

/// A stair to place.
#[derive(Clone, Debug, PartialEq)]
pub struct StairSpec {
    /// the foot of the first riser, at the middle of the width
    pub start: P,
    /// the direction of the run (any point on it; only the direction is used)
    pub toward: P,
    pub width_m: f64,
    /// climb from the base to the top
    pub rise_m: f64,
    pub risers: u32,
    pub tread_m: f64,
    /// how far above its level the foot of the stair is
    pub base_offset_m: f64,
}

/// Revit's defaults, as far as I know them: a tread of 280 mm and a riser of at most 175 mm, 1000 mm wide.
pub const MAX_RISER_M: f64 = 0.175;
pub const TREAD_M: f64 = 0.28;
pub const WIDTH_M: f64 = 1.0;

/// How many risers climb `rise` without any being higher than `max_riser`.
pub fn risers_for(rise: f64, max_riser: f64) -> u32 { (rise / max_riser - 1e-9).ceil().max(2.0) as u32 }

/// The side profile of the stair as (distance along the run, height) points: the foot of the first riser, up and
/// along over each of the `risers - 1` treads, down the end of the last tread, and back along the bottom. The top riser
/// is the edge of the floor above, where the run ends. Clockwise seen from the right.
pub fn profile(risers: u32, riser_h: f64, tread: f64) -> Vec<P> {
    let n = risers as usize;
    let mut p = vec![[0.0, 0.0]];
    for i in 1..n {
        p.push([(i - 1) as f64 * tread, i as f64 * riser_h]);
        p.push([i as f64 * tread, i as f64 * riser_h]);
    }
    p.push([(n - 1) as f64 * tread, 0.0]);
    p
}

/// The solid: sides and the faces of the steps, all wound outward. None for a run with no length.
pub fn mesh(start: P, end: P, width: f64, rise: f64, risers: u32, z0: f64) -> Option<Mesh> {
    let (dx, dy) = (end[0] - start[0], end[1] - start[1]);
    let run = dx.hypot(dy);
    if run < 1e-9 || risers < 2 { return None; }
    let (d, right) = ([dx / run, dy / run], [dy / run, -dx / run]);
    let tread = run / (risers - 1) as f64;
    let prof = profile(risers, rise / risers as f64, tread);
    let m = prof.len();
    let at = |q: P, lat: f64| -> [f64; 3] { [start[0] + d[0] * q[0] + right[0] * lat, start[1] + d[1] * q[0] + right[1] * lat, z0 + q[1]] };
    let mut vertices: Vec<[f64; 3]> = prof.iter().map(|q| at(*q, -width / 2.0)).collect();
    vertices.extend(prof.iter().map(|q| at(*q, width / 2.0)));
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for t in openbim_geom::profile::triangulate(&prof)? {
        // the left side faces left: the profile runs clockwise seen from the right, so counter-clockwise from the left
        let [a, b, c] = if openbim_geom::profile::polygon_area(&[prof[t[0]], prof[t[1]], prof[t[2]]]) < 0.0 { t } else { [t[0], t[2], t[1]] };
        faces.push(vec![a, b, c]);
        faces.push(vec![c + m, b + m, a + m]);
    }
    for i in 0..m {
        let j = (i + 1) % m;
        faces.push(vec![i + m, j + m, j, i]);
    }
    Some(Mesh { vertices, faces })
}

/// The way a stair turns, as Revit's Stair by Component builds it: straight runs joined by an automatic landing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StairShape {
    Straight,
    /// two runs at a right angle, the landing as wide as the stair
    L { left: bool },
    /// two parallel runs going back on each other, with the landing across both
    U { left: bool },
}

impl StairShape {
    pub const ALL: [(StairShape, &'static str); 5] = [
        (StairShape::Straight, "Straight"),
        (StairShape::L { left: true }, "L-shaped, turning left"),
        (StairShape::L { left: false }, "L-shaped, turning right"),
        (StairShape::U { left: true }, "U-shaped, turning left"),
        (StairShape::U { left: false }, "U-shaped, turning right"),
    ];
}

/// The depth of the landing slab, and the clear space between the two runs of a U.
pub const LANDING_THICKNESS_M: f64 = 0.15;
pub const U_GAP_M: f64 = 0.1;

/// Where the parts of a turning stair go.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// risers of the first and of the second run (the landing is the surface between them)
    pub risers: [u32; 2],
    pub run1: [P; 2],
    pub landing: Vec<P>,
    pub run2: [P; 2],
}

/// The layout of an L or U stair of `risers` risers: the runs share the risers as equally as they can, the first one
/// getting the odd one. None for a straight stair, or one with fewer than two risers for a run.
pub fn layout(spec: &StairSpec, shape: StairShape) -> Option<Layout> {
    let (left, u) = match shape { StairShape::Straight => return None, StairShape::L { left } => (left, false), StairShape::U { left } => (left, true) };
    let (dx, dy) = (spec.toward[0] - spec.start[0], spec.toward[1] - spec.start[1]);
    let l = dx.hypot(dy);
    if l < 1e-9 || spec.risers < 4 { return None; }
    let n1 = spec.risers.div_ceil(2);
    let n2 = spec.risers - n1;
    let (d, lf) = ([dx / l, dy / l], [-dy / l, dx / l]);
    let (w, t) = (spec.width_m, spec.tread_m);
    let s = if left { 1.0 } else { -1.0 };
    let e1 = [spec.start[0] + d[0] * (n1 - 1) as f64 * t, spec.start[1] + d[1] * (n1 - 1) as f64 * t];
    let p = |a: f64, lat: f64| [e1[0] + d[0] * a + lf[0] * lat, e1[1] + d[1] * a + lf[1] * lat];
    let (landing, start2, dir2) = if u {
        let off = s * (w + U_GAP_M);
        let (lo, hi) = ((-w / 2.0).min(off - w / 2.0), (w / 2.0).max(off + w / 2.0));
        (vec![p(0.0, lo), p(w, lo), p(w, hi), p(0.0, hi)], p(0.0, off), [-d[0], -d[1]])
    } else {
        (vec![p(0.0, -w / 2.0), p(w, -w / 2.0), p(w, w / 2.0), p(0.0, w / 2.0)], p(w / 2.0, s * w / 2.0), [lf[0] * s, lf[1] * s])
    };
    let end2 = [start2[0] + dir2[0] * (n2 - 1) as f64 * t, start2[1] + dir2[1] * (n2 - 1) as f64 * t];
    Some(Layout { risers: [n1, n2], run1: [spec.start, e1], landing, run2: [start2, end2] })
}

/// What a stair measures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StairFigures {
    pub risers: u32,
    pub treads: u32,
    pub riser_m: f64,
    pub tread_m: f64,
    pub run_m: f64,
    pub rise_m: f64,
}

impl Document {
    /// The level above `level`, if the model has one.
    pub fn level_above(&self, level: ElementId) -> Option<ElementId> {
        let z = self.get(level)?.elevation_m;
        self.elements().filter(|e| e.geometry == Some(Geometry::Level) && e.elevation_m > z + 1e-6).min_by(|a, b| a.elevation_m.total_cmp(&b.elevation_m)).map(|e| e.id)
    }

    /// Places a stair on `level`: the run starts at `spec.start`, goes toward `spec.toward` for as long as its treads.
    pub fn place_stair(&mut self, level: ElementId, spec: &StairSpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let (dx, dy) = (spec.toward[0] - spec.start[0], spec.toward[1] - spec.start[1]);
        let l = dx.hypot(dy);
        if l < 1e-9 || spec.risers < 2 { return Err(EditError::InvalidGeometry(id)); }
        let run = (spec.risers - 1) as f64 * spec.tread_m;
        let end = [spec.start[0] + dx / l * run, spec.start[1] + dy / l * run];
        let n = self.elements.values().filter(|e| e.category == Category::Stair).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Stair, name: format!("Stair {n}"), width_m: spec.width_m, height_m: spec.rise_m, elevation_m: spec.base_offset_m },
            Command::SetCount { id, count: spec.risers },
            Command::SetGeometry { id, geometry: Geometry::Linear { start: spec.start, end, level } },
        ])?;
        Ok(id)
    }

    /// Places an L or U stair: two runs and the landing between them, in one undoable transaction. The second run and the
    /// landing point at the first (as the beams of a beam system point at their system), so that they are selected,
    /// moved, copied and deleted together. `spec.risers` is the total; the landing is the surface after the first run.
    /// A straight shape places a plain stair. Returns the first run.
    pub fn place_turning_stair(&mut self, level: ElementId, spec: &StairSpec, shape: StairShape) -> Result<ElementId, EditError> {
        if shape == StairShape::Straight { return self.place_stair(level, spec); }
        let id = self.next_id();
        let Some(lay) = layout(spec, shape) else { return Err(EditError::InvalidGeometry(id)) };
        let (second, landing) = (ElementId(id.0 + 1), ElementId(id.0 + 2));
        let riser = spec.rise_m / f64::from(spec.risers);
        let climbed = riser * f64::from(lay.risers[0]);
        let n = self.elements.values().filter(|e| e.category == Category::Stair && e.parent.is_none()).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Stair, name: format!("Stair {n}"), width_m: spec.width_m, height_m: riser * f64::from(lay.risers[0]), elevation_m: spec.base_offset_m },
            Command::SetCount { id, count: lay.risers[0] },
            Command::SetGeometry { id, geometry: Geometry::Linear { start: lay.run1[0], end: lay.run1[1], level } },
            Command::Create { id: second, category: Category::Stair, name: format!("Stair {n} (run 2)"), width_m: spec.width_m, height_m: riser * f64::from(lay.risers[1]), elevation_m: spec.base_offset_m + climbed },
            Command::SetCount { id: second, count: lay.risers[1] },
            Command::SetGeometry { id: second, geometry: Geometry::Linear { start: lay.run2[0], end: lay.run2[1], level } },
            Command::SetParent { id: second, parent: Some(id) },
            Command::Create { id: landing, category: Category::Stair, name: format!("Stair {n} (landing)"), width_m: spec.width_m, height_m: LANDING_THICKNESS_M, elevation_m: spec.base_offset_m + climbed },
            Command::SetOutline { id: landing, outline: lay.landing.clone() },
            Command::SetGeometry { id: landing, geometry: Geometry::Area { level } },
            Command::SetParent { id: landing, parent: Some(id) },
        ])?;
        Ok(id)
    }

    /// The runs and the landing of the stair that `id` belongs to, the first run first.
    pub fn stair_parts(&self, id: ElementId) -> Vec<ElementId> {
        let Some(e) = self.get(id).filter(|e| e.category == Category::Stair) else { return Vec::new() };
        let root = e.parent.unwrap_or(id);
        let mut parts = vec![root];
        parts.extend(self.elements().filter(|c| c.category == Category::Stair && c.parent == Some(root)).map(|c| c.id));
        parts
    }

    /// The figures of the stair `id`.
    pub fn stair_figures(&self, id: ElementId) -> Option<StairFigures> {
        let e = self.get(id).filter(|e| e.category == Category::Stair)?;
        let Some(Geometry::Linear { start, end, .. }) = e.geometry else { return None };
        let run = (end[0] - start[0]).hypot(end[1] - start[1]);
        Some(StairFigures { risers: e.count, treads: e.count - 1, riser_m: e.height_m / e.count as f64, tread_m: run / (e.count - 1) as f64, run_m: run, rise_m: e.height_m })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_number_of_risers_keeps_each_under_the_maximum() {
        assert_eq!(risers_for(3.0, 0.175), 18); // 3.0 / 18 = 166.7 mm; 17 would be 176.5 mm
        assert_eq!(risers_for(0.35, 0.175), 2);
        assert_eq!(risers_for(2.8, 0.175), 16); // exactly 175 mm each
    }

    #[test]
    fn the_solid_is_closed_with_outward_faces_and_has_the_volume_of_the_steps() {
        let m = mesh([0.0, 0.0], [0.0, 4.0], 1.0, 1.0, 5, 0.0).unwrap(); // 5 risers of 0.2 over 4 treads of 1.0, going +y
        let key = |p: [f64; 3]| ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64, (p[2] * 1e6).round() as i64);
        let mut seen = std::collections::HashMap::new();
        for f in &m.faces { for k in 0..f.len() { *seen.entry((key(m.vertices[f[k]]), key(m.vertices[f[(k + 1) % f.len()]]))).or_insert(0) += 1; } }
        for ((a, b), n) in &seen { assert_eq!(*n, 1); assert!(seen.contains_key(&(*b, *a)), "an open edge {a:?}->{b:?}"); }
        let mut vol = 0.0;
        for f in &m.faces {
            for k in 1..f.len() - 1 {
                let (a, b, c) = (m.vertices[f[0]], m.vertices[f[k]], m.vertices[f[k + 1]]);
                vol += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0;
            }
        }
        // block under step i (i = 1..=4 treads) is i * 0.2 high and 1.0 long, 1.0 wide; the top riser adds nothing
        assert!((vol - (0.2 + 0.4 + 0.6 + 0.8) * 1.0 * 1.0).abs() < 1e-9, "{vol}");
        // the highest point is the last tread, one riser under the floor above
        assert!((m.vertices.iter().map(|v| v[2]).fold(f64::MIN, f64::max) - 0.8).abs() < 1e-9);
    }

    #[test]
    fn a_stair_is_placed_between_levels_saved_moved_and_mirrored() {
        use crate::{Document, Xf};
        let mut d = Document::spatial_sample();
        let level = ElementId(10);
        let above = d.level_above(level).expect("the sample has a level above");
        let rise = d.get(above).unwrap().elevation_m - d.get(level).unwrap().elevation_m;
        let risers = risers_for(rise, MAX_RISER_M);
        let id = d.place_stair(level, &StairSpec { start: [1.0, 1.0], toward: [1.0, 9.0], width_m: 1.0, rise_m: rise, risers, tread_m: 0.28, base_offset_m: 0.0 }).unwrap();
        let f = d.stair_figures(id).unwrap();
        assert!((f.run_m - (risers - 1) as f64 * 0.28).abs() < 1e-9 && f.riser_m <= MAX_RISER_M + 1e-9 && f.treads == risers - 1, "{f:?}");
        let Some(Geometry::Linear { start, end, .. }) = d.get(id).unwrap().geometry else { panic!() };
        assert_eq!(start, [1.0, 1.0]);
        assert!(end[0] == 1.0 && (end[1] - 1.0 - f.run_m).abs() < 1e-9, "the run goes the way it was pointed: {end:?}");
        // the last tread is one riser under the level above (the foot of the stair is the level)
        let top = d.mesh(id).unwrap().vertices.iter().map(|v| v[2]).fold(f64::MIN, f64::max);
        assert!((top - (d.get(above).unwrap().elevation_m - f.riser_m)).abs() < 1e-9, "{top}");
        // refused: no direction, one riser, a riser too high or a tread too short
        assert!(d.place_stair(level, &StairSpec { toward: [1.0, 1.0], start: [1.0, 1.0], width_m: 1.0, rise_m: 3.0, risers: 10, tread_m: 0.28, base_offset_m: 0.0 }).is_err());
        assert!(d.place_stair(level, &StairSpec { start: [1.0, 1.0], toward: [2.0, 1.0], width_m: 1.0, rise_m: 3.0, risers: 1, tread_m: 0.28, base_offset_m: 0.0 }).is_err());
        assert!(d.place_stair(level, &StairSpec { start: [1.0, 1.0], toward: [2.0, 1.0], width_m: 1.0, rise_m: 3.0, risers: 5, tread_m: 0.28, base_offset_m: 0.0 }).is_err());
        assert!(d.place_stair(level, &StairSpec { start: [1.0, 1.0], toward: [2.0, 1.0], width_m: 1.0, rise_m: 3.0, risers: 18, tread_m: 0.02, base_offset_m: 0.0 }).is_err());
        // saved natively
        let (els, site) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
        let mut back = Document::new();
        back.transact(back.revision(), &crate::native::restore_commands(els, site)).unwrap();
        assert_eq!(back.stair_figures(id), d.stair_figures(id));
        // moved and turned: the same stair
        let ops = d.transform_commands(&[id], Xf::Move([2.0, 0.5])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        let ops = d.transform_commands(&[id], Xf::Rotate([0.0, 0.0], 90f64.to_radians())).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        let g = d.stair_figures(id).unwrap();
        assert!((g.run_m - f.run_m).abs() < 1e-9 && g.risers == f.risers);
        d.undo().unwrap();
        d.undo().unwrap();
        assert!(d.get(id).is_some());
    }

    fn volume(m: &Mesh) -> f64 {
        let mut vol = 0.0;
        for f in &m.faces {
            for k in 1..f.len() - 1 {
                let (a, b, c) = (m.vertices[f[0]], m.vertices[f[k]], m.vertices[f[k + 1]]);
                vol += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0;
            }
        }
        vol
    }

    #[test]
    fn l_and_u_stairs_are_two_runs_and_a_landing_that_meet_and_climb_the_whole_height() {
        use crate::{Document, Xf};
        let spec = StairSpec { start: [1.0, 1.0], toward: [1.0, 9.0], width_m: 1.0, rise_m: 3.0, risers: 18, tread_m: 0.28, base_offset_m: 0.0 };
        for (shape, left, u) in [(StairShape::L { left: true }, true, false), (StairShape::L { left: false }, false, false), (StairShape::U { left: true }, true, true), (StairShape::U { left: false }, false, true)] {
            let mut d = Document::spatial_sample();
            let level = ElementId(10);
            let id = d.place_turning_stair(level, &spec, shape).unwrap();
            let parts = d.stair_parts(id);
            assert_eq!(parts.len(), 3, "{shape:?}");
            let (r1, r2, landing) = (d.get(parts[0]).unwrap().clone(), d.get(parts[1]).unwrap().clone(), d.get(parts[2]).unwrap().clone());
            assert_eq!((r1.count, r2.count), (9, 9));
            assert_eq!((r2.parent, landing.parent), (Some(id), Some(id)));
            // every riser is 3.0 / 18, and the second run starts where the first stopped
            assert!((r1.height_m / 9.0 - 3.0 / 18.0).abs() < 1e-12 && (r2.height_m / 9.0 - 3.0 / 18.0).abs() < 1e-12);
            assert!((r2.elevation_m - 1.5).abs() < 1e-12 && (landing.elevation_m - 1.5).abs() < 1e-12);
            // the top of the second run is one riser under the level above, and the landing top is the first run's top
            let top = |i: ElementId| d.mesh(i).unwrap().vertices.iter().map(|v| v[2]).fold(f64::MIN, f64::max);
            let level_z = d.get(level).unwrap().elevation_m;
            assert!((top(parts[1]) - (level_z + 3.0 - 3.0 / 18.0)).abs() < 1e-9, "{shape:?}");
            assert!((top(parts[2]) - (level_z + 1.5)).abs() < 1e-9);
            assert!((top(parts[0]) - (level_z + 1.5 - 3.0 / 18.0)).abs() < 1e-9);
            // the landing is a closed, outward-wound slab; its area is a square (L) or the width of both runs (U)
            let lm = d.mesh(parts[2]).unwrap();
            let area = openbim_geom::profile::polygon_area(&landing.outline).abs();
            assert!((area - if u { 1.0 * (1.0 * 2.0 + U_GAP_M) } else { 1.0 }).abs() < 1e-9, "{area}");
            assert!((volume(&lm) - area * LANDING_THICKNESS_M).abs() < 1e-9);
            // run 1 ends on the landing's edge; run 2 starts on it
            let Some(Geometry::Linear { end: e1, .. }) = r1.geometry else { panic!() };
            let Some(Geometry::Linear { start: s2, end: e2, .. }) = r2.geometry else { panic!() };
            let inside = |p: [f64; 2]| { let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]); for q in &landing.outline { for k in 0..2 { lo[k] = lo[k].min(q[k]); hi[k] = hi[k].max(q[k]); } } (0..2).all(|k| p[k] >= lo[k] - 1e-9 && p[k] <= hi[k] + 1e-9) };
            assert!(inside(e1) && inside(s2), "{shape:?}");
            // the first run goes +y; L goes left (-x) or right (+x), U comes back toward -y beside it
            if u { assert!(e2[1] < s2[1] - 1.0 && (s2[0] - e1[0]).abs() > 1.0 && ((s2[0] < e1[0]) == left)); }
            else { assert!((e2[1] - s2[1]).abs() < 1e-9 && ((e2[0] < s2[0]) == left)); }
            // the two runs do not touch each other in plan
            let (m1, m2) = (d.mesh(parts[0]).unwrap(), d.mesh(parts[1]).unwrap());
            let flat = |m: &Mesh| { let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]); for v in &m.vertices { for k in 0..2 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } } (lo, hi) };
            let ((a, b), (c, e)) = (flat(&m1), flat(&m2));
            assert!(b[0] <= c[0] + 1e-9 || e[0] <= a[0] + 1e-9 || b[1] <= c[1] + 1e-9 || e[1] <= a[1] + 1e-9, "the runs overlap in plan: {shape:?}");
            // one undo takes the whole stair away
            let mut probe = d.clone();
            probe.undo().unwrap();
            assert!(parts.iter().all(|p| probe.get(*p).is_none()), "{shape:?}");
            // moved by selecting only the first run: the others go with it
            let ops = d.transform_commands(&[id], Xf::Move([2.0, 0.5])).unwrap();
            d.transact(d.revision(), &ops).unwrap();
            assert_eq!(d.stair_parts(id), parts);
            let Some(Geometry::Linear { start, .. }) = d.get(parts[1]).unwrap().geometry else { panic!() };
            assert!((start[0] - s2[0] - 2.0).abs() < 1e-9 && (start[1] - s2[1] - 0.5).abs() < 1e-9);
            assert!((d.get(parts[2]).unwrap().outline[0][0] - landing.outline[0][0] - 2.0).abs() < 1e-9);
            // mirrored: still three parts
            let ops = d.transform_commands(&[id], Xf::Mirror([0.0, 0.0], [0.0, 1.0])).unwrap();
            d.transact(d.revision(), &ops).unwrap();
            assert_eq!(d.stair_parts(id).len(), 3);
            // saved natively and read back
            let (els, site) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
            let mut back = Document::new();
            back.transact(back.revision(), &crate::native::restore_commands(els, site)).unwrap();
            assert_eq!(back.stair_parts(id), parts);
            assert_eq!(back.mesh(parts[2]).unwrap().vertices, d.mesh(parts[2]).unwrap().vertices);
            // copied: a second stair of three parts
            let (ops, _) = d.copy_commands(&[id], Xf::Move([5.0, 0.0])).unwrap();
            let before = d.elements().filter(|e| e.category == Category::Stair).count();
            d.transact(d.revision(), &ops).unwrap();
            assert_eq!(d.elements().filter(|e| e.category == Category::Stair).count(), before + 3);
            // deleting the first run takes the whole stair
            let ops = d.delete_commands(&[id]).unwrap();
            d.transact(d.revision(), &ops).unwrap();
            assert!(parts.iter().all(|p| d.get(*p).is_none()), "{shape:?}");
        }
        // too few risers to make two runs, or no direction: refused, and nothing is left behind
        let mut d = Document::spatial_sample();
        let before = d.elements().count();
        assert!(d.place_turning_stair(ElementId(10), &StairSpec { risers: 3, ..spec.clone() }, StairShape::L { left: true }).is_err());
        assert!(d.place_turning_stair(ElementId(10), &StairSpec { toward: spec.start, ..spec.clone() }, StairShape::U { left: true }).is_err());
        assert_eq!(d.elements().count(), before);
        // a straight shape is the plain stair
        let id = d.place_turning_stair(ElementId(10), &spec, StairShape::Straight).unwrap();
        assert_eq!(d.stair_parts(id), vec![id]);
    }
}
