//! Insulation and duct lining (Revit's Modify | Ducts > Duct Insulation / Duct Lining, Modify | Pipes > Pipe Insulation):
//! select ducts or pipes, choose Insulation on the ribbon, pick a type from Mechanical Settings and a thickness, and Add / Edit
//! or Remove it. The fittings joined to the selected runs can be done with them.
use crate::Desktop;
use eframe::egui;
use openbim_document::{Category, ElementId, InsulationKind, MepNode};

/// The state of the window: the runs it works on and what is chosen.
pub(crate) struct InsulationDialog {
    pub pipe: bool,
    pub runs: Vec<ElementId>,
    pub kind: InsulationKind,
    pub type_name: String,
    pub thickness_mm: f64,
    pub with_fittings: bool,
}

fn kind_name(k: InsulationKind) -> &'static str { if k == InsulationKind::Lining { "lining" } else { "insulation" } }

impl Desktop {
    /// The Insulation button of the Duct or Pipe panel: opens the window for the ducts (or pipes) selected.
    pub(crate) fn open_insulation(&mut self, pipe: bool) {
        let want = if pipe { Category::Pipe } else { Category::Duct };
        let runs: Vec<ElementId> = self.selected_ids().into_iter().filter(|i| self.document.get(*i).is_some_and(|e| e.category == want)).collect();
        if runs.is_empty() {
            self.message = format!("Insulation: select one or more {} first (click one, or drag a box), then choose Insulation", if pipe { "pipes" } else { "ducts" });
            return;
        }
        let mut d = InsulationDialog { pipe, runs, kind: InsulationKind::Insulation, type_name: String::new(), thickness_mm: 25.0, with_fittings: true };
        self.fit_insulation_choice(&mut d);
        self.insulation_dialog = Some(d);
    }

    /// Makes the type and thickness fit the kind chosen: those of the first run that has that kind (Edit), else the first
    /// type of the list and the thickness left as it was.
    pub(crate) fn fit_insulation_choice(&self, d: &mut InsulationDialog) {
        if let Some(i) = d.runs.iter().find_map(|r| self.document.mep().insulation_of(MepNode::Run(*r), d.kind)) {
            d.type_name = i.type_name.clone();
            d.thickness_mm = (i.thickness_m * 1000.0).round();
        } else {
            let s = &self.document.mep().settings;
            let list = if d.kind == InsulationKind::Lining { &s.lining_types } else { &s.insulation_types };
            if !list.contains(&d.type_name) { d.type_name = list.first().cloned().unwrap_or_default(); }
        }
    }

    /// The runs of the window and, if asked, the fittings joined to them.
    pub(crate) fn insulation_nodes(&self, d: &InsulationDialog) -> Vec<MepNode> {
        let mut nodes: Vec<MepNode> = d.runs.iter().map(|r| MepNode::Run(*r)).collect();
        if d.with_fittings {
            for (a, b) in &self.document.mep().links {
                for (r, f) in [(a, b), (b, a)] {
                    if let (MepNode::Run(id), MepNode::Fitting(_)) = (r.node, f.node) {
                        if d.runs.contains(&id) && !nodes.contains(&f.node) { nodes.push(f.node); }
                    }
                }
            }
        }
        nodes
    }

    /// Add / Edit (or Remove, when `remove`): one undoable step.
    pub(crate) fn apply_insulation(&mut self, d: &InsulationDialog, remove: bool) -> bool {
        let nodes = self.insulation_nodes(d);
        let ops = if remove { self.document.remove_insulation_commands(&nodes, d.kind) } else { self.document.insulation_commands(&nodes, d.kind, &d.type_name, d.thickness_mm / 1000.0) };
        let what = kind_name(d.kind);
        match ops {
            Ok(ops) => match self.document.transact(self.document.revision(), &ops) {
                Ok(_) => {
                    let fittings = nodes.iter().filter(|n| matches!(n, MepNode::Fitting(_))).count();
                    let (runs, verb, prep) = (d.runs.len(), if remove { "Removed the" } else { "Put" }, if remove { "from" } else { "on" });
                    self.message = format!("{verb} {what} {prep} {runs} run{} and {fittings} fitting{}; undo takes it back", if runs == 1 { "" } else { "s" }, if fittings == 1 { "" } else { "s" });
                    true
                }
                Err(e) => { self.message = format!("Insulation rejected ({e:?})"); false }
            },
            Err(e) => { self.message = format!("Insulation: {e}"); false }
        }
    }

    pub(crate) fn insulation_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.insulation_dialog.take() else { return };
        d.runs.retain(|r| self.document.get(*r).is_some());
        let (mut open, mut add, mut remove, mut close) = (true, false, false, false);
        let title = format!("{} {}", if d.pipe { "Pipe" } else { "Duct" }, if d.kind == InsulationKind::Lining { "Lining" } else { "Insulation" });
        let before = d.kind;
        egui::Window::new(title).id(egui::Id::new("insulation_window")).open(&mut open).default_width(340.0).show(ctx, |ui| {
            ui.label(format!("{} selected {}", d.runs.len(), if d.pipe { "pipe(s)" } else { "duct(s)" }));
            if !d.pipe {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut d.kind, InsulationKind::Insulation, "Insulation (outside)");
                    ui.radio_value(&mut d.kind, InsulationKind::Lining, "Lining (inside)");
                });
            }
            let list = { let s = &self.document.mep().settings; if d.kind == InsulationKind::Lining { s.lining_types.clone() } else { s.insulation_types.clone() } };
            ui.horizontal(|ui| {
                ui.label("Type");
                egui::ComboBox::from_id_salt("insulation_type").selected_text(d.type_name.clone()).show_ui(ui, |ui| {
                    for t in &list { ui.selectable_value(&mut d.type_name, t.clone(), t); }
                }).response.on_hover_text("The types come from Manage > MEP Settings");
            });
            ui.horizontal(|ui| {
                ui.label("Thickness (mm)");
                ui.add(egui::DragValue::new(&mut d.thickness_mm).speed(1.0).range(1.0..=200.0));
            });
            ui.checkbox(&mut d.with_fittings, "Also the fittings joined to these runs");
            ui.horizontal(|ui| {
                if ui.button("Add / Edit").clicked() { add = true; }
                if ui.button("Remove").clicked() { remove = true; }
                if ui.button("Close").clicked() { close = true; }
            });
            if d.kind == InsulationKind::Lining { ui.small("Lining is kept with the duct and shown in its properties; it is not drawn."); }
        });
        if d.kind != before { self.fit_insulation_choice(&mut d); }
        let done = (add || remove) && self.apply_insulation(&d, remove);
        if open && !close && !done { self.insulation_dialog = Some(d); }
    }

    /// The outlines of the insulation on the ducts and pipes of `level`, for the plan: the run grown by the thickness on both sides.
    pub(crate) fn insulation_outlines(&self, level: ElementId) -> Vec<[[f64; 2]; 4]> {
        self.document.mep().insulations.iter().filter(|i| i.kind == InsulationKind::Insulation).filter_map(|i| {
            let MepNode::Run(id) = i.node else { return None };
            let e = self.document.get(id)?;
            let Some(openbim_document::Geometry::Linear { start, end, level: l }) = e.geometry else { return None };
            if l != level { return None; }
            let len = (end[0] - start[0]).hypot(end[1] - start[1]);
            if len < 1e-9 { return None; }
            let n = [-(end[1] - start[1]) / len, (end[0] - start[0]) / len];
            let h = e.width_m / 2.0 + i.thickness_m;
            Some([[start[0] + n[0] * h, start[1] + n[1] * h], [end[0] + n[0] * h, end[1] + n[1] * h], [end[0] - n[0] * h, end[1] - n[1] * h], [start[0] - n[0] * h, start[1] - n[1] * h]])
        }).collect()
    }

    /// The rows of the Properties palette for the insulation and lining of a run.
    pub(crate) fn insulation_rows(&self, id: ElementId) -> Vec<(String, String)> {
        [InsulationKind::Insulation, InsulationKind::Lining].into_iter().filter_map(|k| {
            let i = self.document.mep().insulation_of(MepNode::Run(id), k)?;
            Some((if k == InsulationKind::Lining { "Lining" } else { "Insulation" }.to_string(), format!("{}, {:.0} mm", i.type_name, i.thickness_m * 1000.0)))
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tool;
    use openbim_view::Kind;

    fn draw_duct(app: &mut Desktop, pts: &[[f64; 2]]) -> Vec<ElementId> {
        let before: Vec<ElementId> = app.document.elements().filter(|e| e.category == Category::Duct).map(|e| e.id).collect();
        app.choose_tool(Tool::Duct);
        (app.width_mm, app.height_mm, app.elevation_mm) = (400.0, 300.0, 3000.0);
        for p in pts { app.run_click(*p); }
        app.finish_run(ElementId(10));
        app.document.elements().filter(|e| e.category == Category::Duct && !before.contains(&e.id)).map(|e| e.id).collect()
    }

    #[test]
    fn the_button_needs_a_selection_of_its_kind_and_the_window_adds_edits_and_removes_in_one_step_each() {
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Select);
        app.select_element(None);
        app.run_command("duct_insulation", "Insulation");
        assert!(app.insulation_dialog.is_none() && app.message.contains("select"), "{}", app.message);
        let ids = draw_duct(&mut app, &[[0.0, 20.0], [6.0, 20.0], [6.0, 26.0]]);
        assert_eq!(ids.len(), 2, "{}", app.message);
        app.choose_tool(Tool::Select);
        app.select_many(ids.clone());
        // a pipe button with only ducts selected does not open
        app.run_command("pipe_insulation", "Insulation");
        assert!(app.insulation_dialog.is_none() && app.message.contains("pipes"), "{}", app.message);
        app.run_command("duct_insulation", "Insulation");
        let d = app.insulation_dialog.take().expect("opens on two ducts");
        assert_eq!((d.runs.len(), d.type_name.as_str(), d.thickness_mm), (2, "Fiberglass", 25.0));
        // the elbow between the runs goes with them when asked, and not otherwise
        assert_eq!(app.insulation_nodes(&d).len(), 3);
        let mut alone = InsulationDialog { with_fittings: false, ..d };
        assert_eq!(app.insulation_nodes(&alone).len(), 2);
        alone.with_fittings = true;
        alone.thickness_mm = 40.0;
        let revision = app.document.revision();
        assert!(app.apply_insulation(&alone, false), "{}", app.message);
        assert!(app.document.revision() > revision && app.message.contains("2 run"), "{}", app.message);
        assert_eq!(app.document.mep().insulations.len(), 3);
        // reopened, it shows what the ducts have (Edit), and the same again is refused without a step
        app.run_command("duct_insulation", "Insulation");
        let mut d = app.insulation_dialog.take().unwrap();
        assert_eq!((d.type_name.as_str(), d.thickness_mm), ("Fiberglass", 40.0));
        let revision = app.document.revision();
        assert!(!app.apply_insulation(&d, false) && app.message.contains("already"), "{}", app.message);
        assert_eq!(app.document.revision(), revision);
        d.type_name = "Mineral wool".into();
        d.thickness_mm = 30.0;
        assert!(app.apply_insulation(&d, false));
        assert_eq!(app.document.mep().insulations.len(), 3, "edited, not added");
        assert_eq!(app.insulation_rows(ids[0]), vec![("Insulation".to_string(), "Mineral wool, 30 mm".to_string())]);
        // lining is a second layer, shown in the properties; the outline in the plan is the insulation's
        d.kind = InsulationKind::Lining;
        app.fit_insulation_choice(&mut d);
        assert_eq!(d.type_name, "Fiberglass liner");
        assert!(app.apply_insulation(&d, false));
        assert_eq!(app.insulation_rows(ids[0]).len(), 2);
        // undo takes the lining back, then the edit
        app.undo_document();
        assert_eq!(app.insulation_rows(ids[0]).len(), 1);
        // remove: the insulation goes from the ducts and the elbow, and removing again says there is none
        d.kind = InsulationKind::Insulation;
        assert!(app.apply_insulation(&d, true), "{}", app.message);
        assert!(app.document.mep().insulations.is_empty());
        assert!(!app.apply_insulation(&d, true) && app.message.contains("none of them"), "{}", app.message);
    }

    #[test]
    fn the_plan_outlines_the_insulation_and_the_3d_view_has_it_as_its_own_mesh_beside_the_duct() {
        let mut app = Desktop::sample();
        let ids = draw_duct(&mut app, &[[0.0, 20.0], [6.0, 20.0]]);
        app.select_many(ids.clone());
        app.run_command("duct_insulation", "Insulation");
        let mut d = app.insulation_dialog.take().unwrap();
        d.thickness_mm = 50.0;
        assert!(app.apply_insulation(&d, false));
        // 400 mm wide duct, 50 mm of insulation: the outline is 0.5 m across, along the run, on its level only
        let o = app.insulation_outlines(ElementId(10));
        assert_eq!(o.len(), 1);
        let ys: Vec<f64> = o[0].iter().map(|p| p[1]).collect();
        assert!((ys.iter().cloned().fold(f64::MIN, f64::max) - 20.25).abs() < 1e-9 && (ys.iter().cloned().fold(f64::MAX, f64::min) - 19.75).abs() < 1e-9, "{ys:?}");
        assert!(app.insulation_outlines(ElementId(11)).is_empty());
        // 3D: the duct keeps its own mesh and the insulation is another, bigger than the duct, with the number of the run
        let b = crate::view3d::batch(&app.document);
        let run = ids[0].0;
        assert!(b.elements.iter().any(|e| e.kind == Kind::Segment && e.id == run), "the duct is still there");
        let ins = b.elements.iter().position(|e| e.kind == Kind::Insulation && e.id == run).expect("the insulation is a mesh of its own");
        let seg = b.elements.iter().position(|e| e.kind == Kind::Segment && e.id == run).unwrap();
        let extent = |el: usize| {
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for (t, owner) in b.tris.iter().zip(&b.tri_element) {
                if *owner as usize != el { continue; }
                for &v in t { for k in 0..3 { lo[k] = lo[k].min(b.verts[v as usize][k]); hi[k] = hi[k].max(b.verts[v as usize][k]); } }
            }
            [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]
        };
        let (e_ins, e_seg) = (extent(ins), extent(seg));
        assert!((e_ins[1] - e_seg[1] - 0.1).abs() < 1e-3 && (e_ins[2] - e_seg[2] - 0.1).abs() < 1e-3, "{e_ins:?} against {e_seg:?}: 50 mm more on every side");
        // lining is not drawn
        d.kind = InsulationKind::Lining;
        app.fit_insulation_choice(&mut d);
        assert!(app.apply_insulation(&d, false));
        let b2 = crate::view3d::batch(&app.document);
        assert_eq!(b2.elements.iter().filter(|e| e.kind == Kind::Insulation).count(), 1);
    }

    #[test]
    fn insulation_goes_out_in_the_ifc_file_and_comes_back_and_the_window_draws() {
        let mut app = Desktop::sample();
        let ids = draw_duct(&mut app, &[[0.0, 20.0], [6.0, 20.0], [6.0, 26.0]]);
        app.select_many(ids.clone());
        app.run_command("duct_insulation", "Insulation");
        let d = app.insulation_dialog.take().unwrap();
        assert!(app.apply_insulation(&d, false));
        let out = crate::export::export_ifc_as(&app.document, "P", 1, "2026-10-19T00:00:00", crate::export::Schema::Ifc4).unwrap();
        let mut back = openbim_document::Document::new();
        crate::import::import_ifc(&mut back, &out.text).unwrap();
        assert_eq!(back.mep().insulations, app.document.mep().insulations);
        // the window draws, with the types of the settings
        app.run_command("duct_insulation", "Insulation");
        let ctx = egui::Context::default();
        let mut text = String::new();
        for _ in 0..2 {
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1000.0, 700.0));
            let mut o = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| { app.insulation_window(ui.ctx()); });
            o.textures_delta.clear();
            text.clear();
            fn walk(s: &egui::Shape, t: &mut String) { match s { egui::Shape::Text(x) => { t.push_str(x.galley.text()); t.push('\n'); } egui::Shape::Vec(v) => v.iter().for_each(|y| walk(y, t)), _ => {} } }
            for c in &o.shapes { walk(&c.shape, &mut text); }
        }
        for want in ["Duct Insulation", "2 selected duct(s)", "Insulation (outside)", "Lining (inside)", "Fiberglass", "Thickness (mm)", "Add / Edit", "Remove"] {
            assert!(text.contains(want), "'{want}' in:\n{text}");
        }
    }
}
