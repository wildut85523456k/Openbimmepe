//! Metre-based definitions shared by plan and 3D views. Meshes are derived,
//! never a second mutable copy of the document's parameters.
use super::{Category, Document, EditError, Element, ElementId};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Geometry {
    Level,
    Grid { start: [f64; 2], end: [f64; 2] },
    Linear { start: [f64; 2], end: [f64; 2], level: ElementId },
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

pub(super) fn validate(elements: &BTreeMap<ElementId, Element>) -> Result<(), EditError> {
    for el in elements.values() {
        let valid = match &el.geometry {
            None => !matches!(el.category, Category::Level | Category::Grid),
            Some(Geometry::Level) => el.category == Category::Level && el.elevation_m.abs() <= 1e6,
            Some(Geometry::Grid { start, end }) => el.category == Category::Grid && valid_axis(*start, *end),
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
        let section: Vec<[f64; 2]> = if el.category == Category::Pipe {
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
    #[test]
    fn a_beam_is_placed_on_a_level_validated_and_undone_in_one_step() {
        let mut d = Document::spatial_sample();
        let id = d.place_beam(ElementId(10), [1.0, 1.0], [5.0, 1.0], 0.2, 0.4, 2.8).unwrap();
        assert_eq!(id, ElementId(d.elements().map(|e| e.id.0).max().unwrap()));
        assert_eq!(d.get(id).unwrap().category, Category::Beam);
        let m = d.mesh(id).unwrap();
        assert_eq!(m.vertices.len(), 8);
        let zs: Vec<f64> = m.vertices.iter().map(|v| v[2]).collect();
        assert!(zs.iter().all(|z| (z - 2.6).abs() < 1e-9 || (z - 3.0).abs() < 1e-9));
        // zero length, no size, and a level that is not one are all refused and change nothing
        let rev = d.revision();
        assert!(d.place_beam(ElementId(10), [1.0, 1.0], [1.0, 1.0], 0.2, 0.4, 2.8).is_err());
        assert!(d.place_beam(ElementId(10), [1.0, 1.0], [5.0, 1.0], 0.0, 0.4, 2.8).is_err());
        assert!(d.place_beam(ElementId(100), [1.0, 1.0], [5.0, 1.0], 0.2, 0.4, 2.8).is_err());
        assert_eq!(d.revision(), rev);
        // a beam has no slope
        assert!(d.transact(d.revision(), &[Command::SetSlope { id, slope_ratio: 0.1 }]).is_err());
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
}
