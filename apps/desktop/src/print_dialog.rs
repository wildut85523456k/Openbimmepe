//! Print/export settings and explicit file writes for the desktop prototype.
use crate::{plan_export, View};
use eframe::egui;
use openbim_document::Document;
use openbim_sheets::export::{ExportBundle, ExportFormat};
use openbim_sheets::profile::PrintProfile;
use openbim_sheets::render::PrintMode;
use std::io::{Read, Write};
use std::path::Path;

struct Prepared {
    revision: u64,
    view: View,
    output: ExportBundle,
}

pub struct PrintDialog {
    pub open: bool,
    profile: PrintProfile,
    paper: String,
    scale: u32,
    format: ExportFormat,
    output_path: String,
    profile_path: String,
    prepared: Option<Prepared>,
    status: String,
}

impl Default for PrintDialog {
    fn default() -> Self {
        Self {
            open: false, profile: plan_export::generic_profile(), paper: "A3 landscape".into(),
            scale: 100, format: ExportFormat::Pdf, output_path: "plan.pdf".into(),
            profile_path: String::new(), prepared: None, status: String::new(),
        }
    }
}

fn load_profile(path: &Path) -> Result<PrintProfile, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Cannot open profile: {e}"))?;
    let mut text = String::new();
    file.take(1_048_577).read_to_string(&mut text).map_err(|e| format!("Cannot read profile: {e}"))?;
    PrintProfile::from_json(&text).map_err(|e| e.to_string())
}

fn extension(format: ExportFormat) -> &'static str {
    match format { ExportFormat::Pdf => "pdf", ExportFormat::Dxf => "dxf" }
}

/// An explicit destination for this single-plan export. Existing files are
/// preserved; a failed write removes only the new file this call created.
fn write_new(path: &Path, format: ExportFormat, bundle: &ExportBundle) -> Result<(), String> {
    if path.extension().and_then(|s| s.to_str()).is_none_or(|s| !s.eq_ignore_ascii_case(extension(format))) {
        return Err(format!("Choose a destination ending in .{}", extension(format)));
    }
    let [file] = bundle.files.as_slice() else { return Err("Expected one plan export file.".into()); };
    let mut target = std::fs::OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|e| format!("Cannot create {}: {e}. Choose an unused filename.", path.display()))?;
    let result = target.write_all(&file.bytes).and_then(|()| target.sync_all());
    drop(target);
    if let Err(error) = result {
        let cleanup = std::fs::remove_file(path);
        return Err(match cleanup {
            Ok(()) => format!("Export failed: {error}. Incomplete file removed."),
            Err(cleanup_error) => format!("Export failed: {error}. Could not remove incomplete file {}: {cleanup_error}", path.display()),
        });
    }
    Ok(())
}

impl PrintDialog {
    fn invalidate(&mut self) { self.prepared = None; self.status.clear(); }

    pub fn show(&mut self, ctx: &egui::Context, document: &Document, view: View, design: bool) {
        if !self.open { return; }
        if self.prepared.as_ref().is_some_and(|p| p.revision != document.revision() || p.view != view || !design) {
            self.invalidate();
            self.status = "View or document changed. Prepare the export again.".into();
        }
        let mut open = self.open;
        egui::Window::new("Print / Export plan").open(&mut open).default_width(620.0)
            .resizable(true).show(ctx, |ui| {
                ui.label("Vector PDF or DXF from the current in-memory plan.");
                ui.small("Generic title block and category layers. Company standards have not been validated.");
                if !design || view == View::ThreeD {
                    ui.colored_label(ui.visuals().error_fg_color, "Switch to a Design plan view to export.");
                }
                ui.separator();
                ui.label(format!("Profile: {}", self.profile.name));
                ui.horizontal(|ui| {
                    ui.label("JSON profile path");
                    ui.text_edit_singleline(&mut self.profile_path);
                    if ui.button("Load profile").clicked() {
                        self.prepared = None;
                        match load_profile(Path::new(&self.profile_path)) {
                            Ok(profile) => {
                                self.invalidate();
                                if !profile.papers.iter().any(|p| p.name == self.paper) {
                                    self.paper = profile.papers[0].name.clone();
                                }
                                self.profile = profile;
                                self.status = "Loaded profile. Title block remains a generic placeholder.".into();
                            }
                            Err(error) => self.status = format!("Profile not loaded: {error}"),
                        }
                    }
                    if ui.button("Use generic").clicked() {
                        self.profile = plan_export::generic_profile();
                        self.paper = "A3 landscape".into(); self.invalidate();
                    }
                });
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label("Paper");
                    egui::ComboBox::from_id_salt("print-paper").selected_text(&self.paper).show_ui(ui, |ui| {
                        for paper in &self.profile.papers {
                            changed |= ui.selectable_value(&mut self.paper, paper.name.clone(), &paper.name).changed();
                        }
                    });
                    if let Some(p) = self.profile.papers.iter().find(|p| p.name == self.paper) {
                        ui.label(format!("{} x {} mm", p.width_mm, p.height_mm));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Scale 1:");
                    changed |= ui.add(egui::DragValue::new(&mut self.scale).range(1..=1_000_000)).changed();
                    ui.small("Fixed scale; independent of viewport zoom/pan.");
                });
                ui.horizontal(|ui| {
                    ui.label("Colour mode");
                    egui::ComboBox::from_id_salt("print-mode").selected_text(match self.profile.mode {
                        PrintMode::Colour => "Colour", PrintMode::Grayscale => "Grayscale", PrintMode::BlackLines => "Black lines",
                    }).show_ui(ui, |ui| {
                        for (mode, label) in [(PrintMode::Colour, "Colour"), (PrintMode::Grayscale, "Grayscale"), (PrintMode::BlackLines, "Black lines")] {
                            changed |= ui.selectable_value(&mut self.profile.mode, mode, label).changed();
                        }
                    });
                    ui.label("Format");
                    let old_format = self.format;
                    changed |= ui.selectable_value(&mut self.format, ExportFormat::Pdf, "PDF").changed();
                    changed |= ui.selectable_value(&mut self.format, ExportFormat::Dxf, "DXF").changed();
                    if old_format != self.format {
                        self.output_path = Path::new(&self.output_path).with_extension(extension(self.format)).to_string_lossy().into_owned();
                    }
                });
                if changed { self.invalidate(); }
                ui.horizontal(|ui| { ui.label("Destination path"); ui.text_edit_singleline(&mut self.output_path); });
                ui.small("Existing files are preserved. Use an absolute path to choose a folder.");
                ui.separator();
                if ui.add_enabled(design && view != View::ThreeD, egui::Button::new("Prepare export")).clicked() {
                    match plan_export::export_plan(document, view, &self.profile, &self.paper, self.scale, self.format) {
                        Ok(output) => {
                            self.prepared = Some(Prepared { revision: document.revision(), view, output });
                            self.status = "Export prepared. Review the warnings below, then save.".into();
                        }
                        Err(error) => { self.prepared = None; self.status = format!("Cannot export: {error}"); }
                    }
                }
                if let Some(prepared) = &self.prepared {
                    let bytes: usize = prepared.output.files.iter().map(|f| f.bytes.len()).sum();
                    ui.label(format!("Ready: {} file, {bytes} bytes, document revision {}", prepared.output.files.len(), prepared.revision));
                    egui::ScrollArea::vertical().id_salt("export-warnings").max_height(150.0).show(ui, |ui| {
                        for warning in &prepared.output.warnings { ui.label(format!("• {warning}")); }
                    });
                    if ui.button(format!("Save {}", extension(self.format).to_ascii_uppercase())).clicked() {
                        self.status = match write_new(Path::new(&self.output_path), self.format, &prepared.output) {
                            Ok(()) => format!("Export saved: {}", self.output_path),
                            Err(error) => error,
                        };
                    }
                }
                if !self.status.is_empty() { ui.label(&self.status); }
                ui.small("For paper printing, open the exported PDF in a PDF viewer. OS printer integration is not connected.");
            });
        self.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("openbim-export-{}-{id}", std::process::id()));
        std::fs::create_dir(&dir).unwrap(); dir
    }

    #[test]
    fn export_writes_real_bytes_and_preserves_existing_destinations() {
        let doc = Document::spatial_sample();
        let profile = plan_export::generic_profile();
        let output = plan_export::export_plan(&doc, View::Plan, &profile, "A3 landscape", 100, ExportFormat::Pdf).unwrap();
        let dir = temp_dir(); let path = dir.join("plan.pdf");
        write_new(&path, ExportFormat::Pdf, &output).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), output.files[0].bytes);
        assert!(write_new(&path, ExportFormat::Pdf, &output).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), output.files[0].bytes);
        assert!(write_new(&dir.join("wrong.dxf"), ExportFormat::Pdf, &output).is_err());
        assert!(!dir.join("wrong.dxf").exists());
        assert!(write_new(&dir.join("missing/plan.pdf"), ExportFormat::Pdf, &output).is_err());
        let profile_path = dir.join("profile.json");
        std::fs::write(&profile_path, profile.to_json().unwrap()).unwrap();
        assert_eq!(load_profile(&profile_path).unwrap(), profile);
        std::fs::write(&profile_path, "{invalid}").unwrap();
        assert!(load_profile(&profile_path).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn settings_dialog_renders_in_both_themes_and_invalidates_stale_bytes() {
        let ctx = egui::Context::default();
        let mut doc = Document::spatial_sample();
        let mut dialog = PrintDialog { open: true, ..Default::default() };
        dialog.prepared = Some(Prepared {
            revision: doc.revision(), view: View::Plan,
            output: plan_export::export_plan(&doc, View::Plan, &dialog.profile, &dialog.paper, dialog.scale, dialog.format).unwrap(),
        });
        for dark in [false, true] {
            ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
            let before = doc.clone();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                dialog.show(ui.ctx(), &doc, View::Plan, true);
            });
            // a run_ui produces texture deltas that must be cleared before the context is dropped,
            // or egui panics "Dropped TexturesDelta with N unapplied deltas" (as the model_view tests do)
            output.textures_delta.clear();
            assert!(!output.shapes.is_empty());
            assert_eq!(doc, before);
        }
        doc.transact(doc.revision(), &[openbim_document::Command::Resize { id: crate::ElementId(1), width_m: 0.4, height_m: 0.3 }]).unwrap();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| dialog.show(ui.ctx(), &doc, View::Plan, true));
        output.textures_delta.clear();
        assert!(dialog.prepared.is_none());
        assert!(dialog.status.contains("changed"));
    }
}
