//! The {3D} view: the document's meshes drawn by the wgpu viewport widget (Revit mouse mapping, ViewCube, visual
//! styles, section box, click selection).
use crate::Desktop;
use eframe::egui;
use openbim_document::{Category, Document, ElementId};
use openbim_view::{Batch, Element, FaceKind, Kind};
use openbim_viewport_egui::Style;
use std::collections::HashSet;
use std::sync::Arc;

fn kind(c: Category) -> Option<Kind> {
    match c { Category::Wall => Some(Kind::Wall), Category::Duct | Category::Pipe => Some(Kind::Segment), Category::Equipment => Some(Kind::Family), _ => None }
}

/// The element of the view that stands for a document element.
fn element(doc: &Document, id: ElementId) -> Option<Element> {
    Some(Element { kind: kind(doc.get(id)?.category)?, id: id.0 })
}

/// All meshes of the document as one batch. Faces are wound outwards (the solids are convex).
pub fn batch(doc: &Document) -> Batch {
    let mut meshes = Vec::new();
    for el in doc.elements() {
        let (Some(k), Some(mesh)) = (kind(el.category), doc.mesh(el.id)) else { continue };
        meshes.push((Element { kind: k, id: el.id.0 }, mesh));
    }
    let mut origin = [f64::MAX; 3];
    for (_, m) in &meshes { for v in &m.vertices { for k in 0..3 { origin[k] = origin[k].min(v[k]); } } }
    if origin[0] == f64::MAX { origin = [0.0; 3]; }
    let mut b = Batch { origin, ..Batch::default() };
    for (ei, (e, m)) in meshes.iter().enumerate() {
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
                let out = (0..3).map(|i| nrm[i] * (p[0][i] - c[i])).sum::<f64>() >= 0.0;
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

pub fn show(app: &mut Desktop, ui: &mut egui::Ui) {
    // new geometry only when the document changed
    if app.view3d_revision != Some(app.document.revision()) {
        app.view3d.set_batch(Arc::new(batch(&app.document)));
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
    let model = app.view3d.model_bounds();
    if let (Some(sec), Some(b)) = (&mut app.view3d.section, model) {
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
