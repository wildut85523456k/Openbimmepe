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
    let proj = |p| project(p, three_d, app.camera_yaw, app.camera_pitch);
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
    // Revit's way of picking a point: a click, or press at the start, drag, release at the end. A drag
    // starting a tool with nothing pending picks its start point at the press and its next point at the release.
    let drawing = !three_d && !matches!(app.tool, crate::Tool::Select | crate::Tool::Rebar | crate::Tool::WallFooting);
    let pick: Option<Pos2> = if !drawing { None } else if response.clicked() { response.interact_pointer_pos() }
        else if response.drag_started_by(egui::PointerButton::Primary) {
            let starts = matches!(app.tool, crate::Tool::Wall | crate::Tool::Beam | crate::Tool::Truss | crate::Tool::Duct | crate::Tool::Pipe | crate::Tool::Slab | crate::Tool::BeamSystem | crate::Tool::Cable);
            let pending = app.beam_start.is_some() || !app.wall_pts.is_empty() || !app.slab_points.is_empty() || !app.run_points.is_empty() || app.cable_from.is_some();
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
        if let (Some(a), Some(h)) = (start, hover) {
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
        else if let Some(p) = pick { app.wall_click(level_id, to_model(p)); }
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
}
