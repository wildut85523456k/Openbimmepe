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
}

impl FamilyParametersDialog {
    pub fn open(family: &str) -> Self {
        Self { family: family.into(), new_name: String::new(), error: String::new(), constraint_a: 0, constraint_b: 1 }
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
        egui::Window::new(format!("Family Parameters: {}", family)).open(&mut open).show(ctx, |ui| {
            if let Some(def) = self.document.family_defs().get(&family) {
                ui.label(format!("{} parameters", def.parameters.len()));
                for p in &def.parameters { ui.label(format!("{} ({:?})", p.name, p.kind)); }
                ui.label(format!("{} constraints", def.constraints.len()));
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
            if !d.error.is_empty() { ui.colored_label(egui::Color32::RED, &d.error); }
        });
        if open { self.family_parameters_dialog = Some(d); }
    }
}
