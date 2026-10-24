//! The egui widget: Revit mouse mapping, wheel zoom to the cursor, the ViewCube, and the paint callback that draws
//! the batch with wgpu.

use crate::gpu::{Gpu, Scene, Target};
use egui::{Align2, Color32, FontId, PointerButton, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use egui_wgpu::{wgpu, CallbackResources, CallbackTrait, RenderState, ScreenDescriptor};
use openbim_view::{Batch, Element};
use openbim_viewport::{pick, Buttons, Camera, CubeDir, Hit, Mods, SectionBox, Style, Sun};
use std::collections::HashSet;
use std::sync::Arc;

/// Names of the faces of the ViewCube.
#[derive(Debug, Clone)]
pub struct CubeLabels { pub top: String, pub bottom: String, pub front: String, pub back: String, pub right: String, pub left: String }

impl CubeLabels {
    pub fn en() -> CubeLabels { CubeLabels { top: "TOP".into(), bottom: "BOTTOM".into(), front: "FRONT".into(), back: "BACK".into(), right: "RIGHT".into(), left: "LEFT".into() } }
    pub fn fr() -> CubeLabels { CubeLabels { top: "DESSUS".into(), bottom: "DESSOUS".into(), front: "AVANT".into(), back: "ARRIÈRE".into(), right: "DROITE".into(), left: "GAUCHE".into() } }
}

/// One of the 26 places of the ViewCube as drawn now: a quadrilateral on a visible face.
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub dir: CubeDir,
    pub quad: [Pos2; 4],
    /// the middle cell of a face, which carries its name
    pub is_face: bool,
}

/// The cells of the cube facing the camera, for a cube whose centre is at `centre` on the screen and whose half edge
/// is `half` pixels.
pub fn cube_cells(cam: &Camera, centre: Pos2, half: f32) -> Vec<Cell> {
    let (r, u) = (cam.right(), cam.up());
    let e = cam.eye();
    let e = [e[0] - cam.target[0], e[1] - cam.target[1], e[2] - cam.target[2]];
    let proj = |p: [f64; 3]| Pos2::new(centre.x + half * (p[0] * r[0] + p[1] * r[1] + p[2] * r[2]) as f32, centre.y - half * (p[0] * u[0] + p[1] * u[1] + p[2] * u[2]) as f32);
    const BAND: [(f64, f64); 3] = [(-1.0, -1.0 / 3.0), (-1.0 / 3.0, 1.0 / 3.0), (1.0 / 3.0, 1.0)];
    let mut out = Vec::new();
    for a in 0..3 {
        for s in [-1.0f64, 1.0] {
            if s * e[a] <= 0.0 { continue; }
            let (b, c) = ((a + 1) % 3, (a + 2) % 3);
            for (ib, ub) in BAND.iter().enumerate() {
                for (ic, uc) in BAND.iter().enumerate() {
                    let corner = |u: f64, w: f64| { let mut p = [0.0; 3]; p[a] = s; p[b] = u; p[c] = w; p };
                    let quad = [proj(corner(ub.0, uc.0)), proj(corner(ub.1, uc.0)), proj(corner(ub.1, uc.1)), proj(corner(ub.0, uc.1))];
                    let mut d = [0i8; 3];
                    d[a] = s as i8;
                    d[b] = ib as i8 - 1;
                    d[c] = ic as i8 - 1;
                    out.push(Cell { dir: CubeDir(d), quad, is_face: ib == 1 && ic == 1 });
                }
            }
        }
    }
    out
}

/// Is the point inside the (convex) quadrilateral?
pub fn in_quad(q: &[Pos2; 4], p: Pos2) -> bool {
    let mut sign = 0.0f32;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        let cr = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        if cr != 0.0 {
            if sign != 0.0 && (cr > 0.0) != (sign > 0.0) { return false; }
            sign = cr;
        }
    }
    true
}

/// The cell under a point, if any.
pub fn pick_cell(cells: &[Cell], p: Pos2) -> Option<CubeDir> { cells.iter().find(|c| in_quad(&c.quad, p)).map(|c| c.dir) }

fn face_name(l: &CubeLabels, d: CubeDir) -> &str {
    match d.0 { [0, 0, 1] => &l.top, [0, 0, -1] => &l.bottom, [0, -1, 0] => &l.front, [0, 1, 0] => &l.back, [1, 0, 0] => &l.right, _ => &l.left }
}

/// What the callback needs to keep between frames: the pipelines and the uploaded scene.
struct Resources { gpu: Gpu, scene: Option<(u64, Style, u64, Scene)>, target: Option<Target>, failed: Option<(u64, Style)> }

struct Draw { budget: Option<u64>, problem: Arc<std::sync::Mutex<Option<String>>>, sun: Sun, camera: Camera, style: Style, generation: u64, batch: Option<Arc<Batch>>, size_px: [u32; 2], section: Option<SectionBox>, selection: Arc<HashSet<Element>>, selection_generation: u64 }

impl Draw {
    fn say(&self, why: Option<String>) { if let Ok(mut p) = self.problem.lock() { *p = why; } }
}

impl CallbackTrait for Draw {
    fn prepare(&self, device: &wgpu::Device, queue: &wgpu::Queue, _screen: &ScreenDescriptor, encoder: &mut wgpu::CommandEncoder, res: &mut CallbackResources) -> Vec<wgpu::CommandBuffer> {
        let Some(r) = res.get_mut::<Resources>() else { return Vec::new() };
        let Some(batch) = &self.batch else { r.scene = None; return Vec::new() };
        if !matches!(&r.scene, Some((g, s, _, _)) if *g == self.generation && *s == self.style) && r.failed != Some((self.generation, self.style)) {
            match r.gpu.try_upload(device, batch, self.style, self.budget) {
                Ok(scene) => { r.scene = Some((self.generation, self.style, u64::MAX, scene)); r.failed = None; self.say(None); }
                Err(why) => { r.scene = None; r.failed = Some((self.generation, self.style)); self.say(Some(why)); }
            }
        }
        if let Some((_, _, sg, scene)) = &mut r.scene {
            scene.sun = self.sun;
            if *sg != self.selection_generation {
                r.gpu.select(device, scene, batch, &self.selection);
                *sg = self.selection_generation;
            }
        }
        if !matches!(&r.target, Some(t) if t.size == [self.size_px[0].max(1), self.size_px[1].max(1)]) {
            r.target = Some(r.gpu.target(device, self.size_px));
        }
        if let (Some((_, _, _, scene)), Some(target)) = (&r.scene, &r.target) { r.gpu.draw(queue, encoder, target, scene, &self.camera, self.section); }
        Vec::new()
    }

    fn paint(&self, _info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, res: &CallbackResources) {
        let Some(r) = res.get::<Resources>() else { return };
        if r.scene.is_none() { return; }
        if let Some(t) = &r.target { r.gpu.blit(pass, t); }
    }
}

/// A datum drawn over the view as a dashed line, like a level or a grid (model coordinates).
#[derive(Debug, Clone, PartialEq)]
pub struct Datum {
    pub a: [f64; 3],
    pub b: [f64; 3],
    pub label: String,
    /// a bubble with the label at both ends (grids); otherwise text at `a` (levels)
    pub bubbles: bool,
}

/// A 3D view. Keep one per view; call `ui` every frame.
pub struct Viewport3d {
    pub camera: Camera,
    pub style: Style,
    /// Where the sun is in the Realistic style.
    pub sun: Sun,
    pub labels: CubeLabels,
    /// Levels and grids to draw over the picture.
    pub datums: Vec<Datum>,
    /// The section box, when it is on (model coordinates).
    pub section: Option<SectionBox>,
    /// The elements that are selected. Change it with `select`, so that the view redraws it.
    selection: Arc<HashSet<Element>>,
    selection_generation: u64,
    /// What the last click found, if anything.
    pub last_hit: Option<Hit>,
    /// Whether to find what is under the pointer on every frame (`hover_hit`). Off by default: it costs a pick a frame.
    pub track_hover: bool,
    /// What is under the pointer now, when `track_hover` is on.
    pub hover_hit: Option<Hit>,
    /// Lines drawn over the picture, in model coordinates, with their colour (a preview of what a click would make).
    pub overlay_lines: Vec<([f64; 3], [f64; 3], [u8; 3])>,
    /// Rings drawn over the picture at points of the model.
    pub overlay_rings: Vec<([f64; 3], [u8; 3])>,
    /// Seconds a ViewCube click takes to turn the camera; 0 for no animation.
    pub snap_seconds: f64,
    anim: Option<(Camera, Camera, f64)>,
    batch: Option<Arc<Batch>>,
    generation: u64,
    /// Bytes of graphics memory a model may take; a larger one is not drawn and `problem` says why. None: no check.
    pub memory_budget: Option<u64>,
    problem: Arc<std::sync::Mutex<Option<String>>>,
}

impl Viewport3d {
    /// Call once at start-up with `frame.wgpu_render_state()`.
    pub fn init(rs: &RenderState) {
        let gpu = Gpu::new(&rs.device, rs.target_format);
        rs.renderer.write().callback_resources.insert(Resources { gpu, scene: None, target: None, failed: None });
    }

    pub fn new() -> Viewport3d {
        Viewport3d { camera: Camera::default_3d([0.0; 3], 20.0), style: Style::Shaded, sun: Sun::default(), labels: CubeLabels::en(), datums: Vec::new(), section: None, selection: Arc::new(HashSet::new()), selection_generation: 0, last_hit: None, track_hover: false, hover_hit: None, overlay_lines: Vec::new(), overlay_rings: Vec::new(), snap_seconds: 0.35, anim: None, batch: None, generation: 0, memory_budget: None, problem: Arc::new(std::sync::Mutex::new(None)) }
    }

    /// Why the model could not be put on the graphics card (too large for `memory_budget`), if that is so.
    pub fn problem(&self) -> Option<String> { self.problem.lock().ok().and_then(|p| p.clone()) }

    /// Show a new batch (after an edit). The camera is not moved.
    pub fn set_batch(&mut self, b: Arc<Batch>) { self.batch = Some(b); self.generation += 1; }

    pub fn selection(&self) -> &HashSet<Element> { &self.selection }

    /// Replace the selection.
    pub fn select(&mut self, elements: HashSet<Element>) {
        if *self.selection != elements { self.selection = Arc::new(elements); self.selection_generation += 1; }
    }

    /// The box around the model in model coordinates.
    pub fn model_bounds(&self) -> Option<SectionBox> {
        let b = self.batch.as_ref()?;
        if b.verts.is_empty() { return None; }
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for v in &b.verts { for k in 0..3 { let x = v[k] as f64 + b.origin[k]; lo[k] = lo[k].min(x); hi[k] = hi[k].max(x); } }
        Some(SectionBox { min: lo, max: hi })
    }

    /// Turn the camera to look from a ViewCube place, with animation.
    pub fn snap_to(&mut self, d: CubeDir, now: f64) {
        let mut to = self.camera;
        to.snap_to(d);
        if self.snap_seconds <= 0.0 { self.camera = to; self.anim = None; } else { self.anim = Some((self.camera, to, now)); }
    }

    /// Where the animation puts the camera at time `now`; ends it when it is over. True while it runs.
    pub fn step_animation(&mut self, now: f64) -> bool {
        let Some((from, to, t0)) = self.anim else { return false };
        let t = ((now - t0) / self.snap_seconds).clamp(0.0, 1.0);
        let e = t * t * (3.0 - 2.0 * t);
        self.camera = Camera::blend(&from, &to, e);
        if t >= 1.0 { self.anim = None; }
        t < 1.0
    }

    /// Zoom to fit the whole model.
    pub fn zoom_to_fit(&mut self, aspect: f64) {
        let Some(b) = &self.batch else { return };
        if b.verts.is_empty() { return; }
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for v in &b.verts { for k in 0..3 { let x = v[k] as f64 + b.origin[k]; lo[k] = lo[k].min(x); hi[k] = hi[k].max(x); } }
        self.camera.fit(lo, hi, aspect);
    }

    /// Draw the view into the free space of `ui` and handle the mouse. Returns the response of the view.
    pub fn ui(&mut self, ui: &mut Ui) -> egui::Response {
        let size = ui.available_size().max(Vec2::splat(16.0));
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point();

        // ViewCube first, so that a click on it does not also start a drag in the view
        let cube_half = 48.0;
        let cube_centre = Pos2::new(rect.right() - cube_half - 18.0, rect.top() + cube_half + 18.0);
        let cells = cube_cells(&self.camera, cube_centre, cube_half * 0.7);
        let hover = resp.hover_pos();
        let over_cube = hover.and_then(|p| pick_cell(&cells, p));
        if over_cube.is_some() && resp.clicked_by(PointerButton::Primary) {
            if let Some(d) = over_cube { let now = ui.input(|i| i.time); self.snap_to(d, now); }
        } else {
            self.mouse(ui, &resp, rect);
        }

        let now = ui.input(|i| i.time);
        if self.step_animation(now) { ui.ctx().request_repaint(); }
        let draw = Draw { budget: self.memory_budget, problem: self.problem.clone(), sun: self.sun, camera: self.camera, style: self.style, generation: self.generation, batch: self.batch.clone(), section: self.section, selection: self.selection.clone(), selection_generation: self.selection_generation, size_px: [(rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32] };
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(rect, draw));
        if let Some(why) = self.problem() {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, format!("The 3D view cannot show this model: {why}.\nPlans, sections and schedules are not affected."), FontId::proportional(14.0), ui.visuals().weak_text_color());
        }
        self.paint_datums(ui, rect);
        self.paint_overlay(ui, rect);
        self.paint_cube(ui, &cells, over_cube);
        resp
    }

    fn mouse(&mut self, ui: &Ui, resp: &egui::Response, rect: Rect) {
        let size = [rect.width() as f64, rect.height() as f64];
        let (buttons, mods, scroll, pos) = ui.input(|i| (
            Buttons { left: i.pointer.button_down(PointerButton::Primary), middle: i.pointer.button_down(PointerButton::Middle), right: i.pointer.button_down(PointerButton::Secondary) },
            Mods { shift: i.modifiers.shift, ctrl: i.modifiers.command },
            i.smooth_scroll_delta.y,
            i.pointer.hover_pos(),
        ));
        if resp.dragged() || (resp.hovered() && scroll != 0.0) { self.anim = None; }
        self.hover_hit = None;
        if self.track_hover && resp.hovered() && !buttons.left && !buttons.middle && !buttons.right {
            if let (Some(p), Some(b)) = (pos, self.batch.clone()) {
                self.hover_hit = pick(&b, &self.camera, [(p.x - rect.left()) as f64, (p.y - rect.top()) as f64], size, self.section);
            }
        }
        if resp.clicked_by(PointerButton::Primary) {
            if let (Some(p), Some(b)) = (pos, self.batch.clone()) {
                self.last_hit = pick(&b, &self.camera, [(p.x - rect.left()) as f64, (p.y - rect.top()) as f64], size, self.section);
                let add = ui.input(|i| i.modifiers.ctrl || i.modifiers.command || i.modifiers.shift);
                let mut sel: HashSet<Element> = if add { (*self.selection).clone() } else { HashSet::new() };
                if let Some(h) = &self.last_hit { if !sel.remove(&h.element) || !add { sel.insert(h.element); } }
                self.select(sel);
            }
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            self.camera.drag(buttons, mods, d.x as f64, d.y as f64, size[1]);
        }
        if resp.hovered() && scroll != 0.0 {
            if let Some(p) = pos {
                // a turn of the wheel forward comes closer, 10 % a notch of 50 points
                let factor = (-(scroll as f64) * 0.0021).exp();
                self.camera.zoom_at([(p.x - rect.left()) as f64, (p.y - rect.top()) as f64], factor, size);
            }
        }
        // a double click of the middle button fits the model, as in Revit
        if resp.double_clicked_by(PointerButton::Middle) { self.zoom_to_fit(size[0] / size[1]); }
    }

    /// A small panel: the visual style and the section box with sliders for its six sides.
    pub fn controls_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (st, name) in [(Style::Wireframe, "Wireframe"), (Style::HiddenLine, "Hidden Line"), (Style::Shaded, "Shaded"), (Style::ConsistentColors, "Consistent Colors"), (Style::Realistic, "Realistic")] {
                ui.selectable_value(&mut self.style, st, name);
            }
        });
        let bounds = self.model_bounds();
        let mut on = self.section.is_some();
        if ui.checkbox(&mut on, "Section Box").changed() {
            self.section = if on { bounds } else { None };
        }
        if let (Some(sec), Some(b)) = (&mut self.section, bounds) {
            for (k, axis) in ["X", "Y", "Z"].iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(*axis);
                    let (lo, hi) = (b.min[k], b.max[k]);
                    let mut a = sec.min[k];
                    let mut z = sec.max[k];
                    ui.add(egui::Slider::new(&mut a, lo..=hi).show_value(false));
                    ui.add(egui::Slider::new(&mut z, lo..=hi).show_value(false));
                    sec.min[k] = a.min(z);
                    sec.max[k] = z.max(a);
                });
            }
        }
    }

    /// Place and time of the sun: latitude, longitude, north angle, clock offset, date and time of day, and where that
    /// puts it. Use `sun_time_ui` alone when the place comes from the project.
    pub fn sun_ui(&mut self, ui: &mut Ui) {
        let s = &mut self.sun;
        ui.horizontal_wrapped(|ui| {
            ui.label("Lat");
            ui.add(egui::DragValue::new(&mut s.latitude).range(-90.0..=90.0).speed(0.05).suffix("°"));
            ui.label("Lon");
            ui.add(egui::DragValue::new(&mut s.longitude).range(-180.0..=180.0).speed(0.05).suffix("°"));
            ui.label("North");
            ui.add(egui::DragValue::new(&mut s.project_north_deg).range(-180.0..=180.0).speed(0.5).suffix("°"));
            ui.label("UTC+");
            ui.add(egui::DragValue::new(&mut s.utc_offset).range(-12.0..=14.0).speed(0.1));
        });
        self.sun_time_ui(ui);
    }

    /// Date and time of day, and where the sun is then.
    pub fn sun_time_ui(&mut self, ui: &mut Ui) {
        let s = &mut self.sun;
        ui.horizontal_wrapped(|ui| {
            ui.label("Date");
            ui.add(egui::DragValue::new(&mut s.day).range(1..=31));
            ui.add(egui::DragValue::new(&mut s.month).range(1..=12));
            ui.label("Time");
            ui.add(egui::Slider::new(&mut s.hour, 0.0..=24.0).show_value(false));
            let m = (s.hour * 60.0).round() as u32;
            ui.label(format!("{:02}:{:02}", (m / 60) % 24, m % 60));
            let p = s.position();
            ui.weak(format!("sun {:.0}° up, {:.0}° from north", p.elevation, p.azimuth));
        });
    }

    fn paint_overlay(&self, ui: &Ui, rect: Rect) {
        let p = ui.painter().with_clip_rect(rect);
        let size = [rect.width() as f64, rect.height() as f64];
        let at = |q: [f64; 3]| self.camera.project(q, size).map(|s| rect.min + Vec2::new(s[0] as f32, s[1] as f32));
        for (a, b, c) in &self.overlay_lines {
            if let (Some(a), Some(b)) = (at(*a), at(*b)) { p.line_segment([a, b], Stroke::new(2.0, Color32::from_rgb(c[0], c[1], c[2]))); }
        }
        for (q, c) in &self.overlay_rings {
            if let Some(q) = at(*q) { p.circle_stroke(q, 8.0, Stroke::new(2.0, Color32::from_rgb(c[0], c[1], c[2]))); }
        }
    }

    fn paint_datums(&self, ui: &Ui, rect: Rect) {
        let p = ui.painter().with_clip_rect(rect);
        let colour = ui.visuals().weak_text_color();
        let size = [rect.width() as f64, rect.height() as f64];
        let at = |q: [f64; 3]| self.camera.project(q, size).map(|s| rect.min + Vec2::new(s[0] as f32, s[1] as f32));
        for d in &self.datums {
            let (Some(a), Some(b)) = (at(d.a), at(d.b)) else { continue };
            let len = (b - a).length();
            if len < 1.0 { continue; }
            let dir = (b - a) / len;
            let mut t = 0.0;
            while t < len {
                p.line_segment([a + dir * t, a + dir * (t + 7.0).min(len)], Stroke::new(1.0, colour));
                t += 12.0;
            }
            if d.bubbles {
                for end in [a, b] {
                    p.circle(end + dir * if end == b { 11.0 } else { -11.0 }, 11.0, ui.visuals().panel_fill, Stroke::new(1.0, colour));
                    p.text(end + dir * if end == b { 11.0 } else { -11.0 }, Align2::CENTER_CENTER, &d.label, FontId::proportional(12.0), colour);
                }
            } else {
                p.text(a, Align2::LEFT_BOTTOM, &d.label, FontId::proportional(12.0), colour);
            }
        }
    }

    fn paint_cube(&self, ui: &Ui, cells: &[Cell], hover: Option<CubeDir>) {
        let p = ui.painter();
        let dark = ui.visuals().dark_mode;
        let (fill, line, text) = if dark { (Color32::from_gray(70), Color32::from_gray(150), Color32::WHITE) } else { (Color32::from_gray(238), Color32::from_gray(120), Color32::BLACK) };
        for c in cells {
            let hot = hover == Some(c.dir);
            let colour = if hot { Color32::from_rgb(120, 170, 235) } else if c.is_face { fill } else { fill.gamma_multiply(0.82) };
            p.add(Shape::convex_polygon(c.quad.to_vec(), colour, Stroke::new(0.75, line)));
            let area = c.quad.iter().zip(c.quad.iter().cycle().skip(1)).map(|(a, b)| a.x * b.y - b.x * a.y).sum::<f32>().abs() / 2.0;
            if c.is_face && area > 120.0 {
                let centre = Pos2::new(c.quad.iter().map(|q| q.x).sum::<f32>() / 4.0, c.quad.iter().map(|q| q.y).sum::<f32>() / 4.0);
                p.text(centre, Align2::CENTER_CENTER, face_name(&self.labels, c.dir), FontId::proportional(8.0), text);
            }
        }
    }
}

impl Default for Viewport3d { fn default() -> Self { Viewport3d::new() } }
