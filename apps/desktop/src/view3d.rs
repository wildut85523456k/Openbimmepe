//! The {3D} view: the document's meshes drawn by the wgpu viewport widget (Revit mouse mapping, ViewCube, visual
//! styles, section box, click selection).
use crate::Desktop;
use eframe::egui;
use openbim_document::{Category, Command, Document, ElementId, Geometry, Site};
use openbim_view::{Batch, Element, FaceKind, Kind};
use openbim_viewport_egui::{Datum, Style};
use std::collections::HashSet;
use std::sync::Arc;

fn kind(c: Category) -> Option<Kind> {
    match c { Category::Wall => Some(Kind::Wall), Category::Beam => Some(Kind::Beam), Category::Column => Some(Kind::Column), Category::Slab | Category::Footing | Category::WallFooting => Some(Kind::Slab), Category::Duct | Category::Pipe => Some(Kind::Segment), Category::Equipment => Some(Kind::Family), _ => None }
}

/// The element of the view that stands for a document element.
fn element(doc: &Document, id: ElementId) -> Option<Element> {
    Some(Element { kind: kind(doc.get(id)?.category)?, id: id.0 })
}

/// All meshes of the document as one batch. Faces are wound outwards (the solids are convex).
pub fn batch(doc: &Document) -> Batch {
    let mut meshes = Vec::new();
    for el in doc.elements() {
        let Some(k) = kind(el.category) else { continue };
        if el.category == Category::Beam {
            // a beam is meshed by the scene mesher: its section may be an I, which is not convex, and its faces
            // come out wound outward, so they are taken as they are
            let Some(m) = crate::export::beam_item(doc, el).and_then(|b| openbim_view::beam_mesh(&b)) else { continue };
            let faces = m.tris.iter().map(|t| t.iter().map(|&i| i as usize).collect()).collect();
            meshes.push((Element { kind: k, id: el.id.0 }, openbim_document::Mesh { vertices: m.verts, faces }, true));
            continue;
        }
        if el.category == Category::Column {
            // like a beam, a column is meshed by the scene mesher, outward wound, whatever its section
            let Some(m) = crate::export::column_item(doc, el).and_then(|c| openbim_view::column_mesh(&c)) else { continue };
            let faces = m.tris.iter().map(|t| t.iter().map(|&i| i as usize).collect()).collect();
            meshes.push((Element { kind: k, id: el.id.0 }, openbim_document::Mesh { vertices: m.verts, faces }, true));
            continue;
        }
        let Some(mesh) = doc.mesh(el.id) else { continue };
        // a slab's outline is not convex, but its faces are wound outward by construction
        meshes.push((Element { kind: k, id: el.id.0 }, mesh, matches!(el.category, Category::Slab | Category::Footing | Category::WallFooting)));
    }
    let mut origin = [f64::MAX; 3];
    for (_, m, _) in &meshes { for v in &m.vertices { for k in 0..3 { origin[k] = origin[k].min(v[k]); } } }
    if origin[0] == f64::MAX { origin = [0.0; 3]; }
    let mut b = Batch { origin, ..Batch::default() };
    for (ei, (e, m, trusted)) in meshes.iter().enumerate() {
        let base = b.verts.len() as u32;
        b.verts.extend(m.vertices.iter().map(|v| [(v[0] - origin[0]) as f32, (v[1] - origin[1]) as f32, (v[2] - origin[2]) as f32]));
        b.elements.push(*e);
        let n = m.vertices.len() as f64;
        let c = m.vertices.iter().fold([0.0; 3], |a, v| [a[0] + v[0] / n, a[1] + v[1] / n, a[2] + v[2] / n]);
        for face in &m.faces {
            for k in 1..face.len().saturating_sub(1) {
                let t = [face[0], face[k], face[k + 1]];
                let p = t.map(|i| m.vertices[i]);
                let (e1, e2) = (std::array::from_fn::<f64, 3, _>(|i| p[1][i] - p[0][i]), std::array::from_fn::<f64, 3, _>(|i| p[2][i] - p[0][i]));
                let nrm = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                let out = *trusted || (0..3).map(|i| nrm[i] * (p[0][i] - c[i])).sum::<f64>() >= 0.0;
                let len = nrm.iter().map(|x| x * x).sum::<f64>().sqrt();
                if len < 1e-12 { continue; }
                let tri = if out { [t[0], t[1], t[2]] } else { [t[0], t[2], t[1]] };
                b.tris.push(tri.map(|i| base + i as u32));
                b.tri_element.push(ei as u32);
                b.tri_face.push(if (nrm[2] / len).abs() > 0.7 { if (nrm[2] > 0.0) == out { FaceKind::Top } else { FaceKind::Bottom } } else { FaceKind::Side });
                b.tri_material.push(u32::MAX);
            }
        }
    }
    b
}

/// Grids at the lowest level and a line for each level across the extent of the grids, as in the plan views.
pub fn datums(doc: &Document) -> Vec<Datum> {
    let base = doc.elements().filter(|e| matches!(e.geometry, Some(Geometry::Level))).map(|e| e.elevation_m).fold(f64::INFINITY, f64::min);
    let base = if base.is_finite() { base } else { 0.0 };
    let mut out = Vec::new();
    let mut ends: Vec<[f64; 2]> = Vec::new();
    for el in doc.elements() {
        if let Some(Geometry::Grid { start, end }) = el.geometry {
            out.push(Datum { a: [start[0], start[1], base], b: [end[0], end[1], base], label: el.name.clone(), bubbles: true });
            ends.extend([start, end]);
        }
    }
    if ends.is_empty() { return out; }
    let left = ends.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let right = ends.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
    let back = ends.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    for el in doc.elements() {
        if matches!(el.geometry, Some(Geometry::Level)) {
            out.push(Datum { a: [left, back, el.elevation_m], b: [right, back, el.elevation_m], label: format!("{}  {:.3} m", el.name, el.elevation_m), bubbles: false });
        }
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// The storeys of the project as height bands: each level up to the next one, the top level up to `top`.
pub fn level_bands(doc: &Document, top: f64) -> Vec<(String, f64, f64)> {
    let mut lv: Vec<(String, f64)> = doc.elements().filter(|e| matches!(e.geometry, Some(Geometry::Level))).map(|e| (e.name.clone(), e.elevation_m)).collect();
    lv.sort_by(|a, b| a.1.total_cmp(&b.1));
    (0..lv.len()).map(|i| (lv[i].0.clone(), lv[i].1, lv.get(i + 1).map_or(top.max(lv[i].1), |n| n.1))).collect()
}

/// The sun of a view at the project's site: place, angle to true north and clock come from the project, date and time
/// stay the view's own.
pub fn sun_at(site: &Site, sun: openbim_viewport_egui::Sun) -> openbim_viewport_egui::Sun {
    openbim_viewport_egui::Sun { latitude: site.latitude, longitude: site.longitude, project_north_deg: site.angle_to_true_north_deg, utc_offset: site.utc_offset_hours, ..sun }
}

/// The site fields. Editing is a document transaction when the value is let go, so it is undoable and the other views
/// see it; until then the view shows the edit.
fn sun_controls(app: &mut Desktop, ui: &mut egui::Ui) {
    let Some(site) = app.document.site() else {
        if ui.button("Set project site").on_hover_text("Latitude, longitude, angle to true north and clock offset of the project").clicked() {
            let s = app.view3d.sun;
            let site = Site { latitude: s.latitude, longitude: s.longitude, angle_to_true_north_deg: s.project_north_deg, utc_offset_hours: s.utc_offset };
            app.apply_site(site);
        }
        app.view3d.sun_time_ui(ui);
        return;
    };
    let mut edit = app.site_edit.unwrap_or(site);
    let mut committed = false;
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.label("Site");
        for (label, value, range, speed, suffix) in [
            ("Lat", &mut edit.latitude, -90.0..=90.0, 0.05, "°"), ("Lon", &mut edit.longitude, -180.0..=180.0, 0.05, "°"),
            ("Angle to true north", &mut edit.angle_to_true_north_deg, -360.0..=360.0, 0.5, "°"), ("UTC+", &mut edit.utc_offset_hours, -12.0..=14.0, 0.1, ""),
        ] {
            ui.label(label);
            let r = ui.add(egui::DragValue::new(value).range(range).speed(speed).suffix(suffix));
            changed |= r.changed();
            committed |= r.drag_stopped() || r.lost_focus();
        }
    });
    if changed { app.site_edit = Some(edit); }
    if committed && edit != site { app.apply_site(edit); app.site_edit = None; }
    else if committed { app.site_edit = None; }
    let shown = app.site_edit.unwrap_or_else(|| app.document.site().unwrap_or(site));
    app.view3d.sun = sun_at(&shown, app.view3d.sun);
    app.view3d.sun_time_ui(ui);
}

impl Desktop {
    /// Set the project's site in one transaction.
    pub(crate) fn apply_site(&mut self, site: Site) {
        match self.document.transact(self.document.revision(), &[Command::SetSite(site)]) {
            Ok(_) => self.message = "Project site changed (undo restores it)".into(),
            Err(e) => self.message = format!("Site not changed: {e:?}"),
        }
    }
}

pub fn show(app: &mut Desktop, ui: &mut egui::Ui) {
    // new geometry only when the document changed
    if app.view3d_revision != Some(app.document.revision()) {
        app.view3d.set_batch(Arc::new(batch(&app.document)));
        app.view3d.datums = datums(&app.document);
        if app.view3d_revision.is_none() { app.view3d.zoom_to_fit(ui.available_width() as f64 / ui.available_height().max(1.0) as f64); }
        app.view3d_revision = Some(app.document.revision());
    }
    ui.horizontal(|ui| {
        ui.small("3D - middle-drag pan / Shift+middle-drag orbit / wheel zoom / double middle-click fit / ViewCube / click selects");
        if ui.small_button("Fit").clicked() { app.view3d.zoom_to_fit(ui.available_width() as f64 / ui.available_height().max(1.0) as f64); }
    });
    ui.horizontal_wrapped(|ui| {
        for (st, name) in [(Style::Wireframe, "Wireframe"), (Style::HiddenLine, "Hidden Line"), (Style::Shaded, "Shaded"), (Style::Realistic, "Realistic")] {
            ui.selectable_value(&mut app.view3d.style, st, name);
        }
        let bounds = app.view3d.model_bounds();
        let mut on = app.view3d.section.is_some();
        if ui.checkbox(&mut on, "Section Box").changed() { app.view3d.section = if on { bounds } else { None }; }
    });
    if app.view3d.style == Style::Realistic { sun_controls(app, ui); }
    let model = app.view3d.model_bounds();
    if let (Some(sec), Some(b)) = (&mut app.view3d.section, model) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Storey");
            for (name, lo, hi) in level_bands(&app.document, b.max[2]) {
                if ui.small_button(&name).on_hover_text(format!("{lo:.3} m to {hi:.3} m")).clicked() {
                    (sec.min, sec.max) = ([b.min[0], b.min[1], lo.max(b.min[2])], [b.max[0], b.max[1], hi.min(b.max[2])]);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            for (k, axis) in ["X", "Y", "Z"].iter().enumerate() {
                ui.label(*axis);
                let (mut lo, mut hi) = (sec.min[k], sec.max[k]);
                ui.add(egui::Slider::new(&mut lo, b.min[k]..=b.max[k]).show_value(false));
                ui.add(egui::Slider::new(&mut hi, b.min[k]..=b.max[k]).show_value(false));
                sec.min[k] = lo.min(hi);
                sec.max[k] = hi.max(lo);
            }
        });
    }
    // the selection of the app drives the view; a click in the view drives the app
    let wanted: HashSet<Element> = app.selected_id().and_then(|id| element(&app.document, id)).into_iter().collect();
    app.view3d.select(wanted.clone());
    app.view3d.ui(ui);
    if *app.view3d.selection() != wanted {
        let id = app.view3d.selection().iter().next().map(|e| ElementId(e.id));
        app.select_element(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_document_becomes_outward_facing_triangles() {
        let doc = Document::spatial_sample();
        let b = batch(&doc);
        assert!(!b.tris.is_empty() && b.elements.len() >= 3);
        // no degenerate triangles
        for (ti, t) in b.tris.iter().enumerate() {
            let p = t.map(|i| b.verts[i as usize].map(f64::from));
            let (e1, e2) = (std::array::from_fn::<f64, 3, _>(|i| p[1][i] - p[0][i]), std::array::from_fn::<f64, 3, _>(|i| p[2][i] - p[0][i]));
            let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            assert!(n.iter().any(|x| x.abs() > 1e-9), "triangle {ti} has area");
        }
        // levels and grids are not solids
        assert!(b.elements.iter().all(|e| matches!(e.kind, Kind::Wall | Kind::Segment | Kind::Family)));
    }

    #[test]
    fn each_level_is_a_band_up_to_the_next_one() {
        let b = level_bands(&Document::spatial_sample(), 6.0);
        assert_eq!(b.len(), 2);
        assert!(b[0].1 < b[0].2 && (b[0].2 - b[1].1).abs() < 1e-9, "the first band ends where the second starts: {b:?}");
        assert!((b[1].2 - 6.0).abs() < 1e-9, "the top level runs to the top of the model");
        assert!(b[1].0.contains("Level 2"));
    }

    #[test]
    fn levels_and_grids_are_datums_of_the_view() {
        let d = datums(&Document::spatial_sample());
        assert_eq!(d.iter().filter(|x| x.bubbles).count(), 6, "six grids");
        let levels: Vec<_> = d.iter().filter(|x| !x.bubbles).collect();
        assert_eq!(levels.len(), 2);
        assert!(levels.iter().any(|l| (l.a[2] - 3.0).abs() < 1e-9 && l.label.contains("Level 2")));
    }

    #[test]
    fn the_project_site_drives_the_sun_and_is_undoable() {
        let ctx = egui::Context::default();
        let mut app = Desktop::sample();
        app.view = crate::View::ThreeD;
        app.view3d.style = Style::Realistic;
        let run = |app: &mut Desktop| {
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1400.0, 700.0))), ..Default::default() },
                |ui| { egui::CentralPanel::default().show(ui, |ui| show(app, ui)); });
            out.drop_without_applying_deltas();
        };
        // the sample has a site and the view takes its place, angle and clock from it
        let site = app.document.site().expect("the sample has a site");
        run(&mut app);
        assert_eq!((app.view3d.sun.latitude, app.view3d.sun.utc_offset), (site.latitude, site.utc_offset_hours));
        // changing the site is one undoable transaction and the sun follows
        let turned = Site { angle_to_true_north_deg: 40.0, latitude: 51.5, utc_offset_hours: 0.0, ..site };
        app.apply_site(turned);
        run(&mut app);
        assert_eq!((app.view3d.sun.project_north_deg, app.view3d.sun.latitude), (40.0, 51.5));
        assert_eq!(app.document.undo(), Ok(true));
        run(&mut app);
        assert_eq!(app.view3d.sun.project_north_deg, 0.0);
        assert_eq!(app.document.site(), Some(site));
    }

    #[test]
    fn the_three_d_view_draws_and_selection_flows_both_ways() {
        let ctx = egui::Context::default();
        let mut app = Desktop::sample();
        app.view = crate::View::ThreeD;
        let run = |app: &mut Desktop| {
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1000.0, 700.0))), ..Default::default() },
                |ui| { egui::CentralPanel::default().show(ui, |ui| show(app, ui)); });
            out.drop_without_applying_deltas();
        };
        run(&mut app);
        assert!(app.view3d_revision.is_some());
        // selecting in the app (browser, Tab) shows in the view
        let wall = app.document.elements().find(|e| e.category == Category::Wall && app.document.mesh(e.id).is_some()).unwrap().id;
        app.select_element(Some(wall));
        run(&mut app);
        assert_eq!(app.view3d.selection().len(), 1);
        assert!(app.view3d.selection().iter().all(|e| e.id == wall.0));
    }
}
