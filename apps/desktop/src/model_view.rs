//! Orthographic plan and orbitable 3D projection of document-derived meshes.
//! Painter-based CPU projection is an initial small-model renderer, not a full
//! depth-buffered GPU BIM engine (cut planes/joins/hidden-line are future work).
use crate::{Desktop, ElementId, View};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use openbim_document::{Category, Geometry};

fn project(point: [f64; 3], three_d: bool, yaw: f32, pitch: f32) -> (Vec2, f32) {
    let [x, y, z] = point.map(|v| v as f32);
    if !three_d { return (Vec2::new(x, -y), z); }
    let horizontal = x * yaw.sin() + y * yaw.cos();
    (Vec2::new(x * yaw.cos() - y * yaw.sin(), horizontal * pitch.sin() - z * pitch.cos()),
        horizontal * pitch.cos() + z * pitch.sin())
}

fn contains(points: &[Pos2], p: Pos2) -> bool {
    if points.len() < 3 { return false; }
    let mut inside = false;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        let a = points[i]; let b = points[j];
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-10)).clamp(0.0, 1.0);
    (p - (a + t * ab)).length()
}

pub fn show(app: &mut Desktop, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for (label, view) in [("Level 1", View::Plan), ("Level 2", View::UpperPlan),
            ("{3D}", View::ThreeD), ("M-101 - Mechanical", View::Mechanical)] {
            if ui.selectable_label(app.view == view, label).clicked() { app.view = view; }
        }
        if ui.small_button("Fit").clicked() { app.pan = Vec2::ZERO; app.zoom = 1.0; }
        if ui.small_button("Clear selection").clicked() { app.select_element(None); }
    });
    let level_id = ElementId(if app.view == View::UpperPlan { 11 } else { 10 });
    let level_z = app.document.get(level_id).map_or(0.0, |el| el.elevation_m);
    let three_d = app.view == View::ThreeD;
    ui.horizontal(|ui| {
        ui.small(if three_d { "3D — right-drag orbit / middle-drag pan / wheel zoom" }
            else { "Plan footprint — middle-drag pan / wheel zoom" });
        if !three_d && ui.small_button(format!("Level: {level_z:.3} m")).clicked() {
            app.select_element(Some(level_id));
        }
    });
    let (response, painter) = ui.allocate_painter(ui.available_size().max(Vec2::splat(50.0)), Sense::click_and_drag());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    if response.dragged_by(egui::PointerButton::Middle) { app.pan += response.drag_delta(); }
    if three_d && response.dragged_by(egui::PointerButton::Secondary) {
        app.camera_yaw += response.drag_delta().x * 0.01;
        app.camera_pitch = (app.camera_pitch + response.drag_delta().y * 0.01).clamp(0.12, 1.45);
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        app.zoom = (app.zoom * (scroll * 0.001).exp()).clamp(0.2, 8.0);
    }
    let proj = |p| project(p, three_d, app.camera_yaw, app.camera_pitch);
    let mut bounds = Rect::NOTHING;
    let mut meshes = Vec::new();
    for el in app.document.elements() {
        if let Some(Geometry::Linear { level, .. }) = &el.geometry {
            if !three_d && *level != level_id { continue; }
            if let Some(mesh) = app.document.mesh(el.id) {
                for vertex in &mesh.vertices { bounds.extend_with(Pos2::ZERO + proj(*vertex).0); }
                meshes.push((el.id, el.category, mesh));
            }
        }
    }
    // Include datum extents so grid bubbles remain inside the viewport.
    for el in app.document.elements() {
        if let Some(Geometry::Grid { start, end }) = el.geometry {
            for [x, y] in [start, end] { bounds.extend_with(Pos2::ZERO + proj([x, y, level_z]).0); }
        }
    }
    if !bounds.is_finite() { return; }
    let fit = ((rect.width() - 100.0).max(10.0) / bounds.width().max(1.0))
        .min((rect.height() - 90.0).max(10.0) / bounds.height().max(1.0));
    let scale = fit * app.zoom;
    let screen = |p| rect.center() + app.pan + (proj(p).0 - bounds.center().to_vec2()) * scale;
    let selected = app.selected_id();
    let blue = ui.visuals().hyperlink_color;
    let axes: Vec<_> = app.document.elements().flat_map(|el| match el.geometry {
        Some(Geometry::Grid { start, end } | Geometry::Linear { start, end, .. }) => vec![start, end],
        _ => vec![],
    }).collect();
    let left = axes.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let right = axes.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
    let back = axes.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    let mut datum_hits = Vec::new();
    for el in app.document.elements() {
        let active = selected == Some(el.id);
        let stroke = Stroke::new(if active { 2.5 } else { 1.0 }, if active { blue } else { Color32::from_gray(135) });
        match el.geometry {
            Some(Geometry::Grid { start, end }) => {
                let a = screen([start[0], start[1], level_z]);
                let b = screen([end[0], end[1], level_z]);
                dashed(&painter, a, b, stroke);
                for p in [a, b] {
                    painter.circle(p, 11.0, ui.visuals().panel_fill, stroke);
                    painter.text(p, egui::Align2::CENTER_CENTER, &el.name, egui::FontId::proportional(12.0), stroke.color);
                }
                datum_hits.push((el.id, a, b));
            }
            Some(Geometry::Level) if three_d => {
                let a = screen([left, back, el.elevation_m]);
                let b = screen([right, back, el.elevation_m]);
                dashed(&painter, a, b, stroke);
                painter.text(a, egui::Align2::LEFT_BOTTOM, format!("{}  {:.3} m", el.name, el.elevation_m),
                    egui::FontId::proportional(12.0), stroke.color);
                datum_hits.push((el.id, a, b));
            }
            _ => {}
        }
    }
    let mut faces = Vec::new();
    for (id, category, mesh) in meshes {
        let color = match category {
            Category::Wall => if app.dark { Color32::from_gray(115) } else { Color32::from_gray(195) },
            Category::Duct => Color32::from_rgb(70, 157, 194),
            Category::Pipe => Color32::from_rgb(136, 153, 166),
            _ => Color32::GRAY,
        };
        for (i, face) in mesh.faces.iter().enumerate() {
            let points: Vec<_> = face.iter().map(|&v| screen(mesh.vertices[v])).collect();
            let area = points.iter().zip(points.iter().cycle().skip(1)).take(points.len())
                .map(|(a, b)| a.x * b.y - b.x * a.y).sum::<f32>().abs();
            if area < 0.2 { continue; }
            let depth = face.iter().map(|&v| proj(mesh.vertices[v]).1).sum::<f32>() / face.len() as f32;
            faces.push((depth, id, points, color.gamma_multiply(if three_d { 0.72 + (i % 3) as f32 * 0.12 } else { 1.0 })));
        }
    }
    faces.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, id, points, color) in &faces {
        painter.add(egui::Shape::convex_polygon(points.clone(), *color,
            Stroke::new(if selected == Some(*id) { 2.0 } else { 0.8 },
                if selected == Some(*id) { blue } else { Color32::from_gray(80) })));
    }
    if response.clicked() {
        if let Some(p) = response.interact_pointer_pos() {
            let hit = faces.iter().rev().find(|(_, _, polygon, _)| contains(polygon, p)).map(|(_, id, _, _)| *id)
                .or_else(|| datum_hits.iter().rev().find(|(_, a, b)| segment_distance(p, *a, *b) < 7.0
                    || p.distance(*a) < 12.0 || p.distance(*b) < 12.0).map(|(id, _, _)| *id));
            app.select_element(hit);
        }
    }
    painter.text(rect.left_bottom() + Vec2::new(8.0, -8.0), egui::Align2::LEFT_BOTTOM,
        "Document geometry • metres • in memory", egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
}

fn dashed(painter: &egui::Painter, a: Pos2, b: Pos2, stroke: Stroke) {
    let delta = b - a;
    let length = delta.length();
    if length <= 0.0 { return; }
    let direction = delta / length;
    let mut t = 0.0;
    while t < length && t < 100_000.0 {
        painter.line_segment([a + direction * t, a + direction * (t + 7.0).min(length)], stroke);
        t += 12.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn polygon_picking_rejects_empty_plan_space() {
        let polygon = [Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0), Pos2::new(10.0, 2.0), Pos2::new(0.0, 2.0)];
        assert!(contains(&polygon, Pos2::new(5.0, 1.0)));
        assert!(!contains(&polygon, Pos2::new(5.0, 5.0)));
        assert_eq!(project([1.0, 2.0, 3.0], false, 0.0, 0.0).0, Vec2::new(1.0, -2.0));
    }

    #[test]
    fn document_views_render_in_both_themes_at_multiple_display_scales() {
        let ctx = egui::Context::default();
        let mut app = Desktop::sample();
        let before = app.document.clone();
        for view in [View::Plan, View::UpperPlan, View::ThreeD, View::Mechanical] {
            app.view = view;
            for scale in [1.0, 1.5, 2.0] {
                ctx.set_pixels_per_point(scale);
                for dark in [false, true] {
                    app.dark = dark;
                    ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
                    let mut output = ctx.run_ui(egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))),
                        ..Default::default()
                    }, |ui| { egui::CentralPanel::default().show(ui, |ui| show(&mut app, ui)); });
                    assert!(output.shapes.len() > 20);
                    // Exercise tessellation too, including edge-on plan faces.
                    let meshes = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
                    assert!(!meshes.is_empty());
                    output.textures_delta.clear();
                }
            }
        }
        assert_eq!(app.document, before, "rendering must not mutate geometry");
    }
}
