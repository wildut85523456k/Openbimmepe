//! Drawing ducts and pipes the way Revit does: click the points of the run in a plan view and the fittings appear by
//! themselves, an elbow at every bend and a reducer where the size changes between two clicks. The run is placed
//! when it is finished (Enter, the Finish button, or Esc once it has a length).
use crate::{Desktop, Tool};
use eframe::egui;
use openbim_document::ElementId;
use openbim_family::mep::{Domain, System};
use openbim_place::penetrations::RunSize;
use openbim_route::snaps::{SnapSettings, Snaps};
use openbim_route::{route_path, RoutingPrefs};

/// What the drawing tool assumes: elbows at any angle, rounded as Revit's defaults (a duct bends on one width, a pipe
/// on one and a half diameters), a reducer at no more than 30 degrees, and a tee for a branch.
pub(crate) fn route_prefs(domain: Domain, system: System) -> RoutingPrefs { openbim_document::default_route_prefs(domain, system) }

/// Where the drawn line sits across the duct or pipe (Revit's Horizontal Justification), looking along the way it is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HJust { Left, Center, Right }
/// Which edge of the duct or pipe the elevation in the options bar refers to (Revit's Vertical Justification).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum VJust { Top, Middle, Bottom }
/// Revit's pipe Slope: Off, or Up / Down along the way the pipe is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SlopeMode { Off, Up, Down }

/// The Place Duct / Place Pipe options that are not sizes: the contextual "Placement Tools" of Revit.
#[derive(Clone, Debug)]
pub(crate) struct RunOpts {
    /// Join the run to what it starts on (an equipment connector, a run to branch from) and to the connector it ends on.
    pub auto_connect: bool,
    /// Take the height of the connector or run the first click is on.
    pub inherit_elevation: bool,
    /// Take the size of the connector or run the first click is on.
    pub inherit_size: bool,
    pub hjust: HJust,
    pub vjust: VJust,
    /// Put a tag on every run as it is placed.
    pub tag: bool,
    pub slope: SlopeMode,
    pub slope_pct: f64,
    pub show_slope_tip: bool,
}

impl Default for RunOpts {
    fn default() -> Self {
        RunOpts { auto_connect: true, inherit_elevation: true, inherit_size: false, hjust: HJust::Center, vjust: VJust::Middle, tag: false, slope: SlopeMode::Off, slope_pct: 1.0, show_slope_tip: true }
    }
}

/// The list of sizes next to a size field: picking one puts it in the field.
fn size_list(ui: &mut egui::Ui, id: &str, sizes: &[f64], value: &mut f64) {
    egui::ComboBox::from_id_salt(id).selected_text("▾").width(14.0).show_ui(ui, |ui| {
        if sizes.is_empty() { ui.weak("No sizes in Mechanical Settings"); }
        for s in sizes { if ui.selectable_label((*value - s).abs() < 0.5, format!("{s} mm")).clicked() { *value = *s; } }
    }).response.on_hover_text("Sizes from Manage > MEP Settings");
}

fn run_size_extent(s: &RunSize) -> (f64, f64) {
    match s { RunSize::Round { diameter } => (*diameter, *diameter), RunSize::Rect { width, height, .. } => (*width, *height), RunSize::Oval { width, height, .. } => (*width, *height) }
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
        if self.run_points.is_empty() { self.run_first_click(point); }
        self.run_points.push(point);
        if self.run_points.len() >= 2 { self.run_sizes.push(self.run_size()); }
        if self.branch_main.is_none() || self.run_points.len() > 1 {
            self.message = format!("{}: {} point{}; Enter or Finish places it, Backspace takes the last point back, Esc ends", if self.tool == Tool::Pipe { "Pipe" } else { "Duct" }, self.run_points.len(), if self.run_points.len() == 1 { "" } else { "s" });
        }
    }

    /// The first click of a run: with Automatically Connect it may start a branch from a run or leave an equipment
    /// connector, and Inherit Size / Inherit Elevation then take the size and the height of what it starts on.
    fn run_first_click(&mut self, point: [f64; 2]) {
        self.branch_main = None;
        if !self.run_opts.auto_connect { return; }
        let level = self.plan_level();
        let level_z = self.document.get(level).map_or(0.0, |l| l.elevation_m);
        if let Some((id, zf)) = self.run_to_branch_from(point) {
            self.branch_main = Some(id);
            self.message = format!("Branch from {}: click the points of the branch; Enter places it with a tee", self.document.get(id).map_or("the run".into(), |e| e.name.clone()));
            if self.run_opts.inherit_elevation { self.elevation_mm = ((zf - level_z) * 1000.0).round(); }
            if self.run_opts.inherit_size {
                if let Some(run) = self.document.mep_run(id) { self.take_size(&run.size); }
            }
        } else if let Some((_, c)) = crate::elec_ui::equipment_connector_near(&self.document, level, point, 0.5, self.run_domain(), None) {
            if self.run_opts.inherit_elevation { self.elevation_mm = ((c.pos[2] - level_z) * 1000.0).round(); }
            if self.run_opts.inherit_size {
                if let Ok(size) = openbim_route::conn_run_size(&c) { self.take_size(&size); }
            }
        }
    }

    /// Puts a size into the options bar (Inherit Size).
    fn take_size(&mut self, size: &RunSize) {
        let (w, h) = run_size_extent(size);
        self.width_mm = (w * 1000.0).round();
        if self.tool != Tool::Pipe { self.height_mm = (h * 1000.0).round(); }
    }

    /// The drawn run of the tool's kind that a first click lies on (inside it): the run a branch starts from, and its
    /// height there. With Inherit Elevation any height does and the run nearest the elevation in the bar wins; without it
    /// the run must be at the height being drawn.
    fn run_to_branch_from(&self, p: [f64; 2]) -> Option<(ElementId, f64)> {
        let want = if self.tool == Tool::Pipe { openbim_document::Category::Pipe } else { openbim_document::Category::Duct };
        let z_bar = |id: ElementId| -> Option<f64> { match self.document.get(id)?.geometry? { openbim_document::Geometry::Linear { level, .. } => Some(self.document.get(level)?.elevation_m + self.elevation_mm / 1000.0), _ => None } };
        let tol = if self.run_opts.inherit_elevation { f64::INFINITY } else { 0.02 };
        self.document.elements().filter(|e| e.category == want).filter_map(|e| {
            let r = self.document.mep_run(e.id)?;
            let (dx, dy) = (r.b[0] - r.a[0], r.b[1] - r.a[1]);
            let len2 = dx * dx + dy * dy;
            if len2 < 1e-9 { return None; }
            let s = ((p[0] - r.a[0]) * dx + (p[1] - r.a[1]) * dy) / len2;
            let (fx, fy) = (r.a[0] + s * dx, r.a[1] + s * dy);
            let along = s * len2.sqrt();
            let zf = r.a[2] + (r.b[2] - r.a[2]) * s;
            let off = (zf - z_bar(e.id)?).abs();
            (s > 0.0 && s < 1.0 && (fx - p[0]).hypot(fy - p[1]) <= 0.05 && along > 0.1 && len2.sqrt() - along > 0.1 && off <= tol).then_some((e.id, zf, off))
        }).min_by(|a, b| a.2.total_cmp(&b.2)).map(|(id, zf, _)| (id, zf))
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
                    let tagged = self.tag_new_runs(&ids);
                    self.message = format!("Branch placed with a tee ({} new {}{}); undo removes it", ids.len(), if self.tool == Tool::Pipe { "pipes" } else { "ducts" }, tagged);
                }
                Err(e) => self.message = format!("Branch rejected ({e:?})"),
            }
            return;
        }
        // a run that starts (or ends) on an equipment connector leaves it straight, as Revit does: a stub along the
        // connector's axis at its size, a reducer to the size drawn, elbows at the bends, all at the connector's height
        let n = self.run_points.len();
        let from_conn = if self.run_opts.auto_connect {
            [(0usize, false), (n - 1, true)].into_iter().find_map(|(i, rev)| {
                crate::elec_ui::equipment_connector_near(&self.document, level, self.run_points[i], 0.5, self.run_domain(), None).map(|(_, c)| (c, rev))
            })
        } else { None };
        if let Some((c, rev)) = from_conn {
            // Inherit Elevation keeps the connector's height all the way; without it the run leaves at the connector's
            // height and goes to the elevation in the options bar after the stub
            let zz = if self.run_opts.inherit_elevation { c.pos[2] } else { z };
            let mut after: Vec<[f64; 3]> = if rev { self.run_points[..n - 1].iter().rev().map(|p| [p[0], p[1], zz]).collect() } else { self.run_points[1..].iter().map(|p| [p[0], p[1], zz]).collect() };
            after.dedup();
            let prefs = route_prefs(self.run_domain(), c.system);
            let route = match openbim_route::route_from_connector(&c, &after, self.run_size(), &prefs, &Snaps::new(SnapSettings::off())) {
                Ok(r) => if rev { r.reversed() } else { r },
                Err(e) => { self.message = format!("This run cannot leave the connector '{}' ({e:?}); move the points, or turn Inherit Elevation on", c.name); return; }
            };
            match self.document.place_mep_route(level, &route) {
                Ok(ids) => {
                    self.link_run_ends_to_equipment(level, &route, &ids);
                    self.run_points.clear();
                    self.run_sizes.clear();
                    let tagged = self.tag_new_runs(&ids);
                    self.message = format!("{} run(s) and {} fitting(s) placed; the run leaves '{}' straight and is joined to it{}; undo removes it", ids.len(), route.fittings.len(), c.name, tagged);
                }
                Err(e) => self.message = format!("Run rejected ({e:?})"),
            }
            return;
        }
        // a plain run: the justification moves it sideways and up or down from the line and the elevation drawn, and a
        // pipe with a slope falls or rises along the way it is drawn
        let (w, h) = self.run_sizes.iter().map(run_size_extent).fold((0.0f64, 0.0f64), |a, b| (a.0.max(b.0), a.1.max(b.1)));
        let side = match self.run_opts.hjust { HJust::Left => -w / 2.0, HJust::Center => 0.0, HJust::Right => w / 2.0 };
        let dz = match self.run_opts.vjust { VJust::Top => -h / 2.0, VJust::Middle => 0.0, VJust::Bottom => h / 2.0 };
        let plan = if side == 0.0 { self.run_points.clone() } else { openbim_route::shift_path(&self.run_points, &vec![side; n - 1]) };
        let prefs = route_prefs(self.run_domain(), self.system_pick);
        let snaps = Snaps::new(SnapSettings::off());
        let result = if self.tool == Tool::Pipe && self.run_opts.slope != SlopeMode::Off {
            // positive falls in the direction drawn: Slope Down; Slope Up rises
            let g = self.run_opts.slope_pct / 100.0 * if self.run_opts.slope == SlopeMode::Down { 1.0 } else { -1.0 };
            openbim_route::route_graded(&plan, z + dz, &vec![g; n - 1], &self.run_sizes, &prefs, &snaps)
        } else {
            let pts: Vec<[f64; 3]> = plan.iter().map(|p| [p[0], p[1], z + dz]).collect();
            route_path(&pts, &self.run_sizes, &prefs, &snaps)
        };
        let route = match result {
            Ok(r) => r,
            Err(e) => { self.message = format!("This run cannot be drawn ({e:?}); take a point back (Backspace) or change a size"); return; }
        };
        match self.document.place_mep_route(level, &route) {
            Ok(ids) => {
                if self.run_opts.auto_connect { self.link_run_ends_to_equipment(level, &route, &ids); }
                let (runs, fits) = (ids.len(), route.fittings.len());
                self.run_points.clear();
                self.run_sizes.clear();
                let tagged = self.tag_new_runs(&ids);
                self.message = format!("{runs} {} and {fits} fitting{} placed{tagged}; undo removes them", if self.tool == Tool::Pipe { "pipes" } else { "ducts" }, if fits == 1 { "" } else { "s" });
            }
            Err(e) => self.message = format!("Run rejected ({e:?})"),
        }
    }

    /// Properties, Start Offset / End Offset of a selected duct or pipe: the middle of that end at `offset_mm` above the
    /// run's level. The slope follows; an end joined to a fitting, equipment or another run is refused.
    pub(crate) fn set_run_end_offset(&mut self, id: ElementId, at_start: bool, offset_mm: f64) {
        let Some(openbim_document::Geometry::Linear { level, .. }) = self.document.get(id).and_then(|e| e.geometry) else { return };
        let z = self.document.get(level).map_or(0.0, |l| l.elevation_m) + offset_mm / 1000.0;
        match self.document.run_end_offset_commands(id, at_start, z) {
            Ok(cmds) => match self.document.transact(self.document.revision(), &cmds) {
                Ok(_) => { self.sync_duct_controls(); self.message = format!("{} of the run moved to {offset_mm:.0} mm above its level; undo puts it back", if at_start { "Start" } else { "End" }); }
                Err(e) => self.message = format!("Offset rejected ({e:?})"),
            },
            Err(e) => self.message = e,
        }
    }

    /// Tag on Placement: a tag at the middle of every new run. Returns the words to add to the message.
    fn tag_new_runs(&mut self, ids: &[ElementId]) -> String {
        if !self.run_opts.tag { return String::new(); }
        let mut n = 0;
        for id in ids {
            let Some(r) = self.document.mep_run(*id) else { continue };
            let mid = [(r.a[0] + r.b[0]) / 2.0, (r.a[1] + r.b[1]) / 2.0];
            if self.document.place_tag(*id, mid, mid).is_ok() { n += 1; }
        }
        format!(", {n} tag{}", if n == 1 { "" } else { "s" })
    }

    /// The tooltip of a pipe being drawn with a slope (Show Slope Tooltip): the slope and the height it changes over the leg
    /// from `from` to `to`.
    pub(crate) fn run_slope_tip(&self, from: [f64; 2], to: [f64; 2]) -> Option<String> {
        let o = &self.run_opts;
        if self.tool != Tool::Pipe || o.slope == SlopeMode::Off || !o.show_slope_tip { return None; }
        let len = (to[0] - from[0]).hypot(to[1] - from[1]);
        Some(format!("Slope {} {:.2} %: {:.0} mm over {:.2} m", if o.slope == SlopeMode::Down { "down" } else { "up" }, o.slope_pct, len * o.slope_pct * 10.0, len))
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
            let Some((el, c)) = crate::elec_ui::equipment_connector_near(&self.document, level, [p[0], p[1]], 0.15, domain, None).filter(|(_, c)| c.system.compatible(sys)) else { continue };
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
        // the sizes of Manage > MEP Settings are offered next to the fields; any other size can be typed
        let (sizes_w, sizes_h, slopes) = { let s = &self.document.mep().settings; (if pipe { s.pipe_diameters_mm.clone() } else { s.duct_widths_mm.clone() }, s.duct_heights_mm.clone(), s.pipe_slopes_pct.clone()) };
        ui.label(if pipe { "Diameter (mm)" } else { "Width (mm)" });
        ui.add(egui::DragValue::new(&mut self.width_mm).speed(1.0).range(10.0..=3000.0));
        size_list(ui, "run_width_list", &sizes_w, &mut self.width_mm);
        if !pipe {
            ui.label("Height (mm)");
            ui.add(egui::DragValue::new(&mut self.height_mm).speed(1.0).range(10.0..=3000.0));
            size_list(ui, "run_height_list", &sizes_h, &mut self.height_mm);
        }
        ui.label("Middle elevation (mm)");
        ui.add(egui::DragValue::new(&mut self.elevation_mm).speed(1.0));
        ui.separator();
        ui.checkbox(&mut self.run_opts.auto_connect, "Automatically Connect").on_hover_text("Join the run to the connector or run it starts on and to the connector it ends on; off draws a plain run");
        ui.checkbox(&mut self.run_opts.inherit_elevation, "Inherit Elevation").on_hover_text("Start at the height of the connector or run the first click is on");
        ui.checkbox(&mut self.run_opts.inherit_size, "Inherit Size").on_hover_text("Take the size of the connector or run the first click is on");
        ui.checkbox(&mut self.run_opts.tag, "Tag on Placement").on_hover_text("Put a tag on every run as it is placed");
        ui.label("Horizontal Justification");
        egui::ComboBox::from_id_salt("run_hjust").selected_text(format!("{:?}", self.run_opts.hjust)).width(70.0).show_ui(ui, |ui| {
            for j in [HJust::Left, HJust::Center, HJust::Right] { ui.selectable_value(&mut self.run_opts.hjust, j, format!("{j:?}")); }
        }).response.on_hover_text("Which side of the run the line you draw is on, looking the way you draw (plain runs)");
        ui.label("Vertical Justification");
        egui::ComboBox::from_id_salt("run_vjust").selected_text(format!("{:?}", self.run_opts.vjust)).width(70.0).show_ui(ui, |ui| {
            for j in [VJust::Top, VJust::Middle, VJust::Bottom] { ui.selectable_value(&mut self.run_opts.vjust, j, format!("{j:?}")); }
        }).response.on_hover_text("Which edge of the run the elevation refers to (plain runs)");
        if pipe {
            ui.label("Slope");
            egui::ComboBox::from_id_salt("run_slope_mode").selected_text(format!("{:?}", self.run_opts.slope)).width(60.0).show_ui(ui, |ui| {
                for m in [SlopeMode::Off, SlopeMode::Up, SlopeMode::Down] { ui.selectable_value(&mut self.run_opts.slope, m, format!("{m:?}")); }
            });
            if self.run_opts.slope != SlopeMode::Off {
                egui::ComboBox::from_id_salt("run_slope_pct").selected_text(format!("{} %", self.run_opts.slope_pct)).width(70.0).show_ui(ui, |ui| {
                    for v in slopes { ui.selectable_value(&mut self.run_opts.slope_pct, v, format!("{v} %")); }
                });
                ui.add(egui::DragValue::new(&mut self.run_opts.slope_pct).speed(0.05).range(0.01..=100.0).suffix(" %"));
                ui.checkbox(&mut self.run_opts.show_slope_tip, "Show Slope Tooltip");
            }
        }
        ui.separator();
        ui.label(format!("{} point{}", self.run_points.len(), if self.run_points.len() == 1 { "" } else { "s" }));
        if ui.add_enabled(self.run_points.len() >= 2, egui::Button::new("Finish")).clicked() { self.finish_run(self.plan_level()); }
        if ui.add_enabled(!self.run_points.is_empty(), egui::Button::new("Clear")).clicked() { self.run_points.clear(); self.run_sizes.clear(); }
        ui.weak("Change the size between two clicks to get a reducer; elbows appear at every bend.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbim_document::{Category, FamilySpec};

    fn lv() -> ElementId { ElementId(10) }
    fn runs(app: &Desktop, cat: Category) -> Vec<ElementId> { app.document.elements().filter(|e| e.category == cat).map(|e| e.id).collect() }
    fn newest(app: &Desktop, cat: Category) -> ElementId { *runs(app, cat).iter().max().unwrap() }
    fn draw(app: &mut Desktop, pts: &[[f64; 2]]) { for p in pts { app.run_click(*p); } app.finish_run(lv()); }

    #[test]
    fn horizontal_justification_puts_the_drawn_line_on_one_edge_of_the_duct() {
        for (j, y) in [(HJust::Center, 0.0), (HJust::Left, -0.2), (HJust::Right, 0.2)] {
            let mut app = Desktop::sample();
            app.choose_tool(Tool::Duct);
            (app.width_mm, app.height_mm, app.elevation_mm) = (400.0, 300.0, 3000.0);
            app.run_opts.hjust = j;
            draw(&mut app, &[[0.0, 20.0], [5.0, 20.0]]);
            let r = app.document.mep_run(newest(&app, Category::Duct)).unwrap();
            // drawn going east, the left is north: a Left line is the north edge, so the centre is 0.2 m south of it
            assert!((r.a[1] - 20.0 - y).abs() < 1e-9 && (r.b[1] - 20.0 - y).abs() < 1e-9, "{j:?}: {:?}", r.a);
            assert!(r.a[0].abs() < 1e-9 && (r.b[0] - 5.0).abs() < 1e-9);
        }
    }

    #[test]
    fn a_justified_run_keeps_its_corners_and_its_elbow() {
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Duct);
        (app.width_mm, app.height_mm, app.elevation_mm) = (400.0, 300.0, 3000.0);
        app.run_opts.hjust = HJust::Right;
        draw(&mut app, &[[0.0, 20.0], [5.0, 20.0], [5.0, 25.0]]);
        assert_eq!(runs(&app, Category::Duct).len() - 1, 2, "two runs besides the sample duct: {}", app.message);
        let ids = runs(&app, Category::Duct);
        let (a, b) = (app.document.mep_run(ids[ids.len() - 2]).unwrap(), app.document.mep_run(ids[ids.len() - 1]).unwrap());
        // right of east is south: the line is the south edge, the centre 0.2 north; right of north is east: centre 0.2 west
        assert!((a.a[1] - 20.2).abs() < 1e-9 && (b.b[0] - 4.8).abs() < 1e-9, "{:?} {:?}", a.a, b.b);
        assert!(!app.document.mep().fittings.is_empty(), "the bend has its elbow");
    }

    #[test]
    fn vertical_justification_says_which_edge_the_elevation_is() {
        for (j, z) in [(VJust::Middle, 3.0), (VJust::Top, 2.85), (VJust::Bottom, 3.15)] {
            let mut app = Desktop::sample();
            app.choose_tool(Tool::Duct);
            (app.width_mm, app.height_mm, app.elevation_mm) = (400.0, 300.0, 3000.0);
            app.run_opts.vjust = j;
            draw(&mut app, &[[0.0, 20.0], [5.0, 20.0]]);
            let r = app.document.mep_run(newest(&app, Category::Duct)).unwrap();
            let l = app.document.get(lv()).unwrap().elevation_m;
            assert!((r.a[2] - l - z).abs() < 1e-9 && (r.b[2] - l - z).abs() < 1e-9, "{j:?}: {}", r.a[2]);
        }
    }

    #[test]
    fn a_pipe_slopes_down_or_up_the_way_it_is_drawn_by_the_value_in_the_bar() {
        for (m, drop) in [(SlopeMode::Off, 0.0), (SlopeMode::Down, 0.1), (SlopeMode::Up, -0.1)] {
            let mut app = Desktop::sample();
            app.choose_tool(Tool::Pipe);
            (app.width_mm, app.elevation_mm) = (100.0, 2000.0);
            app.system_pick = System::Sanitary;
            (app.run_opts.slope, app.run_opts.slope_pct) = (m, 2.0);
            draw(&mut app, &[[0.0, 20.0], [5.0, 20.0]]);
            let r = app.document.mep_run(newest(&app, Category::Pipe)).unwrap_or_else(|| panic!("{}", app.message));
            assert!((r.a[2] - r.b[2] - drop).abs() < 1e-9, "{m:?}: {:?} {:?}", r.a, r.b);
            assert!((r.a[2] - app.document.get(lv()).unwrap().elevation_m - 2.0).abs() < 1e-9, "it starts at the elevation in the bar");
        }
        // the tooltip while drawing says so; off, or switched off, it is silent
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Pipe);
        assert_eq!(app.run_slope_tip([0.0, 0.0], [5.0, 0.0]), None);
        (app.run_opts.slope, app.run_opts.slope_pct) = (SlopeMode::Down, 2.0);
        let tip = app.run_slope_tip([0.0, 0.0], [5.0, 0.0]).unwrap();
        assert!(tip.contains("down") && tip.contains("2.00 %") && tip.contains("100 mm over 5.00 m"), "{tip}");
        app.run_opts.show_slope_tip = false;
        assert_eq!(app.run_slope_tip([0.0, 0.0], [5.0, 0.0]), None);
        // a duct has no slope
        app.run_opts.show_slope_tip = true;
        app.choose_tool(Tool::Duct);
        assert_eq!(app.run_slope_tip([0.0, 0.0], [5.0, 0.0]), None);
    }

    #[test]
    fn tag_on_placement_tags_every_run_and_each_tag_can_be_undone() {
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Duct);
        (app.width_mm, app.height_mm, app.elevation_mm) = (400.0, 300.0, 3000.0);
        draw(&mut app, &[[0.0, 20.0], [5.0, 20.0], [5.0, 25.0]]);
        let tags = |app: &Desktop| app.document.elements().filter(|e| e.category == Category::Tag).count();
        assert_eq!(tags(&app), 0, "off by default");
        app.run_opts.tag = true;
        draw(&mut app, &[[0.0, 30.0], [5.0, 30.0], [5.0, 35.0]]);
        assert_eq!(tags(&app), 2, "{}", app.message);
        assert!(app.message.contains("2 tags"), "{}", app.message);
        // the tags are their own undo steps, after the runs
        let runs_now = runs(&app, Category::Duct).len();
        app.undo_document();
        assert_eq!(tags(&app), 1, "one tag gone");
        app.undo_document();
        assert_eq!((tags(&app), runs(&app, Category::Duct).len()), (0, runs_now));
    }

    #[test]
    fn a_first_click_on_a_run_branches_from_it_and_inherits_its_size_and_height_when_asked() {
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Duct);
        (app.width_mm, app.height_mm, app.elevation_mm) = (400.0, 300.0, 3000.0);
        draw(&mut app, &[[0.0, 20.0], [10.0, 20.0]]);
        let main = newest(&app, Category::Duct);
        // the bar says another size and height; Inherit Size and Inherit Elevation (on) take the run's
        (app.width_mm, app.height_mm, app.elevation_mm) = (150.0, 150.0, 2000.0);
        app.run_opts.inherit_size = true;
        app.run_click([5.0, 20.0]);
        assert_eq!(app.branch_main, Some(main), "{}", app.message);
        assert_eq!((app.width_mm, app.height_mm, app.elevation_mm), (400.0, 300.0, 3000.0));
        app.run_back();
        app.branch_main = None;
        app.run_points.clear();
        // without Inherit Elevation the run must be at the height of the bar, and nothing is taken from it
        app.run_opts.inherit_elevation = false;
        app.run_opts.inherit_size = false;
        (app.width_mm, app.height_mm, app.elevation_mm) = (150.0, 150.0, 2000.0);
        app.run_click([5.0, 20.0]);
        assert_eq!(app.branch_main, None);
        assert_eq!((app.width_mm, app.height_mm, app.elevation_mm), (150.0, 150.0, 2000.0));
        app.run_points.clear();
        // at its height it is found again
        app.elevation_mm = 3000.0;
        app.run_click([5.0, 20.0]);
        assert_eq!(app.branch_main, Some(main));
        app.run_points.clear();
        // with Automatically Connect off a click on a run starts a plain run, whatever else is on
        app.run_opts = RunOpts { auto_connect: false, inherit_elevation: true, inherit_size: true, ..RunOpts::default() };
        app.run_click([5.0, 20.0]);
        assert_eq!(app.branch_main, None);
        let n = app.document.mep().taps.len() + app.document.mep().fittings.len();
        app.run_click([5.0, 24.0]);
        app.finish_run(lv());
        assert_eq!(app.document.mep().taps.len() + app.document.mep().fittings.len(), n, "no tee or tap: {}", app.message);
    }

    #[test]
    fn a_run_from_a_connector_inherits_its_size_and_height_or_goes_to_the_bar_elevation() {
        let mut app = Desktop::sample();
        let fcu = app.document.place_family(lv(), [5.0, 5.0], &FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 2.2 }).unwrap();
        let air = crate::elec_ui::instance_connectors(&app.document, fcu).into_iter().find(|c| c.domain == Domain::Duct && c.flow > 0.0 && c.system == System::SupplyAir).unwrap();
        let (nx, ny) = (air.dir[0], air.dir[1]);
        let openbim_family::mep::Size::Rect { width, height } = air.size else { panic!() };
        let l = app.document.get(lv()).unwrap().elevation_m;
        app.choose_tool(Tool::Duct);
        (app.width_mm, app.height_mm, app.elevation_mm) = (600.0, 300.0, 3300.0);
        app.run_opts.inherit_size = true;
        app.run_click([air.pos[0], air.pos[1]]);
        assert_eq!((app.width_mm, app.height_mm), ((width * 1000.0).round(), (height * 1000.0).round()), "{}", app.message);
        assert_eq!(app.elevation_mm, ((air.pos[2] - l) * 1000.0).round(), "Inherit Elevation is on");
        app.run_click([air.pos[0] + 5.0 * nx, air.pos[1] + 5.0 * ny]);
        app.finish_run(lv());
        assert_eq!(app.document.mep().equip_links.len(), 1, "{}", app.message);
        // Automatically Connect off: a plain run, not joined, at the bar's height
        app.choose_tool(Tool::Duct);
        app.run_opts = RunOpts { auto_connect: false, ..RunOpts::default() };
        (app.width_mm, app.height_mm, app.elevation_mm) = (600.0, 300.0, 3300.0);
        draw(&mut app, &[[air.pos[0], air.pos[1]], [air.pos[0] + 5.0 * nx, air.pos[1] + 5.0 * ny]]);
        assert_eq!(app.document.mep().equip_links.len(), 1, "no new join");
        let r = app.document.mep_run(newest(&app, Category::Duct)).unwrap();
        assert!((r.a[2] - l - 3.3).abs() < 1e-9, "{:?}", r.a);
        // Inherit Elevation off with Automatically Connect on, at the connector's size: it leaves the connector at its
        // height and goes to the bar's elevation after the stub, with its vertical jog
        app.choose_tool(Tool::Duct);
        app.run_opts = RunOpts { inherit_elevation: false, ..RunOpts::default() };
        (app.width_mm, app.height_mm, app.elevation_mm) = ((width * 1000.0).round(), (height * 1000.0).round(), 3300.0);
        let before = runs(&app, Category::Duct);
        draw(&mut app, &[[air.pos[0], air.pos[1]], [air.pos[0] + 5.0 * nx, air.pos[1] + 5.0 * ny]]);
        let new: Vec<_> = runs(&app, Category::Duct).into_iter().filter(|i| !before.contains(i)).collect();
        assert!(!new.is_empty(), "{}", app.message);
        let zs: Vec<f64> = new.iter().filter_map(|i| app.document.mep_run(*i)).flat_map(|r| [r.a[2], r.b[2]]).collect();
        assert!((zs[0] - air.pos[2]).abs() < 1e-9 && zs.iter().any(|z| (z - l - 3.3).abs() < 1e-6), "{zs:?}");
    }

    #[test]
    fn an_end_joined_to_an_elbow_is_not_moved_from_properties_and_says_why() {
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Duct);
        (app.width_mm, app.height_mm, app.elevation_mm) = (400.0, 300.0, 3000.0);
        draw(&mut app, &[[0.0, 20.0], [5.0, 20.0], [5.0, 25.0]]);
        let ids = runs(&app, Category::Duct);
        let (a, b) = (ids[ids.len() - 2], ids[ids.len() - 1]);
        let revision = app.document.revision();
        app.set_run_end_offset(a, false, 3500.0);
        assert!(app.message.contains("joined"), "{}", app.message);
        assert_eq!(app.document.revision(), revision, "nothing changed");
        app.set_run_end_offset(b, false, 3500.0);
        assert!(app.message.starts_with("End of the run"), "{}", app.message);
        let l = app.document.get(lv()).unwrap().elevation_m;
        assert!((app.document.mep_run(b).unwrap().b[2] - l - 3.5).abs() < 1e-9);
    }
}
