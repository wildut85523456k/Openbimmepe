//! Orthographic plan and orbitable 3D projection of document-derived meshes.
//! Painter-based CPU projection is an initial small-model renderer, not a full
//! depth-buffered GPU BIM engine (cut planes/joins/hidden-line are future work).
use crate::{Desktop, ElementId, TempDim, View};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use openbim_document::{Category, Geometry, RangeClass};

/// How near (screen pixels) to a room's label a Select click picks the room instead of the slab it lies on.
const ROOM_LABEL_PICK_PX: f32 = 24.0;

/// The door or window of the wall `wall` whose opening holds the screen point `p`. A plan is cut below the head of a
/// door or window, so a pick there is the door or window, not the wall over it (as in Revit). None when `wall` is not
/// a wall or `p` is in none of its openings.
fn filler_in_opening(app: &Desktop, wall: ElementId, p: Pos2, screen: &dyn Fn([f64; 3]) -> Pos2, level_z: f64) -> Option<ElementId> {
    let w = app.document.get(wall).filter(|w| w.category == Category::Wall)?;
    let Some(Geometry::Linear { start, end, .. }) = w.geometry else { return None };
    let len = (end[0] - start[0]).hypot(end[1] - start[1]);
    if len < 1e-9 { return None; }
    let (d, n) = ([(end[0] - start[0]) / len, (end[1] - start[1]) / len], [-(end[1] - start[1]) / len, (end[0] - start[0]) / len]);
    let t = w.width_m;
    app.document.elements().filter(|e| e.category == Category::Family).find_map(|e| match e.geometry {
        Some(Geometry::InWall { host, along_m }) if host == wall => {
            let half = crate::export::opening_width_of(&app.document, e)? / 2.0;
            let at = |a: f64, side: f64| screen([start[0] + d[0] * a + n[0] * side * t / 2.0, start[1] + d[1] * a + n[1] * side * t / 2.0, level_z]);
            let quad = [at(along_m - half, -1.0), at(along_m + half, -1.0), at(along_m + half, 1.0), at(along_m - half, 1.0)];
            contains(&quad, p).then_some(e.id)
        }
        _ => None,
    })
}

/// The span of the plan view on a model with nothing in it yet, in metres: enough room to place a
/// level and a first wall, and close to what Revit shows for a new project.
const EMPTY_PLAN_SPAN_M: f32 = 10.0;

/// The rectangle the plan view fits to, in projected model units: the extent of every drawn mesh
/// and every grid. On a model with nothing in it yet (a new project's empty canvas), fall back to a
/// default view centred on the origin, so that the drawing tools still preview and accept clicks —
/// as in Revit, where a new project starts with an empty canvas.
fn plan_bounds(bounds: Rect) -> Rect {
    if bounds.is_finite() { bounds } else { Rect::from_center_size(Pos2::ZERO, Vec2::splat(EMPTY_PLAN_SPAN_M)) }
}

/// The plan-view tabs, from the project's levels (lowest first): the first two are the floor and the upper plan,
/// every other level gets a tab of its own. The tab is named by the level, and `View::Plan` / `View::UpperPlan`
/// resolve to the document's own levels (`plan_level`), so a new project's first two tabs point at levels that
/// exist: taking the first two by *position* rather than by the sample's ids 10/11 is what makes that true (A2).
fn plan_tabs(levels: &[(ElementId, String, f64)]) -> Vec<(String, View)> {
    levels.iter().enumerate()
        .map(|(k, (id, name, _))| (name.clone(), match k { 0 => View::Plan, 1 => View::UpperPlan, _ => View::Level(id.0) }))
        .collect()
}

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

/// The lines of a level a pointer in the plan snaps to: the location line and the edges of each wall, and the grids.
fn plan_edges(doc: &openbim_document::Document, level: ElementId) -> Vec<crate::view_tools::Edge> {
    use crate::view_tools::Edge;
    let mut out = Vec::new();
    let mut line = |a: [f64; 2], b: [f64; 2]| out.push(Edge { a, b, id: None, datum: false, centre: false });
    for el in doc.elements() {
        match el.geometry {
            Some(Geometry::Linear { start, end, level: l }) if el.category == Category::Wall && l == level => {
                line(start, end);
                if let Some(r) = doc.wall_plan(el.id) { for i in 0..4 { line(r[i], r[(i + 1) % 4]); } }
            }
            Some(Geometry::Grid { start, end }) => { for w in openbim_document::grid_arc::points(start, end, el.bulge).windows(2) { line(w[0], w[1]); } }
            _ => {}
        }
    }
    out
}

/// Where the pointer at `p` snaps in a plan: as in Revit, to the endpoints, intersections, midpoints and nearest points
/// of the lines (and the foot of the perpendicular from `base`, the last point picked), else to 50 mm. Shift keeps the
/// point level or plumb with `base` when nothing was caught. `xf` maps the plan to the screen.
fn plan_snap(edges: &[crate::view_tools::Edge], xf: &crate::section_ui::Xf, p: Pos2, base: Option<[f64; 2]>, ortho: bool) -> crate::view_tools::Snap {
    use crate::view_tools::{snap_at, SnapKind};
    let mut s = snap_at(edges, xf, p, 12.0, base, false, Some(0.05));
    if let (true, Some(b), SnapKind::Grid) = (ortho, base, s.kind) {
        if (s.at[0] - b[0]).abs() >= (s.at[1] - b[1]).abs() { s.at[1] = b[1]; } else { s.at[0] = b[0]; }
    }
    s
}

pub fn show(app: &mut Desktop, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        let level_tabs = plan_tabs(&app.levels());
        let sections: Vec<(String, View)> = app.document.elements().filter(|e| e.category == Category::Section).map(|e| (e.name.clone(), View::Section(e.id.0)))
            .chain(app.document.elements().filter(|e| e.category == Category::Elevation).flat_map(|e| (0..4u8).filter(move |k| e.count >> k & 1 == 1).map(move |k| (crate::section_ui::name_of_marker(&e.name, k), View::Marked(e.id.0, k)))))
            .collect();
        let tabs = level_tabs.into_iter()
            .chain([("{3D}".to_string(), View::ThreeD), ("Mechanical".to_string(), View::Mechanical)])
            .chain(crate::section_ui::FACINGS.iter().enumerate().map(|(k, f)| (f.1.to_string(), View::Elevation(k as u8))))
            .chain(sections);
        for (label, view) in tabs {
            if ui.selectable_label(app.view == view, label).clicked() { app.view = view; }
        }
        if ui.small_button("Fit").clicked() { app.pan = Vec2::ZERO; app.zoom = 1.0; }
        if ui.small_button("Clear selection").clicked() { app.select_element(None); }
    });
    app.view_control_bar(ui);
    // Spacebar turns the selected wall over
    if app.tool == crate::Tool::Select && !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Space)) { app.flip_selected_wall(); }
    // Delete removes the selected element with what stands on it (Backspace is the Delete key of a Mac keyboard)
    if app.tool == crate::Tool::Select && !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) { app.delete_selected(); }
    if app.view == View::ThreeD { return crate::view3d::show(app, ui); }
    if let View::Schedule(id) = app.view { return app.show_schedule(ui, id); }
    if let View::Sheet(id) = app.view { return app.show_sheet(ui, id); }
    if matches!(app.view, View::Elevation(_) | View::Section(_) | View::Marked(..)) { return crate::section_ui::show(app, ui); }
    let level_id = app.plan_level();
    let level_z = app.document.get(level_id).map_or(0.0, |el| el.elevation_m);
    let three_d = app.view == View::ThreeD;
    let vis = app.vis();
    let range = vis.settings.effective_range();
    let (detail, denom) = (vis.settings.detail, vis.settings.scale);
    let reveal_ink = Color32::from_rgb(220, 0, 220);
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
    // every mesh of the model is classed by the view range: out of it it is not drawn, above the cut plane it is faded
    let plan_meshes = app.plan_meshes();
    let mut meshes: Vec<(ElementId, Category, &openbim_document::Mesh, f32, bool)> = Vec::new();
    let mut classes: std::collections::HashMap<ElementId, RangeClass> = std::collections::HashMap::new();
    let mut out_of_range = 0usize;
    for pm in plan_meshes.iter() {
        let Some(el) = app.document.get(pm.id) else { continue };
        let shown = vis.state(el);
        if shown == crate::view_ui::Show::Hidden { continue; }
        let class = if three_d { RangeClass::Cut } else { crate::view_ui::plan_class(range, level_id, level_z, pm) };
        if class == RangeClass::Out { if pm.level == Some(level_id) { out_of_range += 1; } continue; }
        for vertex in &pm.mesh.vertices { bounds.extend_with(Pos2::ZERO + proj(*vertex).0); }
        classes.insert(pm.id, class);
        meshes.push((pm.id, el.category, &pm.mesh, crate::view_ui::range_fade(class), shown == crate::view_ui::Show::Revealed));
    }
    app.views.out_of_range = out_of_range;
    // Include datum extents so grid bubbles remain inside the viewport.
    for el in app.document.elements() {
        if let Some(Geometry::Grid { start, end }) = el.geometry {
            for [x, y] in openbim_document::grid_arc::points(start, end, el.bulge) { bounds.extend_with(Pos2::ZERO + proj([x, y, level_z]).0); }
        }
    }
    let bounds = plan_bounds(bounds);
    let fit = ((rect.width() - 100.0).max(10.0) / bounds.width().max(1.0))
        .min((rect.height() - 90.0).max(10.0) / bounds.height().max(1.0));
    let scale = fit * app.zoom;
    // annotations keep their size on paper: bigger on screen at a coarser scale (clamped so that they stay legible)
    let ann = |paper_mm: f32, min: f32, max: f32| crate::view_ui::annotation_px(paper_mm, denom, scale, min, max);
    let (bubble_r, note_px) = (ann(1.8, 6.0, 26.0), ann(2.0, 7.0, 20.0));
    let screen = |p| rect.center() + pan0 + (proj(p).0 - bounds.center().to_vec2()) * scale;
    #[cfg(test)]
    {
        app.plan_transform = Some((rect.center() + pan0, scale, bounds.center().to_vec2()));
    }
    let selected = app.selected_id();
    let selected_all = app.selected_ids();
    let blue = ui.visuals().hyperlink_color;
    let axes: Vec<_> = app.document.elements().flat_map(|el| match el.geometry {
        Some(Geometry::Grid { start, end } | Geometry::Linear { start, end, .. }) => vec![start, end],
        _ => vec![],
    }).collect();
    let left = axes.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let right = axes.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
    let back = axes.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    let mut datum_hits = Vec::new();
    // the levels of the project as a datum strip along the left edge of a plan: a line and a bubble for each, the
    // highest on top, the level of this view marked (B22). The levels outside the view range are the ones it is for.
    if !three_d {
        let ink = ui.visuals().text_color();
        let marks = level_bubbles(&app.levels(), level_id);
        for (row, (name, elevation, here)) in marks.iter().enumerate() {
            let c = Pos2::new(rect.left() + bubble_r + 10.0, rect.top() + bubble_r + 10.0 + row as f32 * (2.0 * bubble_r + 6.0));
            let stroke = Stroke::new(if *here { 2.0 } else { 1.0 }, if *here { blue } else { Color32::from_gray(135) });
            dashed(&painter, Pos2::new(c.x + bubble_r, c.y), Pos2::new(c.x + bubble_r + 26.0, c.y), stroke);
            painter.circle(c, bubble_r, ui.visuals().panel_fill, stroke);
            painter.text(Pos2::new(c.x + bubble_r + 30.0, c.y), egui::Align2::LEFT_CENTER, format!("{name}  {elevation:+.3}"), egui::FontId::proportional(note_px), if *here { blue } else { ink });
        }
    }
    // the places of the elevation markers of this level, to press on and move
    let mut markers: Vec<(ElementId, Pos2)> = Vec::new();
    // what a double click opens: section lines and elevation markers
    let mut view_hits: Vec<(View, Pos2, Pos2)> = Vec::new();
    for el in app.document.elements() {
        let shown = vis.state(el);
        if shown == crate::view_ui::Show::Hidden { continue; }
        let active = selected_all.contains(&el.id);
        let stroke = Stroke::new(if active { 2.5 } else { 1.0 }, if shown == crate::view_ui::Show::Revealed { reveal_ink } else if active { blue } else { Color32::from_gray(135) });
        match el.geometry {
            Some(Geometry::Grid { start, end }) => {
                let a = screen([start[0], start[1], level_z]);
                let b = screen([end[0], end[1], level_z]);
                for w in openbim_document::grid_arc::points(start, end, el.bulge).windows(2) { dashed(&painter, screen([w[0][0], w[0][1], level_z]), screen([w[1][0], w[1][1], level_z]), stroke); }
                for p in [a, b] {
                    painter.circle(p, bubble_r, ui.visuals().panel_fill, stroke);
                    painter.text(p, egui::Align2::CENTER_CENTER, &el.name, egui::FontId::proportional(note_px), stroke.color);
                }
                datum_hits.push((el.id, a, b));
            }
            Some(Geometry::Linear { start, end, level }) if el.category == Category::Section && !three_d && level == level_id => {
                // the section line with a tick toward the side the view looks to (the left of the line)
                let (a, b) = (screen([start[0], start[1], level_z]), screen([end[0], end[1], level_z]));
                let stroke = Stroke::new(if active { 2.5 } else { 1.5 }, if shown == crate::view_ui::Show::Revealed { reveal_ink } else if active { blue } else { Color32::from_rgb(190, 60, 60) });
                dashed(&painter, a, b, stroke);
                let d = b - a;
                if d.length() > 1.0 {
                    let left = Vec2::new(d.y, -d.x).normalized() * 18.0; // screen y points down, so the model's left is (dy, -dx)
                    let m = a + d * 0.5;
                    painter.arrow(m, left, stroke);
                    // the head: a bubble at the start with the number of the section, and a tick at the other end
                    let number = el.name.rsplit(' ').next().unwrap_or("");
                    painter.circle(a, bubble_r * 0.9, ui.visuals().panel_fill, stroke);
                    painter.text(a, egui::Align2::CENTER_CENTER, number, egui::FontId::proportional(note_px * 0.9), stroke.color);
                    painter.arrow(b, left * 0.6, stroke);
                    painter.text(a + (a - b).normalized() * 22.0, egui::Align2::CENTER_CENTER, &el.name, egui::FontId::proportional(11.0), stroke.color);
                }
                datum_hits.push((el.id, a, b));
                view_hits.push((View::Section(el.id.0), a, b));
            }
            Some(Geometry::Point { at, level, .. }) if el.category == Category::Elevation && !three_d && level == level_id => {
                // a circle with a pointer for each view the marker holds, pointing the way the view looks
                let c = screen([at[0], at[1], level_z]);
                let ink = if shown == crate::view_ui::Show::Revealed { reveal_ink } else if active { blue } else { Color32::from_rgb(120, 120, 120) };
                painter.circle(c, bubble_r, ui.visuals().panel_fill, Stroke::new(if active { 2.0 } else { 1.0 }, ink));
                painter.text(c, egui::Align2::CENTER_CENTER, el.name.rsplit(' ').next().unwrap_or(""), egui::FontId::proportional(note_px * 0.9), ink);
                for k in 0..4u8 {
                    if el.count >> k & 1 == 0 { continue; }
                    let f = crate::section_ui::facing_vector(usize::from(k));
                    let toward = Vec2::new(f[0] as f32, -f[1] as f32); // screen y points down
                    let side = Vec2::new(-toward.y, toward.x) * 5.0;
                    let tip = c + toward * 21.0;
                    painter.add(egui::Shape::convex_polygon(vec![c + toward * 11.0 + side, c + toward * 11.0 - side, tip], ink, Stroke::NONE));
                    view_hits.push((View::Marked(el.id.0, k), tip, tip));
                }
                datum_hits.push((el.id, c, c));
                markers.push((el.id, c));
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
    // elevation markers: a circle with a pointer towards the building at the middle of each of the four lines
    if !three_d && !vis.category_off(openbim_document::ViewCategory::Elevations) {
        if app.elev_marks.as_ref().is_none_or(|m| m.0 != app.document.revision()) {
            app.elev_marks = Some((app.document.revision(), crate::section_ui::elevation_lines(&app.document)));
        }
        let marks = app.elev_marks.as_ref().map(|m| m.1.clone()).unwrap_or_default();
        for (k, a, b) in marks {
            let (pa, pb) = (screen([a[0], a[1], level_z]), screen([b[0], b[1], level_z]));
            let (mid, d) = (pa + (pb - pa) * 0.5, pb - pa);
            if d.length() < 1.0 { continue; }
            let toward = Vec2::new(d.y, -d.x).normalized(); // the left of a -> b on the screen: where the view looks
            let mid = mid - toward * 30.0; // the marker stands off the line, outside the building, so that it is not drawn over the walls
            let ink = Color32::from_rgb(120, 120, 120);
            painter.circle(mid, 11.0, ui.visuals().panel_fill, Stroke::new(1.0, ink));
            let tip = mid + toward * 18.0;
            let side = Vec2::new(-toward.y, toward.x) * 5.0;
            painter.add(egui::Shape::convex_polygon(vec![mid + toward * 11.0 + side, mid + toward * 11.0 - side, tip], ink, Stroke::NONE));
            painter.text(mid - toward * 22.0, egui::Align2::CENTER_CENTER, crate::section_ui::FACINGS[usize::from(k)].1, egui::FontId::proportional(11.0), ink);
            view_hits.push((View::Elevation(k), mid, mid));
        }
    }
    // a double click on a section line or an elevation marker opens its view, as in Revit
    if !three_d && app.tool == crate::Tool::Select && response.double_clicked() {
        if let Some(p) = response.interact_pointer_pos() {
            if let Some(v) = crate::section_ui::view_at(&view_hits, p, 12.0) { app.view = v; app.select_element(None); }
        }
    }
    let mut faces = Vec::new();
    for (id, category, mesh, fade, revealed) in meshes {
        let color = match category {
            Category::Wall => if app.dark { Color32::from_gray(115) } else { Color32::from_gray(195) },
            Category::Family => Color32::from_rgb(120, 150, 110),
            Category::Beam | Category::BeamSystem | Category::Truss => Color32::from_rgb(176, 140, 104),
            Category::Railing => Color32::from_rgb(120, 120, 130),
            Category::Column => Color32::from_rgb(150, 150, 160),
            Category::Footing | Category::WallFooting => Color32::from_rgb(160, 150, 135),
            Category::Slab if app.document.get(id).is_some_and(|e| e.slab_kind == openbim_document::SlabKind::Ceiling) => if three_d { Color32::from_rgb(232, 230, 220) } else { Color32::from_rgba_unmultiplied(232, 230, 220, 70) },
            Category::Slab if app.document.get(id).is_some_and(|e| e.is_roof()) => if three_d { Color32::from_rgb(178, 96, 78) } else { Color32::from_rgba_unmultiplied(178, 96, 78, 90) },
            Category::Slab => if app.dark { Color32::from_rgb(92, 92, 100) } else { Color32::from_rgb(205, 205, 212) },
            Category::Generic => Color32::from_rgb(158, 140, 170),
            Category::Duct => Color32::from_rgb(70, 157, 194),
            Category::Pipe => Color32::from_rgb(136, 153, 166),
            _ => Color32::GRAY,
        };
        let color = if revealed { reveal_ink } else { color };
        // in a plan a roof is under everything else: it is drawn first and picked last
        let under = !three_d && app.document.get(id).is_some_and(|e| e.is_roof() || (e.category == Category::Slab && e.slab_kind == openbim_document::SlabKind::Ceiling));
        for (i, face) in mesh.faces.iter().enumerate() {
            let points: Vec<_> = face.iter().map(|&v| screen(mesh.vertices[v])).collect();
            let area = points.iter().zip(points.iter().cycle().skip(1)).take(points.len())
                .map(|(a, b)| a.x * b.y - b.x * a.y).sum::<f32>().abs();
            if area < 0.2 { continue; }
            let depth = if under { f32::MIN / 2.0 } else { face.iter().map(|&v| proj(mesh.vertices[v]).1).sum::<f32>() / face.len() as f32 };
            faces.push((depth, id, points, color.gamma_multiply(fade * if three_d { 0.72 + (i % 3) as f32 * 0.12 } else { 1.0 })));
        }
    }
    // rooms of this level (plan only): a tinted floor under everything, picked last; a room without a closed space shows a warning
    let mut room_labels: Vec<(ElementId, Pos2, String, bool)> = Vec::new();
    if !three_d {
        let rooms: Vec<(ElementId, [f64; 2], String)> = app.document.elements().filter_map(|e| match e.geometry {
            Some(Geometry::Room { at, level }) if level == level_id && !vis.hidden(e) => Some((e.id, at, e.name.clone())),
            _ => None,
        }).collect();
        for (id, at, name) in rooms {
            let shape = app.document.room_shape(id);
            if let Ok(shape) = &shape {
                let tint = Color32::from_rgba_unmultiplied(110, 170, 220, if app.dark { 60 } else { 70 });
                if let Some(tris) = openbim_geom::profile::triangulate(&shape.outline) {
                    let mut mesh = egui::epaint::Mesh::default();
                    for p in &shape.outline { mesh.colored_vertex(screen([p[0], p[1], level_z]), tint); }
                    for t in tris { mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32); }
                    painter.add(egui::Shape::mesh(mesh));
                }
                let pts: Vec<Pos2> = shape.outline.iter().map(|p| screen([p[0], p[1], level_z])).collect();
                faces.push((f32::MIN, id, pts, Color32::TRANSPARENT));
                room_labels.push((id, screen([at[0], at[1], level_z]), format!("{}\n{:.1} m²", name, shape.area()), true));
            } else {
                room_labels.push((id, screen([at[0], at[1], level_z]), format!("{name}\nNot Enclosed"), false));
            }
        }
    }
    // openings (plan only): a wall opening is two dashed jambs across its wall, a shaft a dashed outline crossed by two
    // lines on every level it passes through; each can be picked on its symbol
    let mut opening_marks: Vec<(ElementId, Vec<Pos2>, bool)> = Vec::new();
    if !three_d {
        for el in app.document.elements().filter(|e| e.category == Category::Opening && !vis.hidden(e)) {
            match el.geometry {
                Some(Geometry::InWall { host, along_m }) => {
                    let Some(w) = app.document.get(host) else { continue };
                    let Some(Geometry::Linear { start, end, level }) = w.geometry else { continue };
                    if level != level_id { continue; }
                    let len = (end[0] - start[0]).hypot(end[1] - start[1]);
                    let (d, n) = ([(end[0] - start[0]) / len, (end[1] - start[1]) / len], [-(end[1] - start[1]) / len, (end[0] - start[0]) / len]);
                    let t = w.width_m / 2.0;
                    let at = |a: f64, side: f64| screen([start[0] + d[0] * a + n[0] * side * t, start[1] + d[1] * a + n[1] * side * t, level_z]);
                    let (a, b) = (along_m - el.width_m / 2.0, along_m + el.width_m / 2.0);
                    let quad = vec![at(a, -1.0), at(b, -1.0), at(b, 1.0), at(a, 1.0)];
                    faces.push((f32::MAX / 2.0, el.id, quad.clone(), Color32::TRANSPARENT));
                    opening_marks.push((el.id, quad, false));
                }
                Some(Geometry::Shaft { .. }) => {
                    let Some((lo, hi)) = app.document.shaft_range(el.id) else { continue };
                    if level_z < lo - 1e-6 || level_z > hi + 1e-6 { continue; }
                    let ring: Vec<Pos2> = el.outline.iter().map(|p| screen([p[0], p[1], level_z])).collect();
                    faces.push((f32::MIN / 2.0, el.id, ring.clone(), Color32::TRANSPARENT));
                    opening_marks.push((el.id, ring, true));
                }
                // a vertical opening: on the level of its host
                Some(Geometry::Area { level }) if level == level_id => {
                    let ring: Vec<Pos2> = el.outline.iter().map(|p| screen([p[0], p[1], level_z])).collect();
                    faces.push((f32::MIN / 2.0, el.id, ring.clone(), Color32::TRANSPARENT));
                    opening_marks.push((el.id, ring, true));
                }
                _ => {}
            }
        }
    }
    faces.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (id, ring, shaft) in &opening_marks {
        let shown = selected_all.contains(id);
        let c = if shown { blue } else { ui.visuals().text_color() };
        let stroke = Stroke::new(if shown { 2.0 } else { 1.0 }, c);
        if *shaft {
            for i in 0..ring.len() { dashed(&painter, ring[i], ring[(i + 1) % ring.len()], stroke); }
            if ring.len() >= 3 {
                let r = ring.iter().fold(Rect::NOTHING, |r, p| r.union(Rect::from_min_max(*p, *p)));
                painter.line_segment([r.left_top(), r.right_bottom()], Stroke::new(0.8, c.gamma_multiply(0.7)));
                painter.line_segment([r.right_top(), r.left_bottom()], Stroke::new(0.8, c.gamma_multiply(0.7)));
            }
        } else {
            dashed(&painter, ring[0], ring[3], stroke);
            dashed(&painter, ring[1], ring[2], stroke);
        }
    }
    for (_, id, points, color) in &faces {
        painter.add(egui::Shape::convex_polygon(points.clone(), *color,
            Stroke::new(if selected_all.contains(id) { 2.0 } else { 0.8 },
                if selected_all.contains(id) { blue } else { Color32::from_gray(80) })));
    }
    if !three_d {
        // the layers of a layered wall: a thin line where each ends, a heavier one at the core boundaries
        let ink = if app.dark { Color32::from_gray(170) } else { Color32::from_gray(90) };
        for el in app.document.elements().filter(|e| e.category == Category::Wall && e.type_id.is_some()) {
            if app.document.level_of(el.id) != Some(level_id) || detail == openbim_document::Detail::Coarse || !classes.contains_key(&el.id) { continue; }
            for (a, b, core) in app.document.wall_layer_lines(el.id) {
                painter.line_segment([screen([a[0], a[1], level_z]), screen([b[0], b[1], level_z])], Stroke::new(if core { 1.2 } else { 0.5 }, ink));
            }
        }
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
        // aligned dimensions of this level; each can be picked on its line
        let ink = ui.visuals().text_color();
        let dims: Vec<ElementId> = app.document.elements().filter(|e| e.category == Category::Dimension && !vis.hidden(e)).map(|e| e.id).collect();
        for id in dims {
            let Some(Geometry::Dimension { a, b, at, level }) = app.document.get(id).and_then(|d| d.geometry) else { continue };
            if level != level_id { continue; }
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = dx.hypot(dy);
            let off = ((at[0] - a[0]) * -dy + (at[1] - a[1]) * dx) / l;
            let (nx, ny) = (-dy / l * off, dx / l * off);
            let (q1, q2) = (screen([a[0] + nx, a[1] + ny, level_z]), screen([b[0] + nx, b[1] + ny, level_z]));
            let shown = selected_all.contains(&id);
            draw_dimension(&painter, screen([a[0], a[1], level_z]), screen([b[0], b[1], level_z]), q1, q2, &format!("{:.0}", l * 1000.0), if shown { blue } else { ink });
            datum_hits.push((id, q1, q2));
        }
    }
    if !three_d {
        // the plan symbol of a stair: an arrow up its centre line from the first riser, with UP at the foot
        let ink = ui.visuals().text_color();
        for el in app.document.elements().filter(|e| e.category == Category::Stair && !vis.hidden(e)) {
            let Some(Geometry::Linear { start, end, level }) = el.geometry else { continue };
            if level != level_id { continue; }
            let (a, b) = (screen([start[0], start[1], level_z]), screen([end[0], end[1], level_z]));
            let shown = selected_all.contains(&el.id);
            let c = if shown { blue } else { ink };
            painter.line_segment([a, b], Stroke::new(1.0, c));
            painter.circle_filled(a, 2.5, c);
            let dir = (b - a).normalized();
            if dir.length() > 0.0 {
                let side = Vec2::new(-dir.y, dir.x);
                painter.line_segment([b, b - dir * 9.0 + side * 4.5], Stroke::new(1.0, c));
                painter.line_segment([b, b - dir * 9.0 - side * 4.5], Stroke::new(1.0, c));
            }
            painter.text(a + Vec2::new(6.0, -4.0), egui::Align2::LEFT_BOTTOM, "UP", egui::FontId::proportional(11.0), c);
        }
    }
    for (id, at, text, closed) in &room_labels {
        let ink = if *closed { ui.visuals().text_color() } else { Color32::from_rgb(200, 60, 60) };
        let shown = selected_all.contains(id);
        painter.line_segment([*at - Vec2::new(5.0, 0.0), *at + Vec2::new(5.0, 0.0)], Stroke::new(1.0, if shown { blue } else { ink }));
        painter.line_segment([*at - Vec2::new(0.0, 5.0), *at + Vec2::new(0.0, 5.0)], Stroke::new(1.0, if shown { blue } else { ink }));
        painter.text(*at + Vec2::new(0.0, 8.0), egui::Align2::CENTER_TOP, text, egui::FontId::proportional(note_px), if shown { blue } else { ink });
    }
    if !three_d {
        // tags: the label of an element, boxed, at a constant size, on a leader to the place that was clicked
        let tag_ids: Vec<ElementId> = app.document.elements().filter(|e| e.category == Category::Tag && !vis.hidden(e)).map(|e| e.id).collect();
        for id in tag_ids {
            let Some(Geometry::Tag { host, at, anchor }) = app.document.get(id).and_then(|t| t.geometry) else { continue };
            if app.document.level_of(host) != Some(level_id) { continue; }
            let (head, tip) = (screen([at[0], at[1], level_z]), screen([anchor[0], anchor[1], level_z]));
            let text = app.document.tag_text(id);
            let ink = ui.visuals().text_color();
            if head.distance(tip) > 2.0 { painter.line_segment([head, tip], Stroke::new(1.0, ink)); painter.circle_filled(tip, 2.5, ink); }
            let galley = painter.layout_no_wrap(text, egui::FontId::proportional(note_px), ink);
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
            let starts = matches!(app.tool, crate::Tool::Wall | crate::Tool::Grid | crate::Tool::Beam | crate::Tool::Truss | crate::Tool::Duct | crate::Tool::Pipe | crate::Tool::Slab | crate::Tool::Shaft | crate::Tool::VerticalOpening | crate::Tool::BeamSystem | crate::Tool::Cable | crate::Tool::Move | crate::Tool::Copy | crate::Tool::Mirror | crate::Tool::Dimension | crate::Tool::Measure | crate::Tool::Stair | crate::Tool::Section);
            let pending = !app.edit_pts.is_empty() || app.beam_start.is_some() || !app.wall_pts.is_empty() || !app.slab_points.is_empty() || !app.run_points.is_empty() || app.cable_from.is_some();
            if starts && !pending { ui.input(|i| i.pointer.press_origin()) } else { None }
        }
        else if response.drag_stopped_by(egui::PointerButton::Primary) { response.interact_pointer_pos().or(response.hover_pos()) }
        else { None };
    // a pushpin on every pinned element of this level
    if !three_d {
        for m in app.pin_marks(level_id) {
            let p = screen([m[0], m[1], level_z]) + Vec2::new(0.0, -10.0);
            painter.line_segment([p, p + Vec2::new(0.0, 12.0)], Stroke::new(1.5, Color32::from_rgb(60, 60, 60)));
            painter.circle(p, 4.5, Color32::from_rgb(220, 140, 0), Stroke::new(1.0, Color32::from_rgb(60, 60, 60)));
        }
    }
    // the insulation round ducts and pipes of this level
    if !three_d {
        for o in app.insulation_outlines(level_id) {
            let pts: Vec<Pos2> = o.iter().map(|p| screen([p[0], p[1], level_z])).collect();
            painter.add(egui::Shape::closed_line(pts, Stroke::new(1.2, Color32::from_rgb(214, 190, 70))));
        }
    }
    if !three_d {
        // the placeholders of Generate Layout: the trunk heavier, the branches lighter, dashed
        for (trunk, pts) in app.layout_placeholders(level_id) {
            let stroke = Stroke::new(if trunk { 3.0 } else { 2.0 }, if trunk { Color32::from_rgb(220, 140, 0) } else { Color32::from_rgb(70, 130, 200) });
            for w in pts.windows(2) { dashed(&painter, screen([w[0][0], w[0][1], level_z]), screen([w[1][0], w[1][1], level_z]), stroke); }
        }
    }
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
        let edges = plan_edges(&app.document, level_id);
        let xf = crate::section_ui::Xf { centre: centre + pan, scale, cu: f64::from(bc.x), cz: -f64::from(bc.y) };
        let ortho = ui.input(|i| i.modifiers.shift);
        let start = app.wall_pts.last().copied();
        let snap_to = |p: Pos2| -> ([f64; 2], crate::view_tools::SnapKind) { let s = plan_snap(&edges, &xf, p, start, ortho); (s.at, s.kind) };
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
        // polygon, circle and arc: the walls that the points picked and the pointer would make
        if app.wall_mode >= 3 && !app.wall_pts.is_empty() {
            if let Some(h) = hover {
                let (b, _) = snap_to(h);
                let mut pts = app.wall_pts.clone();
                pts.push(b);
                // an arc with two points is shown as the line between them until the third is under the pointer
                let lines = app.shape_lines(&pts).unwrap_or_default();
                for (s, t) in &lines {
                    let (dx, dy) = (t[0] - s[0], t[1] - s[1]);
                    let l = dx.hypot(dy).max(1e-9);
                    let (nx, ny) = (-dy / l * app.width_mm / 2000.0, dx / l * app.width_mm / 2000.0);
                    let ring = [[s[0] - nx, s[1] - ny], [t[0] - nx, t[1] - ny], [t[0] + nx, t[1] + ny], [s[0] + nx, s[1] + ny]].map(|p| screen([p[0], p[1], level_z]));
                    painter.add(egui::Shape::convex_polygon(ring.to_vec(), Color32::from_rgba_unmultiplied(120, 120, 130, 110), Stroke::new(1.0, blue)));
                }
                let a = app.wall_pts[0];
                dashed(&painter, screen([a[0], a[1], level_z]), screen([b[0], b[1], level_z]), Stroke::new(1.0, blue));
                painter.text(screen([b[0], b[1], level_z]) + Vec2::new(10.0, -10.0), egui::Align2::LEFT_BOTTOM, format!("{:.0} mm", (b[0] - a[0]).hypot(b[1] - a[1]) * 1000.0), egui::FontId::proportional(13.0), blue);
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
            let (m, kind) = snap_to(h);
            crate::view_tools::draw_snap(&painter, screen([m[0], m[1], level_z]), kind, Color32::from_rgb(220, 140, 0));
        }
        let typed: String = ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None }).collect());
        if app.wall_pts.last().is_some() && app.wall_mode == 0 { for c in typed.chars().filter(|c| c.is_ascii_digit() || *c == '.') { app.wall_len.push(c); } }
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
    } else if app.tool == crate::Tool::Stair && !three_d {
        // the foot of the first riser, then the direction; the treads are drawn from the first point toward the pointer
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let ortho = ui.input(|i| i.modifiers.shift);
        let first = app.edit_pts.first().copied();
        let to_model = |p: Pos2| {
            let v = (p - centre - pan) / scale + bc;
            let mut m = [(v.x as f64 / 0.05).round() * 0.05, (-v.y as f64 / 0.05).round() * 0.05];
            if let (true, Some(s)) = (ortho, first) { if (m[0] - s[0]).abs() >= (m[1] - s[1]).abs() { m[1] = s[1]; } else { m[0] = s[0]; } }
            m
        };
        if let (Some(s), Some(h)) = (first, response.hover_pos()) {
            let m = to_model(h);
            let l = (m[0] - s[0]).hypot(m[1] - s[1]);
            if l > 0.05 {
                let rise = match app.document.level_above(level_id) {
                    Some(a) => app.document.get(a).map_or(0.0, |a| a.elevation_m) - app.document.get(level_id).map_or(0.0, |l| l.elevation_m) - app.elevation_mm / 1000.0,
                    None => app.stair_rise_mm / 1000.0,
                };
                if rise >= 0.2 {
                    let n = openbim_document::risers_for(rise, app.stair_riser_mm / 1000.0);
                    let (t, w) = (app.stair_tread_mm / 1000.0, app.stair_width_mm / 1000.0);
                    let spec = openbim_document::StairSpec { start: s, toward: m, width_m: w, rise_m: rise, risers: n, tread_m: t, base_offset_m: 0.0 };
                    // each run drawn as its treads between two edges; a turning stair is two runs with the landing outlined
                    let run = |a: [f64; 2], b: [f64; 2], risers: u32| {
                        let len = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9);
                        let (d, right) = ([(b[0] - a[0]) / len, (b[1] - a[1]) / len], [(b[1] - a[1]) / len, -(b[0] - a[0]) / len]);
                        let pt = |along: f64, lat: f64| screen([a[0] + d[0] * along + right[0] * lat, a[1] + d[1] * along + right[1] * lat, level_z]);
                        for i in 0..risers { let x = f64::from(i) * t; painter.line_segment([pt(x, -w / 2.0), pt(x, w / 2.0)], Stroke::new(1.0, blue)); }
                        let far = f64::from(risers - 1) * t;
                        painter.line_segment([pt(0.0, -w / 2.0), pt(far, -w / 2.0)], Stroke::new(1.5, blue));
                        painter.line_segment([pt(0.0, w / 2.0), pt(far, w / 2.0)], Stroke::new(1.5, blue));
                        pt(far, 0.0)
                    };
                    let (label_at, risers_text) = match openbim_document::stair_layout(&spec, app.stair_shape) {
                        Some(lay) => {
                            run(lay.run1[0], [lay.run1[0][0] + (m[0] - s[0]) / l, lay.run1[0][1] + (m[1] - s[1]) / l], lay.risers[0]);
                            let corners = lay.landing.len();
                            for i in 0..corners {
                                let (p, q) = (lay.landing[i], lay.landing[(i + 1) % corners]);
                                painter.line_segment([screen([p[0], p[1], level_z]), screen([q[0], q[1], level_z])], Stroke::new(1.5, blue));
                            }
                            let end = run(lay.run2[0], lay.run2[1], lay.risers[1]);
                            (end, format!("{} risers ({} + {}), {:.0} mm", spec.risers, lay.risers[0], lay.risers[1], rise / f64::from(spec.risers) * 1000.0))
                        }
                        None => (run(s, m, n), format!("{n} risers, {:.0} mm", rise / f64::from(n) * 1000.0)),
                    };
                    painter.text(label_at + Vec2::new(10.0, -10.0), egui::Align2::LEFT_BOTTOM, risers_text, egui::FontId::proportional(13.0), blue);
                }
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if app.edit_pts.is_empty() { app.choose_tool(crate::Tool::Select); } else { app.edit_pts.clear(); }
        } else if let Some(p) = pick { app.stair_click(level_id, to_model(p)); }
    } else if app.tool == crate::Tool::Room && !three_d {
        // the space under the pointer is previewed; a click places a room in it
        if let Some(h) = response.hover_pos() {
            let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
            let at = { let v = (h - centre - pan) / scale + bc; [v.x as f64, -v.y as f64] };
            match app.document.room_region_at(level_id, at) {
                Ok(shape) => {
                    let pts: Vec<Pos2> = shape.outline.iter().map(|p| screen([p[0], p[1], level_z])).collect();
                    painter.add(egui::Shape::closed_line(pts, Stroke::new(2.0, blue)));
                    painter.text(h + Vec2::new(12.0, -12.0), egui::Align2::LEFT_BOTTOM, format!("{:.1} m²", shape.area()), egui::FontId::proportional(13.0), blue);
                }
                Err(_) => { painter.text(h + Vec2::new(12.0, -12.0), egui::Align2::LEFT_BOTTOM, "not enclosed", egui::FontId::proportional(13.0), Color32::from_rgb(200, 60, 60)); }
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); }
        else if let Some(p) = pick {
            let at = { let v = (p - rect.center() - app.pan) / scale + bounds.center().to_vec2(); [v.x as f64, -v.y as f64] };
            app.room_click(level_id, at);
        }
    } else if app.tool == crate::Tool::Section && !three_d {
        // two picks, snapped to 50 mm: the ends of the line; a rubber band shows it with the side the view looks to
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [(v.x as f64 / 0.05).round() * 0.05, (-v.y as f64 / 0.05).round() * 0.05] };
        if let (Some(a), Some(h)) = (app.edit_pts.first().copied(), response.hover_pos()) {
            let b = to_model(h);
            dashed(&painter, screen([a[0], a[1], level_z]), screen([b[0], b[1], level_z]), Stroke::new(1.5, Color32::from_rgb(190, 60, 60)));
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { if app.edit_pts.is_empty() { app.choose_tool(crate::Tool::Select); } else { app.edit_pts.clear(); } }
        else if let Some(p) = pick { app.section_click(level_id, to_model(p)); }
    } else if app.tool == crate::Tool::Elevation && !three_d {
        // one click puts a marker with its four views; a ghost follows the pointer, snapped to 50 mm
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [(v.x as f64 / 0.05).round() * 0.05, (-v.y as f64 / 0.05).round() * 0.05] };
        if let Some(h) = response.hover_pos() {
            let m = to_model(h);
            painter.circle_stroke(screen([m[0], m[1], level_z]), 11.0, Stroke::new(1.5, Color32::from_rgb(120, 120, 120)));
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); }
        else if let Some(p) = pick { app.elevation_click(level_id, to_model(p)); }
    } else if app.tool == crate::Tool::WallJoins && !three_d {
        // The ends of the walls of this level are marked: green where the join is allowed, red where it is not.
        let mut ends: Vec<(ElementId, bool, Pos2, bool)> = Vec::new();
        for el in app.document.elements().filter(|e| e.category == Category::Wall) {
            if let Some(Geometry::Linear { start, end, level }) = el.geometry {
                if level != level_id { continue; }
                ends.push((el.id, true, screen([start[0], start[1], level_z]), el.join_ends[0]));
                ends.push((el.id, false, screen([end[0], end[1], level_z]), el.join_ends[1]));
            }
        }
        let hover = response.hover_pos();
        let near = hover.and_then(|h| ends.iter().filter(|e| e.2.distance(h) < 14.0).min_by(|a, b| a.2.distance(h).total_cmp(&b.2.distance(h))).copied());
        for (_, _, p, allowed) in &ends {
            let c = if *allowed { Color32::from_rgb(60, 160, 80) } else { Color32::from_rgb(200, 60, 60) };
            painter.circle_stroke(*p, 5.0, Stroke::new(1.5, c));
        }
        if let Some((_, _, p, _)) = near { painter.circle_stroke(p, 9.0, Stroke::new(2.0, blue)); }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); }
        else if response.clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                match ends.iter().filter(|e| e.2.distance(p) < 14.0).min_by(|a, b| a.2.distance(p).total_cmp(&b.2.distance(p))) {
                    Some(&(id, at_start, _, _)) => app.wall_join_click(id, at_start),
                    None => app.message = "Wall Joins: click the end of a wall".into(),
                }
            }
        }
    } else if matches!(app.tool, crate::Tool::Dimension | crate::Tool::Measure) && !three_d {
        // Two points (snapped as in Revit: endpoints, midpoints, crossings, perpendicular, nearest, else 50 mm); the Dimension tool then takes the place of its line.
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let edges = plan_edges(&app.document, level_id);
        let xf = crate::section_ui::Xf { centre: centre + pan, scale, cu: f64::from(bc.x), cz: -f64::from(bc.y) };
        let ortho = ui.input(|i| i.modifiers.shift);
        let first = app.edit_pts.first().copied();
        let snap_to = |p: Pos2| -> ([f64; 2], crate::view_tools::SnapKind) { let s = plan_snap(&edges, &xf, p, first, ortho); (s.at, s.kind) };
        let hover = response.hover_pos();
        let ink = Color32::from_rgb(220, 140, 0);
        if let Some(h) = hover {
            let (m, kind) = snap_to(h);
            crate::view_tools::draw_snap(&painter, screen([m[0], m[1], level_z]), kind, ink);
            match app.edit_pts.as_slice() {
                [a] => {
                    let (pa, pb) = (screen([a[0], a[1], level_z]), screen([m[0], m[1], level_z]));
                    painter.line_segment([pa, pb], Stroke::new(1.0, blue));
                    painter.text(pb + Vec2::new(10.0, -10.0), egui::Align2::LEFT_BOTTOM, format!("{:.0} mm", (m[0] - a[0]).hypot(m[1] - a[1]) * 1000.0), egui::FontId::proportional(13.0), blue);
                }
                [a, b] => {
                    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                    let l = dx.hypot(dy).max(1e-9);
                    let off = ((m[0] - a[0]) * -dy + (m[1] - a[1]) * dx) / l;
                    let (nx, ny) = (-dy / l * off, dx / l * off);
                    draw_dimension(&painter, screen([a[0], a[1], level_z]), screen([b[0], b[1], level_z]), screen([a[0] + nx, a[1] + ny, level_z]), screen([b[0] + nx, b[1] + ny, level_z]), &format!("{:.0}", l * 1000.0), blue);
                }
                _ => {}
            }
        }
        if app.tool == crate::Tool::Measure {
            if let Some((a, b)) = app.measured {
                let (pa, pb) = (screen([a[0], a[1], level_z]), screen([b[0], b[1], level_z]));
                painter.line_segment([pa, pb], Stroke::new(1.5, ink));
                for p in [pa, pb] { painter.circle_filled(p, 3.0, ink); }
                painter.text(pa + (pb - pa) * 0.5 + Vec2::new(8.0, -8.0), egui::Align2::LEFT_BOTTOM, format!("{:.0} mm", (b[0] - a[0]).hypot(b[1] - a[1]) * 1000.0), egui::FontId::proportional(13.0), ink);
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { if app.edit_pts.is_empty() { app.choose_tool(crate::Tool::Select); } else { app.edit_pts.clear(); } }
        else if let Some(p) = pick {
            let (m, _) = snap_to(p);
            if app.tool == crate::Tool::Dimension { app.dimension_click(level_id, m); } else { app.measure_click(m); }
        }
    } else if matches!(app.tool, crate::Tool::Move | crate::Tool::Copy | crate::Tool::Rotate | crate::Tool::Mirror) && !three_d {
        // As in Revit: the element is selected first (a click selects one when none is), then the tool picks its points:
        // a base point and where to, a centre and the two rays of a turn, the two points of a mirror axis. The pointer
        // snaps as in Revit (endpoint, midpoint, intersection, perpendicular, nearest, else 50 mm); Shift keeps a move square; a ghost shows the result.
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let raw = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [v.x as f64, -v.y as f64] };
        let edges = plan_edges(&app.document, level_id);
        let xf = crate::section_ui::Xf { centre: centre + pan, scale, cu: f64::from(bc.x), cz: -f64::from(bc.y) };
        let ortho = ui.input(|i| i.modifiers.shift);
        let first = app.edit_pts.first().copied();
        let snap_to = |p: Pos2| -> [f64; 2] { plan_snap(&edges, &xf, p, first, ortho).at };
        let typed: String = ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None }).collect());
        if !app.edit_pts.is_empty() { for c in typed.chars() { if c.is_ascii_digit() || c == '.' || (c == '-' && app.edit_num.is_empty()) { app.edit_num.push(c); } } }
        let hover = response.hover_pos();
        // the ghost: the selected element's outline after the change the next pick would make
        let moving: Vec<ElementId> = if app.edit_src.is_empty() { selected_all.clone() } else { app.edit_src.clone() };
        if let (false, Some(h)) = (moving.is_empty(), hover) {
            let next = snap_to(h);
            let xf = match app.edit_num.parse::<f64>() {
                Ok(v) if app.tool == crate::Tool::Rotate => app.edit_pts.first().map(|c| openbim_document::Xf::Rotate(*c, v.to_radians())),
                Ok(v) => first.and_then(|a| { let (dx, dy) = (next[0] - a[0], next[1] - a[1]); let l = dx.hypot(dy); (l > 1e-9).then(|| openbim_document::Xf::Move([dx / l * v / 1000.0, dy / l * v / 1000.0])) }),
                Err(_) => app.edit_xf(app.tool, next),
            };
            let ghost = Stroke::new(1.5, Color32::from_rgb(220, 140, 0));
            if let Some(xf) = xf {
                for (_, id, points, _) in faces.iter().filter(|f| moving.contains(&f.1)) {
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
    } else if app.tool == crate::Tool::Paste && !three_d {
        // the clipboard follows the pointer by its centre, snapped to 50 mm; a click puts it down, Esc ends
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [(v.x as f64 / 0.05).round() * 0.05, (-v.y as f64 / 0.05).round() * 0.05] };
        if let (Some(cb), Some(h)) = (app.clipboard.as_ref(), response.hover_pos()) {
            let at = to_model(h);
            let d = [at[0] - cb.base[0], at[1] - cb.base[1]];
            let ghost = Stroke::new(1.5, Color32::from_rgb(220, 140, 0));
            for shape in cb.shapes(d) {
                let pts: Vec<Pos2> = shape.iter().map(|q| screen([q[0], q[1], level_z])).collect();
                match pts.len() { 1 => { painter.circle_stroke(pts[0], 6.0, ghost); } 2 => { painter.line_segment([pts[0], pts[1]], ghost); } _ => { painter.add(egui::Shape::closed_line(pts, ghost)); } }
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.choose_tool(crate::Tool::Select); }
        else if let Some(p) = pick { app.paste_at(to_model(p)); }
    } else if matches!(app.tool, crate::Tool::Align | crate::Tool::Trim | crate::Tool::Offset | crate::Tool::MatchType) && !three_d {
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
            // a door or window is tagged by a click in its opening, as it is selected
            let hit = hit.map(|h| filler_in_opening(app, h, p, &screen, level_z).unwrap_or(h));
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
        else if ui.input(|i| i.key_pressed(egui::Key::Space)) && !ui.ctx().egui_wants_keyboard_input() { app.angle_deg = (app.angle_deg + 90.0) % 360.0; app.message = format!("{} turned to {:.0} degrees", app.family_pick, app.angle_deg); }
        else if ui.input(|i| i.key_pressed(egui::Key::Tab)) && !ui.ctx().egui_wants_keyboard_input() { app.family_snap_k += 1; app.message = format!("{}: the next connector of the family meets the open one", app.family_pick); }
        else if pick.is_some() {
            if let Some(p) = pick {
                let wall = faces.iter().rev().find(|(_, id, polygon, _)| contains(polygon, p) && app.document.get(*id).is_some_and(|e| e.category == Category::Wall)).map(|(_, id, _, _)| *id);
                app.family_click(level_id, to_model(p), wall);
            }
        }
        // the ghost: the family where a click would put it, before the click
        if let Some(h) = response.hover_pos().filter(|_| pick.is_none() && !ui.input(|i| i.key_pressed(egui::Key::Escape))) {
            if let Some(g) = app.family_ghost(level_id, to_model(h)) {
                let ink = if g.refused { Color32::from_rgb(200, 70, 60) } else { Color32::from_rgb(70, 130, 200) };
                let ring: Vec<Pos2> = g.hull.iter().map(|p| screen([p[0], p[1], level_z])).collect();
                if ring.len() >= 3 {
                    painter.add(egui::Shape::convex_polygon(ring.clone(), Color32::from_rgba_unmultiplied(ink.r(), ink.g(), ink.b(), 70), Stroke::new(1.5, ink)));
                }
                for (c, joined) in &g.connectors {
                    let sp = screen([c[0], c[1], level_z]);
                    if *joined { painter.circle_stroke(sp, 8.0, Stroke::new(2.0, Color32::from_rgb(60, 170, 90))); } else { painter.circle_filled(sp, 3.0, ink); }
                }
                if let (Some(text), Some(first)) = (&g.label, ring.first()) {
                    painter.text(*first + egui::vec2(10.0, -10.0), egui::Align2::LEFT_BOTTOM, text, egui::FontId::proportional(12.0), ink);
                }
            }
        }
    } else if app.tool == crate::Tool::WallOpening && !three_d {
        // the two ends of the opening are picked on a wall; the distance along the wall is that of the click
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [(v.x as f64 / 0.05).round() * 0.05, (-v.y as f64 / 0.05).round() * 0.05] };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { if app.opening_from.is_some() { app.opening_from = None; } else { app.choose_tool(crate::Tool::Select); } }
        else if pick.is_some() {
            if let Some(p) = pick {
                let wall = faces.iter().rev().find(|(_, id, polygon, _)| contains(polygon, p) && app.document.get(*id).is_some_and(|e| e.category == Category::Wall)).map(|(_, id, _, _)| *id);
                let at = to_model(p);
                let along = wall.and_then(|w| match app.document.get(w).and_then(|e| e.geometry) {
                    Some(Geometry::Linear { start, end, .. }) => {
                        let len = (end[0] - start[0]).hypot(end[1] - start[1]);
                        Some((((at[0] - start[0]) * (end[0] - start[0]) + (at[1] - start[1]) * (end[1] - start[1])) / len).clamp(0.0, len))
                    }
                    _ => None,
                });
                app.wall_opening_click(wall, along.unwrap_or(0.0));
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
    } else if matches!(app.tool, crate::Tool::Slab | crate::Tool::BeamSystem | crate::Tool::Shaft | crate::Tool::Railing | crate::Tool::VerticalOpening) && !three_d {
        // corners snapped to 50 mm; Enter or a click on the first corner closes the outline
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let snap = |v: f32| (v as f64 / 0.05).round() * 0.05;
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [snap(v.x), snap(-v.y)] };
        let pts: Vec<Pos2> = app.slab_points.iter().map(|p| screen([p[0], p[1], level_z])).collect();
        let stroke = Stroke::new(2.0, Color32::from_rgb(70, 130, 200));
        if app.tool == crate::Tool::Slab && app.slab_kind_pick == openbim_document::SlabKind::Ceiling && app.ceiling_auto {
            // Automatic Ceiling: the space under the pointer is outlined
            if let Some(h) = response.hover_pos() {
                if let Ok(shape) = app.document.room_region_at(level_id, to_model(h)) {
                    let ring: Vec<Pos2> = shape.outline.iter().map(|p| screen([p[0], p[1], level_z])).collect();
                    painter.add(egui::Shape::closed_line(ring, stroke));
                }
            }
        }
        if let (Some(last), Some(h)) = (pts.last(), response.hover_pos()) {
            painter.line_segment([*last, h], Stroke::new(1.0, stroke.color.gamma_multiply(0.6)));
        }
        for w in pts.windows(2) { painter.line_segment([w[0], w[1]], stroke); }
        for (i, p) in pts.iter().enumerate() { painter.circle_filled(*p, if i == 0 { 5.0 } else { 3.5 }, stroke.color); }
        // Vertical Opening: the host is outlined while the opening is drawn; Tab takes the next one under the click
        if app.tool == crate::Tool::VerticalOpening {
            if let Some(h) = app.opening_host.and_then(|h| app.document.get(h)) {
                let ring: Vec<Pos2> = h.outline.iter().map(|p| screen([p[0], p[1], level_z])).collect();
                painter.add(egui::Shape::closed_line(ring, Stroke::new(1.5, Color32::from_rgb(70, 130, 200).gamma_multiply(0.7))));
            }
            if ui.input(|i| i.key_pressed(egui::Key::Tab)) && app.slab_points.is_empty() { app.vertical_host_next(); }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if app.slab_points.is_empty() {
                if app.tool == crate::Tool::VerticalOpening && app.opening_host.is_some() { app.opening_host = None; app.opening_cands.clear(); app.message = "Vertical Opening: click the floor, roof or ceiling to cut".into(); }
                else { app.choose_tool(crate::Tool::Select); }
            } else { app.slab_points.clear(); }
        } else if ui.input(|i| i.key_pressed(egui::Key::Backspace)) { app.slab_points.pop();
        } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) && app.slab_points.len() >= if app.tool == crate::Tool::Railing { 2 } else { 3 } { app.finish_slab(level_id);
        } else if pick.is_some() {
            if let Some(p) = pick {
                let closing = app.tool != crate::Tool::Railing && pts.first().is_some_and(|f| f.distance(p) < 10.0);
                if app.tool == crate::Tool::VerticalOpening && app.opening_host.is_none() { app.vertical_host_click(level_id, to_model(p)); }
                else { app.slab_click(level_id, to_model(p), closing); }
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
        // the truss keeps its start in beam_start, so Esc steps back before it leaves the tool (R2)
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if app.beam_start.is_some() { app.beam_start = None; app.message = "Truss: click the start".into(); }
            else { app.choose_tool(crate::Tool::Select); }
        } else if pick.is_some() { if let Some(p) = pick { app.truss_click(level_id, to_model(p)); } }
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
            if let Some(tip) = app.run_points.last().and_then(|f| app.run_slope_tip(*f, m)) {
                painter.text(h + Vec2::new(14.0, 14.0), egui::Align2::LEFT_TOP, tip, egui::FontId::proportional(12.0), colour.to_opaque());
            }
        }
        for p in &pts { painter.circle_filled(*p, 3.5, colour); }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            // Esc places the run once it has a length; a single point is taken back first, and only then does Esc
            // leave the tool (R2)
            if app.run_points.len() >= 2 { app.finish_run(level_id); }
            else if !app.run_points.is_empty() { app.run_back(); app.message = "Esc takes the point back; Esc again ends the run".into(); }
            else { app.run_sizes.clear(); app.choose_tool(crate::Tool::Select); }
        } else if ui.input(|i| i.key_pressed(egui::Key::Backspace)) { app.run_back();
        } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) && app.run_points.len() >= 2 { app.finish_run(level_id);
        } else if pick.is_some() {
            if let Some(p) = pick { app.run_click(to_model(p)); }
        }
    } else if app.tool == crate::Tool::Beam && !three_d {
        // two clicks place a beam, snapped to 50 mm; Esc takes the start back and then ends the tool (R2)
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let snap = |v: f32| (v as f64 / 0.05).round() * 0.05;
        let to_model = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [snap(v.x), snap(-v.y)] };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if app.beam_start.is_some() { app.beam_start = None; app.message = "Beam: click the start".into(); }
            else { app.choose_tool(crate::Tool::Select); }
        } else {
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
            // As in Revit, a floor, ceiling or roof is not picked by its inside in a plan while a room is: a click on
            // a room's label (its name and area) selects the room rather than the slab it lies on. Walls, families
            // and openings above it still win; Tab cycles to the slab.
            let on_label = room_labels.iter().find(|(_, at, _, _)| p.distance(*at) < ROOM_LABEL_PICK_PX).map(|(id, ..)| *id);
            let hit = match (hit, on_label) {
                (Some(h), Some(room)) if app.document.get(h).is_some_and(|e| e.category == Category::Slab) => Some(room),
                (hit, _) => hit,
            };
            // A plan is cut below the head of a door or window, so a click in its opening picks it (see
            // `filler_in_opening`), as in Revit, rather than the wall above it.
            let hit = hit.map(|h| filler_in_opening(app, h, p, &screen, level_z).unwrap_or(h));
            let mods = ui.input(|i| i.modifiers);
            match hit {
                Some(id) if mods.command || mods.ctrl => app.change_selection(id, true),
                Some(id) if mods.shift => app.change_selection(id, false),
                _ => if hit.is_some() || !(mods.command || mods.ctrl || mods.shift) { app.select_element(hit); },
            }
        }
    }
    if app.tool == crate::Tool::Select && !three_d {
        // As in Revit: a selected wall shows a grip at each end, and dragging a grip moves that end (the walls joined
        // there follow); pressing on an element and dragging moves it with what stands on it. A ghost shows the result.
        let (pan, centre, bc) = (app.pan, rect.center(), bounds.center().to_vec2());
        let raw = |p: Pos2| { let v = (p - centre - pan) / scale + bc; [v.x as f64, -v.y as f64] };
        let edges = plan_edges(&app.document, level_id);
        let xf = crate::section_ui::Xf { centre: centre + pan, scale, cu: f64::from(bc.x), cz: -f64::from(bc.y) };
        let ortho = ui.input(|i| i.modifiers.shift);
        let snap_to = |p: Pos2, from: Option<[f64; 2]>| -> [f64; 2] { plan_snap(&edges, &xf, p, from, ortho).at };
        let wall_ends = selected.and_then(|id| match app.document.get(id) {
            Some(e) if matches!(e.category, Category::Wall | Category::Section) && app.document.level_of(id) == Some(level_id) => match e.geometry { Some(Geometry::Linear { start, end, .. }) => Some((id, start, end)), _ => None },
            _ => None,
        });
        let grip_at = |m: [f64; 2]| screen([m[0], m[1], level_z]);
        if let (Some((_, a, b)), None) = (wall_ends, app.drag) {
            for g in [a, b] { let c = grip_at(g); painter.rect_filled(Rect::from_center_size(c, Vec2::splat(9.0)), 1.0, ui.visuals().panel_fill); painter.rect_stroke(Rect::from_center_size(c, Vec2::splat(9.0)), 1.0, Stroke::new(1.5, blue), egui::StrokeKind::Middle); }
        }
        // Temporary dimensions of the selected wall, as Revit shows them: its length and its distance to the nearest line
        // alongside on each side. Click a number and type another: the wall changes.
        app.temp_labels.clear();
        if let (Some((id, a, b)), None, 1, true) = (wall_ends, app.drag, selected_all.len(), selected.is_some_and(|s| app.document.get(s).is_some_and(|e| e.category == Category::Wall))) {
            let ink = Color32::from_rgb(70, 130, 180);
            let (pa, pb) = (grip_at(a), grip_at(b));
            let len = (b[0] - a[0]).hypot(b[1] - a[1]);
            let dir = if (pb - pa).length() > 1e-3 { (pb - pa).normalized() } else { Vec2::X };
            let off = Vec2::new(-dir.y, dir.x) * -26.0;
            type Shown = (Pos2, f64, TempDim, Vec<(Pos2, Pos2)>);
            let mut shown: Vec<Shown> = Vec::new();
            shown.push((pa + (pb - pa) * 0.5 + off, len, TempDim::Length(id), vec![(pa, pa + off * 1.15), (pb, pb + off * 1.15), (pa + off, pb + off)]));
            let (ux, uy) = ((b[0] - a[0]) / len.max(1e-9), (b[1] - a[1]) / len.max(1e-9));
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            for (side, n) in app.document.wall_neighbours(id).into_iter().enumerate() {
                let Some((other, d)) = n else { continue };
                let sign = if side == 0 { 1.0 } else { -1.0 };
                let tip = [mid[0] - uy * sign * d, mid[1] + ux * sign * d];
                let (s0, s1) = (grip_at(mid), grip_at(tip));
                shown.push((s0 + (s1 - s0) * 0.5, d, TempDim::Distance(id, other), vec![(s0, s1)]));
            }
            for (at, value, dim, lines) in shown {
                for (p, q) in lines { painter.line_segment([p, q], Stroke::new(1.0, ink)); }
                let editing = app.temp_edit == Some(dim);
                let text = format!("{:.0}", value * 1000.0);
                let galley = painter.layout_no_wrap(text.clone(), egui::FontId::proportional(12.0), ink);
                let r = Rect::from_center_size(at, galley.size() + Vec2::new(10.0, 6.0));
                if editing {
                    let edit = ui.put(r.expand2(Vec2::new(14.0, 2.0)), egui::TextEdit::singleline(&mut app.temp_text).desired_width(r.width() + 24.0));
                    edit.request_focus();
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) { app.apply_temp_dim(); }
                    else if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.temp_edit = None; }
                } else {
                    painter.rect_filled(r, 2.0, ui.visuals().panel_fill);
                    painter.rect_stroke(r, 2.0, Stroke::new(1.0, ink), egui::StrokeKind::Middle);
                    painter.galley(r.center() - galley.size() / 2.0, galley, ink);
                    let hit = ui.interact(r, egui::Id::new(("tempdim", dim.key())), Sense::click());
                    if hit.clicked() { app.temp_edit = Some(dim); app.temp_text = text; }
                    if hit.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::Text); }
                    app.temp_labels.push((r, dim, value));
                }
            }
        } else if app.temp_edit.is_some() { app.temp_edit = None; }
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                if let Some((id, a, b)) = wall_ends.filter(|(_, a, b)| grip_at(*a).distance(p) < 10.0 || grip_at(*b).distance(p) < 10.0) {
                    app.drag = Some(crate::Drag::Grip { id, at_start: grip_at(a).distance(p) <= grip_at(b).distance(p) });
                } else if let Some(hit) = markers.iter().find(|(_, c)| c.distance(p) < 13.0).map(|(id, _)| *id) {
                    if !selected_all.contains(&hit) { app.select_element(Some(hit)); }
                    app.drag = Some(crate::Drag::Move { id: hit, from: snap_to(p, None) });
                } else if let Some(hit) = faces.iter().rev().find(|(_, _, polygon, _)| contains(polygon, p)).map(|(_, id, _, _)| *id) {
                    if !selected_all.contains(&hit) { app.select_element(Some(hit)); }
                    app.drag = Some(crate::Drag::Move { id: hit, from: snap_to(p, None) });
                } else {
                    app.marquee = Some(p);
                }
            }
        }
        if let Some(start) = app.marquee {
            // a box dragged on empty ground selects what it holds (left to right) or touches (right to left, dashed)
            let here = response.interact_pointer_pos().or(response.hover_pos()).unwrap_or(start);
            let r = Rect::from_two_pos(start, here);
            let crossing = here.x < start.x;
            let colour = if crossing { Color32::from_rgb(60, 160, 80) } else { blue };
            if crossing { for (a, b) in [(r.left_top(), r.right_top()), (r.right_top(), r.right_bottom()), (r.right_bottom(), r.left_bottom()), (r.left_bottom(), r.left_top())] { dashed(&painter, a, b, Stroke::new(1.0, colour)); } }
            else { painter.rect_stroke(r, 0.0, Stroke::new(1.0, colour), egui::StrokeKind::Middle); }
            painter.rect_filled(r, 0.0, colour.gamma_multiply(0.08));
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { app.marquee = None; }
            else if response.drag_stopped_by(egui::PointerButton::Primary) {
                app.marquee = None;
                let mut boxes: Vec<(ElementId, Rect)> = Vec::new();
                for (_, id, points, _) in faces.iter() {
                    let b = Rect::from_points(points);
                    match boxes.iter_mut().find(|x| x.0 == *id) { Some(x) => x.1 = x.1.union(b), None => boxes.push((*id, b)) }
                }
                for (id, a, b) in &datum_hits { if app.document.get(*id).is_some_and(|e| e.category == Category::Grid) { boxes.push((*id, Rect::from_two_pos(*a, *b))); } }
                let hits: Vec<ElementId> = boxes.iter().filter(|(_, b)| if crossing { r.intersects(*b) } else { r.contains_rect(*b) }).map(|(id, _)| *id).collect();
                let mods = ui.input(|i| i.modifiers);
                let mut all = if mods.command || mods.shift { selected_all.clone() } else { Vec::new() };
                for h in hits { if !all.contains(&h) { all.push(h); } }
                app.select_many(all);
                if app.selected_ids().is_empty() { app.message = "Nothing in the box".into(); }
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
                        let group = if selected_all.contains(&id) { selected_all.clone() } else { vec![id] };
                        for (_, _, points, _) in faces.iter().filter(|f| group.contains(&f.1)) {
                            let moved: Vec<Pos2> = points.iter().map(|q| { let m = xf.point(raw(*q)); screen([m[0], m[1], level_z]) }).collect();
                            painter.add(egui::Shape::convex_polygon(moved, fill, ghost));
                        }
                        for (_, c) in markers.iter().filter(|m| group.contains(&m.0)) {
                            let m = xf.point(raw(*c));
                            painter.circle(screen([m[0], m[1], level_z]), 11.0, fill, ghost);
                        }
                        painter.text(h + Vec2::new(12.0, -12.0), egui::Align2::LEFT_BOTTOM, format!("{:.0} mm", (to[0] - from[0]).hypot(to[1] - from[1]) * 1000.0), egui::FontId::proportional(13.0), blue);
                        if (to[0] - from[0]).hypot(to[1] - from[1]) > 1e-6 { done = Some(app.document.transform_commands(&group, xf)); }
                    }
                    crate::Drag::Grip { id, at_start } => if let Some((_, a, b)) = wall_ends {
                        let (fixed, moving) = if at_start { (b, a) } else { (a, b) };
                        let to = snap_to(h, Some(fixed));
                        let (dx, dy) = (to[0] - fixed[0], to[1] - fixed[1]);
                        let l = dx.hypot(dy);
                        if l > 1e-6 {
                            let is_section = app.document.get(id).is_some_and(|e| e.category == Category::Section);
                            let w = if is_section { 0.0 } else { app.document.get(id).map_or(0.2, |e| e.width_m) / 2.0 };
                            let (nx, ny) = (-dy / l * w, dx / l * w);
                            let ring = [[fixed[0] - nx, fixed[1] - ny], [to[0] - nx, to[1] - ny], [to[0] + nx, to[1] + ny], [fixed[0] + nx, fixed[1] + ny]].map(|p| screen([p[0], p[1], level_z]));
                            if is_section { dashed(&painter, grip_at(fixed), grip_at(to), ghost); } else { painter.add(egui::Shape::convex_polygon(ring.to_vec(), fill, ghost)); }
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

/// An aligned dimension: extension lines from the measured points `p1` and `p2` to the dimension line `q1`-`q2`, ticks at its
/// ends and the text on it. Returns the middle of the text.
pub(crate) fn draw_dimension(painter: &egui::Painter, p1: Pos2, p2: Pos2, q1: Pos2, q2: Pos2, text: &str, ink: Color32) -> Pos2 {
    let stroke = Stroke::new(1.0, ink);
    for (p, q) in [(p1, q1), (p2, q2)] {
        let d = (q - p).normalized();
        if (q - p).length() > 1.0 { painter.line_segment([p + d * 3.0, q + d * 4.0], stroke); }
    }
    painter.line_segment([q1, q2], stroke);
    let along = if (q2 - q1).length() > 1e-3 { (q2 - q1).normalized() } else { Vec2::X };
    for q in [q1, q2] { painter.line_segment([q - (along + Vec2::new(-along.y, along.x)) * 4.0, q + (along + Vec2::new(-along.y, along.x)) * 4.0], Stroke::new(1.5, ink)); }
    let mid = q1 + (q2 - q1) * 0.5;
    let normal = Vec2::new(-along.y, along.x);
    let normal = if normal.y > 0.0 { -normal } else { normal };
    let galley = painter.layout_no_wrap(text.to_string(), egui::FontId::proportional(12.0), ink);
    let centre = mid + normal * (galley.size().y * 0.5 + 3.0);
    painter.galley(centre - galley.size() / 2.0, galley, ink);
    centre
}

/// The level bubbles of a plan, top first: the name, the elevation and whether it is the level of the view.
fn level_bubbles(levels: &[(ElementId, String, f64)], current: ElementId) -> Vec<(String, f64, bool)> {
    let mut v: Vec<_> = levels.iter().map(|(id, n, z)| (n.clone(), *z, *id == current)).collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v
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
    #[test]
    fn a_plan_shows_a_bubble_for_every_level_highest_first_and_marks_its_own() {
        let levels = vec![(ElementId(1), "Level 1".to_string(), 0.0), (ElementId(2), "Level 2".to_string(), 3.2), (ElementId(3), "Basement".to_string(), -3.0)];
        let marks = super::level_bubbles(&levels, ElementId(1));
        assert_eq!(marks.iter().map(|m| m.0.as_str()).collect::<Vec<_>>(), ["Level 2", "Level 1", "Basement"]);
        assert_eq!(marks.iter().map(|m| m.2).collect::<Vec<_>>(), [false, true, false]);
    }

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

    #[test]
    fn the_plan_snaps_to_walls_like_revit_not_only_to_their_corners() {
        use crate::view_tools::SnapKind;
        let mut app = Desktop::sample();
        let level = match app.document.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        // a wall of its own, away from everything else in the sample, so that nothing crosses it
        app.run_command("wall", "Wall");
        let n = walls(&app);
        app.wall_click(level, [30.0, 30.0]);
        app.wall_click(level, [30.0, 36.0]);
        assert_eq!(walls(&app), n + 1, "{}", app.message);
        let (start, end): ([f64; 2], [f64; 2]) = ([30.0, 30.0], [30.0, 36.0]);
        let edges = plan_edges(&app.document, level);
        assert!(edges.len() >= 5, "the location line and the four edges of the wall");
        let xf = crate::section_ui::Xf { centre: Pos2::new(500.0, 350.0), scale: 100.0, cu: 30.0, cz: 33.0 };
        let (lo, hi) = (start[1].min(end[1]), start[1].max(end[1]));
        let mid = [30.0, (lo + hi) / 2.0];
        // the middle of the location line, which the old corner snap never offered
        let s = plan_snap(&edges, &xf, xf.screen(mid), None, false);
        assert!(s.kind == SnapKind::Midpoint && (s.at[0] - 30.0).abs() < 1e-6 && (s.at[1] - mid[1]).abs() < 1e-6, "{s:?}");
        // an end of the wall is still an endpoint
        let s = plan_snap(&edges, &xf, xf.screen([30.0, lo]) + Vec2::new(2.0, 2.0), None, false);
        assert!(s.kind == SnapKind::Endpoint, "{s:?}");
        // from a point away from the wall, the foot of the perpendicular on the wall
        let y = lo + (hi - lo) * 0.25;
        let s = plan_snap(&edges, &xf, xf.screen([30.0, y]), Some([27.0, y]), false);
        assert!(s.kind == SnapKind::Perpendicular && (s.at[0] - 30.0).abs() < 1e-6 && (s.at[1] - y).abs() < 1e-6, "{s:?}");
        // without a base point the same spot is the nearest point of the line
        let s = plan_snap(&edges, &xf, xf.screen([30.0, y]), None, false);
        assert!(s.kind == SnapKind::Nearest, "{s:?}");
        // far from every line: 50 mm, and Shift keeps it level with the base point
        let far = [34.123, 33.777];
        let s = plan_snap(&edges, &xf, xf.screen(far), Some([33.0, 33.0]), false);
        assert!(s.kind == SnapKind::Grid && (s.at[0] - 34.1).abs() < 1e-9 && (s.at[1] - 33.8).abs() < 1e-9, "{s:?}");
        let s = plan_snap(&edges, &xf, xf.screen(far), Some([33.0, 33.0]), true);
        assert!(s.kind == SnapKind::Grid && (s.at[1] - 33.0).abs() < 1e-9, "{s:?}");
    }

    fn frames() -> (egui::Context, Rect) { (egui::Context::default(), Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))) }
    fn frame(ctx: &egui::Context, screen: Rect, app: &mut Desktop, events: Vec<egui::Event>) {
        let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() },
            |ui| { egui::CentralPanel::default().show(ui, |ui| show(app, ui)); });
        out.textures_delta.clear();
    }
    fn button(at: Pos2, pressed: bool) -> egui::Event { egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() } }
    fn walls(app: &Desktop) -> usize { app.document.elements().filter(|e| e.category == Category::Wall).count() }

    /// The whole window, as the app runs it: the toolbar, the ribbon, the options bar, the panels and the viewport,
    /// without an `eframe::Frame`, so a test can click what the ribbon draws (A8).
    fn window_frame(ctx: &egui::Context, screen: Rect, app: &mut Desktop, events: Vec<egui::Event>) {
        let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ui| app.window(ui));
        out.textures_delta.clear();
    }

    /// Where a model point falls on the screen of the plan: the transform the viewport itself used in its last frame
    /// (A8), so a click can be aimed at a point of the model.
    fn plan_screen(app: &Desktop, at: [f64; 2]) -> Pos2 {
        let (centre, scale, box_centre) = app.plan_transform.expect("the plan has drawn a frame");
        centre + (Vec2::new(at[0] as f32, -(at[1] as f32)) - box_centre) * scale
    }

    /// Clicks the middle of a ribbon button, where the last frame put it. False when the ribbon shows no such button.
    fn ribbon_click(ctx: &egui::Context, screen: Rect, app: &mut Desktop, id: &str) -> bool {
        let Some((_, rect)) = app.ribbon_hits.iter().find(|(hit, _)| *hit == id).copied() else { return false };
        let at = rect.center();
        window_frame(ctx, screen, app, vec![egui::Event::PointerMoved(at)]);
        window_frame(ctx, screen, app, vec![button(at, true)]);
        window_frame(ctx, screen, app, vec![button(at, false)]);
        true
    }

    /// A frame in which the window handles the global keys first and then draws the viewport, as `ui()` does (A3, A4).
    fn key_frame(ctx: &egui::Context, screen: Rect, app: &mut Desktop, events: Vec<egui::Event>) {
        let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() },
            |ui| { app.global_keys(ui.ctx()); egui::CentralPanel::default().show(ui, |ui| show(app, ui)); });
        out.textures_delta.clear();
    }
    fn key_with(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    #[test]
    fn escape_steps_back_inside_the_wall_tool_before_leaving_it() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.run_command("wall", "Wall");
        let at = Pos2::new(400.0, 300.0);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, screen, &mut app, vec![button(at, true)]);
        frame(&ctx, screen, &mut app, vec![button(at, false)]);
        assert_eq!(app.wall_pts.len(), 1, "{}", app.message);
        // the first Esc takes the pending point back and keeps the tool
        key_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Wall, "Esc inside a chain keeps the tool: {}", app.message);
        assert!(app.wall_pts.is_empty(), "the point is taken back");
        // with nothing pending the second Esc leaves it
        key_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select, "{}", app.message);
    }

    #[test]
    fn the_keymap_shortcuts_choose_a_tool_and_ctrl_n_starts_a_new_model() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        // "W A" is the keymap's Wall shortcut, typed one key at a time: half of it starts nothing
        key_frame(&ctx, screen, &mut app, vec![key(egui::Key::W)]);
        assert!(app.tool == crate::Tool::Select, "a half-typed shortcut starts nothing: {}", app.message);
        key_frame(&ctx, screen, &mut app, vec![key(egui::Key::A)]);
        assert!(app.tool == crate::Tool::Wall, "{}", app.message);
        assert_eq!((app.width_mm, app.height_mm), (200.0, 3000.0), "the Wall command's own numbers");
        // Ctrl+N starts a new model, which brings its two levels with it
        key_frame(&ctx, screen, &mut app, vec![key_with(egui::Key::N, egui::Modifiers::CTRL)]);
        assert_eq!(app.levels().len(), 2, "{}", app.message);
        assert!(app.tool == crate::Tool::Select, "{}", app.message);
    }

    #[test]
    fn a_level_view_is_named_after_its_level_and_follows_a_rename() {
        let mut app = Desktop::sample();
        let level = app.levels()[0].0;
        app.rename_level(level, "Ground floor");
        assert_eq!(crate::panels::level_view_name(&app, "Level 1", View::Plan), "Ground floor", "{}", app.message);
        assert_eq!(crate::panels::level_view_name(&app, "Level 2", View::UpperPlan), "Level 2", "the second level keeps its name");
        // a view that is not a level keeps the name the browser table gives it
        let other = Desktop::sample();
        assert_eq!(crate::panels::level_view_name(&other, "{3D}", View::ThreeD), "{3D}");
    }

    #[test]
    fn the_ribbon_starts_a_tool_and_a_button_that_is_not_wired_changes_nothing() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.ribbon = "Architecture";
        window_frame(&ctx, screen, &mut app, vec![]);
        assert!(ribbon_click(&ctx, screen, &mut app, "wall"), "the Architecture tab shows a Wall button");
        assert!(app.tool == crate::Tool::Wall, "{}", app.message);
        // a button the decision of 21 Oct 2026 leaves disabled does nothing at all
        app.ribbon = "Systems";
        app.systems_sub = "HVAC";
        window_frame(&ctx, screen, &mut app, vec![]);
        assert!(app.ribbon_hits.iter().any(|(id, _)| *id == "damper"), "the HVAC tab shows a Damper button");
        assert!(!crate::Desktop::ribbon_command_implemented("damper"), "the Damper command is not wired yet");
        ribbon_click(&ctx, screen, &mut app, "damper");
        assert!(app.tool == crate::Tool::Wall, "a disabled button changes nothing: {}", app.message);
    }

    /// Refit before every click; never bypass the window's picking, snapping or global-key handlers.
    fn office_plan_click(ctx: &egui::Context, screen: Rect, app: &mut Desktop, point: [f64; 2]) {
        window_frame(ctx, screen, app, vec![]);
        let mut at = plan_screen(app, point);
        // The viewport refits to the window every frame, and a panel whose height changes (the status bar
        // wrapping a new message) moves the transform one frame later, so the refit's transform can be stale
        // by the time the click is taken. Re-aim the click at the transform the taking frame will use.
        for _ in 0..3 {
            window_frame(ctx, screen, app, vec![egui::Event::PointerMoved(at)]);
            let next = plan_screen(app, point);
            if (next - at).length() < 0.05 { break; }
            at = next;
        }
        assert!(screen.contains(at), "office point {point:?} is outside the window: {at:?}");
        window_frame(ctx, screen, app, vec![button(at, true)]);
        window_frame(ctx, screen, app, vec![button(at, false)]);
    }

    #[test]
    fn the_small_office_is_built_from_a_new_project_then_saved_reopened_and_exported() {
        // wide enough for the whole Architecture and View tabs: the ribbon scrolls sideways, and a button off screen
        // cannot be clicked
        let (ctx, screen) = (egui::Context::default(), Rect::from_min_size(Pos2::ZERO, Vec2::new(2200.0, 800.0)));
        let mut app = Desktop::sample();
        window_frame(&ctx, screen, &mut app, vec![]);
        assert!(ribbon_click(&ctx, screen, &mut app, "new"));
        assert_eq!(app.levels().len(), 2);
        app.view = View::Plan;
        app.ribbon = "Architecture";
        // Parameters and active tabs are fixture setup; geometry must pass through window pointer events.
        // a grid, by two clicks on the empty canvas, where the transform is still the default box
        window_frame(&ctx, screen, &mut app, vec![]);
        assert!(ribbon_click(&ctx, screen, &mut app, "grid"), "the ribbon shows the Grid button");
        assert!(app.tool == crate::Tool::Grid, "the Grid button starts the tool: {}", app.message);
        window_frame(&ctx, screen, &mut app, vec![]);
        for p in [[-4.0, -4.0], [4.0, 4.0]] { office_plan_click(&ctx, screen, &mut app, p); }
        assert_eq!(app.document.elements().filter(|e| e.category == Category::Grid).count(), 1, "{}", app.message);
        // Four walls through pointer events. Recompute the screen position after every frame because the plan refits.
        window_frame(&ctx, screen, &mut app, vec![]);
        assert!(ribbon_click(&ctx, screen, &mut app, "wall"), "the ribbon shows the Wall button");
        assert!(app.tool == crate::Tool::Wall, "{}", app.message);
        app.wall_loc = 2;
        for p in [[-3.0, -2.0], [3.0, -2.0], [3.0, 2.0], [-3.0, 2.0], [-3.0, -2.0]] { office_plan_click(&ctx, screen, &mut app, p); }
        assert_eq!(walls(&app), 4, "{}", app.message);
        // a floor over them, and the room they enclose
        window_frame(&ctx, screen, &mut app, vec![]);
        assert!(ribbon_click(&ctx, screen, &mut app, "floor"), "the ribbon shows the Floor button");
        for p in [[-3.0, -2.0], [3.0, -2.0], [3.0, 2.0], [-3.0, 2.0]] { office_plan_click(&ctx, screen, &mut app, p); }
        window_frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        assert_eq!(app.document.elements().filter(|e| e.category == Category::Slab).count(), 1, "{}", app.message);
        // Hosted families use the ribbon and the actual wall axes to aim pointer events.
        let host_axes: Vec<(ElementId, [f64; 2])> = app.document.elements().filter_map(|e| {
            if e.category != Category::Wall { return None; }
            match e.geometry {
                Some(Geometry::Linear { start, end, .. }) => Some((e.id, [(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0])),
                _ => None,
            }
        }).collect();
        for (command, (host, at)) in ["door", "window"].into_iter().zip(host_axes.iter().take(2)) {
            window_frame(&ctx, screen, &mut app, vec![]);
            assert!(ribbon_click(&ctx, screen, &mut app, command));
            let before = app.document.elements().filter(|e| matches!(e.geometry, Some(Geometry::InWall { .. }))).count();
            office_plan_click(&ctx, screen, &mut app, *at);
            let hosted: Vec<_> = app.document.elements().filter(|e| matches!(e.geometry, Some(Geometry::InWall { .. }))).collect();
            assert_eq!(hosted.len(), before + 1, "{command} places one hosted instance: {}", app.message);
            assert!(hosted.iter().any(|e| matches!(e.geometry, Some(Geometry::InWall { host: picked, .. }) if picked == *host)), "{command} keeps its picked host");
        }
        window_frame(&ctx, screen, &mut app, vec![]);
        assert!(ribbon_click(&ctx, screen, &mut app, "room"), "the ribbon shows the Room button");
        office_plan_click(&ctx, screen, &mut app, [0.0, 0.0]);
        let room = app.document.elements().find(|e| e.category == Category::Room).expect("the room the walls enclose").id;
        let area = app.document.room_shape(room).expect("the walls enclose the room").area();
        // the walls were drawn on their exterior face round a 6 x 4 m rectangle, so the room between their inner
        // faces is that rectangle
        assert!((area - 24.0).abs() < 0.05, "the room is the 6 x 4 m the walls enclose: {area}");
        // a section through it, from the View tab
        app.ribbon = "View";
        window_frame(&ctx, screen, &mut app, vec![]);
        assert!(ribbon_click(&ctx, screen, &mut app, "section"), "the View tab shows the Section button");
        assert!(app.tool == crate::Tool::Section, "the Section button starts the tool: {}", app.message);
        office_plan_click(&ctx, screen, &mut app, [0.0, -3.0]);
        office_plan_click(&ctx, screen, &mut app, [0.0, 3.0]);
        assert_eq!(app.document.elements().filter(|e| e.category == Category::Section).count(), 1, "{}", app.message);
        // Exercise the real quick-access actions, then require exact restoration of the model records.
        let before_undo = openbim_document::native::records(&app.document);
        assert!(ribbon_click(&ctx, screen, &mut app, "undo"));
        assert_eq!(app.document.elements().filter(|e| e.category == Category::Section).count(), 0, "{}", app.message);
        assert!(ribbon_click(&ctx, screen, &mut app, "redo"));
        assert_eq!(openbim_document::native::records(&app.document), before_undo, "redo restores every native record");
        // save, reopen and export: every element comes back with its identity and its geometry, and the IFC carries
        // the four walls
        let path = std::env::temp_dir().join(format!("openbim-small-office-e2e-{}.obim", std::process::id()));
        let path = path.to_str().unwrap().to_string();
        app.write_model(&path);
        let saved = app.document.clone();
        let mut reopened = Desktop::sample();
        reopened.open_path(&path);
        assert_eq!(reopened.document.elements().count(), saved.elements().count(), "{}", reopened.message);
        for el in saved.elements() {
            let back = reopened.document.get(el.id).unwrap_or_else(|| panic!("element {} came back", el.id.0));
            assert_eq!(back.geometry, el.geometry, "element {} keeps its geometry", el.id.0);
            assert_eq!(back.name, el.name, "element {} keeps its name", el.id.0);
        }
        assert_eq!(openbim_document::native::records(&reopened.document), openbim_document::native::records(&saved), "save/reopen preserves all records, including view and room metadata");
        let back_room = reopened.document.room_shape(room).expect("the reopened room is enclosed");
        assert!((back_room.area() - area).abs() < 1e-9, "the room keeps its area: {} against {area}", back_room.area());
        let out = crate::export::export_ifc_with(&reopened.document, "Office", reopened.document.project_key(), "2026-10-21T00:00:00", crate::export::Schema::Ifc4, false)
            .expect("the model exports");
        assert_eq!(out.walls, 4, "{}", out.warnings.join("; "));
        assert!(out.text.contains("IFCWALL("), "the file carries walls");
        assert_eq!(out.text.matches("=IFCDOOR(").count(), 1);
        assert_eq!(out.text.matches("=IFCWINDOW(").count(), 1);
        assert_eq!(out.text.matches("=IFCRELFILLSELEMENT(").count(), 2, "both families fill their wall openings");
        std::fs::remove_file(&path).ok();
    }

    /// Where the last frame drew the control named `name` (see `test_hit!`), after drawing a fresh frame. A control
    /// scrolled out of sight is scrolled to with the mouse wheel over its own scroll area, as a user would; only
    /// the part a pointer reaches is returned.
    fn widget(ctx: &egui::Context, screen: Rect, app: &mut Desktop, name: &str) -> Rect {
        for _ in 0..40 {
            window_frame(ctx, screen, app, vec![]);
            let Some((_, reach, full)) = app.widget_hits.iter().rev().find(|(hit, ..)| hit == name).cloned() else { break };
            if reach.height() >= full.height().min(8.0) && reach.width() > 1.0 { return reach; }
            // the scroll area clips the control: wheel over a visible control of the same group (the same name
            // before the colon, so the same scroll area) in the direction that reveals it
            let group = name.split(':').next().unwrap_or(name);
            let over = app.widget_hits.iter().rev().find(|(n, r, _)| n.split(':').next() == Some(group) && r.height() > 4.0).map(|(_, r, _)| r.center());
            let Some(over) = over else { break };
            let up = full.center().y < over.y;
            window_frame(ctx, screen, app, vec![egui::Event::PointerMoved(over), egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, if up { 40.0 } else { -40.0 }), phase: egui::TouchPhase::Move, modifiers: Default::default(),
            }]);
        }
        panic!("the window shows no reachable control {name:?}; it shows {:?} ({})",
            app.widget_hits.iter().map(|(n, r, _)| (n.as_str(), *r)).collect::<Vec<_>>(), app.message)
    }

    /// One click of the primary button at the middle of `rect`.
    fn click_at(ctx: &egui::Context, screen: Rect, app: &mut Desktop, rect: Rect) {
        let at = rect.center();
        assert!(screen.contains(at), "the control at {rect:?} is outside the window");
        window_frame(ctx, screen, app, vec![egui::Event::PointerMoved(at)]);
        window_frame(ctx, screen, app, vec![button(at, true)]);
        window_frame(ctx, screen, app, vec![button(at, false)]);
    }

    fn click_widget(ctx: &egui::Context, screen: Rect, app: &mut Desktop, name: &str) {
        let rect = widget(ctx, screen, app, name);
        click_at(ctx, screen, app, rect);
    }

    /// Two clicks in quick succession, as a user opens a view from the Project Browser.
    fn double_click_widget(ctx: &egui::Context, screen: Rect, app: &mut Desktop, name: &str) {
        let at = widget(ctx, screen, app, name).center();
        window_frame(ctx, screen, app, vec![egui::Event::PointerMoved(at)]);
        // egui tells a double click by the time between the clicks, so these frames carry their own clock; it starts
        // a second after the last frame so the clicks before it do not make this a triple click
        let start = ctx.input(|i| i.time) + 1.0;
        for (k, pressed) in [true, false, true, false].into_iter().enumerate() {
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(start + 0.05 * (k + 1) as f64), events: vec![button(at, pressed)], ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| app.window(ui));
            out.textures_delta.clear();
        }
    }

    /// Click a text field, select what it holds and type `text` over it, as a user does.
    fn type_into(ctx: &egui::Context, screen: Rect, app: &mut Desktop, name: &str, text: &str) {
        click_widget(ctx, screen, app, name);
        window_frame(ctx, screen, app, vec![key_with(egui::Key::A, egui::Modifiers::COMMAND)]);
        window_frame(ctx, screen, app, vec![egui::Event::Text(text.into())]);
    }

    fn ribbon_button(ctx: &egui::Context, screen: Rect, app: &mut Desktop, tab: &str, id: &str) {
        click_widget(ctx, screen, app, &format!("tab:{tab}"));
        assert_eq!(app.ribbon, tab, "the {tab} tab opens");
        window_frame(ctx, screen, app, vec![]);
        assert!(ribbon_click(ctx, screen, app, id), "the {tab} tab shows the {id} button");
    }

    /// The axis of each wall on `level`, in the order they were drawn.
    fn wall_axes(app: &Desktop, level: ElementId) -> Vec<(ElementId, [f64; 2], [f64; 2])> {
        app.document.elements().filter(|e| e.category == Category::Wall).filter_map(|e| match e.geometry {
            Some(Geometry::Linear { start, end, level: l }) if l == level => Some((e.id, start, end)),
            _ => None,
        }).collect()
    }

    /// P0-4, first slice: a two-storey office through the window's own controls. Every step below is a click or
    /// typing on a control the window drew, located from that frame: ribbon tabs and buttons, the options bar, the
    /// Project Browser, the Properties palette, the Export IFC dialog and the plan itself. The one substitution is
    /// the platform file dialog behind Save and Open, answered through `dialog::answer` (disclosed there).
    /// Annotation, schedules, sheets, PDF, host edits and recovery are later slices of the journey.
    #[test]
    fn the_two_storey_office_is_drawn_saved_reopened_and_exported_through_the_window() {
        // a wide, tall window: the ribbon scrolls sideways and the Properties palette scrolls down, and a control
        // scrolled out of sight cannot be clicked
        let (ctx, screen) = (egui::Context::default(), Rect::from_min_size(Pos2::ZERO, Vec2::new(2200.0, 1400.0)));
        let mut app = Desktop::sample();
        window_frame(&ctx, screen, &mut app, vec![]);
        assert!(ribbon_click(&ctx, screen, &mut app, "new"));
        assert_eq!(app.levels().len(), 2, "{}", app.message);

        // Datums: rename both levels from the Level tool's options bar, and add a roof level
        ribbon_button(&ctx, screen, &mut app, "Architecture", "level");
        assert!(app.tool == crate::Tool::Level, "{}", app.message);
        for (old, new) in [("Level 1", "Ground Floor"), ("Level 2", "First Floor")] {
            click_widget(&ctx, screen, &mut app, &format!("level:rename:{old}"));
            type_into(&ctx, screen, &mut app, "level:rename_field", new);
            click_widget(&ctx, screen, &mut app, "level:rename_ok");
            assert!(app.levels().iter().any(|l| l.1 == new), "{old} is renamed {new}: {}", app.message);
        }
        type_into(&ctx, screen, &mut app, "level:name", "Roof");
        click_widget(&ctx, screen, &mut app, "level:place");
        assert_eq!(app.levels().iter().map(|l| (l.1.as_str(), l.2)).collect::<Vec<_>>(), vec![("Ground Floor", 0.0), ("First Floor", 3.0), ("Roof", 6.0)], "{}", app.message);
        // storeys of 3.6 m: the roof goes up first, so the first floor never has to pass it
        for (level, mm) in [("Roof", "7200"), ("First Floor", "3600")] {
            click_widget(&ctx, screen, &mut app, &format!("level:elevation:{level}"));
            type_into(&ctx, screen, &mut app, "level:elevation_field", mm);
            click_widget(&ctx, screen, &mut app, "level:elevation_ok");
        }
        let levels = app.levels();
        assert_eq!(levels.iter().map(|l| (l.1.as_str(), l.2)).collect::<Vec<_>>(), vec![("Ground Floor", 0.0), ("First Floor", 3.6), ("Roof", 7.2)], "{}", app.message);
        let (ground, first) = (levels[0].0, levels[1].0);

        // the browser names the plans after their levels; opening one is a double click
        double_click_widget(&ctx, screen, &mut app, "view:Ground Floor");
        assert_eq!(app.view, View::Plan, "{}", app.message);

        // two grids frame the building; the plan fits its view to what the model holds, so they also give the
        // empty plan its extent before the walls exist
        ribbon_button(&ctx, screen, &mut app, "Architecture", "grid");
        for p in [[-4.0, -3.0], [4.0, -3.0]] { office_plan_click(&ctx, screen, &mut app, p); }
        // one grid fills the plan edge to edge, so zoom out from the View tab before drawing the next (the Grid
        // tool stays active, as in Revit)
        for _ in 0..4 { ribbon_button(&ctx, screen, &mut app, "View", "zoom_out"); }
        assert!(app.tool == crate::Tool::Grid, "{}", app.message);
        for p in [[-4.0, -3.0], [-4.0, 3.0]] { office_plan_click(&ctx, screen, &mut app, p); }
        assert_eq!(app.document.elements().filter(|e| e.category == Category::Grid).count(), 2, "{}", app.message);

        // Envelope on the ground floor: walls on their exterior face, chosen in the options bar
        ribbon_button(&ctx, screen, &mut app, "Architecture", "wall");
        click_widget(&ctx, screen, &mut app, "wall_loc");
        click_widget(&ctx, screen, &mut app, "wall_loc:Finish Face: Exterior");
        assert_eq!(Desktop::WALL_LOCATIONS[app.wall_loc], "Finish Face: Exterior");
        for p in [[-3.0, -2.0], [3.0, -2.0], [3.0, 2.0], [-3.0, 2.0], [-3.0, -2.0]] { office_plan_click(&ctx, screen, &mut app, p); }
        assert_eq!(wall_axes(&app, ground).len(), 4, "{}", app.message);
        let outer = wall_axes(&app, ground);
        // a door and a window a quarter of the way along two outer walls, clear of the partition to come
        for (command, (host, a, b)) in ["door", "window"].into_iter().zip(outer.iter().take(2)) {
            ribbon_button(&ctx, screen, &mut app, "Architecture", command);
            let at = [a[0] + (b[0] - a[0]) * 0.25, a[1] + (b[1] - a[1]) * 0.25];
            office_plan_click(&ctx, screen, &mut app, at);
            assert!(app.document.elements().any(|e| matches!(e.geometry, Some(Geometry::InWall { host: h, .. }) if h == *host)), "the {command} is in its wall: {}", app.message);
        }
        // a partition on its centreline divides the plan in two
        ribbon_button(&ctx, screen, &mut app, "Architecture", "wall");
        click_widget(&ctx, screen, &mut app, "wall_loc");
        click_widget(&ctx, screen, &mut app, "wall_loc:Wall Centerline");
        for p in [[0.0, -2.0], [0.0, 2.0]] { office_plan_click(&ctx, screen, &mut app, p); }
        window_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert_eq!(wall_axes(&app, ground).len(), 5, "{}", app.message);
        // the floor under them
        ribbon_button(&ctx, screen, &mut app, "Architecture", "floor");
        for p in [[-3.0, -2.0], [3.0, -2.0], [3.0, 2.0], [-3.0, 2.0]] { office_plan_click(&ctx, screen, &mut app, p); }
        window_frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        assert_eq!(app.document.elements().filter(|e| e.category == Category::Slab).count(), 1, "{}", app.message);

        // Rooms: one each side of the partition, numbered and named in the Properties palette
        ribbon_button(&ctx, screen, &mut app, "Architecture", "room");
        for p in [[-1.5, 0.0], [1.5, 0.0]] { office_plan_click(&ctx, screen, &mut app, p); }
        let rooms: Vec<ElementId> = app.document.elements().filter(|e| e.category == Category::Room).map(|e| e.id).collect();
        assert_eq!(rooms.len(), 2, "{}", app.message);
        for room in &rooms {
            let area = app.document.room_shape(*room).expect("each room is enclosed").area();
            // drawn counter-clockwise on the exterior face, the walls stand outside the 6 x 4 m line (as in the
            // small-office regression), so each side of the 200 mm partition is 2.9 x 4 m
            assert!((area - 2.9 * 4.0).abs() < 0.05, "room area {area}");
        }
        window_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        window_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select, "{}", app.message);
        for (at, number, name) in [([-1.5, 0.0], "101", "Office"), ([1.5, 0.0], "102", "Meeting")] {
            office_plan_click(&ctx, screen, &mut app, at);
            let picked = app.selected_id().expect("a click in the room selects it");
            assert!(rooms.contains(&picked), "the click selects a room, not {:?}: {}", app.document.get(picked).map(|e| e.category), app.message);
            type_into(&ctx, screen, &mut app, "room:Number", number);
            type_into(&ctx, screen, &mut app, "room:Name", name);
            click_widget(&ctx, screen, &mut app, "room:Apply");
            assert_eq!((app.document.room_text(picked, "Number"), app.document.get(picked).unwrap().name.clone()), (number.to_string(), name.to_string()), "{}", app.message);
        }

        // Change: a bigger door, picked from its types in Properties, then undone and redone
        let (host, a, b) = outer[0];
        let door_at = [a[0] + (b[0] - a[0]) * 0.25, a[1] + (b[1] - a[1]) * 0.25];
        // the plan is cut below the door head, so a click in the door's opening picks the door, not its wall
        office_plan_click(&ctx, screen, &mut app, door_at);
        let door = app.selected_id().expect("a click on the door selects it");
        assert!(app.document.get(door).is_some_and(|e| e.category == Category::Family && matches!(e.geometry, Some(Geometry::InWall { host: h, .. }) if h == host)), "the door is selected, not {:?}: {}", app.document.get(door).map(|e| (e.category, e.geometry)), app.message);
        click_widget(&ctx, screen, &mut app, "family_type");
        click_widget(&ctx, screen, &mut app, "family_type:Type B");
        click_widget(&ctx, screen, &mut app, "props:Apply");
        assert_eq!(app.document.get(door).unwrap().type_name, "Type B", "{}", app.message);
        assert!(ribbon_click(&ctx, screen, &mut app, "undo"));
        assert_eq!(app.document.get(door).unwrap().type_name, "", "undo restores the default type");
        assert!(ribbon_click(&ctx, screen, &mut app, "redo"));
        assert_eq!(app.document.get(door).unwrap().type_name, "Type B");
        // Change: move the wall that hosts the door 500 mm out; its joined walls, its door and both rooms follow
        let areas = |app: &Desktop| rooms.iter().map(|r| app.document.room_shape(*r).unwrap().area()).collect::<Vec<_>>();
        let (before_records, before_areas) = (openbim_document::native::records(&app.document), areas(&app));
        let grip = [a[0] + (b[0] - a[0]) * 0.75, a[1] + (b[1] - a[1]) * 0.75];
        office_plan_click(&ctx, screen, &mut app, grip);
        assert_eq!(app.selected_id(), Some(host), "a click on the wall selects it: {}", app.message);
        ribbon_button(&ctx, screen, &mut app, "Modify", "move");
        assert!(app.tool == crate::Tool::Move, "{}", app.message);
        for p in [grip, [grip[0], grip[1] - 0.5]] { office_plan_click(&ctx, screen, &mut app, p); }
        let moved = wall_axes(&app, ground).into_iter().find(|w| w.0 == host).unwrap();
        assert!((moved.1[1] - (a[1] - 0.5)).abs() < 1e-6 && (moved.2[1] - (b[1] - 0.5)).abs() < 1e-6, "the wall moved 500 mm: {moved:?} ({})", app.message);
        assert!(matches!(app.document.get(door).unwrap().geometry, Some(Geometry::InWall { host: h, .. }) if h == host), "the door stays in its wall");
        for (now, was) in areas(&app).iter().zip(&before_areas) {
            assert!((now - was - 2.9 * 0.5).abs() < 0.05, "each room grows by 2.9 x 0.5 m: {was} -> {now}");
        }
        window_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        let after_records = openbim_document::native::records(&app.document);
        assert!(ribbon_click(&ctx, screen, &mut app, "undo"));
        assert_eq!(openbim_document::native::records(&app.document), before_records, "undo restores every record");
        assert!(ribbon_click(&ctx, screen, &mut app, "redo"));
        assert_eq!(openbim_document::native::records(&app.document), after_records, "redo repeats the move exactly");
        let outer = wall_axes(&app, ground);

        // a U-shaped stair beside the building, from the ground floor up to the first; it takes its rise from the
        // levels, so the 3.6 m storey sets its risers
        ribbon_button(&ctx, screen, &mut app, "Architecture", "stair");
        click_widget(&ctx, screen, &mut app, "stair_shape");
        click_widget(&ctx, screen, &mut app, "stair_shape:U-shaped, turning right");
        for p in [[3.8, -1.8], [3.8, 0.0]] { office_plan_click(&ctx, screen, &mut app, p); }
        let stairs: Vec<_> = app.document.elements().filter(|e| e.category == Category::Stair).collect();
        assert!(!stairs.is_empty(), "{}", app.message);
        assert!(app.message.contains("21 risers") && app.message.contains("landing"), "3.6 m at 175 mm or less is 21 risers in two runs: {}", app.message);
        assert!(stairs.iter().all(|e| app.document.level_of(e.id) == Some(ground)), "the stair stands on the ground floor");
        // an automatic ceiling in the office, from a click in the space
        ribbon_button(&ctx, screen, &mut app, "Architecture", "ceiling");
        assert!(app.ceiling_auto, "Automatic Ceiling is the default, as in Revit");
        office_plan_click(&ctx, screen, &mut app, [-1.5, 0.0]);
        let ceilings: Vec<_> = app.document.elements().filter(|e| e.category == Category::Slab && e.slab_kind == openbim_document::SlabKind::Ceiling).collect();
        assert_eq!(ceilings.len(), 1, "{}", app.message);
        window_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);

        // the first floor, opened from the browser: its own four walls
        double_click_widget(&ctx, screen, &mut app, "view:First Floor");
        assert_eq!(app.view, View::UpperPlan, "{}", app.message);
        ribbon_button(&ctx, screen, &mut app, "Architecture", "wall");
        click_widget(&ctx, screen, &mut app, "wall_loc");
        click_widget(&ctx, screen, &mut app, "wall_loc:Finish Face: Exterior");
        for p in [[-3.0, -2.0], [3.0, -2.0], [3.0, 2.0], [-3.0, 2.0], [-3.0, -2.0]] { office_plan_click(&ctx, screen, &mut app, p); }
        assert_eq!(wall_axes(&app, first).len(), 4, "the first floor has its own walls: {}", app.message);
        assert_eq!(wall_axes(&app, ground).len(), 5, "the ground floor is unchanged");
        // a roof on the roof level, by its footprint at the outer face of the first-floor walls
        double_click_widget(&ctx, screen, &mut app, "view:Roof");
        assert!(matches!(app.view, View::Level(_)), "{:?}", app.view);
        ribbon_button(&ctx, screen, &mut app, "Architecture", "roof");
        for p in [[-3.2, -2.2], [3.2, -2.2], [3.2, 2.2], [-3.2, 2.2]] { office_plan_click(&ctx, screen, &mut app, p); }
        window_frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        let roofs: Vec<_> = app.document.elements().filter(|e| e.is_roof()).collect();
        assert_eq!(roofs.len(), 1, "{}", app.message);
        assert_eq!(app.document.level_of(roofs[0].id), Some(levels[2].0), "the roof is on the roof level");

        // Annotation on the ground-floor plan: a tag on the door, and an aligned dimension along the moved wall
        double_click_widget(&ctx, screen, &mut app, "view:Ground Floor");
        assert_eq!(app.view, View::Plan, "{}", app.message);
        ribbon_button(&ctx, screen, &mut app, "Annotate", "tag");
        let (_, a, b) = outer.iter().copied().find(|w| w.0 == host).unwrap();
        let door_at = [a[0] + (b[0] - a[0]) * 0.25, a[1] + (b[1] - a[1]) * 0.25];
        office_plan_click(&ctx, screen, &mut app, door_at);
        let tags: Vec<_> = app.document.elements().filter(|e| e.category == Category::Tag).collect();
        assert_eq!(tags.len(), 1, "{}", app.message);
        assert!(matches!(tags[0].geometry, Some(Geometry::Tag { host: h, .. }) if h == door), "the tag is on the door: {}", app.message);
        assert!(app.message.contains("Type B"), "the tag reads the door's type: {}", app.message);
        window_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        ribbon_button(&ctx, screen, &mut app, "Annotate", "dimension");
        for p in [a, b, [0.0, a[1] - 1.0]] { office_plan_click(&ctx, screen, &mut app, p); }
        let dims: Vec<_> = app.document.elements().filter(|e| matches!(e.geometry, Some(Geometry::Dimension { .. }))).collect();
        assert_eq!(dims.len(), 1, "{}", app.message);
        let Some(Geometry::Dimension { a: da, b: db, level: dl, .. }) = dims[0].geometry else { unreachable!() };
        assert_eq!(dl, ground, "the dimension belongs to the ground-floor plan");
        assert!(((db[0] - da[0]).hypot(db[1] - da[1]) - 6.2).abs() < 0.01, "it measures the 6.2 m wall axis: {da:?} {db:?}");
        // known limitation (capability register D-DIM): the dimension keeps two points, not references to the wall,
        // so a later edit of the wall does not update it; this pins that it stores coordinates only
        assert!(matches!(dims[0].geometry, Some(Geometry::Dimension { .. })) && dims[0].parent.is_none(), "a dimension has no element reference yet");
        window_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);

        // Schedules from the View tab: rooms and doors, with the fields Revit starts with
        let schedule_of = |app: &mut Desktop, kind: &str| -> u64 {
            let before: Vec<u64> = app.document.mep().schedules.iter().map(|s| s.id).collect();
            ribbon_button(&ctx, screen, app, "View", "schedule");
            click_widget(&ctx, screen, app, &format!("schedule_new:{kind}"));
            click_widget(&ctx, screen, app, "schedule_new:ok");
            click_widget(&ctx, screen, app, "schedule_props:ok");
            let id = app.document.mep().schedules.iter().map(|s| s.id).find(|id| !before.contains(id)).unwrap_or_else(|| panic!("a {kind} schedule is made: {}", app.message));
            assert_eq!(app.view, View::Schedule(id), "the new schedule opens");
            id
        };
        let room_schedule = schedule_of(&mut app, "Rooms");
        let table = app.document.schedule_table(app.document.schedule(room_schedule).unwrap());
        let col = |t: &openbim_document::ScheduleTable, label: &str| t.headings.iter().position(|h| h.starts_with(label)).unwrap_or_else(|| panic!("a {label} column in {:?}", t.headings));
        let data: Vec<_> = table.rows.iter().filter(|r| r.kind == openbim_document::RowKind::Data).collect();
        assert_eq!(data.len(), 2, "one row per room");
        let (number, area) = (col(&table, "Number"), col(&table, "Area"));
        for row in &data {
            let room = row.ids[0];
            assert_eq!(table.text(row, number), app.document.room_text(room, "Number"));
            let shown: f64 = table.text(row, area).split_whitespace().next().unwrap().parse().unwrap();
            assert!((shown - app.document.room_shape(room).unwrap().area()).abs() < 0.01, "the schedule's area is the room's: {shown}");
        }
        let door_schedule = schedule_of(&mut app, "Doors");
        let table = app.document.schedule_table(app.document.schedule(door_schedule).unwrap());
        let data: Vec<_> = table.rows.iter().filter(|r| r.kind == openbim_document::RowKind::Data).collect();
        assert_eq!((data.len(), data[0].ids.clone()), (1, vec![door]), "one door");
        assert!(table.text(data[0], col(&table, "Type")).contains("Type B"), "the door's type: {:?}", table.text(data[0], col(&table, "Type")));
        assert!(table.text(data[0], col(&table, "Width")).starts_with("1200"), "Type B is 1200 mm wide: {}", table.text(data[0], col(&table, "Width")));

        // A sheet with the ground-floor plan and the room schedule, printed to PDF
        ribbon_button(&ctx, screen, &mut app, "View", "sheet");
        type_into(&ctx, screen, &mut app, "sheet_new:number", "A-101");
        type_into(&ctx, screen, &mut app, "sheet_new:name", "Ground Floor Plan");
        click_widget(&ctx, screen, &mut app, "sheet_new:ok");
        let sheet = match app.view { View::Sheet(id) => id, v => panic!("the new sheet opens, not {v:?}: {}", app.message) };
        let room_schedule_name = app.document.schedule(room_schedule).unwrap().name.clone();
        for view in ["Ground Floor", room_schedule_name.as_str()] {
            click_widget(&ctx, screen, &mut app, "sheet:place_view");
            click_widget(&ctx, screen, &mut app, "add_view:view");
            click_widget(&ctx, screen, &mut app, &format!("add_view:view:{view}"));
            click_widget(&ctx, screen, &mut app, "add_view:add");
        }
        let def = app.document.sheet(sheet).unwrap().clone();
        assert_eq!((def.number.as_str(), def.name.as_str()), ("A-101", "Ground Floor Plan"));
        assert_eq!(def.viewports.iter().map(|v| v.view.clone()).collect::<Vec<_>>(), vec![format!("p{}", ground.0), openbim_document::schedule_view(room_schedule)], "{}", app.message);
        let dir = std::env::temp_dir().join(format!("openbim-two-storey-office-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pdf = dir.join("A-101.pdf");
        click_widget(&ctx, screen, &mut app, "sheet:print");
        crate::dialog::answer(Some(pdf.clone()));
        click_widget(&ctx, screen, &mut app, "print:go");
        let bytes = std::fs::read(&pdf).unwrap_or_else(|e| panic!("the PDF is written: {e} ({})", app.message));
        assert!(bytes.starts_with(b"%PDF-"), "a PDF file");
        // checked by an independent reader when one is installed: one page, the paper of the sheet
        if let Ok(out) = std::process::Command::new("pdfinfo").arg(&pdf).output() {
            let info = String::from_utf8_lossy(&out.stdout);
            assert!(info.lines().any(|l| l.starts_with("Pages:") && l.trim_end().ends_with(" 1")), "{info}");
            let (w, h) = def.dims_mm();
            let size = info.lines().find(|l| l.starts_with("Page size:")).unwrap_or_default().to_string();
            let pts: Vec<f64> = size.split_whitespace().filter_map(|t| t.parse().ok()).collect();
            assert!(pts.len() >= 2 && (pts[0] - w / 25.4 * 72.0).abs() < 1.0 && (pts[1] - h / 25.4 * 72.0).abs() < 1.0, "the page is the sheet's paper {w} x {h} mm: {size}");
        }

        // Save through the quick-access Save; the platform dialog's answer is the one substitution
        let file = dir.join("Bureau d'étude.obim");
        let asked = crate::dialog::asked();
        crate::dialog::answer(Some(file.clone()));
        assert!(ribbon_click(&ctx, screen, &mut app, "save"));
        assert_eq!(crate::dialog::asked(), asked + 1, "Save of an unnamed model asks where");
        assert!(file.exists() && !app.is_dirty(), "{}", app.message);
        let saved = openbim_document::native::records(&app.document);

        // New, then Open the file again
        assert!(ribbon_click(&ctx, screen, &mut app, "new"));
        assert!(app.document.elements().all(|e| e.category == Category::Level), "New leaves only its two levels");
        crate::dialog::answer(Some(file.clone()));
        assert!(ribbon_click(&ctx, screen, &mut app, "open"));
        assert_eq!(openbim_document::native::records(&app.document), saved, "the reopened file holds every record: {}", app.message);
        assert_eq!(app.levels().iter().map(|l| l.1.as_str()).collect::<Vec<_>>(), vec!["Ground Floor", "First Floor", "Roof"]);

        // Export IFC from the Insert tab, typing the file name in the dialog
        let ifc = dir.join("office.ifc");
        ribbon_button(&ctx, screen, &mut app, "Insert", "ifc_export");
        type_into(&ctx, screen, &mut app, "export:path", ifc.to_str().unwrap());
        click_widget(&ctx, screen, &mut app, "export:go");
        let text = std::fs::read_to_string(&ifc).unwrap_or_else(|e| panic!("the export wrote {}: {e} ({})", ifc.display(), app.message));
        let storey = |name: &str| -> f64 {
            let line = text.lines().find(|l| l.contains("=IFCBUILDINGSTOREY(") && l.contains(&format!("'{name}'"))).unwrap_or_else(|| panic!("a storey {name}"));
            line.trim_end_matches(';').trim_end_matches(')').rsplit(',').next().unwrap().parse().unwrap()
        };
        assert_eq!((storey("Ground Floor"), storey("First Floor"), storey("Roof")), (0.0, 3.6, 7.2), "the storeys keep the edited elevations");
        // the IFC carries what was drawn: 9 walls on their storeys, both rooms, the door and window in their openings
        assert_eq!(text.matches("=IFCBUILDINGSTOREY(").count(), 3);
        assert_eq!(text.matches("=IFCWALL(").count() + text.matches("=IFCWALLSTANDARDCASE(").count(), 9);
        assert_eq!(text.matches("=IFCSPACE(").count(), 2);
        assert_eq!(text.matches("=IFCDOOR(").count(), 1);
        assert_eq!(text.matches("=IFCWINDOW(").count(), 1);
        assert_eq!(text.matches("=IFCRELFILLSELEMENT(").count(), 2);
        // known limitation (capability register X-IFC): the U stair leaves as one straight IfcStair per run, not one
        // IfcStair aggregating IfcStairFlight and landing parts; this pins what is written today
        assert_eq!(text.matches("=IFCSTAIR(").count(), 2, "each run of the U stair");
        assert_eq!(text.matches(".STRAIGHT_RUN_STAIR.").count(), 2);
        assert!(text.contains("=IFCROOF(") || text.contains(".ROOF."), "the roof");
        assert!(text.contains(".CEILING."), "the ceiling");
        assert!(text.contains("'101 Office'") && text.contains("'102 Meeting'"), "the room numbers and names reach the IFC spaces");

        // Recovery: an edit after the save is written to the journal by the autosave timer (called directly: the
        // window's 30 s timer does not run in a test), then the program stops without saving
        let saved_text = std::fs::read_to_string(&file).unwrap();
        double_click_widget(&ctx, screen, &mut app, "view:Ground Floor");
        ribbon_button(&ctx, screen, &mut app, "Architecture", "wall");
        for p in [[-1.5, -1.0], [-1.5, 1.0]] { office_plan_click(&ctx, screen, &mut app, p); }
        window_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        let walls_now = wall_axes(&app, ground).len();
        assert_eq!(walls_now, 6, "{}", app.message);
        assert!(app.autosave_tick(std::time::Duration::ZERO), "the journal is written");
        let unsaved = openbim_document::native::records(&app.document);
        drop(app);
        // a new session opens the file and is offered the unsaved work
        let mut app = Desktop::sample();
        window_frame(&ctx, screen, &mut app, vec![]);
        assert!(ribbon_click(&ctx, screen, &mut app, "new"));
        crate::dialog::answer(Some(file.clone()));
        assert!(ribbon_click(&ctx, screen, &mut app, "open"));
        assert_eq!(wall_axes(&app, app.levels()[0].0).len(), 5, "the file holds what was saved");
        click_widget(&ctx, screen, &mut app, "recovery:recover");
        assert_eq!(openbim_document::native::records(&app.document), unsaved, "the recovered model is the unsaved one: {}", app.message);
        assert!(app.is_dirty(), "recovered work stays unsaved until it is saved");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), saved_text, "the saved file is untouched");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn escape_steps_back_inside_the_framing_and_run_tools_too() {
        let (ctx, screen) = frames();
        // the truss keeps its start
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let level = app.plan_level();
        app.run_command("truss", "Truss");
        app.truss_click(level, [1.0, 1.0]);
        assert!(app.beam_start.is_some(), "{}", app.message);
        key_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Truss && app.beam_start.is_none(), "{}", app.message);
        key_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select, "{}", app.message);
        // the beam keeps its start
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.run_command("beam", "Beam");
        app.beam_click(app.plan_level(), [1.0, 1.0]);
        assert!(app.beam_start.is_some());
        key_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Beam && app.beam_start.is_none(), "{}", app.message);
        key_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select, "{}", app.message);
        // a duct run with one point takes the point back, and the second Esc ends the run
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.run_command("duct", "Duct");
        app.run_click([1.0, 1.0]);
        assert_eq!(app.run_points.len(), 1);
        key_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Duct && app.run_points.is_empty(), "{}", app.message);
        key_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select, "{}", app.message);
        // the reference of Align is dropped first: the global handler must not steal that Esc
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.run_command("align", "Align");
        app.edit_ref = Some((ElementId(20), [1.0, 0.0]));
        key_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Align && app.edit_ref.is_none(), "{}", app.message);
        key_frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select, "{}", app.message);
    }

    #[test]
    fn the_framing_and_footing_tools_start_from_their_own_numbers() {
        let mut app = Desktop::sample();
        app.run_command("rebar", "Rebar");
        assert_eq!((app.width_mm, app.height_mm), (16.0, 8.0));
        app.run_command("beam", "Beam");
        assert_eq!((app.width_mm, app.height_mm), (200.0, 300.0), "a beam is not 16 x 8 mm because a rebar came before it");
        app.run_command("truss", "Truss");
        assert_eq!((app.width_mm, app.height_mm, app.elevation_mm, app.spacing_mm), (200.0, 300.0, 2700.0, 600.0));
        app.run_command("rebar", "Rebar");
        app.run_command("structural_column", "Column");
        assert_eq!((app.width_mm, app.height_mm), (200.0, 300.0));
        app.run_command("ceiling", "Ceiling");
        assert_eq!(app.slab_offset_mm, 2700.0);
        app.run_command("foundation", "Foundation");
        assert!(app.tool == crate::Tool::Footing, "{}", app.message);
        assert_eq!((app.width_mm, app.height_mm, app.footing_thickness_mm, app.slab_offset_mm), (200.0, 300.0, 500.0, 0.0),
            "the pad starts from its own box, not from the ceiling's height");
        app.run_command("ceiling", "Ceiling");
        app.run_command("footing", "Wall Footing");
        assert!(app.tool == crate::Tool::WallFooting, "{}", app.message);
        assert_eq!((app.width_mm, app.height_mm, app.slab_offset_mm), (200.0, 300.0, 0.0));
    }

    #[test]
    fn a_level_rename_keeps_its_draft_across_frames_and_commits_on_enter() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        let level = app.levels()[0].0;
        app.run_command("level", "Level");
        app.start_level_rename(level);
        app.level_rename_name = "Grou".into();
        window_frame(&ctx, screen, &mut app, vec![]);
        assert_eq!(app.level_rename_name, "Grou", "the draft survives a frame instead of being rebuilt from the document");
        app.level_rename_name.push('n');
        app.level_rename_name.push('d');
        window_frame(&ctx, screen, &mut app, vec![]);
        assert_eq!(app.level_rename_name, "Ground");
        app.commit_level_rename();
        assert_eq!(app.document.get(level).unwrap().name, "Ground", "{}", app.message);
        assert!(app.level_rename_id.is_none(), "the field closes when the name is accepted");
        // a name another level has is refused, and the field stays open so it can be corrected
        app.start_level_rename(level);
        app.level_rename_name = "Level 2".into();
        app.commit_level_rename();
        assert!(app.message.contains("not renamed"), "{}", app.message);
        assert_eq!(app.level_rename_id, Some(level), "the field stays open on a refused name");
        app.cancel_level_rename();
        assert!(app.level_rename_id.is_none());
        assert_eq!(app.document.get(level).unwrap().name, "Ground", "cancelling changes nothing");
    }

    #[test]
    fn the_browser_and_the_sheet_plans_take_their_levels_from_the_document() {
        let mut app = Desktop::sample();
        app.reset_model();
        assert_eq!(app.levels().len(), 2);
        let third = app.document.place_level("Level 3", 6.0).unwrap();
        let extra: Vec<_> = app.levels().into_iter().skip(2).collect();
        assert_eq!(extra.len(), 1, "the browser adds the levels the tree does not show");
        assert_eq!(extra[0].0, third);
        app.rename_level(third, "Roof");
        assert_eq!(crate::panels::level_view_name(&app, "Level plan", View::Level(third.0)), "Roof");
        assert_eq!(crate::panels::level_view_name(&app, "Level 1", View::Plan), app.levels()[0].1);
        // a sheet's plan views resolve their level from the document, never from a sample id
        assert_eq!(crate::sheet_draw::plan_level_id(&app.document, "mech"), Some(app.levels()[0].0));
        assert_eq!(crate::sheet_draw::plan_level_id(&app.document, "p10"), Some(app.levels()[0].0));
        assert_eq!(crate::sheet_draw::plan_level_id(&app.document, "p11"), Some(app.levels()[1].0));
        assert_eq!(crate::sheet_draw::plan_level_id(&app.document, &format!("p{}", third.0)), Some(third));
    }

    #[test]
    fn a_level_is_renamed_and_deleted_from_the_options_bar_of_the_level_tool() {
        let mut app = Desktop::sample();
        let levels = app.levels();
        assert_eq!(levels.len(), 2);
        // a level with nothing on it can be renamed and deleted
        let spare = app.document.place_level("Spare", 6.0).unwrap();
        app.rename_level(spare, "Roof");
        assert_eq!(app.document.get(spare).unwrap().name, "Roof", "{}", app.message);
        assert!(app.message.contains("renamed"), "{}", app.message);
        app.rename_level(spare, "Level 1");
        assert!(app.message.contains("not renamed"), "a name another level has is refused: {}", app.message);
        app.delete_level(spare);
        assert!(app.document.get(spare).is_none(), "{}", app.message);
        app.delete_level(levels[0].0);
        assert!(app.document.get(levels[0].0).is_some());
        assert!(app.message.contains("cannot be deleted"), "{}", app.message);
    }

    #[test]
    fn the_options_bar_does_not_leak_from_one_tool_to_the_next() {
        let mut app = Desktop::sample();
        app.run_command("ceiling", "Ceiling");
        assert_eq!(app.slab_offset_mm, 2700.0, "the ceiling hangs 2.7 m above its level");
        app.run_command("door", "Door");
        assert_eq!(app.slab_offset_mm, 0.0, "a door is not placed 2.7 m up because a ceiling was drawn before it");
        assert!(app.tool == crate::Tool::Family, "{}", app.message);
        app.run_command("ceiling", "Ceiling");
        assert_eq!(app.slab_offset_mm, 2700.0, "the ceiling gets its own numbers back");
        app.run_command("wall", "Wall");
        assert_eq!((app.width_mm, app.height_mm, app.elevation_mm), (200.0, 3000.0, 0.0));
        app.run_command("duct", "Duct");
        assert_eq!((app.width_mm, app.height_mm, app.elevation_mm), (400.0, 300.0, 2700.0));
        app.run_command("door", "Door");
        assert_eq!(app.slab_offset_mm, 0.0, "and the duct's elevation does not follow the door either");
    }

    #[test]
    fn a_new_model_has_its_own_levels_and_takes_a_drawn_wall_and_grid() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.reset_model();
        let levels = app.levels();
        assert_eq!(levels.len(), 2, "New gives Level 1 and Level 2: {:?}", levels.iter().map(|l| l.1.clone()).collect::<Vec<_>>());
        assert_eq!((levels[0].2, levels[1].2), (0.0, 3.0));
        assert_eq!(app.plan_level(), levels[0].0, "the plan sits on the lowest level of the document");
        assert_ne!(app.plan_level(), ElementId(10), "a new model must not point at the sample's level");
        app.view = View::UpperPlan;
        assert_eq!(app.plan_level(), levels[1].0, "the upper plan sits on the second level");
        app.view = View::Plan;
        // two clicks with the Wall tool, on a canvas with nothing drawn on it yet (A1)
        app.run_command("wall", "Wall");
        assert!(app.tool == crate::Tool::Wall, "{}", app.message);
        let (a, b) = (Pos2::new(400.0, 300.0), Pos2::new(600.0, 300.0));
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(a)]);
        for at in [a, b] {
            frame(&ctx, screen, &mut app, vec![button(at, true)]);
            frame(&ctx, screen, &mut app, vec![button(at, false)]);
        }
        assert_eq!(walls(&app), 1, "{}", app.message);
        let wall = app.document.elements().find(|e| e.category == Category::Wall).expect("the wall that was drawn");
        assert_eq!(app.document.level_of(wall.id), Some(levels[0].0), "the wall stands on the level of the plan");
        // and a grid, on the same empty canvas
        app.choose_tool(crate::Tool::Grid);
        for at in [Pos2::new(300.0, 200.0), Pos2::new(300.0, 500.0)] {
            frame(&ctx, screen, &mut app, vec![button(at, true)]);
            frame(&ctx, screen, &mut app, vec![button(at, false)]);
        }
        assert_eq!(app.document.elements().filter(|e| e.category == Category::Grid).count(), 1, "{}", app.message);
    }

    #[test]
    fn a_project_with_no_level_refuses_a_drawing_tool() {
        let mut app = Desktop::sample();
        app.document = openbim_document::Document::new();
        app.view = View::Plan;
        app.choose_tool(crate::Tool::Wall);
        assert!(app.tool == crate::Tool::Select, "the tool must not start without a level");
        assert!(app.message.contains("Add a level first"), "{}", app.message);
    }

    #[test]
    fn plan_bounds_falls_back_to_a_default_view_on_an_empty_model() {
        // a real extent passes through unchanged
        let real = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(4.0, 3.0));
        assert_eq!(plan_bounds(real), real);
        // an empty model (Rect::NOTHING has an infinite min) gets a finite, centred default view
        let empty = plan_bounds(Rect::NOTHING);
        assert!(empty.is_finite(), "the fallback bounds must be finite");
        assert_eq!(empty.center(), Pos2::ZERO, "the empty view is centred on the origin");
        assert_eq!(empty.width(), EMPTY_PLAN_SPAN_M);
    }

    #[test]
    fn a_new_project_starts_with_an_empty_canvas_but_the_drawing_tools_still_work() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.document = openbim_document::Document::new();
        assert_eq!(app.document.elements().count(), 0, "the test must start from an empty project");
        app.view = View::Plan;
        app.choose_tool(crate::Tool::Level);
        let before = app.document.elements().count();
        let p = Pos2::new(500.0, 350.0);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(p)]);
        frame(&ctx, screen, &mut app, vec![button(p, true)]);
        frame(&ctx, screen, &mut app, vec![button(p, false)]);
        assert!(app.document.elements().count() > before, "the Level tool must place a level on an empty canvas; message: {}", app.message);
    }

    #[test]
    fn plan_tabs_name_the_first_two_levels_and_give_every_other_level_its_own_tab() {
        // the first two levels are the floor and the upper plan, whatever their ids are, and the rest get their own tab
        let named = vec![(ElementId(10), "Ground".to_string(), 0.0), (ElementId(11), "First".to_string(), 3.0), (ElementId(20), "Roof".to_string(), 9.0)];
        assert_eq!(plan_tabs(&named), vec![
            ("Ground".to_string(), View::Plan),
            ("First".to_string(), View::UpperPlan),
            ("Roof".to_string(), View::Level(20)),
        ]);
        // a project whose levels have other ids (a new project numbers them from 1): the tabs still name the
        // document's own levels, and `plan_level` resolves the first two to levels that exist
        let renumbered = vec![(ElementId(1), "Level 1".to_string(), 0.0), (ElementId(2), "Level 2".to_string(), 3.0)];
        assert_eq!(plan_tabs(&renumbered), vec![
            ("Level 1".to_string(), View::Plan),
            ("Level 2".to_string(), View::UpperPlan),
        ]);
        // no levels at all (an IFC-loaded project with none): there is no plan tab to offer
        assert!(plan_tabs(&[]).is_empty());
    }

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
        frame(&ctx, screen, &mut app, vec![button(c, true)]);
        frame(&ctx, screen, &mut app, vec![button(c, false)]);
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
    fn a_pipe_drawn_with_clicks_and_a_slope_falls_and_its_free_end_is_edited_in_properties() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.run_command("pipe", "Pipe");
        (app.run_opts.slope, app.run_opts.slope_pct) = (crate::mep_tool::SlopeMode::Down, 2.0);
        let pipes = |a: &Desktop| a.document.elements().filter(|e| e.category == Category::Pipe).map(|e| e.id).collect::<Vec<_>>();
        let n = pipes(&app).len();
        let (d, e) = (Pos2::new(200.0, 500.0), Pos2::new(420.0, 500.0));
        click(&ctx, screen, &mut app, d);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(e)]);
        // the tooltip follows the pointer while the options bar and the view draw
        assert!(app.run_slope_tip(app.run_points[0], [app.run_points[0][0] + 2.0, app.run_points[0][1]]).is_some());
        click(&ctx, screen, &mut app, e);
        assert_eq!(app.run_points.len(), 2, "{}", app.message);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        let ids = pipes(&app);
        assert_eq!(ids.len(), n + 1, "{}", app.message);
        let id = *ids.iter().max().unwrap();
        let run = app.document.mep_run(id).unwrap();
        let plan = (run.b[0] - run.a[0]).hypot(run.b[1] - run.a[1]);
        assert!(plan > 1.0 && ((run.a[2] - run.b[2]) / plan - 0.02).abs() < 1e-9, "falls 2 % along the drawing: {run:?}");
        // Properties: the end offsets of the selected pipe; its free end goes 100 mm lower, the start stays
        let level_z = app.document.get(ElementId(10)).unwrap().elevation_m;
        let end_now = (run.b[2] - level_z) * 1000.0;
        app.select_element(Some(id));
        frame(&ctx, screen, &mut app, vec![]);
        app.set_run_end_offset(id, false, end_now - 100.0);
        let after = app.document.mep_run(id).unwrap();
        assert!((after.a[2] - run.a[2]).abs() < 1e-9 && (after.b[2] - (run.b[2] - 0.1)).abs() < 1e-9, "{after:?}");
        assert!(app.message.starts_with("End of the run"), "{}", app.message);
        app.undo_document();
        assert!((app.document.mep_run(id).unwrap().b[2] - run.b[2]).abs() < 1e-9);
        frame(&ctx, screen, &mut app, vec![]);
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
    fn paste_follows_the_pointer_with_a_ghost_and_a_click_puts_it_down_again_and_again() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.select_element(Some(ElementId(24)));
        app.copy_to_clipboard(false);
        app.paste_start();
        assert!(app.tool == crate::Tool::Paste, "{}", app.message);
        let (before, at) = (walls(&app), Pos2::new(500.0, 350.0));
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at + Vec2::new(30.0, 0.0))]);
        assert_eq!(walls(&app), before, "moving the pointer pastes nothing");
        click(&ctx, screen, &mut app, at);
        assert_eq!(walls(&app), before + 1, "{}", app.message);
        let first = app.selected_id().unwrap();
        // the centre of the wall went to the pointer: the pasted wall is as long as the first and elsewhere
        let (a, b) = (wall_line(&app, first), wall_line(&app, ElementId(24)));
        assert!(((a.0[0] - a.1[0]).hypot(a.0[1] - a.1[1]) - (b.0[0] - b.1[0]).hypot(b.0[1] - b.1[1])).abs() < 1e-9 && a != b);
        assert!(app.tool == crate::Tool::Paste, "the tool stays on");
        click(&ctx, screen, &mut app, at + Vec2::new(0.0, 60.0));
        assert_eq!(walls(&app), before + 2);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select);
        // a pinned element is drawn with its pushpin and the frame still draws
        app.select_element(Some(ElementId(20)));
        app.pin_selected(true);
        frame(&ctx, screen, &mut app, vec![]);
        assert_eq!(app.pin_marks(ElementId(10)).len(), 1);
        assert!(app.pin_marks(ElementId(11)).is_empty(), "the pin belongs to its level");
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
        let mut probe = Desktop::sample();
        probe.view = app.view;
        probe.document = app.document.clone();
        for y in (100..600).step_by(6) { for x in (60..940).step_by(2) {
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
    fn dragging_a_section_end_moves_only_that_end_and_undo_puts_it_back() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let level = app.plan_level();
        let (y, left, right) = wall_span(&ctx, screen, &app);
        // the section lies along wall 20 (y = 0 m); one metre is (right - left + 8) / 8 pixels, as the wall test measures it
        let per_m = (right - left + 8.0) / 8.0;
        let x_at = |m: f32| left + 1.0 + per_m * m;
        let id = app.document.place_section(level, [1.0, 0.0], [7.0, 0.0], 20.0).unwrap();
        app.select_element(Some(id));
        frame(&ctx, screen, &mut app, vec![]);
        // the first row that picks the wall is its upper edge: the line is half the wall's thickness (0.1 m) lower
        let y = y + per_m * 0.1;
        let (press, to) = (Pos2::new(x_at(7.0), y), Pos2::new(x_at(7.0), y + 60.0));
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press)]);
        frame(&ctx, screen, &mut app, vec![button(press, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press + Vec2::new(0.0, 20.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(to)]);
        assert!(matches!(app.drag, Some(crate::Drag::Grip { at_start: false, .. })), "{:?} {}", app.drag, app.message);
        frame(&ctx, screen, &mut app, vec![button(to, false)]);
        let (a, b) = wall_line(&app, id);
        assert!(a == [1.0, 0.0] && b[1] < -0.3 && (b[0] - 7.0).abs() < 0.5, "{a:?} {b:?} {}", app.message);
        // the wall under it did not move, and the section's view is built from the new line
        assert_eq!(wall_line(&app, ElementId(20)), ([0.0, 0.0], [8.0, 0.0]));
        assert_eq!(crate::section_ui::build(&app.document, View::Section(id.0)).expect("builds").section.b, b);
        app.document.undo().unwrap();
        assert_eq!(wall_line(&app, id), ([1.0, 0.0], [7.0, 0.0]));
        // no length at all is refused
        assert!(app.document.wall_end_commands(id, false, [1.0, 0.01]).is_err());
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
        // Polygon, circle and arc: joined walls in one undo step, the exterior outward along the Location Line
        app.wall_loc = 2;
        app.width_mm = 200.0;
        app.wall_mode = 3;
        app.wall_sides = 6;
        let n = walls(&app);
        app.wall_click(level, [40.0, 10.0]);
        assert_eq!(walls(&app), n);
        app.wall_click(level, [43.0, 10.0]);
        assert_eq!(walls(&app), n + 6, "{}", app.message);
        let ids: Vec<_> = app.document.elements().filter(|e| e.category == Category::Wall).map(|e| e.id).skip(n).collect();
        assert!(ids.iter().all(|i| app.document.wall_plan(*i).is_some()));
        // the exterior faces are on the hexagon with its corners 3 m from the centre
        let far = ids.iter().flat_map(|i| app.document.wall_plan(*i).unwrap()).map(|p| (p[0] - 40.0).hypot(p[1] - 10.0)).fold(0.0, f64::max);
        assert!((far - 3.0).abs() < 0.2 / 2.0 + 0.05 && far > 2.9, "outer corners about 3 m out: {far}");
        app.document.undo().unwrap();
        assert_eq!(walls(&app), n);
        // circumscribed: the point picked is the middle of a side, and four sides make a square 6 m across
        app.wall_mode = 4;
        app.wall_sides = 4;
        app.wall_click(level, [40.0, 10.0]);
        app.wall_click(level, [43.0, 10.0]);
        assert_eq!(walls(&app), n + 4, "{}", app.message);
        let ids: Vec<_> = app.document.elements().filter(|e| e.category == Category::Wall).map(|e| e.id).skip(n).collect();
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for id in &ids { for p in app.document.wall_plan(*id).unwrap() { for k in 0..2 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } } }
        assert!(near(lo, [37.0, 7.0]) && near(hi, [43.0, 13.0]), "exterior faces on the square: {lo:?} {hi:?}");
        app.document.undo().unwrap();
        // a circle of 2 m radius is many walls, all of them placed or none
        app.wall_mode = 5;
        app.wall_click(level, [40.0, 10.0]);
        app.wall_click(level, [42.0, 10.0]);
        let made = walls(&app) - n;
        assert!(made >= 12, "{made}: {}", app.message);
        assert!(app.message.contains("straight walls"));
        app.document.undo().unwrap();
        assert_eq!(walls(&app), n);
        // an arc keeps its ends: start, end, a point on it
        app.wall_mode = 6;
        app.wall_loc = 0;
        app.wall_click(level, [30.0, 10.0]);
        app.wall_click(level, [34.0, 10.0]);
        assert_eq!(walls(&app), n);
        app.wall_click(level, [32.0, 12.0]);
        let made: Vec<_> = app.document.elements().filter(|e| e.category == Category::Wall).skip(n).collect();
        assert!(made.len() >= 2, "{}", app.message);
        let first = match made[0].geometry { Some(Geometry::Linear { start, .. }) => start, _ => panic!() };
        let last = match made.last().unwrap().geometry { Some(Geometry::Linear { end, .. }) => end, _ => panic!() };
        assert!(near(first, [30.0, 10.0]) && near(last, [34.0, 10.0]), "{first:?} {last:?}");
        // three points on a line make no arc, and nothing is left half done
        let before = walls(&app);
        app.wall_click(level, [0.0, 50.0]);
        app.wall_click(level, [4.0, 50.0]);
        app.wall_click(level, [2.0, 50.0]);
        assert_eq!(walls(&app), before);
        assert!(app.wall_pts.is_empty() && app.message.contains("no shape"), "{}", app.message);
        app.wall_mode = 0;
        app.wall_loc = 2;
        // the ghost of each shape draws while the pointer moves
        for mode in 3..=6 {
            app.wall_mode = mode;
            app.wall_pts = vec![[40.0, 10.0], [43.0, 10.0]][..if mode == 6 { 2 } else { 1 }].to_vec();
            frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(Pos2::new(420.0, 310.0))]);
        }
        app.wall_pts.clear();
        app.wall_mode = 0;
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

    fn frame_mod(ctx: &egui::Context, screen: Rect, app: &mut Desktop, events: Vec<egui::Event>, modifiers: egui::Modifiers) {
        let events: Vec<egui::Event> = std::iter::once(egui::Event::ModifiersChanged(modifiers)).chain(events).collect();
        let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() },
            |ui| { egui::CentralPanel::default().show(ui, |ui| show(app, ui)); });
        out.textures_delta.clear();
    }

    #[test]
    fn several_elements_are_selected_by_ctrl_click_by_a_box_and_edited_together() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let level = match app.document.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6;
        let b = app.document.place_wall(level, [0.0, 4.0], [8.0, 4.0], 0.2, 3.0, 0.0).unwrap();
        // API: the selection grows and shrinks, the first is the primary one
        app.select_element(Some(ElementId(20)));
        app.change_selection(b, true);
        assert_eq!(app.selected_ids(), vec![ElementId(20), b]);
        app.change_selection(ElementId(20), false);
        assert_eq!(app.selected_ids(), vec![b]);
        app.select_many(vec![ElementId(20), b]);
        // Move and Copy act on all of them
        app.run_command("move", "Move");
        app.edit_click([0.0, 0.0]);
        app.edit_click([0.0, -10.0]);
        assert!(near(wall_line(&app, ElementId(20)).0, [0.0, -10.0]) && near(wall_line(&app, b).0, [0.0, -6.0]), "{}", app.message);
        app.document.undo().unwrap();
        assert!(app.selected_ids().len() == 2);
        let n = walls(&app);
        app.run_command("copy", "Copy");
        app.edit_click([0.0, 0.0]);
        app.edit_click([20.0, 0.0]);
        assert_eq!(walls(&app), n + 2, "{}", app.message);
        assert_eq!(app.selected_ids().len(), 2, "the copies are selected");
        app.document.undo().unwrap();
        // Delete removes both, with one undo
        app.choose_tool(crate::Tool::Select);
        app.select_many(vec![ElementId(20), b]);
        app.delete_selected();
        assert!(app.document.get(ElementId(20)).is_none() && app.document.get(b).is_none(), "{}", app.message);
        app.document.undo().unwrap();
        assert!(app.document.get(ElementId(20)).is_some() && app.document.get(b).is_some());
        // Ctrl or Cmd click adds the clicked wall, Shift click takes it away
        app.select_element(None);
        let (y, left, right) = wall_span(&ctx, screen, &app);
        let mid = Pos2::new((left + right) / 2.0, y);
        let click = |app: &mut Desktop, at: Pos2, m: egui::Modifiers| {
            frame_mod(&ctx, screen, app, vec![egui::Event::PointerMoved(at)], m);
            frame_mod(&ctx, screen, app, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: m }], m);
            frame_mod(&ctx, screen, app, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: m }], m);
        };
        click(&mut app, mid, egui::Modifiers::NONE);
        assert_eq!(app.selected_ids(), vec![ElementId(20)]);
        app.select_many(vec![b]);
        click(&mut app, mid, egui::Modifiers::COMMAND);
        assert!(app.selected_ids().contains(&b) && app.selected_ids().contains(&ElementId(20)), "{:?}", app.selected_ids());
        click(&mut app, mid, egui::Modifiers::SHIFT);
        assert_eq!(app.selected_ids(), vec![b]);
    }

    #[test]
    fn a_box_selects_what_it_holds_left_to_right_and_what_it_touches_right_to_left() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.select_element(None);
        let drag = |app: &mut Desktop, a: Pos2, b: Pos2| {
            frame(&ctx, screen, app, vec![egui::Event::PointerMoved(a)]);
            frame(&ctx, screen, app, vec![button(a, true)]);
            frame(&ctx, screen, app, vec![egui::Event::PointerMoved(a + (b - a) * 0.5)]);
            frame(&ctx, screen, app, vec![egui::Event::PointerMoved(b)]);
            frame(&ctx, screen, app, vec![button(b, false)]);
        };
        let (y, left, right) = wall_span(&ctx, screen, &app);
        // a small box left to right around the middle of the wall holds nothing whole
        drag(&mut app, Pos2::new(15.0, 130.0), Pos2::new(left + 20.0, y + 25.0));
        assert!(!app.selected_ids().contains(&ElementId(20)), "{:?}", app.selected_ids());
        // right to left, touching the wall only at its left part
        let _ = right;
        drag(&mut app, Pos2::new(left + 20.0, y + 25.0), Pos2::new(15.0, 130.0));
        assert!(app.selected_ids().contains(&ElementId(20)), "{:?} {}", app.selected_ids(), app.message);
        // everything inside a big box, left to right
        app.select_element(None);
        drag(&mut app, Pos2::new(8.0, 125.0), Pos2::new(990.0, 695.0));
        let all = app.selected_ids();
        assert!(all.len() > 1 && all.contains(&ElementId(20)), "{all:?} {}", app.message);
        // Delete takes them all, one undo brings them back
        let n = app.document.elements().count();
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Delete)]);
        assert!(app.document.elements().count() < n);
        app.document.undo().unwrap();
        assert_eq!(app.document.elements().count(), n);
    }

    #[test]
    fn a_selected_wall_shows_temporary_dimensions_and_typing_a_number_changes_the_wall() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let level = match app.document.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let w = app.document.place_wall(level, [1.0, -3.0], [6.0, -3.0], 0.2, 3.0, 0.0).unwrap();
        app.select_element(Some(w));
        frame(&ctx, screen, &mut app, vec![]);
        let find = |app: &Desktop, want: TempDim| app.temp_labels.iter().find(|l| l.1 == want).map(|l| l.0.center());
        let at = find(&app, TempDim::Length(w)).expect("the length is shown");
        assert!(app.temp_labels.len() >= 2, "the length and a neighbour: {:?}", app.temp_labels);
        // click the length, type 4000 and Enter: the end moves
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, screen, &mut app, vec![button(at, true)]);
        frame(&ctx, screen, &mut app, vec![button(at, false)]);
        assert_eq!(app.temp_edit, Some(TempDim::Length(w)), "{}", app.message);
        assert_eq!(app.temp_text, "5000");
        app.temp_text = "4000".into();
        frame(&ctx, screen, &mut app, vec![]);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        assert_eq!(wall_line(&app, w), ([1.0, -3.0], [5.0, -3.0]), "{}", app.message);
        assert!(app.temp_edit.is_none());
        // the distance to the neighbour on the other side: type a new one and the wall moves
        frame(&ctx, screen, &mut app, vec![]);
        let (dim, at) = app.temp_labels.iter().find_map(|l| match l.1 { d @ TempDim::Distance(..) => Some((d, l.0.center())), _ => None }).expect("a neighbour is shown");
        let TempDim::Distance(_, other) = dim else { unreachable!() };
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, screen, &mut app, vec![button(at, true)]);
        frame(&ctx, screen, &mut app, vec![button(at, false)]);
        assert_eq!(app.temp_edit, Some(dim), "{}", app.message);
        app.temp_text = "2500".into();
        app.apply_temp_dim();
        let (s, _) = wall_line(&app, w);
        let (o, _) = app.document.line_of(other).unwrap();
        // the wall is 2.5 m from the neighbour's line (a horizontal line)
        assert!(((s[1] - o[1]).abs() - 2.5).abs() < 1e-6, "{s:?} {o:?} {}", app.message);
        app.document.undo().unwrap();
        // not a number
        app.temp_edit = Some(TempDim::Length(w));
        app.temp_text = "abc".into();
        app.apply_temp_dim();
        assert!(app.message.contains("number"));
        // Delete does not delete the wall while a number is being typed
        app.temp_edit = Some(TempDim::Length(w));
        app.temp_text = "1".into();
        frame(&ctx, screen, &mut app, vec![]);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Backspace)]);
        assert!(app.document.get(w).is_some(), "the wall is still there");
    }

    #[test]
    fn dimension_and_measure_tools_take_points_and_the_dimension_is_kept() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let dims = |a: &Desktop| a.document.elements().filter(|e| e.category == Category::Dimension).count();
        app.run_command("dimension", "Aligned");
        assert!(app.tool == crate::Tool::Dimension);
        app.dimension_click(ElementId(10), [0.0, 0.0]);
        app.dimension_click(ElementId(10), [4.0, 3.0]);
        assert_eq!(dims(&app), 0);
        app.dimension_click(ElementId(10), [1.0, 4.0]);
        assert_eq!(dims(&app), 1, "{}", app.message);
        assert!(app.message.contains("5000"), "{}", app.message);
        frame(&ctx, screen, &mut app, vec![]);
        app.document.undo().unwrap();
        assert_eq!(dims(&app), 0);
        // a dimension is selected, moved and deleted like any element
        app.dimension_click(ElementId(10), [0.0, 0.0]);
        app.dimension_click(ElementId(10), [2.0, 0.0]);
        app.dimension_click(ElementId(10), [1.0, 1.0]);
        let d = app.document.elements().find(|e| e.category == Category::Dimension).unwrap().id;
        app.choose_tool(crate::Tool::Select);
        app.select_element(Some(d));
        app.delete_selected();
        assert_eq!(dims(&app), 0);
        // Measure shows the distance and keeps the two points
        app.run_command("measure", "Measure");
        app.measure_click([0.0, 0.0]);
        app.measure_click([3.0, 4.0]);
        assert!(app.message.contains("5000") && app.measured == Some(([0.0, 0.0], [3.0, 4.0])), "{}", app.message);
        frame(&ctx, screen, &mut app, vec![]);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select);
    }

    #[test]
    fn wall_joins_tool_disallows_and_allows_the_join_at_a_wall_end() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let ids = app.document.place_walls(ElementId(10), &[([50.0, 0.0], [60.0, 0.0]), ([60.0, 0.0], [60.0, 10.0])], 0.2, 3.0, 0.0, None).unwrap();
        app.run_command("wall_joins", "Wall Joins");
        assert!(app.tool == crate::Tool::WallJoins);
        frame(&ctx, screen, &mut app, vec![]);
        let before = app.document.wall_plan(ids[0]).unwrap();
        app.wall_join_click(ids[0], false);
        assert_eq!(app.document.get(ids[0]).unwrap().join_ends, [true, false], "{}", app.message);
        assert!(app.document.wall_plan(ids[0]).unwrap() != before);
        frame(&ctx, screen, &mut app, vec![]);
        app.wall_join_click(ids[0], false);
        assert_eq!(app.document.get(ids[0]).unwrap().join_ends, [true, true]);
        assert_eq!(app.document.wall_plan(ids[0]).unwrap(), before);
        app.document.undo().unwrap();
        assert_eq!(app.document.get(ids[0]).unwrap().join_ends, [true, false]);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select);
    }

    #[test]
    fn room_tool_places_a_room_in_a_closed_space_shows_its_area_and_follows_the_walls() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let rooms = |a: &Desktop| a.document.elements().filter(|e| e.category == Category::Room).count();
        app.run_command("room", "Room");
        assert!(app.tool == crate::Tool::Room);
        // find a spot on screen inside a closed space by trying clicks on a copy
        let mut probe = Desktop::sample();
        probe.view = View::Plan;
        probe.run_command("room", "Room");
        let mut spot = None;
        'scan: for y in (120..560).step_by(25) { for x in (80..920).step_by(25) {
            let at = Pos2::new(x as f32, y as f32);
            frame(&ctx, screen, &mut probe, vec![egui::Event::PointerMoved(at)]);
            frame(&ctx, screen, &mut probe, vec![button(at, true)]);
            frame(&ctx, screen, &mut probe, vec![button(at, false)]);
            if rooms(&probe) == 1 { spot = Some(at); break 'scan; }
        } }
        let at = spot.expect("a click inside a closed space of the sample places a room");
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, screen, &mut app, vec![button(at, true)]);
        frame(&ctx, screen, &mut app, vec![button(at, false)]);
        assert_eq!(rooms(&app), 1, "{}", app.message);
        assert!(app.message.contains("Room 1") && app.message.contains("m²"), "{}", app.message);
        let room = app.document.elements().find(|e| e.category == Category::Room).unwrap().id;
        let area = app.document.room_shape(room).unwrap().area();
        assert!((area - 3.8 * 5.8).abs() < 1e-6, "{area}");
        // a second click in the same space adds nothing
        frame(&ctx, screen, &mut app, vec![button(at, true)]);
        frame(&ctx, screen, &mut app, vec![button(at, false)]);
        assert_eq!(rooms(&app), 1);
        assert!(app.message.contains("already"), "{}", app.message);
        // outside every space: nothing
        let outside = Pos2::new(40.0, 130.0);
        frame(&ctx, screen, &mut app, vec![button(outside, true)]);
        frame(&ctx, screen, &mut app, vec![button(outside, false)]);
        assert_eq!(rooms(&app), 1);
        // Esc ends the tool; a click in the room then selects it
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select);
        frame(&ctx, screen, &mut app, vec![button(at, true)]);
        frame(&ctx, screen, &mut app, vec![button(at, false)]);
        assert_eq!(app.selected_id(), Some(room), "{}", app.message);
        // the room follows its walls: the wall that divides the sample is moved by 1 m
        let divider = app.document.elements().find(|e| matches!(e.geometry, Some(Geometry::Linear { start, end, .. }) if e.category == Category::Wall && start[0] == 4.0 && end[0] == 4.0)).expect("the sample has a wall at x = 4").id;
        let ops = app.document.transform_commands(&[divider], openbim_document::Xf::Move([-1.0, 0.0])).unwrap();
        app.document.transact(app.document.revision(), &ops).unwrap();
        frame(&ctx, screen, &mut app, vec![]);
        let moved = app.document.room_shape(room).unwrap().area();
        assert!((moved - area).abs() > 1.0, "{moved} {area}");
        // delete the selected room with the key
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Delete)]);
        assert_eq!(rooms(&app), 0);
    }

    #[test]
    fn roof_tool_draws_by_clicks_and_a_roof_in_a_plan_lies_under_the_walls_it_covers() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let level = match app.document.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let roofs = |a: &Desktop| a.document.elements().filter(|e| e.is_roof()).count();
        // a roof over the whole sample does not hide its walls from the pointer
        let covering = openbim_document::RoofSpec { outline: vec![[-1.0, -1.0], [9.0, -1.0], [9.0, 7.0], [-1.0, 7.0]], thickness_m: 0.2, base_offset_m: 3.0, slope_ratio: 0.5, edges: vec![], material: openbim_document::Material::Timber };
        app.document.place_roof(level, &covering).unwrap();
        frame(&ctx, screen, &mut app, vec![]);
        let (y, left, right) = wall_span(&ctx, screen, &app);
        assert!(right > left, "the wall is found under the roof: {left} {right}");
        // drawing: four clicks and Enter
        app.run_command("roof", "Roof");
        let pts = [Pos2::new(300.0, 300.0), Pos2::new(500.0, 300.0), Pos2::new(500.0, 450.0), Pos2::new(300.0, 450.0)];
        for at in pts {
            frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
            frame(&ctx, screen, &mut app, vec![button(at, true)]);
            frame(&ctx, screen, &mut app, vec![button(at, false)]);
        }
        assert_eq!(app.slab_points.len(), 4, "{}", app.message);
        assert_eq!(roofs(&app), 1);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        assert_eq!(roofs(&app), 2, "{}", app.message);
        let r = app.document.elements().filter(|e| e.is_roof()).last().unwrap();
        assert!((r.slope_ratio.unwrap() - 30f64.to_radians().tan()).abs() < 1e-9 && r.slope_edges == [true; 4]);
        // it is drawn (a frame with it selected runs), and Esc ends the tool
        let id = r.id;
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        app.select_element(Some(id));
        frame(&ctx, screen, &mut app, vec![]);
        assert!(app.tool == crate::Tool::Select || app.tool == crate::Tool::Slab);
        let _ = y;
    }

    #[test]
    fn stair_tool_draws_by_press_drag_release_and_shows_its_plan_symbol() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.run_command("stair", "Stair");
        let (a, b) = (Pos2::new(300.0, 200.0), Pos2::new(300.0, 500.0));
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(a)]);
        frame(&ctx, screen, &mut app, vec![button(a, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(b)]);
        frame(&ctx, screen, &mut app, vec![button(b, false)]);
        let stairs = |a: &Desktop| a.document.elements().filter(|e| e.category == Category::Stair).count();
        assert_eq!(stairs(&app), 1, "{}", app.message);
        let id = app.document.elements().find(|e| e.category == Category::Stair).unwrap().id;
        let Some(Geometry::Linear { start, end, .. }) = app.document.get(id).unwrap().geometry else { panic!() };
        // the run goes the way it was dragged (down the screen is toward -y), and is as long as its treads
        assert!((start[0] - end[0]).abs() < 1e-9 && end[1] < start[1], "{start:?} {end:?}");
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select);
        app.select_element(Some(id));
        frame(&ctx, screen, &mut app, vec![]);
    }
    #[test]
    fn section_tool_draws_a_line_in_a_plan_and_its_view_and_the_four_elevations_show_the_building() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.run_command("section", "Section");
        assert!(app.tool == crate::Tool::Section, "{}", app.message);
        for at in [Pos2::new(200.0, 300.0), Pos2::new(500.0, 300.0)] {
            frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
            frame(&ctx, screen, &mut app, vec![button(at, true)]);
            frame(&ctx, screen, &mut app, vec![button(at, false)]);
        }
        let lines: Vec<ElementId> = app.document.elements().filter(|e| e.category == Category::Section).map(|e| e.id).collect();
        assert_eq!(lines.len(), 1, "{}", app.message);
        assert!(app.message.contains("placed"), "{}", app.message);
        // its view shows the cut walls and the levels
        let view = crate::section_ui::build(&app.document, View::Section(lines[0].0)).expect("the section builds");
        assert!(!view.items.is_empty() && !view.labels.is_empty());
        // each elevation builds and has the levels drawn in it
        for k in 0..4u8 {
            let v = crate::section_ui::build(&app.document, View::Elevation(k)).unwrap_or_else(|e| panic!("elevation {k}: {e}"));
            assert!(!v.items.is_empty() && v.labels.len() >= 2, "elevation {k}");
        }
        // they and the section are drawn without panicking, and the line is also drawn back in the plan
        for view in [View::Elevation(0), View::Elevation(2), View::Section(lines[0].0)] {
            app.view = view;
            frame(&ctx, screen, &mut app, vec![]);
        }
        app.view = View::Plan;
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        frame(&ctx, screen, &mut app, vec![]);
        assert!(app.tool == crate::Tool::Select);
        // the section is not part of the model's geometry: it is removable and does not enter the export
        assert!(crate::export::scene_for_view(&app.document).is_ok());
    }
    #[test]
    fn wall_opening_is_sketched_on_a_wall_in_an_elevation_and_openings_and_walls_are_picked_there() {
        use crate::section_ui::{build, opening_rects, pick_at, wall_frame};
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        // an elevation that looks square on at a wall of the sample
        let walls: Vec<ElementId> = app.document.elements().filter(|e| e.category == Category::Wall).map(|e| e.id).collect();
        let (k, wall, view) = (0..4u8).find_map(|k| {
            let v = build(&app.document, View::Elevation(k)).ok()?;
            walls.iter().copied().find(|w| wall_frame(&app.document, &v, *w).is_some()).map(|w| (k, w, v))
        }).expect("a wall faces one of the four elevations");
        let f = wall_frame(&app.document, &view, wall).unwrap();
        assert!(f.height >= 2.3 && f.len > 2.0, "{f:?}");
        // a place on the wall where a 1 m by 1.2 m opening is free of anything else
        let (z0, z1) = (f.base + 0.9, f.base + 2.1);
        let umid = [0.15, 0.3, 0.5, 0.7, 0.85].iter().map(|t| f.u0 + f.sign * f.len * t)
            .find(|&u| [(u - 0.5, z0), (u + 0.5, z1), (u, (z0 + z1) / 2.0)].iter().all(|&(a, b)| pick_at(&app.document, &view, [a, b]) == Some(wall)))
            .expect("a free place on the wall");
        // the wall is picked under the pointer
        assert_eq!(pick_at(&app.document, &view, [umid, (z0 + z1) / 2.0]), Some(wall));
        let before = app.document.wall_openings().len();
        app.run_command("opening_wall", "Wall");
        assert!(app.tool == crate::Tool::WallOpening);
        app.view_opening_click(&view, [umid - 0.5, z0]);
        assert!(app.view_corner.is_some(), "{}", app.message);
        app.view_opening_click(&view, [umid + 0.5, z1]);
        assert_eq!(app.document.wall_openings().len(), before + 1, "{}", app.message);
        // the opening comes back in the same place in the view, and is what is picked there
        let view = build(&app.document, View::Elevation(k)).unwrap();
        let (id, host, r) = opening_rects(&app.document, &view).into_iter().max_by_key(|(id, _, _)| id.0).unwrap();
        assert_eq!(host, wall);
        assert!((r[0] - (umid - 0.5)).abs() < 1e-6 && (r[2] - (umid + 0.5)).abs() < 1e-6 && (r[1] - z0).abs() < 1e-6 && (r[3] - z1).abs() < 1e-6, "{r:?}");
        assert_eq!(pick_at(&app.document, &view, [umid, (z0 + z1) / 2.0]), Some(id));
        // the opening is a hole in the wall
        let hole = view.items.iter().any(|i| i.element.kind == openbim_view::Kind::Wall && i.element.id == wall.0 && matches!(&i.shape, openbim_view::Shape::Region { holes, .. } if !holes.is_empty()));
        assert!(hole, "the wall has no hole for the opening");
        // a click in the drawn view selects what is under the pointer with the Select tool
        app.choose_tool(crate::Tool::Select);
        app.view = View::Elevation(k);
        frame(&ctx, screen, &mut app, vec![]);
        let xf = app.section_xf.expect("the view was drawn");
        for (point, want) in [([umid, (z0 + z1) / 2.0], id), ([umid + 0.8, (z0 + z1) / 2.0], wall)] {
            let at = xf.screen(point);
            frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
            frame(&ctx, screen, &mut app, vec![button(at, true)]);
            frame(&ctx, screen, &mut app, vec![button(at, false)]);
            assert_eq!(app.selected_id(), Some(want), "{point:?}: {}", app.message);
        }
        // Delete removes the selected wall with its opening; the view still draws
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Delete)]);
        assert!(app.document.get(wall).is_none() && app.document.wall_openings().len() == before, "{}", app.message);
        frame(&ctx, screen, &mut app, vec![]);
    }
    #[test]
    fn dragging_in_an_elevation_slides_an_opening_along_its_wall_and_up_and_esc_cancels() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Elevation(2);
        let op = app.document.place_wall_opening(ElementId(20), 1.0, 2.0, 0.9, 2.1).unwrap();
        frame(&ctx, screen, &mut app, vec![]);
        let xf = app.section_xf.expect("the view was drawn");
        let view = crate::section_ui::build(&app.document, app.view).expect("builds");
        let rect = crate::section_ui::opening_rects(&app.document, &view).into_iter().find(|r| r.0 == op).expect("the opening is seen").2;
        let centre = [(rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0];
        let (press, to) = (xf.screen(centre), xf.screen([centre[0] + 1.0, centre[1] + 0.3]));
        let along = |a: &Desktop| match a.document.get(op).unwrap().geometry { Some(Geometry::InWall { along_m, .. }) => along_m, g => panic!("{g:?}") };
        let (a0, sill0) = (along(&app), app.document.get(op).unwrap().elevation_m);
        let sign = (view.section.b[0] - view.section.a[0]).signum();
        // Esc in the middle of a drag leaves it where it was
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press)]);
        frame(&ctx, screen, &mut app, vec![button(press, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press + Vec2::new(20.0, -5.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(to)]);
        assert!(app.view_drag.is_some() && app.selected_id() == Some(op), "{}", app.message);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        frame(&ctx, screen, &mut app, vec![button(to, false)]);
        assert_eq!(along(&app), a0);
        // a drag of one metre along and 300 mm up
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press)]);
        frame(&ctx, screen, &mut app, vec![button(press, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press + Vec2::new(20.0, -5.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(to)]);
        frame(&ctx, screen, &mut app, vec![button(to, false)]);
        assert!((along(&app) - a0 - sign).abs() < 1e-6, "{} -> {} ({sign}) {}", a0, along(&app), app.message);
        assert!((app.document.get(op).unwrap().elevation_m - sill0 - 0.3).abs() < 1e-6, "{}", app.message);
        assert!(app.view_drag.is_none());
        app.document.undo().unwrap();
        assert_eq!(along(&app), a0);
        // pressing on the wall itself and dragging up raises the wall, and the opening in it goes with it
        let f = crate::section_ui::wall_frame(&app.document, &view, ElementId(20)).expect("the wall is parallel to the view");
        let spot = [f.u0 + f.sign * 6.0, f.base + 1.5];
        let (press, to) = (xf.screen(spot), xf.screen([spot[0], spot[1] + 0.5]));
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press)]);
        frame(&ctx, screen, &mut app, vec![button(press, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press + Vec2::new(0.0, -10.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(to)]);
        frame(&ctx, screen, &mut app, vec![button(to, false)]);
        assert!((app.document.get(ElementId(20)).unwrap().elevation_m - 0.5).abs() < 1e-6, "{}", app.message);
        assert_eq!((along(&app), app.document.get(op).unwrap().elevation_m), (a0, sill0), "the opening keeps its place in the wall");
    }

    /// A view that looks north from the south of the sample, with a window-sized opening in wall 20, and where to click.
    fn south_view_with_openings(app: &mut Desktop) -> (ElementId, ElementId) {
        app.view = View::Elevation(2);
        let a = app.document.place_wall_opening(ElementId(20), 1.0, 2.0, 0.9, 2.1).unwrap();
        let b = app.document.place_wall_opening(ElementId(20), 4.0, 5.0, 0.5, 1.5).unwrap();
        (a, b)
    }
    fn spot(app: &Desktop, id: ElementId, fx: f64, fy: f64) -> Pos2 {
        let xf = app.section_xf.expect("drawn");
        let view = crate::section_ui::build(&app.document, app.view).expect("builds");
        let r = crate::section_ui::opening_rects(&app.document, &view).into_iter().find(|r| r.0 == id).expect("seen").2;
        xf.screen([r[0] + (r[2] - r[0]) * fx, r[1] + (r[3] - r[1]) * fy])
    }
    fn click(ctx: &egui::Context, screen: Rect, app: &mut Desktop, at: Pos2) {
        frame(ctx, screen, app, vec![egui::Event::PointerMoved(at)]);
        frame(ctx, screen, app, vec![button(at, true)]);
        frame(ctx, screen, app, vec![button(at, false)]);
    }
    fn sill(app: &Desktop, id: ElementId) -> f64 { app.document.get(id).unwrap().elevation_m }
    fn along_of(app: &Desktop, id: ElementId) -> f64 { match app.document.get(id).unwrap().geometry { Some(Geometry::InWall { along_m, .. }) => along_m, g => panic!("{g:?}") } }

    #[test]
    fn move_in_a_view_takes_a_base_point_and_a_typed_distance_and_copy_keeps_the_original() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        let (a, _) = south_view_with_openings(&mut app);
        frame(&ctx, screen, &mut app, vec![]);
        app.select_element(None);
        app.run_command("move", "Move");
        assert!(app.tool == crate::Tool::Move);
        // nothing selected: the first click picks the element, the second the base point, then 500 mm and Enter
        let (c0, a0, s0) = (spot(&app, a, 0.5, 0.5), along_of(&app, a), sill(&app, a));
        click(&ctx, screen, &mut app, c0);
        assert_eq!(app.selected_id(), Some(a), "{}", app.message);
        click(&ctx, screen, &mut app, c0);
        assert!(app.view_edit.as_ref().is_some_and(|e| e.pts.len() == 1), "{}", app.message);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(c0 + Vec2::new(150.0, 0.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::Text("500".into())]);
        assert_eq!(app.edit_num, "500");
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        let (da, dz) = (along_of(&app, a) - a0, sill(&app, a) - s0);
        assert!(da > 0.3 && (da.hypot(dz) - 0.5).abs() < 1e-6, "moved {da} along and {dz} up: {}", app.message);
        assert!(app.tool == crate::Tool::Select, "Move ends after one move");
        app.document.undo().unwrap();
        // Copy: the base point, then a second point; the original stays and the tool goes on
        app.view = View::Elevation(2);
        frame(&ctx, screen, &mut app, vec![]);
        app.select_element(Some(a));
        app.run_command("copy", "Copy");
        let c0 = spot(&app, a, 0.5, 0.5);
        click(&ctx, screen, &mut app, c0);
        let count = app.document.wall_openings().len();
        click(&ctx, screen, &mut app, c0 + Vec2::new(200.0, 0.0));
        assert_eq!(app.document.wall_openings().len(), count + 1, "{}", app.message);
        assert_eq!(along_of(&app, a), a0);
        assert!(app.tool == crate::Tool::Copy);
        // Esc once drops the base point, again ends the tool
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select);
    }

    #[test]
    fn mirror_rotate_and_align_in_a_view() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        let (a, b) = south_view_with_openings(&mut app);
        frame(&ctx, screen, &mut app, vec![]);
        // Mirror: a vertical axis by two points; the wall is 8 m, so the axis at 4.5 m turns the opening at 1.5 into 7.5
        let view = crate::section_ui::build(&app.document, app.view).expect("builds");
        let f = crate::section_ui::wall_frame(&app.document, &view, ElementId(20)).expect("parallel");
        let xf = app.section_xf.unwrap();
        let axis = |z: f64| xf.screen([f.u0 + f.sign * 4.5, z]);
        app.select_element(Some(a));
        app.run_command("mirror", "Mirror");
        assert!(app.mirror_copy, "Copy is on by default, as in Revit");
        click(&ctx, screen, &mut app, axis(0.5));
        // a slanted axis is refused with the reason
        click(&ctx, screen, &mut app, xf.screen([f.u0 + f.sign * 6.5, 2.5]));
        assert!(app.message.contains("vertical"), "{}", app.message);
        click(&ctx, screen, &mut app, axis(2.6));
        let mirrored: Vec<f64> = app.document.wall_openings().iter().map(|o| o.2[0] + o.2[1] / 2.0).collect();
        assert!(mirrored.iter().any(|m| (m - 7.5).abs() < 1e-6) && mirrored.iter().any(|m| (m - 1.5).abs() < 1e-6), "{mirrored:?} {}", app.message);
        app.document.undo().unwrap();
        // Rotate: a wall does not turn in an elevation (Revit's own rule), and a beam tilts about its middle by a typed angle
        app.select_element(Some(ElementId(20)));
        app.run_command("rotate", "Rotate");
        let p = spot(&app, a, 0.5, 0.5);
        click(&ctx, screen, &mut app, p);
        frame(&ctx, screen, &mut app, vec![egui::Event::Text("30".into())]);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        assert!(app.message.contains("walls do not rotate"), "{}", app.message);
        let level = app.plan_level();
        let beam = app.document.place_beam(level, &openbim_document::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.2, 0.4, 2.5)).unwrap();
        app.select_element(Some(beam));
        app.run_command("rotate", "Rotate");
        click(&ctx, screen, &mut app, p);
        frame(&ctx, screen, &mut app, vec![egui::Event::Text("30".into())]);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        let slope = app.document.get(beam).unwrap().slope_ratio.unwrap();
        assert!((slope - 30f64.to_radians().tan()).abs() < 1e-9, "{slope} {}", app.message);
        assert!((app.document.get(beam).unwrap().elevation_m + slope * 2.0 - 2.5).abs() < 1e-9, "the middle stays");
        // Align: the top of the second opening (1.5 m) onto the top of the first (2.1 m): it rises 0.6 m, and the reference stays
        app.select_element(None);
        app.run_command("align", "Align");
        let pa = spot(&app, a, 0.5, 1.0);
        click(&ctx, screen, &mut app, pa);
        assert!(app.view_edit.as_ref().is_some_and(|e| e.edge.is_some()), "{}", app.message);
        let pb = spot(&app, b, 0.5, 1.0);
        click(&ctx, screen, &mut app, pb);
        assert!((sill(&app, b) - 1.1).abs() < 1e-6, "{} {}", sill(&app, b), app.message);
        assert!((sill(&app, a) - 0.9).abs() < 1e-9 && app.tool == crate::Tool::Align);
        app.document.undo().unwrap();
        assert!((sill(&app, b) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn the_elevation_tool_puts_a_marker_with_views_that_switch_on_and_off_open_and_move() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.run_command("elevation", "Elevation");
        assert!(app.tool == crate::Tool::Elevation && app.view == View::Plan, "{}", app.message);
        let at = Pos2::new(420.0, 400.0);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, screen, &mut app, vec![button(at, true)]);
        frame(&ctx, screen, &mut app, vec![button(at, false)]);
        let marks: Vec<ElementId> = app.document.elements().filter(|e| e.category == Category::Elevation).map(|e| e.id).collect();
        assert_eq!(marks.len(), 1, "{}", app.message);
        let m = marks[0];
        assert_eq!(app.document.get(m).unwrap().count, 0b1111);
        assert!(app.tool == crate::Tool::Elevation, "the tool stays for the next marker");
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select);
        // it is drawn in the plan, and each of its views is a view of the building seen from its place
        frame(&ctx, screen, &mut app, vec![]);
        let Some(Geometry::Point { at: place, .. }) = app.document.get(m).unwrap().geometry else { panic!() };
        for k in 0..4u8 {
            let v = crate::section_ui::build(&app.document, View::Marked(m.0, k)).unwrap_or_else(|e| panic!("view {k}: {e}"));
            // the line of the view stands across the way it looks, through the marker
            let f = crate::section_ui::facing_vector(usize::from(k));
            let mid = [(v.section.a[0] + v.section.b[0]) / 2.0, (v.section.a[1] + v.section.b[1]) / 2.0];
            assert!((mid[0] - place[0]).abs() < 1e-9 && (mid[1] - place[1]).abs() < 1e-9);
            let d = [v.section.b[0] - v.section.a[0], v.section.b[1] - v.section.a[1]];
            assert!((-d[1] * f[0] + d[0] * f[1]).signum() > 0.0 && (d[0] * f[0] + d[1] * f[1]).abs() < 1e-9, "view {k} looks the wrong way");
            app.view = View::Marked(m.0, k);
            frame(&ctx, screen, &mut app, vec![]);
        }
        assert_eq!(crate::section_ui::name_of(&app, View::Marked(m.0, 2)), "Elevation 1 - South");
        // a view is switched off and then it is gone, a marker keeps one at least, undo brings a view back
        app.view = View::Plan;
        app.toggle_marker_view(m, 0);
        assert_eq!(app.document.get(m).unwrap().count, 0b1110);
        assert!(crate::section_ui::build(&app.document, View::Marked(m.0, 0)).is_err());
        for k in 1..3 { app.toggle_marker_view(m, k); }
        app.toggle_marker_view(m, 3);
        assert_eq!(app.document.get(m).unwrap().count, 0b1000, "{}", app.message);
        assert!(app.message.contains("at least one"), "{}", app.message);
        app.document.undo().unwrap();
        assert_eq!(app.document.get(m).unwrap().count, 0b1100);
        // pressing on the circle and dragging moves the marker; undo puts it back
        app.select_element(None);
        frame(&ctx, screen, &mut app, vec![]);
        let to = at + Vec2::new(90.0, 0.0);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, screen, &mut app, vec![button(at, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at + Vec2::new(30.0, 0.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(to)]);
        assert!(matches!(app.drag, Some(crate::Drag::Move { id, .. }) if id == m), "{:?} {}", app.drag, app.message);
        frame(&ctx, screen, &mut app, vec![button(to, false)]);
        let Some(Geometry::Point { at: moved, .. }) = app.document.get(m).unwrap().geometry else { panic!() };
        assert!(moved[0] > place[0] + 0.5 && (moved[1] - place[1]).abs() < 0.2, "{place:?} -> {moved:?} {}", app.message);
        app.document.undo().unwrap();
        assert!(matches!(app.document.get(m).unwrap().geometry, Some(Geometry::Point { at, .. }) if at == place));
        // Delete takes it away, and a marker is not part of the exported building
        assert!(crate::export::scene_for_view(&app.document).is_ok());
        app.select_element(Some(m));
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Delete)]);
        assert!(app.document.get(m).is_none(), "{}", app.message);
    }

    #[test]
    fn plans_show_section_heads_and_elevation_markers_that_open_their_views_and_a_section_flips() {
        use crate::section_ui::{elevation_lines, view_at};
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        // four elevation lines, each looking at the building from outside (the middle of the building is to their left)
        let lines = elevation_lines(&app.document);
        assert_eq!(lines.len(), 4);
        // a section, then the plan draws its head and the markers
        let level = app.plan_level();
        let id = app.document.place_section(level, [0.0, 3.0], [8.0, 3.0], 20.0).unwrap();
        frame(&ctx, screen, &mut app, vec![]);
        assert!(app.elev_marks.as_ref().is_some_and(|m| m.1.len() == 4));
        // a double click near a marker or a section line opens its view
        let hits = [(View::Elevation(1), egui::pos2(100.0, 100.0), egui::pos2(100.0, 100.0)), (View::Section(id.0), egui::pos2(0.0, 0.0), egui::pos2(100.0, 0.0))];
        assert!(view_at(&hits, egui::pos2(104.0, 98.0), 12.0) == Some(View::Elevation(1)));
        assert!(view_at(&hits, egui::pos2(50.0, 5.0), 12.0) == Some(View::Section(id.0)));
        assert!(view_at(&hits, egui::pos2(50.0, 40.0), 12.0).is_none());
        // Space (or the Flip direction button) turns the section round: the same line the other way
        let Some(Geometry::Linear { start, end, .. }) = app.document.get(id).unwrap().geometry else { panic!() };
        app.select_element(Some(id));
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Space)]);
        let Some(Geometry::Linear { start: s2, end: e2, .. }) = app.document.get(id).unwrap().geometry else { panic!() };
        assert_eq!((s2, e2), (end, start), "{}", app.message);
        // it looks the other way now: its view is built from the swapped line, and undo restores it
        let v = crate::section_ui::build(&app.document, View::Section(id.0)).expect("builds");
        assert_eq!(v.section.a, end);
        app.document.undo().unwrap();
        let Some(Geometry::Linear { start: s3, .. }) = app.document.get(id).unwrap().geometry else { panic!() };
        assert_eq!(s3, start);
    }
    #[test]
    fn vertical_opening_picks_its_host_with_tab_cuts_only_that_one_and_its_symbol_is_picked_in_the_plan() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let level = match app.document.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        // a floor and a roof over the same ground on the level
        let floor = app.document.place_slab(level, &openbim_document::SlabSpec { outline: vec![[-1.0, -1.0], [9.0, -1.0], [9.0, 7.0], [-1.0, 7.0]], thickness_m: 0.2, top_offset_m: 0.0, kind: openbim_document::SlabKind::Floor, material: openbim_document::Material::Concrete }).unwrap();
        let roof = app.document.place_roof(level, &openbim_document::RoofSpec { outline: vec![[-1.0, -1.0], [9.0, -1.0], [9.0, 7.0], [-1.0, 7.0]], thickness_m: 0.2, base_offset_m: 3.0, slope_ratio: 0.3, edges: vec![true; 4], material: openbim_document::Material::Timber }).unwrap();
        let area = |a: &Desktop, id| a.document.cut_shapes(id).iter().map(|s| s.area()).sum::<f64>();
        app.run_command("opening_vertical", "Vertical");
        assert!(app.tool == crate::Tool::VerticalOpening && app.message.contains("Vertical Opening"), "{}", app.message);
        let click = |app: &mut Desktop, at: Pos2| {
            frame(&ctx, screen, app, vec![egui::Event::PointerMoved(at)]);
            frame(&ctx, screen, app, vec![button(at, true)]);
            frame(&ctx, screen, app, vec![button(at, false)]);
        };
        // the first click picks the floor; Tab takes the roof and Tab again the floor
        click(&mut app, Pos2::new(340.0, 340.0));
        assert_eq!(app.opening_host, Some(floor), "{}", app.message);
        assert!(app.message.contains("2 under the click"), "{}", app.message);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Tab)]);
        assert_eq!(app.opening_host, Some(roof), "{}", app.message);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Tab)]);
        assert_eq!(app.opening_host, Some(floor));
        // the corners, then Enter
        for at in [Pos2::new(300.0, 300.0), Pos2::new(380.0, 300.0), Pos2::new(380.0, 380.0), Pos2::new(300.0, 380.0)] { click(&mut app, at); }
        assert_eq!(app.slab_points.len(), 4, "{}", app.message);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        let openings: Vec<ElementId> = app.document.elements().filter(|e| openbim_document::Document::is_vertical_opening(e)).map(|e| e.id).collect();
        assert_eq!(openings.len(), 1, "{}", app.message);
        assert!(app.message.contains("Vertical opening") && app.message.contains("placed"), "{}", app.message);
        assert_eq!(app.document.get(openings[0]).unwrap().parent, Some(floor));
        // only the floor is cut
        assert!(area(&app, floor) < 80.0 - 0.1 && app.document.cuts_of(roof).is_empty(), "{}", area(&app, floor));
        // Esc forgets the host, Esc again leaves the tool
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::VerticalOpening && app.opening_host.is_none());
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select);
        // the symbol is picked on its middle, and Properties shows the host
        click(&mut app, Pos2::new(340.0, 340.0));
        assert_eq!(app.selected_id(), Some(openings[0]), "{}", app.message);
        // a second one in the roof: pick it with Tab; the roof is cut in its shape and the floor is as it was
        app.run_command("opening_vertical", "Vertical");
        click(&mut app, Pos2::new(500.0, 500.0));
        if app.opening_host != Some(roof) { frame(&ctx, screen, &mut app, vec![key(egui::Key::Tab)]); }
        assert_eq!(app.opening_host, Some(roof), "{}", app.message);
        for at in [Pos2::new(500.0, 500.0), Pos2::new(540.0, 500.0), Pos2::new(540.0, 540.0), Pos2::new(500.0, 540.0)] { click(&mut app, at); }
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        assert_eq!(app.document.cuts_of(roof).len(), 1, "{}", app.message);
        let before = app.document.mesh(roof).unwrap();
        assert!(before.faces.len() > 10);
        // undo takes the roof's opening away; the roof is whole again
        assert_eq!(app.document.undo(), Ok(true));
        assert!(app.document.cuts_of(roof).is_empty());
    }
    #[test]
    fn shaft_tool_sketches_an_outline_cuts_the_floor_above_and_shows_its_symbol_and_wall_opening_tool_picks_two_ends() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let level = match app.document.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        let upper = app.document.level_above(level).expect("the sample has a second level");
        // a floor on the level above, for the shaft to go through
        let floor = app.document.place_slab(upper, &openbim_document::SlabSpec { outline: vec![[-1.0, -1.0], [9.0, -1.0], [9.0, 7.0], [-1.0, 7.0]], thickness_m: 0.2, top_offset_m: 0.0, kind: openbim_document::SlabKind::Floor, material: openbim_document::Material::Concrete }).unwrap();
        let area = |a: &Desktop| a.document.cut_shapes(floor).iter().map(|s| s.area()).sum::<f64>();
        assert!((area(&app) - 80.0).abs() < 1e-9);
        // the Shaft button in the ribbon
        app.run_command("shaft", "Shaft");
        assert!(app.tool == crate::Tool::Shaft && app.message.contains("Shaft Opening"), "{}", app.message);
        for at in [Pos2::new(300.0, 300.0), Pos2::new(380.0, 300.0), Pos2::new(380.0, 380.0), Pos2::new(300.0, 380.0)] {
            frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
            frame(&ctx, screen, &mut app, vec![button(at, true)]);
            frame(&ctx, screen, &mut app, vec![button(at, false)]);
        }
        assert_eq!(app.slab_points.len(), 4, "{}", app.message);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        let shafts: Vec<ElementId> = app.document.elements().filter(|e| matches!(e.geometry, Some(Geometry::Shaft { .. }))).map(|e| e.id).collect();
        assert_eq!(shafts.len(), 1, "{}", app.message);
        assert!(app.message.contains("cuts 1 floor"), "{}", app.message);
        // it goes from the level it was drawn on up to the next one, and the floor there has a hole
        let Some(Geometry::Shaft { level: base, top, .. }) = app.document.get(shafts[0]).unwrap().geometry else { panic!() };
        assert_eq!((base, top), (level, upper));
        assert!(area(&app) < 80.0 - 0.1, "{}", area(&app));
        // its symbol can be picked in the plan: select the tool, press Esc, click on its middle
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select);
        let mid = Pos2::new(340.0, 340.0);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(mid)]);
        frame(&ctx, screen, &mut app, vec![button(mid, true)]);
        frame(&ctx, screen, &mut app, vec![button(mid, false)]);
        assert_eq!(app.selected_id(), Some(shafts[0]), "{}", app.message);
        // undo gives the floor back
        let undone = app.document.undo();
        assert_eq!(undone, Ok(true));
        assert!((area(&app) - 80.0).abs() < 1e-9);

        // Wall Opening: two clicks along the first wall
        let (y, left, right) = wall_span(&ctx, screen, &app);
        app.run_command("opening_wall", "Wall");
        assert!(app.tool == crate::Tool::WallOpening, "{}", app.message);
        let (a, b) = (Pos2::new(left + (right - left) * 0.3, y), Pos2::new(left + (right - left) * 0.6, y));
        for at in [a, b] {
            frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(at)]);
            frame(&ctx, screen, &mut app, vec![button(at, true)]);
            frame(&ctx, screen, &mut app, vec![button(at, false)]);
        }
        let openings: Vec<ElementId> = app.document.elements().filter(|e| matches!(e.geometry, Some(Geometry::InWall { .. }))).filter(|e| e.category == Category::Opening).map(|e| e.id).collect();
        assert_eq!(openings.len(), 1, "{}", app.message);
        let o = app.document.get(openings[0]).unwrap();
        assert!(o.width_m > 0.5, "{}", o.width_m);
        assert_eq!((o.elevation_m, o.height_m), (0.9, 1.2000000000000002));
    }


    #[test]
    fn tags_and_dimensions_are_placed_in_an_elevation_belong_to_it_follow_their_element_and_delete_with_undo() {
        use openbim_document::ViewRef;
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        let (a, _) = south_view_with_openings(&mut app);
        app.select_element(None);
        frame(&ctx, screen, &mut app, vec![]);
        let view_tags = |d: &Desktop| d.document.elements().filter(|e| matches!(e.geometry, Some(Geometry::ViewTag { .. }))).count();
        let plan_tags = |d: &Desktop| d.document.elements().filter(|e| matches!(e.geometry, Some(Geometry::Tag { .. }))).count();
        // Tag: a click on the opening tags it, in this view only
        app.run_command("tag", "Tag");
        assert!(app.tool == crate::Tool::Tag);
        let at = spot(&app, a, 0.5, 0.5);
        click(&ctx, screen, &mut app, at);
        assert_eq!((view_tags(&app), plan_tags(&app)), (1, 0), "{}", app.message);
        let tag = app.document.elements().find(|e| matches!(e.geometry, Some(Geometry::ViewTag { .. }))).unwrap().id;
        assert!(matches!(app.document.get(tag).unwrap().geometry, Some(Geometry::ViewTag { host, view: ViewRef::Elevation(2), .. }) if host == a));
        // a click on nothing says so and places nothing
        let nothing = app.section_xf.unwrap().screen([-50.0, -50.0]);
        click(&ctx, screen, &mut app, nothing);
        assert_eq!(view_tags(&app), 1);
        app.choose_tool(crate::Tool::Select);
        // Dimension: the two vertical edges of the opening, then where the line goes
        app.run_command("dimension", "Dimension");
        assert!(app.tool == crate::Tool::Dimension);
        frame(&ctx, screen, &mut app, vec![]);
        let (left, right, above) = (spot(&app, a, 0.0, 0.5), spot(&app, a, 1.0, 0.5), spot(&app, a, 0.5, -0.8));
        click(&ctx, screen, &mut app, left);
        assert_eq!(app.view_edit.as_ref().map(|e| e.pts.len()), Some(1), "{}", app.message);
        // a horizontal edge cannot be the second reference of a horizontal dimension
        let top = spot(&app, a, 0.5, 1.0);
        click(&ctx, screen, &mut app, top);
        assert_eq!(app.view_edit.as_ref().map(|e| e.pts.len()), Some(1), "{}", app.message);
        click(&ctx, screen, &mut app, right);
        click(&ctx, screen, &mut app, above);
        let dim = app.document.elements().find(|e| matches!(e.geometry, Some(Geometry::ViewDimension { .. }))).expect(&app.message).id;
        match app.document.get(dim).unwrap().geometry {
            Some(Geometry::ViewDimension { view: ViewRef::Elevation(2), a: p, b: q, .. }) => assert!((p[1] - q[1]).abs() < 1e-9 && ((p[0] - q[0]).abs() - 1.0).abs() < 1e-6, "{p:?} {q:?}"),
            g => panic!("{g:?}"),
        }
        // nothing of it is in the plan or in another view; another view still draws
        app.choose_tool(crate::Tool::Select);
        app.view = View::Elevation(0);
        frame(&ctx, screen, &mut app, vec![]);
        app.view = View::Elevation(2);
        frame(&ctx, screen, &mut app, vec![]);
        // a click on the tag selects it; dragging the opening takes the tag along with it
        let head = match app.document.get(tag).unwrap().geometry { Some(Geometry::ViewTag { at, .. }) => at, g => panic!("{g:?}") };
        let xf = app.section_xf.unwrap();
        click(&ctx, screen, &mut app, xf.screen(head));
        assert_eq!(app.selected_id(), Some(tag), "{}", app.message);
        app.select_element(Some(a));
        let (press, to) = (spot(&app, a, 0.5, 0.5), spot(&app, a, 0.5, 0.5) + Vec2::new(xf.scale * 0.5, 0.0));
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press)]);
        frame(&ctx, screen, &mut app, vec![button(press, true)]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(press + Vec2::new(10.0, 0.0))]);
        frame(&ctx, screen, &mut app, vec![egui::Event::PointerMoved(to)]);
        frame(&ctx, screen, &mut app, vec![button(to, false)]);
        let head2 = match app.document.get(tag).unwrap().geometry { Some(Geometry::ViewTag { at, .. }) => at, g => panic!("{g:?}") };
        assert!((head2[0] - head[0]).abs() > 0.2 && (head2[1] - head[1]).abs() < 1e-6, "{head:?} -> {head2:?} {}", app.message);
        app.document.undo().unwrap();
        // Delete takes a selected dimension away and undo brings it back
        app.select_element(Some(dim));
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Delete)]);
        assert!(app.document.get(dim).is_none(), "{}", app.message);
        app.document.undo().unwrap();
        assert!(app.document.get(dim).is_some());
        // taking the opening away takes its tag, and the dimension stays
        app.select_element(Some(a));
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Delete)]);
        assert!(app.document.get(tag).is_none() && app.document.get(dim).is_some());
    }

    #[test]
    fn the_railing_tool_sketches_a_path_by_clicks_and_enter_edits_in_properties_and_is_drawn_in_every_view() {
        let (ctx, screen) = frames();
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.select_element(None);
        app.run_command("railing", "Railing");
        assert!(app.tool == crate::Tool::Railing, "{}", app.message);
        assert_eq!((app.height_mm, app.width_mm, app.spacing_mm), (1100.0, 50.0, 150.0));
        frame(&ctx, screen, &mut app, vec![]);
        let railings = |a: &Desktop| a.document.elements().filter(|e| e.category == Category::Railing).count();
        // three points, Enter finishes; Backspace takes a point back and Esc clears a path
        for p in [Pos2::new(300.0, 200.0), Pos2::new(300.0, 400.0), Pos2::new(500.0, 400.0)] { click(&ctx, screen, &mut app, p); }
        assert_eq!(app.slab_points.len(), 3, "{}", app.message);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Backspace)]);
        assert_eq!(app.slab_points.len(), 2);
        click(&ctx, screen, &mut app, Pos2::new(500.0, 400.0));
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        assert_eq!(railings(&app), 1, "{}", app.message);
        assert!(app.slab_points.is_empty());
        let id = app.document.elements().find(|e| e.category == Category::Railing).unwrap().id;
        let el = app.document.get(id).unwrap();
        assert_eq!((el.outline.len(), el.height_m, el.width_m, el.spacing_m), (3, 1.1, 0.05, 0.15));
        // one point is not a railing
        click(&ctx, screen, &mut app, Pos2::new(700.0, 300.0));
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Enter)]);
        assert_eq!(railings(&app), 1);
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.slab_points.is_empty());
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.tool == crate::Tool::Select);
        // selected, its height, rail size and spacing are edited in Properties and undone
        app.select_element(Some(id));
        assert_eq!((app.height_mm, app.spacing_mm), (1100.0, 150.0));
        (app.height_mm, app.spacing_mm, app.elevation_mm) = (900.0, 100.0, 50.0);
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        let el = app.document.get(id).unwrap();
        assert_eq!((el.height_m, el.spacing_m, el.elevation_m), (0.9, 0.1, 0.05), "{}", app.message);
        app.document.undo().unwrap();
        assert_eq!(app.document.get(id).unwrap().height_m, 1.1);
        // the plan, the 3D view and an elevation that looks at it draw it, and its parts are in the section engine's view
        frame(&ctx, screen, &mut app, vec![]);
        assert!(crate::view3d::batch(&app.document).verts.len() >= 8 * 20);
        let railing_drawn_in = |view: View| crate::section_ui::build(&app.document, view).ok().is_some_and(|v| v.items.iter().any(|i| i.element.id == id.0 && i.element.kind == openbim_view::Kind::Railing));
        assert!((0..4u8).any(|k| railing_drawn_in(View::Elevation(k))), "an elevation shows the railing");
        // a section line across it cuts the rail, the posts and the balusters
        let sec = {
            let o = &app.document.get(id).unwrap().outline;
            app.document.place_section(ElementId(10), [o[0][0] - 1.0, (o[0][1] + o[1][1]) / 2.0 + 0.011], [o[2][0] + 1.0, (o[0][1] + o[1][1]) / 2.0 + 0.011], 5.0)
        };
        let sec = sec.expect("a section line");
        let v = crate::section_ui::build(&app.document, View::Section(sec.0)).unwrap();
        let cut = v.items.iter().filter(|i| i.element.id == id.0 && i.class == openbim_view::Class::Cut).count();
        assert!(cut >= 1, "the plane cuts the railing: {cut}");
        // Delete and undo
        app.select_element(Some(id));
        frame(&ctx, screen, &mut app, vec![key(egui::Key::Delete)]);
        assert_eq!(railings(&app), 0);
        app.document.undo().unwrap();
        assert_eq!(railings(&app), 1);
    }
}
