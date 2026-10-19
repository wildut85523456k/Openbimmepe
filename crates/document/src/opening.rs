//! Openings, as Revit's Opening panel makes them. A **wall opening** is a rectangle cut in a wall, from a distance
//! along the wall to another and from a sill height to a head height (`Geometry::InWall`: the middle of the opening
//! along the wall, `width_m` wide, `height_m` high, its sill `elevation_m` above the base of the wall). A **shaft** is
//! an outline in plan between a base and a top level (`Geometry::Shaft`) that cuts every floor, foundation slab and
//! ceiling or roof it passes through. A **vertical opening** (an opening with an outline in plan that points at its host) cuts only the one floor, roof or ceiling it is made in.
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

    /// The heights a floor, foundation slab, ceiling or roof takes: from its underside to its top (for a roof, from the
    /// lowest underside to the highest top).
    fn slab_range(&self, id: ElementId) -> Option<(f64, f64)> {
        let e = self.get(id).filter(|e| e.category == Category::Slab)?;
        let Some(Geometry::Area { level }) = e.geometry else { return None };
        if e.is_roof() {
            let slope = e.slope_ratio.unwrap_or(0.0);
            let z0 = self.get(level)?.elevation_m + e.elevation_m;
            let faces = crate::roof::top_faces(&e.outline, &e.slope_edges, slope, z0)?;
            let zs = faces.iter().flatten().map(|p| p[2]);
            return Some((zs.clone().fold(f64::MAX, f64::min) - e.height_m * (1.0 + slope * slope).sqrt(), zs.fold(f64::MIN, f64::max)));
        }
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

    /// A vertical opening: a hole cut straight through the one floor, roof or ceiling `host`, whatever else is at its height.
    pub fn place_vertical_opening(&mut self, host: ElementId, outline: &[P]) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let level = match self.get(host) {
            Some(h) if h.category == Category::Slab => match h.geometry { Some(Geometry::Area { level }) => level, _ => return Err(EditError::InvalidElement) },
            _ => return Err(EditError::InvalidElement),
        };
        let n = self.elements.values().filter(|e| Self::is_vertical_opening(e)).count() + 1;
        self.transact(self.revision, &[
            Command::Create { id, category: Category::Opening, name: format!("Vertical Opening {n}"), width_m: 1.0, height_m: 1.0, elevation_m: 0.0 },
            Command::SetOutline { id, outline: outline.to_vec() },
            Command::SetGeometry { id, geometry: Geometry::Area { level } },
            Command::SetParent { id, parent: Some(host) },
        ])?;
        Ok(id)
    }

    /// Whether `e` is a vertical opening (an opening with an outline in plan on a level, pointing at its host).
    pub fn is_vertical_opening(e: &crate::Element) -> bool { e.category == Category::Opening && matches!(e.geometry, Some(Geometry::Area { .. })) }

    /// Everything that cuts the floor, roof or ceiling `id`: the shafts through it and the vertical openings made in it
    /// that lie over it, in element order.
    pub fn cuts_of(&self, id: ElementId) -> Vec<ElementId> {
        let mut cuts = self.shafts_through(id);
        let outline = self.get(id).map(|e| e.outline.clone()).unwrap_or_default();
        cuts.extend(self.elements().filter(|o| Self::is_vertical_opening(o) && o.parent == Some(id) && !Self::clip(&outline, &o.outline).is_empty()).map(|o| o.id));
        cuts.sort();
        cuts
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
        self.cuts_of(id).into_iter().flat_map(|s| {
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
        let shafts = self.cuts_of(id);
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

    fn volume(m: &crate::Mesh) -> f64 {
        let mut vol = 0.0;
        for f in &m.faces {
            for k in 1..f.len() - 1 {
                let (a, b, c) = (m.vertices[f[0]], m.vertices[f[k]], m.vertices[f[k + 1]]);
                vol += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0;
            }
        }
        vol
    }

    fn open_edges(m: &crate::Mesh) -> Vec<String> {
        let key = |p: [f64; 3]| ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64, (p[2] * 1e6).round() as i64);
        let mut seen = std::collections::HashMap::new();
        for f in &m.faces { for k in 0..f.len() { *seen.entry((key(m.vertices[f[k]]), key(m.vertices[f[(k + 1) % f.len()]]))).or_insert(0) += 1; } }
        seen.iter().filter(|((a, b), n)| **n != 1 || !seen.contains_key(&(*b, *a))).map(|((a, b), n)| format!("{a:?}->{b:?} x{n}")).collect()
    }

    fn closed(m: &crate::Mesh) -> bool {
        let key = |p: [f64; 3]| ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64, (p[2] * 1e6).round() as i64);
        let mut seen = std::collections::HashMap::new();
        for f in &m.faces { for k in 0..f.len() { *seen.entry((key(m.vertices[f[k]]), key(m.vertices[f[(k + 1) % f.len()]]))).or_insert(0) += 1; } }
        seen.iter().all(|((a, b), n)| *n == 1 && seen.contains_key(&(*b, *a)))
    }

    #[test]
    fn a_vertical_opening_cuts_the_one_floor_it_is_made_in_and_goes_with_it() {
        let (mut d, _l0, _l1, f0, f1) = building();
        let o = d.place_vertical_opening(f1, &square(4.0, 4.0, 2.0)).unwrap();
        assert_eq!(d.get(o).unwrap().parent, Some(f1));
        // the floor below, at the same place, is not cut
        assert_eq!(d.cuts_of(f1), vec![o]);
        assert!(d.cuts_of(f0).is_empty());
        let m = d.mesh(f1).unwrap();
        assert!((volume(&m) - 96.0 * 0.2).abs() < 1e-9, "{}", volume(&m));
        assert!((volume(&d.mesh(f0).unwrap()) - 100.0 * 0.2).abs() < 1e-9);
        // the opening is not a body
        assert!(d.mesh(o).is_none());
        // the shapes and the voids the exports and views take
        let shapes = d.cut_shapes(f1);
        assert_eq!((shapes.len(), shapes[0].holes.len()), (1, 1));
        assert_eq!(d.shaft_cuts(f1).len(), 1);
        assert_eq!(d.shaft_cuts(f1)[0].0, "Vertical Opening 1");
        // an opening over nothing of its host cuts nothing; a host that is not a floor, roof or ceiling is refused
        let far = d.place_vertical_opening(f1, &square(30.0, 30.0, 1.0)).unwrap();
        assert_eq!(d.cuts_of(f1), vec![o]);
        assert!(d.place_vertical_opening(ElementId(1), &square(1.0, 1.0, 1.0)).is_err());
        assert!(d.place_vertical_opening(f1, &[[0.0, 0.0], [1.0, 1.0]]).is_err());
        let _ = far;
        // one undo takes it away; the floor is whole again
        let mut probe = d.clone();
        probe.undo().unwrap();
        probe.undo().unwrap();
        assert!(probe.get(o).is_none() && (volume(&probe.mesh(f1).unwrap()) - 20.0).abs() < 1e-9);
        // moved with its floor; saved and read back; copied; deleted with the floor
        let ops = d.transform_commands(&[f1], crate::Xf::Move([1.0, 0.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert_eq!(d.get(o).unwrap().outline[0][0], 5.0);
        let (els, site) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
        let mut back = Document::new();
        back.transact(back.revision(), &crate::native::restore_commands(els, site)).unwrap();
        assert_eq!(back.cuts_of(f1), vec![o]);
        let (ops, _) = d.copy_commands(&[f1], crate::Xf::Move([0.0, 20.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert_eq!(d.elements().filter(|e| Document::is_vertical_opening(e)).count(), 4);
        let ops = d.delete_commands(&[f1]).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(d.get(o).is_none() && d.get(far).is_none());
    }

    fn hip_roof(d: &mut Document, level: ElementId) -> ElementId {
        d.place_roof(level, &crate::RoofSpec { outline: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]], thickness_m: 0.2, base_offset_m: 0.0, slope_ratio: 0.5, edges: vec![true; 4], material: crate::Material::Timber }).unwrap()
    }

    #[test]
    fn a_roof_is_cut_by_a_vertical_opening_and_by_a_shaft_keeping_its_slope_and_staying_closed() {
        let (mut d, l0, l1, _f0, _f1) = building();
        let roof = hip_roof(&mut d, l1);
        let whole = d.mesh(roof).unwrap();
        let per_m2 = 0.2 * 1.25f64.sqrt();
        assert!((volume(&whole) - 60.0 * per_m2).abs() < 1e-9 && closed(&whole));
        // a hole in one plane
        let o = d.place_vertical_opening(roof, &square(1.0, 1.0, 1.0)).unwrap();
        let m = d.mesh(roof).unwrap();
        assert!((volume(&m) - 59.0 * per_m2).abs() < 1e-6, "{}", volume(&m));
        assert!(closed(&m), "a hole inside a plane");
        // the walls of the hole are vertical and go the thickness of the roof
        assert!(m.faces.iter().any(|f| f.len() == 4 && f.iter().all(|&i| (m.vertices[i][0] - 1.0).abs() < 1e-9 || (m.vertices[i][0] - 2.0).abs() < 1e-9) && f.iter().map(|&i| m.vertices[i][2]).fold(f64::MIN, f64::max) - f.iter().map(|&i| m.vertices[i][2]).fold(f64::MAX, f64::min) > 0.2));
        // a hole across the ridge (3 m up, 3 m from the long edges), and one over a corner of the eaves
        let across = d.place_vertical_opening(roof, &[[4.0, 2.0], [6.0, 2.0], [6.0, 4.0], [4.0, 4.0]]).unwrap();
        let corner = d.place_vertical_opening(roof, &square(-1.0, -1.0, 1.5)).unwrap();
        let m = d.mesh(roof).unwrap();
        let cut_area = 1.0 + 4.0 + 0.25;
        assert!((volume(&m) - (60.0 - cut_area) * per_m2).abs() < 1e-6, "{}", volume(&m));
        assert!(closed(&m), "holes across the ridge and over the eaves: {:#?}", open_edges(&m));
        // the cut keeps the slope: the highest point is still the ridge (1.5 up at 0.5), minus nothing
        let top = m.vertices.iter().map(|v| v[2]).fold(f64::MIN, f64::max);
        assert!((top - (3.0 + 1.5)).abs() < 1e-9, "{top}");
        let _ = (across, corner, o);
        // a shaft passes through it too: from the ground to the level above the roof's level
        let (mut d, l0b, l1b, _, _) = building();
        let _ = (l0, l1);
        let roof = hip_roof(&mut d, l1b);
        let s = d.place_shaft(l0b, &ShaftSpec { outline: square(1.0, 1.0, 1.0), top: l1b, base_offset_m: 0.0, top_offset_m: 0.5 }).unwrap();
        assert_eq!(d.cuts_of(roof), vec![s]);
        let m = d.mesh(roof).unwrap();
        assert!((volume(&m) - 59.0 * per_m2).abs() < 1e-6 && closed(&m));
        // a shaft that ends under the roof does not
        let (mut d, l0c, l1c, _, _) = building();
        let roof = hip_roof(&mut d, l1c);
        d.place_shaft(l0c, &ShaftSpec { outline: square(1.0, 1.0, 1.0), top: l1c, base_offset_m: 0.0, top_offset_m: -0.5 }).unwrap();
        assert!(d.cuts_of(roof).is_empty());
    }
}
