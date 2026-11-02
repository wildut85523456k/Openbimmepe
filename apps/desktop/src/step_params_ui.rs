//! STEP parameter table: the geometry of an imported STEP part, as a table of
//! the parameters that become the family's instance parameters (Width / Depth /
//! Height / Volume) and drive quantity take-off. Shown after Import STEP so the
//! user can read what the manufacturer's part is before placing it.
use eframe::egui;

/// The parameter table of one imported STEP part.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StepParamsDialog {
    pub name: String,
    pub params: crate::step_import::StepParams,
}

impl StepParamsDialog {
    pub(crate) fn window(&mut self, ctx: &egui::Context) {
        let d = self.clone();
        let mut open = true;
        egui::Window::new(format!("STEP parameters: {}", d.name))
            .open(&mut open)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.heading("Geometry");
                egui::Grid::new("step_params_grid").striped(true).show(ui, |ui| {
                    row(ui, "Width (X)", d.params.width);
                    row(ui, "Depth (Y)", d.params.depth);
                    row(ui, "Height (Z)", d.params.height);
                    row(ui, "Volume", d.params.volume);
                    row(ui, "Centroid X", d.params.centroid[0]);
                    row(ui, "Centroid Y", d.params.centroid[1]);
                    row(ui, "Centroid Z", d.params.centroid[2]);
                });
                ui.separator();
                ui.label(format!(
                    "Mesh: {} vertices, {} triangles (cadrum display resolution)",
                    d.params.vertices, d.params.triangles
                ));
                ui.add_space(8.0);
            });
        if !open { self.name.clear(); }
    }
}

fn row(ui: &mut egui::Ui, label: &str, metres: f64) {
    ui.label(label);
    ui.monospace(format!("{metres:.4} m"));
    ui.end_row();
}
