//! Systems of equipment and Generate Layout, as Revit's Systems tab has them: select equipment and choose Mechanical
//! System / Piping System (a type and a name), Edit it in the System Browser (add or remove the equipment selected, choose
//! the base), then Generate Layout: numbered solutions are shown as placeholders in the plan, the arrows step through
//! them, the trunk size can be changed, and Finish Layout turns the chosen one into ducts, pipes and fittings joined to
//! the equipment connectors in one undoable step. The placeholders are not saved with the model.
use crate::Desktop;
use eframe::egui;
use openbim_document::layout::{Layout, LayoutConn};
use openbim_document::{Category, Command, ElementId, MepSystemDef};
use openbim_family::mep::{Domain, System};
use openbim_place::penetrations::RunSize;

const AIR: [System; 4] = [System::SupplyAir, System::ReturnAir, System::ExhaustAir, System::OutsideAir];
const PIPING: [System; 12] = [
    System::ChilledWaterSupply, System::ChilledWaterReturn, System::HeatingWaterSupply, System::HeatingWaterReturn, System::CondenserWaterSupply, System::CondenserWaterReturn,
    System::DomesticCold, System::DomesticHot, System::Sanitary, System::Vent, System::StormWater, System::FireProtection,
];

/// Create Mechanical / Piping System: the type and name asked for, and the equipment selected.
pub(crate) struct CreateSystemDialog {
    pub pipe: bool,
    pub system: System,
    pub name: String,
    pub members: Vec<ElementId>,
}

/// Generate Layout in progress: the solutions found and the one on show.
pub(crate) struct LayoutSession {
    pub name: String,
    pub system: System,
    pub level: ElementId,
    pub solutions: Vec<Layout>,
    pub why: Vec<String>,
    pub current: usize,
    /// the trunk size asked for, in mm (width and height of a duct, diameter of a pipe); None: the largest connector's
    pub trunk_mm: Option<[f64; 2]>,
}

pub(crate) enum BrowserAct { Add(String), Remove(String), Select(String), Base(String, Option<ElementId>), Layout(String), Delete(String), Rename(String, String) }

#[derive(Default)]
pub(crate) struct SystemsUi {
    pub create: Option<CreateSystemDialog>,
    pub browser: bool,
    pub layout: Option<LayoutSession>,
}

fn domain_of(system: System) -> Domain { if system.domain_ok(Domain::Duct) && system != System::Other { Domain::Duct } else { Domain::Pipe } }

fn size_mm(s: RunSize) -> [f64; 2] {
    match s { RunSize::Round { diameter } => [(diameter * 1000.0).round(), 0.0], RunSize::Rect { width, height, .. } => [(width * 1000.0).round(), (height * 1000.0).round()] }
}

impl Desktop {
    /// The Mechanical System / Piping System button: opens the window for the equipment selected.
    pub(crate) fn open_create_system(&mut self, pipe: bool) {
        let members: Vec<ElementId> = self.selected_ids().into_iter().filter(|i| self.document.get(*i).is_some_and(|e| e.category == Category::Family)).collect();
        if members.is_empty() {
            self.message = format!("{} System: select the equipment first (click one, or drag a box), then choose {} System", if pipe { "Piping" } else { "Mechanical" }, if pipe { "Piping" } else { "Mechanical" });
            return;
        }
        self.systems.create = Some(CreateSystemDialog { pipe, system: if pipe { System::ChilledWaterSupply } else { System::SupplyAir }, name: String::new(), members });
    }

    pub(crate) fn create_system_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.systems.create.take() else { return };
        d.members.retain(|m| self.document.get(*m).is_some());
        let (mut open, mut ok, mut cancel) = (true, false, false);
        let list: &[System] = if d.pipe { &PIPING } else { &AIR };
        let hint = self.document.default_system_name(d.system);
        egui::Window::new(if d.pipe { "Create Piping System" } else { "Create Mechanical System" }).id(egui::Id::new("create_system_window")).open(&mut open).default_width(340.0).show(ctx, |ui| {
            ui.label(format!("{} piece{} of equipment selected", d.members.len(), if d.members.len() == 1 { "" } else { "s" }));
            ui.horizontal(|ui| {
                ui.label("System type");
                egui::ComboBox::from_id_salt("system_type").selected_text(d.system.name()).show_ui(ui, |ui| {
                    for s in list { ui.selectable_value(&mut d.system, *s, s.name()); }
                });
            });
            ui.horizontal(|ui| {
                ui.label("System name");
                ui.add(egui::TextEdit::singleline(&mut d.name).hint_text(hint.as_str()).desired_width(180.0));
            });
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() { ok = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
            });
        });
        let mut keep = open && !cancel;
        if ok && self.confirm_create_system(&d) { keep = false; }
        if keep { self.systems.create = Some(d); }
    }

    /// OK of the Create System window.
    pub(crate) fn confirm_create_system(&mut self, d: &CreateSystemDialog) -> bool {
        match self.document.create_system_commands(&d.name, d.system, &d.members) {
            Ok(ops) => match self.document.transact(self.document.revision(), &ops) {
                Ok(_) => {
                    let name = self.document.mep().systems.last().map(|s| s.name.clone()).unwrap_or_default();
                    self.message = format!("System \"{name}\" made from {} piece(s) of equipment; open the System Browser to edit it, or choose Generate Layout; undo takes it back", d.members.len());
                    self.systems.browser = true;
                    true
                }
                Err(e) => { self.message = format!("System rejected ({e:?})"); false }
            },
            Err(e) => { self.message = format!("System: {e}"); false }
        }
    }

    fn apply_system(&mut self, ops: Result<Vec<Command>, String>, done: &str) -> bool {
        match ops {
            Ok(ops) => match self.document.transact(self.document.revision(), &ops) {
                Ok(_) => { self.message = format!("{done}; undo takes it back"); true }
                Err(e) => { self.message = format!("System rejected ({e:?})"); false }
            },
            Err(e) => { self.message = format!("System: {e}"); false }
        }
    }

    /// The connectors of the system's type on its members, one per piece of equipment, and the members that have none.
    pub(crate) fn system_connectors(&self, def: &MepSystemDef) -> (Vec<LayoutConn>, Vec<ElementId>) {
        let (mut conns, mut without) = (Vec::new(), Vec::new());
        for m in &def.members {
            match crate::elec_ui::instance_connectors(&self.document, *m).into_iter().find(|c| c.system == def.system && c.domain == domain_of(def.system)) {
                Some(conn) => conns.push(LayoutConn { element: *m, conn }),
                None => without.push(*m),
            }
        }
        (conns, without)
    }

    /// Whether a connector of a piece of equipment has a duct or pipe joined to it.
    fn connector_joined(&self, element: ElementId, name: &str) -> bool {
        self.document.mep().equip_links.iter().any(|q| q.element == element && q.connector == name)
    }

    /// The Generate Layout button: for the system of the equipment selected, or the only system there is.
    pub(crate) fn open_generate_layout(&mut self) {
        let systems = self.document.mep().systems.clone();
        if systems.is_empty() { self.message = "Generate Layout: make a system first (select the equipment, then Mechanical System or Piping System)".into(); return; }
        let selected = self.selected_ids();
        let of_selection: Vec<&MepSystemDef> = systems.iter().filter(|s| s.members.iter().any(|m| selected.contains(m))).collect();
        let pick = match (of_selection.len(), systems.len()) {
            (1, _) => Some(of_selection[0].name.clone()),
            (0, 1) => Some(systems[0].name.clone()),
            _ => None,
        };
        match pick {
            Some(n) => self.start_layout(&n),
            None => { self.systems.browser = true; self.message = "Generate Layout: choose the system in the System Browser (its Generate Layout button)".into(); }
        }
    }

    /// Works out the solutions for a system and shows the first.
    pub(crate) fn start_layout(&mut self, name: &str) {
        self.generate_layout_for(name, None, 0);
    }

    fn generate_layout_for(&mut self, name: &str, trunk_mm: Option<[f64; 2]>, keep: usize) {
        let Some(def) = self.document.mep().systems.iter().find(|s| s.name == name).cloned() else { self.message = format!("Generate Layout: there is no system \"{name}\""); return };
        let level = self.systems.layout.as_ref().map_or_else(|| self.plan_level(), |l| l.level);
        let (conns, without) = self.system_connectors(&def);
        if conns.len() < 2 {
            self.message = format!("Generate Layout: \"{name}\" has {} connector{} of type {}; a layout needs two or more (the equipment must have one of that type)", conns.len(), if conns.len() == 1 { "" } else { "s" }, def.system.name());
            return;
        }
        // the base: the one chosen, else a connector that supplies, else the first
        let base = def.base.and_then(|b| conns.iter().position(|c| c.element == b))
            .or_else(|| conns.iter().position(|c| c.conn.flow_dir == openbim_family::mep::FlowDir::Out)).unwrap_or(0);
        let domain = domain_of(def.system);
        let prefs = crate::mep_tool::route_prefs(domain, def.system);
        let trunk = trunk_mm.map(|[w, h]| if domain == Domain::Pipe { RunSize::Round { diameter: w / 1000.0 } } else { RunSize::Rect { width: w / 1000.0, height: h / 1000.0, roll: 0.0 } });
        match self.document.generate_layouts(level, &conns, base, trunk, &prefs) {
            Ok((solutions, why)) => {
                let n = solutions.len();
                let current = keep.min(n - 1);
                let note = if without.is_empty() { String::new() } else { format!("; {} member{} without a connector of that type", without.len(), if without.len() == 1 { " is" } else { "s are" }) };
                self.message = format!("Generate Layout: {n} solution{} for \"{name}\"; the arrows step through them, Finish Layout places the one shown{note}", if n == 1 { "" } else { "s" });
                self.systems.layout = Some(LayoutSession { name: name.to_string(), system: def.system, level, solutions, why, current, trunk_mm });
            }
            Err(e) => { self.message = format!("Generate Layout: {e}"); self.systems.layout = None; }
        }
    }

    /// Finish Layout: the solution on show becomes ducts or pipes, one undoable step.
    pub(crate) fn finish_layout(&mut self) {
        let Some(s) = self.systems.layout.take() else { return };
        let Some(layout) = s.solutions.get(s.current) else { return };
        let prefs = crate::mep_tool::route_prefs(domain_of(s.system), s.system);
        match self.document.place_layout(s.level, layout, &prefs) {
            Ok(r) => {
                let what = if domain_of(s.system) == Domain::Pipe { "pipes" } else { "ducts" };
                let left = if r.failed.is_empty() { String::new() } else { format!("; {} connector(s) left out: {}", r.failed.len(), r.failed[0].1) };
                let message = format!("Layout placed: {} {what} joined to {} connector(s){left}; undo takes it all back", r.runs.len(), r.connected);
                self.select_many(r.runs);
                self.message = message;
            }
            Err(e) => { self.message = format!("Finish Layout: {e}"); self.systems.layout = Some(s); }
        }
    }

    /// The window of Generate Layout (Revit's Layout tab: Solution Type, the arrows, Finish).
    pub(crate) fn layout_window(&mut self, ctx: &egui::Context) {
        let Some(mut s) = self.systems.layout.take() else { return };
        let (mut finish, mut cancel, mut regenerate) = (false, false, false);
        let mut open = true;
        let pipe = domain_of(s.system) == Domain::Pipe;
        let shown = s.solutions[s.current].trunk.sizes.iter().copied().map(size_mm).fold([0.0f64, 0.0], |a, b| [a[0].max(b[0]), a[1].max(b[1])]);
        let mut size = s.trunk_mm.unwrap_or(shown);
        egui::Window::new(format!("Generate Layout: {}", s.name)).id(egui::Id::new("layout_window")).open(&mut open).default_width(420.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("◀").clicked() { s.current = (s.current + s.solutions.len() - 1) % s.solutions.len(); }
                ui.label(format!("{} of {}", s.current + 1, s.solutions.len()));
                if ui.button("▶").clicked() { s.current = (s.current + 1) % s.solutions.len(); }
            });
            ui.label(s.solutions[s.current].label.clone());
            ui.horizontal(|ui| {
                ui.label(if pipe { "Trunk diameter (mm)" } else { "Trunk width x height (mm)" });
                let mut ch = ui.add(egui::DragValue::new(&mut size[0]).speed(1.0).range(10.0..=3000.0)).changed();
                if !pipe { ch |= ui.add(egui::DragValue::new(&mut size[1]).speed(1.0).range(10.0..=3000.0)).changed(); }
                if ch { regenerate = true; }
            });
            let l = &s.solutions[s.current];
            ui.small(format!("{} branch{}; branches take the size of their connector", l.branches.len(), if l.branches.len() == 1 { "" } else { "es" }));
            for (el, why) in &l.unconnected {
                let name = self.document.get(*el).map_or("equipment".to_string(), |e| e.name.clone());
                ui.colored_label(egui::Color32::from_rgb(200, 90, 40), format!("{name}: {why}"));
            }
            for w in &s.why { ui.small(w.clone()); }
            ui.small("Placeholders are shown dashed in the plan and are not saved until Finish Layout.");
            ui.horizontal(|ui| {
                if ui.button("Finish Layout").clicked() { finish = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
            });
        });
        if regenerate {
            let (name, keep) = (s.name.clone(), s.current);
            self.systems.layout = Some(s);
            self.generate_layout_for(&name, Some(size), keep);
            return;
        }
        if finish { self.systems.layout = Some(s); self.finish_layout(); return; }
        if open && !cancel { self.systems.layout = Some(s); } else { self.message = "Generate Layout cancelled: nothing was placed".into(); }
    }

    /// The placeholders of the solution on show in the plan of `level`: the trunk and each branch, as plan polylines.
    pub(crate) fn layout_placeholders(&self, level: ElementId) -> Vec<(bool, Vec<[f64; 2]>)> {
        let Some(s) = self.systems.layout.as_ref().filter(|s| s.level == level) else { return Vec::new() };
        let Some(l) = s.solutions.get(s.current) else { return Vec::new() };
        let plan = |p: &[[f64; 3]]| p.iter().map(|v| [v[0], v[1]]).collect::<Vec<_>>();
        std::iter::once((true, plan(&l.trunk.points))).chain(l.branches.iter().map(|b| (false, plan(&b.path.points)))).collect()
    }

    /// A button of the System Browser.
    pub(crate) fn run_browser_act(&mut self, act: BrowserAct, selected: &[ElementId]) {
        match act {
            BrowserAct::Add(n) => { let ops = self.document.edit_system_commands(&n, selected, &[]); self.apply_system(ops, &format!("Added the selection to \"{n}\"")); }
            BrowserAct::Remove(n) => { let ops = self.document.edit_system_commands(&n, &[], selected); self.apply_system(ops, &format!("Took the selection out of \"{n}\"")); }
            BrowserAct::Select(n) => { if let Some(d) = self.document.mep().systems.iter().find(|s| s.name == n) { let m = d.members.clone(); self.select_many(m); } }
            BrowserAct::Base(n, b) => { let ops = self.document.set_system_base_commands(&n, b); self.apply_system(ops, &format!("Base of \"{n}\" set")); }
            BrowserAct::Layout(n) => self.start_layout(&n),
            BrowserAct::Delete(n) => { let ops = self.document.delete_system_commands(&n); self.apply_system(ops, &format!("System \"{n}\" deleted (its equipment and pipes stay)")); }
            BrowserAct::Rename(n, new) => { let ops = self.document.rename_system_commands(&n, &new); self.apply_system(ops, &format!("\"{n}\" is now \"{new}\"")); }
        }
    }

    /// The System Browser: every system with its equipment, whether each connector has something joined to it, and the
    /// buttons of Edit System.
    pub(crate) fn system_browser_window(&mut self, ctx: &egui::Context) {
        if !self.systems.browser { return; }
        let mut open = true;
        let selected = self.selected_ids();
        let systems = self.document.mep().systems.clone();
        let mut act: Option<BrowserAct> = None;
        egui::Window::new("System Browser").id(egui::Id::new("system_browser_window")).open(&mut open).default_width(460.0).show(ctx, |ui| {
            if systems.is_empty() { ui.label("No systems yet: select equipment and choose Mechanical System or Piping System."); }
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                for def in &systems {
                    ui.separator();
                    let (conns, without) = self.system_connectors(def);
                    let open_ends = conns.iter().filter(|c| !self.connector_joined(c.element, &c.conn.name)).count();
                    ui.strong(format!("{} ({})", def.name, def.system.name()));
                    ui.small(format!("{} members, {} connector{} not yet joined{}", def.members.len(), open_ends, if open_ends == 1 { "" } else { "s" }, if without.is_empty() { String::new() } else { format!(", {} without a connector of this type", without.len()) }));
                    for m in &def.members {
                        let name = self.document.get(*m).map_or("?".to_string(), |e| e.name.clone());
                        let joined = conns.iter().find(|c| c.element == *m).map(|c| if self.connector_joined(c.element, &c.conn.name) { "joined" } else { "open" });
                        ui.horizontal(|ui| {
                            ui.label(format!("{name}{}", if def.base == Some(*m) { "  (base)" } else { "" }));
                            ui.small(joined.unwrap_or("no connector of this type"));
                        });
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Add selected").on_hover_text("Edit System > Add to System").clicked() { act = Some(BrowserAct::Add(def.name.clone())); }
                        if ui.button("Remove selected").on_hover_text("Edit System > Remove from System").clicked() { act = Some(BrowserAct::Remove(def.name.clone())); }
                        if ui.button("Select members").clicked() { act = Some(BrowserAct::Select(def.name.clone())); }
                        if ui.button("Generate Layout").clicked() { act = Some(BrowserAct::Layout(def.name.clone())); }
                        if ui.button("Delete system").clicked() { act = Some(BrowserAct::Delete(def.name.clone())); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Base");
                        let current = def.base.and_then(|b| self.document.get(b)).map_or("automatic".to_string(), |e| e.name.clone());
                        egui::ComboBox::from_id_salt(format!("base_{}", def.name)).selected_text(current).show_ui(ui, |ui| {
                            if ui.selectable_label(def.base.is_none(), "automatic").clicked() { act = Some(BrowserAct::Base(def.name.clone(), None)); }
                            for m in &def.members {
                                let name = self.document.get(*m).map_or("?".to_string(), |e| e.name.clone());
                                if ui.selectable_label(def.base == Some(*m), name).clicked() { act = Some(BrowserAct::Base(def.name.clone(), Some(*m))); }
                            }
                        });
                        let mut name = def.name.clone();
                        let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(150.0));
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && name != def.name { act = Some(BrowserAct::Rename(def.name.clone(), name)); }
                    });
                }
            });
        });
        if !open { self.systems.browser = false; }
        if let Some(a) = act { self.run_browser_act(a, &selected); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Pos2, Rect, Vec2};
    use openbim_document::{FamilySpec, Geometry};

    fn fan_coils(app: &mut Desktop, at: &[[f64; 2]]) -> Vec<ElementId> {
        let level = app.plan_level();
        at.iter().map(|p| app.document.place_family(level, *p, &FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 }).unwrap()).collect()
    }
    fn count(app: &Desktop, c: Category) -> usize { app.document.elements().filter(|e| e.category == c).count() }

    #[test]
    fn equipment_becomes_a_system_which_is_laid_out_and_finished_into_pipes_joined_to_its_connectors() {
        let mut app = Desktop::sample();
        // without a system, or without a selection, the buttons say what to do first
        app.open_create_system(true);
        assert!(app.systems.create.is_none() && app.message.contains("select the equipment first"), "{}", app.message);
        app.open_generate_layout();
        assert!(app.systems.layout.is_none() && app.message.contains("make a system first"), "{}", app.message);
        let fc = fan_coils(&mut app, &[[20.0, 20.0], [20.0, 23.0], [20.0, 26.0]]);
        app.select_many(fc.clone());
        app.open_create_system(true);
        let mut d = app.systems.create.take().expect("the window opens for the equipment selected");
        assert_eq!(d.members, fc);
        d.name = "CHW-1".into();
        d.system = System::ChilledWaterSupply;
        assert!(app.confirm_create_system(&d), "{}", app.message);
        assert_eq!(app.document.mep().systems.len(), 1);
        assert!(app.systems.browser, "the System Browser opens to edit it");
        // the connectors of the system's type are found on each piece, none of them joined yet
        let def = app.document.mep().systems[0].clone();
        let (conns, without) = app.system_connectors(&def);
        assert_eq!((conns.len(), without.len()), (3, 0));
        assert!(conns.iter().all(|c| c.conn.name == "chw_in" && !app.connector_joined(c.element, &c.conn.name)));
        // Generate Layout for the system of the selection
        let pipes = count(&app, Category::Pipe);
        app.open_generate_layout();
        let s = app.systems.layout.as_ref().unwrap_or_else(|| panic!("{}", app.message));
        assert!(!s.solutions.is_empty() && s.solutions[0].label.starts_with("Solution 1: network"), "{}", s.solutions[0].label);
        let level = app.plan_level();
        let ph = app.layout_placeholders(level);
        assert!(ph.iter().any(|(trunk, p)| *trunk && p.len() >= 3) && ph.iter().filter(|(t, _)| !*t).count() == 1, "{ph:?}");
        assert_eq!(count(&app, Category::Pipe), pipes, "placeholders are not in the model");
        // another trunk size regenerates the solutions and the trunk takes it
        app.generate_layout_for("CHW-1", Some([80.0, 0.0]), 0);
        let s = app.systems.layout.as_ref().unwrap();
        assert!(s.solutions[0].trunk.sizes.iter().any(|z| matches!(z, RunSize::Round { diameter } if (*diameter - 0.08).abs() < 1e-9)));
        // Finish Layout: pipes appear, joined to the three connectors, selected, one undo step
        let steps = app.document.history_mark();
        app.finish_layout();
        assert!(app.systems.layout.is_none() && app.message.starts_with("Layout placed"), "{}", app.message);
        assert!(count(&app, Category::Pipe) >= pipes + 4, "{}", count(&app, Category::Pipe));
        assert!(conns.iter().all(|c| app.connector_joined(c.element, &c.conn.name)));
        assert_eq!(app.document.history_mark(), steps + 1);
        assert!(app.selected_ids().len() >= 4);
        app.document.undo().unwrap();
        assert_eq!(count(&app, Category::Pipe), pipes);
        assert!(conns.iter().all(|c| !app.connector_joined(c.element, &c.conn.name)));
        assert_eq!(app.document.mep().systems.len(), 1, "the system stays");
    }

    #[test]
    fn the_browser_edits_the_system_and_its_windows_draw() {
        let mut app = Desktop::sample();
        let fc = fan_coils(&mut app, &[[20.0, 20.0], [20.0, 23.0], [20.0, 26.0]]);
        app.select_many(fc[..2].to_vec());
        app.open_create_system(true);
        let d = app.systems.create.take().unwrap();
        assert!(app.confirm_create_system(&d));
        let name = app.document.mep().systems[0].name.clone();
        assert_eq!(name, "Chilled water supply 1");
        // Add selected / Remove selected / Base / Rename / Select members
        app.run_browser_act(BrowserAct::Add(name.clone()), &fc[2..]);
        assert_eq!(app.document.mep().systems[0].members.len(), 3, "{}", app.message);
        app.run_browser_act(BrowserAct::Base(name.clone(), Some(fc[1])), &[]);
        assert_eq!(app.document.mep().systems[0].base, Some(fc[1]));
        app.run_browser_act(BrowserAct::Remove(name.clone()), &fc[1..2]);
        assert_eq!(app.document.mep().systems[0].members, vec![fc[0], fc[2]]);
        assert_eq!(app.document.mep().systems[0].base, None);
        app.run_browser_act(BrowserAct::Rename(name.clone(), "CHW-A".into()), &[]);
        app.run_browser_act(BrowserAct::Select("CHW-A".into()), &[]);
        assert_eq!(app.selected_ids(), vec![fc[0], fc[2]]);
        // a bad edit says why and changes nothing
        app.run_browser_act(BrowserAct::Remove("CHW-A".into()), &fc);
        assert!(app.message.contains("at least one member"), "{}", app.message);
        // the windows draw (a new window needs two frames)
        app.systems.browser = true;
        app.run_browser_act(BrowserAct::Layout("CHW-A".into()), &[]);
        assert!(app.systems.layout.is_some(), "{}", app.message);
        app.open_create_system(true);
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0))), ..Default::default() }, |ui| {
                let c = ui.ctx().clone();
                app.create_system_window(&c);
                app.system_browser_window(&c);
                app.layout_window(&c);
            });
            out.textures_delta.clear();
        }
        assert!(app.systems.layout.is_some() && app.systems.create.is_some() && app.systems.browser);
        // deleting the system leaves the equipment
        app.run_browser_act(BrowserAct::Delete("CHW-A".into()), &[]);
        assert!(app.document.mep().systems.is_empty() && count(&app, Category::Family) >= 3);
        let _ = Geometry::Level;
    }

    #[test]
    fn systems_go_out_in_the_ifc_file_and_come_back_and_the_import_does_not_double_them() {
        let mut app = Desktop::sample();
        let fc = fan_coils(&mut app, &[[20.0, 20.0], [20.0, 23.0]]);
        app.select_many(fc.clone());
        app.open_create_system(true);
        let mut d = app.systems.create.take().unwrap();
        d.name = "CHW, \"north\"; 1".into();
        assert!(app.confirm_create_system(&d));
        app.run_browser_act(BrowserAct::Base(d.name.clone(), Some(fc[1])), &[]);
        let out = crate::export::export_ifc_as(&app.document, "P", 1, "2026-10-19T00:00:00", crate::export::Schema::Ifc4).unwrap();
        let mut back = openbim_document::Document::new();
        crate::import::import_ifc(&mut back, &out.text).unwrap();
        assert_eq!(back.mep().systems, app.document.mep().systems);
        // importing the same file into the model that has it adds nothing
        crate::import::import_ifc(&mut app.document, &out.text).unwrap();
        assert_eq!(app.document.mep().systems.len(), 1);
    }

    #[test]
    fn a_mechanical_system_of_ducted_units_gets_a_duct_trunk_along_the_row() {
        let mut app = Desktop::sample();
        let fc = fan_coils(&mut app, &[[20.0, 20.0], [23.0, 20.0], [26.0, 20.0]]);
        app.select_many(fc.clone());
        app.open_create_system(false);
        let d = app.systems.create.take().unwrap();
        assert_eq!(d.system, System::SupplyAir);
        assert!(app.confirm_create_system(&d), "{}", app.message);
        let ducts = count(&app, Category::Duct);
        app.open_generate_layout();
        let s = app.systems.layout.as_ref().unwrap_or_else(|| panic!("{}", app.message));
        let along_x = s.solutions.iter().position(|l| l.label.contains("trunk along X")).unwrap_or_else(|| panic!("{:?} {:?}", s.solutions.iter().map(|l| &l.label).collect::<Vec<_>>(), s.why));
        assert!(s.solutions.iter().all(|l| l.unconnected.is_empty()), "{:?}", s.solutions.iter().map(|l| &l.unconnected).collect::<Vec<_>>());
        app.systems.layout.as_mut().unwrap().current = along_x;
        app.finish_layout();
        assert!(app.message.starts_with("Layout placed"), "{}", app.message);
        assert!(count(&app, Category::Duct) >= ducts + 4);
        assert_eq!(app.document.mep().equip_links.iter().filter(|q| fc.contains(&q.element)).count(), 3);
        assert!(app.document.elements().filter(|e| e.category == Category::Duct).all(|e| app.document.mep().run_system(e.id).is_none_or(|s| s == System::SupplyAir)));
    }
}
