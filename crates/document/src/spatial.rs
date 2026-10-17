//! Metre-based definitions shared by plan and 3D views. Meshes are derived,
//! never a second mutable copy of the document's parameters.
use super::{Category, Document, EditError, Element, ElementId};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Geometry {
    Level,
    Grid { start: [f64; 2], end: [f64; 2] },
    Linear { start: [f64; 2], end: [f64; 2], level: ElementId },
    /// a point on `level` with something standing from there up to the level `top` (a column)
    Point { at: [f64; 2], level: ElementId, top: ElementId },
    /// a slab lies on `level`; its outline is the element's
    Area { level: ElementId },
    /// a pad footing centred on `at`, `thickness_m` thick
    Pad { at: [f64; 2], level: ElementId, thickness_m: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<[f64; 3]>,
    pub faces: Vec<Vec<usize>>,
}

fn valid_axis(start: [f64; 2], end: [f64; 2]) -> bool {
    start.into_iter().chain(end).all(|n| n.is_finite() && n.abs() <= 1e6)
        && (end[0] - start[0]).hypot(end[1] - start[1]) > 1e-6
}

/// A simple polygon: 3 to 1000 finite corners, some area, and no edge crossing another.
fn valid_ring(r: &[[f64; 2]]) -> bool {
    let n = r.len();
    if !(3..=1000).contains(&n) || r.iter().any(|p| !p[0].is_finite() || !p[1].is_finite() || p[0].abs() > 1e6 || p[1].abs() > 1e6) {
        return false;
    }
    if openbim_geom::profile::polygon_area(r).abs() < 1e-6 {
        return false;
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let on = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| {
        cross(a, b, p).abs() < 1e-12 && p[0] >= a[0].min(b[0]) - 1e-12 && p[0] <= a[0].max(b[0]) + 1e-12 && p[1] >= a[1].min(b[1]) - 1e-12 && p[1] <= a[1].max(b[1]) + 1e-12
    };
    // non-adjacent edges may neither cross nor touch
    let hit = |a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]| {
        let (d1, d2, d3, d4) = (cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d));
        (d1 * d2 < 0.0 && d3 * d4 < 0.0) || on(c, d, a) || on(c, d, b) || on(a, b, c) || on(a, b, d)
    };
    for i in 0..n {
        for j in (i + 2)..n {
            if i == 0 && j == n - 1 { continue; }
            if hit(r[i], r[(i + 1) % n], r[j], r[(j + 1) % n]) { return false; }
        }
    }
    true
}

pub(super) fn validate(elements: &BTreeMap<ElementId, Element>) -> Result<(), EditError> {
    for el in elements.values() {
        let valid = match &el.geometry {
            None => !matches!(el.category, Category::Level | Category::Grid | Category::Column | Category::Slab | Category::Footing),
            Some(Geometry::Level) => el.category == Category::Level && el.elevation_m.abs() <= 1e6,
            Some(Geometry::Grid { start, end }) => el.category == Category::Grid && valid_axis(*start, *end),
            Some(Geometry::Pad { at, level, thickness_m }) => {
                el.category == Category::Footing && at.iter().all(|v| v.is_finite() && v.abs() <= 1e6)
                    && thickness_m.is_finite() && *thickness_m > 0.0 && *thickness_m <= 5.0
                    && elements.get(level).is_some_and(|l| l.category == Category::Level && l.geometry == Some(Geometry::Level))
            }
            Some(Geometry::Area { level }) => {
                el.category == Category::Slab && valid_ring(&el.outline)
                    && elements.get(level).is_some_and(|l| l.category == Category::Level && l.geometry == Some(Geometry::Level))
                    && el.elevation_m.abs() <= 1e6
            }
            Some(Geometry::Point { at, level, top }) => {
                let lv = |id: &ElementId| elements.get(id).filter(|l| l.category == Category::Level && l.geometry == Some(Geometry::Level));
                el.category == Category::Column
                    && at.iter().all(|n| n.is_finite() && n.abs() <= 1e6)
                    && el.width_m <= 1e6 && el.height_m <= 1e6
                    && match (lv(level), lv(top)) {
                        (Some(a), Some(b)) => b.elevation_m - (a.elevation_m + el.elevation_m) > 1e-6 && el.elevation_m.abs() <= 1e6,
                        _ => false,
                    }
            }
            Some(Geometry::Linear { start, end, level }) => {
                let datum = elements.get(level).filter(|l| l.category == Category::Level
                    && l.geometry == Some(Geometry::Level));
                matches!(el.category, Category::Wall | Category::Beam | Category::Duct | Category::Pipe)
                    && valid_axis(*start, *end) && datum.is_some()
                    && el.width_m <= 1e6 && el.height_m <= 1e6
                    && (el.category != Category::Pipe || (el.width_m - el.height_m).abs() <= 1e-9)
                    && {
                        let z = datum.map_or(0.0, |l| l.elevation_m) + el.elevation_m;
                        let end_z = z + (end[0] - start[0]).hypot(end[1] - start[1]) * el.slope_ratio.unwrap_or(0.0);
                        z.abs() <= 1e6 && end_z.is_finite() && end_z.abs() <= 1e6
                    }
            }
        };
        if !valid { return Err(EditError::InvalidGeometry(el.id)); }
    }
    Ok(())
}

impl Document {
    pub fn mesh(&self, id: ElementId) -> Option<Mesh> {
        let el = self.get(id)?;
        if let Some(Geometry::Point { at, level, top }) = el.geometry {
            // the section stood on its point from the foot to the top level; the end faces of an I-section are
            // concave polygons, which a plan does not mind (the 3D view meshes columns itself)
            let (z0, z1) = (self.get(level)?.elevation_m + el.elevation_m, self.get(top)?.elevation_m);
            let section: Vec<[f64; 2]> = el.profile().outline().into_iter().map(|p| [at[0] + p[0], at[1] + p[1]]).collect();
            let n = section.len();
            let mut vertices: Vec<[f64; 3]> = section.iter().map(|p| [p[0], p[1], z0]).collect();
            vertices.extend(section.iter().map(|p| [p[0], p[1], z1]));
            let mut faces: Vec<Vec<usize>> = vec![(0..n).rev().collect(), (n..2 * n).collect()];
            for i in 0..n { faces.push(vec![i, (i + 1) % n, (i + 1) % n + n, i + n]); }
            return Some(Mesh { vertices, faces });
        }
        if let Some(Geometry::Pad { at, level, thickness_m }) = el.geometry {
            let top = self.get(level)?.elevation_m + el.elevation_m;
            let (hx, hy) = (el.width_m / 2.0, el.height_m / 2.0);
            let ring = [[at[0] - hx, at[1] - hy], [at[0] + hx, at[1] - hy], [at[0] + hx, at[1] + hy], [at[0] - hx, at[1] + hy]];
            let mut vertices: Vec<[f64; 3]> = ring.iter().map(|p| [p[0], p[1], top - thickness_m]).collect();
            vertices.extend(ring.iter().map(|p| [p[0], p[1], top]));
            let mut faces = vec![vec![3, 2, 1, 0], vec![4, 5, 6, 7]];
            for i in 0..4 { faces.push(vec![i, (i + 1) % 4, (i + 1) % 4 + 4, i + 4]); }
            return Some(Mesh { vertices, faces });
        }
        if let Some(Geometry::Area { level }) = el.geometry {
            // the slab hangs from its top, which is `elevation_m` above the level
            let ring = &el.outline;
            let n = ring.len();
            let top = self.get(level)?.elevation_m + el.elevation_m;
            let bottom = top - el.height_m;
            let mut vertices: Vec<[f64; 3]> = ring.iter().map(|p| [p[0], p[1], bottom]).collect();
            vertices.extend(ring.iter().map(|p| [p[0], p[1], top]));
            let ccw = openbim_geom::profile::polygon_area(ring) > 0.0;
            let mut faces: Vec<Vec<usize>> = Vec::new();
            for t in openbim_geom::profile::triangulate(ring)? {
                // each triangle is wound on its own: counter-clockwise on top, clockwise underneath
                let [a, b, c] = if openbim_geom::profile::polygon_area(&[ring[t[0]], ring[t[1]], ring[t[2]]]) > 0.0 { t } else { [t[0], t[2], t[1]] };
                faces.push(vec![a + n, b + n, c + n]);
                faces.push(vec![c, b, a]);
            }
            for i in 0..n {
                let j = (i + 1) % n;
                faces.push(if ccw { vec![i, j, j + n, i + n] } else { vec![j, i, i + n, j + n] });
            }
            return Some(Mesh { vertices, faces });
        }
        let Geometry::Linear { start, end, level } = el.geometry.as_ref()? else { return None; };
        let z = self.get(*level)?.elevation_m + el.elevation_m;
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let length = dx.hypot(dy);
        let slope = el.slope_ratio.unwrap_or(0.0);
        let norm = (1.0 + slope * slope).sqrt();
        let across = [-dy / length, dx / length, 0.0];
        let up = [-dx / length * slope / norm, -dy / length * slope / norm, 1.0 / norm];
        let center_z = z + if el.category == Category::Wall { el.height_m / 2.0 } else { 0.0 };
        let centers = [[start[0], start[1], center_z], [end[0], end[1], center_z + length * slope]];
        let section: Vec<[f64; 2]> = if el.category == Category::Beam {
            // the outline of an I-section is not convex: its end faces are polygons that are fine for a plan, but
            // the 3D view meshes beams itself (`openbim_view::beam_mesh`)
            el.profile().outline()
        } else if el.category == Category::Pipe {
            (0..16).map(|i| {
                let angle = i as f64 * std::f64::consts::TAU / 16.0;
                [el.width_m / 2.0 * angle.cos(), el.width_m / 2.0 * angle.sin()]
            }).collect()
        } else {
            vec![[-el.width_m / 2.0, -el.height_m / 2.0],
                 [el.width_m / 2.0, -el.height_m / 2.0],
                 [el.width_m / 2.0, el.height_m / 2.0],
                 [-el.width_m / 2.0, el.height_m / 2.0]]
        };
        let n = section.len();
        let mut vertices = Vec::with_capacity(n * 2);
        for center in centers {
            for [a, b] in &section {
                vertices.push(std::array::from_fn(|i| center[i] + across[i] * a + up[i] * b));
            }
        }
        let mut faces = vec![(0..n).rev().collect(), (n..2 * n).collect()];
        for i in 0..n { faces.push(vec![i, (i + 1) % n, (i + 1) % n + n, i + n]); }
        Some(Mesh { vertices, faces })
    }

    /// All spatial seed data is created through the same transaction interface
    /// used by edits. Renderers contain no duplicate wall/grid coordinates.
    pub fn spatial_sample() -> Self {
        let mut ops = Vec::new();
        let mut add = |id, category, name: &str, width_m, height_m, elevation_m, geometry| {
            ops.push(super::Command::Create { id: ElementId(id), category, name: name.into(), width_m, height_m, elevation_m });
            ops.push(super::Command::SetGeometry { id: ElementId(id), geometry });
        };
        add(10, Category::Level, "Level 1", 1.0, 1.0, 0.0, Geometry::Level);
        add(11, Category::Level, "Level 2", 1.0, 1.0, 3.0, Geometry::Level);
        for (id, name, start, end) in [
            (100, "A", [0.0, -1.0], [0.0, 7.0]), (101, "B", [4.0, -1.0], [4.0, 7.0]),
            (102, "C", [8.0, -1.0], [8.0, 7.0]), (103, "1", [-1.0, 0.0], [9.0, 0.0]),
            (104, "2", [-1.0, 3.0], [9.0, 3.0]), (105, "3", [-1.0, 6.0], [9.0, 6.0]),
        ] { add(id, Category::Grid, name, 1.0, 1.0, 0.0, Geometry::Grid { start, end }); }
        for (i, start, end) in [
            (0, [0.0, 0.0], [8.0, 0.0]), (1, [8.0, 0.0], [8.0, 6.0]),
            (2, [8.0, 6.0], [0.0, 6.0]), (3, [0.0, 6.0], [0.0, 0.0]),
            (4, [4.0, 0.0], [4.0, 6.0]),
        ] { add(20 + i, Category::Wall, &format!("Wall {}", i + 1), 0.2, 3.0, 0.0,
            Geometry::Linear { start, end, level: ElementId(10) }); }
        add(30, Category::Wall, "Upper wall", 0.2, 2.8, 0.0,
            Geometry::Linear { start: [0.0, 6.0], end: [8.0, 6.0], level: ElementId(11) });
        add(1, Category::Duct, "Supply Air Duct", 0.2, 0.3, 2.7,
            Geometry::Linear { start: [1.0, 2.0], end: [7.0, 2.0], level: ElementId(10) });
        add(2, Category::Pipe, "Drainage Pipe", 0.1, 0.1, 0.6,
            Geometry::Linear { start: [1.0, 4.0], end: [7.0, 4.0], level: ElementId(10) });
        // the sample's site: an illustration, like the rest of the sample, and editable
        ops.push(super::Command::SetSite(crate::Site { latitude: 11.5564, longitude: 104.9282, angle_to_true_north_deg: 0.0, utc_offset_hours: 7.0 }));
        let mut document = Self::new();
        document.transact(0, &ops).expect("valid spatial sample");
        document.clear_undo_history();
        document
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Material, SlabKind, SlabSpec};
    #[test]
    fn a_beam_takes_a_round_or_an_i_section_changes_with_its_size_and_the_sections_are_validated() {
        use crate::Section;
        let mut d = Document::spatial_sample();
        let place = |d: &mut Document, w, h, s| d.place_beam(ElementId(10), &crate::BeamSpec { section: s, ..crate::BeamSpec::level([0.0, 1.0], [4.0, 1.0], w, h, 2.8) });
        // a round beam is as deep as it is wide, and its mesh is a 32-gon prism
        let round = place(&mut d, 0.3, 0.3, Section::Circle).unwrap();
        assert_eq!(d.mesh(round).unwrap().vertices.len(), 64);
        let other = place(&mut d, 0.3, 0.4, Section::Circle).unwrap();
        assert_eq!(d.get(other).unwrap().height_m, 0.3, "a circle takes its width as its diameter");
        // an I-section needs a web thinner than its flanges are wide and room between its flanges
        let i = place(&mut d, 0.2, 0.4, Section::IShape { web_m: 0.01, flange_m: 0.02 }).unwrap();
        assert_eq!(d.mesh(i).unwrap().vertices.len(), 24);
        assert!(place(&mut d, 0.2, 0.4, Section::IShape { web_m: 0.3, flange_m: 0.02 }).is_err());
        assert!(place(&mut d, 0.2, 0.04, Section::IShape { web_m: 0.01, flange_m: 0.02 }).is_err());
        // a section and its size change in one transaction: I to circle
        d.transact(d.revision(), &[
            Command::Resize { id: i, width_m: 0.25, height_m: 0.5 },
            Command::SetSection { id: i, section: Section::Circle },
        ]).unwrap();
        assert_eq!((d.get(i).unwrap().width_m, d.get(i).unwrap().height_m), (0.25, 0.25), "the circle takes the width");
        d.undo().unwrap();
        assert_eq!(d.get(i).unwrap().section, Section::IShape { web_m: 0.01, flange_m: 0.02 });
        // only beams have a section
        assert!(d.transact(d.revision(), &[Command::SetSection { id: ElementId(20), section: Section::Circle }]).is_err());
    }

    #[test]
    fn a_column_stands_between_two_levels_is_validated_resized_and_undone() {
        use crate::{ColumnSpec, Material, Section};
        let mut d = Document::spatial_sample();
        let spec = ColumnSpec { width_m: 0.4, depth_m: 0.4, base_offset_m: 0.0, section: Section::Rectangle, material: Material::Concrete };
        let id = d.place_column(ElementId(10), ElementId(11), [2.0, 2.0], &spec).unwrap();
        let el = d.get(id).unwrap();
        assert_eq!((el.category, el.material, el.name.as_str()), (Category::Column, Material::Concrete, "Column 1"));
        let m = d.mesh(id).unwrap();
        let zs: Vec<f64> = m.vertices.iter().map(|v| v[2]).collect();
        assert!(zs.iter().all(|z| *z == 0.0 || *z == 3.0), "from Level 1 (0) to Level 2 (3): {zs:?}");
        assert_eq!(m.vertices.len(), 8);
        // the top has to be above the foot: Level 2 to Level 1, or a foot raised to the top, are refused
        assert!(d.place_column(ElementId(11), ElementId(10), [2.0, 2.0], &spec).is_err());
        assert!(d.place_column(ElementId(10), ElementId(11), [2.0, 2.0], &ColumnSpec { base_offset_m: 3.0, ..spec }).is_err());
        assert!(d.place_column(ElementId(10), ElementId(20), [2.0, 2.0], &spec).is_err(), "a wall is not a level");
        assert!(d.place_column(ElementId(10), ElementId(11), [f64::NAN, 2.0], &spec).is_err());
        // a round column, and the foot raised 0.5 m: one transaction
        d.transact(d.revision(), &[
            Command::SetSection { id, section: Section::Circle },
            Command::MoveElevation { id, elevation_m: 0.5 },
        ]).unwrap();
        let el = d.get(id).unwrap();
        assert_eq!((el.section, el.width_m, el.height_m, el.elevation_m), (Section::Circle, 0.4, 0.4, 0.5));
        assert_eq!(d.mesh(id).unwrap().vertices.len(), 64);
        // moving Level 2 down so the column would have no height is refused and changes nothing
        let rev = d.revision();
        assert!(d.transact(rev, &[Command::MoveElevation { id: ElementId(11), elevation_m: 0.4 }]).is_err());
        assert_eq!(d.revision(), rev);
        // a column cannot be created without its place
        assert!(d.transact(d.revision(), &[Command::Create { id: ElementId(500), category: Category::Column, name: "C".into(), width_m: 0.3, height_m: 0.3, elevation_m: 0.0 }]).is_err());
        d.undo().unwrap();
        assert_eq!(d.get(id).unwrap().section, Section::Rectangle);
        d.undo().unwrap();
        assert!(d.get(id).is_none());
    }

    #[test]
    fn a_brace_is_a_sloped_beam_with_a_usage_and_a_material_and_only_beams_have_them() {
        use crate::{BeamSpec, Material, Usage};
        let mut d = Document::spatial_sample();
        // 3 m of run rising 3 m: a diagonal from 0.5 m at the start to 3.5 m at the end
        let spec = BeamSpec { usage: Usage::Brace, material: Material::Steel, slope_ratio: 1.0, ..BeamSpec::level([0.0, 1.0], [3.0, 1.0], 0.1, 0.1, 0.5) };
        let id = d.place_beam(ElementId(10), &spec).unwrap();
        let el = d.get(id).unwrap();
        assert_eq!((el.usage, el.material, el.slope_ratio, el.name.as_str()), (Usage::Brace, Material::Steel, Some(1.0), "Brace 1"));
        let zs: Vec<f64> = d.mesh(id).unwrap().vertices.iter().map(|v| v[2]).collect();
        let (lo, hi) = (zs.iter().cloned().fold(f64::MAX, f64::min), zs.iter().cloned().fold(f64::MIN, f64::max));
        // the centre line runs 0.5 to 3.5; the 0.1 square section is turned with the member
        assert!(lo > 0.4 - 0.1 && lo < 0.5 && hi > 3.5 && hi < 3.6, "{lo} {hi}");
        d.transact(d.revision(), &[Command::SetUsage { id, usage: Usage::Joist }, Command::SetMaterial { id, material: Material::Timber }]).unwrap();
        assert_eq!((d.get(id).unwrap().usage, d.get(id).unwrap().material), (Usage::Joist, Material::Timber));
        d.undo().unwrap();
        assert_eq!((d.get(id).unwrap().usage, d.get(id).unwrap().material), (Usage::Brace, Material::Steel));
        assert!(d.transact(d.revision(), &[Command::SetUsage { id: ElementId(20), usage: Usage::Brace }]).is_err());
        assert!(d.transact(d.revision(), &[Command::SetMaterial { id: ElementId(20), material: Material::Steel }]).is_err());
    }

    #[test]
    fn a_beam_is_placed_on_a_level_validated_and_undone_in_one_step() {
        let mut d = Document::spatial_sample();
        let id = d.place_beam(ElementId(10), &crate::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.2, 0.4, 2.8)).unwrap();
        assert_eq!(id, ElementId(d.elements().map(|e| e.id.0).max().unwrap()));
        assert_eq!(d.get(id).unwrap().category, Category::Beam);
        let m = d.mesh(id).unwrap();
        assert_eq!(m.vertices.len(), 8);
        let zs: Vec<f64> = m.vertices.iter().map(|v| v[2]).collect();
        assert!(zs.iter().all(|z| (z - 2.6).abs() < 1e-9 || (z - 3.0).abs() < 1e-9));
        // zero length, no size, and a level that is not one are all refused and change nothing
        let rev = d.revision();
        assert!(d.place_beam(ElementId(10), &crate::BeamSpec::level([1.0, 1.0], [1.0, 1.0], 0.2, 0.4, 2.8)).is_err());
        assert!(d.place_beam(ElementId(10), &crate::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.0, 0.4, 2.8)).is_err());
        assert!(d.place_beam(ElementId(100), &crate::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.2, 0.4, 2.8)).is_err());
        assert_eq!(d.revision(), rev);
        d.undo().unwrap();
        assert!(d.get(id).is_none(), "one undo removes it whole");
    }

    use crate::Command;

    #[test]
    fn wall_dimensions_and_level_elevation_drive_vertices_and_undo() {
        let mut d = Document::spatial_sample();
        let original = d.mesh(ElementId(20)).unwrap();
        assert!(!d.can_undo());
        d.transact(d.revision(), &[
            Command::Resize { id: ElementId(20), width_m: 0.4, height_m: 4.0 },
            Command::MoveElevation { id: ElementId(10), elevation_m: 1.0 },
        ]).unwrap();
        let changed = d.mesh(ElementId(20)).unwrap();
        assert_eq!(changed.vertices[0][1], -0.2);
        assert_eq!(changed.vertices[0][2], 1.0);
        assert_eq!(changed.vertices[2][2], 5.0);
        d.undo().unwrap();
        assert_eq!(d.mesh(ElementId(20)).unwrap(), original);
    }

    #[test]
    fn duct_and_pipe_slope_raise_the_end_by_rise_over_run() {
        let mut d = Document::spatial_sample();
        for id in [ElementId(1), ElementId(2)] {
            d.transact(d.revision(), &[Command::SetSlope { id, slope_ratio: 0.02 }]).unwrap();
            let mesh = d.mesh(id).unwrap();
            let n = mesh.vertices.len() / 2;
            assert!((mesh.vertices[n][2] - mesh.vertices[0][2] - 0.12).abs() < 1e-9);
        }
    }

    #[test]
    fn invalid_axes_and_referenced_level_deletion_are_atomic() {
        let mut d = Document::spatial_sample();
        let before = d.clone();
        assert!(d.transact(d.revision(), &[Command::Delete { id: ElementId(10) }]).is_err());
        assert_eq!(d, before);
        assert!(d.transact(d.revision(), &[Command::SetGeometry {
            id: ElementId(100), geometry: Geometry::Grid { start: [0.0, 0.0], end: [0.0, 0.0] },
        }]).is_err());
        assert_eq!(d, before);
    }

    #[test]
    fn datum_dimensions_and_inconsistent_pipe_sections_are_rejected() {
        let mut d = Document::spatial_sample();
        let before = d.clone();
        for id in [ElementId(10), ElementId(100)] {
            assert_eq!(d.transact(d.revision(), &[Command::Resize { id, width_m: 1.0, height_m: 2.0 }]),
                Err(EditError::UnsupportedProperty(id)));
        }
        assert!(d.transact(d.revision(), &[Command::Resize { id: ElementId(2), width_m: 0.2, height_m: 0.1 }]).is_err());
        assert_eq!(d, before);
    }

    fn slab(outline: Vec<[f64; 2]>) -> SlabSpec {
        SlabSpec { outline, thickness_m: 0.2, top_offset_m: 0.0, kind: SlabKind::Floor, material: Material::Concrete }
    }

    fn volume(mesh: &Mesh) -> f64 {
        // divergence theorem over fan-triangulated faces: positive when the faces face outwards
        let mut v = 0.0;
        for f in &mesh.faces {
            for k in 1..f.len() - 1 {
                let (a, b, c) = (mesh.vertices[f[0]], mesh.vertices[f[k]], mesh.vertices[f[k + 1]]);
                v += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0;
            }
        }
        v
    }

    #[test]
    fn slabs_are_placed_validated_undone_and_meshed_closed() {
        let mut d = Document::spatial_sample();
        let level = ElementId(10);
        let before = d.clone();
        // an L-shaped slab, 5 x 4 minus a 2 x 2 corner = 16 m2, in both windings
        let l = vec![[0.0, 0.0], [5.0, 0.0], [5.0, 2.0], [3.0, 2.0], [3.0, 4.0], [0.0, 4.0]];
        let mut rev = l.clone();
        rev.reverse();
        for ring in [l.clone(), rev] {
            let id = d.place_slab(level, &slab(ring)).unwrap();
            let el = d.get(id).unwrap();
            assert_eq!((el.category, el.material, el.slab_kind), (Category::Slab, Material::Concrete, SlabKind::Floor));
            let mesh = d.mesh(id).unwrap();
            assert!((volume(&mesh) - 16.0 * 0.2).abs() < 1e-9, "volume {}", volume(&mesh));
        }
        // refused: too few corners, no area, a bow-tie, a vertex touching an edge, a foundation outline on a missing level
        for bad in [
            vec![[0.0, 0.0], [1.0, 0.0]],
            vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]],
            vec![[0.0, 0.0], [2.0, 2.0], [2.0, 0.0], [0.0, 2.0]],
            vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [2.0, 0.0], [0.0, 4.0]],
            vec![[0.0, 0.0], [f64::NAN, 0.0], [1.0, 1.0]],
        ] {
            let n = d.elements().count();
            assert!(d.place_slab(level, &slab(bad)).is_err());
            assert_eq!(d.elements().count(), n);
        }
        assert!(d.place_slab(ElementId(9999), &slab(l)).is_err());
        // undo takes the slabs away again
        while d.undo().unwrap() {}
        assert_eq!(d.elements().count(), before.elements().count());
        assert!(d.elements().all(|e| e.category != Category::Slab));
    }

    #[test]
    fn a_foundation_slab_hangs_from_its_top_and_keeps_its_kind() {
        let mut d = Document::spatial_sample();
        let spec = SlabSpec { outline: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]], thickness_m: 0.5, top_offset_m: -0.1, kind: SlabKind::Foundation, material: Material::Concrete };
        let id = d.place_slab(ElementId(10), &spec).unwrap();
        let mesh = d.mesh(id).unwrap();
        let zs: Vec<f64> = mesh.vertices.iter().map(|v| v[2]).collect();
        let base = d.get(ElementId(10)).unwrap().elevation_m;
        assert!((zs.iter().cloned().fold(f64::MIN, f64::max) - (base - 0.1)).abs() < 1e-9);
        assert!((zs.iter().cloned().fold(f64::MAX, f64::min) - (base - 0.6)).abs() < 1e-9);
        assert_eq!(d.get(id).unwrap().slab_kind, SlabKind::Foundation);
        // only slabs take an outline or a kind
        assert!(d.transact(d.revision(), &[Command::SetSlabKind { id: ElementId(2), kind: SlabKind::Foundation }]).is_err());
        assert!(d.transact(d.revision(), &[Command::SetOutline { id: ElementId(2), outline: vec![] }]).is_err());
    }
}
