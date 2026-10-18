//! Drawing ducts and pipes the way Revit does: click the points of the run in a plan view and the fittings appear by
//! themselves, an elbow at every bend and a reducer where the size changes between two clicks. The run is placed
//! when it is finished (Enter, the Finish button, or Esc once it has a length).
use crate::{Desktop, Tool};
use eframe::egui;
use openbim_document::ElementId;
use openbim_family::mep::{Domain, System};
use openbim_place::penetrations::RunSize;
use openbim_route::snaps::{SnapSettings, Snaps};
use openbim_route::{route_path, AngleSet, Junction, RoutingPrefs};

/// What the drawing tool assumes: elbows at any angle, rounded as Revit's defaults (a duct bends on one width, a pipe
/// on one and a half diameters), a reducer at no more than 30 degrees, and a tee for a branch.
pub(crate) fn route_prefs(domain: Domain, system: System) -> RoutingPrefs {
    RoutingPrefs {
        domain, system, label: if domain == Domain::Duct { format!("{} Duct", system.name()) } else { format!("{} Pipe", system.name()) },
        elbow_angles: AngleSet::Any, bend_radius_factor: if domain == Domain::Duct { 1.0 } else { 1.5 }, min_straight: 0.0,
        junction: Junction::Tee, tee_angles: AngleSet::Any, tee_half_factor: 1.0, branch_stub_factor: 1.0, pick_tolerance: 0.02,
        transition_angle_deg: 30.0, min_transition_len: 0.1, connector_stub: 0.5, margin: 0.0, catalogue: None,
    }
}

impl Desktop {
    fn run_domain(&self) -> Domain { if self.tool == Tool::Pipe { Domain::Pipe } else { Domain::Duct } }

    /// The systems the tool can draw: those that belong to a duct or to a pipe.
    pub(crate) fn run_systems(&self) -> Vec<System> {
        let d = self.run_domain();
        System::ALL.iter().copied().filter(|s| *s != System::Other && s.domain_ok(d)).collect()
    }

    /// The size the options bar says: a duct is width by height, a pipe has a diameter (the width field).
    fn run_size(&self) -> RunSize {
        if self.tool == Tool::Pipe { RunSize::Round { diameter: self.width_mm / 1000.0 } }
        else { RunSize::Rect { width: self.width_mm / 1000.0, height: self.height_mm / 1000.0, roll: 0.0 } }
    }

    /// Picks the first system that fits the tool when the one picked does not (after switching from duct to pipe).
    pub(crate) fn fit_run_system(&mut self) {
        let d = self.run_domain();
        if !self.system_pick.domain_ok(d) || self.system_pick == System::Other { if let Some(s) = self.run_systems().first() { self.system_pick = *s; } }
    }

    /// A click of the Duct or Pipe tool: the next point of the run, with the size now in the options bar for the leg
    /// that ends there. A different size from the leg before makes a reducer.
    pub(crate) fn run_click(&mut self, point: [f64; 2]) {
        if self.run_points.last() == Some(&point) { return; }
        if self.run_points.is_empty() {
            self.branch_main = self.run_to_branch_from(point);
            if let Some(id) = self.branch_main {
                self.message = format!("Branch from {}: click the points of the branch; Enter places it with a tee", self.document.get(id).map_or("the run".into(), |e| e.name.clone()));
            }
        }
        self.run_points.push(point);
        if self.run_points.len() >= 2 { self.run_sizes.push(self.run_size()); }
        self.message = format!("{}: {} point{}; Enter or Finish places it, Backspace takes the last point back, Esc ends", if self.tool == Tool::Pipe { "Pipe" } else { "Duct" }, self.run_points.len(), if self.run_points.len() == 1 { "" } else { "s" });
    }

    /// The drawn run of the tool's kind that a first click lies on (inside it, at the height being drawn): the run a
    /// branch starts from.
    fn run_to_branch_from(&self, p: [f64; 2]) -> Option<ElementId> {
        let want = if self.tool == Tool::Pipe { openbim_document::Category::Pipe } else { openbim_document::Category::Duct };
        let z = |id: ElementId| -> Option<f64> { match self.document.get(id)?.geometry? { openbim_document::Geometry::Linear { level, .. } => Some(self.document.get(level)?.elevation_m + self.elevation_mm / 1000.0), _ => None } };
        self.document.elements().filter(|e| e.category == want).find_map(|e| {
            let r = self.document.mep_run(e.id)?;
            let (dx, dy) = (r.b[0] - r.a[0], r.b[1] - r.a[1]);
            let len2 = dx * dx + dy * dy;
            if len2 < 1e-9 { return None; }
            let s = ((p[0] - r.a[0]) * dx + (p[1] - r.a[1]) * dy) / len2;
            let (fx, fy) = (r.a[0] + s * dx, r.a[1] + s * dy);
            let along = s * len2.sqrt();
            let zf = r.a[2] + (r.b[2] - r.a[2]) * s;
            (s > 0.0 && s < 1.0 && (fx - p[0]).hypot(fy - p[1]) <= 0.05 && along > 0.1 && len2.sqrt() - along > 0.1 && (zf - z(e.id)?).abs() <= 0.02).then_some(e.id)
        })
    }

    pub(crate) fn run_back(&mut self) {
        self.run_points.pop();
        self.run_sizes.truncate(self.run_points.len().saturating_sub(1));
    }

    /// Places the run drawn so far on `level`, with its elbows and reducers, as one undoable step.
    pub(crate) fn finish_run(&mut self, level: ElementId) {
        if self.run_points.len() < 2 { return; }
        let Some(z0) = self.document.get(level).map(|l| l.elevation_m) else { return };
        let z = z0 + self.elevation_mm / 1000.0;
        let pts: Vec<[f64; 3]> = self.run_points.iter().map(|p| [p[0], p[1], z]).collect();
        if let Some(main) = self.branch_main {
            let Some(mrun) = self.document.mep_run(main) else { self.branch_main = None; return };
            let sys = if mrun.system == System::Other { self.system_pick } else { mrun.system };
            let prefs = route_prefs(self.run_domain(), sys);
            let br = match openbim_route::branch_from_run(&mrun, &pts, &self.run_sizes, &prefs, &Snaps::new(SnapSettings::off())) {
                Ok(b) => b,
                Err(e) => { self.message = format!("This branch cannot be drawn ({e:?}); take a point back (Backspace), or change a size"); return; }
            };
            match self.document.place_mep_branch(level, main, &br) {
                Ok(ids) => {
                    self.run_points.clear();
                    self.run_sizes.clear();
                    self.branch_main = None;
                    self.message = format!("Branch placed with a tee ({} new {}); undo removes it", ids.len(), if self.tool == Tool::Pipe { "pipes" } else { "ducts" });
                }
                Err(e) => self.message = format!("Branch rejected ({e:?})"),
            }
            return;
        }
        let prefs = route_prefs(self.run_domain(), self.system_pick);
        let route = match route_path(&pts, &self.run_sizes, &prefs, &Snaps::new(SnapSettings::off())) {
            Ok(r) => r,
            Err(e) => { self.message = format!("This run cannot be drawn ({e:?}); take a point back (Backspace) or change a size"); return; }
        };
        match self.document.place_mep_route(level, &route) {
            Ok(ids) => {
                self.link_run_ends_to_equipment(level, &route, &ids);
                let (runs, fits) = (ids.len(), route.fittings.len());
                self.run_points.clear();
                self.run_sizes.clear();
                self.message = format!("{runs} {} and {fits} fitting{} placed; undo removes them", if self.tool == Tool::Pipe { "pipes" } else { "ducts" }, if fits == 1 { "" } else { "s" });
            }
            Err(e) => self.message = format!("Run rejected ({e:?})"),
        }
    }

    /// A new run that ends on a connector of a family instance (same kind, a system that fits, within 15 cm in plan and
    /// arriving square to it, within 10 degrees) is joined to it, as Revit does, and put exactly on it. One more undoable step.
    fn link_run_ends_to_equipment(&mut self, level: ElementId, route: &openbim_route::Route, ids: &[ElementId]) {
        let (Some(first), Some(last)) = (route.runs.first(), route.runs.last()) else { return };
        let domain = self.run_domain();
        let mut data = self.document.mep().clone();
        let (mut cmds, mut made) = (Vec::new(), 0);
        for (id, port, p, away, sys) in [(ids[0], 0usize, first.a, [first.a[0] - first.b[0], first.a[1] - first.b[1], first.a[2] - first.b[2]], first.system), (ids[ids.len() - 1], 1usize, last.b, [last.b[0] - last.a[0], last.b[1] - last.a[1], last.b[2] - last.a[2]], last.system)] {
            let mp = openbim_document::MepPort { node: openbim_document::MepNode::Run(id), port };
            if data.partner(mp).is_some() || data.equip_links.iter().any(|q| q.port == mp) { continue; }
            let Some((el, c)) = crate::elec_ui::equipment_connector_near(&self.document, level, [p[0], p[1]], 0.15, domain).filter(|(_, c)| c.system.compatible(sys)) else { continue };
            // the run must arrive along the connector's axis: its outward direction opposes the way the connector faces
            let (n, k) = ((away[0] * away[0] + away[1] * away[1] + away[2] * away[2]).sqrt(), (c.dir[0] * c.dir[0] + c.dir[1] * c.dir[1] + c.dir[2] * c.dir[2]).sqrt());
            if n < 1e-9 || k < 1e-9 || -(away[0] * c.dir[0] + away[1] * c.dir[1] + away[2] * c.dir[2]) / (n * k) < 10f64.to_radians().cos() { continue; }
            let q = openbim_document::EquipLink { port: mp, element: el, connector: c.name.clone() };
            cmds.extend(crate::compute_ui::snap_commands(&self.document, &q, &c));
            data.equip_links.push(q);
            made += 1;
        }
        if made > 0 {
            cmds.push(openbim_document::Command::SetMep(Box::new(data)));
            if self.document.transact(self.document.revision(), &cmds).is_ok() { self.message = format!("{made} end(s) joined to equipment connectors"); }
        }
    }

    /// The options bar of the tool.
    pub(crate) fn run_options(&mut self, ui: &mut egui::Ui) {
        self.fit_run_system();
        let pipe = self.tool == Tool::Pipe;
        ui.strong(if pipe { "Place Pipe" } else { "Place Duct" });
        ui.separator();
        ui.label("System");
        let systems = self.run_systems();
        egui::ComboBox::from_id_salt("run_system").selected_text(self.system_pick.name()).show_ui(ui, |ui| {
            for s in systems { ui.selectable_value(&mut self.system_pick, s, s.name()); }
        });
        ui.label(if pipe { "Diameter (mm)" } else { "Width (mm)" });
        ui.add(egui::DragValue::new(&mut self.width_mm).speed(1.0).range(10.0..=3000.0));
        if !pipe {
            ui.label("Height (mm)");
            ui.add(egui::DragValue::new(&mut self.height_mm).speed(1.0).range(10.0..=3000.0));
        }
        ui.label("Middle elevation (mm)");
        ui.add(egui::DragValue::new(&mut self.elevation_mm).speed(1.0));
        ui.label(format!("{} point{}", self.run_points.len(), if self.run_points.len() == 1 { "" } else { "s" }));
        if ui.add_enabled(self.run_points.len() >= 2, egui::Button::new("Finish")).clicked() { self.finish_run(self.plan_level()); }
        if ui.add_enabled(!self.run_points.is_empty(), egui::Button::new("Clear")).clicked() { self.run_points.clear(); self.run_sizes.clear(); }
        ui.weak("Change the size between two clicks to get a reducer; elbows appear at every bend.");
    }
}
