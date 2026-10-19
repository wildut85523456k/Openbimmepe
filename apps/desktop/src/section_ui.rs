//! Elevation and section views: the building seen from outside from its four sides, and cut along a section line
//! drawn in a plan. Drawn from the view engine's lines and fills (`openbim_view::elevation_view` / `section_view`).
use crate::{Desktop, View};
use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};
use openbim_document::{Category, ElementId, Geometry};
use openbim_view::{Class, Facing, Kind, SectionView, Shape};


/// The four building elevations in the order Revit lists them: East, North, South, West.
pub const FACINGS: [(Facing, &str); 4] = [(Facing::East, "East"), (Facing::North, "North"), (Facing::South, "South"), (Facing::West, "West")];

/// How far outside the model the line of an elevation stands, in metres.
const MARGIN: f64 = 2.0;
/// How deep a new section looks, in metres.
pub const DEFAULT_DEPTH_M: f64 = 20.0;
/// How wide a new elevation marker's views are, in metres.
pub const MARKER_WIDTH_M: f64 = 20.0;

/// The name of an elevation or section view.
pub fn name_of(app: &Desktop, view: View) -> String {
    match view {
        View::Elevation(k) => FACINGS.get(usize::from(k)).map_or("Elevation".into(), |f| format!("{} elevation", f.1)),
        View::Section(id) => app.document.get(ElementId(id)).map_or("Section".into(), |e| e.name.clone()),
        View::Marked(id, k) => format!("{} - {}", app.document.get(ElementId(id)).map_or("Elevation", |e| e.name.as_str()), FACINGS.get(usize::from(k)).map_or("", |f| f.1)),
        _ => String::new(),
    }
}

/// The view `view` shows, or why it cannot.
pub fn build(doc: &openbim_document::Document, view: View) -> Result<SectionView, String> {
    let (scene, _) = crate::export::scene_for_view(doc)?;
    let around = openbim_view::standard_elevations(&scene, MARGIN);
    match view {
        View::Elevation(k) => {
            let facing = FACINGS.get(usize::from(k)).ok_or("no such elevation")?.0;
            let sec = around.and_then(|v| v.into_iter().find(|f| f.0 == facing)).ok_or("the model has nothing to look at yet")?.1;
            openbim_view::elevation_view(&scene, sec).map_err(|e| format!("{e:?}"))
        }
        View::Section(id) => {
            let el = doc.get(ElementId(id)).filter(|e| e.category == Category::Section).ok_or("the section line is gone")?;
            let Some(Geometry::Linear { start, end, .. }) = el.geometry else { return Err("the section has no line".into()) };
            let (bottom, top) = around.and_then(|v| v.first().map(|f| (f.1.bottom, f.1.top))).unwrap_or((-1.0, 10.0));
            openbim_view::section_view(&scene, openbim_view::Section { a: start, b: end, bottom, top, depth: el.width_m }).map_err(|e| format!("{e:?}"))
        }
        View::Marked(id, k) => {
            let el = doc.get(ElementId(id)).filter(|e| e.category == Category::Elevation).ok_or("the elevation marker is gone")?;
            let Some(Geometry::Point { at, .. }) = el.geometry else { return Err("the marker has no place".into()) };
            let k = usize::from(k);
            if k >= 4 || el.count >> k & 1 == 0 { return Err("the marker has no such view".into()); }
            let (a, b) = marker_line(at, k, el.width_m);
            let (bottom, top) = around.and_then(|v| v.first().map(|f| (f.1.bottom, f.1.top))).unwrap_or((-1.0, 10.0));
            openbim_view::section_view(&scene, openbim_view::Section { a, b, bottom, top, depth: el.height_m }).map_err(|e| format!("{e:?}"))
        }
        _ => Err("not an elevation or a section".into()),
    }
}

/// The name of the view `k` of the marker called `marker`.
pub fn name_of_marker(marker: &str, k: u8) -> String { format!("{marker} - {}", FACINGS.get(usize::from(k)).map_or("", |f| f.1)) }

/// The direction an elevation looks, as a unit vector in the plan: East, North, South, West.
pub fn facing_vector(k: usize) -> [f64; 2] {
    match k { 0 => [1.0, 0.0], 1 => [0.0, 1.0], 2 => [0.0, -1.0], _ => [-1.0, 0.0] }
}

/// The line of the view `k` of a marker at `at`, `width` across: it looks to the left of a -> b, which is the way the pointer points.
pub fn marker_line(at: [f64; 2], k: usize, width: f64) -> ([f64; 2], [f64; 2]) {
    let f = facing_vector(k);
    let d = [f[1], -f[0]];
    let h = width / 2.0;
    ([at[0] - d[0] * h, at[1] - d[1] * h], [at[0] + d[0] * h, at[1] + d[1] * h])
}

fn ink(dark: bool, class: Class) -> (Color32, f32) {
    match (class, dark) {
        (Class::Cut, false) => (Color32::BLACK, 2.0),
        (Class::Cut, true) => (Color32::WHITE, 2.0),
        (Class::Datum, _) => (Color32::from_rgb(70, 130, 200), 1.0),
        (_, false) => (Color32::from_gray(60), 1.0),
        (_, true) => (Color32::from_gray(190), 1.0),
    }
}

fn fill(dark: bool, class: Class, kind: Kind) -> Color32 {
    match (class, kind, dark) {
        (Class::Cut, _, false) => Color32::from_gray(105),
        (Class::Cut, _, true) => Color32::from_gray(150),
        (_, Kind::Roof, false) => Color32::from_rgb(200, 150, 140),
        (_, Kind::Roof, true) => Color32::from_rgb(140, 90, 80),
        (_, Kind::Family, false) => Color32::from_rgb(190, 215, 190),
        (_, Kind::Family, true) => Color32::from_rgb(80, 110, 80),
        (_, Kind::Stair, false) => Color32::from_gray(210),
        (_, _, false) => Color32::from_gray(232),
        (_, _, true) => Color32::from_gray(72),
    }
}

/// The triangles that fill a region with holes.
pub(crate) fn triangles(outer: &[[f64; 2]], holes: &[Vec<[f64; 2]>]) -> Vec<([f64; 2], [f64; 2], [f64; 2])> {
    let Some(m) = openbim_family::meshing::prism(outer, holes, 0.0, 1.0) else { return Vec::new() };
    m.tris.iter().filter(|t| t.iter().all(|&i| m.verts[i as usize][2] > 0.5)).map(|t| {
        let p = |i: u32| { let v = m.verts[i as usize]; [v[0], v[1]] };
        (p(t[0]), p(t[1]), p(t[2]))
    }).collect()
}

/// How (u, z) of a view maps to the screen: `centre` is where (cu, cz) is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf { pub centre: Pos2, pub scale: f32, pub cu: f64, pub cz: f64 }

impl Xf {
    pub fn screen(&self, p: [f64; 2]) -> Pos2 { self.centre + Vec2::new((p[0] - self.cu) as f32 * self.scale, -(p[1] - self.cz) as f32 * self.scale) }
    pub fn model(&self, p: Pos2) -> [f64; 2] { [self.cu + f64::from((p.x - self.centre.x) / self.scale), self.cz - f64::from((p.y - self.centre.y) / self.scale)] }
}

/// Where a wall stands in a view that looks square on at it: the u of its start, whether u grows along the wall, its
/// length, the height of its base and its height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WallFrame { pub u0: f64, pub sign: f64, pub len: f64, pub base: f64, pub height: f64 }

/// The frame of a wall parallel to the view (within about a degree), else None: a wall seen end on or at an angle
/// has no place for an opening in this view.
pub fn wall_frame(doc: &openbim_document::Document, view: &SectionView, wall: ElementId) -> Option<WallFrame> {
    let el = doc.get(wall).filter(|e| e.category == Category::Wall)?;
    let Some(Geometry::Linear { start, end, level }) = el.geometry else { return None };
    let (a, b) = (view.section.a, view.section.b);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let line = dx.hypot(dy);
    let (wx, wy) = (end[0] - start[0], end[1] - start[1]);
    let len = wx.hypot(wy);
    if line < 1e-9 || len < 1e-6 { return None; }
    let (d, w) = ([dx / line, dy / line], [wx / len, wy / len]);
    if (w[0] * d[1] - w[1] * d[0]).abs() > 0.02 { return None; }
    let sign = if w[0] * d[0] + w[1] * d[1] >= 0.0 { 1.0 } else { -1.0 };
    let u0 = (start[0] - a[0]) * d[0] + (start[1] - a[1]) * d[1];
    let base = doc.get(level)?.elevation_m + el.elevation_m;
    Some(WallFrame { u0, sign, len, base, height: el.height_m })
}

/// The wall openings seen in the view, each as (opening, wall, [u0, z0, u1, z1]).
pub fn opening_rects(doc: &openbim_document::Document, view: &SectionView) -> Vec<(ElementId, ElementId, [f64; 4])> {
    doc.wall_openings().into_iter().filter_map(|(id, host, [off, width, sill, height])| {
        if !view.items.iter().any(|i| i.element.kind == Kind::Wall && i.element.id == host.0) { return None; }
        let f = wall_frame(doc, view, host)?;
        let (ua, ub) = (f.u0 + f.sign * off, f.u0 + f.sign * (off + width));
        Some((id, host, [ua.min(ub), f.base + sill, ua.max(ub), f.base + sill + height]))
    }).collect()
}

fn inside(ring: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut c = false;
    let mut j = ring.len().wrapping_sub(1);
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] { c = !c; }
        j = i;
    }
    c
}

/// The element under the point (u, z) of a view: a wall opening first (it is a hole in its wall), then the nearest
/// item drawn there. Level lines and anything not in the model are not picked.
pub fn pick_at(doc: &openbim_document::Document, view: &SectionView, p: [f64; 2]) -> Option<ElementId> {
    if let Some((id, _, _)) = opening_rects(doc, view).into_iter().find(|(_, _, r)| p[0] >= r[0] && p[0] <= r[2] && p[1] >= r[1] && p[1] <= r[3]) { return Some(id); }
    view.items.iter().rev().filter(|i| i.class != Class::Datum && i.element.kind != Kind::Level).find(|i| match &i.shape {
        Shape::Region { outer, holes } => inside(outer, p) && !holes.iter().any(|h| inside(h, p)),
        Shape::Fill(ring) => inside(ring, p),
        _ => false,
    }).map(|i| ElementId(i.element.id)).filter(|id| doc.get(*id).is_some())
}

/// The wall and the rectangle (u0, z0, u1, z1) under a wall opening sketched with two corners, as the wall, the
/// distances along the wall, and the sill and head heights above the wall's base.
pub fn opening_of_corners(doc: &openbim_document::Document, view: &SectionView, wall: ElementId, c0: [f64; 2], c1: [f64; 2]) -> Option<(f64, f64, f64, f64)> {
    let f = wall_frame(doc, view, wall)?;
    let along = |u: f64| (f.sign * (u - f.u0)).clamp(0.0, f.len);
    let (a, b) = (along(c0[0]), along(c1[0]));
    let (sill, head) = ((c0[1].min(c1[1]) - f.base).max(0.0), (c0[1].max(c1[1]) - f.base).min(f.height));
    Some((a.min(b), a.max(b), sill, head))
}

/// The lines the four elevations look along (East, North, South, West as `FACINGS` numbers them), each with the two
/// ends of its line: the building is to the left of a -> b. Empty when the model has nothing to look at.
pub fn elevation_lines(doc: &openbim_document::Document) -> Vec<(u8, [f64; 2], [f64; 2])> {
    let Ok((scene, _)) = crate::export::scene_for_view(doc) else { return Vec::new() };
    let Some(all) = openbim_view::standard_elevations(&scene, MARGIN) else { return Vec::new() };
    FACINGS.iter().enumerate().filter_map(|(k, f)| all.iter().find(|v| v.0 == f.0).map(|v| (k as u8, v.1.a, v.1.b))).collect()
}

/// The view a double click at `p` opens: the section line or the elevation marker within `reach` pixels of it.
pub fn view_at(hits: &[(View, Pos2, Pos2)], p: Pos2, reach: f32) -> Option<View> {
    hits.iter().rev().find(|(_, a, b)| {
        let ab = *b - *a;
        let t = ((p - *a).dot(ab) / ab.length_sq().max(1e-10)).clamp(0.0, 1.0);
        (p - (*a + t * ab)).length() <= reach
    }).map(|h| h.0)
}

pub fn show(app: &mut Desktop, ui: &mut egui::Ui) {
    let name = name_of(app, app.view);
    let seen = app.view_document();
    let built = build(seen.as_ref().unwrap_or(&app.document), app.view);
    ui.horizontal(|ui| {
        ui.small(format!("{name} — middle-drag pan / wheel zoom / click selects / drag moves"));
        if !matches!(app.tool, crate::Tool::Select | crate::Tool::WallOpening | crate::Tool::Move | crate::Tool::Copy | crate::Tool::Rotate | crate::Tool::Mirror | crate::Tool::Align | crate::Tool::Tag | crate::Tool::Dimension) { ui.small("— this tool is not available in an elevation or a section yet; press Esc"); }
        if let Ok(v) = &built {
            if let Some(w) = v.warnings.first() { ui.small(format!("— {w}")); }
        }
    });
    let (response, painter) = ui.allocate_painter(ui.available_size().max(Vec2::splat(50.0)), egui::Sense::click_and_drag());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    if response.dragged_by(egui::PointerButton::Middle) { app.pan += response.drag_delta(); }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        app.zoom = (app.zoom * (scroll * 0.001).exp()).clamp(0.2, 8.0);
    }
    let view = match built {
        Ok(v) => v,
        Err(e) => { app.section_xf = None; painter.text(rect.center(), egui::Align2::CENTER_CENTER, e, egui::FontId::proportional(14.0), ui.visuals().weak_text_color()); return; }
    };
    let [u0, z0, u1, z1] = view.bounds();
    let (cw, ch) = ((u1 - u0).max(1.0), (z1 - z0).max(1.0));
    let fit = ((rect.width() - 140.0).max(10.0) / cw as f32).min((rect.height() - 90.0).max(10.0) / ch as f32);
    let xf = Xf { centre: rect.center() + app.pan, scale: fit * app.zoom, cu: (u0 + u1) / 2.0, cz: (z0 + z1) / 2.0 };
    app.section_xf = Some(xf);
    let (scale, screen) = (xf.scale, |p: [f64; 2]| xf.screen(p));
    let dark = app.dark;
    // fills first, in the order the engine gives them (far to near), each followed by its lines
    for item in &view.items {
        let (colour, width) = ink(dark, item.class);
        match &item.shape {
            Shape::Region { outer, holes } => {
                let mut mesh = egui::epaint::Mesh::default();
                let c = fill(dark, item.class, item.element.kind);
                for (a, b, d) in triangles(outer, holes) {
                    let base = mesh.vertices.len() as u32;
                    for p in [a, b, d] { mesh.colored_vertex(screen(p), c); }
                    mesh.add_triangle(base, base + 1, base + 2);
                }
                painter.add(egui::Shape::mesh(mesh));
            }
            Shape::Fill(ring) => {
                let pts: Vec<Pos2> = ring.iter().map(|p| screen(*p)).collect();
                painter.add(egui::Shape::convex_polygon(pts, fill(dark, item.class, item.element.kind), Stroke::NONE));
            }
            Shape::Line(a, b) => {
                let (a, b) = (screen(*a), screen(*b));
                if item.class == Class::Datum {
                    let delta = b - a;
                    let length = delta.length();
                    let mut t = 0.0;
                    while t < length && t < 100_000.0 {
                        painter.line_segment([a + delta * (t / length), a + delta * ((t + 14.0).min(length) / length)], Stroke::new(width, colour));
                        t += 22.0;
                    }
                } else {
                    painter.line_segment([a, b], Stroke::new(width, colour));
                }
            }
            Shape::Arc { c, r, a0, a1 } => {
                let sweep = (a1 - a0).rem_euclid(360.0);
                let n = 24;
                let pts: Vec<Pos2> = (0..=n).map(|i| { let a = (a0 + sweep * f64::from(i) / f64::from(n)).to_radians(); screen([c[0] + r * a.cos(), c[1] + r * a.sin()]) }).collect();
                painter.add(egui::Shape::line(pts, Stroke::new(width, colour)));
            }
            Shape::Circle { c, r } => { painter.circle_stroke(screen(*c), (*r as f32 * scale).max(1.0), Stroke::new(width, colour)); }
        }
    }
    // level names and heights at the right end of their datum lines
    for l in &view.labels {
        let at = screen(l.at);
        painter.text(at + Vec2::new(8.0, 0.0), egui::Align2::LEFT_CENTER, format!("{}  {:+.3}", l.text, l.at[1]), egui::FontId::proportional(12.0), Color32::from_rgb(70, 130, 200));
    }
    // the selected element in the highlight colour
    let blue = ui.visuals().hyperlink_color;
    let chosen = app.selected_ids();
    let rects = opening_rects(&app.document, &view);
    for item in view.items.iter().filter(|i| i.class != Class::Datum && chosen.contains(&ElementId(i.element.id)) && i.element.kind != Kind::Level) {
        let ring = |r: &[[f64; 2]]| painter.add(egui::Shape::closed_line(r.iter().map(|p| screen(*p)).collect(), Stroke::new(2.5, blue)));
        match &item.shape {
            Shape::Region { outer, holes } => { ring(outer); for h in holes { ring(h); } }
            Shape::Fill(r) => { ring(r); }
            Shape::Line(a, b) => { painter.line_segment([screen(*a), screen(*b)], Stroke::new(2.5, blue)); }
            _ => {}
        }
    }
    for (_, _, r) in rects.iter().filter(|(id, _, _)| chosen.contains(id)) {
        painter.rect_stroke(egui::Rect::from_two_pos(screen([r[0], r[1]]), screen([r[2], r[3]])), 0.0, Stroke::new(2.5, blue), egui::StrokeKind::Middle);
    }
    // the tags and dimensions of this view
    let notes = crate::view_notes::view_ref(app.view).map(|v| crate::view_notes::draw(app, ui, &painter, &xf, v)).unwrap_or_default();
    // interaction: Select picks the element under the pointer; Wall Opening sketches a rectangle on a wall
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    match app.tool {
        crate::Tool::Select => {
            // press on an element and drag: it moves along the view and up or down (Ctrl: a copy), the pointer snapping to
            // what is drawn; a ghost shows where
            let edges = crate::view_tools::edges(&app.document, &view);
            let snap = |p: Pos2| crate::view_tools::snap(&edges, &xf, p, 10.0, None, false);
            if response.drag_started_by(egui::PointerButton::Primary) {
                if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                    if let Some(hit) = crate::view_notes::pick(&notes, p).or_else(|| pick_at(&app.document, &view, xf.model(p))) {
                        if !chosen.contains(&hit) { app.select_element(Some(hit)); }
                        app.view_drag = Some(snap(p).at);
                    }
                }
            }
            if let (Some(from), Some(h)) = (app.view_drag, response.interact_pointer_pos().or(response.hover_pos())) {
                let ids = app.selected_ids();
                let s = snap(h);
                let (du, dz) = (s.at[0] - from[0], s.at[1] - from[1]);
                let copy = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
                crate::view_tools::draw_ghost(&painter, &view, &app.document, &xf, &ids, du, dz);
                crate::view_tools::draw_snap(&painter, screen(s.at), s.kind, blue);
                painter.text(h + Vec2::new(12.0, -12.0), egui::Align2::LEFT_BOTTOM, format!("{}{:+.0} mm along, {:+.0} mm up", if copy { "copy: " } else { "" }, du * 1000.0, dz * 1000.0), egui::FontId::proportional(13.0), blue);
                if esc { app.view_drag = None; app.message = "Move cancelled".into(); }
                else if response.drag_stopped_by(egui::PointerButton::Primary) {
                    app.view_drag = None;
                    if let (Some(frame), true) = (openbim_document::ViewFrame::from_line(view.section.a, view.section.b), du != 0.0 || dz != 0.0) { app.view_move(frame, &ids, du, dz, copy); }
                }
            }
            if !ui.input(|i| i.pointer.primary_down()) { app.view_drag = None; }
            if response.clicked() {
                if let Some(h) = response.interact_pointer_pos() {
                    let hit = crate::view_notes::pick(&notes, h).or_else(|| pick_at(&app.document, &view, xf.model(h)));
                    app.select_element(hit);
                }
            }
        }
        crate::Tool::Tag | crate::Tool::Dimension => {
            let edges = crate::view_tools::edges(&app.document, &view);
            crate::view_notes::tool(app, ui, &painter, &response, &view, xf, &edges);
        }
        crate::Tool::Move | crate::Tool::Copy | crate::Tool::Rotate | crate::Tool::Mirror | crate::Tool::Align => {
            let edges = crate::view_tools::edges(&app.document, &view);
            crate::view_tools::modify(app, ui, &painter, &response, &view, xf, &edges);
        }
        crate::Tool::WallOpening => {
            // the corners snap to what is drawn, as everywhere else in a view
            let edges = crate::view_tools::edges(&app.document, &view);
            let snapped = |p: Pos2| crate::view_tools::snap(&edges, &xf, p, 10.0, None, false);
            if let Some(h) = response.hover_pos().map(snapped) { crate::view_tools::draw_snap(&painter, screen(h.at), h.kind, blue); }
            if let (Some((_, c)), Some(h)) = (app.view_corner, response.hover_pos().map(snapped)) {
                painter.rect_stroke(egui::Rect::from_two_pos(screen(c), screen(h.at)), 0.0, Stroke::new(1.5, blue), egui::StrokeKind::Middle);
            }
            if esc { if app.view_corner.is_some() { app.view_corner = None; } else { app.choose_tool(crate::Tool::Select); } }
            else if response.clicked() {
                if let Some(h) = response.interact_pointer_pos() { app.view_opening_click(&view, snapped(h).at); }
            }
        }
        _ => { if esc { app.choose_tool(crate::Tool::Select); } }
    }
}

impl Desktop {
    /// A click of the Wall Opening tool in an elevation or section: the first click is a corner on a wall, the second the
    /// opposite corner (on the same wall); the rectangle between them is the opening, as Revit sketches it in a view.
    pub(crate) fn view_opening_click(&mut self, view: &SectionView, at: [f64; 2]) {
        let Some((wall, c0)) = self.view_corner else {
            let hit = view.items.iter().rev().find(|i| i.element.kind == Kind::Wall && match &i.shape {
                Shape::Region { outer, holes } => inside(outer, at) && !holes.iter().any(|h| inside(h, at)),
                Shape::Fill(ring) => inside(ring, at),
                _ => false,
            });
            let Some(hit) = hit else { self.message = "Wall Opening: click a corner of the opening on a wall".into(); return };
            let wall = ElementId(hit.element.id);
            if wall_frame(&self.document, view, wall).is_none() { self.message = "Wall Opening: that wall is not parallel to this view; use a view that looks square on at it".into(); return; }
            self.view_corner = Some((wall, at));
            self.message = "Wall Opening: click the opposite corner of the opening".into();
            return;
        };
        self.view_corner = None;
        let Some((from, to, sill, head)) = opening_of_corners(&self.document, view, wall, c0, at) else { self.message = "Wall Opening: the wall is not parallel to this view".into(); return };
        match self.document.place_wall_opening(wall, from, to, sill, head) {
            Ok(id) => self.message = format!("Wall opening {} placed, {:.0} mm wide and {:.0} mm high; undo removes it", id.0, (to - from) * 1000.0, (head - sill) * 1000.0),
            Err(e) => self.message = format!("Wall opening rejected ({e:?}): it must be at least 50 mm each way and inside the wall"),
        }
    }

    /// Turn the view of the selected section line round: it looks to the other side of the line.
    pub(crate) fn flip_selected_section(&mut self) -> bool {
        let Some(id) = self.selected_id() else { return false };
        let Some(el) = self.document.get(id).filter(|e| e.category == Category::Section) else { return false };
        let Some(Geometry::Linear { start, end, level }) = el.geometry else { return false };
        match self.document.transact(self.document.revision(), &[openbim_document::Command::SetGeometry { id, geometry: Geometry::Linear { start: end, end: start, level } }]) {
            Ok(_) => self.message = "Section flipped: it looks to the other side of the line; undo takes it back".into(),
            Err(e) => self.message = format!("Cannot flip: {e:?}"),
        }
        true
    }

    /// A click of the Elevation tool: a marker with its four views at the point.
    pub(crate) fn elevation_click(&mut self, level: ElementId, point: [f64; 2]) {
        match self.document.place_elevation(level, point, 0b1111, MARKER_WIDTH_M, DEFAULT_DEPTH_M) {
            Ok(id) => {
                let name = self.document.get(id).map_or_else(String::new, |e| e.name.clone());
                self.message = format!("{name} placed with its four views; select it to switch views on or off, double click a pointer to open one; undo removes it");
            }
            Err(e) => self.message = format!("Elevation rejected ({e:?})"),
        }
    }

    /// Switch the view `k` of the selected marker on or off. A marker keeps at least one view.
    pub(crate) fn toggle_marker_view(&mut self, id: ElementId, k: usize) {
        let Some(el) = self.document.get(id).filter(|e| e.category == Category::Elevation) else { return };
        let count = el.count ^ (1 << k);
        if count == 0 { self.message = "An elevation marker keeps at least one view; delete the marker to remove it".into(); return; }
        match self.document.transact(self.document.revision(), &[openbim_document::Command::SetCount { id, count }]) {
            Ok(_) => self.message = format!("{} view {}; undo takes it back", FACINGS[k].1, if count >> k & 1 == 1 { "added" } else { "removed" }),
            Err(e) => self.message = format!("Cannot do that: {e:?}"),
        }
    }

    /// A click of the Section tool: one end of the line, then the other. The view looks to the left of the line.
    pub(crate) fn section_click(&mut self, level: ElementId, point: [f64; 2]) {
        let Some(a) = self.edit_pts.first().copied() else {
            self.edit_pts.push(point);
            self.message = "Section: pick the other end; the view looks to the left of the line".into();
            return;
        };
        if (point[0] - a[0]).hypot(point[1] - a[1]) < 0.1 { self.message = "Section: the line is too short".into(); return; }
        match self.document.place_section(level, a, point, DEFAULT_DEPTH_M) {
            Ok(id) => {
                self.edit_pts.clear();
                let name = self.document.get(id).map_or_else(String::new, |e| e.name.clone());
                self.message = format!("{name} placed; open it from the Project Browser or Properties. It looks to the left of the line; undo removes it");
            }
            Err(e) => self.message = format!("Section rejected ({e:?})"),
        }
    }
}
