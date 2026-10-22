//! The electrical network in the drawing: place a source, boards and loads, draw the cables between them, then
//! compute (cable, protective device, voltage drop, fault checks per IEC 60364 with the owner's tables as CSV).
use crate::Desktop;
use eframe::egui::{self, Color32, Pos2, Stroke};
use openbim_calc as calc;
use openbim_document::compute_elec::{self, ElecComputeError, ElecComputeSettings, ElecReport};
use openbim_document::{Category, Command, ElecCable, ElecKind, ElecNode, Document, ElementId, Geometry};
use openbim_family::mep::{Connector, Domain, Pose, System};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode { Place, Edit, Remove }

/// What the node tool places and the cable tool draws.
#[derive(Clone)]
pub(crate) struct ElecDraft {
    pub mode: Mode,
    pub kind: ElecKind,
    pub name: String,
    pub kw: f64,
    pub power_factor: f64,
    pub efficiency: f64,
    pub motor: bool,
    pub start_multiple: f64,
    pub start_pf: f64,
    pub three_phase: bool,
    pub extra_m: f64,
    pub grouped: u32,
    pub ambient: Option<f64>,
    pub demand_factor: f64,
    pub rcd_ma: Option<f64>,
    /// Load only: take the place and the power from the electrical connector of the family instance clicked
    pub from_equipment: bool,
    /// the node (Edit mode, node tool) or the cable, named by the node it feeds (Edit mode, cable tool), being edited
    pub selected: Option<u64>,
}

impl Default for ElecDraft {
    fn default() -> Self {
        Self { mode: Mode::Place, kind: ElecKind::Load, name: String::new(), kw: 10.0, power_factor: 0.9, efficiency: 1.0, motor: false, start_multiple: 6.0, start_pf: 0.35,
            three_phase: true, extra_m: 0.0, grouped: 1, ambient: None, demand_factor: 1.0, rcd_ma: None, from_equipment: false, selected: None }
    }
}

pub(crate) struct ElecDialog {
    pub open: bool,
    pub line_voltage: f64,
    pub max_total_drop: f64,
    pub max_branch_drop: f64,
    pub max_start_drop: f64,
    pub max_parallel: u32,
    /// 0 TN, 1 TT, 2 IT
    pub earthing: usize,
    pub electrode_ohm: f64,
    pub neutral_distributed: bool,
    pub use_fault: bool,
    pub isc_ka: f64,
    pub x_over_r: f64,
    pub clearing_s: f64,
    pub c_min: f64,
    pub hot_factor: f64,
    pub trip_multiple: f64,
    pub touch_v: f64,
    pub source_kva: f64,
    pub k: f64,
    pub table: String,
    pub grouping: String,
    pub temperature: String,
    pub harmonics: String,
    pub devices: String,
    pub pe_rule: String,
    pub report: Option<(u64, ElecReport)>,
    pub error: String,
}

impl Default for ElecDialog {
    fn default() -> Self {
        Self {
            open: false, line_voltage: 400.0, max_total_drop: 5.0, max_branch_drop: 0.0, max_start_drop: 0.0, max_parallel: 1, earthing: 0, electrode_ohm: 10.0,
            neutral_distributed: false, use_fault: false, isc_ka: 25.0, x_over_r: 2.0, clearing_s: 0.4, c_min: 0.95, hot_factor: 1.2, trip_multiple: 10.0, touch_v: 50.0,
            source_kva: 0.0, k: 115.0, table: String::new(), grouping: String::new(), temperature: String::new(), harmonics: String::new(), devices: String::new(), pe_rule: String::new(),
            report: None, error: String::new(),
        }
    }
}

/// The connectors of a family instance (a chiller, a fan coil), solved and put where the instance stands.
pub(crate) fn instance_connectors(doc: &Document, id: ElementId) -> Vec<Connector> {
    let Some(el) = doc.get(id).filter(|e| e.category == Category::Family) else { return Vec::new() };
    let Some(Geometry::Placed { at, level }) = el.geometry else { return Vec::new() };
    let Some(f) = crate::export::library().iter().find(|f| f.name() == el.family) else { return Vec::new() };
    // P2: the instance's own values and those of its type of a family, so a type loaded from a catalogue sets the
    // size of the ports a run meets here (and the exported ports with them)
    let parts = crate::export::request_parts(doc, el, f);
    let Ok(inst) = f.solve(&parts.request((!el.type_name.is_empty()).then_some(el.type_name.as_str()))) else { return Vec::new() };
    let z = doc.get(level).map_or(0.0, |l| l.elevation_m) + el.elevation_m;
    let pose = Pose { x: at[0], y: at[1], z, angle: el.angle_deg };
    inst.connectors.iter().map(|c| c.world(&pose)).collect()
}

/// The family instance nearest a plan point (within `tol`) that has a connector of `domain`
/// (and of `system`, when given), with that connector.
pub(crate) fn equipment_connector_near(doc: &Document, level: ElementId, at: [f64; 2], tol: f64, domain: Domain, system: Option<System>) -> Option<(ElementId, Connector)> {
    doc.elements().filter(|e| e.category == Category::Family && matches!(e.geometry, Some(Geometry::Placed { level: l, .. }) if l == level)).flat_map(|e| {
        instance_connectors(doc, e.id).into_iter().filter(|c| c.domain == domain && system.is_none_or(|s| c.system == s)).map(move |c| (e.id, c))
    }).map(|(id, c)| ((c.pos[0] - at[0]).hypot(c.pos[1] - at[1]), id, c)).filter(|(d, _, _)| *d <= tol).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, id, c)| (id, c))
}

fn table_err(what: &str, e: calc::TableError) -> String {
    match e { calc::TableError::TooShort => format!("{what}: no usable row"), calc::TableError::Line(n, w) => format!("{what}, line {n}: {w}") }
}

fn describe(e: &ElecComputeError, doc: &Document) -> String {
    let name = |id: &u64| doc.mep().elec_nodes.iter().find(|n| n.id == *id).map_or("a node".into(), |n| format!("\"{}\"", n.name));
    match e {
        ElecComputeError::NoSource => "No source. Place one with the Electrical Node tool (kind Source), then draw cables from it.".into(),
        ElecComputeError::MoreThanOneSource => "More than one source; compute one supply at a time (remove the other).".into(),
        ElecComputeError::NoLoad => "No load with a power. Place loads with the Electrical Node tool.".into(),
        ElecComputeError::NotFed(id) => format!("{} is not fed by any cable. Draw a cable to it, or remove it.", name(id)),
        ElecComputeError::Loop(id) => format!("The cables form a loop at {}; the network must be a tree.", name(id)),
        ElecComputeError::Calc(err, node) => {
            let at = node.map_or(String::new(), |id| format!(" at {}", name(&id)));
            match err {
                calc::EError::NoCable(_) => format!("No cable in the table is enough{at}: add larger sizes, or allow parallel cables."),
                calc::EError::NoRating(_) => format!("The design current{at} is above the largest device rating in the list."),
                calc::EError::DropNotMet(_) => format!("The voltage drop limit cannot be met{at}, even with the largest cables."),
                calc::EError::BadSettings(w) => format!("Check the settings: {w}."),
                calc::EError::BadValue(_, w) => format!("A value is not usable{at}: {w}."),
                other => format!("{other:?}"),
            }
        }
    }
}

impl Desktop {
    fn nearest_node(&self, level: ElementId, at: [f64; 2], tol: f64) -> Option<u64> {
        self.document.mep().elec_nodes.iter().filter(|n| n.level == level).map(|n| ((n.at[0] - at[0]).hypot(n.at[1] - at[1]), n.id)).filter(|(d, _)| *d <= tol).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, id)| id)
    }

    fn set_elec(&mut self, data: openbim_document::MepData, ok: String) {
        self.message = match self.document.transact(self.document.revision(), &[Command::SetMep(Box::new(data))]) { Ok(_) => ok, Err(e) => format!("Rejected ({e:?})") };
    }

    /// A click of the Electrical Node tool.
    pub(crate) fn elec_node_click(&mut self, level: ElementId, at: [f64; 2]) {
        let mut data = self.document.mep().clone();
        let hit = self.nearest_node(level, at, 0.4);
        if self.elec_draft.mode == Mode::Remove {
            let Some(id) = hit else { self.message = "Nothing to remove there".into(); return };
            data.elec_nodes.retain(|n| n.id != id);
            data.elec_cables.retain(|c| c.from != id && c.to != id);
            return self.set_elec(data, "Node and its cables removed; undo brings them back".into());
        }
        if self.elec_draft.mode == Mode::Edit {
            let Some(id) = hit else { self.message = "Edit: click a node to change it".into(); return };
            let n = data.elec_nodes.iter().find(|n| n.id == id).cloned().expect("hit node exists");
            let d = &mut self.elec_draft;
            (d.selected, d.kind, d.name, d.kw, d.power_factor, d.efficiency) = (Some(id), n.kind, n.name.clone(), n.watts / 1000.0, n.power_factor, n.efficiency);
            d.motor = n.motor.is_some();
            if let Some((a, b)) = n.motor { (d.start_multiple, d.start_pf) = (a, b); }
            self.message = format!("{} selected: change the values in the options bar, then press Apply", n.name);
            return;
        }
        if hit.is_some() { self.message = "There is a node there already".into(); return; }
        let d = self.elec_draft.clone();
        if d.from_equipment && d.kind == ElecKind::Load {
            let Some((el, c)) = equipment_connector_near(&self.document, level, at, 1.5, Domain::Electrical, Some(System::Power)) else { self.message = "No equipment with an electrical power connector within 1.5 m of the click".into(); return };
            if data.elec_nodes.iter().any(|n| n.element == Some(el) && n.connector == c.name) { self.message = "That connector feeds a load already".into(); return; }
            let name = self.document.get(el).map_or("Equipment".into(), |e| e.name.clone());
            let mut node = ElecNode::new(data.next_node_id(), &name, ElecKind::Load, [c.pos[0], c.pos[1]], level);
            (node.watts, node.power_factor, node.element, node.connector) = (c.flow, c.power_factor, Some(el), c.name.clone());
            let (eff, motor) = (d.efficiency, d.motor.then_some((d.start_multiple, d.start_pf)));
            (node.efficiency, node.motor) = (eff, motor);
            data.elec_nodes.push(node);
            return self.set_elec(data, format!("{name} connected: {:.1} kW from its connector '{}'; undo removes it", c.flow / 1000.0, c.name));
        }
        if d.kind == ElecKind::Source && data.elec_nodes.iter().any(|n| n.kind == ElecKind::Source) { self.message = "There is a source already; compute one supply at a time".into(); return; }
        let n = data.elec_nodes.iter().filter(|n| n.kind == d.kind).count() + 1;
        let name = if d.name.trim().is_empty() { format!("{} {n}", match d.kind { ElecKind::Source => "Source", ElecKind::Board => "Board", ElecKind::Load => "Load" }) } else { d.name.trim().to_string() };
        let mut node = ElecNode::new(data.next_node_id(), &name, d.kind, [(at[0] / 0.05).round() * 0.05, (at[1] / 0.05).round() * 0.05], level);
        if d.kind == ElecKind::Load {
            (node.watts, node.power_factor, node.efficiency) = (d.kw * 1000.0, d.power_factor, d.efficiency);
            node.motor = d.motor.then_some((d.start_multiple, d.start_pf));
        }
        data.elec_nodes.push(node);
        self.set_elec(data, format!("{name} placed; undo removes it"));
    }

    /// A click of the Cable tool: the feeding node, the points on the way, the fed node.
    pub(crate) fn cable_click(&mut self, level: ElementId, at: [f64; 2]) {
        let hit = self.nearest_node(level, at, 0.4);
        let mut data = self.document.mep().clone();
        if self.elec_draft.mode == Mode::Remove {
            let find = |id: u64| data.elec_nodes.iter().find(|n| n.id == id).map(|n| n.at);
            let best = data.elec_cables.iter().enumerate().filter_map(|(i, c)| {
                let mut pts = vec![find(c.from)?];
                pts.extend(c.via.iter().copied());
                pts.push(find(c.to)?);
                let d = pts.windows(2).map(|w| seg_dist(at, w[0], w[1])).fold(f64::MAX, f64::min);
                (d <= 0.3).then_some((d, i))
            }).min_by(|a, b| a.0.total_cmp(&b.0));
            let Some((_, i)) = best else { self.message = "No cable there".into(); return };
            data.elec_cables.remove(i);
            return self.set_elec(data, "Cable removed; undo brings it back".into());
        }
        if self.elec_draft.mode == Mode::Edit {
            let find = |id: u64| data.elec_nodes.iter().find(|n| n.id == id).map(|n| n.at);
            let best = data.elec_cables.iter().filter_map(|c| {
                let mut pts = vec![find(c.from)?];
                pts.extend(c.via.iter().copied());
                pts.push(find(c.to)?);
                let d = pts.windows(2).map(|w| seg_dist(at, w[0], w[1])).fold(f64::MAX, f64::min);
                (d <= 0.3).then_some((d, c.clone()))
            }).min_by(|a, b| a.0.total_cmp(&b.0));
            let Some((_, c)) = best else { self.message = "Edit: click a cable to change it".into(); return };
            let d = &mut self.elec_draft;
            (d.selected, d.three_phase, d.extra_m, d.grouped, d.ambient, d.demand_factor, d.rcd_ma) = (Some(c.to), c.three_phase, c.extra_m, c.grouped_circuits, c.ambient_c, c.demand_factor, c.rcd_ma);
            self.message = "Cable selected: change the values in the options bar, then press Apply".into();
            return;
        }
        match (self.cable_from, hit) {
            (None, Some(id)) => {
                if data.elec_nodes.iter().find(|n| n.id == id).is_some_and(|n| n.kind == ElecKind::Load) { self.message = "A load does not feed anything: start the cable at the source or a board".into(); return; }
                self.cable_from = Some(id);
                self.cable_via.clear();
                self.message = "Cable: click the points on the way, then the node it feeds (Esc cancels)".into();
            }
            (None, None) => self.message = "Cable: start by clicking the source or a board".into(),
            (Some(_), None) => self.cable_via.push([(at[0] / 0.05).round() * 0.05, (at[1] / 0.05).round() * 0.05]),
            (Some(from), Some(to)) => {
                if from == to { self.message = "A cable needs two different nodes".into(); return; }
                if data.elec_cables.iter().any(|c| c.to == to) { self.message = "That node is fed by a cable already; remove it first".into(); return; }
                if data.elec_nodes.iter().find(|n| n.id == to).is_some_and(|n| n.kind == ElecKind::Source) { self.message = "A source is not fed by a cable".into(); return; }
                let d = self.elec_draft.clone();
                let mut c = ElecCable::new(from, to, d.three_phase);
                c.via = std::mem::take(&mut self.cable_via);
                (c.extra_m, c.grouped_circuits, c.ambient_c, c.demand_factor, c.rcd_ma) = (d.extra_m, d.grouped.max(1), d.ambient, d.demand_factor, d.rcd_ma);
                data.elec_cables.push(c);
                self.cable_from = None;
                self.set_elec(data, "Cable placed; undo removes it".into());
            }
        }
    }

    pub(crate) fn elec_options(&mut self, ui: &mut egui::Ui) {
        let node_tool = self.tool == crate::Tool::ElecNode;
        let d = &mut self.elec_draft;
        ui.strong(if node_tool { "Electrical Node" } else { "Cable" });
        ui.separator();
        ui.selectable_value(&mut d.mode, Mode::Place, "Place");
        ui.selectable_value(&mut d.mode, Mode::Edit, "Edit");
        ui.selectable_value(&mut d.mode, Mode::Remove, "Remove");
        if d.mode == Mode::Remove { return; }
        if d.mode == Mode::Edit && d.selected.is_none() { ui.weak("Click a node or cable to change it"); return; }
        if node_tool {
            ui.selectable_value(&mut d.kind, ElecKind::Source, "Source");
            ui.selectable_value(&mut d.kind, ElecKind::Board, "Board");
            ui.selectable_value(&mut d.kind, ElecKind::Load, "Load");
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(90.0));
            if d.kind == ElecKind::Load { ui.checkbox(&mut d.from_equipment, "From equipment").on_hover_text("Click near an equipment family with an electrical connector: the load sits at the connector and takes its power"); }
            if d.kind == ElecKind::Load && !d.from_equipment {
                ui.label("kW");
                ui.add(egui::DragValue::new(&mut d.kw).speed(0.1).range(0.0..=1.0e5));
                ui.label("pf");
                ui.add(egui::DragValue::new(&mut d.power_factor).speed(0.01).range(0.05..=1.0));
                ui.label("efficiency");
                ui.add(egui::DragValue::new(&mut d.efficiency).speed(0.01).range(0.05..=1.0));
            }
            if d.kind == ElecKind::Load {
                ui.checkbox(&mut d.motor, "Motor started direct");
                if d.motor {
                    ui.add(egui::DragValue::new(&mut d.start_multiple).speed(0.1).range(1.0..=20.0).suffix(" x In"));
                    ui.add(egui::DragValue::new(&mut d.start_pf).speed(0.01).range(0.05..=1.0).suffix(" pf start"));
                }
            }
        } else {
            ui.checkbox(&mut d.three_phase, "Three-phase");
            ui.label("Extra length (m)");
            ui.add(egui::DragValue::new(&mut d.extra_m).speed(0.1).range(0.0..=1000.0));
            ui.label("Circuits grouped");
            ui.add(egui::DragValue::new(&mut d.grouped).range(1..=50));
            let mut amb = d.ambient.is_some();
            ui.checkbox(&mut amb, "Ambient");
            if amb { let t = d.ambient.get_or_insert(30.0); ui.add(egui::DragValue::new(t).speed(0.5).range(-20.0..=90.0).suffix(" C")); } else { d.ambient = None; }
            ui.label("Demand factor");
            ui.add(egui::DragValue::new(&mut d.demand_factor).speed(0.01).range(0.05..=1.0));
            let mut rcd = d.rcd_ma.is_some();
            ui.checkbox(&mut rcd, "RCD");
            if rcd { let m = d.rcd_ma.get_or_insert(30.0); ui.add(egui::DragValue::new(m).range(1.0..=1000.0).suffix(" mA")); } else { d.rcd_ma = None; }
        }
        if d.mode == Mode::Edit && ui.button("Apply").clicked() { self.elec_apply(); }
    }

    /// Moves every load tied to equipment to where its connector is now and gives it the connector's power again. Gives
    /// the number changed. One undoable step.
    pub(crate) fn refresh_equipment_loads(&mut self) -> usize {
        let mut data = self.document.mep().clone();
        let mut changed = 0;
        for n in data.elec_nodes.iter_mut() {
            let Some(el) = n.element else { continue };
            let Some(c) = instance_connectors(&self.document, el).into_iter().find(|c| c.name == n.connector && c.domain == Domain::Electrical) else { continue };
            let (at, w, pf) = ([c.pos[0], c.pos[1]], c.flow, c.power_factor);
            if n.at != at || n.watts != w || n.power_factor != pf { (n.at, n.watts, n.power_factor) = (at, w, pf); changed += 1; }
        }
        if changed > 0 { self.set_elec(data, format!("{changed} load(s) updated from their equipment; undo reverts")); }
        changed
    }

    /// Writes the options bar's values to the node or cable being edited.
    pub(crate) fn elec_apply(&mut self) {
        let Some(id) = self.elec_draft.selected else { return };
        let d = self.elec_draft.clone();
        let mut data = self.document.mep().clone();
        if self.tool == crate::Tool::ElecNode {
            let Some(n) = data.elec_nodes.iter_mut().find(|n| n.id == id) else { self.message = "That node is gone".into(); return };
            if !d.name.trim().is_empty() { n.name = d.name.trim().to_string(); }
            if n.kind == ElecKind::Load && n.element.is_some() {
                // power and place come from the equipment's connector; the rest is the designer's
                n.efficiency = d.efficiency;
                n.motor = d.motor.then_some((d.start_multiple, d.start_pf));
            } else if n.kind == ElecKind::Load {
                (n.watts, n.power_factor, n.efficiency) = (d.kw * 1000.0, d.power_factor, d.efficiency);
                n.motor = d.motor.then_some((d.start_multiple, d.start_pf));
            }
            self.set_elec(data, "Node changed; undo reverts".into());
        } else {
            let Some(c) = data.elec_cables.iter_mut().find(|c| c.to == id) else { self.message = "That cable is gone".into(); return };
            (c.three_phase, c.extra_m, c.grouped_circuits, c.ambient_c, c.demand_factor, c.rcd_ma) = (d.three_phase, d.extra_m, d.grouped.max(1), d.ambient, d.demand_factor, d.rcd_ma);
            self.set_elec(data, "Cable changed; undo reverts".into());
        }
    }

    fn elec_settings(&self) -> Result<ElecComputeSettings, String> {
        let d = &self.elec;
        let mut table = calc::CableTable::from_csv("cables", d.k, &d.table).map_err(|e| table_err("cable table", e))?;
        if !d.grouping.trim().is_empty() { table = table.with_grouping(calc::FactorTable::from_csv(calc::Lookup::Step, &d.grouping).map_err(|e| table_err("grouping factors", e))?); }
        if !d.temperature.trim().is_empty() { table = table.with_temperature(calc::FactorTable::from_csv(calc::Lookup::Interpolate, &d.temperature).map_err(|e| table_err("temperature factors", e))?); }
        if !d.harmonics.trim().is_empty() { table = table.with_harmonics(calc::FactorTable::from_csv(calc::Lookup::Step, &d.harmonics).map_err(|e| table_err("harmonic factors", e))?); }
        let mut s = calc::ESettings::new(d.line_voltage, vec![table], d.max_total_drop);
        if !d.devices.trim().is_empty() { s.devices = calc::parse_devices(&d.devices).map_err(|e| table_err("devices", e))?; }
        if !d.pe_rule.trim().is_empty() { s.pe_rule = Some(calc::parse_pe_rule(&d.pe_rule).map_err(|e| table_err("protective conductor rule", e))?); }
        s.max_branch_drop_pct = (d.max_branch_drop > 0.0).then_some(d.max_branch_drop);
        s.max_start_drop_pct = (d.max_start_drop > 0.0).then_some(d.max_start_drop);
        s.max_parallel = d.max_parallel.max(1);
        s.earthing = match d.earthing { 1 => calc::Earthing::Tt { electrode_ohm: d.electrode_ohm }, 2 => calc::Earthing::It { neutral_distributed: d.neutral_distributed }, _ => calc::Earthing::Tn };
        s.touch_voltage_v = d.touch_v;
        s.source_va = (d.source_kva > 0.0).then_some(d.source_kva * 1000.0);
        if d.use_fault { s.fault = Some(calc::Fault { isc_a: d.isc_ka * 1000.0, x_over_r: d.x_over_r, clearing_time_s: d.clearing_s, c_min: d.c_min, hot_resistance_factor: d.hot_factor, trip_multiple: d.trip_multiple }); }
        Ok(ElecComputeSettings { calc: s })
    }

    pub(crate) fn run_elec_compute(&mut self) {
        self.elec.error.clear();
        self.refresh_equipment_loads();
        let settings = match self.elec_settings() { Ok(s) => s, Err(e) => { self.elec.error = e; return } };
        match compute_elec::compute_electrical(&self.document, &settings) {
            Ok(r) => {
                self.message = format!("Electrical computed: {} cables, {:.1} kW, {:.1} kVA", r.cables.len(), r.total_kw, r.total_kva);
                self.elec.report = Some((self.document.revision(), r));
            }
            Err(e) => { self.elec.report = None; self.elec.error = describe(&e, &self.document); }
        }
    }

    pub(crate) fn elec_window(&mut self, ctx: &egui::Context) {
        if !self.elec.open { return; }
        let mut open = true;
        let mut run = false;
        egui::Window::new("Compute electrical").open(&mut open).default_width(640.0).show(ctx, |ui| {
            let d = &mut self.elec;
            egui::CollapsingHeader::new("Settings").default_open(true).show(ui, |ui| {
                egui::Grid::new("elec_grid").num_columns(4).show(ui, |ui| {
                    ui.label("Line voltage (V)"); ui.add(egui::DragValue::new(&mut d.line_voltage).speed(1.0).range(50.0..=100000.0));
                    ui.label("Max drop, whole path (%)"); ui.add(egui::DragValue::new(&mut d.max_total_drop).speed(0.1).range(0.1..=50.0)); ui.end_row();
                    ui.label("Max drop, one cable (%, 0 = none)"); ui.add(egui::DragValue::new(&mut d.max_branch_drop).speed(0.1).range(0.0..=50.0));
                    ui.label("Max motor start drop (%, 0 = none)"); ui.add(egui::DragValue::new(&mut d.max_start_drop).speed(0.1).range(0.0..=50.0)); ui.end_row();
                    ui.label("Cables in parallel, at most"); ui.add(egui::DragValue::new(&mut d.max_parallel).range(1..=12));
                    ui.label("Adiabatic constant k"); ui.add(egui::DragValue::new(&mut d.k).speed(1.0).range(1.0..=400.0)); ui.end_row();
                    ui.label("Source rating (kVA, 0 = unknown)"); ui.add(egui::DragValue::new(&mut d.source_kva).speed(10.0).range(0.0..=1.0e6));
                    ui.label("Touch voltage limit (V)"); ui.add(egui::DragValue::new(&mut d.touch_v).speed(1.0).range(10.0..=120.0)); ui.end_row();
                });
                ui.horizontal(|ui| {
                    ui.label("Earthing");
                    ui.selectable_value(&mut d.earthing, 0, "TN");
                    ui.selectable_value(&mut d.earthing, 1, "TT");
                    ui.selectable_value(&mut d.earthing, 2, "IT");
                    if d.earthing == 1 { ui.add(egui::DragValue::new(&mut d.electrode_ohm).speed(0.5).range(0.1..=1000.0).suffix(" ohm electrode")); }
                    if d.earthing == 2 { ui.checkbox(&mut d.neutral_distributed, "Neutral distributed"); }
                });
                ui.checkbox(&mut d.use_fault, "Check short circuit and fault protection");
                if d.use_fault {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(egui::DragValue::new(&mut d.isc_ka).speed(0.5).range(0.1..=500.0).suffix(" kA at source"));
                        ui.add(egui::DragValue::new(&mut d.x_over_r).speed(0.1).range(0.0..=50.0).suffix(" X/R"));
                        ui.add(egui::DragValue::new(&mut d.clearing_s).speed(0.01).range(0.001..=5.0).suffix(" s clearing"));
                        ui.add(egui::DragValue::new(&mut d.c_min).speed(0.01).range(0.5..=1.2).suffix(" c min"));
                        ui.add(egui::DragValue::new(&mut d.hot_factor).speed(0.01).range(1.0..=2.0).suffix(" hot R factor"));
                        ui.add(egui::DragValue::new(&mut d.trip_multiple).speed(0.5).range(1.0..=50.0).suffix(" x In trip"));
                    });
                }
                for (label, text, hint) in [
                    ("Cable table: label, area mm2, R ohm/km, X ohm/km, ampacity A", &mut d.table, true),
                    ("Grouping factors: circuits, factor", &mut d.grouping, false),
                    ("Temperature factors: degrees C, factor", &mut d.temperature, false),
                    ("Harmonic factors: third harmonic %, factor", &mut d.harmonics, false),
                    ("Device ratings: rating A, breaking kA", &mut d.devices, false),
                    ("Protective conductor rule: phase mm2, PE mm2", &mut d.pe_rule, false),
                ] {
                    ui.label(label);
                    ui.add(egui::TextEdit::multiline(text).desired_rows(if hint { 4 } else { 2 }).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
                }
                if ui.small_button("Load the cable table from a CSV file...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("Cable table").add_filter("CSV", &["csv", "txt"]).pick_file() {
                        match std::fs::read_to_string(&p) { Ok(t) => d.table = t, Err(e) => d.error = format!("{}: {e}", p.display()) }
                    }
                }
            });
            if ui.button("Compute").clicked() { run = true; }
            if !self.elec.error.is_empty() { ui.colored_label(Color32::from_rgb(200, 60, 40), &self.elec.error); }
            let now = self.document.revision();
            if let Some((rev, r)) = &self.elec.report {
                if *rev != now { ui.colored_label(Color32::from_rgb(190, 120, 20), "The drawing has changed since this was computed; compute again."); }
                ui.separator();
                ui.label(format!("{:.1} kW, {:.1} kVA at the source, power factor {:.2}{}. The most loaded path is in red.", r.total_kw, r.total_kva, r.source_power_factor, r.source_loading_pct.map_or(String::new(), |p| format!(", source loaded {p:.0} %"))));
                let name = |id: u64| self.document.mep().elec_nodes.iter().find(|n| n.id == id).map_or("?".to_string(), |n| n.name.clone());
                egui::ScrollArea::both().max_height(280.0).show(ui, |ui| {
                    egui::Grid::new("elec_cables").striped(true).num_columns(11).show(ui, |ui| {
                        for h in ["From", "To", "m", "Ib A", "Device A", "Cable", "Capacity A", "Drop %", "Path %", "PE mm2", "Checks"] { ui.strong(h); }
                        ui.end_row();
                        for c in &r.cables {
                            ui.label(name(c.from)); ui.label(name(c.to));
                            ui.label(format!("{:.1}", c.length_m)); ui.label(format!("{:.1}", c.design_current)); ui.label(format!("{:.0}", c.device_a));
                            ui.label(if c.parallel > 1 { format!("{} x {}", c.parallel, c.cable) } else { c.cable.clone() });
                            ui.label(format!("{:.0}", c.capacity_a)); ui.label(format!("{:.2}", c.drop_pct)); ui.label(format!("{:.2}", c.path_drop_pct)); ui.label(format!("{:.1}", c.pe_area));
                            let mut flags = Vec::new();
                            if c.critical { flags.push("critical"); }
                            if !c.trip_ok { flags.push("trip too slow"); }
                            if !c.breaking_ok { flags.push("breaking capacity"); }
                            if c.rcd_ok == Some(false) { flags.push("RCD"); }
                            if c.fails { flags.push("FAILS"); }
                            ui.label(flags.join(", "));
                            ui.end_row();
                        }
                    });
                    ui.separator();
                    egui::Grid::new("elec_ends").striped(true).num_columns(4).show(ui, |ui| {
                        for h in ["End", "Drop to it %", "Margin %", "Motor start drop %"] { ui.strong(h); }
                        ui.end_row();
                        for (id, drop, margin, start) in &r.ends {
                            ui.label(name(*id)); ui.label(format!("{drop:.2}")); ui.label(format!("{margin:.2}")); ui.label(start.map_or("-".into(), |s| format!("{s:.2}")));
                            ui.end_row();
                        }
                    });
                });
            }
        });
        if run { self.run_elec_compute(); }
        self.elec.open = open;
    }
}

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

/// Draws the nodes and cables of the level, the most loaded path in red, and the cable being drawn.
pub(crate) fn draw_overlay(doc: &Document, report: Option<&ElecReport>, painter: &egui::Painter, screen: &dyn Fn([f64; 3]) -> Pos2, level: ElementId, z: f64, pending: Option<(u64, &[[f64; 2]])>) {
    let m = doc.mep();
    let at = |p: [f64; 2]| screen([p[0], p[1], z]);
    let node = |id: u64| m.elec_nodes.iter().find(|n| n.id == id);
    for c in &m.elec_cables {
        let (Some(a), Some(b)) = (node(c.from), node(c.to)) else { continue };
        if a.level != level && b.level != level { continue; }
        let mut pts = vec![at(a.at)];
        pts.extend(c.via.iter().map(|p| at(*p)));
        pts.push(at(b.at));
        let crit = report.is_some_and(|r| r.cables.iter().any(|x| x.to == c.to && x.critical));
        let stroke = if crit { Stroke::new(3.5, Color32::from_rgba_unmultiplied(220, 50, 40, 200)) } else { Stroke::new(1.8, Color32::from_rgb(150, 90, 20)) };
        for w in pts.windows(2) { painter.line_segment([w[0], w[1]], stroke); }
        if let Some(x) = report.and_then(|r| r.cables.iter().find(|x| x.to == c.to)) {
            let mid = pts[pts.len() / 2];
            let label = format!("{}{} {:.0}A", if x.parallel > 1 { format!("{}x", x.parallel) } else { String::new() }, x.cable, x.device_a);
            painter.text(mid + egui::Vec2::new(4.0, -4.0), egui::Align2::LEFT_BOTTOM, label, egui::FontId::proportional(10.0), if x.fails { Color32::from_rgb(200, 40, 40) } else { Color32::from_rgb(110, 70, 20) });
        }
    }
    if let Some((from, via)) = pending {
        if let Some(n) = node(from) {
            let mut pts = vec![at(n.at)];
            pts.extend(via.iter().map(|p| at(*p)));
            for w in pts.windows(2) { painter.line_segment([w[0], w[1]], Stroke::new(1.5, Color32::from_rgb(230, 150, 40))); }
        }
    }
    for n in m.elec_nodes.iter().filter(|n| n.level == level) {
        let p = at(n.at);
        let col = match n.kind { ElecKind::Source => Color32::from_rgb(200, 40, 40), ElecKind::Board => Color32::from_rgb(40, 90, 200), ElecKind::Load => Color32::from_rgb(40, 150, 80) };
        match n.kind {
            ElecKind::Source => { painter.add(egui::Shape::convex_polygon(vec![p + egui::Vec2::new(0.0, -8.0), p + egui::Vec2::new(8.0, 0.0), p + egui::Vec2::new(0.0, 8.0), p + egui::Vec2::new(-8.0, 0.0)], col, Stroke::NONE)); }
            ElecKind::Board => { painter.rect_filled(egui::Rect::from_center_size(p, egui::Vec2::splat(12.0)), 1.0, col); }
            ElecKind::Load => { painter.circle_filled(p, 6.0, col); }
        }
        let text = if n.watts > 0.0 { format!("{} {:.1} kW", n.name, n.watts / 1000.0) } else { n.name.clone() };
        painter.text(p + egui::Vec2::new(9.0, -9.0), egui::Align2::LEFT_BOTTOM, text, egui::FontId::proportional(11.0), col);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tool;

    const TABLE: &str = "1.5mm2,1.5,12.1,0.1,19\n2.5mm2,2.5,7.41,0.1,26\n4mm2,4,4.61,0.1,35\n6mm2,6,3.08,0.1,44\n10mm2,10,1.83,0.08,60\n16mm2,16,1.15,0.08,80\n25mm2,25,0.727,0.08,105\n35mm2,35,0.524,0.08,128\n50mm2,50,0.387,0.08,155\n";

    #[test]
    fn nodes_and_cables_are_drawn_by_clicks_computed_in_the_window_and_undone() {
        let mut app = Desktop::sample();
        let lv = ElementId(10);
        app.choose_tool(Tool::ElecNode);
        app.elec_draft.kind = ElecKind::Source;
        app.elec_node_click(lv, [0.0, 0.0]);
        app.elec_node_click(lv, [0.1, 0.0]);
        assert_eq!(app.document.mep().elec_nodes.len(), 1, "a node is there already: {}", app.message);
        app.elec_draft.kind = ElecKind::Source;
        app.elec_node_click(lv, [5.0, 5.0]);
        assert!(app.message.contains("source already"), "{}", app.message);
        app.elec_draft.kind = ElecKind::Board;
        app.elec_node_click(lv, [10.0, 0.0]);
        app.elec_draft.kind = ElecKind::Load;
        (app.elec_draft.kw, app.elec_draft.power_factor, app.elec_draft.name) = (30.0, 0.85, "Chiller".into());
        app.elec_node_click(lv, [20.0, 0.0]);
        app.elec_draft.kw = 7.5;
        app.elec_draft.motor = true;
        app.elec_draft.name = "Pump".into();
        app.elec_node_click(lv, [10.0, 8.0]);
        assert_eq!(app.document.mep().elec_nodes.len(), 4);
        // cables: a load cannot feed, the points on the way are kept, a node is fed once
        app.choose_tool(Tool::Cable);
        app.cable_click(lv, [20.0, 0.0]);
        assert!(app.cable_from.is_none() && app.message.contains("does not feed"), "{}", app.message);
        for p in [[0.0, 0.0], [10.0, 0.0]] { app.cable_click(lv, p); }
        assert_eq!(app.document.mep().elec_cables.len(), 1);
        app.cable_click(lv, [10.0, 0.0]);
        app.cable_click(lv, [20.0, 3.0]);
        app.cable_click(lv, [20.0, 0.0]);
        let long = app.document.mep().elec_cables.iter().find(|c| c.via.len() == 1).expect("with a point on the way");
        assert_eq!(long.via, vec![[20.0, 3.0]]);
        app.cable_click(lv, [10.0, 0.0]);
        app.cable_click(lv, [10.0, 8.0]);
        assert_eq!(app.document.mep().elec_cables.len(), 3, "{}", app.message);
        app.cable_click(lv, [10.0, 0.0]);
        app.cable_click(lv, [20.0, 0.0]);
        assert!(app.message.contains("fed by a cable already"), "{}", app.message);
        app.cable_from = None;
        // compute through the window
        app.elec.table = TABLE.into();
        app.elec.max_start_drop = 15.0;
        app.run_elec_compute();
        assert!(app.elec.error.is_empty(), "{}", app.elec.error);
        let (_, r) = app.elec.report.as_ref().unwrap();
        assert_eq!(r.cables.len(), 3);
        assert!((r.total_kw - 37.5).abs() < 1e-9, "{}", r.total_kw);
        assert!(r.cables.iter().any(|c| c.critical) && r.cables.iter().any(|c| !c.critical));
        app.elec.open = true;
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| app.elec_window(ui.ctx()));
        out.textures_delta.clear();
        // a bad table is explained by line
        app.elec.table = "a,1,2,3,4\n10,x\n".into();
        app.run_elec_compute();
        assert!(app.elec.error.contains("cable table, line 2"), "{}", app.elec.error);
        // remove a cable and a node; undo
        app.elec.table = TABLE.into();
        app.elec_draft.mode = Mode::Remove;
        app.cable_click(lv, [5.0, 0.0]);
        assert_eq!(app.document.mep().elec_cables.len(), 2, "{}", app.message);
        app.run_elec_compute();
        assert!(app.elec.error.contains("not fed"), "{}", app.elec.error);
        app.choose_tool(Tool::ElecNode);
        app.elec_draft.mode = Mode::Remove;
        app.elec_node_click(lv, [10.0, 8.0]);
        assert_eq!((app.document.mep().elec_nodes.len(), app.document.mep().elec_cables.len()), (3, 1));
        app.undo_document();
        assert_eq!((app.document.mep().elec_nodes.len(), app.document.mep().elec_cables.len()), (4, 2));
    }

    #[test]
    fn a_node_and_a_cable_are_changed_after_placing_and_undone() {
        let mut app = Desktop::sample();
        let lv = ElementId(10);
        app.choose_tool(Tool::ElecNode);
        app.elec_draft.kind = ElecKind::Source;
        app.elec_node_click(lv, [0.0, 0.0]);
        app.elec_draft.kind = ElecKind::Load;
        (app.elec_draft.kw, app.elec_draft.name) = (10.0, "Fan".into());
        app.elec_node_click(lv, [10.0, 0.0]);
        app.choose_tool(Tool::Cable);
        app.cable_click(lv, [0.0, 0.0]);
        app.cable_click(lv, [10.0, 0.0]);
        // edit the load
        app.choose_tool(Tool::ElecNode);
        app.elec_draft.mode = Mode::Edit;
        app.elec_node_click(lv, [10.1, 0.0]);
        assert!(app.elec_draft.selected.is_some() && app.elec_draft.kw == 10.0, "{}", app.message);
        (app.elec_draft.kw, app.elec_draft.name, app.elec_draft.motor) = (22.0, "Fan 2".into(), true);
        app.elec_apply();
        let n = app.document.mep().elec_nodes.iter().find(|n| n.kind == ElecKind::Load).unwrap();
        assert_eq!((n.watts, n.name.as_str(), n.motor.is_some()), (22000.0, "Fan 2", true));
        // edit the cable
        app.choose_tool(Tool::Cable);
        app.elec_draft.mode = Mode::Edit;
        app.cable_click(lv, [5.0, 0.1]);
        assert!(app.elec_draft.selected.is_some(), "{}", app.message);
        (app.elec_draft.three_phase, app.elec_draft.grouped, app.elec_draft.rcd_ma) = (false, 3, Some(30.0));
        app.elec_apply();
        let c = &app.document.mep().elec_cables[0];
        assert_eq!((c.three_phase, c.grouped_circuits, c.rcd_ma), (false, 3, Some(30.0)));
        // a bad value is refused and nothing changes
        app.choose_tool(Tool::ElecNode);
        app.elec_draft.mode = Mode::Edit;
        app.elec_node_click(lv, [10.0, 0.0]);
        app.elec_draft.power_factor = 2.0;
        app.elec_apply();
        assert!(app.message.contains("Rejected"), "{}", app.message);
        assert_eq!(app.document.mep().elec_nodes.iter().find(|n| n.kind == ElecKind::Load).unwrap().power_factor, 0.9);
        app.undo_document();
        app.undo_document();
        assert_eq!(app.document.mep().elec_nodes.iter().find(|n| n.kind == ElecKind::Load).unwrap().name, "Fan");
        assert!(app.document.mep().elec_cables[0].three_phase);
    }

    #[test]
    fn a_load_takes_its_place_and_power_from_the_equipment_and_follows_it_and_a_duct_end_takes_the_airflow() {
        use openbim_document::FamilySpec;
        let mut app = Desktop::sample();
        let lv = ElementId(10);
        let fcu = app.document.place_family(lv, [5.0, 5.0], &FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: vec![], angle_deg: 90.0, elevation_m: 2.2 }).unwrap();
        let conns = instance_connectors(&app.document, fcu);
        let power = conns.iter().find(|c| c.domain == Domain::Electrical).expect("electrical connector").clone();
        let air = conns.iter().find(|c| c.domain == Domain::Duct && c.flow > 0.0 && c.system == openbim_family::mep::System::SupplyAir).expect("an air connector").clone();
        // a click near the unit makes a load at its connector with the connector's power
        app.choose_tool(Tool::ElecNode);
        app.elec_draft.kind = ElecKind::Load;
        app.elec_draft.from_equipment = true;
        app.elec_node_click(lv, [5.5, 5.5]);
        let n = app.document.mep().elec_nodes.first().expect("a load").clone();
        assert_eq!((n.element, n.connector.as_str()), (Some(fcu), power.name.as_str()), "{}", app.message);
        assert_eq!((n.at, n.watts, n.power_factor), ([power.pos[0], power.pos[1]], power.flow, power.power_factor));
        // the tie is saved and comes back from the file's records
        let lines = openbim_document::native::records(&app.document);
        assert_eq!(openbim_document::native::parse_mep(&lines).unwrap(), *app.document.mep());
        let out = crate::export::export_ifc_as(&app.document, "P", 1, "2026-10-18T00:00:00", crate::export::Schema::Ifc4x3).unwrap();
        let mut back = Document::new();
        crate::import::import_ifc(&mut back, &out.text).unwrap();
        assert_eq!(back.mep().elec_nodes, app.document.mep().elec_nodes);
        app.elec_node_click(lv, [5.5, 5.5]);
        assert!(app.message.contains("already"), "{}", app.message);
        // far from any equipment: refused
        app.elec_node_click(lv, [30.0, 30.0]);
        assert!(app.message.contains("No equipment"), "{}", app.message);
        // the unit moves: the load follows when refreshed (and on compute); one undoable step
        app.document.transact(app.document.revision(), &[Command::SetGeometry { id: fcu, geometry: Geometry::Placed { at: [8.0, 5.0], level: lv } }]).unwrap();
        assert_eq!(app.refresh_equipment_loads(), 1);
        let moved = app.document.mep().elec_nodes[0].at;
        assert!((moved[0] - power.pos[0] - 3.0).abs() < 1e-9 && (moved[1] - power.pos[1]).abs() < 1e-9, "{moved:?}");
        assert_eq!(app.refresh_equipment_loads(), 0);
        // deleting the unit lets go of the load but keeps it
        let before = app.document.revision();
        app.document.transact(before, &[Command::Delete { id: fcu }]).unwrap();
        let n = &app.document.mep().elec_nodes[0];
        assert!(n.element.is_none() && n.connector.is_empty() && n.watts == power.flow);
        assert!(app.document.undo().unwrap());
        assert_eq!(app.document.mep().elec_nodes[0].element, Some(fcu));
        // the unit is back where it was moved to; a duct that ends at its air connector gives the end point the airflow
        let air = instance_connectors(&app.document, fcu).into_iter().find(|c| c.name == air.name).unwrap();
        app.choose_tool(Tool::Duct);
        app.elevation_mm = 2200.0;
        app.run_click([air.pos[0] + 3.0 * air.dir[0], air.pos[1] + 3.0 * air.dir[1]]);
        app.run_click([air.pos[0] + 0.1 * air.dir[0], air.pos[1] + 0.1 * air.dir[1]]);
        app.finish_run(lv);
        // the run was joined to the connector when it was drawn, and put exactly on it
        let run_end = |app: &Desktop| { let id = app.document.mep().equip_links[0].port; let r = app.document.mep_run(match id.node { openbim_document::MepNode::Run(r) => r, _ => unreachable!() }).unwrap(); if id.port == 0 { r.a } else { r.b } };
        let end = run_end(&app);
        assert!(end.iter().zip(&air.pos).all(|(a, b)| (a - b).abs() < 1e-9), "{end:?} against {:?}", air.pos);
        assert_eq!(app.document.mep().equip_links.len(), 1, "{}", app.message);
        assert_eq!(app.document.mep().equip_links[0].element, fcu);
        app.choose_tool(Tool::EndPoint);
        app.end_draft.mark = crate::compute_ui::Mark::Terminal;
        app.end_draft.kind = 4;
        app.end_point_click(lv, [air.pos[0], air.pos[1]]);
        let e = app.document.mep().endpoints.first().unwrap_or_else(|| panic!("{}", app.message));
        assert_eq!(e.demand, openbim_document::EndDemand::Flow(air.flow));
        assert_eq!(e.required_pressure_pa, air.pressure_drop);
        assert_eq!(app.document.mep().equip_links.len(), 1);
        // the tie is saved, comes back from the file, and a change of the unit's airflow reaches the end point on compute
        let lines = openbim_document::native::records(&app.document);
        assert_eq!(openbim_document::native::parse_mep(&lines).unwrap(), *app.document.mep());
        let out = crate::export::export_ifc_as(&app.document, "P", 1, "2026-10-18T00:00:00", crate::export::Schema::Ifc4).unwrap();
        assert_eq!(out.text.matches("IFCRELCONNECTSPORTS(").count(), 3, "two at the reducer, one to the unit's own port");
        assert!(!out.text.contains("OpenBIM_Equipment"), "no placeholder: the unit's own port is used");
        assert!(!out.warnings.iter().any(|w| w.contains("do not fit")), "{:?}", out.warnings);
        let mut back = Document::new();
        crate::import::import_ifc(&mut back, &out.text).unwrap();
        assert_eq!(back.mep().equip_links, app.document.mep().equip_links);
        // the unit moves: the run follows it, exactly, and the join stays
        app.document.transact(app.document.revision(), &[Command::SetGeometry { id: fcu, geometry: Geometry::Placed { at: [9.0, 6.0], level: lv } }]).unwrap();
        assert_eq!(app.refresh_equipment_ends(), 1, "{}", app.message);
        let air2 = instance_connectors(&app.document, fcu).into_iter().find(|c| c.name == air.name).unwrap();
        let end = run_end(&app);
        assert!(end.iter().zip(&air2.pos).all(|(a, b)| (a - b).abs() < 1e-9), "{end:?} against {:?}", air2.pos);
        assert_eq!(app.document.mep().equip_links.len(), 1);
        assert_eq!(app.refresh_equipment_ends(), 0);
        app.undo_document();
        let end = run_end(&app);
        assert!(end.iter().zip(&air.pos).all(|(a, b)| (a - b).abs() < 1e-9), "undo puts the run back: {end:?}");
        app.document.transact(app.document.revision(), &[Command::SetGeometry { id: fcu, geometry: Geometry::Placed { at: [8.0, 5.0], level: lv } }]).unwrap();
        // the end point differs from what the unit gives now (as after a change of type): compute brings it back
        let mut stale = app.document.mep().clone();
        stale.endpoints[0].demand = openbim_document::EndDemand::Flow(0.25);
        app.document.transact(app.document.revision(), &[Command::SetMep(Box::new(stale))]).unwrap();
        assert_eq!(app.refresh_equipment_ends(), 1, "{}", app.message);
        assert_eq!(app.document.mep().endpoints[0].demand, openbim_document::EndDemand::Flow(air.flow));
        assert_eq!(app.refresh_equipment_ends(), 0);
        // deleting the unit drops the tie but keeps the end point and the run
        app.document.transact(app.document.revision(), &[Command::Delete { id: fcu }]).unwrap();
        assert!(app.document.mep().equip_links.is_empty() && app.document.mep().endpoints.len() == 1);
    }

    #[test]
    fn a_run_drawn_from_a_connector_leaves_it_straight_with_a_reducer_and_elbows_and_is_joined() {
        use openbim_document::FamilySpec;
        let mut app = Desktop::sample();
        let lv = ElementId(10);
        let fcu = app.document.place_family(lv, [5.0, 5.0], &FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 2.2 }).unwrap();
        let air = instance_connectors(&app.document, fcu).into_iter().find(|c| c.domain == Domain::Duct && c.flow > 0.0 && c.system == openbim_family::mep::System::SupplyAir).unwrap();
        let (nx, ny) = (air.dir[0], air.dir[1]);
        // the tool's size differs from the connector's: a reducer; the second point is to one side: an elbow
        app.choose_tool(Tool::Duct);
        (app.width_mm, app.height_mm) = (600.0, 300.0);
        app.run_click([air.pos[0] + 0.2 * -ny, air.pos[1] + 0.2 * nx]); // near the connector, not on it
        app.run_click([air.pos[0] + 6.0 * nx, air.pos[1] + 6.0 * ny]);
        app.run_click([air.pos[0] + 6.0 * nx - 4.0 * ny, air.pos[1] + 6.0 * ny + 4.0 * nx]);
        app.finish_run(lv);
        let link = app.document.mep().equip_links.first().unwrap_or_else(|| panic!("joined: {}", app.message)).clone();
        assert_eq!(link.element, fcu);
        let openbim_document::MepNode::Run(first) = link.port.node else { panic!() };
        let r = app.document.mep_run(first).unwrap();
        let (end, other) = if link.port.port == 0 { (r.a, r.b) } else { (r.b, r.a) };
        assert!(end.iter().zip(&air.pos).all(|(a, b)| (a - b).abs() < 1e-9), "starts exactly on the connector: {end:?} against {:?}", air.pos);
        let d = [other[0] - end[0], other[1] - end[1], other[2] - end[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!((d[0] * air.dir[0] + d[1] * air.dir[1] + d[2] * air.dir[2]) / len > 1.0 - 1e-9, "the first run is along the connector's axis");
        // the stub is at the connector's size
        let el = app.document.get(first).unwrap();
        let openbim_family::mep::Size::Rect { width, height } = air.size else { panic!() };
        assert_eq!((el.width_m, el.height_m), (width, height));
        let kinds: Vec<_> = app.document.mep().fittings.iter().map(|f| std::mem::discriminant(&f.fitting.kind)).collect();
        assert!(kinds.len() >= 2, "a reducer and an elbow: {kinds:?}");
        assert!(app.document.mep().fittings.iter().any(|f| matches!(f.fitting.kind, openbim_route::FittingKind::Transition)));
        assert!(app.document.mep().fittings.iter().any(|f| matches!(f.fitting.kind, openbim_route::FittingKind::Elbow { .. })));
        // one undo step removes the join, one the run
        app.undo_document();
        assert!(app.document.mep().equip_links.is_empty());
        app.undo_document();
        assert!(app.document.mep().fittings.is_empty());
        // ending on a connector works the same, drawn the other way round
        app.choose_tool(Tool::Duct);
        app.run_click([air.pos[0] + 6.0 * nx - 4.0 * ny, air.pos[1] + 6.0 * ny + 4.0 * nx]);
        app.run_click([air.pos[0] + 6.0 * nx, air.pos[1] + 6.0 * ny]);
        app.run_click([air.pos[0] + 0.2 * -ny, air.pos[1] + 0.2 * nx]);
        app.finish_run(lv);
        assert_eq!(app.document.mep().equip_links.len(), 1, "{}", app.message);
        let q = &app.document.mep().equip_links[0];
        let openbim_document::MepNode::Run(last) = q.port.node else { panic!() };
        let r = app.document.mep_run(last).unwrap();
        let end = if q.port.port == 0 { r.a } else { r.b };
        assert!(end.iter().zip(&air.pos).all(|(a, b)| (a - b).abs() < 1e-9), "{end:?}");
    }
}
