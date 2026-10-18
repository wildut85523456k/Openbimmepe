//! Openings, as Revit's Opening panel makes them. A **wall opening** is a rectangle cut in a wall, from a distance
//! along the wall to another and from a sill height to a head height (`Geometry::InWall`: the middle of the opening
//! along the wall, `width_m` wide, `height_m` high, its sill `elevation_m` above the base of the wall). A **shaft** is
//! an outline in plan between a base and a top level (`Geometry::Shaft`) that cuts every floor, foundation slab and
//! ceiling it passes through. Roofs are not cut yet.
use crate::{Category, Command, Document, EditError, ElementId, Geometry, RoomShape, SlabKind};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;

type P = [f64; 2];

/// What a shaft is: its outline, the level it ends at and how far above or below each level it starts and ends.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaftSpec {
    pub outline: Vec<P>,
    pub top: ElementId,
    pub base_offset_m: f64,
    pub top_offset_m: f64,
}

/// A slab below this much of a shaft's height, or a cut with less area than this, is not a cut.
const EPS: f64 = 1e-6;

fn area(r: &[P]) -> f64 { (0..r.len()).map(|i| { let (a, b) = (r[i], r[(i + 1) % r.len()]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>() / 2.0 }

fn ccw(mut r: Vec<P>) -> Vec<P> { if area(&r) < 0.0 { r.reverse(); } r }

fn shape_of(rings: &[Vec<P>]) -> RoomShape {
    RoomShape { outline: ccw(rings[0].clone()), holes: rings[1..].iter().map(|h| { let mut h = h.clone(); if area(&h) > 0.0 { h.reverse(); } h }).collect() }
}

impl Document {
    /// A rectangular opening in the wall `host`, from `from_m` to `to_m` along it and from `sill_m` to `head_m` above its base.
    pub fn place_wall_opening(&mut self, host: ElementId, from_m: f64, to_m: f64, sill_m: f64, head_m: f64) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let (a, b) = (from_m.min(to_m), from_m.max(to_m));
        let n = self.elements.values().filter(|e| e.category == Category::Opening && matches!(e.geometry, Some(Geometry::InWall { .. }))).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Opening, name: format!("Wall Opening {n}"), width_m: b - a, height_m: head_m - sill_m, elevation_m: sill_m },
            Command::SetGeometry { id, geometry: Geometry::InWall { host, along_m: (a + b) / 2.0 } },
        ])?;
        Ok(id)
    }

    /// A shaft from `level` (plus its base offset) up to `spec.top` (plus its top offset).
    pub fn place_shaft(&mut self, level: ElementId, spec: &ShaftSpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let n = self.elements.values().filter(|e| matches!(e.geometry, Some(Geometry::Shaft { .. }))).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Opening, name: format!("Shaft Opening {n}"), width_m: 1.0, height_m: 1.0, elevation_m: 0.0 },
            Command::SetOutline { id, outline: spec.outline.clone() },
            Command::SetGeometry { id, geometry: Geometry::Shaft { level, top: spec.top, base_offset_m: spec.base_offset_m, top_offset_m: spec.top_offset_m } },
        ])?;
        Ok(id)
    }

    /// The heights a shaft runs between.
    pub fn shaft_range(&self, id: ElementId) -> Option<(f64, f64)> {
        let Some(Geometry::Shaft { level, top, base_offset_m, top_offset_m }) = self.get(id)?.geometry else { return None };
        Some((self.get(level)?.elevation_m + base_offset_m, self.get(top)?.elevation_m + top_offset_m))
    }

    /// The heights a floor, foundation slab or ceiling takes: from its underside to its top.
    fn slab_range(&self, id: ElementId) -> Option<(f64, f64)> {
        let e = self.get(id).filter(|e| e.category == Category::Slab && !e.is_roof())?;
        let Some(Geometry::Area { level }) = e.geometry else { return None };
        let top = self.get(level)?.elevation_m + e.elevation_m + if e.slab_kind == SlabKind::Ceiling { e.height_m } else { 0.0 };
        Some((top - e.height_m, top))
    }

    /// The shafts that cut the slab `id`, in element order: they pass through its thickness and over its outline.
    pub fn shafts_through(&self, id: ElementId) -> Vec<ElementId> {
        let Some((lo, hi)) = self.slab_range(id) else { return Vec::new() };
        let outline = &self.get(id).map(|e| e.outline.clone()).unwrap_or_default();
        self.elements().filter(|s| matches!(s.geometry, Some(Geometry::Shaft { .. }))).filter(|s| {
            self.shaft_range(s.id).is_some_and(|(b, t)| lo < t - EPS && hi > b + EPS)
                && !Self::clip(outline, &s.outline).is_empty()
        }).map(|s| s.id).collect()
    }

    /// Where `cutter` lies over `outline`, as rings.
    fn clip(outline: &[P], cutter: &[P]) -> Vec<Vec<P>> {
        let subject: Vec<P> = outline.to_vec();
        let clip: Vec<Vec<P>> = vec![cutter.to_vec()];
        subject.overlay(&clip, OverlayRule::Intersect, FillRule::NonZero).into_iter()
            .map(|shape| ccw(shape.into_iter().next().unwrap_or_default())).filter(|r| r.len() >= 3 && area(r) > EPS).collect()
    }

    /// For each shaft that cuts the slab `id`, its name and the part of it that lies over the slab.
    pub fn shaft_cuts(&self, id: ElementId) -> Vec<(String, Vec<P>)> {
        let outline = self.get(id).map(|e| e.outline.clone()).unwrap_or_default();
        self.shafts_through(id).into_iter().flat_map(|s| {
            let e = self.get(s);
            let name = e.map(|e| e.name.clone()).unwrap_or_default();
            let ring = e.map(|e| e.outline.clone()).unwrap_or_default();
            Self::clip(&outline, &ring).into_iter().map(move |r| (name.clone(), r))
        }).collect()
    }

    /// What is left of the slab `id` once its shafts are cut: the pieces, each with the holes inside it. A slab that no
    /// shaft cuts is one piece, its outline.
    pub fn cut_shapes(&self, id: ElementId) -> Vec<RoomShape> {
        let Some(e) = self.get(id) else { return Vec::new() };
        let shafts = self.shafts_through(id);
        if shafts.is_empty() { return vec![RoomShape { outline: ccw(e.outline.clone()), holes: Vec::new() }]; }
        let subject: Vec<P> = e.outline.clone();
        let clip: Vec<Vec<P>> = shafts.iter().filter_map(|s| self.get(*s)).map(|s| s.outline.clone()).collect();
        let mut shapes: Vec<RoomShape> = subject.overlay(&clip, OverlayRule::Difference, FillRule::NonZero).into_iter()
            .filter(|rings| !rings.is_empty() && rings[0].len() >= 3).map(|rings| shape_of(&rings)).filter(|s| s.area() > EPS).collect();
        shapes.sort_by(|a, b| b.area().total_cmp(&a.area()));
        shapes
    }

    /// The walls' openings: for each wall opening, the wall it is in and where (offset along the wall, width, sill, height).
    pub fn wall_openings(&self) -> Vec<(ElementId, ElementId, [f64; 4])> {
        self.elements().filter(|e| e.category == Category::Opening).filter_map(|e| match e.geometry {
            Some(Geometry::InWall { host, along_m }) => Some((e.id, host, [along_m - e.width_m / 2.0, e.width_m, e.elevation_m, e.height_m])),
            _ => None,
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Material, SlabSpec};

    fn building() -> (Document, ElementId, ElementId, ElementId, ElementId) {
        let mut d = Document::new();
        let (l0, l1) = (ElementId(1), ElementId(2));
        d.transact(0, &[
            Command::Create { id: l0, category: Category::Level, name: "Level 0".into(), width_m: 1.0, height_m: 1.0, elevation_m: 0.0 },
            Command::SetGeometry { id: l0, geometry: Geometry::Level },
            Command::Create { id: l1, category: Category::Level, name: "Level 1".into(), width_m: 1.0, height_m: 1.0, elevation_m: 3.0 },
            Command::SetGeometry { id: l1, geometry: Geometry::Level },
        ]).unwrap();
        let floor = |d: &mut Document, level| d.place_slab(level, &SlabSpec { outline: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]], thickness_m: 0.2, top_offset_m: 0.0, kind: SlabKind::Floor, material: Material::Concrete }).unwrap();
        let f0 = floor(&mut d, l0);
        let f1 = floor(&mut d, l1);
        (d, l0, l1, f0, f1)
    }

    fn square(x: f64, y: f64, s: f64) -> Vec<P> { vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s]] }

    #[test]
    fn a_shaft_cuts_the_floor_at_its_top_but_not_the_one_it_starts_on() {
        let (mut d, l0, l1, f0, f1) = building();
        let s = d.place_shaft(l0, &ShaftSpec { outline: square(4.0, 4.0, 2.0), top: l1, base_offset_m: 0.0, top_offset_m: 0.0 }).unwrap();
        assert_eq!(d.shaft_range(s), Some((0.0, 3.0)));
        // the floor of level 1 has its top at the top of the shaft: the shaft passes through it
        assert_eq!(d.shafts_through(f1), vec![s]);
        // the floor of level 0 has its top at the base of the shaft: the shaft starts on it
        assert!(d.shafts_through(f0).is_empty());
        let shapes = d.cut_shapes(f1);
        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].holes.len(), 1);
        assert!((shapes[0].area() - 96.0).abs() < 1e-9, "{}", shapes[0].area());
        // the mesh has the hole: the volume is the slab's less the shaft's
        let m = d.mesh(f1).unwrap();
        let vol: f64 = m.faces.iter().map(|f| (1..f.len() - 1).map(|k| { let (a, b, c) = (m.vertices[f[0]], m.vertices[f[k]], m.vertices[f[k + 1]]); (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0 }).sum::<f64>()).sum();
        assert!((vol - 96.0 * 0.2).abs() < 1e-6, "{vol}");
    }

    #[test]
    fn a_shaft_over_the_edge_of_a_floor_notches_it() {
        let (mut d, l0, l1, _f0, f1) = building();
        d.place_shaft(l0, &ShaftSpec { outline: square(9.0, 4.0, 2.0), top: l1, base_offset_m: 0.0, top_offset_m: 0.0 }).unwrap();
        let shapes = d.cut_shapes(f1);
        assert_eq!(shapes.len(), 1);
        assert!(shapes[0].holes.is_empty());
        assert!((shapes[0].area() - 98.0).abs() < 1e-9, "{}", shapes[0].area());
        let cuts = d.shaft_cuts(f1);
        assert_eq!(cuts.len(), 1);
        assert!((area(&cuts[0].1) - 2.0).abs() < 1e-9);
        // a shaft that does not reach the floor's outline cuts nothing
        d.place_shaft(l0, &ShaftSpec { outline: square(20.0, 20.0, 1.0), top: l1, base_offset_m: 0.0, top_offset_m: 0.0 }).unwrap();
        assert_eq!(d.shafts_through(f1).len(), 1);
    }

    #[test]
    fn a_shaft_that_stops_short_of_the_floor_does_not_cut_it() {
        let (mut d, l0, l1, _f0, f1) = building();
        d.place_shaft(l0, &ShaftSpec { outline: square(4.0, 4.0, 2.0), top: l1, base_offset_m: 0.0, top_offset_m: -0.3 }).unwrap();
        assert!(d.shafts_through(f1).is_empty());
        assert_eq!(d.cut_shapes(f1)[0].holes.len(), 0);
    }

    #[test]
    fn a_shaft_that_runs_below_the_ground_floor_cuts_it_too() {
        let (mut d, l0, l1, f0, _) = building();
        d.place_shaft(l0, &ShaftSpec { outline: square(4.0, 4.0, 2.0), top: l1, base_offset_m: -1.0, top_offset_m: 0.0 }).unwrap();
        assert_eq!(d.shafts_through(f0).len(), 1);
    }

    #[test]
    fn a_wall_opening_is_a_rectangle_in_its_wall_and_follows_it() {
        let mut d = Document::new();
        let l0 = ElementId(1);
        d.transact(0, &[
            Command::Create { id: l0, category: Category::Level, name: "Level 0".into(), width_m: 1.0, height_m: 1.0, elevation_m: 0.0 },
            Command::SetGeometry { id: l0, geometry: Geometry::Level },
        ]).unwrap();
        let w = d.place_wall(l0, [0.0, 0.0], [6.0, 0.0], 0.2, 3.0, 0.0).unwrap();
        let o = d.place_wall_opening(w, 4.0, 1.0, 0.9, 2.1).unwrap();
        let e = d.get(o).unwrap();
        assert_eq!((e.category, e.width_m, e.height_m, e.elevation_m), (Category::Opening, 3.0, 1.2000000000000002, 0.9));
        assert!(matches!(e.geometry, Some(Geometry::InWall { host, along_m }) if host == w && (along_m - 2.5).abs() < 1e-9));
        // outside the wall, or above it, is not an opening in it
        assert!(d.place_wall_opening(w, 5.0, 7.0, 0.0, 1.0).is_err());
        assert!(d.place_wall_opening(w, 1.0, 2.0, 0.0, 3.5).is_err());
        assert!(d.place_wall_opening(w, 1.0, 1.0, 0.0, 1.0).is_err());
        assert_eq!(d.wall_openings().len(), 1);
        // deleting the wall deletes it
        let ops = d.delete_commands(&[w]).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(d.get(o).is_none());
    }
}
