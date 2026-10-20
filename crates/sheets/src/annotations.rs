//! View-anchored datum bubbles: positions are model metres, sizes are paper mm.
//! Both PDF and DXF receive the same validated vector outline and centred label.
use std::collections::BTreeMap;

use crate::model::{Params, Sheet};
use crate::paper::Rect;
use crate::pdf::text_width_mm;
use crate::render::{halftone, layer_name, LineWeightTable, Prim, PrintMode, RenderedSheet, MAX_PRIMS};
use crate::titleblock::Align;
use crate::visibility::{Display, ElementRef, ViewLibrary};
use crate::{clean_name, finite, SheetError};

pub const MAX_MARKERS_PER_VIEW: usize = 10_000;
const SEGMENTS: usize = 32;

#[derive(Debug, Clone)]
pub struct ViewMarker {
    /// Model position in the view, metres; stays attached when scale changes.
    pub at: [f64; 2],
    pub label: String,
    pub category: String,
    /// Minimum radius in paper mm. Long labels expand the bubble to fit.
    pub radius_mm: f64,
    pub text_height_mm: f64,
    pub weight: u8,
}

pub type ViewMarkers = BTreeMap<String, Vec<ViewMarker>>;

impl ViewMarker {
    fn validate(&self) -> Result<(), SheetError> {
        clean_name(&self.label, "marker label")?;
        clean_name(&self.category, "marker category")?;
        for n in self.at { finite(n, "marker position")?; }
        finite(self.radius_mm, "marker radius")?;
        finite(self.text_height_mm, "marker text height")?;
        if !(0.5..=50.0).contains(&self.radius_mm) || !(0.5..=10.0).contains(&self.text_height_mm)
            || !(1..=16).contains(&self.weight) {
            return Err(SheetError::BadValue { what: "marker style", value: "radius 0.5–50 mm, text 0.5–10 mm, pen 1–16 required".into() });
        }
        Ok(())
    }
}

pub(crate) fn append_markers(sheet: &Sheet, views: &ViewLibrary, markers: &ViewMarkers,
    weights: &LineWeightTable, mode: PrintMode, output: &mut RenderedSheet) -> Result<(), SheetError>
{
    let params = Params::new();
    for viewport in &sheet.viewports {
        let Some(items) = markers.get(&viewport.view) else { continue; };
        if items.len() > MAX_MARKERS_PER_VIEW { return Err(SheetError::TooMany("view markers")); }
        let view = views.view(&viewport.view).ok_or_else(|| SheetError::Unknown { what: "view", value: viewport.view.clone() })?;
        let frame = viewport.frame();
        let k = 1000.0 / f64::from(viewport.scale);
        let mut clipped = 0;
        for marker in items {
            marker.validate()?;
            let el = ElementRef { category: &marker.category, params: &params, link: None };
            let Display::Shown { halftone: ht, weight, colour } = views.display(view, &el)? else { continue; };
            let category = views.category(&marker.category).expect("display checked category");
            let centre = [viewport.centre[0] + (marker.at[0] - viewport.model_centre[0]) * k,
                viewport.centre[1] + (marker.at[1] - viewport.model_centre[1]) * k];
            for n in centre { finite(n, "marker sheet coordinate")?; }
            let radius = marker.radius_mm.max(marker.text_height_mm * 0.7)
                .max(text_width_mm(&marker.label, marker.text_height_mm, false) / 2.0 + 0.6);
            // A bubble must remain whole. Never emit half a label at the frame edge.
            if !Rect::new(centre[0] - radius, centre[1] - radius, radius * 2.0, radius * 2.0).inside(&frame) {
                clipped += 1; continue;
            }
            if output.prims.len() + SEGMENTS + 1 > MAX_PRIMS { return Err(SheetError::TooMany("drawing primitives")); }
            let layer = layer_name(category.discipline.code(), &category.name);
            let base = mode.base_colour(colour.unwrap_or([0; 3]));
            let rgb = if ht { halftone(base) } else { base };
            let weight_mm = weights.paper_mm(weight.unwrap_or(marker.weight));
            let point = |i: usize| {
                let angle = (i % SEGMENTS) as f64 * std::f64::consts::TAU / SEGMENTS as f64;
                [centre[0] + radius * angle.cos(), centre[1] + radius * angle.sin()]
            };
            for i in 0..SEGMENTS {
                output.prims.push(Prim::Line { a: point(i), b: point(i + 1), weight_mm, rgb,
                    layer: format!("{layer}_MARKERS") });
            }
            output.prims.push(Prim::Text {
                at: [centre[0], centre[1] - marker.text_height_mm * 0.35], height: marker.text_height_mm,
                align: Align::Centre, text: marker.label.clone(), bold: false, layer: format!("{layer}_LABELS"),
            });
        }
        if clipped > 0 {
            output.warnings.push(format!("sheet {}: {clipped} grid/axis markers omitted at or outside viewport '{}'; enlarge the paper or increase the scale denominator", sheet.number, viewport.view));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Viewport;
    use crate::paper::PaperSize;
    use crate::visibility::{Category, CategoryKind, Discipline, Override, ViewSettings};

    fn marker() -> ViewMarker {
        ViewMarker { at: [1.0, 2.0], label: "A".into(), category: "Grids".into(), radius_mm: 2.5,
            text_height_mm: 2.5, weight: 2 }
    }

    fn render(items: Vec<ViewMarker>, scale: u32, hidden: bool) -> Result<RenderedSheet, SheetError> {
        let mut sheet = Sheet::new("T-001", "Test", "unused");
        sheet.viewports.push(Viewport { view: "Plan".into(), centre: [100.0, 100.0], size: [100.0, 100.0],
            scale, model_centre: [0.0, 0.0], title: None });
        let mut views = ViewLibrary::new();
        views.add_category(Category { name: "Grids".into(), discipline: Discipline::Coordination, kind: CategoryKind::Annotation })?;
        let mut view = ViewSettings::new("Plan", Discipline::Coordination);
        if hidden { view.categories.insert("Grids".into(), Override { visible: Some(false), ..Default::default() }); }
        views.add_view(view)?;
        let markers = ViewMarkers::from([("Plan".into(), items)]);
        let weights = LineWeightTable::uniform(vec![0.25; 16])?;
        let mut output = RenderedSheet { number: "T-001".into(), name: "Test".into(), size: PaperSize::iso_a(3).unwrap(),
            prims: Vec::new(), warnings: Vec::new() };
        append_markers(&sheet, &views, &markers, &weights, PrintMode::Colour, &mut output)?;
        Ok(output)
    }

    #[test]
    fn paper_size_is_fixed_while_anchor_follows_model_scale() {
        for (scale, centre) in [(100, [110.0, 120.0]), (200, [105.0, 110.0])] {
            let output = render(vec![marker()], scale, false).unwrap();
            assert_eq!(output.prims.len(), SEGMENTS + 1);
            for (i, prim) in output.prims.iter().take(SEGMENTS).enumerate() {
                let Prim::Line { a, b, weight_mm, layer, .. } = prim else { panic!("bubble must be vector lines"); };
                assert_eq!(*weight_mm, 0.25);
                assert_eq!(layer, "C-GRIDS_MARKERS");
                assert!(((a[0] - centre[0]).hypot(a[1] - centre[1]) - 2.5).abs() < 1e-9);
                let Prim::Line { a: next, .. } = &output.prims[(i + 1) % SEGMENTS] else { unreachable!() };
                assert_eq!(b, next, "the bubble must close without a gap");
            }
            let Prim::Text { at, height, text, align, layer, .. } = output.prims.last().unwrap() else { panic!("label missing"); };
            assert_eq!(at, &[centre[0], centre[1] - 0.875]);
            assert_eq!(*height, 2.5); assert_eq!(text, "A"); assert_eq!(*align, Align::Centre);
            assert_eq!(layer, "C-GRIDS_LABELS");
        }
    }

    #[test]
    fn long_labels_expand_bubbles_and_edge_markers_are_never_partially_drawn() {
        let mut long = marker(); long.label = "Grid-ABC".into();
        let output = render(vec![long], 100, false).unwrap();
        let Prim::Line { a, .. } = output.prims[0] else { unreachable!() };
        assert!(a[0] > 112.5, "long label must expand the radius");
        let mut edge = marker(); edge.at = [-5.0, 0.0];
        let output = render(vec![edge, marker()], 100, false).unwrap();
        assert_eq!(output.prims.len(), SEGMENTS + 1);
        assert!(output.warnings.iter().any(|w| w.contains("1 grid/axis markers omitted")));
        assert!(render(vec![marker()], 100, true).unwrap().prims.is_empty());
    }

    #[test]
    fn invalid_marker_data_is_rejected_before_encoding() {
        let mut invalid = Vec::new();
        let mut item = marker(); item.at[0] = f64::NAN; invalid.push(item);
        let mut item = marker(); item.radius_mm = -1.0; invalid.push(item);
        let mut item = marker(); item.text_height_mm = f64::INFINITY; invalid.push(item);
        let mut item = marker(); item.weight = 0; invalid.push(item);
        let mut item = marker(); item.label = "A\nB".into(); invalid.push(item);
        let mut item = marker(); item.category = "missing".into(); invalid.push(item);
        for item in invalid { assert!(render(vec![item], 100, false).is_err()); }
        assert!(render(vec![marker(); MAX_MARKERS_PER_VIEW + 1], 100, false).is_err());
    }
}
