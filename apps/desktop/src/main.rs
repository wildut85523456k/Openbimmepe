//! First executable desktop prototype. The viewport is a clearly labelled SAMPLE scene:
//! no model persistence, IFC import, live routing, or central collaboration is implied.
#![forbid(unsafe_code)]

mod icons;
mod ribbon;
mod panels;
mod model_view;
mod model_edit;
mod automation;
mod automation_canvas;
mod automation_icons;

use openbim_document::{Document, ElementId};
use eframe::egui::{self, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Workspace { Design, Coordination, Automation }
#[derive(Clone, Copy, PartialEq, Eq)]
enum View { Plan, UpperPlan, ThreeD, Mechanical }
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool { Select, Duct, Pipe, Wall }
#[derive(Clone, Copy, PartialEq, Eq)]
enum SampleElement { Duct, Model(ElementId) }

struct Desktop {
    automation: automation::Automation,
    document: Document,
    workspace: Workspace,
    view: View,
    tool: Tool,
    selected: Option<SampleElement>,
    ribbon: &'static str,
    systems_sub: &'static str,
    width_mm: f64,
    height_mm: f64,
    elevation_mm: f64,
    slope_percent: f64,
    axis_mm: [f64; 4],
    camera_yaw: f32,
    camera_pitch: f32,
    property_query: String,
    browser_query: String,
    browser_selected: String,
    message: String,
    show_properties: bool,
    show_browser: bool,
    show_help: bool,
    dark: bool,
    zoom: f32,
    pan: Vec2,
    keymap: openbim_keymap::Keymap,
}

impl Desktop {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        Self::sample()
    }

    fn sample() -> Self {
        let mut keymap = openbim_keymap::Keymap::new(&["Select", "Duct", "Pipe", "Wall"])
            .expect("static shortcut commands");
        keymap.set_preset(&[
            ("Duct", &["D D"]),
            ("Pipe", &["P I"]),
            ("Wall", &["W A"]),
        ]).expect("static shortcuts");
        let document = Document::spatial_sample();
        Self {
            automation: automation::Automation::default(),
            document,
            workspace: Workspace::Design, view: View::Mechanical, tool: Tool::Select,
            selected: Some(SampleElement::Duct), ribbon: "Systems", systems_sub: "HVAC",
            width_mm: 200.0, height_mm: 300.0, elevation_mm: 2700.0,
            slope_percent: 0.0,
            axis_mm: [1000.0, 2000.0, 7000.0, 2000.0],
            camera_yaw: 0.7, camera_pitch: 0.6,
            property_query: String::new(), browser_query: String::new(),
            browser_selected: String::new(),
            message: "Sample scene - no BIM edits are saved".into(),
            show_properties: true, show_browser: true, show_help: false, dark: false,
            zoom: 1.0, pan: Vec2::ZERO, keymap,
        }
    }

    fn choose_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.message = match tool {
            Tool::Select => "Select a sample element. Tab cycles; Esc cancels.".into(),
            Tool::Duct => "Duct command preview only; routing engine is not connected yet.".into(),
            Tool::Pipe => "Pipe command preview only; routing engine is not connected yet.".into(),
            Tool::Wall => "Wall placement is not connected yet; select an existing wall to edit it.".into(),
        };
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("OpenBIM").strong());
            ui.separator();
            // Quick access toolbar: common commands with the 3D icons.
            for (id, label) in ribbon::QUICK_ACCESS {
                let img = ribbon::icon_image(ui, id, self.dark, Vec2::splat(24.0));
                if ui.add(egui::Button::image(img).frame(false)).on_hover_text(*label).clicked() {
                    self.run_command(id, label);
                }
            }
            ui.separator();
            ui.label("Project1 (SAMPLE)");
            ui.separator();
            for (label, ws) in [
                ("Design", Workspace::Design), ("Coordination", Workspace::Coordination),
                ("Automation", Workspace::Automation),
            ] {
                if ui.selectable_label(self.workspace == ws, label).clicked() {
                    self.workspace = ws;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(if self.dark { "Light" } else { "Dark" }).clicked() {
                    self.dark = !self.dark;
                    ui.ctx().set_visuals(if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() });
                }
                if ui.button("Help").clicked() { self.show_help = !self.show_help; }
            });
        });
    }

    /// Commands without an implementation yet report it in the status bar.
    fn run_command(&mut self, id: &str, label: &str) {
        match id {
            "automation_editor" | "automation_player" => {
                self.workspace = Workspace::Automation;
                self.automation.player = id == "automation_player";
            }
            "automation_mcp" | "automation_snapshot" => {
                self.workspace = Workspace::Automation;
                self.automation.show_mcp = true;
                if id == "automation_snapshot" { self.automation.export_snapshot(&self.document); }
            }
            "duct" => self.choose_tool(Tool::Duct),
            "pipe" => self.choose_tool(Tool::Pipe),
            "wall" => self.choose_tool(Tool::Wall),
            "select" => self.choose_tool(Tool::Select),
            "undo" => self.undo_document(),
            "zoom_in" => self.zoom = (self.zoom * 1.25).min(4.0),
            "zoom_out" => self.zoom = (self.zoom / 1.25).max(0.3),
            _ => self.message = format!("{label} is not implemented in this UI prototype"),
        }
    }

    fn ribbon(&mut self, ui: &mut egui::Ui) {
        // Ribbon tabs, each with a small icon.
        ui.horizontal(|ui| {
            for (tab, icon) in ribbon::TABS {
                let img = ribbon::icon_image(ui, icon, self.dark, Vec2::splat(16.0));
                if ui.add(egui::Button::image_and_text(img, *tab).selected(self.ribbon == *tab).frame(false)).clicked() {
                    self.ribbon = tab;
                }
            }
        });
        if self.ribbon == "Systems" {
            ui.horizontal(|ui| {
                for (sub, icon) in ribbon::SYSTEMS_SUBTABS {
                    let img = ribbon::icon_image(ui, icon, self.dark, Vec2::splat(14.0));
                    if ui.add(egui::Button::image_and_text(img, *sub).selected(self.systems_sub == *sub).small()).clicked() {
                        self.systems_sub = sub;
                    }
                }
            });
        }
        ui.separator();
        let groups = ribbon::groups(self.ribbon, self.systems_sub);
        let mut clicked = None;
        egui::ScrollArea::horizontal().id_salt("ribbon").show(ui, |ui| {
            ui.horizontal(|ui| {
                for group in groups {
                    ribbon::panel(ui, group.title, |ui| {
                        for cmd in group.commands {
                            let active = matches!((cmd.id, self.tool),
                                ("duct", Tool::Duct) | ("pipe", Tool::Pipe) | ("wall", Tool::Wall));
                            if ribbon::big_button(ui, cmd.id, cmd.label, self.dark, active).clicked() {
                                clicked = Some((cmd.id, cmd.label));
                            }
                        }
                    });
                }
            });
        });
        if let Some((id, label)) = clicked { self.run_command(id, label); }
        ui.separator();
        panels::options(self, ui);
    }

    fn viewport(&mut self, ui: &mut egui::Ui) { model_view::show(self, ui); }
}

impl Desktop {
    fn undo_document(&mut self) {
        match self.document.undo() {
            Ok(true) => { self.sync_duct_controls(); self.message = "Undid last document transaction".into(); },
            Ok(false) => self.message = "Nothing to undo".into(),
            Err(err) => self.message = format!("Undo rejected: {err:?}"),
        }
    }
}

impl eframe::App for Desktop {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { self.choose_tool(Tool::Select); }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
            let ids: Vec<_> = self.document.elements().map(|el| el.id).collect();
            if !ids.is_empty() {
                let next = self.selected_id().and_then(|id| ids.iter().position(|candidate| *candidate == id))
                    .map_or(0, |index| (index + 1) % ids.len());
                self.select_element(Some(ids[next]));
            }
        }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z)) {
            self.undo_document();
        }
        egui::Panel::top("header").show(ui, |ui| {
            self.toolbar(ui);
            self.ribbon(ui);
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(&self.message);
                ui.separator();
                ui.label("Orthographic / fit to view");
                ui.separator();
                ui.label("Workset: Mechanical (sample)");
                ui.separator();
                ui.label(format!("In-memory BIM revision {} | NOT saved / synchronized", self.document.revision()));
                ui.separator();
                ui.checkbox(&mut self.show_properties, "Properties");
                ui.checkbox(&mut self.show_browser, "Browser");
            });
        });
        if self.show_properties || self.show_browser {
            egui::Panel::left("side").default_size(310.0).min_size(260.0)
                .max_size(520.0).resizable(true).show(ui, |ui| panels::sidebar(self, ui));
        }
        egui::CentralPanel::default().show(ui, |ui| {
            match self.workspace {
                Workspace::Design => self.viewport(ui),
                Workspace::Coordination => {
                    ui.heading("Coordination - planned");
                    ui.label("Federated IFC models, clash detection and issue tracking require the shared BIM document core.");
                }
                Workspace::Automation => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if self.automation.ui(ui, &mut self.document) { self.sync_duct_controls(); }
                    });
                }
            }
        });
        if self.show_help {
            egui::Window::new("Prototype scope").open(&mut self.show_help).show(&ctx, |ui| {
                ui.label("Plan and orbitable 3D views derive geometry from the same in-memory document.");
                ui.label("Model files, server, IFC editing, routing and persistence are not connected yet.");
                ui.label("ESC: cancel. TAB: cycle sample selection. Mouse wheel: zoom. Middle-drag: pan. Right-drag: orbit in 3D.");
                ui.label(format!("Existing keymap crate: Duct shortcut = {:?}",
                    self.keymap.shortcuts("Duct")));
            });
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions { viewport: egui::ViewportBuilder::default()
        .with_title("OpenBIM - Revit-style Desktop Prototype")
        .with_inner_size([1440.0, 850.0])
        .with_min_inner_size([900.0, 550.0]), ..Default::default() };
    eframe::run_native("OpenBIM Desktop Prototype", options,
        Box::new(|cc| Ok(Box::new(Desktop::new(cc)))))
}
