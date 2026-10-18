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
}
