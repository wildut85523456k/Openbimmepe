//! View properties, as Revit has them on the View Control Bar and in the Properties of a view: the scale and detail level,
//! the View Range of a plan, Visibility/Graphics (categories turned off, elements hidden), Hide in View, Reveal Hidden
//! Elements, and Temporary Hide/Isolate (which is not saved and not an edit). The settings are saved with the model, one
//! undo step each (`openbim_document::ViewSettings`).
use crate::{Desktop, View};
use eframe::egui;
use openbim_document::{Detail, Document, Element, ElementId, Mesh, RangeClass, ViewCategory, ViewRange, ViewSettings, STANDARD_SCALES};
use std::sync::Arc;

/// The key the document knows a view by; None for a view that has no settings (a schedule).
///
/// `p10` and `p11` are the keys of the first two plan views — names, not level ids: the level a plan shows is resolved
/// from the document (`sheet_draw::plan_level_id`, `panels::level_view_name`), so a project whose levels have other ids
/// still resolves correctly (A2/R1). A `pN` key names the level `N` itself.
pub(crate) fn view_key(v: View) -> Option<String> {
    Some(match v {
        View::Plan => "p10".into(),
        View::UpperPlan => "p11".into(),
        View::Level(id) => format!("p{id}"),
        View::Mechanical => "mech".into(),
        View::ThreeD => "3d".into(),
        View::Elevation(k) => format!("e{k}"),
        View::Section(id) => format!("s{id}"),
        View::Marked(id, k) => format!("m{id}_{k}"),
        View::Schedule(_) | View::Sheet(_) => return None,
    })
}

/// The view a key stands for; the inverse of `view_key`. A schedule is `sc` and its number.
pub(crate) fn view_from_key(key: &str) -> Option<View> {
    let num = |t: &str| -> Option<u64> { (!t.is_empty() && t.bytes().all(|b| b.is_ascii_digit())).then(|| t.parse().ok()).flatten() };
    match key {
        "mech" => return Some(View::Mechanical),
        "3d" => return Some(View::ThreeD),
        _ => {}
    }
    if let Some(n) = key.strip_prefix("sc") { return num(n).map(View::Schedule); }
    if let Some(n) = key.strip_prefix('p') { return num(n).map(|n| match n { 10 => View::Plan, 11 => View::UpperPlan, n => View::Level(n) }); }
    if let Some(n) = key.strip_prefix('e') { return num(n).filter(|k| *k < 4).map(|k| View::Elevation(k as u8)); }
    if let Some(n) = key.strip_prefix('s') { return num(n).map(View::Section); }
    if let Some(r) = key.strip_prefix('m') {
        let (id, k) = r.split_once('_')?;
        return Some(View::Marked(num(id)?, num(k).filter(|k| *k < 4)? as u8));
    }
    None
}

/// What Temporary Hide/Isolate does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TempMode { Hide, Isolate }

/// Temporary Hide/Isolate: not saved, not an edit; it belongs to the view it was made in.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Temp {
    pub key: String,
    pub mode: TempMode,
    pub elements: Vec<ElementId>,
    pub categories: Vec<ViewCategory>,
}

/// The View Range window.
pub(crate) struct RangeDialog {
    pub key: String,
    pub range: ViewRange,
}

/// The Visibility/Graphics window.
pub(crate) struct VgDialog {
    pub draft: ViewSettings,
    pub annotation: bool,
}

#[derive(Default)]
pub(crate) struct ViewUi {
    pub range: Option<RangeDialog>,
    pub vg: Option<VgDialog>,
    pub temp: Option<Temp>,
    /// Reveal Hidden Elements
    pub reveal: bool,
    /// how many elements the view range kept out of the plan on show, for the control bar
    pub out_of_range: usize,
    plan: Option<(u64, Arc<Vec<PlanMesh>>)>,
    /// the temporary hide/isolate the 3D batch was made with
    pub batch_temp: Option<Temp>,
}

/// What the plan draws of an element: its mesh and how high it stands.
pub(crate) struct PlanMesh {
    pub id: ElementId,
    pub mesh: Mesh,
    pub z0: f64,
    pub z1: f64,
    /// the level the element stands on (its own, or its host's)
    pub level: Option<ElementId>,
}

/// How an element is shown in a view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Show { Normal, Hidden, Revealed }

/// The settings of the view on show, taken once for a frame.
#[derive(Clone)]
pub(crate) struct ViewVis {
    pub settings: ViewSettings,
    pub temp: Option<Temp>,
    reveal: bool,
}

impl ViewVis {
    pub fn state(&self, e: &Element) -> Show {
        if let Some(t) = &self.temp {
            let listed = t.elements.contains(&e.id) || ViewCategory::of(e).is_some_and(|c| t.categories.contains(&c));
            if (t.mode == TempMode::Hide) == listed { return Show::Hidden; }
        }
        if self.settings.hides(e) { if self.reveal { Show::Revealed } else { Show::Hidden } } else { Show::Normal }
    }
    pub fn hidden(&self, e: &Element) -> bool { self.state(e) == Show::Hidden }
    /// Whether a whole category (the markers of elevations, which are not elements) is off in the view.
    pub fn category_off(&self, c: ViewCategory) -> bool {
        self.settings.hidden_categories.contains(&c) || match &self.temp {
            Some(t) if t.mode == TempMode::Hide => t.categories.contains(&c),
            Some(t) => !t.categories.contains(&c),
            None => false,
        }
    }
    /// Whether anything is hidden at all (so the document must be filtered to make a 3D view or a section).
    pub fn any(&self) -> bool {
        self.temp.is_some() || !self.settings.hidden_categories.is_empty() || !self.settings.hidden_elements.is_empty()
    }
}

/// Where a mesh of the plan is in the view range of the plan of `level` (whose elevation is `level_z`). What stands on the
/// plan's own level and lies below the range (a slab, a footing) is seen as in the range, as in Revit's plans of a floor.
pub(crate) fn plan_class(range: Option<ViewRange>, level: ElementId, level_z: f64, pm: &PlanMesh) -> RangeClass {
    let Some(r) = range else { return RangeClass::Cut };
    let class = r.classify(level_z, pm.z0, pm.z1);
    if matches!(class, RangeClass::Out | RangeClass::Beyond) && pm.level == Some(level) && pm.z1 <= level_z + r.bottom_m + 1e-6 { RangeClass::Below } else { class }
}

/// The size, in pixels, of something that is `paper_mm` on paper in a view of scale 1:`denominator` drawn at `px_per_m`.
pub(crate) fn annotation_px(paper_mm: f32, denominator: u32, px_per_m: f32, min: f32, max: f32) -> f32 {
    (paper_mm * denominator as f32 / 1000.0 * px_per_m).clamp(min, max)
}

pub(crate) fn all_meshes(doc: &Document) -> Vec<PlanMesh> {
    use openbim_document::{Category, Geometry};
    let mut out: Vec<PlanMesh> = Vec::new();
    let put = |id: ElementId, mesh: Mesh, out: &mut Vec<PlanMesh>| {
        let (mut z0, mut z1) = (f64::INFINITY, f64::NEG_INFINITY);
        for v in &mesh.vertices { z0 = z0.min(v[2]); z1 = z1.max(v[2]); }
        if !z0.is_finite() { return; }
        let level = doc.level_of(id);
        match out.iter().position(|m| m.id == id) { Some(i) => out[i] = PlanMesh { id, mesh, z0, z1, level }, None => out.push(PlanMesh { id, mesh, z0, z1, level }) }
    };
    for el in doc.elements() {
        let hosted = matches!(el.geometry, Some(Geometry::OnWall { .. }));
        let placed = matches!(el.geometry, Some(Geometry::Linear { .. } | Geometry::Point { .. } | Geometry::Area { .. } | Geometry::Pad { .. }));
        if (hosted || placed) && el.category != Category::Section {
            if let Some(mesh) = doc.mesh(el.id) { put(el.id, mesh, &mut out); }
        }
    }
    // family instances, and the walls and members they cut, come from the scene mesher
    if let Some(scene) = crate::view3d::scene_meshes(doc) {
        for em in scene.iter() {
            let id = ElementId(em.element.id);
            let Some(el) = doc.get(id) else { continue };
            if matches!(el.geometry, Some(Geometry::Placed { .. } | Geometry::Linear { .. } | Geometry::Point { .. } | Geometry::Area { .. } | Geometry::InWall { .. })) {
                put(id, crate::view3d::doc_mesh(em), &mut out);
            }
        }
    }
    out
}

impl Desktop {
    pub(crate) fn current_view_key(&self) -> Option<String> { view_key(self.view) }

    /// The settings of the view on show.
    pub(crate) fn current_view_settings(&self) -> ViewSettings {
        match self.current_view_key() { Some(k) => self.document.view_settings(&k), None => ViewSettings::default_for("none") }
    }

    /// The settings, temporary state and reveal flag of the view on show, for one frame's drawing.
    pub(crate) fn vis(&self) -> ViewVis {
        let settings = self.current_view_settings();
        let temp = self.views.temp.clone().filter(|t| Some(&t.key) == self.current_view_key().as_ref());
        ViewVis { settings, temp, reveal: self.views.reveal }
    }

    /// The meshes of every element of the plan, made once for each revision of the model.
    pub(crate) fn plan_meshes(&mut self) -> Arc<Vec<PlanMesh>> {
        let rev = self.document.revision();
        if let Some((r, m)) = &self.views.plan { if *r == rev { return m.clone(); } }
        let m = Arc::new(all_meshes(&self.document));
        self.views.plan = Some((rev, m.clone()));
        m
    }

    /// The document as the view on show sees it for its geometry, when it hides anything (for the 3D view and sections).
    /// Annotation categories stay, because the scene does not draw them and a section line is an element too.
    pub(crate) fn view_document(&self) -> Option<Document> {
        let vis = self.vis();
        vis.any().then(|| self.document.filtered(&|e| !vis.hidden(e) || ViewCategory::of(e).is_some_and(|c| c.annotation())))
    }

    /// The same with the annotation categories hidden too, for the grids and levels the 3D view draws.
    pub(crate) fn view_datum_document(&self) -> Option<Document> {
        let vis = self.vis();
        vis.any().then(|| self.document.filtered(&|e| !vis.hidden(e)))
    }

    /// Saves changed settings of the view `key` in one undo step.
    pub(crate) fn apply_view_settings(&mut self, vs: &ViewSettings, done: &str) -> bool {
        match self.document.set_view_settings_commands(vs) {
            Ok(ops) => match self.document.transact(self.document.revision(), &ops) {
                Ok(_) => { self.message = format!("{done}; undo takes it back"); true }
                Err(e) => { self.message = format!("View settings rejected ({e:?})"); false }
            },
            Err(e) => { self.message = format!("View settings: {e}"); false }
        }
    }

    pub(crate) fn open_view_range(&mut self) {
        let Some(key) = self.current_view_key() else { return };
        match self.current_view_settings().effective_range() {
            Some(range) => self.views.range = Some(RangeDialog { key, range }),
            None => self.message = "View Range is for plan views".into(),
        }
    }

    pub(crate) fn open_vis_graphics(&mut self) {
        if self.current_view_key().is_none() { self.message = "Visibility/Graphics: open a plan, a 3D view, an elevation or a section".into(); return; }
        self.views.vg = Some(VgDialog { draft: self.current_view_settings(), annotation: false });
    }

    /// Hide in View: the selected elements (or their categories) are hidden in the view, saved with it.
    pub(crate) fn hide_in_view(&mut self, categories: bool) {
        let Some(key) = self.current_view_key() else { return };
        let sel = self.selected_ids();
        if sel.is_empty() { self.message = "Hide in View: select something first".into(); return; }
        let mut vs = self.current_view_settings();
        if categories {
            for id in &sel { if let Some(c) = self.document.get(*id).and_then(ViewCategory::of) { if !vs.hidden_categories.contains(&c) { vs.hidden_categories.push(c); } } }
        } else {
            for id in &sel { if !vs.hidden_elements.contains(id) { vs.hidden_elements.push(*id); } }
        }
        debug_assert_eq!(vs.key, key);
        if self.apply_view_settings(&vs, if categories { "Categories hidden in the view" } else { "Elements hidden in the view" }) { self.select_many(Vec::new()); }
    }

    /// Unhide Element / Category (with Reveal Hidden Elements on): the selected hidden elements, or their categories, show again.
    pub(crate) fn unhide_in_view(&mut self, categories: bool) {
        let sel = self.selected_ids();
        if sel.is_empty() { self.message = "Unhide: select the hidden elements (Reveal Hidden Elements shows them)".into(); return; }
        let mut vs = self.current_view_settings();
        if categories {
            let cats: Vec<ViewCategory> = sel.iter().filter_map(|id| self.document.get(*id).and_then(ViewCategory::of)).collect();
            vs.hidden_categories.retain(|c| !cats.contains(c));
        } else {
            vs.hidden_elements.retain(|id| !sel.contains(id));
        }
        self.apply_view_settings(&vs, if categories { "Categories shown in the view" } else { "Elements shown in the view" });
    }

    /// Temporary Hide/Isolate of the selection: the elements or their categories. It is not saved and is not an edit.
    pub(crate) fn temporary_hide_isolate(&mut self, mode: TempMode, categories: bool) {
        let Some(key) = self.current_view_key() else { return };
        let sel = self.selected_ids();
        if sel.is_empty() { self.message = "Temporary Hide/Isolate: select something first".into(); return; }
        let cats: Vec<ViewCategory> = {
            let mut v: Vec<ViewCategory> = Vec::new();
            for id in &sel { if let Some(c) = self.document.get(*id).and_then(ViewCategory::of) { if !v.contains(&c) { v.push(c); } } }
            v
        };
        self.views.temp = Some(Temp { key, mode, elements: if categories { Vec::new() } else { sel }, categories: if categories { cats } else { Vec::new() } });
        if mode == TempMode::Hide { self.select_many(Vec::new()); }
        self.message = format!("Temporary {}: nothing is saved; Reset Temporary Hide/Isolate, or Apply it to the view", if mode == TempMode::Hide { "Hide" } else { "Isolate" });
    }

    pub(crate) fn reset_temporary(&mut self) {
        if self.views.temp.take().is_some() { self.message = "Temporary Hide/Isolate reset".into(); }
    }

    /// Apply Hide/Isolate to View: what is hidden or isolated temporarily becomes the view's own setting.
    pub(crate) fn apply_temporary(&mut self) {
        let Some(t) = self.views.temp.clone() else { return };
        let mut vs = self.current_view_settings();
        let hide: Vec<&Element> = self.document.elements().filter(|e| {
            let listed = t.elements.contains(&e.id) || ViewCategory::of(e).is_some_and(|c| t.categories.contains(&c));
            (t.mode == TempMode::Hide) == listed && ViewCategory::of(e).is_some()
        }).collect();
        if t.mode == TempMode::Hide && !t.categories.is_empty() {
            for c in &t.categories { if !vs.hidden_categories.contains(c) { vs.hidden_categories.push(*c); } }
        } else {
            for e in hide { if !vs.hidden_elements.contains(&e.id) { vs.hidden_elements.push(e.id); } }
        }
        if self.apply_view_settings(&vs, "Hide/Isolate applied to the view") { self.views.temp = None; }
    }

    /// The View Control Bar: scale, detail level, View Range, Visibility/Graphics, Hide in View, Temporary Hide/Isolate.
    pub(crate) fn view_control_bar(&mut self, ui: &mut egui::Ui) {
        let Some(key) = self.current_view_key() else { return };
        let vs = self.document.view_settings(&key);
        let plan = ViewSettings::is_plan_key(&key);
        let mut next = vs.clone();
        ui.horizontal_wrapped(|ui| {
            ui.label("Scale");
            let current = format!("1 : {}", vs.scale);
            egui::ComboBox::from_id_salt("view_scale").selected_text(current).width(90.0).show_ui(ui, |ui| {
                for s in STANDARD_SCALES { ui.selectable_value(&mut next.scale, s, format!("1 : {s}")); }
                if !STANDARD_SCALES.contains(&vs.scale) { ui.selectable_value(&mut next.scale, vs.scale, format!("1 : {} (custom)", vs.scale)); }
            });
            let mut custom = vs.scale;
            if ui.add(egui::DragValue::new(&mut custom).range(1..=10_000).prefix("1 : ").speed(1.0)).on_hover_text("A custom scale").drag_stopped() { next.scale = custom; }
            ui.label("Detail Level");
            egui::ComboBox::from_id_salt("view_detail").selected_text(vs.detail.name()).width(80.0).show_ui(ui, |ui| {
                for d in Detail::ALL { ui.selectable_value(&mut next.detail, d, d.name()); }
            });
            if plan && ui.button("View Range…").clicked() { self.open_view_range(); }
            if ui.button("Visibility/Graphics…").clicked() { self.open_vis_graphics(); }
            ui.menu_button("Temporary Hide/Isolate", |ui| {
                if ui.button("Hide Element").clicked() { self.temporary_hide_isolate(TempMode::Hide, false); ui.close(); }
                if ui.button("Hide Category").clicked() { self.temporary_hide_isolate(TempMode::Hide, true); ui.close(); }
                if ui.button("Isolate Element").clicked() { self.temporary_hide_isolate(TempMode::Isolate, false); ui.close(); }
                if ui.button("Isolate Category").clicked() { self.temporary_hide_isolate(TempMode::Isolate, true); ui.close(); }
                ui.separator();
                if ui.add_enabled(self.views.temp.is_some(), egui::Button::new("Apply Hide/Isolate to View")).clicked() { self.apply_temporary(); ui.close(); }
                if ui.add_enabled(self.views.temp.is_some(), egui::Button::new("Reset Temporary Hide/Isolate")).clicked() { self.reset_temporary(); ui.close(); }
            });
            ui.menu_button("Hide in View", |ui| {
                if ui.button("Elements").clicked() { self.hide_in_view(false); ui.close(); }
                if ui.button("Category").clicked() { self.hide_in_view(true); ui.close(); }
                ui.separator();
                if ui.add_enabled(self.views.reveal, egui::Button::new("Unhide Elements")).clicked() { self.unhide_in_view(false); ui.close(); }
                if ui.add_enabled(self.views.reveal, egui::Button::new("Unhide Category")).clicked() { self.unhide_in_view(true); ui.close(); }
            });
            ui.checkbox(&mut self.views.reveal, "Reveal Hidden Elements").on_hover_text("Hidden elements are drawn in magenta; select them to unhide");
            if plan && self.views.out_of_range > 0 { ui.weak(format!("{} element(s) outside the view range", self.views.out_of_range)); }
        });
        if self.views.temp.as_ref().is_some_and(|t| t.key == key) {
            ui.horizontal(|ui| {
                ui.colored_label(egui::Color32::from_rgb(0, 160, 200), "Temporary Hide/Isolate mode: nothing is saved");
                if ui.small_button("Apply to View").clicked() { self.apply_temporary(); }
                if ui.small_button("Reset").clicked() { self.reset_temporary(); }
            });
        }
        if next != vs { self.apply_view_settings(&next, "View settings changed"); }
    }

    pub(crate) fn view_range_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.views.range.take() else { return };
        let (mut open, mut ok, mut apply, mut cancel, mut restore) = (true, false, false, false, false);
        let level = self.plan_level();
        let level_name = self.document.get(level).map_or(String::new(), |l| l.name.clone());
        egui::Window::new("View Range").id(egui::Id::unique("view_range_window")).open(&mut open).default_width(360.0).show(ctx, |ui| {
            ui.label(format!("Plan of {level_name}: heights are in millimetres above the level"));
            egui::Grid::new("view_range_grid").num_columns(2).show(ui, |ui| {
                for (label, v) in [("Top", &mut d.range.top_m), ("Cut plane", &mut d.range.cut_m), ("Bottom", &mut d.range.bottom_m), ("View depth", &mut d.range.depth_m)] {
                    ui.label(label);
                    let mut mm = *v * 1000.0;
                    if ui.add(egui::DragValue::new(&mut mm).speed(10.0).suffix(" mm")).changed() { *v = mm / 1000.0; }
                    ui.end_row();
                }
            });
            ui.weak("Top: the highest things seen. Cut plane: walls and columns it crosses are drawn cut. Bottom: the lowest things seen normally. View depth: things this far below the bottom are still seen, faint.");
            match d.range.problem() {
                Some(p) => { ui.colored_label(ui.visuals().warn_fg_color, format!("Not allowed: {p}")); }
                None => { ui.weak(format!("{} element(s) of the model fall outside this range", self.views.out_of_range)); }
            }
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() { ok = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
                if ui.button("Apply").clicked() { apply = true; }
                if ui.button("Restore default").clicked() { restore = true; }
            });
        });
        if restore {
            d.range = if d.key == "mech" { ViewRange::MECHANICAL } else { ViewRange::ARCHITECTURE };
        }
        let mut keep = open && !cancel;
        if (ok || apply) && d.range.valid() {
            let mut vs = self.document.view_settings(&d.key);
            let default = if d.key == "mech" { ViewRange::MECHANICAL } else { ViewRange::ARCHITECTURE };
            vs.range = (d.range != default).then_some(d.range);
            if vs == self.document.view_settings(&d.key) { if ok { keep = false; } }
            else if self.apply_view_settings(&vs, "View range changed") && ok { keep = false; }
        }
        if keep { self.views.range = Some(d); }
    }

    pub(crate) fn vis_graphics_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.views.vg.take() else { return };
        let (mut open, mut ok, mut apply, mut cancel) = (true, false, false, false);
        let present: Vec<ViewCategory> = {
            let mut v: Vec<ViewCategory> = Vec::new();
            for e in self.document.elements() { if let Some(c) = ViewCategory::of(e) { if !v.contains(&c) { v.push(c); } } }
            v
        };
        egui::Window::new("Visibility/Graphics Overrides").id(egui::Id::unique("vis_graphics_window")).open(&mut open).default_width(380.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut d.annotation, false, "Model Categories");
                ui.selectable_value(&mut d.annotation, true, "Annotation Categories");
            });
            let list: Vec<ViewCategory> = ViewCategory::all().into_iter().filter(|c| c.annotation() == d.annotation).collect();
            ui.horizontal(|ui| {
                if ui.small_button("All").clicked() { d.draft.hidden_categories.retain(|c| c.annotation() != d.annotation); }
                if ui.small_button("None").clicked() { for c in &list { if !d.draft.hidden_categories.contains(c) { d.draft.hidden_categories.push(*c); } } }
                if ui.small_button("Invert").clicked() {
                    for c in &list { if let Some(i) = d.draft.hidden_categories.iter().position(|x| x == c) { d.draft.hidden_categories.remove(i); } else { d.draft.hidden_categories.push(*c); } }
                }
            });
            egui::ScrollArea::vertical().id_salt("vg_list").max_height(280.0).show(ui, |ui| {
                for c in &list {
                    let mut visible = !d.draft.hidden_categories.contains(c);
                    let label = if present.contains(c) { c.name().to_string() } else { format!("{} (none in the model)", c.name()) };
                    if ui.checkbox(&mut visible, label).changed() {
                        if visible { d.draft.hidden_categories.retain(|x| x != c); } else { d.draft.hidden_categories.push(*c); }
                    }
                }
            });
            ui.separator();
            ui.label(format!("{} element(s) hidden in this view", d.draft.hidden_elements.len()));
            if !d.draft.hidden_elements.is_empty() {
                egui::ScrollArea::vertical().id_salt("vg_hidden").max_height(100.0).show(ui, |ui| {
                    let mut unhide: Option<ElementId> = None;
                    for id in &d.draft.hidden_elements {
                        ui.horizontal(|ui| {
                            ui.label(self.document.get(*id).map_or("?".to_string(), |e| e.name.clone()));
                            if ui.small_button("Unhide").clicked() { unhide = Some(*id); }
                        });
                    }
                    if let Some(u) = unhide { d.draft.hidden_elements.retain(|x| *x != u); }
                });
            }
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() { ok = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
                if ui.button("Apply").clicked() { apply = true; }
            });
        });
        let mut keep = open && !cancel;
        if ok || apply {
            // the window changes categories and hidden elements; the scale, detail and range may have changed since it opened
            let mut vs = self.document.view_settings(&d.draft.key);
            vs.hidden_categories = d.draft.hidden_categories.clone();
            vs.hidden_elements = d.draft.hidden_elements.clone();
            if vs == self.document.view_settings(&d.draft.key) { if ok { keep = false; } }
            else if self.apply_view_settings(&vs, "Visibility/Graphics changed") && ok { keep = false; }
        }
        if keep { self.views.vg = Some(d); }
    }
}

/// How an element of the plan is drawn for its class in the view range.
pub(crate) fn range_fade(class: RangeClass) -> f32 {
    match class { RangeClass::Cut | RangeClass::Below => 1.0, RangeClass::Above => 0.55, RangeClass::Beyond => 0.35, RangeClass::Out => 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Pos2, Rect, Vec2};
    use openbim_document::{Category, ScheduleKind};

    fn frames(app: &mut Desktop, n: usize, f: &dyn Fn(&mut Desktop, &mut egui::Ui)) {
        let ctx = egui::Context::default();
        for _ in 0..n {
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), ..Default::default() }, |ui| f(app, ui));
            out.textures_delta.clear();
        }
    }

    fn first(app: &Desktop, c: Category) -> ElementId { app.document.elements().find(|e| e.category == c).expect("the sample has one").id }

    #[test]
    fn views_have_keys_and_a_schedule_has_none() {
        assert_eq!(view_key(View::Plan).as_deref(), Some("p10"));
        assert_eq!(view_key(View::UpperPlan).as_deref(), Some("p11"));
        assert_eq!(view_key(View::Level(40)).as_deref(), Some("p40"));
        assert_eq!(view_key(View::Mechanical).as_deref(), Some("mech"));
        assert_eq!(view_key(View::ThreeD).as_deref(), Some("3d"));
        assert_eq!(view_key(View::Section(7)).as_deref(), Some("s7"));
        assert_eq!(view_key(View::Marked(7, 2)).as_deref(), Some("m7_2"));
        assert_eq!(view_key(View::Schedule(1)), None);
        // every key is one the document accepts
        for v in [View::Plan, View::UpperPlan, View::Level(40), View::Mechanical, View::ThreeD, View::Elevation(3), View::Section(7), View::Marked(7, 2)] {
            assert!(ViewSettings::default_for(&view_key(v).unwrap()).check(&Default::default()).is_ok(), "{v:?}");
        }
    }

    #[test]
    fn a_key_gives_back_its_view() {
        for v in [View::Plan, View::UpperPlan, View::Level(40), View::Mechanical, View::ThreeD, View::Elevation(3), View::Section(7), View::Marked(7, 2)] {
            assert_eq!(view_from_key(&view_key(v).unwrap()), Some(v), "{v:?}");
        }
        assert_eq!(view_from_key("sc12"), Some(View::Schedule(12)));
        for bad in ["", "p", "pp1", "e9", "m7", "m7_9", "s", "sc", "x1", "p-1", "3D"] { assert_eq!(view_from_key(bad), None, "{bad:?}"); }
    }

    #[test]
    fn annotations_keep_their_size_on_paper() {
        assert_eq!(annotation_px(2.0, 100, 50.0, 0.0, 1000.0), 10.0);
        assert_eq!(annotation_px(2.0, 50, 50.0, 0.0, 1000.0), 5.0, "a finer scale draws them smaller");
        assert_eq!(annotation_px(2.0, 100, 1000.0, 7.0, 20.0), 20.0, "and they stay legible");
        assert_eq!(annotation_px(2.0, 100, 1.0, 7.0, 20.0), 7.0);
    }

    #[test]
    fn the_view_range_decides_what_a_plan_shows() {
        let mut app = Desktop::sample();
        let level = ElementId(10);
        let z = app.document.get(level).unwrap().elevation_m;
        let meshes = app.plan_meshes();
        let duct = first(&app, Category::Duct);
        let wall = first(&app, Category::Wall);
        let of = |id: ElementId| meshes.iter().find(|m| m.id == id).unwrap_or_else(|| panic!("{id:?} has no mesh"));
        // the sample duct hangs at 2.7 to 3.0 m: above the 2300 mm top of a plan, inside the 3000 mm top of the mechanical plan
        assert_eq!(plan_class(Some(ViewRange::ARCHITECTURE), level, z, of(duct)), RangeClass::Out);
        assert_eq!(plan_class(Some(ViewRange::MECHANICAL), level, z, of(duct)), RangeClass::Above);
        // a wall that stands through the cut plane is cut
        assert_eq!(plan_class(Some(ViewRange::ARCHITECTURE), level, z, of(wall)), RangeClass::Cut);
        // what stands on the level below the range (a slab) is still seen, and the other level's walls are not
        for s in app.document.elements().filter(|e| e.category == Category::Slab && !e.is_roof() && app.document.level_of(e.id) == Some(level)) {
            if let Some(m) = meshes.iter().find(|m| m.id == s.id) { assert_ne!(plan_class(Some(ViewRange::ARCHITECTURE), level, z, m), RangeClass::Out, "{}", s.name); }
        }
        let other = app.document.elements().find(|e| e.category == Category::Wall && app.document.level_of(e.id) == Some(ElementId(11))).map(|e| e.id);
        if let Some(o) = other { assert_eq!(plan_class(Some(ViewRange::ARCHITECTURE), level, z, of(o)), RangeClass::Out); }
        // the plan counts what its range keeps out; raising the top brings the duct in
        app.view = View::Plan;
        frames(&mut app, 2, &|app, ui| crate::model_view::show(app, ui));
        let hidden_before = app.views.out_of_range;
        assert!(hidden_before >= 1, "the duct is out of the range of the plan");
        let mut vs = app.document.view_settings("p10");
        vs.range = Some(ViewRange { top_m: 3.5, ..ViewRange::ARCHITECTURE });
        let steps = app.document.history_mark();
        assert!(app.apply_view_settings(&vs, "x"));
        assert_eq!(app.document.history_mark(), steps + 1, "one undo step");
        frames(&mut app, 2, &|app, ui| crate::model_view::show(app, ui));
        assert!(app.views.out_of_range < hidden_before);
        // the mechanical plan has its own default
        app.view = View::Mechanical;
        frames(&mut app, 2, &|app, ui| crate::model_view::show(app, ui));
        assert_eq!(app.views.out_of_range, 0, "nothing of the mechanical plan is out of its range");
        // undo takes the range back
        app.document.undo().unwrap();
        assert_eq!(app.document.view_settings("p10").range, None);
    }

    #[test]
    fn the_view_range_window_applies_a_valid_range_only() {
        let mut app = Desktop::sample();
        app.view = View::Plan;
        app.run_command("view_range", "View Range");
        assert!(app.views.range.is_some());
        frames(&mut app, 2, &|app, ui| { let c = ui.ctx().clone(); app.view_range_window(&c); });
        assert!(app.views.range.is_some(), "the window stays open until OK or Cancel");
        // 3D has no view range
        app.views.range = None;
        app.view = View::ThreeD;
        app.open_view_range();
        assert!(app.views.range.is_none());
        assert!(app.message.contains("plan"));
    }

    #[test]
    fn visibility_graphics_turns_a_category_off_in_one_view_only() {
        let mut app = Desktop::sample();
        let wall = first(&app, Category::Wall);
        app.view = View::Plan;
        app.run_command("visibility", "Visibility/Graphics");
        assert!(app.views.vg.is_some());
        frames(&mut app, 2, &|app, ui| { let c = ui.ctx().clone(); app.vis_graphics_window(&c); });
        let mut vs = app.document.view_settings("p10");
        vs.hidden_categories.push(ViewCategory::Model(ScheduleKind::Walls));
        assert!(app.apply_view_settings(&vs, "x"));
        assert!(app.vis().hidden(app.document.get(wall).unwrap()), "walls are off in the first plan");
        app.view = View::UpperPlan;
        assert!(!app.vis().hidden(app.document.get(wall).unwrap()), "and on in the others");
        app.view = View::Plan;
        // the plan draws without them, and the 3D view and a section are made from a document without them
        frames(&mut app, 2, &|app, ui| crate::model_view::show(app, ui));
        assert!(app.view_document().is_none() || app.view_document().unwrap().elements().all(|e| e.category != Category::Wall));
        app.view = View::ThreeD;
        assert!(app.view_document().is_none(), "the 3D view has its own settings and hides nothing");
        let mut v3 = app.document.view_settings("3d");
        v3.hidden_categories.push(ViewCategory::Model(ScheduleKind::Walls));
        assert!(app.apply_view_settings(&v3, "x"));
        let seen = app.view_document().expect("walls are off in 3D");
        assert!(seen.elements().all(|e| e.category != Category::Wall));
        assert!(app.document.elements().any(|e| e.category == Category::Wall), "the model keeps them");
        frames(&mut app, 2, &|app, ui| crate::view3d::show(app, ui));
        let with = crate::view3d::batch(&app.document, None).tris.len();
        let without = crate::view3d::batch(&seen, None).tris.len();
        assert!(without < with, "the batch of the view has fewer items ({without} against {with})");
        // an elevation made from the document without the walls has fewer lines than the one made from the whole model
        let (a, b) = (crate::section_ui::build(&app.document, View::Elevation(0)), crate::section_ui::build(&seen, View::Elevation(0)));
        let a = a.expect("the elevation");
        // the sample has nothing but walls to look at from outside, so without them the elevation has nothing to show
        assert!(b.as_ref().map_or(true, |b| b.items.len() < a.items.len()), "the elevation still shows the walls");
        // the settings go with the model and undo one step at a time
        app.document.undo().unwrap();
        assert!(app.view_document().is_none());
    }

    #[test]
    fn hide_in_view_and_reveal_hidden_elements() {
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let wall = first(&app, Category::Wall);
        app.select_many(vec![wall]);
        app.hide_in_view(false);
        assert_eq!(app.document.view_settings("p10").hidden_elements, vec![wall]);
        assert!(app.selected_ids().is_empty(), "what is hidden is not selected any more");
        assert_eq!(app.vis().state(app.document.get(wall).unwrap()), Show::Hidden);
        app.views.reveal = true;
        assert_eq!(app.vis().state(app.document.get(wall).unwrap()), Show::Revealed);
        frames(&mut app, 2, &|app, ui| crate::model_view::show(app, ui));
        app.select_many(vec![wall]);
        app.unhide_in_view(false);
        assert!(app.document.view_settings("p10").hidden_elements.is_empty());
        // the whole category
        app.select_many(vec![wall]);
        app.hide_in_view(true);
        assert_eq!(app.document.view_settings("p10").hidden_categories, vec![ViewCategory::Model(ScheduleKind::Walls)]);
        app.views.reveal = true;
        app.select_many(vec![wall]);
        app.unhide_in_view(true);
        assert!(app.document.view_settings("p10").hidden_categories.is_empty());
        // nothing selected: a message, no change
        app.select_many(Vec::new());
        let steps = app.document.history_mark();
        app.hide_in_view(false);
        assert_eq!(app.document.history_mark(), steps);
    }

    #[test]
    fn temporary_hide_and_isolate_are_not_saved_and_belong_to_their_view() {
        let mut app = Desktop::sample();
        app.view = View::Plan;
        let wall = first(&app, Category::Wall);
        let duct = first(&app, Category::Duct);
        app.select_many(vec![wall]);
        let (rev, steps) = (app.document.revision(), app.document.history_mark());
        app.temporary_hide_isolate(TempMode::Isolate, true);
        assert!(app.vis().hidden(app.document.get(duct).unwrap()), "only the walls are left");
        assert!(!app.vis().hidden(app.document.get(wall).unwrap()));
        assert!(app.vis().category_off(ViewCategory::Elevations));
        frames(&mut app, 2, &|app, ui| crate::model_view::show(app, ui));
        assert_eq!((app.document.revision(), app.document.history_mark()), (rev, steps), "not an edit");
        // another view is not affected
        app.view = View::UpperPlan;
        assert!(!app.vis().hidden(app.document.get(duct).unwrap()));
        app.view = View::Plan;
        app.reset_temporary();
        assert!(!app.vis().hidden(app.document.get(duct).unwrap()));
        // Hide Element, then Apply to View makes it the view's own setting
        app.select_many(vec![duct]);
        app.temporary_hide_isolate(TempMode::Hide, false);
        assert!(app.vis().hidden(app.document.get(duct).unwrap()));
        assert_eq!(app.document.history_mark(), steps);
        app.apply_temporary();
        assert!(app.views.temp.is_none());
        assert_eq!(app.document.view_settings("p10").hidden_elements, vec![duct]);
        assert_eq!(app.document.history_mark(), steps + 1);
        // Isolate Element applied hides everything else
        app.select_many(vec![wall]);
        app.temporary_hide_isolate(TempMode::Isolate, false);
        app.apply_temporary();
        let hidden = app.document.view_settings("p10").hidden_elements;
        assert!(!hidden.contains(&wall) && hidden.contains(&duct));
    }

    #[test]
    fn the_control_bar_changes_scale_and_detail_in_one_step_each() {
        let mut app = Desktop::sample();
        app.view = View::Plan;
        frames(&mut app, 2, &|app, ui| app.view_control_bar(ui));
        let steps = app.document.history_mark();
        let mut vs = app.document.view_settings("p10");
        vs.scale = 50;
        vs.detail = Detail::Coarse;
        assert!(app.apply_view_settings(&vs, "x"));
        assert_eq!(app.document.history_mark(), steps + 1);
        frames(&mut app, 2, &|app, ui| crate::model_view::show(app, ui));
        // all views draw their bar, a schedule has none
        for v in [View::UpperPlan, View::Mechanical, View::ThreeD, View::Elevation(0)] {
            app.view = v;
            frames(&mut app, 2, &|app, ui| app.view_control_bar(ui));
        }
        app.view = View::Schedule(1);
        frames(&mut app, 1, &|app, ui| app.view_control_bar(ui));
    }
}
