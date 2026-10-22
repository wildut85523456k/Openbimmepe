//! The Compute button: marking where a system starts and what its end points take, running the sizing and losses of a
//! system (HVAC piping and ductwork, plumbing, fire water) from the drawing, reading the answer, and writing the sizes
//! back. The calculation itself is `openbim-calc`; the reading of the drawing is `openbim_document::compute`.
use crate::Desktop;
use eframe::egui::{self, Color32, Pos2, Stroke};
use openbim_calc as calc;
use openbim_document::compute::{self, ComputeError, ComputeSettings, FittingLosses, SystemReport};
use openbim_document::{Category, Command, Document, ElementId, EndDemand, EndRole, Geometry, MepEndpoint, MepNode, MepPort};
use openbim_family::mep::{Domain, System};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mark { Source, Terminal, Remove }

/// What the End Point tool places.
#[derive(Clone)]
pub(crate) struct EndDraft {
    pub mark: Mark,
    /// 0 flow, 1 power and temperature difference, 2 loading units, 3 sprinkler
    pub kind: usize,
    pub a: f64,
    pub b: f64,
    pub pressure_kpa: f64,
}

impl Default for EndDraft {
    fn default() -> Self { Self { mark: Mark::Terminal, kind: 1, a: 10.0, b: 5.0, pressure_kpa: 20.0 } }
}

impl EndDraft {
    pub(crate) const KINDS: [&'static str; 5] = ["Flow (l/s)", "Power (kW) and temperature difference (K)", "Loading units", "Sprinkler: K-factor and minimum flow (l/min)", "From the equipment connector it meets"];

    pub(crate) fn demand(&self) -> EndDemand {
        match self.kind {
            0 => EndDemand::Flow(self.a / 1000.0),
            1 => EndDemand::Power { watts: self.a * 1000.0, delta_t: self.b },
            2 => EndDemand::LoadingUnits(self.a),
            _ => EndDemand::Sprinkler { k_l_min_bar: self.a, min_flow_l_min: self.b },
        }
    }
}

/// The Compute window: the settings, the answer of the last run.
pub(crate) struct ComputeDialog {
    pub open: bool,
    pub system: System,
    pub density: f64,
    pub viscosity_mpas: f64,
    pub specific_heat: f64,
    pub roughness_mm: f64,
    /// 0 for no limit
    pub max_velocity: f64,
    pub max_friction: f64,
    /// Hazen-Williams C for fire water; 0 for Darcy-Weisbach
    pub hazen_c: f64,
    pub count_static: bool,
    pub keep_sizes: bool,
    /// elbow, reducer, tee, tap
    pub k: [f64; 4],
    pub catalogue: String,
    pub loading: String,
    pub report: Option<(u64, SystemReport)>,
    pub error: String,
}

impl Default for ComputeDialog {
    fn default() -> Self {
        Self {
            open: false, system: System::ChilledWaterSupply, density: 998.0, viscosity_mpas: 1.0, specific_heat: 4186.0, roughness_mm: 0.045,
            max_velocity: 1.5, max_friction: 0.0, hazen_c: 0.0, count_static: false, keep_sizes: false, k: [0.3, 0.1, 0.5, 0.5],
            catalogue: String::new(), loading: String::new(), report: None, error: String::new(),
        }
    }
}

fn is_duct(s: System) -> bool { s.domain_ok(Domain::Duct) && s != System::Other }

/// `label,inside diameter mm` for pipes; `label,width mm,height mm` for ducts. Blank lines, `#` comments and a header are allowed.
pub(crate) fn parse_catalogue(text: &str, duct: bool) -> Result<Vec<calc::Size>, String> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let c: Vec<&str> = line.split([',', ';']).map(str::trim).collect();
        let want = if duct { 3 } else { 2 };
        if c.len() != want { return Err(format!("line {}: {} columns are expected ({})", n + 1, want, if duct { "label, width mm, height mm" } else { "label, inside diameter mm" })); }
        let nums: Result<Vec<f64>, _> = c[1..].iter().map(|x| x.parse::<f64>()).collect();
        match nums {
            Ok(v) if v.iter().all(|x| x.is_finite() && *x > 0.0) => out.push(if duct { calc::Size::rect(c[0], v[0] / 1000.0, v[1] / 1000.0) } else { calc::Size::round(c[0], v[0] / 1000.0) }),
            Ok(_) => return Err(format!("line {}: a size must be more than zero", n + 1)),
            Err(_) if out.is_empty() => continue,
            Err(_) => return Err(format!("line {}: not a number", n + 1)),
        }
    }
    Ok(out)
}

fn describe(e: &ComputeError) -> String {
    match e {
        ComputeError::NoSource => "This system has no source. Use the End Point tool to mark where it starts (a pump, a fan, a riser) on an open end of a duct or pipe, and give the runs this system.".into(),
        ComputeError::MoreThanOneSource => "This system has more than one source; mark one.".into(),
        ComputeError::NoTerminal => "No end point with a demand is connected to the source. Mark the coils, terminals, fixtures or sprinklers with the End Point tool.".into(),
        ComputeError::Loop(id) => format!("Run {} closes a loop. Rings are not computed yet; open the loop or split it into branches.", id.0),
        ComputeError::Calc(c) => match c {
            calc::TreeError::NoSize(i) => format!("No size in the catalogue is enough for branch {i}: add larger sizes, or raise the limits."),
            calc::TreeError::BadSettings(w) => format!("Check the settings: {w}."),
            calc::TreeError::BadValue(i, w) => format!("A value is not usable at node {i}: {w}."),
            calc::TreeError::OutsideCurve(i) => format!("The loading units at node {i} are more than the loading-unit table covers."),
            other => format!("{other:?}"),
        },
    }
}

impl Desktop {
    /// A click of the End Point tool: the open end of a duct or pipe nearest the click takes (or loses) the end point.
    pub(crate) fn end_point_click(&mut self, level: ElementId, at: [f64; 2]) {
        let tol = 0.4;
        let mut best: Option<(f64, MepPort)> = None;
        for e in self.document.elements().filter(|e| matches!(e.category, Category::Duct | Category::Pipe)) {
            let Some(Geometry::Linear { start, end, level: l }) = e.geometry else { continue };
            if l != level { continue; }
            for (port, p) in [(0usize, start), (1, end)] {
                let d = (p[0] - at[0]).hypot(p[1] - at[1]);
                let mp = MepPort { node: MepNode::Run(e.id), port };
                let open = self.document.mep().partner(mp).is_none();
                if d <= tol && open && best.is_none_or(|(b, _)| d < b) { best = Some((d, mp)); }
            }
        }
        let Some((_, port)) = best else { self.message = "End point: click on an open end of a duct or pipe (an end with a fitting on it is not open)".into(); return };
        let mut data = self.document.mep().clone();
        let had = data.endpoints.len();
        data.endpoints.retain(|e| e.port != port);
        data.equip_links.retain(|q| q.port != port);
        let removed = data.endpoints.len() != had;
        let d = self.end_draft.clone();
        match d.mark {
            Mark::Remove => {
                if !removed { self.message = "End point: nothing to remove there".into(); return; }
            }
            Mark::Source => data.endpoints.push(MepEndpoint { port, role: EndRole::Source, demand: EndDemand::None, required_pressure_pa: 0.0 }),
            Mark::Terminal if d.kind == 4 => {
                let (domain, at) = (if self.document.get(match port.node { MepNode::Run(id) => id, _ => ElementId(0) }).is_some_and(|e| e.category == Category::Pipe) { openbim_family::mep::Domain::Pipe } else { openbim_family::mep::Domain::Duct }, port_pos(&self.document, port).unwrap_or(at));
                let Some((el, c)) = crate::elec_ui::equipment_connector_near(&self.document, level, at, 0.6, domain, None) else { self.message = "End point: no equipment connector of this kind within 0.6 m of that end".into(); return };
                if c.flow <= 0.0 { self.message = format!("The connector '{}' of {} has no flow", c.name, self.document.get(el).map_or("the equipment", |e| e.name.as_str())); return; }
                data.equip_links.push(openbim_document::EquipLink { port, element: el, connector: c.name.clone() });
                data.endpoints.push(MepEndpoint { port, role: EndRole::Terminal, demand: EndDemand::Flow(c.flow), required_pressure_pa: c.pressure_drop });
            }
            Mark::Terminal => data.endpoints.push(MepEndpoint { port, role: EndRole::Terminal, demand: d.demand(), required_pressure_pa: d.pressure_kpa * 1000.0 }),
        }
        match self.document.transact(self.document.revision(), &[Command::SetMep(Box::new(data))]) {
            Ok(_) => self.message = match d.mark { Mark::Remove => "End point removed".into(), Mark::Source => "Source marked; undo removes it".into(), Mark::Terminal => "End point placed; undo removes it".into() },
            Err(e) => self.message = format!("End point rejected ({e:?})"),
        }
    }

    pub(crate) fn end_point_options(&mut self, ui: &mut egui::Ui) {
        ui.strong("End Point");
        ui.separator();
        ui.selectable_value(&mut self.end_draft.mark, Mark::Source, "Source");
        ui.selectable_value(&mut self.end_draft.mark, Mark::Terminal, "Terminal");
        ui.selectable_value(&mut self.end_draft.mark, Mark::Remove, "Remove");
        if self.end_draft.mark == Mark::Terminal {
            let d = &mut self.end_draft;
            egui::ComboBox::from_id_salt("end_kind").selected_text(EndDraft::KINDS[d.kind.min(4)]).show_ui(ui, |ui| {
                for (i, k) in EndDraft::KINDS.iter().enumerate() { ui.selectable_value(&mut d.kind, i, *k); }
            });
            if d.kind != 4 { ui.add(egui::DragValue::new(&mut d.a).speed(0.1).range(0.0..=1.0e6)); }
            if d.kind == 1 || d.kind == 3 { ui.add(egui::DragValue::new(&mut d.b).speed(0.1).range(0.0..=1.0e6)); }
            if d.kind != 4 {
                ui.label("Pressure it needs (kPa)");
                ui.add(egui::DragValue::new(&mut d.pressure_kpa).speed(0.5).range(0.0..=1.0e5));
            }
        }
        ui.weak("Click an open end of a duct or pipe. Source = where the pump or fan is; terminals take the flow.");
    }

    /// The settings of the dialog as the calculation takes them.
    fn compute_settings(&self) -> Result<ComputeSettings, String> {
        let d = &self.compute;
        let catalogue = parse_catalogue(&d.catalogue, is_duct(d.system))?;
        let loading = if d.loading.trim().is_empty() { None } else { Some(calc::LoadingCurve::from_csv(&d.loading).map_err(|e| format!("loading-unit table: {e:?}"))?) };
        Ok(ComputeSettings {
            system: d.system,
            calc: calc::Settings {
                fluid: calc::Fluid { density: d.density, viscosity: d.viscosity_mpas / 1000.0, specific_heat: d.specific_heat },
                roughness: d.roughness_mm / 1000.0,
                criteria: calc::Criteria { max_velocity: (d.max_velocity > 0.0).then_some(d.max_velocity), max_friction: (d.max_friction > 0.0).then_some(d.max_friction) },
                catalogue, count_static: d.count_static, loading, hazen_williams: (d.hazen_c > 0.0).then_some(d.hazen_c),
            },
            losses: FittingLosses { elbow: d.k[0], transition: d.k[1], tee: d.k[2], tap: d.k[3] },
            keep_sizes: d.keep_sizes,
        })
    }

    /// Brings what is tied to equipment up to date: a run end joined to a connector follows the unit (the whole run moves
    /// when its other end is free, otherwise the end is stretched), and an end point takes the unit's flow and pressure
    /// drop again. One undoable step. Gives how many run ends and end points changed.
    pub(crate) fn refresh_equipment_ends(&mut self) -> usize {
        let mut data = self.document.mep().clone();
        let (mut ends, mut moved) = (0, 0);
        let mut cmds: Vec<Command> = Vec::new();
        for q in data.equip_links.clone() {
            let Some(c) = crate::elec_ui::instance_connectors(&self.document, q.element).into_iter().find(|c| c.name == q.connector && matches!(c.domain, openbim_family::mep::Domain::Duct | openbim_family::mep::Domain::Pipe)) else { continue };
            let snap = snap_commands(&self.document, &q, &c);
            if !snap.is_empty() { moved += 1; cmds.extend(snap); }
            let Some(e) = data.endpoints.iter_mut().find(|e| e.port == q.port && matches!(e.demand, EndDemand::Flow(_))) else { continue };
            if e.demand != EndDemand::Flow(c.flow) || e.required_pressure_pa != c.pressure_drop { (e.demand, e.required_pressure_pa) = (EndDemand::Flow(c.flow), c.pressure_drop); ends += 1; }
        }
        if ends + moved > 0 {
            if ends > 0 { cmds.push(Command::SetMep(Box::new(data))); }
            self.message = match self.document.transact(self.document.revision(), &cmds) { Ok(_) => format!("{moved} run end(s) moved to their equipment, {ends} end point(s) updated; undo reverts"), Err(e) => format!("Rejected ({e:?})") };
        }
        ends + moved
    }

    pub(crate) fn run_compute(&mut self) {
        self.compute.error.clear();
        self.refresh_equipment_ends();
        let settings = match self.compute_settings() { Ok(s) => s, Err(e) => { self.compute.error = e; return } };
        match compute::compute_system(&self.document, &settings) {
            Ok(r) => { self.message = format!("{} computed: {} runs, duty {:.1} kPa", r.system.name(), r.runs.len(), r.source_pressure / 1000.0); self.compute.report = Some((self.document.revision(), r)); }
            Err(e) => { self.compute.report = None; self.compute.error = describe(&e); }
        }
    }

    pub(crate) fn compute_window(&mut self, ctx: &egui::Context) {
        if !self.compute.open { return; }
        let mut open = true;
        let mut run = false;
        let mut apply = false;
        let mut assign = false;
        let mut select: Option<ElementId> = None;
        egui::Window::new("Compute").open(&mut open).default_width(560.0).show(ctx, |ui| {
            let d = &mut self.compute;
            ui.horizontal_wrapped(|ui| {
                ui.label("System");
                egui::ComboBox::from_id_salt("compute_system").selected_text(d.system.name()).show_ui(ui, |ui| {
                    for s in System::ALL.iter().copied().filter(|s| *s != System::Other && !matches!(s, System::Power | System::Data)) { ui.selectable_value(&mut d.system, s, s.name()); }
                });
                if ui.button("Water").on_hover_text("density 998, viscosity 1.0 mPa.s, specific heat 4186").clicked() { (d.density, d.viscosity_mpas, d.specific_heat, d.roughness_mm) = (998.0, 1.0, 4186.0, 0.045); }
                if ui.button("Air").on_hover_text("density 1.2, viscosity 0.0181 mPa.s, specific heat 1006").clicked() { (d.density, d.viscosity_mpas, d.specific_heat, d.roughness_mm) = (1.2, 0.0181, 1006.0, 0.09); }
            });
            egui::CollapsingHeader::new("Settings").default_open(true).show(ui, |ui| {
                egui::Grid::new("compute_grid").num_columns(4).show(ui, |ui| {
                    ui.label("Density (kg/m3)"); ui.add(egui::DragValue::new(&mut d.density).speed(0.1).range(0.01..=2000.0));
                    ui.label("Viscosity (mPa.s)"); ui.add(egui::DragValue::new(&mut d.viscosity_mpas).speed(0.001).range(0.001..=1000.0)); ui.end_row();
                    ui.label("Specific heat (J/kg.K)"); ui.add(egui::DragValue::new(&mut d.specific_heat).speed(1.0).range(100.0..=10000.0));
                    ui.label("Roughness (mm)"); ui.add(egui::DragValue::new(&mut d.roughness_mm).speed(0.005).range(0.0..=10.0)); ui.end_row();
                    ui.label("Max velocity (m/s, 0 = none)"); ui.add(egui::DragValue::new(&mut d.max_velocity).speed(0.05).range(0.0..=100.0));
                    ui.label("Max friction (Pa/m, 0 = none)"); ui.add(egui::DragValue::new(&mut d.max_friction).speed(0.05).range(0.0..=10000.0)); ui.end_row();
                    ui.label("Hazen-Williams C (0 = Darcy)"); ui.add(egui::DragValue::new(&mut d.hazen_c).speed(1.0).range(0.0..=200.0));
                    ui.label(""); ui.checkbox(&mut d.count_static, "Count height as pressure"); ui.end_row();
                    ui.label("K elbow / reducer"); ui.horizontal(|ui| { ui.add(egui::DragValue::new(&mut d.k[0]).speed(0.01).range(0.0..=50.0)); ui.add(egui::DragValue::new(&mut d.k[1]).speed(0.01).range(0.0..=50.0)); });
                    ui.label("K tee / tap"); ui.horizontal(|ui| { ui.add(egui::DragValue::new(&mut d.k[2]).speed(0.01).range(0.0..=50.0)); ui.add(egui::DragValue::new(&mut d.k[3]).speed(0.01).range(0.0..=50.0)); }); ui.end_row();
                });
                ui.checkbox(&mut d.keep_sizes, "Keep the drawn sizes (only report the losses and the limits)");
                ui.label(if is_duct(d.system) { "Size catalogue: label, width mm, height mm (inside dimensions), one per line" } else { "Size catalogue: label, inside diameter mm, one per line" });
                ui.add(egui::TextEdit::multiline(&mut d.catalogue).desired_rows(4).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
                if ui.small_button("Load a CSV file...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("Size catalogue").add_filter("CSV", &["csv", "txt"]).pick_file() {
                        match std::fs::read_to_string(&p) { Ok(t) => d.catalogue = t, Err(e) => d.error = format!("{}: {e}", p.display()) }
                    }
                }
                if d.system == System::DomesticCold || d.system == System::DomesticHot {
                    ui.label("Loading units to design flow (loading_units,flow_l_s), from your code's table");
                    ui.add(egui::TextEdit::multiline(&mut d.loading).desired_rows(3).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Compute").clicked() { run = true; }
                if ui.add_enabled(d.report.is_some() && !d.keep_sizes, egui::Button::new("Apply sizes")).on_hover_text("Write the computed sizes to the ducts and pipes (one undoable step)").clicked() { apply = true; }
                if ui.button("Give the selected line this system").on_hover_text("Select a duct or pipe: it and everything joined to it take the system above").clicked() { assign = true; }
                if ui.add_enabled(d.report.is_some(), egui::Button::new("Clear")).clicked() { d.report = None; }
            });
            if !d.error.is_empty() { ui.colored_label(Color32::from_rgb(200, 60, 40), &d.error); }
            let now = self.document.revision();
            if let Some((rev, r)) = &self.compute.report {
                if *rev != now { ui.colored_label(Color32::from_rgb(190, 120, 20), "The drawing has changed since this was computed; compute again."); }
                ui.separator();
                ui.label(format!("{}: total flow {:.3} l/s; pump or fan duty {:.2} kPa; {} runs, most resistant path in red.", r.system.name(), r.total_flow * 1000.0, r.source_pressure / 1000.0, r.runs.len()));
                if !r.unreached.is_empty() { ui.colored_label(Color32::from_rgb(190, 120, 20), format!("{} end point(s) with a demand are not connected to the source.", r.unreached.len())); }
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    egui::Grid::new("compute_runs").striped(true).num_columns(8).show(ui, |ui| {
                        for h in ["Run", "Flow l/s", "Size", "m/s", "Pa/m", "Loss Pa", "Length m", ""] { ui.strong(h); }
                        ui.end_row();
                        for run in &r.runs {
                            let name = self.document.get(run.run).map_or(format!("#{}", run.run.0), |e| format!("{} (#{})", e.name, run.run.0));
                            if ui.selectable_label(false, name).clicked() { select = Some(run.run); }
                            ui.label(format!("{:.3}", run.flow * 1000.0));
                            ui.label(&run.size.label);
                            ui.label(format!("{:.2}", run.velocity));
                            ui.label(format!("{:.1}", run.friction_per_metre));
                            ui.label(format!("{:.0}", run.dp_total));
                            ui.label(format!("{:.2}", run.length));
                            ui.label(format!("{}{}", if run.critical { "critical " } else { "" }, if run.exceeds_limits { "over limit" } else { "" }));
                            ui.end_row();
                        }
                    });
                    ui.separator();
                    egui::Grid::new("compute_ends").striped(true).num_columns(4).show(ui, |ui| {
                        for h in ["End point", "Flow l/s", "Needs at source kPa", "To take up kPa"] { ui.strong(h); }
                        ui.end_row();
                        for e in &r.ends {
                            let place = match e.port { Some(MepPort { node: MepNode::Run(id), port }) => format!("run #{} end {}", id.0, port), Some(_) => "fitting".into(), None => "dead end".into() };
                            ui.label(place);
                            ui.label(format!("{:.3}", e.flow * 1000.0));
                            ui.label(format!("{:.2}", e.needed_at_source / 1000.0));
                            ui.label(format!("{:.2}{}", e.excess / 1000.0, e.pressure.map_or(String::new(), |p| format!("  (gets {:.1} kPa)", p / 1000.0))));
                            ui.end_row();
                        }
                    });
                });
            }
        });
        if run { self.run_compute(); }
        if assign {
            let picked = self.selected_id().filter(|id| self.document.get(*id).is_some_and(|e| matches!(e.category, Category::Duct | Category::Pipe)));
            self.message = match picked {
                None => "Select a duct or pipe first".into(),
                Some(id) => {
                    let line = compute::connected_runs(&self.document, id);
                    match compute::assign_system(&mut self.document, &line, self.compute.system) {
                        Ok(()) => format!("{} run(s) now belong to {}; undo reverts", line.len(), self.compute.system.name()),
                        Err(_) => format!("{} cannot be given to a {} (ducts take air systems, pipes take water systems)", self.compute.system.name(), self.document.get(id).map_or("run", |e| if e.category == Category::Duct { "duct" } else { "pipe" })),
                    }
                }
            };
        }
        if apply {
            if let Some((_, r)) = self.compute.report.clone() {
                self.message = match compute::apply_sizes(&mut self.document, &r) {
                    Ok(n) => format!("{n} run(s) resized; undo reverts"),
                    Err(e) => format!("Sizes rejected ({e:?})"),
                };
                self.sync_duct_controls();
                self.run_compute();
            }
        }
        if let Some(id) = select { self.select_element(Some(id)); }
        self.compute.open = open;
    }
}

fn port_pos(doc: &Document, p: MepPort) -> Option<[f64; 2]> {
    match p.node {
        MepNode::Run(id) => match doc.get(id)?.geometry? { Geometry::Linear { start, end, .. } => Some(if p.port == 0 { start } else { end }), _ => None },
        MepNode::Fitting(f) => doc.mep().fitting(f).map(|f| [f.fitting.at[0], f.fitting.at[1]]),
    }
}

/// Marks the end points, and draws the most resistant path in red, over the plan.
pub(crate) fn draw_overlay(doc: &Document, report: Option<&SystemReport>, painter: &egui::Painter, screen: &dyn Fn([f64; 3]) -> Pos2, level_z: f64, tool_active: bool) {
    if let Some(r) = report {
        for id in &r.critical_runs {
            if let Some(Geometry::Linear { start, end, .. }) = doc.get(*id).and_then(|e| e.geometry) {
                painter.line_segment([screen([start[0], start[1], level_z]), screen([end[0], end[1], level_z])], Stroke::new(4.0, Color32::from_rgba_unmultiplied(220, 50, 40, 190)));
            }
        }
    }
    for e in &doc.mep().endpoints {
        let Some(p) = port_pos(doc, e.port) else { continue };
        let at = screen([p[0], p[1], level_z]);
        match e.role {
            EndRole::Source => {
                painter.rect_filled(egui::Rect::from_center_size(at, egui::Vec2::splat(12.0)), 2.0, Color32::from_rgb(40, 90, 200));
                painter.text(at + egui::Vec2::new(9.0, -9.0), egui::Align2::LEFT_BOTTOM, "source", egui::FontId::proportional(11.0), Color32::from_rgb(40, 90, 200));
            }
            EndRole::Terminal => {
                painter.circle_filled(at, 6.0, Color32::from_rgb(40, 160, 90));
                let label = match e.demand {
                    EndDemand::None => String::new(),
                    EndDemand::Flow(q) => format!("{:.2} l/s", q * 1000.0),
                    EndDemand::Power { watts, delta_t } => format!("{:.1} kW / {delta_t:.0} K", watts / 1000.0),
                    EndDemand::LoadingUnits(u) => format!("{u:.1} LU"),
                    EndDemand::Sprinkler { k_l_min_bar, .. } => format!("K{k_l_min_bar:.0}"),
                };
                let extra = report.and_then(|r| r.ends.iter().find(|x| x.port == Some(e.port))).map_or(String::new(), |x| format!("  +{:.1} kPa", x.excess / 1000.0));
                painter.text(at + egui::Vec2::new(9.0, -9.0), egui::Align2::LEFT_BOTTOM, format!("{label}{extra}"), egui::FontId::proportional(11.0), Color32::from_rgb(40, 130, 80));
            }
        }
    }
    if tool_active {
        for e in doc.elements().filter(|e| matches!(e.category, Category::Duct | Category::Pipe)) {
            for port in 0..2 {
                let mp = MepPort { node: MepNode::Run(e.id), port };
                if doc.mep().partner(mp).is_none() && doc.mep().endpoint_at(mp).is_none() {
                    if let Some(p) = port_pos(doc, mp) { painter.circle_stroke(screen([p[0], p[1], level_z]), 5.0, Stroke::new(1.5, Color32::from_rgb(230, 150, 40))); }
                }
            }
        }
    }
}


/// The commands that put the run end of `q` on the connector `c`: when the run's other end is free (nothing joined, no
/// tap on the run) the whole run moves, so it keeps its direction and square-on meeting; otherwise the end is stretched
/// to the connector and the run takes the slope the height asks. Nothing when it is there already (or cannot follow).
pub(crate) fn snap_commands(doc: &Document, q: &openbim_document::EquipLink, c: &openbim_family::mep::Connector) -> Vec<Command> {
    let MepNode::Run(id) = q.port.node else { return Vec::new() };
    let Some(e) = doc.get(id) else { return Vec::new() };
    let Some(Geometry::Linear { start, end, level }) = e.geometry else { return Vec::new() };
    let lz = doc.get(level).map_or(0.0, |l| l.elevation_m);
    let (z0, slope) = (lz + e.elevation_m, e.slope_ratio.unwrap_or(0.0));
    let plan = (end[0] - start[0]).hypot(end[1] - start[1]);
    let z1 = z0 + slope * plan;
    let (p, pz) = if q.port.port == 0 { (start, z0) } else { (end, z1) };
    let d = [c.pos[0] - p[0], c.pos[1] - p[1], c.pos[2] - pz];
    if d.iter().all(|v| v.abs() < 1e-9) { return Vec::new(); }
    let other = MepPort { node: q.port.node, port: 1 - q.port.port };
    let has_taps = doc.mep().taps.iter().any(|t| t.run == id);
    let mut out = Vec::new();
    if doc.mep().partner(other).is_none() && !has_taps {
        out.push(Command::SetGeometry { id, geometry: Geometry::Linear { start: [start[0] + d[0], start[1] + d[1]], end: [end[0] + d[0], end[1] + d[1]], level } });
        if d[2].abs() > 1e-12 { out.push(Command::MoveElevation { id, elevation_m: e.elevation_m + d[2] }); }
    } else {
        let (ns, ne) = if q.port.port == 0 { ([c.pos[0], c.pos[1]], end) } else { (start, [c.pos[0], c.pos[1]]) };
        let np = (ne[0] - ns[0]).hypot(ne[1] - ns[1]);
        if np < 0.05 || (q.port.port == 0 && has_taps) { return Vec::new(); }
        let (nz0, nz1) = if q.port.port == 0 { (c.pos[2], z1) } else { (z0, c.pos[2]) };
        out.push(Command::SetGeometry { id, geometry: Geometry::Linear { start: ns, end: ne, level } });
        out.push(Command::MoveElevation { id, elevation_m: nz0 - lz });
        out.push(Command::SetSlope { id, slope_ratio: (nz1 - nz0) / np });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tool;

    #[test]
    fn end_points_are_placed_on_open_ends_and_compute_sizes_the_drawn_duct() {
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Duct);
        app.system_pick = System::SupplyAir;
        app.elevation_mm = 3000.0;
        for p in [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0]] { app.run_click(p); }
        app.finish_run(ElementId(10));
        // a branch starting in the middle of the first leg is joined with a tee
        let n_before = app.document.elements().count();
        app.run_click([5.0, 0.0]);
        assert!(app.branch_main.is_some(), "{}", app.message);
        app.run_click([5.0, -4.0]);
        app.finish_run(ElementId(10));
        assert!(app.branch_main.is_none() && app.run_points.is_empty(), "{}", app.message);
        assert_eq!(app.document.elements().count(), n_before + 2, "{}", app.message);
        let runs: Vec<ElementId> = app.document.elements().filter(|e| e.category == Category::Duct).map(|e| e.id).collect();
        assert!(!runs.is_empty(), "{}", app.message);
        // nothing to click on far from the run
        app.choose_tool(Tool::EndPoint);
        app.end_draft.mark = Mark::Source;
        app.end_point_click(ElementId(10), [50.0, 50.0]);
        assert!(app.document.mep().endpoints.is_empty());
        app.end_point_click(ElementId(10), [0.1, 0.0]);
        app.end_draft = EndDraft { mark: Mark::Terminal, kind: 0, a: 400.0, b: 0.0, pressure_kpa: 0.0 };
        app.end_point_click(ElementId(10), [10.0, 6.1]);
        app.end_draft.a = 200.0;
        app.end_point_click(ElementId(10), [5.0, -4.1]);
        assert_eq!(app.document.mep().endpoints.len(), 3, "{}", app.message);
        // the line takes the system, the dialog computes it
        let first = runs[0];
        let line = compute::connected_runs(&app.document, first);
        compute::assign_system(&mut app.document, &line, System::SupplyAir).unwrap();
        app.compute.system = System::SupplyAir;
        (app.compute.density, app.compute.viscosity_mpas, app.compute.specific_heat, app.compute.roughness_mm) = (1.2, 0.0181, 1006.0, 0.09);
        app.compute.catalogue = "300x200,300,200\n500x300,500,300\n800x400,800,400\n1200x600,1200,600\n".into();
        app.run_compute();
        assert!(app.compute.error.is_empty(), "{}", app.compute.error);
        let (_, r) = app.compute.report.as_ref().expect("report");
        assert!((r.total_flow - 0.6).abs() < 1e-9);
        assert!(r.runs.iter().any(|x| x.critical) && r.runs.iter().any(|x| !x.critical), "the longer way is critical, the branch is not");
        assert_eq!(r.ends.len(), 2);
        // the window draws
        app.compute.open = true;
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| app.compute_window(ui.ctx()));
        out.textures_delta.clear();
        // sizes are applied and undone
        let before: Vec<f64> = line.iter().map(|id| app.document.get(*id).unwrap().width_m).collect();
        let r = app.compute.report.clone().unwrap().1;
        compute::apply_sizes(&mut app.document, &r).unwrap();
        app.undo_document();
        let after: Vec<f64> = line.iter().map(|id| app.document.get(*id).unwrap().width_m).collect();
        assert_eq!(before, after);
        // a bad catalogue line is explained
        assert!(parse_catalogue("a,b,c\n300x200,300\n", true).is_err());
    }

    #[test]
    fn a_loaded_families_airflow_drives_the_sized_duct() {
        let mut app = Desktop::sample();
        let lv = ElementId(10);
        // a diffuser whose air flow comes from a loaded catalogue type -- not the library default (0.05)
        let msg = app.load_family_catalogue_text("family\tCeiling diffuser, square\ntype\tneck_d\tair_flow\nD-400\t0.3\t0.4\n", "d.tsv");
        assert!(msg.contains("1 type"), "{msg}");
        let t = app.document.types_for(Category::Family).iter().find(|t| t.name == "D-400").map(|t| t.id).unwrap();
        let spec = openbim_document::FamilySpec { family: "Ceiling diffuser, square".into(), type_name: String::new(), overrides: Vec::new(), angle_deg: 0.0, elevation_m: 2.7 };
        let diff = app.document.place_family(lv, [5.0, 5.0], &spec).unwrap();
        app.document.assign_type(&[diff], t).unwrap();
        // the type layer reaches the connector: 0.4 m3/s, not the library default 0.05
        let air = crate::elec_ui::instance_connectors(&app.document, diff).into_iter()
            .find(|c| c.domain == Domain::Duct && c.flow > 0.0).expect("an air connector").clone();
        assert!((air.flow - 0.4).abs() < 1e-12, "the loaded type's air_flow reached the connector: {}", air.flow);
        // a duct to carry it, its terminal end taking the type's airflow as the demand
        let id = app.document.next_id();
        let port = |p: usize| MepPort { node: MepNode::Run(id), port: p };
        let end = |p: usize, role: EndRole, demand: EndDemand, rp: f64| MepEndpoint { port: port(p), role, demand, required_pressure_pa: rp };
        app.document.transact(app.document.revision(), &[
            Command::Create { id, category: Category::Duct, name: "Duct".into(), width_m: 0.6, height_m: 0.3, elevation_m: 2.7 },
            Command::SetSlope { id, slope_ratio: 0.0 },
            Command::SetGeometry { id, geometry: Geometry::Linear { start: [0.0, 0.0], end: [20.0, 0.0], level: lv } },
            Command::SetMep(Box::new(openbim_document::MepData {
                run_systems: vec![(id, System::SupplyAir)],
                endpoints: vec![end(0, EndRole::Source, EndDemand::None, 0.0), end(1, EndRole::Terminal, EndDemand::Flow(air.flow), 0.0)],
                ..Default::default()
            })),
        ]).unwrap();
        // size it for air
        app.compute.system = System::SupplyAir;
        (app.compute.density, app.compute.viscosity_mpas, app.compute.specific_heat, app.compute.roughness_mm) = (1.2, 0.0181, 1006.0, 0.09);
        app.compute.max_velocity = 6.0;
        app.compute.max_friction = 1.0;
        app.compute.catalogue = "300x200,300,200\n500x300,500,300\n800x400,800,400\n1200x600,1200,600\n".into();
        app.run_compute();
        assert!(app.compute.error.is_empty(), "{}", app.compute.error);
        let r = app.compute.report.as_ref().expect("a report").1.clone();
        // the loaded type's air flow is the system's total flow: the type drives the sizing
        assert!((r.total_flow - air.flow).abs() < 1e-9, "total_flow {} == the type's air flow {}", r.total_flow, air.flow);
        let sized = &r.runs[0];
        assert!(sized.velocity <= 6.0 && sized.friction_per_metre <= 1.0, "velocity {} friction {}", sized.velocity, sized.friction_per_metre);
        assert!(!sized.exceeds_limits, "the sized duct carries the type's airflow within the criteria: v={}, f={}", sized.velocity, sized.friction_per_metre);
        // the placeholder duct is resized to the catalogue size that carries the flow
        let before = app.document.get(id).unwrap().width_m;
        compute::apply_sizes(&mut app.document, &r).unwrap();
        let after = app.document.get(id).unwrap().width_m;
        assert!(after != before, "the run was resized from its placeholder {} to the catalogue size that carries the airflow", before);
    }
}
