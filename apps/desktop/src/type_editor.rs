//! Edit Type for a wall type, as Revit's Type Properties and Edit Assembly: the name, the height, and the layers from
//! the exterior to the interior (function, material, thickness, wraps) with the core between two boundaries. Nothing
//! changes in the model until OK or Apply; then the type and every wall of it change in one undoable step.
use crate::Desktop;
use eframe::egui;
use openbim_document::{Category, Document, ElementId, LayerFunction, WallLayer};

/// Names offered for a layer's material besides those the model already uses. Only names: nothing is known about them.
const MATERIAL_NAMES: [&str; 10] = ["Concrete", "Brick", "Block", "Gypsum Board", "Plaster", "Render", "Insulation", "Air Gap", "Timber", "Steel"];

/// The wall type being edited, as typed so far.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TypeEditor {
    pub type_id: ElementId,
    pub name: String,
    pub height_mm: f64,
    /// the thickness of a type without layers
    pub plain_mm: f64,
    /// the layers, exterior first; none for a plain type
    pub layers: Option<Vec<WallLayer>>,
    /// the first and last layer of the core; the whole wall when `whole_core`
    pub core: (usize, usize),
    pub whole_core: bool,
    /// the layer the Insert, Delete, Up and Down buttons act on
    pub row: usize,
}

fn r6(x: f64) -> f64 { (x * 1e6).round() / 1e6 }

impl TypeEditor {
    pub fn open(doc: &Document, type_id: ElementId) -> Option<Self> {
        let t = doc.get(type_id).filter(|t| t.category == Category::Type && t.type_for == Some(Category::Wall))?;
        let n = t.layers.len();
        Some(Self {
            type_id, name: t.name.clone(), height_mm: t.height_m * 1000.0, plain_mm: t.width_m * 1000.0,
            layers: t.is_layered().then(|| t.layers.clone()),
            core: t.core.unwrap_or((0, n.saturating_sub(1))), whole_core: t.core.is_none(), row: 0,
        })
    }

    /// The thickness the type will have, in metres.
    pub fn thickness_m(&self) -> f64 { self.layers.as_ref().map_or(self.plain_mm / 1000.0, |l| openbim_document::layers_thickness(l)) }

    pub fn core_option(&self) -> Option<(usize, usize)> { if self.whole_core { None } else { Some(self.core) } }

    /// Why this cannot be applied, if it cannot.
    pub fn problem(&self) -> Option<String> {
        if self.name.trim().is_empty() { return Some("The type needs a name".into()); }
        if !(self.height_mm > 0.0) { return Some("The height must be above 0".into()); }
        match &self.layers {
            Some(l) => openbim_document::check_layers(l, self.core_option()).err(),
            None => (!(self.plain_mm > 0.0)).then(|| "The thickness must be above 0".to_string()),
        }
    }

    /// Gives a type without layers its first one, as thick as the type is.
    pub fn add_layers(&mut self) {
        if self.layers.is_none() {
            self.layers = Some(vec![WallLayer::new("Concrete", r6(self.plain_mm / 1000.0), LayerFunction::Structure)]);
            (self.core, self.whole_core, self.row) = ((0, 0), true, 0);
        }
    }

    /// Takes the layers away: the type keeps the thickness they added up to.
    pub fn remove_layers(&mut self) {
        if let Some(l) = self.layers.take() { self.plain_mm = r6(openbim_document::layers_thickness(&l) * 1000.0) ; self.whole_core = true; }
    }

    fn keep_core_on_layers(&mut self) {
        let n = self.layers.as_ref().map_or(0, Vec::len);
        if n == 0 { return; }
        self.core = (self.core.0.min(n - 1), self.core.1.min(n - 1));
        if self.core.0 > self.core.1 { self.whole_core = true; self.core = (0, n - 1); }
        self.row = self.row.min(n - 1);
    }

    /// A new layer under the one picked.
    pub fn insert(&mut self) {
        let Some(l) = self.layers.as_mut() else { return };
        let at = (self.row + 1).min(l.len());
        l.insert(at, WallLayer::new("Concrete", 0.01, LayerFunction::Structure));
        // the core's boundaries stay where they were: a boundary below the new layer moves down with its layer
        if self.core.0 >= at { self.core.0 += 1; }
        if self.core.1 >= at { self.core.1 += 1; }
        self.row = at;
        self.keep_core_on_layers();
    }

    /// Deletes the layer picked; the last one cannot go.
    pub fn delete(&mut self) {
        let Some(l) = self.layers.as_mut() else { return };
        if l.len() <= 1 { return; }
        let at = self.row.min(l.len() - 1);
        l.remove(at);
        if self.core.0 > at { self.core.0 -= 1; }
        if self.core.1 >= at && self.core.1 > 0 { self.core.1 -= 1; }
        self.keep_core_on_layers();
    }

    /// Moves the layer picked one place toward the exterior (`up`) or the interior.
    pub fn shift(&mut self, up: bool) {
        let Some(l) = self.layers.as_mut() else { return };
        let r = self.row.min(l.len().saturating_sub(1));
        let other = if up { r.checked_sub(1) } else { Some(r + 1).filter(|n| *n < l.len()) };
        if let Some(o) = other { l.swap(r, o); self.row = o; }
    }
}

impl Desktop {
    /// Opens Edit Type on a wall type.
    pub(crate) fn open_type_editor(&mut self, type_id: ElementId) {
        match TypeEditor::open(&self.document, type_id) {
            Some(e) => self.type_editor = Some(e),
            None => self.message = "Edit Type is for wall types".into(),
        }
    }

    /// Applies what the window holds to the type and its walls, in one transaction.
    pub(crate) fn apply_type_editor(&mut self, e: &TypeEditor) -> bool {
        if let Some(why) = e.problem() { self.message = format!("Type not changed: {why}"); return false; }
        let layers = e.layers.clone().unwrap_or_default();
        match self.document.edit_wall_type(e.type_id, &e.name, r6(e.height_mm / 1000.0), layers, e.core_option(), r6(e.plain_mm / 1000.0)) {
            Ok(()) => {
                let n = self.document.instances_of(e.type_id).len();
                if self.wall_type == Some(e.type_id) { (self.width_mm, self.height_mm) = (r6(e.thickness_m() * 1000.0), e.height_mm); }
                self.message = format!("Type {} is {:.0} mm thick{}; {n} wall{} follow{}", e.name.trim(), e.thickness_m() * 1000.0, if e.layers.is_some() { " in its layers" } else { "" }, if n == 1 { "" } else { "s" }, if n == 1 { "s" } else { "" });
                true
            }
            Err(error) => { self.message = format!("Type not changed ({error:?}): the name must be new among the wall types"); false }
        }
    }

    /// The Edit Type window.
    pub(crate) fn type_editor_window(&mut self, ctx: &egui::Context) {
        let Some(mut e) = self.type_editor.take() else { return };
        if self.document.get(e.type_id).is_none() { return; }
        let mut open = true;
        let (mut ok, mut apply, mut cancel) = (false, false, false);
        let mut known: Vec<String> = self.document.types_for(Category::Wall).iter().flat_map(|t| t.layers.iter().map(|l| l.material.clone())).collect();
        known.extend(MATERIAL_NAMES.iter().map(|s| s.to_string()));
        known.sort();
        known.dedup();
        let users = self.document.instances_of(e.type_id).len();
        egui::Window::new("Type Properties").open(&mut open).default_width(600.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Family: Basic Wall   Type");
                ui.add(egui::TextEdit::singleline(&mut e.name).desired_width(200.0));
                ui.label(format!("{users} wall{} of this type", if users == 1 { "" } else { "s" }));
            });
            ui.horizontal(|ui| {
                ui.label("Height (mm)");
                ui.add(egui::DragValue::new(&mut e.height_mm).speed(10.0).range(100.0..=30000.0));
            });
            ui.separator();
            if e.layers.is_none() {
                ui.horizontal(|ui| {
                    ui.label("Thickness (mm)");
                    ui.add(egui::DragValue::new(&mut e.plain_mm).speed(5.0).range(10.0..=2000.0));
                    if ui.button("Edit Structure: add layers").on_hover_text("Make the wall of layers, from the exterior to the interior").clicked() { e.add_layers(); }
                });
            } else {
                ui.strong("Structure");
                ui.small("Exterior side (left of the wall's direction) is at the top; the interior side at the bottom.");
                let n = e.layers.as_ref().map_or(0, Vec::len);
                let core = if e.whole_core { (0, n.saturating_sub(1)) } else { e.core };
                egui::Grid::new("layer_table").striped(true).num_columns(6).show(ui, |ui| {
                    for h in ["#", "Function", "Material", "Thickness (mm)", "Wraps", "Core"] { ui.strong(h); }
                    ui.end_row();
                    for i in 0..n {
                        if ui.selectable_label(e.row == i, format!("{}", i + 1)).clicked() { e.row = i; }
                        let layers = e.layers.as_mut().expect("layered");
                        let l = &mut layers[i];
                        egui::ComboBox::from_id_salt(("layer_function", i)).selected_text(l.function.name()).width(130.0).show_ui(ui, |ui| {
                            for f in LayerFunction::ALL { ui.selectable_value(&mut l.function, f, f.name()); }
                        });
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut l.material).desired_width(120.0));
                            ui.menu_button("▾", |ui| {
                                for name in &known { if ui.button(name).clicked() { l.material = name.clone(); ui.close(); } }
                            });
                        });
                        let mut mm = l.thickness_m * 1000.0;
                        if ui.add(egui::DragValue::new(&mut mm).speed(1.0).range(0.0..=5000.0)).changed() { l.thickness_m = r6(mm / 1000.0); }
                        ui.checkbox(&mut l.wraps, "");
                        ui.label(if (core.0..=core.1).contains(&i) { "core" } else { "" });
                        ui.end_row();
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Insert").on_hover_text("A layer under the one picked").clicked() { e.insert(); }
                    if ui.add_enabled(n > 1, egui::Button::new("Delete")).clicked() { e.delete(); }
                    if ui.add_enabled(e.row > 0, egui::Button::new("Up")).clicked() { e.shift(true); }
                    if ui.add_enabled(e.row + 1 < n, egui::Button::new("Down")).clicked() { e.shift(false); }
                    if ui.button("Remove layers").on_hover_text("Back to one thickness").clicked() { e.remove_layers(); }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut e.whole_core, "Whole wall is core");
                    if !e.whole_core {
                        let (mut a, mut b) = (e.core.0 + 1, e.core.1 + 1);
                        ui.label("Core from layer");
                        ui.add(egui::DragValue::new(&mut a).range(1..=n.max(1)));
                        ui.label("to");
                        ui.add(egui::DragValue::new(&mut b).range(1..=n.max(1)));
                        e.core = (a - 1, b - 1);
                    }
                });
            }
            ui.label(format!("Total thickness: {:.1} mm", e.thickness_m() * 1000.0));
            let problem = e.problem();
            if let Some(why) = &problem { ui.colored_label(egui::Color32::from_rgb(200, 60, 60), why); }
            ui.separator();
            ui.horizontal(|ui| {
                let can = problem.is_none();
                if ui.add_enabled(can, egui::Button::new("OK")).clicked() { ok = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
                if ui.add_enabled(can, egui::Button::new("Apply")).clicked() { apply = true; }
            });
        });
        if cancel { open = false; }
        if ok || apply {
            let done = self.apply_type_editor(&e);
            if ok && done { open = false; }
        }
        if open { self.type_editor = Some(e); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use LayerFunction::*;

    fn cavity() -> Vec<WallLayer> {
        vec![WallLayer::new("Render", 0.02, Finish1), WallLayer::new("Brick", 0.1, Structure), WallLayer::new("Insulation", 0.05, ThermalAir), WallLayer::new("Block", 0.15, Structure), WallLayer::new("Plaster", 0.015, Finish2)]
    }

    #[test]
    fn layers_are_inserted_deleted_and_moved_and_the_core_stays_on_its_layers() {
        let mut d = Document::spatial_sample();
        let t = d.add_layered_wall_type("Cavity", 3.0, cavity(), Some((1, 3))).unwrap();
        let mut e = TypeEditor::open(&d, t).unwrap();
        assert_eq!((e.core, e.whole_core, e.problem()), ((1, 3), false, None));
        // a layer under the render pushes the whole core down
        e.row = 0;
        e.insert();
        assert_eq!((e.layers.as_ref().unwrap().len(), e.row, e.core), (6, 1, (2, 4)));
        // deleting it brings the core back
        e.delete();
        assert_eq!((e.layers.as_ref().unwrap().len(), e.core), (5, (1, 3)));
        // deleting the first layer of the core leaves the core one layer shorter
        e.row = 1;
        e.delete();
        assert_eq!((e.layers.as_ref().unwrap()[1].material.as_str(), e.core), ("Insulation", (1, 2)));
        // moving the plaster up swaps it with the block
        e.row = 3;
        e.shift(true);
        assert_eq!((e.row, e.layers.as_ref().unwrap()[2].material.as_str()), (2, "Plaster"));
        // the last layer cannot be deleted
        let mut one = TypeEditor::open(&d, t).unwrap();
        one.layers = Some(vec![WallLayer::new("Concrete", 0.2, Structure)]);
        one.whole_core = true;
        one.delete();
        assert_eq!(one.layers.as_ref().unwrap().len(), 1);
        // a core with no structure is a problem the window shows
        let mut bad = TypeEditor::open(&d, t).unwrap();
        bad.core = (0, 0);
        assert!(bad.problem().unwrap().contains("structure"));
        bad.name = " ".into();
        assert!(bad.problem().unwrap().contains("name"));
    }

    #[test]
    fn a_plain_type_is_given_layers_and_applying_changes_the_type_its_walls_and_the_wall_tool_in_one_undo() {
        let mut app = Desktop::sample();
        let level = ElementId(10);
        let t = app.document.add_wall_type("Plain 200", 0.2, 3.0).unwrap();
        let w = app.document.place_wall_typed(level, [20.0, 0.0], [26.0, 0.0], 0.2, 3.0, 0.0, &[], Some(t)).unwrap();
        app.wall_type = Some(t);
        app.open_type_editor(t);
        let mut e = app.type_editor.clone().unwrap();
        assert!(e.layers.is_none() && (e.plain_mm - 200.0).abs() < 1e-9);
        e.add_layers();
        assert_eq!(e.layers.as_ref().unwrap().len(), 1);
        assert!((e.thickness_m() - 0.2).abs() < 1e-12, "the first layer is as thick as the wall was");
        e.layers.as_mut().unwrap()[0].thickness_m = 0.15;
        e.row = 0;
        e.insert();
        e.layers.as_mut().unwrap()[1] = WallLayer::new("Plaster", 0.015, Finish2);
        e.name = "Block 165".into();
        let revision = app.document.revision();
        assert!(app.apply_type_editor(&e), "{}", app.message);
        assert_eq!(app.document.revision(), revision + 1);
        assert!((app.document.get(w).unwrap().width_m - 0.165).abs() < 1e-12 && app.document.get(t).unwrap().name == "Block 165");
        assert!((app.width_mm - 165.0).abs() < 1e-9, "the wall tool draws the new thickness");
        assert!(app.message.contains("165 mm") && app.message.contains("1 wall follows"), "{}", app.message);
        // the location line follows the layers: the finish face on the exterior is half the thickness from the axis
        app.wall_loc = 2;
        let (a, b) = app.wall_axis([0.0, 0.0], [4.0, 0.0]);
        assert!((a[1] + 0.0825).abs() < 1e-9 && (b[1] + 0.0825).abs() < 1e-9, "{a:?}");
        // core: the block (layer 1) only; its exterior face is on the line
        let mut e2 = TypeEditor::open(&app.document, t).unwrap();
        e2.whole_core = false;
        e2.core = (0, 0);
        assert!(app.apply_type_editor(&e2));
        app.wall_loc = 4;
        let (a, _) = app.wall_axis([0.0, 0.0], [4.0, 0.0]);
        assert!((a[1] - (0.0 - 0.0825)).abs() < 1e-9, "the core face is on the exterior face of this wall: {a:?}");
        app.wall_loc = 5;
        let (a, _) = app.wall_axis([0.0, 0.0], [4.0, 0.0]);
        assert!((a[1] - (0.15 - 0.0825)).abs() < 1e-9, "{a:?}");
        // a problem is refused with the reason and nothing changes
        let mut bad = TypeEditor::open(&app.document, t).unwrap();
        bad.layers.as_mut().unwrap()[0].material = String::new();
        let revision = app.document.revision();
        assert!(!app.apply_type_editor(&bad) && app.message.contains("not changed"), "{}", app.message);
        assert_eq!(app.document.revision(), revision);
        // layers away again: a plain type of the same thickness
        let mut plain = TypeEditor::open(&app.document, t).unwrap();
        plain.remove_layers();
        assert!(app.apply_type_editor(&plain));
        assert!(app.document.get(t).unwrap().layers.is_empty() && (app.document.get(w).unwrap().width_m - 0.165).abs() < 1e-9);
        // not a wall type
        app.open_type_editor(w);
        assert!(app.message.contains("wall types"));
    }

    #[test]
    fn the_window_draws_with_layers_and_applies_with_ok() {
        let mut app = Desktop::sample();
        let t = app.document.add_layered_wall_type("Cavity", 3.0, cavity(), Some((1, 3))).unwrap();
        app.open_type_editor(t);
        let ctx = egui::Context::default();
        for _ in 0..3 { ctx.run_ui(Default::default(), |ui| app.type_editor_window(ui.ctx())).textures_delta.clear(); }
        assert!(app.type_editor.is_some(), "the window stays open while it is not closed");
        // a type deleted meanwhile closes the window
        app.type_editor.as_mut().unwrap().type_id = ElementId(999_999);
        ctx.run_ui(Default::default(), |ui| app.type_editor_window(ui.ctx())).textures_delta.clear();
        assert!(app.type_editor.is_none());
    }
}
