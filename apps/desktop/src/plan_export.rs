//! Metre-based document footprints to the shared sheets command. No camera,
//! selection highlighting, screen pixels or stale copies of model geometry.
use crate::View;
use openbim_sheets::annotations::{ViewMarker, ViewMarkers};
use openbim_document::{Category, Document, ElementId, Geometry};
use openbim_sheets::export::{ExportBundle, ExportCommand, ExportFormat, ExportSource};
use openbim_sheets::model::{Params, PrintContext, Sheet, SheetSet, Viewport};
use openbim_sheets::profile::{PrintProfile, ProfileMargins, ProfilePaper, ScalePens};
use openbim_sheets::render::{ModelPrim, PrintMode, ViewContents};
use openbim_sheets::visibility::{Category as PrintCategory, CategoryKind, Discipline, Override, ViewLibrary, ViewSettings};

pub fn generic_profile() -> PrintProfile {
    let mut papers = Vec::new();
    for n in 0..=4 {
        let size = openbim_sheets::paper::PaperSize::iso_a(n).expect("ISO preset");
        for portrait in [false, true] {
            let (width_mm, height_mm) = if portrait { size.portrait() } else { (size.width_mm, size.height_mm) };
            papers.push(ProfilePaper {
                name: format!("A{n} {}", if portrait { "portrait" } else { "landscape" }),
                width_mm, height_mm,
                margins_mm: ProfileMargins { left: 10.0, top: 10.0, right: 10.0, bottom: 10.0 },
            });
        }
    }
    // Editable demonstration values, not a company standard.
    let pens = vec![0.05, 0.09, 0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1.0, 1.4, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0];
    PrintProfile {
        schema_version: 1, name: "Generic demonstration profile".into(), papers,
        paper_pens_mm: pens.clone(), model_pen_tables: vec![ScalePens { scale: 100, widths_mm: pens }],
        mode: PrintMode::Colour,
    }
}

fn category(category: Category) -> (&'static str, Discipline, [u8; 3], u8) {
    match category {
        Category::Wall => ("Walls", Discipline::Architectural, [80; 3], 4),
        Category::Duct => ("Ducts", Discipline::Mechanical, [70, 157, 194], 4),
        Category::Pipe => ("Pipes", Discipline::Plumbing, [136, 153, 166], 3),
        Category::Grid => ("Grids", Discipline::Coordination, [135; 3], 2),
        _ => ("Equipment", Discipline::Mechanical, [128; 3], 4),
    }
}

/// Convex footprint of the same mesh the viewport uses. This intentionally
/// exports an outline, rather than hidden faces or pipe tessellation seams.
fn footprint(mut points: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    if points.len() < 3 { return points; }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut lower = Vec::new();
    let mut upper = Vec::new();
    for point in points.iter().copied() {
        while lower.len() >= 2 && cross(lower[lower.len()-2], lower[lower.len()-1], point) <= 0.0 { lower.pop(); }
        lower.push(point);
    }
    for point in points.iter().rev().copied() {
        while upper.len() >= 2 && cross(upper[upper.len()-2], upper[upper.len()-1], point) <= 0.0 { upper.pop(); }
        upper.push(point);
    }
    lower.pop(); upper.pop(); lower.extend(upper); lower
}

pub struct PlanContent {
    pub name: String,
    pub number: String,
    pub lines: Vec<ModelPrim>,
    pub markers: Vec<ViewMarker>,
    pub warnings: Vec<String>,
}

pub fn plan_content(document: &Document, view: View) -> Result<PlanContent, String> {
    // the plan views belong to the document's levels, lowest first (A2): no level id is hard-coded
    let levels: Vec<ElementId> = {
        let mut l: Vec<(ElementId, f64)> = document.elements().filter(|e| e.geometry == Some(Geometry::Level)).map(|e| (e.id, e.elevation_m)).collect();
        l.sort_by(|a, b| a.1.total_cmp(&b.1));
        l.into_iter().map(|(id, _)| id).collect()
    };
    let (level, number) = match view {
        View::Plan => (*levels.first().ok_or("The project has no level: add one before exporting a plan.")?, "PLAN-L1"),
        View::UpperPlan => (*levels.get(1).or_else(|| levels.first()).ok_or("The project has no level: add one before exporting a plan.")?, "PLAN-L2"),
        View::Mechanical => (*levels.first().ok_or("The project has no level: add one before exporting a plan.")?, "M-101"),
        View::ThreeD => return Err("Select a plan view before exporting; 3D export is not connected.".into()),
        // Elevations, sections, schedules, sheets and level plans are not wired to a plan export yet.
        _ => return Err("Select a plan or mechanical view before exporting.".into()),
    };
    let datum = document.get(level).filter(|el| el.geometry == Some(Geometry::Level))
        .ok_or_else(|| "The selected plan's level is missing.".to_string())?;
    let name = format!("{} footprint", datum.name);
    let mut lines = Vec::new();
    let mut markers = Vec::new();
    let mut skipped = 0;
    for el in document.elements() {
        let (cat, _, _, weight) = category(el.category);
        let mut push = |a, b| lines.push(ModelPrim {
            a, b, category: cat.into(), params: Params::new(), link: None, weight,
        });
        match el.geometry {
            Some(Geometry::Grid { start, end }) => {
                for w in openbim_document::grid_arc::points(start, end, el.bulge).windows(2) { push(w[0], w[1]); }
                for at in [start, end] {
                    markers.push(ViewMarker { at, label: el.name.clone(), category: cat.into(),
                        radius_mm: 2.5, text_height_mm: 2.5, weight: 2 });
                }
            }
            Some(Geometry::Linear { level: element_level, .. }) if element_level == level => {
                let mesh = document.mesh(el.id).ok_or_else(|| format!("Cannot derive element {} geometry.", el.id.0))?;
                let hull = footprint(mesh.vertices.iter().map(|p| [p[0], p[1]]).collect());
                for i in 0..hull.len() {
                    let a = hull[i]; let b = hull[(i + 1) % hull.len()];
                    if a != b { push(a, b); }
                }
            }
            None => skipped += 1,
            _ => {}
        }
    }
    let mut warnings = vec!["Plan footprint outlines, grid axes and labelled bubbles; fills, dimensions and cut-plane/hidden-line processing are not exported.".into()];
    if skipped > 0 { warnings.push(format!("Skipped {skipped} elements without spatial geometry.")); }
    if lines.is_empty() { return Err("No spatial geometry in the selected plan.".into()); }
    Ok(PlanContent { name, number: number.into(), lines, markers, warnings })
}

/// Fixed scale, centred on the selected level's geometry. Camera zoom and pan
/// never change print scale. Existing project title blocks are not overwritten.
pub fn export_plan(document: &Document, view: View, profile: &PrintProfile, paper: &str,
    scale: u32, format: ExportFormat) -> Result<ExportBundle, String>
{
    let plan = plan_content(document, view)?;
    let block = profile.generic_titleblock(paper, "Generic export title block").map_err(|e| e.to_string())?;
    let free = block.free_rect;
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for line in &plan.lines {
        for point in [line.a, line.b] {
            for axis in 0..2 { min[axis] = min[axis].min(point[axis]); max[axis] = max[axis].max(point[axis]); }
        }
    }
    let size = [free.width() - 4.0, free.height() - 12.0];
    if size.iter().any(|v| *v <= 0.0) { return Err("The selected paper has no usable plan area.".into()); }
    let padding = plan.markers.iter().map(|marker| marker.radius_mm
        .max(marker.text_height_mm * 0.7)
        .max(openbim_sheets::pdf::text_width_mm(&marker.label, marker.text_height_mm, false) / 2.0 + 0.6))
        .fold(0.0_f64, f64::max);
    let usable = [size[0] - 2.0 * padding, size[1] - 2.0 * padding];
    if usable.iter().any(|v| *v <= 0.0) { return Err("Grid labels do not fit on the selected paper.".into()); }
    let mut warnings = plan.warnings;
    if scale > 0 && (0..2).any(|axis| (max[axis] - min[axis]) * 1000.0 / f64::from(scale) > usable[axis]) {
        let needed = ((max[0] - min[0]) * 1000.0 / usable[0])
            .max((max[1] - min[1]) * 1000.0 / usable[1]).ceil() as u32;
        warnings.push(format!("Plan clipped at 1:{scale}. Use a larger sheet or a scale denominator of at least {needed} to fit."));
    }
    let mut set = SheetSet::default();
    // Leave sign-off and company fields empty: renderer warnings reach the UI.
    set.project.insert("Project Name".into(), "OpenBIM sample".into());
    set.project.insert("Drawing Type".into(), "Plan footprint".into());
    set.add_titleblock(block).map_err(|e| e.to_string())?;
    let mut sheet = Sheet::new(&plan.number, &plan.name, "Generic export title block");
    sheet.viewports.push(Viewport {
        view: plan.name.clone(), centre: [free.centre()[0], free.centre()[1] + 4.0], size, scale,
        model_centre: [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0], title: None,
    });
    set.add_sheet(sheet).map_err(|e| e.to_string())?;
    let mut views = ViewLibrary::new();
    let mut settings = ViewSettings::new(&plan.name, Discipline::Coordination);
    // Current desktop plan views show all categories on the selected level.
    // Do not introduce discipline hiding that the viewport does not perform.
    for cat in [Category::Wall, Category::Duct, Category::Pipe, Category::Grid] {
        let (name, discipline, colour, _) = category(cat);
        views.add_category(PrintCategory { name: name.into(), discipline,
            kind: if cat == Category::Grid { CategoryKind::Annotation } else { CategoryKind::Model } })
            .map_err(|e| e.to_string())?;
        settings.categories.insert(name.into(), Override { colour: Some(colour), ..Default::default() });
    }
    views.add_view(settings).map_err(|e| e.to_string())?;
    let markers = ViewMarkers::from([(plan.name.clone(), plan.markers)]);
    let contents = ViewContents::from([(plan.name.clone(), plan.lines)]);
    let context = PrintContext { plot_date: String::new(), file_path: "OpenBIM in-memory sample".into() };
    let mut output = ExportCommand { format, sheets: vec![plan.number], title: plan.name }
        .execute_with_markers(ExportSource { set: &set, views: &views, contents: &contents, profile, context: &context }, &markers)
        .map_err(|e| e.to_string())?;
    warnings.append(&mut output.warnings);
    output.warnings = warnings;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbim_document::Command;

    fn bounds(plan: &PlanContent, category: &str) -> ([f64; 2], [f64; 2]) {
        let mut min = [f64::INFINITY; 2]; let mut max = [f64::NEG_INFINITY; 2];
        for line in plan.lines.iter().filter(|l| l.category == category) {
            for p in [line.a, line.b] {
                for i in 0..2 { min[i] = min[i].min(p[i]); max[i] = max[i].max(p[i]); }
            }
        }
        (min, max)
    }

    #[test]
    fn active_level_filters_models_and_keeps_shared_grid_axes() {
        let doc = Document::spatial_sample();
        let lower = plan_content(&doc, View::Plan).unwrap();
        let upper = plan_content(&doc, View::UpperPlan).unwrap();
        assert_eq!(lower.lines.iter().filter(|l| l.category == "Walls").count(), 20);
        assert_eq!(upper.lines.iter().filter(|l| l.category == "Walls").count(), 4);
        assert_eq!(lower.lines.iter().filter(|l| l.category == "Grids").count(), 6);
        assert_eq!(upper.lines.iter().filter(|l| l.category == "Grids").count(), 6);
        assert_eq!(lower.markers.len(), 12);
        assert_eq!(upper.markers.len(), 12);
        assert_eq!(lower.markers.iter().filter(|marker| marker.label == "A").count(), 2);
        assert!(!upper.lines.iter().any(|l| l.category == "Ducts" || l.category == "Pipes"));
        assert!(plan_content(&doc, View::ThreeD).is_err());
    }

    #[test]
    fn edits_and_undo_change_real_exports_without_mutating_document() {
        let mut doc = Document::spatial_sample();
        let profile = generic_profile();
        let original = export_plan(&doc, View::Plan, &profile, "A3 landscape", 100, ExportFormat::Pdf).unwrap();
        let before = doc.clone();
        let lower = plan_content(&doc, View::Plan).unwrap();
        let (min, max) = bounds(&lower, "Ducts");
        assert!((max[1] - min[1] - 0.2).abs() < 1e-9);
        doc.transact(doc.revision(), &[
            Command::Resize { id: ElementId(1), width_m: 0.8, height_m: 0.3 },
            Command::SetSlope { id: ElementId(1), slope_ratio: -0.02 },
            Command::SetGeometry { id: ElementId(1), geometry: Geometry::Linear {
                start: [2.0, 2.0], end: [7.0, 2.0], level: ElementId(10),
            } },
        ]).unwrap();
        let edited = plan_content(&doc, View::Plan).unwrap();
        let (min, max) = bounds(&edited, "Ducts");
        assert!((max[1] - min[1] - 0.8).abs() < 1e-9);
        assert!(min[0] < 2.0 && min[0] > 1.99, "signed slope must use mesh projection");
        let changed = export_plan(&doc, View::Plan, &profile, "A3 landscape", 100, ExportFormat::Pdf).unwrap();
        assert_ne!(original.files[0].bytes, changed.files[0].bytes);
        doc.undo().unwrap();
        let restored = doc.clone();
        let output = export_plan(&doc, View::Plan, &profile, "A3 landscape", 100, ExportFormat::Pdf).unwrap();
        assert_eq!(original.files[0].bytes, output.files[0].bytes);
        assert_eq!(doc, restored, "export must not change document or undo history");
        assert_eq!(doc.mesh(ElementId(1)), before.mesh(ElementId(1)));
    }

    #[test]
    fn orientation_scale_mode_and_warnings_reach_both_encoders() {
        let doc = Document::spatial_sample();
        let mut profile = generic_profile();
        for (paper, media) in [("A3 landscape", "/MediaBox [0 0 1190.551 841.89]"),
            ("A3 portrait", "/MediaBox [0 0 841.89 1190.551]")] {
            let out = export_plan(&doc, View::Mechanical, &profile, paper, 100, ExportFormat::Pdf).unwrap();
            assert!(String::from_utf8_lossy(&out.files[0].bytes).contains(media));
        }
        let normal = export_plan(&doc, View::Plan, &profile, "A3 landscape", 100, ExportFormat::Pdf).unwrap();
        profile.mode = PrintMode::BlackLines;
        let black = export_plan(&doc, View::Plan, &profile, "A3 landscape", 100, ExportFormat::Pdf).unwrap();
        assert_ne!(normal.files[0].bytes, black.files[0].bytes);
        let dxf = export_plan(&doc, View::Plan, &profile, "A3 landscape", 100, ExportFormat::Dxf).unwrap();
        let text = String::from_utf8_lossy(&dxf.files[0].bytes);
        assert!(text.contains("M-DUCTS_W018"));
        assert!(text.contains("P-PIPES_W013"));
        let clipped = export_plan(&doc, View::Plan, &profile, "A4 portrait", 1, ExportFormat::Pdf).unwrap();
        assert!(clipped.warnings.iter().any(|w| w.contains("clipped")));
        assert!(export_plan(&doc, View::Plan, &profile, "A3 landscape", 0, ExportFormat::Pdf).is_err());
        assert!(export_plan(&doc, View::Plan, &profile, "missing", 100, ExportFormat::Pdf).is_err());
    }

    /// CI requests this explicit fixture; ordinary test runs do no file writes.
    #[test]
    #[ignore = "writes independent PDF/DXF fixtures to OPENBIM_PRINT_TEST_OUTPUT"]
    fn desktop_export_fixtures() {
        let dir = std::path::PathBuf::from(std::env::var_os("OPENBIM_PRINT_TEST_OUTPUT").expect("fixture output directory"));
        std::fs::create_dir_all(&dir).unwrap();
        let doc = Document::spatial_sample();
        let profile = generic_profile();
        for (view, view_name) in [(View::Plan, "lower"), (View::UpperPlan, "upper")] {
            for (paper, orientation) in [("A3 landscape", "landscape"), ("A3 portrait", "portrait")] {
                for (format, ext) in [(ExportFormat::Pdf, "pdf"), (ExportFormat::Dxf, "dxf")] {
                    let output = export_plan(&doc, view, &profile, paper, 100, format).unwrap();
                    std::fs::write(dir.join(format!("{view_name}-{orientation}.{ext}")), &output.files[0].bytes).unwrap();
                }
            }
        }
    }
}

