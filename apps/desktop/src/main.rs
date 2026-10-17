//! First executable desktop prototype. The viewport is a clearly labelled SAMPLE scene:
//! no model persistence, IFC import, live routing, or central collaboration is implied.
#![forbid(unsafe_code)]

mod icons;
mod ribbon;
mod panels;
mod model_view;
mod view3d;
mod model_edit;
mod export;
mod automation;
mod automation_canvas;
mod automation_groups;
mod automation_slider;
mod automation_icons;

use openbim_document::{Document, ElementId};
use eframe::egui::{self, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Workspace { Design, Coordination, Automation }
#[derive(Clone, Copy, PartialEq, Eq)]
enum View { Plan, UpperPlan, ThreeD, Mechanical }
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool { Select, Duct, Pipe, Wall, Beam, Column, Slab, Footing, WallFooting, BeamSystem, Truss, Rebar }
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
    /// the section chosen for a beam: 0 rectangle, 1 circle, 2 I-shape; with the web and flange thickness of an I
    section_pick: u8,
    usage_pick: openbim_document::Usage,
    material_pick: openbim_document::Material,
    /// how far a brace rises over its run when it is placed by clicking
    brace_rise_mm: f64,
    web_mm: f64,
    flange_mm: f64,
    axis_mm: [f64; 4],
    /// the first point of a beam being placed in a plan view
    beam_start: Option<[f64; 2]>,
    /// the level the options bar places beams on; the lowest when None
    beam_level: Option<ElementId>,
    /// the corners of the slab being drawn in a plan view
    slab_points: Vec<[f64; 2]>,
    /// beam system: the distance between its beams and their direction
    spacing_mm: f64,
    angle_deg: f64,
    /// rebar: the number of bars along the bottom of the beam
    bar_count: f64,
    slab_thickness_mm: f64,
    /// how far above its level the top of a new slab is
    slab_offset_mm: f64,
    slab_kind_pick: openbim_document::SlabKind,
    /// footing: the thickness of a new pad, or of the selected one
    footing_thickness_mm: f64,
    /// the beam being edited: whether its start and end are joined to the columns they end in
    join_pick: [bool; 2],
    camera_yaw: f32,
    camera_pitch: f32,
    view3d: openbim_viewport_egui::Viewport3d,
    view3d_revision: Option<u64>,
    /// a site edit in progress in the sun controls, committed when the value is let go
    site_edit: Option<openbim_document::Site>,
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
        if let Some(rs) = cc.wgpu_render_state.as_ref() { openbim_viewport_egui::Viewport3d::init(rs); }
        Self::sample()
    }

    fn sample() -> Self {
        let mut keymap = openbim_keymap::Keymap::new(&["Select", "Duct", "Pipe", "Wall", "Beam", "Column", "Slab", "Footing", "Wall Footing", "Beam System", "Truss", "Rebar"])
            .expect("static shortcut commands");
        keymap.set_preset(&[
            ("Duct", &["D D"]),
            ("Pipe", &["P I"]),
            ("Wall", &["W A"]),
            ("Beam", &["B M"]),
            ("Column", &["C L"]),
            ("Slab", &["S L"]),
            ("Footing", &["F T"]),
            ("Wall Footing", &["W F"]),
            ("Beam System", &["B S"]),
            ("Truss", &["T R"]),
            ("Rebar", &["R B"]),
        ]).expect("static shortcuts");
        let document = Document::spatial_sample();
        Self {
            automation: automation::Automation::default(),
            document,
            workspace: Workspace::Design, view: View::Mechanical, tool: Tool::Select,
            selected: Some(SampleElement::Duct), ribbon: "Systems", systems_sub: "HVAC",
            width_mm: 200.0, height_mm: 300.0, elevation_mm: 2700.0,
            slope_percent: 0.0, section_pick: 0, usage_pick: openbim_document::Usage::Girder, material_pick: openbim_document::Material::Unspecified, brace_rise_mm: 3000.0, web_mm: 10.0, flange_mm: 20.0,
            axis_mm: [1000.0, 2000.0, 7000.0, 2000.0], beam_start: None, beam_level: None,
            slab_points: Vec::new(), spacing_mm: 600.0, angle_deg: 0.0, bar_count: 3.0, slab_thickness_mm: 200.0, slab_offset_mm: 0.0, slab_kind_pick: openbim_document::SlabKind::Floor, footing_thickness_mm: 500.0, join_pick: [true; 2],
            camera_yaw: 0.7, camera_pitch: 0.6,
            view3d: openbim_viewport_egui::Viewport3d::new(), view3d_revision: None, site_edit: None,
            property_query: String::new(), browser_query: String::new(),
            browser_selected: String::new(),
            message: "Sample scene - no BIM edits are saved".into(),
            show_properties: true, show_browser: true, show_help: false, dark: false,
            zoom: 1.0, pan: Vec2::ZERO, keymap,
        }
    }

    fn choose_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.beam_start = None;
        self.slab_points.clear();
        self.message = match tool {
            Tool::Select => "Select a sample element. Tab cycles; Esc cancels.".into(),
            Tool::Duct => "Duct command preview only; routing engine is not connected yet.".into(),
            Tool::Pipe => "Pipe command preview only; routing engine is not connected yet.".into(),
            Tool::Beam => "Beam: click the start and the end in a plan view (Esc ends), or type the ends in the options bar and press Place Beam.".into(),
            Tool::Column => "Column: click its place in a plan view; it stands from that level to the next one up (Esc ends).".into(),
            Tool::Slab => "Slab: click the corners in a plan view, then Enter, a click on the first corner or Finish closes it (Backspace takes the last corner back, Esc cancels).".into(),
            Tool::Footing => "Isolated footing: click its place in a plan view; it is as wide and long as the options bar says, under the level (Esc ends).".into(),
            Tool::WallFooting => "Wall footing: click a wall in a plan view; the footing runs under it and follows it (Esc ends).".into(),
            Tool::BeamSystem => "Beam system: click the corners of the area in a plan view, then Enter, a click on the first corner or Finish lays parallel beams at the spacing and angle of the options bar (Esc cancels).".into(),
            Tool::Truss => "Truss: click the start and the end of the truss in a plan view; it is built with the panel length, depth and member size of the options bar (Esc ends).".into(),
            Tool::Rebar => "Rebar: click a beam in a plan view; bars and stirrups are placed inside it and follow it (Esc ends).".into(),
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
            "beam" => { self.usage_pick = openbim_document::Usage::Girder; self.choose_tool(Tool::Beam) }
            "structural_column" | "column" => self.choose_tool(Tool::Column),
            "structural_floor" | "floor" => { self.slab_kind_pick = openbim_document::SlabKind::Floor; self.choose_tool(Tool::Slab) }
            "footing" => self.choose_tool(Tool::WallFooting),
            "truss" => self.choose_tool(Tool::Truss),
            "rebar" => { (self.width_mm, self.height_mm, self.elevation_mm, self.spacing_mm, self.bar_count) = (16.0, 8.0, 40.0, 200.0, 3.0); self.choose_tool(Tool::Rebar) }
            "beam_system" => { self.usage_pick = openbim_document::Usage::Joist; self.choose_tool(Tool::BeamSystem) }
            "foundation" => self.choose_tool(Tool::Footing),
            "slab" => { self.slab_kind_pick = openbim_document::SlabKind::Foundation; self.choose_tool(Tool::Slab) }
            "brace" => { self.usage_pick = openbim_document::Usage::Brace; self.choose_tool(Tool::Beam) }
            "select" => self.choose_tool(Tool::Select),
            "ifc_export" => self.export_ifc_file(),
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
                                ("duct", Tool::Duct) | ("pipe", Tool::Pipe) | ("wall", Tool::Wall) | ("beam" | "brace", Tool::Beam) | ("structural_column", Tool::Column) | ("structural_floor" | "floor" | "slab", Tool::Slab) | ("foundation", Tool::Footing) | ("footing", Tool::WallFooting) | ("beam_system", Tool::BeamSystem) | ("truss", Tool::Truss) | ("rebar", Tool::Rebar));
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
