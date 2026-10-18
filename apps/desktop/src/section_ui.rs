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

/// The name of an elevation or section view.
pub fn name_of(app: &Desktop, view: View) -> String {
    match view {
        View::Elevation(k) => FACINGS.get(usize::from(k)).map_or("Elevation".into(), |f| format!("{} elevation", f.1)),
        View::Section(id) => app.document.get(ElementId(id)).map_or("Section".into(), |e| e.name.clone()),
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
        _ => Err("not an elevation or a section".into()),
    }
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
fn triangles(outer: &[[f64; 2]], holes: &[Vec<[f64; 2]>]) -> Vec<([f64; 2], [f64; 2], [f64; 2])> {
    let Some(m) = openbim_family::meshing::prism(outer, holes, 0.0, 1.0) else { return Vec::new() };
    m.tris.iter().filter(|t| t.iter().all(|&i| m.verts[i as usize][2] > 0.5)).map(|t| {
        let p = |i: u32| { let v = m.verts[i as usize]; [v[0], v[1]] };
        (p(t[0]), p(t[1]), p(t[2]))
    }).collect()
}

pub fn show(app: &mut Desktop, ui: &mut egui::Ui) {
    let name = name_of(app, app.view);
    let built = build(&app.document, app.view);
    ui.horizontal(|ui| {
        ui.small(format!("{name} — middle-drag pan / wheel zoom"));
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
        Err(e) => { painter.text(rect.center(), egui::Align2::CENTER_CENTER, e, egui::FontId::proportional(14.0), ui.visuals().weak_text_color()); return; }
    };
    let [u0, z0, u1, z1] = view.bounds();
    let (cw, ch) = ((u1 - u0).max(1.0), (z1 - z0).max(1.0));
    let fit = ((rect.width() - 140.0).max(10.0) / cw as f32).min((rect.height() - 90.0).max(10.0) / ch as f32);
    let scale = fit * app.zoom;
    let (cu, cz) = ((u0 + u1) / 2.0, (z0 + z1) / 2.0);
    let screen = |p: [f64; 2]| rect.center() + app.pan + Vec2::new((p[0] - cu) as f32 * scale, -(p[1] - cz) as f32 * scale);
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
}

impl Desktop {
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
