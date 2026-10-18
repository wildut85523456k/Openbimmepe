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
        let added: Vec<(View, String)> = app.levels().into_iter().filter(|l| l.0 != ElementId(10) && l.0 != ElementId(11)).map(|l| (View::Level(l.0.0), l.1)).collect();
        let tabs = [("Level 1".to_string(), View::Plan), ("Level 2".to_string(), View::UpperPlan)].into_iter()
            .chain(added.into_iter().map(|(v, n)| (n, v)))
            .chain([("{3D}".to_string(), View::ThreeD), ("M-101 - Mechanical".to_string(), View::Mechanical)]);
        for (label, view) in tabs {
            if ui.selectable_label(app.view == view, label).clicked() { app.view = view; }
        }
        if ui.small_button("Fit").clicked() { app.pan = Vec2::ZERO; app.zoom = 1.0; }
        if ui.small_button("Clear selection").clicked() { app.select_element(None); }
    });
    // Spacebar turns the selected wall over
    if app.tool == crate::Tool::Select && !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Space)) { app.flip_selected_wall(); }
    // Delete removes the selected element with what stands on it (Backspace is the Delete key of a Mac keyboard)
    if app.tool == crate::Tool::Select && !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) { app.delete_selected(); }
    if app.view == View::ThreeD { return crate::view3d::show(app, ui); }
    let level_id = app.plan_level();
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
    let (yaw0, pitch0, pan0) = (app.camera_yaw, app.camera_pitch, app.pan);
    let proj = |p| project(p, three_d, yaw0, pitch0);
    let mut bounds = Rect::NOTHING;
    let mut meshes = Vec::new();
    for el in app.document.elements() {
        // a footing hosted by a wall is on the wall's level
        let hosted = match el.geometry { Some(Geometry::OnWall { host }) => app.document.get(host).and_then(|w| match w.geometry { Some(Geometry::Linear { level, .. }) => Some(level), _ => None }), _ => None };
        if let Some(level) = hosted.as_ref().or(match &el.geometry { Some(Geometry::Linear { level, .. } | Geometry::Point { level, .. } | Geometry::Area { level } | Geometry::Pad { level, .. }) => Some(level), _ => None }) {
            if !three_d && *level != level_id { continue; }
            if let Some(mesh) = app.document.mesh(el.id) {
                for vertex in &mesh.vertices { bounds.extend_with(Pos2::ZERO + proj(*vertex).0); }
                meshes.push((el.id, el.category, mesh));
            }
        }
    }
    // family instances, and the walls and members they cut, come from the scene mesher
    if let Some(scene) = crate::view3d::scene_meshes(&app.document) {
        for em in scene.iter() {
            let id = ElementId(em.element.id);
            let Some(el) = app.document.get(id) else { continue };
            let level = match el.geometry {
                Some(Geometry::Placed { level, .. } | Geometry::Linear { level, .. } | Geometry::Point { level, .. } | Geometry::Area { level }) => Some(level),
                Some(Geometry::InWall { host, .. }) => app.document.get(host).and_then(|w| match w.geometry { Some(Geometry::Linear { level, .. }) => Some(level), _ => None }),
                _ => None,
            };
            let Some(level) = level else { continue };
            if !three_d && level != level_id { continue; }
            let mesh = crate::view3d::doc_mesh(em);
            for vertex in &mesh.vertices { bounds.extend_with(Pos2::ZERO + proj(*vertex).0); }
            match meshes.iter().position(|(m, _, _)| *m == id) { Some(i) => meshes[i] = (id, el.category, mesh), None => meshes.push((id, el.category, mesh)) }
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
    let screen = |p| rect.center() + pan0 + (proj(p).0 - bounds.center().to_vec2()) * scale;
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
            Category::Family => Color32::from_rgb(120, 150, 110),
            Category::Beam | Category::BeamSystem | Category::Truss => Color32::from_rgb(176, 140, 104),
            Category::Column => Color32::from_rgb(150, 150, 160),
            Category::Footing | Category::WallFooting => Color32::from_rgb(160, 150, 135),
            Category::Slab => if app.dark { Color32::from_rgb(92, 92, 100) } else { Color32::from_rgb(205, 205, 212) },
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
    if !three_d {
        let report = app.compute.report.as_ref().map(|(_, r)| r);
        crate::compute_ui::draw_overlay(&app.document, report, &painter, &screen, level_z, app.tool == crate::Tool::EndPoint);
    }
    if !three_d {
        let report = app.elec.report.as_ref().map(|(_, r)| r);
        let pending = app.cable_from.map(|f| (f, app.cable_via.as_slice()));
        crate::elec_ui::draw_overlay(&app.document, report, &painter, &screen, level_id, level_z, pending);
    }
    if !three_d {
        // tags: the label of an element, boxed, at a constant size, on a leader to the place that was clicked
        let tag_ids: Vec<ElementId> = app.document.elements().filter(|e| e.category == Category::Tag).map(|e| e.id).collect();
        for id in tag_ids {
            let Some(Geometry::Tag { host, at, anchor }) = app.document.get(id).and_then(|t| t.geometry) else { continue };
            if app.document.level_of(host) != Some(level_id) { continue; }
            let (head, tip) = (screen([at[0], at[1], level_z]), screen([anchor[0], anchor[1], level_z]));
            let text = app.document.tag_text(id);
            let ink = ui.visuals().text_color();
            if head.distance(tip) > 2.0 { painter.line_segment([head, tip], Stroke::new(1.0, ink)); painter.circle_filled(tip, 2.5, ink); }
            let galley = painter.layout_no_wrap(text, egui::FontId::proportional(12.0), ink);
            let r = Rect::from_center_size(head, galley.size() + Vec2::new(10.0, 6.0));
            painter.rect_filled(r, 2.0, ui.visuals().panel_fill);
            painter.rect_stroke(r, 2.0, Stroke::new(1.0, ink), egui::StrokeKind::Middle);
            painter.galley(r.center() - galley.size() / 2.0, galley, ink);
        }
    }
    // Revit's way of picking a point: a click, or press at the start, drag, release at the end. A drag
    // starting a tool with nothing pending picks its start point at the press and its next point at the release.
    let drawing = !three_d && !matches!(app.tool, crate::Tool::Select | crate::Tool::Rebar | crate::Tool::WallFooting | crate::Tool::Level);
    let pick: Option<Pos2> = if !drawing { None } else if response.clicked() { response.interact_pointer_pos() }
        else if response.drag_started_by(egui::PointerButton::Primary) {
            let starts = matches!(app.tool, crate::Tool::Wall | crate::Tool::Grid | crate::Tool::Beam | crate::Tool::Truss | crate::Tool::Duct | crate::Tool::Pipe | crate::Tool::Slab | crate::Tool::BeamSystem | crate::Tool::Cable | crate::Tool::Move | crate::Tool::Copy | crate::Tool::Mirror);
            let pending = !app.edit_pts.is_empty() || app.beam_start.is_some() || !app.wall_pts.is_empty() || !app.slab_points.is_empty() || !app.run_points.is_empty() || app.cable_from.is_some();
            if starts && !pending { ui.input(|i| i.pointer.press_origin()) } else { None }
        }
        else if response.drag_stopped_by(egui::PointerButton::Primary) { response.interact_pointer_pos().or(response.hover_pos()) }
        else { None };
    if matches!(app.tool, crate::Tool::ElecNode | crate::Tool::Cable) && !three_d {
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [v.x as f64, -v.y as f64] };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { if app.cable_from.is_some() { app.cable_from = None; app.cable_via.clear(); } else { app.choose_tool(crate::Tool::Select); } }
        else if pick.is_some() {
            if let Some(p) = pick { if app.tool == crate::Tool::ElecNode { app.elec_node_click(level_id, to_model(p)); } else { app.cable_click(level_id, to_model(p)); } }
        }
    } else if app.tool == crate::Tool::Wall && !three_d {
        // As in Revit: pick the start, then each next point (a chain), or press, drag and release for one wall; the
        // line drawn is the Location Line. The pointer snaps to the corners of walls, the ends and crossings of grids
        // and to grid lines (a square marks it), else to 50 mm. Shift keeps the wall square, digits type its length in
        // mm and Enter draws it, Backspace takes a digit back, and a pick on the first point closes the loop.
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let mut points: Vec<[f64; 2]> = Vec::new();
        let mut grids: Vec<([f64; 2], [f64; 2])> = Vec::new();
        for el in app.document.elements() {
            match el.geometry {
                Some(Geometry::Linear { start, end, level }) if el.category == Category::Wall && level == level_id => {
                    points.extend([start, end]);
                    if let Some(r) = app.document.wall_plan(el.id) { points.extend(r); }
                }
                Some(Geometry::Grid { start, end }) => { points.extend([start, end]); grids.push((start, end)); }
                _ => {}
            }
        }
        for (i, (a, b)) in grids.iter().enumerate() { for (c, d) in &grids[i + 1..] {
            if let Some(p) = openbim_document::line_meet(*a, [b[0] - a[0], b[1] - a[1]], *c, [d[0] - c[0], d[1] - c[1]]) {
                let on = |s: &([f64; 2], [f64; 2])| { let (dx, dy) = (s.1[0] - s.0[0], s.1[1] - s.0[1]); let t = ((p[0] - s.0[0]) * dx + (p[1] - s.0[1]) * dy) / (dx * dx + dy * dy); (-1e-9..=1.0 + 1e-9).contains(&t) };
                if on(&(*a, *b)) && on(&(*c, *d)) { points.push(p); }
            }
        } }
        if let Some(&p) = app.wall_pts.first() { points.push(p); }
        let ortho = ui.input(|i| i.modifiers.shift);
        let start = app.wall_pts.last().copied();
        let snap_to = |p: Pos2| -> ([f64; 2], bool) {
            let v = (p - centre - pan) / scale + bc;
            let raw = [v.x as f64, -v.y as f64];
            if let Some(c) = points.iter().filter(|c| screen([c[0], c[1], level_z]).distance(p) < 12.0)
                .min_by(|a, b| screen([a[0], a[1], level_z]).distance(p).total_cmp(&screen([b[0], b[1], level_z]).distance(p))) { return (*c, true); }
            for (a, b) in &grids {
                let (sa, sb) = (screen([a[0], a[1], level_z]), screen([b[0], b[1], level_z]));
                if segment_distance(p, sa, sb) < 8.0 {
                    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                    let t = (((raw[0] - a[0]) * dx + (raw[1] - a[1]) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                    return ([a[0] + dx * t, a[1] + dy * t], true);
                }
            }
            let mut m = [(raw[0] / 0.05).round() * 0.05, (raw[1] / 0.05).round() * 0.05];
            if let (true, Some(s)) = (ortho, start) { if (m[0] - s[0]).abs() >= (m[1] - s[1]).abs() { m[1] = s[1]; } else { m[0] = s[0]; } }
            (m, false)
        };
        let to_model = |p: Pos2| snap_to(p).0;
        let hover = response.hover_pos();
        if ui.input(|i| i.key_pressed(egui::Key::Space)) { app.wall_flip = !app.wall_flip; app.message = format!("Wall {}", if app.wall_flip { "flipped: the other face is the reference" } else { "unflipped" }); }
        if let (1, Some(a), Some(h)) = (app.wall_mode, app.wall_pts.first().copied(), hover) {
            let (b, _) = snap_to(h);
            if (b[0] - a[0]).abs() > 1e-6 && (b[1] - a[1]).abs() > 1e-6 {
                for (s, t) in app.rectangle_lines(a, b) {
                    let (dx, dy) = (t[0] - s[0], t[1] - s[1]);
                    let l = dx.hypot(dy).max(1e-9);
                    let (nx, ny) = (-dy / l * app.width_mm / 2000.0, dx / l * app.width_mm / 2000.0);
                    let ring = [[s[0] - nx, s[1] - ny], [t[0] - nx, t[1] - ny], [t[0] + nx, t[1] + ny], [s[0] + nx, s[1] + ny]].map(|p| screen([p[0], p[1], level_z]));
                    painter.add(egui::Shape::convex_polygon(ring.to_vec(), Color32::from_rgba_unmultiplied(120, 120, 130, 110), Stroke::new(1.0, blue)));
                }
                painter.text(screen([b[0], b[1], level_z]) + Vec2::new(10.0, -10.0), egui::Align2::LEFT_BOTTOM, format!("{:.0} x {:.0} mm", (b[0] - a[0]).abs() * 1000.0, (b[1] - a[1]).abs() * 1000.0), egui::FontId::proportional(13.0), blue);
            }
        }
        if let (0, Some(a), Some(h)) = (app.wall_mode, start, hover) {
            let (mut b, _) = snap_to(h);
            // a typed length keeps the direction of the pointer
            if let Ok(mm) = app.wall_len.parse::<f64>() {
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let l = dx.hypot(dy).max(1e-9);
                b = [a[0] + dx / l * mm / 1000.0, a[1] + dy / l * mm / 1000.0];
            }
            let (pa, pb) = app.wall_axis(a, b);
            let (dx, dy) = (pb[0] - pa[0], pb[1] - pa[1]);
            let l = dx.hypot(dy);
            if l > 1e-6 {
                let (nx, ny) = (-dy / l * app.width_mm / 2000.0, dx / l * app.width_mm / 2000.0);
                let ring = [[pa[0] - nx, pa[1] - ny], [pb[0] - nx, pb[1] - ny], [pb[0] + nx, pb[1] + ny], [pa[0] + nx, pa[1] + ny]].map(|p| screen([p[0], p[1], level_z]));
                painter.add(egui::Shape::convex_polygon(ring.to_vec(), Color32::from_rgba_unmultiplied(120, 120, 130, 110), Stroke::new(1.0, blue)));
                dashed(&painter, screen([a[0], a[1], level_z]), screen([b[0], b[1], level_z]), Stroke::new(1.0, blue));
                let shown = if app.wall_len.is_empty() { format!("{:.0}", (b[0] - a[0]).hypot(b[1] - a[1]) * 1000.0) } else { format!("{}_", app.wall_len) };
                painter.text(screen([b[0], b[1], level_z]) + Vec2::new(10.0, -10.0), egui::Align2::LEFT_BOTTOM, format!("{shown} mm"), egui::FontId::proportional(13.0), blue);
            }
        }
        if let Some(h) = hover {
            let (m, snapped) = snap_to(h);
            if snapped { painter.rect_stroke(Rect::from_center_size(screen([m[0], m[1], level_z]), Vec2::splat(9.0)), 0.0, Stroke::new(1.5, Color32::from_rgb(220, 140, 0)), egui::StrokeKind::Middle); }
        }
        let typed: String = ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None }).collect());
        if app.wall_pts.last().is_some() { for c in typed.chars().filter(|c| c.is_ascii_digit() || *c == '.') { app.wall_len.push(c); } }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if !app.wall_pts.is_empty() { app.wall_pts.clear(); app.wall_ids.clear(); app.wall_len.clear(); app.message = "Wall: chain ended; Esc again leaves the tool".into(); } else { app.choose_tool(crate::Tool::Select); }
        } else if ui.input(|i| i.key_pressed(egui::Key::Backspace)) && !app.wall_len.is_empty() { app.wall_len.pop();
        } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !app.wall_len.is_empty() {
            if let (Some(a), Some(h), Ok(mm)) = (start, hover, app.wall_len.parse::<f64>()) {
                let (b, _) = snap_to(h);
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let l = dx.hypot(dy);
                if l > 1e-9 && mm > 0.0 { app.wall_click(level_id, [a[0] + dx / l * mm / 1000.0, a[1] + dy / l * mm / 1000.0]); }
            }
        }
        else if let Some(p) = pick {
            if app.wall_mode == 2 {
                let hit = faces.iter().rev().find(|(_, id, polygon, _)| contains(polygon, p) && app.document.line_of(*id).is_some()).map(|(_, id, _, _)| *id)
                    .or_else(|| datum_hits.iter().rev().find(|(_, a, b)| segment_distance(p, *a, *b) < 7.0).map(|(id, _, _)| *id));
                match hit { Some(h) => app.wall_pick_line(level_id, h, to_model(p)), None => app.message = "Pick Lines: click a grid, a wall or a beam".into() }
            } else { app.wall_click(level_id, to_model(p)); }
        }
    } else if matches!(app.tool, crate::Tool::Move | crate::Tool::Copy | crate::Tool::Rotate | crate::Tool::Mirror) && !three_d {
        // As in Revit: the element is selected first (a click selects one when none is), then the tool picks its points:
        // a base point and where to, a centre and the two rays of a turn, the two points of a mirror axis. The pointer
        // snaps to the corners of walls and the ends of grids, else to 50 mm; Shift keeps a move square; a ghost shows the result.
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let raw = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [v.x as f64, -v.y as f64] };
        let corners: Vec<[f64; 2]> = app.document.elements().flat_map(|el| match el.geometry {
            Some(Geometry::Linear { start, end, level }) if el.category == Category::Wall && level == level_id => { let mut v = vec![start, end]; if let Some(r) = app.document.wall_plan(el.id) { v.extend(r); } v }
            Some(Geometry::Grid { start, end }) => vec![start, end],
            _ => vec![],
        }).collect();
        let ortho = ui.input(|i| i.modifiers.shift);
        let first = app.edit_pts.first().copied();
        let snap_to = |p: Pos2| -> [f64; 2] {
            if let Some(c) = corners.iter().filter(|c| screen([c[0], c[1], level_z]).distance(p) < 12.0)
                .min_by(|a, b| screen([a[0], a[1], level_z]).distance(p).total_cmp(&screen([b[0], b[1], level_z]).distance(p))) { return *c; }
            let r = raw(p);
            let mut m = [(r[0] / 0.05).round() * 0.05, (r[1] / 0.05).round() * 0.05];
            if let (true, Some(s)) = (ortho, first) { if (m[0] - s[0]).abs() >= (m[1] - s[1]).abs() { m[1] = s[1]; } else { m[0] = s[0]; } }
            m
        };
        let typed: String = ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None }).collect());
        if !app.edit_pts.is_empty() { for c in typed.chars() { if c.is_ascii_digit() || c == '.' || (c == '-' && app.edit_num.is_empty()) { app.edit_num.push(c); } } }
        let hover = response.hover_pos();
        // the ghost: the selected element's outline after the change the next pick would make
        if let (Some(sel), Some(h)) = (app.edit_src.or(selected), hover) {
            let next = snap_to(h);
            let xf = match app.edit_num.parse::<f64>() {
                Ok(v) if app.tool == crate::Tool::Rotate => app.edit_pts.first().map(|c| openbim_document::Xf::Rotate(*c, v.to_radians())),
                Ok(v) => first.and_then(|a| { let (dx, dy) = (next[0] - a[0], next[1] - a[1]); let l = dx.hypot(dy); (l > 1e-9).then(|| openbim_document::Xf::Move([dx / l * v / 1000.0, dy / l * v / 1000.0])) }),
                Err(_) => app.edit_xf(app.tool, next),
            };
            let ghost = Stroke::new(1.5, Color32::from_rgb(220, 140, 0));
            if let Some(xf) = xf {
                for (_, id, points, _) in faces.iter().filter(|f| f.1 == sel) {
                    let moved: Vec<Pos2> = points.iter().map(|q| { let m = xf.point(raw(*q)); screen([m[0], m[1], level_z]) }).collect();
                    painter.add(egui::Shape::convex_polygon(moved, Color32::from_rgba_unmultiplied(220, 140, 0, 40), ghost));
                    let _ = id;
                }
            }
            if let Some(a) = first { painter.line_segment([screen([a[0], a[1], level_z]), screen([next[0], next[1], level_z])], Stroke::new(1.0, blue)); }
            if let (crate::Tool::Rotate, [c, r]) = (app.tool, app.edit_pts.as_slice()) { painter.line_segment([screen([c[0], c[1], level_z]), screen([r[0], r[1], level_z])], Stroke::new(1.0, blue)); }
            if !app.edit_num.is_empty() { painter.text(h + Vec2::new(12.0, -12.0), egui::Align2::LEFT_BOTTOM, format!("{}_", app.edit_num), egui::FontId::proportional(13.0), blue); }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { if app.edit_pts.is_empty() { app.choose_tool(crate::Tool::Select); } else { app.edit_pts.clear(); app.edit_num.clear(); } }
        else if ui.input(|i| i.key_pressed(egui::Key::Backspace)) && !app.edit_num.is_empty() { app.edit_num.pop(); }
        else if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !app.edit_num.is_empty() { if let Some(h) = hover { let t = snap_to(h); app.edit_enter(t); } }
        else if let Some(p) = pick {
            if selected.is_none() {
                let hit = faces.iter().rev().find(|(_, _, polygon, _)| contains(polygon, p)).map(|(_, id, _, _)| *id);
                app.select_element(hit);
                app.message = if hit.is_some() { "Now pick the points".into() } else { "Click an element to work on".into() };
            } else { app.edit_click(snap_to(p)); }
        } else if selected.is_none() && response.clicked() {
            if let Some(p) = response.interact_pointer_pos() { let hit = faces.iter().rev().find(|(_, _, polygon, _)| contains(polygon, p)).map(|(_, id, _, _)| *id); app.select_element(hit); }
        }
    } else if matches!(app.tool, crate::Tool::Align | crate::Tool::Trim | crate::Tool::Offset) && !three_d {
        // Revit's pick tools: Align takes the line to align to and then the element to move, Trim/Extend two elements
        // on the parts to keep, Offset one element on the side to offset to.
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [v.x as f64, -v.y as f64] };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { if app.edit_ref.is_some() { app.edit_ref = None; app.message = "Pick the first element".into(); } else { app.choose_tool(crate::Tool::Select); } }
        else if response.clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                let hit = faces.iter().rev().find(|(_, _, polygon, _)| contains(polygon, p)).map(|(_, id, _, _)| *id)
                    .or_else(|| datum_hits.iter().rev().find(|(_, a, b)| segment_distance(p, *a, *b) < 7.0).map(|(id, _, _)| *id));
                app.modify_click(hit, to_model(p));
            }
        }
    } else if app.tool == crate::Tool::Tag && !three_d {
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [v.x as f64, -v.y as f64] };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); }
        else if let Some(p) = pick {
            let hit = faces.iter().rev().find(|(_, id, polygon, _)| contains(polygon, p) && app.document.get(*id).is_some()).map(|(_, id, _, _)| *id);
            match hit { Some(id) => app.tag_click(id, to_model(p)), None => app.message = "Tag: click an element".into() }
        }
    } else if app.tool == crate::Tool::Grid && !three_d {
        // two picks (or press, drag, release) draw a grid line, snapped to 50 mm and to the ends of other grid lines
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let ends: Vec<[f64; 2]> = app.document.elements().flat_map(|el| match el.geometry { Some(Geometry::Grid { start, end }) => vec![start, end], _ => vec![] }).collect();
        let ortho = ui.input(|i| i.modifiers.shift);
        let start = app.beam_start;
        let to_model = |p: Pos2| {
            let v = (p - centre - pan) / scale + bc;
            let mut m = [(v.x as f64 / 0.05).round() * 0.05, (-v.y as f64 / 0.05).round() * 0.05];
            if let Some(c) = ends.iter().find(|c| screen([c[0], c[1], level_z]).distance(p) < 12.0) { m = *c; }
            else if let (true, Some(s)) = (ortho, start) { if (m[0] - s[0]).abs() >= (m[1] - s[1]).abs() { m[1] = s[1]; } else { m[0] = s[0]; } }
            m
        };
        if let (Some(a), Some(h)) = (start, response.hover_pos()) {
            let b = to_model(h);
            let (sa, sb) = (screen([a[0], a[1], level_z]), screen([b[0], b[1], level_z]));
            dashed(&painter, sa, sb, Stroke::new(1.5, blue));
            for p in [sa, sb] { painter.circle_stroke(p, 11.0, Stroke::new(1.0, blue)); }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { if app.beam_start.is_some() { app.beam_start = None; } else { app.choose_tool(crate::Tool::Select); } }
        else if let Some(p) = pick { app.grid_click(to_model(p)); }
    } else if app.tool == crate::Tool::Level && !three_d {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); }
        else if response.clicked() { app.level_click(); }
    } else if app.tool == crate::Tool::EndPoint && !three_d {
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [v.x as f64, -v.y as f64] };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); }
        else if pick.is_some() { if let Some(p) = pick { app.end_point_click(level_id, to_model(p)); } }
    } else if app.tool == crate::Tool::Family && !three_d {
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [(v.x as f64 / 0.05).round() * 0.05, (-v.y as f64 / 0.05).round() * 0.05] };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); }
        else if pick.is_some() {
            if let Some(p) = pick {
                let wall = faces.iter().rev().find(|(_, id, polygon, _)| contains(polygon, p) && app.document.get(*id).is_some_and(|e| e.category == Category::Wall)).map(|(_, id, _, _)| *id);
                app.family_click(level_id, to_model(p), wall);
            }
        }
    } else if app.tool == crate::Tool::Rebar && !three_d {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); }
        else if pick.is_some() {
            if let Some(p) = pick {
                let hit = faces.iter().rev().find(|(_, _, polygon, _)| contains(polygon, p)).map(|(_, id, _, _)| *id);
                match hit { Some(id) => app.rebar_click(id), None => app.message = "Rebar: click a beam".into() }
            }
        }
    } else if app.tool == crate::Tool::WallFooting && !three_d {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); }
        else if pick.is_some() {
            if let Some(p) = pick {
                let hit = faces.iter().rev().find(|(_, _, polygon, _)| contains(polygon, p)).map(|(_, id, _, _)| *id);
                match hit { Some(id) => app.wall_footing_click(id), None => app.message = "Wall footing: click a wall".into() }
            }
        }
    } else if app.tool == crate::Tool::Footing && !three_d {
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [(v.x as f64 / 0.05).round() * 0.05, (-v.y as f64 / 0.05).round() * 0.05] };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); }
        else if pick.is_some() { if let Some(p) = pick { app.footing_click(level_id, to_model(p)); } }
    } else if matches!(app.tool, crate::Tool::Slab | crate::Tool::BeamSystem) && !three_d {
        // corners snapped to 50 mm; Enter or a click on the first corner closes the outline
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let snap = |v: f32| (v as f64 / 0.05).round() * 0.05;
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [snap(v.x), snap(-v.y)] };
        let pts: Vec<Pos2> = app.slab_points.iter().map(|p| screen([p[0], p[1], level_z])).collect();
        let stroke = Stroke::new(2.0, Color32::from_rgb(70, 130, 200));
        if let (Some(last), Some(h)) = (pts.last(), response.hover_pos()) {
            painter.line_segment([*last, h], Stroke::new(1.0, stroke.color.gamma_multiply(0.6)));
        }
        for w in pts.windows(2) { painter.line_segment([w[0], w[1]], stroke); }
        for (i, p) in pts.iter().enumerate() { painter.circle_filled(*p, if i == 0 { 5.0 } else { 3.5 }, stroke.color); }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if app.slab_points.is_empty() { app.choose_tool(crate::Tool::Select); } else { app.slab_points.clear(); }
        } else if ui.input(|i| i.key_pressed(egui::Key::Backspace)) { app.slab_points.pop();
        } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) && app.slab_points.len() >= 3 { app.finish_slab(level_id);
        } else if pick.is_some() {
            if let Some(p) = pick {
                let closing = pts.first().is_some_and(|f| f.distance(p) < 10.0);
                app.slab_click(level_id, to_model(p), closing);
            }
        }
    } else if app.tool == crate::Tool::Column && !three_d {
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [(v.x as f64 / 0.05).round() * 0.05, (-v.y as f64 / 0.05).round() * 0.05] };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); }
        else if pick.is_some() { if let Some(p) = pick { app.column_click(level_id, to_model(p)); } }
    } else if app.tool == crate::Tool::Truss && !three_d {
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let snap = |v: f32| (v as f64 / 0.05).round() * 0.05;
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [snap(v.x), snap(-v.y)] };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); }
        else if pick.is_some() { if let Some(p) = pick { app.truss_click(level_id, to_model(p)); } }
    } else if matches!(app.tool, crate::Tool::Duct | crate::Tool::Pipe) && !three_d {
        // the points of a duct or pipe run, snapped to 50 mm; Enter or Esc (once it has a length) places it
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let snap = |v: f32| (v as f64 / 0.05).round() * 0.05;
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [snap(v.x), snap(-v.y)] };
        let pts: Vec<Pos2> = app.run_points.iter().map(|p| screen([p[0], p[1], level_z])).collect();
        let colour = Color32::from_rgba_unmultiplied(90, 150, 200, 160);
        for (i, w) in pts.windows(2).enumerate() {
            let width = match app.run_sizes.get(i) { Some(openbim_place::penetrations::RunSize::Round { diameter }) => *diameter, Some(openbim_place::penetrations::RunSize::Rect { width, .. }) => *width, None => 0.1 } as f32;
            painter.line_segment([w[0], w[1]], Stroke::new((width * scale).max(2.0), colour));
        }
        if let (Some(last), Some(h)) = (pts.last(), response.hover_pos()) {
            let m = to_model(h);
            painter.line_segment([*last, screen([m[0], m[1], level_z])], Stroke::new(1.0, colour));
        }
        for p in &pts { painter.circle_filled(*p, 3.5, colour); }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if app.run_points.len() >= 2 { app.finish_run(level_id); } else { app.run_points.clear(); app.run_sizes.clear(); app.choose_tool(crate::Tool::Select); }
        } else if ui.input(|i| i.key_pressed(egui::Key::Backspace)) { app.run_back();
        } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) && app.run_points.len() >= 2 { app.finish_run(level_id);
        } else if pick.is_some() {
            if let Some(p) = pick { app.run_click(to_model(p)); }
        }
    } else if app.tool == crate::Tool::Beam && !three_d {
        // two clicks place a beam, snapped to 50 mm; Esc ends the tool
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let snap = |v: f32| (v as f64 / 0.05).round() * 0.05;
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [snap(v.x), snap(-v.y)] };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); } else {
            if let (Some(a), Some(h)) = (app.beam_start, response.hover_pos()) {
                let (a, b) = (screen([a[0], a[1], level_z]), screen([to_model(h)[0], to_model(h)[1], level_z]));
                painter.line_segment([a, b], Stroke::new((app.width_mm as f32 / 1000.0 * scale).max(2.0), Color32::from_rgba_unmultiplied(176, 140, 104, 140)));
            }
            if pick.is_some() { if let Some(p) = pick { app.beam_click(level_id, to_model(p)); } }
        }
    } else if response.clicked() {
        if let Some(p) = response.interact_pointer_pos() {
            let hit = faces.iter().rev().find(|(_, _, polygon, _)| contains(polygon, p)).map(|(_, id, _, _)| *id)
                .or_else(|| datum_hits.iter().rev().find(|(_, a, b)| segment_distance(p, *a, *b) < 7.0
                    || p.distance(*a) < 12.0 || p.distance(*b) < 12.0).map(|(id, _, _)| *id));
            app.select_element(hit);
        }
    }
    if app.tool == crate::Tool::Select && !three_d {
        // As in Revit: a selected wall shows a grip at each end, and dragging a grip moves that end (the walls joined
        // there follow); pressing on an element and dragging moves it with what stands on it. A ghost shows the result.
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let raw = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [v.x as f64, -v.y as f64] };
        let corners: Vec<[f64; 2]> = app.document.elements().flat_map(|el| match el.geometry {
            Some(Geometry::Linear { start, end, level }) if el.category == Category::Wall && level == level_id => { let mut v = vec![start, end]; if let Some(r) = app.document.wall_plan(el.id) { v.extend(r); } v }
            Some(Geometry::Grid { start, end }) => vec![start, end],
            _ => vec![],
        }).collect();
        let ortho = ui.input(|i| i.modifiers.shift);
        let snap_to = |p: Pos2, from: Option<[f64; 2]>| -> [f64; 2] {
            if let Some(c) = corners.iter().filter(|c| screen([c[0], c[1], level_z]).distance(p) < 12.0)
                .min_by(|a, b| screen([a[0], a[1], level_z]).distance(p).total_cmp(&screen([b[0], b[1], level_z]).distance(p))) { return *c; }
            let r = raw(p);
            let mut m = [(r[0] / 0.05).round() * 0.05, (r[1] / 0.05).round() * 0.05];
            if let (true, Some(s)) = (ortho, from) { if (m[0] - s[0]).abs() >= (m[1] - s[1]).abs() { m[1] = s[1]; } else { m[0] = s[0]; } }
            m
        };
        let wall_ends = selected.and_then(|id| match app.document.get(id) {
            Some(e) if e.category == Category::Wall && app.document.level_of(id) == Some(level_id) => match e.geometry { Some(Geometry::Linear { start, end, .. }) => Some((id, start, end)), _ => None },
            _ => None,
        });
        let grip_at = |m: [f64; 2]| screen([m[0], m[1], level_z]);
        if let (Some((_, a, b)), None) = (wall_ends, app.drag) {
            for g in [a, b] { let c = grip_at(g); painter.rect_filled(Rect::from_center_size(c, Vec2::splat(9.0)), 1.0, ui.visuals().panel_fill); painter.rect_stroke(Rect::from_center_size(c, Vec2::splat(9.0)), 1.0, Stroke::new(1.5, blue), egui::StrokeKind::Middle); }
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                if let Some((id, a, b)) = wall_ends.filter(|(_, a, b)| grip_at(*a).distance(p) < 10.0 || grip_at(*b).distance(p) < 10.0) {
                    app.drag = Some(crate::Drag::Grip { id, at_start: grip_at(a).distance(p) <= grip_at(b).distance(p) });
                } else if let Some(hit) = faces.iter().rev().find(|(_, _, polygon, _)| contains(polygon, p)).map(|(_, id, _, _)| *id) {
                    app.select_element(Some(hit));
                    app.drag = Some(crate::Drag::Move { id: hit, from: snap_to(p, None) });
                }
            }
        }
        if let Some(d) = app.drag {
            let here = response.interact_pointer_pos().or(response.hover_pos());
            let ghost = Stroke::new(1.5, Color32::from_rgb(220, 140, 0));
            let fill = Color32::from_rgba_unmultiplied(220, 140, 0, 40);
            let mut done: Option<Result<Vec<openbim_document::Command>, String>> = None;
            if let Some(h) = here {
                match d {
                    crate::Drag::Move { id, from } => {
                        let to = snap_to(h, Some(from));
                        let xf = openbim_document::Xf::Move([to[0] - from[0], to[1] - from[1]]);
                        for (_, _, points, _) in faces.iter().filter(|f| f.1 == id) {
                            let moved: Vec<Pos2> = points.iter().map(|q| { let m = xf.point(raw(*q)); screen([m[0], m[1], level_z]) }).collect();
                            painter.add(egui::Shape::convex_polygon(moved, fill, ghost));
                        }
                        painter.text(h + Vec2::new(12.0, -12.0), egui::Align2::LEFT_BOTTOM, format!("{:.0} mm", (to[0] - from[0]).hypot(to[1] - from[1]) * 1000.0), egui::FontId::proportional(13.0), blue);
                        if (to[0] - from[0]).hypot(to[1] - from[1]) > 1e-6 { done = Some(app.document.transform_commands(&[id], xf)); }
                    }
                    crate::Drag::Grip { id, at_start } => if let Some((_, a, b)) = wall_ends {
                        let (fixed, moving) = if at_start { (b, a) } else { (a, b) };
                        let to = snap_to(h, Some(fixed));
                        let (dx, dy) = (to[0] - fixed[0], to[1] - fixed[1]);
                        let l = dx.hypot(dy);
                        if l > 1e-6 {
                            let w = app.document.get(id).map_or(0.2, |e| e.width_m) / 2.0;
                            let (nx, ny) = (-dy / l * w, dx / l * w);
                            let ring = [[fixed[0] - nx, fixed[1] - ny], [to[0] - nx, to[1] - ny], [to[0] + nx, to[1] + ny], [fixed[0] + nx, fixed[1] + ny]].map(|p| screen([p[0], p[1], level_z]));
                            painter.add(egui::Shape::convex_polygon(ring.to_vec(), fill, ghost));
                            painter.text(grip_at(to) + Vec2::new(10.0, -10.0), egui::Align2::LEFT_BOTTOM, format!("{:.0} mm", l * 1000.0), egui::FontId::proportional(13.0), blue);
                        }
                        if (to[0] - moving[0]).hypot(to[1] - moving[1]) > 1e-6 { done = Some(app.document.wall_end_commands(id, at_start, to)); }
                    }
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.drag = None; app.message = "Drag cancelled".into(); }
            else if response.drag_stopped_by(egui::PointerButton::Primary) {
                app.drag = None;
                match done {
                    Some(Ok(ops)) => match app.document.transact(app.document.revision(), &ops) {
                        Ok(_) => app.message = "Moved; undo takes it back".into(),
                        Err(e) => app.message = format!("Cannot do that: {e:?}"),
                    },
                    Some(Err(e)) => app.message = format!("Cannot do that: {e}"),
                    None => {}
                }
            }
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

    #[test]
    fn the_beam_tool_draws_a_rubber_band_and_a_click_in_the_plan_places_the_beam() {
        let ctx = egui::Context::default();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.choose_tool(crate::Tool::Beam);
        app.beam_start = Some([1.0, 1.0]);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
        let frame = |app: &mut Desktop, events: Vec<egui::Event>| {
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() },
                |ui| { egui::CentralPanel::default().show(ui, |ui| show(app, ui)); });
            out.textures_delta.clear();
        };
        let at = Pos2::new(500.0, 350.0);
        let btn = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        frame(&mut app, vec![egui::Event::PointerMoved(at)]);
        frame(&mut app, vec![btn(true)]);
        let n = app.document.elements().filter(|e| e.category == Category::Beam).count();
        frame(&mut app, vec![btn(false)]);
        assert_eq!(app.document.elements().filter(|e| e.category == Category::Beam).count(), n + 1, "{}", app.message);
        assert!(app.tool == crate::Tool::Beam, "the tool stays on");
        frame(&mut app, vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
        assert!(app.tool == crate::Tool::Select);
    }

    fn frames() -> (egui::Context, Rect) { (egui::Context::default(), Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))) }
    fn frame(ctx: &egui::Context, screen: Rect, app: &mut Desktop, events: Vec<egui::Event>) {
        let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() },
            |ui| { egui::CentralPanel::default().show(ui, |ui| show(app, ui)); });
        out.textures_delta.clear();
    }
    fn button(at: Pos2, pressed: bool) -> egui::Event { egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() } }
    fn walls(app: &Desktop) -> usize { app.document.elements().filter(|e| e.category == Category::Wall).count() }

    #[test]
    fn a_wall_is_drawn_by_two_clicks_as_a_chain_and_by_press_drag_release() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.run_command("wall", "Wall");
        assert!(app.tool == crate::Tool::Wall && app.width_mm == 200.0 && app.height_mm == 3000.0);
        let (a, b, c) = (Pos2::new(300.0, 300.0), Pos2::new(500.0, 300.0), Pos2::new(500.0, 450.0));
        let n = walls(&app);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(a)]);
        for at in [a, b] {
            frame(&ctx, screen, &mut app, vec![button(at, true)]);
            frame(&ctx, screen, &mut app, vec![button(at, false)]);
        }
        assert_eq!(walls(&app), n + 1, "{}", app.message);
        // the chain goes on from the end of the last wall
        for at in [c] { frame(&ctx, screen, &mut app, vec![button(at, true)]); frame(&ctx, screen, &mut app, vec![button(at, false)]); }
        assert_eq!(walls(&app), n + 2, "{}", app.message);
        // Esc ends the chain, and then a press-drag-release draws one wall
        frame(&ctx, screen, &mut app, vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
        assert!(app.tool == crate::Tool::Wall && app.wall_pts.is_empty());
        let (d, e) = (Pos2::new(200.0, 500.0), Pos2::new(400.0, 520.0));
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(d)]);
        frame(&ctx, screen, &mut app, vec![button(d, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(Pos2::new(300.0, 510.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(e)]);
        frame(&ctx, screen, &mut app, vec![button(e, false)]);
        assert_eq!(walls(&app), n + 3, "{}", app.message);
        // the walls are real: undo takes them away one by one
        app.document.undo().unwrap();
        assert_eq!(walls(&app), n + 2);
    }

    #[test]
    fn a_beam_and_a_duct_are_drawn_by_dragging_too() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.choose_tool(crate::Tool::Beam);
        let beams = |a: &Desktop| a.document.elements().filter(|e| e.category == Category::Beam).count();
        let n = beams(&app);
        let (d, e) = (Pos2::new(200.0, 500.0), Pos2::new(420.0, 500.0));
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(d)]);
        frame(&ctx, screen, &mut app, vec![button(d, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(Pos2::new(300.0, 500.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(e)]);
        frame(&ctx, screen, &mut app, vec![button(e, false)]);
        assert_eq!(beams(&app), n + 1, "{}", app.message);
        // a duct run: press and release give its two points
        app.run_command("duct", "Duct");
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(d)]);
        frame(&ctx, screen, &mut app, vec![button(d, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(e)]);
        frame(&ctx, screen, &mut app, vec![button(e, false)]);
        assert_eq!(app.run_points.len(), 2, "{}", app.message);
    }

    #[test]
    fn walls_follow_the_location_line_chain_at_corners_and_close_the_loop() {
        let mut app = Desktop::sample();
        app.run_command("wall", "Wall");
        let level = ElementId(10);
        let line = |app: &Desktop, id: ElementId| match app.document.get(id).unwrap().geometry { Some(Geometry::Linear { start, end, .. }) => (start, end), _ => panic!() };
        let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9;
        // drawn clockwise along the exterior face of 200 mm walls, the axes lie 100 mm inside
        app.wall_loc = 2;
        let n = walls(&app);
        for p in [[20.0, 20.0], [26.0, 20.0], [26.0, 14.0], [20.0, 14.0], [20.0, 20.0]] { app.wall_click(level, p); }
        assert_eq!(walls(&app), n + 4, "{} / {:?}", app.message, app.wall_pts);
        assert!(app.wall_pts.is_empty(), "the loop is closed and the chain over");
        let ids: Vec<_> = app.document.elements().filter(|e| e.category == Category::Wall).map(|e| e.id).skip(n).collect();
        assert_eq!(ids.len(), 4);
        // axes: top (19.9) from 20.1 to 25.9 ... each wall's axis runs between the corners of the offset lines
        let top = line(&app, ids[0]);
        assert!(near(top.0, [20.1, 19.9]) && near(top.1, [25.9, 19.9]), "{top:?}");
        let right = line(&app, ids[1]);
        assert!(near(right.0, [25.9, 19.9]) && near(right.1, [25.9, 14.1]), "{right:?}");
        let left = line(&app, ids[3]);
        assert!(near(left.1, [20.1, 19.9]), "{left:?}");
        // the outer face of the top wall is on the drawn line: its plan has corners at y = 20
        let ring = app.document.wall_plan(ids[0]).unwrap();
        assert!(ring.iter().any(|p| (p[1] - 20.0).abs() < 1e-9 && (p[0] - 20.0).abs() < 1e-9), "{ring:?}");
        // interior face: the axis lies 100 mm to the left
        app.wall_loc = 3;
        app.wall_click(level, [40.0, 20.0]);
        app.wall_click(level, [46.0, 20.0]);
        let last = *app.document.elements().filter(|e| e.category == Category::Wall).map(|e| e.id).collect::<Vec<_>>().last().unwrap();
        assert!(near(line(&app, last).0, [40.0, 20.1]), "{:?}", line(&app, last));
        // without Chain each pick pair is a wall of its own
        app.wall_pts.clear();
        app.wall_ids.clear();
        app.wall_chain_on = false;
        app.wall_click(level, [50.0, 20.0]);
        app.wall_click(level, [52.0, 20.0]);
        assert!(app.wall_pts.is_empty());
        // a typed length is exact
        app.wall_chain_on = true;
        app.wall_loc = 0;
        app.wall_pts = vec![[60.0, 20.0]];
        app.wall_click(level, [63.5, 20.0]);
        let last = *app.document.elements().filter(|e| e.category == Category::Wall).map(|e| e.id).collect::<Vec<_>>().last().unwrap();
        assert!(near(line(&app, last).1, [63.5, 20.0]));
    }

    #[test]
    fn grids_are_drawn_named_in_sequence_and_levels_get_their_own_plan() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let grids = |a: &Desktop| a.document.elements().filter(|e| e.category == Category::Grid).count();
        let n = grids(&app);
        app.run_command("grid", "Grid");
        assert!(app.tool == crate::Tool::Grid);
        let (a, b, c, d) = (Pos2::new(300.0, 200.0), Pos2::new(300.0, 500.0), Pos2::new(420.0, 200.0), Pos2::new(420.0, 500.0));
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(a)]);
        // a click on each end, then a press-drag-release
        for at in [a, b] { frame(&ctx, screen, &mut app, vec![button(at, true)]); frame(&ctx, screen, &mut app, vec![button(at, false)]); }
        assert_eq!(grids(&app), n + 1, "{}", app.message);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(c)]);
        frame(&ctx, screen, &mut app, vec![button(c, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(Pos2::new(420.0, 300.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(d)]);
        frame(&ctx, screen, &mut app, vec![button(d, false)]);
        assert_eq!(grids(&app), n + 2, "{}", app.message);
        let names: Vec<String> = app.document.elements().filter(|e| e.category == Category::Grid).map(|e| e.name.clone()).collect();
        assert_eq!(&names[n..], ["4", "5"], "the sample ends at 3: {names:?}");
        assert_eq!(openbim_document::Document::spatial_sample().next_grid_name(), "4");
        // levels: the name counts on, a level at an elevation in use is refused, and the new one has a plan to draw in
        app.run_command("level", "Level");
        assert_eq!(app.level_elev_mm, 6000.0);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(a)]);
        frame(&ctx, screen, &mut app, vec![button(a, true)]);
        frame(&ctx, screen, &mut app, vec![button(a, false)]);
        let level = app.document.elements().find(|e| e.name == "Level 3").expect("a third level").id;
        assert!(matches!(app.view, View::Level(id) if id == level.0) && app.plan_level() == level, "{}", app.message);
        app.level_elev_mm = 6000.0;
        app.level_click();
        assert!(app.message.contains("rejected"), "{}", app.message);
        app.document.undo().unwrap();
        assert!(app.document.get(level).is_none());
        // a wall drawn in the plan of the new level stands on it
        app.level_elev_mm = 7000.0;
        app.level_click();
        let level = app.document.elements().find(|e| e.name == "Level 3").unwrap().id;
        app.run_command("wall", "Wall");
        let w = [Pos2::new(300.0, 300.0), Pos2::new(500.0, 300.0)];
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(w[0])]);
        for at in w { frame(&ctx, screen, &mut app, vec![button(at, true)]); frame(&ctx, screen, &mut app, vec![button(at, false)]); }
        let wall = app.document.elements().filter(|e| e.category == Category::Wall).last().unwrap();
        assert!(matches!(wall.geometry, Some(Geometry::Linear { level: l, .. }) if l == level), "{:?}", wall.geometry);
    }

    #[test]
    fn a_tag_is_placed_on_the_clicked_element_drawn_in_its_plan_and_undone() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.run_command("tag", "Tag");
        assert!(app.tool == crate::Tool::Tag);
        let tags = |a: &Desktop| a.document.elements().filter(|e| e.category == Category::Tag).count();
        app.tag_click(ElementId(20), [2.0, 0.0]);
        assert_eq!(tags(&app), 1, "{}", app.message);
        frame(&ctx, screen, &mut app, vec![]);
        app.view = View::UpperPlan;
        frame(&ctx, screen, &mut app, vec![]);
        app.tag_leader = false;
        app.tag_click(ElementId(1), [3.0, 2.0]);
        let last = app.document.elements().filter(|e| e.category == Category::Tag).last().unwrap();
        assert!(matches!(last.geometry, Some(Geometry::Tag { at, anchor, .. }) if at == anchor));
        // levels cannot be tagged
        app.tag_click(ElementId(10), [0.0, 0.0]);
        assert_eq!(tags(&app), 2);
        app.document.undo().unwrap();
        assert_eq!(tags(&app), 1);
    }

    fn key(k: egui::Key) -> egui::Event { egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() } }
    fn wall_line(app: &Desktop, id: ElementId) -> ([f64; 2], [f64; 2]) { match app.document.get(id).and_then(|e| e.geometry) { Some(Geometry::Linear { start, end, .. }) => (start, end), g => panic!("{g:?}") } }

    #[test]
    fn delete_takes_the_element_with_its_door_and_tag_and_undo_brings_them_back() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let spec = openbim_document::FamilySpec { family: "Door, single flush".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 };
        app.document.place_family_in_wall(ElementId(20), 3.0, &spec).unwrap();
        app.document.place_tag(ElementId(20), [2.0, 1.0], [2.0, 0.0]).unwrap();
        let n = app.document.elements().count();
        app.select_element(Some(ElementId(20)));
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Delete)]);
        assert!(app.document.get(ElementId(20)).is_none() && app.document.elements().count() == n - 3, "{}", app.message);
        assert!(app.selected_id().is_none());
        app.document.undo().unwrap();
        assert_eq!(app.document.elements().count(), n);
        // a level is not deleted
        app.select_element(Some(ElementId(10)));
        app.delete_selected();
        assert!(app.document.get(ElementId(10)).is_some(), "{}", app.message);
    }

    #[test]
    fn move_copy_rotate_and_mirror_work_on_the_selected_element_by_picked_points() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.select_element(Some(ElementId(20)));
        let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6;
        // Move: base point, then where to; the tool ends
        app.run_command("move", "Move");
        assert!(app.tool == crate::Tool::Move);
        app.edit_click([0.0, 0.0]);
        app.edit_click([0.0, 3.0]);
        assert!(near(wall_line(&app, ElementId(20)).0, [0.0, 3.0]), "{}", app.message);
        assert!(app.tool == crate::Tool::Select);
        app.document.undo().unwrap();
        // Move by a typed distance towards the pointer
        app.run_command("move", "Move");
        app.edit_click([0.0, 0.0]);
        app.edit_num = "2500".into();
        app.edit_enter([0.0, 5.0]);
        assert!(near(wall_line(&app, ElementId(20)).0, [0.0, 2.5]), "{}", app.message);
        app.document.undo().unwrap();
        // Copy goes on until Esc, and selects the newest copy
        let n = walls(&app);
        app.run_command("copy", "Copy");
        app.edit_click([0.0, 0.0]);
        app.edit_click([0.0, 4.0]);
        app.edit_click([0.0, 8.0]);
        assert_eq!(walls(&app), n + 2, "{}", app.message);
        assert!(app.tool == crate::Tool::Copy && near(wall_line(&app, app.selected_id().unwrap()).0, [0.0, 8.0]));
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select);
        // Rotate: centre, start ray, end ray (a quarter turn), and by a typed angle
        app.select_element(Some(ElementId(20)));
        app.run_command("rotate", "Rotate");
        app.edit_click([0.0, 0.0]);
        app.edit_click([1.0, 0.0]);
        app.edit_click([0.0, 1.0]);
        assert!(near(wall_line(&app, ElementId(20)).1, [0.0, 8.0]), "{:?} {}", wall_line(&app, ElementId(20)), app.message);
        app.document.undo().unwrap();
        app.run_command("rotate", "Rotate");
        app.edit_click([0.0, 0.0]);
        app.edit_num = "180".into();
        app.edit_enter([0.0, 0.0]);
        assert!(near(wall_line(&app, ElementId(20)).1, [-8.0, 0.0]), "{:?} {}", wall_line(&app, ElementId(20)), app.message);
        app.document.undo().unwrap();
        // Mirror with Copy (the default) leaves the original; without it the element itself turns over
        let n = walls(&app);
        app.run_command("mirror", "Mirror");
        app.edit_click([0.0, 1.0]);
        app.edit_click([1.0, 1.0]);
        assert_eq!(walls(&app), n + 1, "{}", app.message);
        assert!(near(wall_line(&app, ElementId(20)).0, [0.0, 0.0]));
        assert!(app.selected_id() != Some(ElementId(20)));
        app.select_element(Some(ElementId(20)));
        app.run_command("mirror", "Mirror");
        app.mirror_copy = false;
        app.edit_click([0.0, 1.0]);
        app.edit_click([1.0, 1.0]);
        assert_eq!(walls(&app), n + 1);
        assert!(near(wall_line(&app, ElementId(20)).1, [0.0, 2.0]), "{:?}", wall_line(&app, ElementId(20)));
    }

    #[test]
    fn move_by_press_drag_release_and_a_click_picks_the_element_when_none_is_selected() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.run_command("move", "Move");
        app.select_element(None);
        let before = wall_line(&app, ElementId(20));
        // nothing is selected: a click on the wall selects it, and does not move anything
        let mut hit = None;
        'find: for y in (100..600).step_by(10) { for x in (100..900).step_by(10) {
            let mut probe = Desktop::sample();
            probe.view = View::Plan;
            probe.run_command("move", "Move");
            probe.select_element(None);
            let at = Pos2::new(x as f32, y as f32);
            frame(&ctx, screen, &mut probe, vec![egui::Event::PointerMoved(at)]);
            frame(&ctx, screen, &mut probe, vec![button(at, true)]);
            frame(&ctx, screen, &mut probe, vec![button(at, false)]);
            if probe.selected_id() == Some(ElementId(20)) { hit = Some(at); break 'find; }
        } }
        let at = hit.expect("the wall is somewhere in the view");
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, screen, &mut app, vec![button(at, true)]);
        frame(&ctx, screen, &mut app, vec![button(at, false)]);
        assert_eq!(app.selected_id(), Some(ElementId(20)), "{}", app.message);
        assert_eq!(wall_line(&app, ElementId(20)), before);
        // press, drag and release moves it by the drag
        let to = at + Vec2::new(0.0, 80.0);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, screen, &mut app, vec![button(at, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at + Vec2::new(0.0, 40.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(to)]);
        frame(&ctx, screen, &mut app, vec![button(to, false)]);
        let after = wall_line(&app, ElementId(20));
        assert!((after.0[0] - before.0[0]).abs() < 0.2 && after.0[1] < before.0[1] - 0.1, "{before:?} {after:?} {}", app.message);
    }

    /// The screen row and the left and right ends of where a click selects wall 20 in the plan.
    fn wall_span(ctx: &egui::Context, screen: Rect, app: &Desktop) -> (f32, f32, f32) {
        let mut found = Vec::new();
        for y in (100..600).step_by(4) { for x in (60..940).step_by(2) {
            let mut probe = Desktop::sample();
            probe.view = app.view;
            probe.document = app.document.clone();
            probe.select_element(None);
            let at = Pos2::new(x as f32, y as f32);
            frame(ctx, screen, &mut probe, vec![egui::Event::PointerMoved(at)]);
            frame(ctx, screen, &mut probe, vec![button(at, true)]);
            frame(ctx, screen, &mut probe, vec![button(at, false)]);
            if probe.selected_id() == Some(ElementId(20)) { found.push((x as f32, y as f32)); }
        } if !found.is_empty() { break; } }
        let y = found[0].1;
        (y, found.iter().map(|f| f.0).fold(f32::INFINITY, f32::min), found.iter().map(|f| f.0).fold(f32::NEG_INFINITY, f32::max))
    }

    #[test]
    fn dragging_a_wall_end_grip_moves_that_end_and_the_wall_joined_to_it() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let level = match app.document.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let b = app.document.place_wall(level, [8.0, 0.0], [8.0, 6.0], 0.2, 3.0, 0.0).unwrap();
        let (y, left, right) = wall_span(&ctx, screen, &app);
        app.select_element(Some(ElementId(20)));
        frame(&ctx, screen, &mut app, vec![]);
        let (press, to) = (Pos2::new(right + 9.0, y), Pos2::new(right + 70.0, y));
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press)]);
        frame(&ctx, screen, &mut app, vec![button(press, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press + Vec2::new(20.0, 0.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(to)]);
        assert!(matches!(app.drag, Some(crate::Drag::Grip { at_start: false, .. })), "{:?}", app.drag);
        frame(&ctx, screen, &mut app, vec![button(to, false)]);
        let (a1, a2) = wall_line(&app, ElementId(20));
        let (b1, b2) = wall_line(&app, b);
        assert!(a2[0] > 8.2 && a1 == [0.0, 0.0] && a2[1] == 0.0, "{a1:?} {a2:?} {}", app.message);
        assert!(b1 == a2 && b2 == [8.0, 6.0], "the joined wall follows: {b1:?} {a2:?}");
        assert!(app.drag.is_none());
        app.document.undo().unwrap();
        assert_eq!(wall_line(&app, b).0, [8.0, 0.0]);
        // the start grip: only that end moves
        let (press, to) = (Pos2::new(left + 1.0, y), Pos2::new(left + 1.0, y + 50.0));
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press)]);
        frame(&ctx, screen, &mut app, vec![button(press, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press + Vec2::new(0.0, 20.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(to)]);
        frame(&ctx, screen, &mut app, vec![button(to, false)]);
        let (a1, a2) = wall_line(&app, ElementId(20));
        assert!(a1[1] < -0.1 && a2 == [8.0, 0.0], "{a1:?} {a2:?} {}", app.message);
        // Esc cancels a drag in progress
        let mid = Pos2::new((left + right) / 2.0, y);
        let _ = mid;
    }

    #[test]
    fn dragging_an_element_in_select_mode_moves_it_and_esc_cancels() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let (y, left, right) = wall_span(&ctx, screen, &app);
        let before = wall_line(&app, ElementId(20));
        let mid = Pos2::new((left + right) / 2.0, y);
        // Esc in the middle of a drag leaves the wall where it was
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(mid)]);
        frame(&ctx, screen, &mut app, vec![button(mid, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(mid + Vec2::new(0.0, 30.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(mid + Vec2::new(0.0, 60.0))]);
        assert!(matches!(app.drag, Some(crate::Drag::Move { .. })), "{:?}", app.drag);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        frame(&ctx, screen, &mut app, vec![button(mid + Vec2::new(0.0, 60.0), false)]);
        assert!(app.drag.is_none());
        assert_eq!(wall_line(&app, ElementId(20)), before);
        // a full drag moves it, with its door and tag
        let spec = openbim_document::FamilySpec { family: "Door, single flush".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 };
        app.document.place_family_in_wall(ElementId(20), 3.0, &spec).unwrap();
        app.document.place_tag(ElementId(20), [2.0, 1.0], [2.0, 0.0]).unwrap();
        let to = mid + Vec2::new(0.0, 70.0);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(mid)]);
        frame(&ctx, screen, &mut app, vec![button(mid, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(mid + Vec2::new(0.0, 30.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(to)]);
        frame(&ctx, screen, &mut app, vec![button(to, false)]);
        let after = wall_line(&app, ElementId(20));
        assert!(after.0[1] < -0.1 && after.1[1] == after.0[1] && after.0[0].abs() < 0.2, "{after:?} {}", app.message);
        let tag = app.document.elements().find(|e| e.category == Category::Tag).unwrap();
        assert!(matches!(tag.geometry, Some(Geometry::Tag { anchor, .. }) if anchor[1] == after.0[1]));
        app.document.undo().unwrap();
        assert_eq!(wall_line(&app, ElementId(20)), before);
    }

    #[test]
    fn offset_align_and_trim_work_by_clicking_the_elements() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let level = match app.document.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6;
        // Offset with Copy: a new wall 1.5 m to the side clicked; the original stays
        app.run_command("offset", "Offset");
        assert!(app.tool == crate::Tool::Offset);
        app.offset_mm = 1500.0;
        app.offset_copy = true;
        let n = walls(&app);
        app.modify_click(Some(ElementId(20)), [2.0, 0.5]);
        assert_eq!(walls(&app), n + 1, "{}", app.message);
        assert!(near(wall_line(&app, app.selected_id().unwrap()).0, [0.0, 1.5]) && near(wall_line(&app, ElementId(20)).0, [0.0, 0.0]));
        // without Copy it moves the wall itself, to the other side
        app.offset_copy = false;
        app.modify_click(Some(ElementId(20)), [2.0, -0.5]);
        assert!(near(wall_line(&app, ElementId(20)).0, [0.0, -1.5]), "{}", app.message);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select);
        // Align: the second wall goes onto the line of the first, which stays the reference until Esc
        let a = app.document.place_wall(level, [0.0, 6.0], [4.0, 6.0], 0.2, 3.0, 0.0).unwrap();
        let b = app.document.place_wall(level, [0.0, 9.0], [4.0, 9.0], 0.2, 3.0, 0.0).unwrap();
        app.run_command("align", "Align");
        app.modify_click(Some(ElementId(20)), [1.0, 0.0]);
        app.modify_click(Some(a), [1.0, 6.0]);
        assert!(near(wall_line(&app, a).0, [0.0, -1.5]), "{}", app.message);
        app.modify_click(Some(b), [1.0, 9.0]);
        assert!(near(wall_line(&app, b).1, [4.0, -1.5]), "{}", app.message);
        app.modify_click(None, [0.0, 0.0]);
        assert!(app.message.contains("Click a wall"));
        // Trim/Extend to Corner: two walls that stop short of each other meet at the corner
        app.run_command("trim", "Trim/Extend");
        let c = app.document.place_wall(level, [20.0, 0.0], [26.0, 0.0], 0.2, 3.0, 0.0).unwrap();
        let d = app.document.place_wall(level, [30.0, 4.0], [30.0, 10.0], 0.2, 3.0, 0.0).unwrap();
        app.modify_click(Some(c), [21.0, 0.0]);
        app.modify_click(Some(d), [30.0, 8.0]);
        assert!(near(wall_line(&app, c).1, [30.0, 0.0]) && near(wall_line(&app, d).0, [30.0, 0.0]), "{}", app.message);
        app.document.undo().unwrap();
        assert!(near(wall_line(&app, c).1, [26.0, 0.0]) && near(wall_line(&app, d).0, [30.0, 4.0]));
        // a click on a wall in the plan reaches the tool
        frame(&ctx, screen, &mut app, vec![]);
    }

    #[test]
    fn walls_have_types_a_rectangle_pick_lines_and_the_spacebar_flip() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.run_command("wall", "Wall");
        let level = ElementId(10);
        let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9;
        // a type is added from the thickness and height of the options bar and used by the walls drawn after it
        app.width_mm = 300.0;
        app.height_mm = 3500.0;
        let t = app.document.add_wall_type("Exterior 300", 0.3, 3.5).unwrap();
        app.wall_type = Some(t);
        app.wall_click(level, [100.0, 0.0]);
        app.wall_click(level, [104.0, 0.0]);
        let w = app.document.elements().filter(|e| e.category == Category::Wall).last().unwrap();
        assert_eq!((w.type_id, w.width_m, w.height_m), (Some(t), 0.3, 3.5), "{}", app.message);
        app.wall_pts.clear();
        app.wall_type = None;
        // Rectangle: four joined walls from two corners, the exterior face on the picked rectangle, one undo
        app.wall_mode = 1;
        app.width_mm = 200.0;
        app.wall_loc = 2;
        let n = walls(&app);
        app.wall_click(level, [20.0, 14.0]);
        assert_eq!(walls(&app), n);
        app.wall_click(level, [26.0, 20.0]);
        assert_eq!(walls(&app), n + 4, "{}", app.message);
        let ids: Vec<_> = app.document.elements().filter(|e| e.category == Category::Wall).map(|e| e.id).skip(n).collect();
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for id in &ids { for p in app.document.wall_plan(*id).unwrap() { for k in 0..2 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } } }
        assert!(near(lo, [20.0, 14.0]) && near(hi, [26.0, 20.0]), "{lo:?} {hi:?}");
        app.document.undo().unwrap();
        assert_eq!(walls(&app), n);
        // Spacebar while drawing flips the side of the Location Line
        app.wall_mode = 0;
        let before = app.wall_axis([0.0, 0.0], [4.0, 0.0]);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Space)]);
        assert!(app.wall_flip);
        let after = app.wall_axis([0.0, 0.0], [4.0, 0.0]);
        assert!(near(before.0, [0.0, -0.1]) && near(after.0, [0.0, 0.1]), "{before:?} {after:?}");
        app.wall_flip = false;
        // Pick Lines: a wall along a grid, facing the side that was clicked
        let g = app.document.place_grid([0.0, -20.0], [10.0, -20.0], "").unwrap();
        app.wall_mode = 2;
        app.wall_loc = 0;
        let n = walls(&app);
        app.wall_pick_line(level, g, [3.0, -19.0]);
        assert_eq!(walls(&app), n + 1, "{}", app.message);
        let last = app.document.elements().filter(|e| e.category == Category::Wall).last().unwrap();
        assert!(matches!(last.geometry, Some(Geometry::Linear { start, end, .. }) if near(start, [0.0, -20.0]) && near(end, [10.0, -20.0])));
        app.wall_loc = 2;
        app.wall_pick_line(level, g, [3.0, -21.0]);
        let last = app.document.elements().filter(|e| e.category == Category::Wall).last().unwrap();
        assert!(matches!(last.geometry, Some(Geometry::Linear { start, end, .. }) if near(start, [10.0, -19.9]) && near(end, [0.0, -19.9])), "{:?}", last.geometry);
        // Spacebar on a selected wall turns it over
        app.choose_tool(crate::Tool::Select);
        app.select_element(Some(ElementId(20)));
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Space)]);
        assert_eq!(wall_line(&app, ElementId(20)), ([8.0, 0.0], [0.0, 0.0]), "{}", app.message);
        app.document.undo().unwrap();
        assert_eq!(wall_line(&app, ElementId(20)), ([0.0, 0.0], [8.0, 0.0]));
    }
}
