//! Georeferencing of the model for the IFC file: the map the surveyor works in and where the survey point is on it.
//! The site's latitude, longitude and angle to true north are the document's own (the Site of the 3D view); what the
//! document does not hold is kept here as one `U|kind=georef` line of the file, so it is saved and reopened with the
//! model. Without a map name the file gets no map conversion, and the export says so.
use crate::compute_ui::{esc, unesc};
use eframe::egui;
use openbim_ifc::georef::{CoordinateBase, Crs, Georef};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GeorefInput {
    /// the map by name, for example an authority code with its number; empty means no georeferencing is written
    pub crs_name: String,
    pub geodetic_datum: String,
    pub vertical_datum: String,
    pub map_projection: String,
    pub map_zone: String,
    /// shared coordinates of the survey point: easting, northing, height
    pub shared: [f64; 3],
    /// where the survey point is in the model, x and y
    pub survey_xy: [f64; 2],
    /// elevation of the site above the vertical datum
    pub elevation_m: f64,
    pub base: CoordinateBase,
}

impl Default for GeorefInput {
    fn default() -> Self {
        Self { crs_name: String::new(), geodetic_datum: String::new(), vertical_datum: String::new(), map_projection: String::new(), map_zone: String::new(),
            shared: [0.0; 3], survey_xy: [0.0; 2], elevation_m: 0.0, base: CoordinateBase::InternalOrigin }
    }
}

fn base_code(b: CoordinateBase) -> u8 { match b { CoordinateBase::InternalOrigin => 0, CoordinateBase::ProjectBasePoint => 1, CoordinateBase::SurveyPoint => 2, CoordinateBase::SharedCoordinates => 3 } }
fn base_of(c: &str) -> Option<CoordinateBase> { Some(match c { "0" => CoordinateBase::InternalOrigin, "1" => CoordinateBase::ProjectBasePoint, "2" => CoordinateBase::SurveyPoint, "3" => CoordinateBase::SharedCoordinates, _ => return None }) }
fn base_name(b: CoordinateBase) -> &'static str {
    match b { CoordinateBase::InternalOrigin => "Internal origin", CoordinateBase::ProjectBasePoint => "Project base point", CoordinateBase::SurveyPoint => "Survey point", CoordinateBase::SharedCoordinates => "Shared coordinates" }
}

impl GeorefInput {
    /// The line the file keeps; None while nothing has been entered.
    pub(crate) fn record(&self) -> Option<String> {
        if *self == Self::default() { return None; }
        Some(format!("U|kind=georef|crs={}|dat={}|vdat={}|proj={}|zone={}|e={:?}|n={:?}|h={:?}|sx={:?}|sy={:?}|el={:?}|base={}",
            esc(&self.crs_name), esc(&self.geodetic_datum), esc(&self.vertical_datum), esc(&self.map_projection), esc(&self.map_zone),
            self.shared[0], self.shared[1], self.shared[2], self.survey_xy[0], self.survey_xy[1], self.elevation_m, base_code(self.base)))
    }

    /// Reads the line back from the application's records; anything that does not read leaves the defaults.
    pub(crate) fn from_records(lines: &[String]) -> Self {
        let Some(line) = lines.iter().find(|l| l.starts_with("U|kind=georef|")) else { return Self::default() };
        let get = |k: &str| line.split('|').find_map(|f| f.strip_prefix(&format!("{k}=")));
        let num = |k: &str| get(k).and_then(|v| v.parse::<f64>().ok());
        (|| -> Option<Self> {
            Some(Self {
                crs_name: unesc(get("crs")?), geodetic_datum: unesc(get("dat")?), vertical_datum: unesc(get("vdat")?), map_projection: unesc(get("proj")?), map_zone: unesc(get("zone")?),
                shared: [num("e")?, num("n")?, num("h")?], survey_xy: [num("sx")?, num("sy")?], elevation_m: num("el")?, base: base_of(get("base")?)?,
            })
        })().unwrap_or_default()
    }

    /// What the export gets, when there is a site and a map name. Everything else the export needs is the site's.
    pub(crate) fn georef(&self, site: Option<openbim_document::Site>) -> Option<Georef> {
        let site = site?;
        if self.crs_name.trim().is_empty() { return None; }
        let opt = |t: &str| (!t.trim().is_empty()).then(|| t.trim().to_string());
        Some(Georef {
            crs: Crs { name: self.crs_name.trim().to_string(), geodetic_datum: opt(&self.geodetic_datum), vertical_datum: opt(&self.vertical_datum), map_projection: opt(&self.map_projection), map_zone: opt(&self.map_zone) },
            project_base_point: [0.0; 3], survey_point: [self.survey_xy[0], self.survey_xy[1], 0.0], survey_shared: self.shared,
            angle_to_true_north_deg: site.angle_to_true_north_deg, scale: 1.0, coordinate_base: self.base,
            latitude: site.latitude, longitude: site.longitude, elevation: self.elevation_m,
        })
    }
}

impl crate::Desktop {
    /// Puts the application's own lines into the document, so that the file and the export see them.
    pub(crate) fn sync_app_records(&mut self) {
        let records: Vec<String> = self.compute.record().into_iter().chain(self.georef.record()).collect();
        self.document.set_app_records(records);
    }

    pub(crate) fn georef_window(&mut self, ctx: &egui::Context) {
        if !self.show_georef { return; }
        let before = self.georef.clone();
        let mut open = true;
        let has_site = self.document.site().is_some();
        egui::Window::new("Georeferencing").open(&mut open).collapsible(false).default_width(420.0).show(ctx, |ui| {
            if let Some(s) = self.document.site() {
                ui.label(format!("Site: latitude {:.5}, longitude {:.5}, angle to true north {:.2} degrees (set with the 3D view's sun and site)", s.latitude, s.longitude, s.angle_to_true_north_deg));
            } else {
                ui.colored_label(egui::Color32::from_rgb(190, 90, 30), "The model has no site yet: set latitude and longitude first, then the map can be written.");
            }
            let g = &mut self.georef;
            egui::Grid::new("georef_grid").num_columns(2).show(ui, |ui| {
                ui.label("Map (name)"); ui.add(egui::TextEdit::singleline(&mut g.crs_name).hint_text("for example EPSG:32648").desired_width(220.0)); ui.end_row();
                ui.label("Geodetic datum"); ui.add(egui::TextEdit::singleline(&mut g.geodetic_datum).desired_width(220.0)); ui.end_row();
                ui.label("Vertical datum"); ui.add(egui::TextEdit::singleline(&mut g.vertical_datum).desired_width(220.0)); ui.end_row();
                ui.label("Map projection"); ui.add(egui::TextEdit::singleline(&mut g.map_projection).desired_width(220.0)); ui.end_row();
                ui.label("Map zone"); ui.add(egui::TextEdit::singleline(&mut g.map_zone).desired_width(220.0)); ui.end_row();
                ui.label("Survey point, easting (m)"); ui.add(egui::DragValue::new(&mut g.shared[0]).speed(1.0)); ui.end_row();
                ui.label("Survey point, northing (m)"); ui.add(egui::DragValue::new(&mut g.shared[1]).speed(1.0)); ui.end_row();
                ui.label("Survey point, height (m)"); ui.add(egui::DragValue::new(&mut g.shared[2]).speed(0.1)); ui.end_row();
                ui.label("Survey point in the model, x / y (m)"); ui.horizontal(|ui| { ui.add(egui::DragValue::new(&mut g.survey_xy[0]).speed(0.1)); ui.add(egui::DragValue::new(&mut g.survey_xy[1]).speed(0.1)); }); ui.end_row();
                ui.label("Site elevation (m)"); ui.add(egui::DragValue::new(&mut g.elevation_m).speed(0.1)); ui.end_row();
                ui.label("The file's origin is");
                egui::ComboBox::from_id_salt("georef_base").selected_text(base_name(g.base)).show_ui(ui, |ui| {
                    for b in [CoordinateBase::InternalOrigin, CoordinateBase::ProjectBasePoint, CoordinateBase::SurveyPoint, CoordinateBase::SharedCoordinates] { ui.selectable_value(&mut g.base, b, base_name(b)); }
                });
                ui.end_row();
            });
            if g.crs_name.trim().is_empty() { ui.small("Without a map name the file carries no map conversion."); }
            else if !has_site { ui.small("Needs the site above before it is written."); }
            else if let Some(e) = g.georef(self.document.site()).and_then(|x| x.check().err()) { ui.colored_label(egui::Color32::from_rgb(190, 60, 60), format!("Not usable yet: {e:?}")); }
        });
        if !open { self.show_georef = false; }
        if self.georef != before { self.sync_app_records(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_map_survives_the_file_and_needs_a_name_and_a_site() {
        let g = GeorefInput { crs_name: "EPSG:32648 | UTM 48N".into(), map_zone: "48N".into(), shared: [500_000.5, 1_281_000.25, 12.0], survey_xy: [3.0, -2.0], elevation_m: 11.0, base: CoordinateBase::SharedCoordinates, ..Default::default() };
        let line = g.record().unwrap();
        assert!(!line.contains('\n') && line.starts_with("U|kind=georef|"));
        assert_eq!(GeorefInput::from_records(&[line]), g);
        assert_eq!(GeorefInput::default().record(), None);
        assert_eq!(GeorefInput::from_records(&["U|kind=georef|crs=x".to_string()]), GeorefInput::default(), "a line that does not read gives the defaults");
        let site = openbim_document::Site { latitude: 11.5564, longitude: 104.9282, angle_to_true_north_deg: 20.0, utc_offset_hours: 7.0 };
        let made = g.georef(Some(site)).unwrap();
        assert_eq!((made.latitude, made.angle_to_true_north_deg, made.elevation), (11.5564, 20.0, 11.0));
        assert!(made.check().is_ok());
        assert!(g.georef(None).is_none(), "no site, no map");
        assert!(GeorefInput { crs_name: "  ".into(), ..g }.georef(Some(site)).is_none(), "no name, no map");
    }
}
