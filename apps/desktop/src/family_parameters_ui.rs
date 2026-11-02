//! Family Parameters dialog (Revit-parity): lists the parameters of a family's FamilyDef
//! and lets the user add/remove/rename them. Backed by document.family_defs.
use crate::Desktop;
use eframe::egui;
use openbim_document::Document;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct FamilyParametersDialog {
    pub family: String,
    pub new_name: String,
    pub error: String,
    pub constraint_a: usize,
    pub constraint_b: usize,
    pub geom_name: String,
    pub geom_vis: usize,
    pub geom_mat: usize,
    pub nested_name: String,
}

impl FamilyParametersDialog {
    pub fn open(family: &str) -> Self {
        Self { family: family.into(), new_name: String::new(), error: String::new(), constraint_a: 0, constraint_b: 1, geom_name: String::new(), geom_vis: 0, geom_mat: 0, nested_name: String::new() }
    }
    pub fn show(&mut self, ctx: &egui::Context, doc: &mut Document) -> bool {
        let mut open = true;
        let mut changed = false;
        egui::Window::new("Family Parameters").open(&mut open).show(ctx, |ui| {
            ui.label(format!("Family: {}", self.family));
            if let Some(def) = doc.family_defs().get(&self.family).cloned() {
                ui.label(format!("{} parameters", def.parameters.len()));
                for p in &def.parameters {
                    ui.label(format!("{} ({:?})", p.name, p.kind));
                }
            } else {
                ui.label("No FamilyDef stored for this family.");
            }
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.new_name);
                if ui.button("Add").clicked() && !self.new_name.trim().is_empty() {
                    let mut defs = doc.family_defs().clone();
                    if let Some(def) = defs.get_mut(&self.family) {
                        let p = openbim_document::family_params::Parameter {
                            id: (def.parameters.len() as u64) + 1,
                            name: self.new_name.trim().to_string(),
                            discipline: openbim_document::family_params::ParamDiscipline::Common,
                            kind: openbim_document::family_params::ParamKind::Number,
                            group: openbim_document::family_params::ParamGroup::Other,
                            is_instance: false,
                            formula: None,
                            value: openbim_document::family_params::ParamValue::Number(0.0),
                        };
                        if def.add_parameter(p) { self.new_name.clear(); changed = true; }
                        else { self.error = "duplicate name".into(); }
                    }
                    doc.set_family_defs(defs);
                }
            });
            if !self.error.is_empty() { ui.colored_label(egui::Color32::RED, &self.error); }
        });
        !open || changed
    }
}

impl crate::Desktop {
    pub(crate) fn family_parameters_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.family_parameters_dialog.take() else { return };
        let mut open = true;
        let family = d.family.clone();
        let mut add_binding = false;
        let mut add_nested = false;
        egui::Window::new(format!("Family Parameters: {}", family)).open(&mut open).show(ctx, |ui| {
            if let Some(def) = self.document.family_defs().get(&family) {
                ui.label(format!("{} parameters", def.parameters.len()));
                for p in &def.parameters { ui.label(format!("{} ({:?})", p.name, p.kind)); }
                ui.label(format!("{} constraints", def.constraints.len()));
                ui.label(format!("{} geometry bindings", def.geometry_bindings.len()));
                let vis_names: Vec<String> = std::iter::once("(none)".to_string()).chain(def.parameters.iter().map(|p| p.name.clone())).collect();
                let mat_names: Vec<String> = std::iter::once("(none)".to_string()).chain(def.parameters.iter().map(|p| p.name.clone())).collect();
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut d.geom_name);
                    egui::ComboBox::from_id_salt("gv").selected_text(vis_names.get(d.geom_vis).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, n) in vis_names.iter().enumerate() { ui.selectable_value(&mut d.geom_vis, i, n); }
                    });
                    egui::ComboBox::from_id_salt("gm").selected_text(mat_names.get(d.geom_mat).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, n) in mat_names.iter().enumerate() { ui.selectable_value(&mut d.geom_mat, i, n); }
                    });
                    if ui.button("Add binding").clicked() && !d.geom_name.trim().is_empty() {
                        add_binding = true;
                    }
                });
                ui.label(format!("{} nested families", def.nested.len()));
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut d.nested_name);
                    if ui.button("Add nested").clicked() && !d.nested_name.trim().is_empty() {
                        add_nested = true;
                    }
                });
                let names: Vec<String> = def.parameters.iter().map(|p| p.name.clone()).collect();
                let ids: Vec<u64> = def.parameters.iter().map(|p| p.id).collect();
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("ca").selected_text(names.get(d.constraint_a).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() { ui.selectable_value(&mut d.constraint_a, i, n); }
                    });
                    egui::ComboBox::from_id_salt("cb").selected_text(names.get(d.constraint_b).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() { ui.selectable_value(&mut d.constraint_b, i, n); }
                    });
                    if ui.button("Add Equal").clicked() && d.constraint_a < ids.len() && d.constraint_b < ids.len() && d.constraint_a != d.constraint_b {
                        let mut defs = self.document.family_defs().clone();
                        if let Some(def) = defs.get_mut(&family) {
                            let c = openbim_document::constraints::Constraint::equal(
                                (def.constraints.len() as u64) + 1,
                                ids[d.constraint_a],
                                ids[d.constraint_b],
                            );
                            def.constraints.push(c);
                        }
                        self.document.set_family_defs(defs);
                    }
                });
            } else {
                ui.label("No FamilyDef stored for this family.");
            }
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut d.new_name);
                if ui.button("Add").clicked() && !d.new_name.trim().is_empty() {
                    let mut defs = self.document.family_defs().clone();
                    if let Some(def) = defs.get_mut(&family) {
                        let p = openbim_document::family_params::Parameter {
                            id: (def.parameters.len() as u64) + 1,
                            name: d.new_name.trim().to_string(),
                            discipline: openbim_document::family_params::ParamDiscipline::Common,
                            kind: openbim_document::family_params::ParamKind::Number,
                            group: openbim_document::family_params::ParamGroup::Other,
                            is_instance: false,
                            formula: None,
                            value: openbim_document::family_params::ParamValue::Number(0.0),
                        };
                        if def.add_parameter(p) { d.new_name.clear(); }
                        else { d.error = "duplicate name".into(); }
                    }
                    self.document.set_family_defs(defs);
                }
            });
            if add_binding {
                let mut defs = self.document.family_defs().clone();
                if let Some(def) = defs.get_mut(&family) {
                    let vis_param = if d.geom_vis > 0 && d.geom_vis - 1 < def.parameters.len() { Some(def.parameters[d.geom_vis - 1].id) } else { None };
                    let mat_param = if d.geom_mat > 0 && d.geom_mat - 1 < def.parameters.len() { Some(def.parameters[d.geom_mat - 1].id) } else { None };
                    def.geometry_bindings.push(openbim_document::geometry_binding::GeometryBinding {
                        geometry: d.geom_name.trim().to_string(),
                        dimension_params: Vec::new(),
                        visibility_param: vis_param,
                        material_param: mat_param,
                    });
                    d.geom_name.clear();
                }
                self.document.set_family_defs(defs);
            }
            if add_nested {
                let mut defs = self.document.family_defs().clone();
                if let Some(def) = defs.get_mut(&family) {
                    def.nested.push(openbim_document::nested_family::NestedFamily {
                        family: d.nested_name.trim().to_string(),
                        ..Default::default()
                    });
                    d.nested_name.clear();
                }
                self.document.set_family_defs(defs);
            }
            if !d.error.is_empty() { ui.colored_label(egui::Color32::RED, &d.error); }
        });
        if open { self.family_parameters_dialog = Some(d); }
    }
}
