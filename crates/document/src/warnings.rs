//! Warnings: conditions the document allows but that are probably not what was meant. They are worked out from the
//! document as it is now, never stored, so they appear and clear by themselves as the model is edited.
use std::collections::BTreeMap;

use crate::{Category, Document, ElementId, Geometry};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WarningKind {
    /// The bars of a rebar no longer fit in the beam (the beam was made smaller or shorter): no bars are drawn.
    RebarDoesNotFit,
    /// A column stands inside a wall.
    ColumnInsideWall,
    /// Two beams on the same line at the same height.
    DuplicateBeam,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Warning {
    pub kind: WarningKind,
    /// The elements concerned, the first one being the one to select.
    pub elements: Vec<ElementId>,
    pub message: String,
}

impl Document {
    /// Every warning of the document, in a stable order: by kind, then by element.
    pub fn warnings(&self) -> Vec<Warning> {
        let name = |id: ElementId| self.get(id).map_or_else(|| format!("#{}", id.0), |e| if e.name.is_empty() { format!("#{}", id.0) } else { e.name.clone() });
        let mut out = Vec::new();

        for r in self.elements().filter(|e| e.category == Category::Rebar) {
            if self.rebar_bars(r).is_none() {
                let host = match r.geometry { Some(Geometry::OnMember { host }) => vec![host], _ => vec![] };
                let beam = host.first().map_or(String::new(), |h| format!(" in {}", name(*h)));
                out.push(Warning {
                    kind: WarningKind::RebarDoesNotFit,
                    message: format!("{}: the bars do not fit{beam}; no bars are drawn. Make the beam larger or the bars smaller.", name(r.id)),
                    elements: std::iter::once(r.id).chain(host).collect(),
                });
            }
        }

        let walls: Vec<_> = self.elements().filter_map(|w| {
            let (Category::Wall, Some(Geometry::Linear { start, end, level })) = (w.category, w.geometry) else { return None };
            let z0 = self.get(level)?.elevation_m + w.elevation_m;
            Some((w.id, start, end, w.width_m / 2.0, z0, z0 + w.height_m))
        }).collect();
        for c in self.elements().filter(|e| e.category == Category::Column) {
            let Some(Geometry::Point { at, level, top }) = c.geometry else { continue };
            let (Some(a), Some(b)) = (self.get(level), self.get(top)) else { continue };
            let (z0, z1) = (a.elevation_m + c.elevation_m, b.elevation_m);
            for &(wid, s, e, half, w0, w1) in &walls {
                if z1 <= w0 + 1e-6 || z0 >= w1 - 1e-6 { continue; }
                let (dx, dy) = (e[0] - s[0], e[1] - s[1]);
                let len2 = dx * dx + dy * dy;
                if len2 <= 0.0 { continue; }
                let t = (((at[0] - s[0]) * dx + (at[1] - s[1]) * dy) / len2).clamp(0.0, 1.0);
                if (at[0] - (s[0] + t * dx)).hypot(at[1] - (s[1] + t * dy)) < half - 1e-6 {
                    out.push(Warning {
                        kind: WarningKind::ColumnInsideWall,
                        message: format!("{} stands inside {}.", name(c.id), name(wid)),
                        elements: vec![c.id, wid],
                    });
                }
            }
        }

        let mm = |v: f64| (v * 1000.0).round() as i64;
        let mut lines: BTreeMap<(u64, [i64; 4], i64, i64), Vec<ElementId>> = BTreeMap::new();
        for b in self.elements().filter(|e| e.category == Category::Beam) {
            let Some(Geometry::Linear { start, end, level }) = b.geometry else { continue };
            let (p, q) = ([mm(start[0]), mm(start[1])], [mm(end[0]), mm(end[1])]);
            let (p, q, flip) = if p <= q { (p, q, false) } else { (q, p, true) };
            // the same line is the same wherever it starts; a slope reads the other way round when it is flipped
            let slope = mm(b.slope_ratio.unwrap_or(0.0)) * if flip { -1 } else { 1 };
            lines.entry((level.0, [p[0], p[1], q[0], q[1]], mm(b.elevation_m), slope)).or_default().push(b.id);
        }
        for ids in lines.into_values().filter(|v| v.len() > 1) {
            out.push(Warning {
                kind: WarningKind::DuplicateBeam,
                message: format!("{} lie on top of each other.", ids.iter().map(|i| name(*i)).collect::<Vec<_>>().join(" and ")),
                elements: ids,
            });
        }

        out.sort_by(|a, b| (a.kind, &a.elements).cmp(&(b.kind, &b.elements)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeamSpec, RebarSpec};

    fn rebar() -> RebarSpec { RebarSpec { count: 3, bar_m: 0.016, stirrup_m: 0.008, cover_m: 0.03, stirrup_spacing_m: 0.2 } }

    #[test]
    fn a_rebar_that_no_longer_fits_its_beam_is_reported_until_the_beam_is_large_enough_again() {
        let mut d = Document::spatial_sample();
        let level = ElementId(10);
        let beam = d.place_beam(level, &BeamSpec::level([1.0, 3.0], [7.0, 3.0], 0.3, 0.5, 2.8)).unwrap();
        let rebar = d.place_rebar(beam, &rebar()).unwrap();
        assert!(d.warnings().iter().all(|w| w.kind != WarningKind::RebarDoesNotFit));
        d.transact(d.revision(), &[crate::Command::Resize { id: beam, width_m: 0.06, height_m: 0.5 }]).unwrap();
        let w = d.warnings();
        let w = w.iter().find(|w| w.kind == WarningKind::RebarDoesNotFit).expect("reported");
        assert_eq!(w.elements, vec![rebar, beam]);
        assert!(w.message.contains("do not fit"), "{}", w.message);
        d.transact(d.revision(), &[crate::Command::Resize { id: beam, width_m: 0.3, height_m: 0.5 }]).unwrap();
        assert!(d.warnings().is_empty());
    }

    #[test]
    fn two_beams_on_one_line_are_reported_in_either_direction_but_a_different_height_is_not() {
        let mut d = Document::spatial_sample();
        let level = ElementId(10);
        let a = d.place_beam(level, &BeamSpec::level([1.0, 3.0], [7.0, 3.0], 0.3, 0.5, 2.8)).unwrap();
        let b = d.place_beam(level, &BeamSpec::level([7.0, 3.0], [1.0, 3.0], 0.2, 0.4, 2.8)).unwrap();
        let _ = d.place_beam(level, &BeamSpec::level([1.0, 3.0], [7.0, 3.0], 0.3, 0.5, 3.4)).unwrap();
        let w: Vec<_> = d.warnings().into_iter().filter(|w| w.kind == WarningKind::DuplicateBeam).collect();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].elements, vec![a, b]);
    }
}

#[cfg(test)]
mod column_tests {
    use super::*;
    use crate::{ColumnSpec, Material, Section};

    #[test]
    fn a_column_in_a_wall_is_reported_and_one_beside_it_is_not() {
        let mut d = Document::spatial_sample();
        let col = ColumnSpec { width_m: 0.3, depth_m: 0.3, base_offset_m: 0.0, section: Section::Rectangle, material: Material::Concrete };
        let inside = d.place_column(ElementId(10), ElementId(11), [4.0, 3.0], &col).unwrap();
        let beside = d.place_column(ElementId(10), ElementId(11), [2.0, 3.0], &col).unwrap();
        let w: Vec<_> = d.warnings().into_iter().filter(|w| w.kind == WarningKind::ColumnInsideWall).collect();
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(w[0].elements[0], inside);
        assert!(w.iter().all(|w| w.elements[0] != beside));
    }
}
