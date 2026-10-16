//! First executable desktop prototype. The viewport is a clearly labelled SAMPLE scene:
//! no model persistence, IFC import, live routing, or central collaboration is implied.
#![forbid(unsafe_code)]

mod icons;
mod ribbon;
mod panels;

use openbim_document::{Command, Document, ElementId, Category};
use eframe::egui::{self, Color32, Rect, Sense, Stroke, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Workspace { Design, Coordination, Automation }
#[derive(Clone, Copy, PartialEq, Eq)]
enum View { Plan, ThreeD, Mechanical }
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool { Select, Duct, Pipe, Wall }
#[derive(Clone, Copy, PartialEq, Eq)]
enum SampleElement { Ahu, Duct, Room }

struct Desktop {
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
    inherit_size: bool,
    inherit_elevation: bool,
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
        let mut document = Document::new();
        document.transact(0, &[Command::Create {
            id: ElementId(1), category: Category::Duct,
            name: "Sample Supply Air Duct".into(),
            width_m: 0.2, height_m: 0.3, elevation_m: 2.7,
        }]).expect("valid sample document");
        Self {
            document,
            workspace: Workspace::Design, view: View::Mechanical, tool: Tool::Select,
            selected: Some(SampleElement::Duct), ribbon: "Systems", systems_sub: "HVAC",
            width_mm: 200.0, height_mm: 300.0, elevation_mm: 2700.0,
            slope_percent: 0.0,
            inherit_size: false, inherit_elevation: false,
            property_query: String::new(), browser_query: String::new(),
            browser_selected: String::new(),
            message: "Sample scene — no BIM edits are saved".into(),
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
            Tool::Wall => "Wall command preview only; geometry engine is not connected yet.".into(),
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
        ui.horizontal_wrapped(|ui| {
            let label = match self.tool {
                Tool::Duct => "Modify | Place Duct",
                Tool::Pipe => "Modify | Place Pipe",
                Tool::Wall => "Modify | Place Wall",
                Tool::Select if self.selected == Some(SampleElement::Duct) => "Modify | Ducts",
                Tool::Select => "Modify | Select",
            };
            ui.strong(label);
            let selected_duct = self.selected == Some(SampleElement::Duct)
                && self.document.get(ElementId(1)).is_some();
            if selected_duct && matches!(self.tool, Tool::Select | Tool::Duct) {
                ui.separator();
                ui.label("Width (mm)");
                ui.add(egui::DragValue::new(&mut self.width_mm).range(1.0..=100_000.0).speed(10.0));
                ui.label("Height (mm)");
                ui.add(egui::DragValue::new(&mut self.height_mm).range(1.0..=100_000.0).speed(10.0));
                ui.label("Middle Elevation (mm)");
                ui.add(egui::DragValue::new(&mut self.elevation_mm).speed(50.0));
                if self.document.get(ElementId(1)).is_some_and(|el| el.slope_ratio.is_some()) {
                    ui.label("Slope (%)");
                    ui.add(egui::DragValue::new(&mut self.slope_percent).speed(0.1));
                }
                if self.tool == Tool::Duct {
                    ui.add_enabled(false, egui::Checkbox::new(&mut self.inherit_elevation, "Inherit Elevation"));
                    ui.add_enabled(false, egui::Checkbox::new(&mut self.inherit_size, "Inherit Size"));
                }
                if ui.button("Apply").clicked() {
                    self.apply_duct_edit();
                }
                if ui.button("Reset").clicked() { self.sync_duct_controls(); }
            } else if self.tool != Tool::Select {
                ui.weak("Placement preview — select the sample duct to edit its parameters");
            }
        });
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (name, view) in [
                ("Level 1", View::Plan), ("{3D}", View::ThreeD),
                ("M-101 - Mechanical Plan", View::Mechanical),
            ] {
                if ui.selectable_label(self.view == view, name).clicked() { self.view = view; }
            }
        });
        ui.separator();
        ui.label("SAMPLE VIEW — duct properties use in-memory Rust BIM document; geometry illustrative");
        let size = ui.available_size().max(Vec2::new(250.0, 250.0));
        let (response, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let rect = response.rect;
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        if response.dragged_by(egui::PointerButton::Middle) { self.pan += response.drag_delta(); }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 { self.zoom = (self.zoom * (1.0 + scroll * 0.001)).clamp(0.3, 4.0); }
        }
        let origin = rect.center() + self.pan;
        let scale = self.zoom * (rect.width().min(rect.height()) / 650.0);
        let pt = |x: f32, y: f32| origin + Vec2::new((x - 300.0) * scale, (y - 220.0) * scale);
        let wall = Stroke::new(8.0 * scale.max(0.5), Color32::from_gray(105));
        for (a,b) in [
            ((65.,65.),(535.,65.)), ((535.,65.),(535.,385.)),
            ((535.,385.),(65.,385.)), ((65.,385.),(65.,65.)),
            ((300.,65.),(300.,385.)), ((65.,235.),(535.,235.))
        ] { painter.line_segment([pt(a.0,a.1),pt(b.0,b.1)],wall); }
        let ahu = Rect::from_two_pos(pt(105.,95.),pt(185.,170.));
        let duct = Rect::from_two_pos(pt(145.,173.),pt(438.,189.));
        painter.rect_filled(ahu, 3.0, Color32::from_rgb(135, 174, 155));
        painter.text(ahu.center(), egui::Align2::CENTER_CENTER, "AHU",
            egui::FontId::proportional(15.0), Color32::BLACK);
        painter.rect_filled(duct, 1.0, Color32::from_rgb(75, 170, 208));
        painter.text(pt(210.,286.),egui::Align2::LEFT_CENTER,"Office 101",
            egui::FontId::proportional(17.0), ui.visuals().text_color());
        painter.text(pt(380.,130.),egui::Align2::LEFT_CENTER,"Meeting 102",
            egui::FontId::proportional(16.0), ui.visuals().text_color());
        painter.text(pt(395.,320.),egui::Align2::LEFT_CENTER,"Store 103",
            egui::FontId::proportional(16.0), ui.visuals().text_color());
        for x in [125.,295.,470.] {
            painter.circle_filled(pt(x,40.),12.0,Color32::from_gray(190));
        }
        if self.selected == Some(SampleElement::Duct) {
            painter.rect_stroke(duct.expand(2.0), 0.0, Stroke::new(2.5_f32, Color32::YELLOW),
                egui::StrokeKind::Outside);
        }
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.selected = if duct.contains(pos) { Some(SampleElement::Duct) }
                    else if ahu.contains(pos) { Some(SampleElement::Ahu) }
                    else if rect.contains(pos) { Some(SampleElement::Room) } else { None };
            }
        }
        painter.text(rect.right_top() + Vec2::new(-12.0, 12.0),
            egui::Align2::RIGHT_TOP,
            if self.view == View::ThreeD { "TOP / 3D PLACEHOLDER" } else { "N   TOP" },
            egui::FontId::proportional(14.0), ui.visuals().text_color());
    }
}

impl Desktop {
    fn apply_duct_edit(&mut self) {
        if self.selected != Some(SampleElement::Duct) {
            self.message = "Select the sample duct to edit its parameters".into();
            return;
        }
        let id = ElementId(1);
        let revision = self.document.revision();
        let result = self.document.transact(revision, &[
            Command::Resize { id, width_m: self.width_mm / 1000.0, height_m: self.height_mm / 1000.0 },
            Command::MoveElevation { id, elevation_m: self.elevation_mm / 1000.0 },
            Command::SetSlope { id, slope_ratio: self.slope_percent / 100.0 },
        ]);
        match result {
            Ok(new_rev) => self.message = format!("Duct parameters committed to in-memory document revision {new_rev}"),
            Err(err) => self.message = format!("Document edit rejected: {err:?}"),
        }
    }

    fn sync_duct_controls(&mut self) {
        if let Some(duct) = self.document.get(ElementId(1)) {
            self.width_mm = duct.width_m * 1000.0;
            self.height_mm = duct.height_m * 1000.0;
            self.elevation_mm = duct.elevation_m * 1000.0;
            self.slope_percent = duct.slope_ratio.unwrap_or(0.0) * 100.0;
        }
    }

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
            self.selected = Some(match self.selected {
                Some(SampleElement::Duct) => SampleElement::Ahu,
                Some(SampleElement::Ahu) => SampleElement::Room,
                _ => SampleElement::Duct,
            });
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
                ui.label("Scale 1:100");
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
                    ui.heading("Coordination — planned");
                    ui.label("Federated IFC models, clash detection and issue tracking require the shared BIM document core.");
                }
                Workspace::Automation => {
                    ui.heading("Automation — planned");
                    ui.label("Dynamo-like typed graph editor (external automation software) — planned.");
                }
            }
        });
        if self.show_help {
            egui::Window::new("Prototype scope").open(&mut self.show_help).show(&ctx, |ui| {
                ui.label("This is a Rust desktop UI shell with an illustrative 2D scene.");
                ui.label("No real model files, server, IFC editing, routing or persistence are implemented here.");
                ui.label("ESC: cancel. TAB: cycle sample selection. Mouse wheel: zoom. Middle-drag: pan.");
                ui.label(format!("Existing keymap crate: Duct shortcut = {:?}",
                    self.keymap.shortcuts("Duct")));
            });
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions { viewport: egui::ViewportBuilder::default()
        .with_title("OpenBIM — Revit-style Desktop Prototype")
        .with_inner_size([1440.0, 850.0])
        .with_min_inner_size([900.0, 550.0]), ..Default::default() };
    eframe::run_native("OpenBIM Desktop Prototype", options,
        Box::new(|cc| Ok(Box::new(Desktop::new(cc)))))
}
