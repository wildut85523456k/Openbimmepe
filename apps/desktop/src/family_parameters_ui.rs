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
}

impl FamilyParametersDialog {
    pub fn open(family: &str) -> Self {
        Self { family: family.into(), new_name: String::new(), error: String::new() }
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
