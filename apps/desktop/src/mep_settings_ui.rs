//! Manage > MEP Settings > Mechanical Settings: the lists the Duct and Pipe tools offer (pipe slopes, pipe diameters, duct
//! widths and heights). The lists are kept with the model, so they are saved, exported and read back with it.
use crate::Desktop;
use eframe::egui;
use openbim_document::MepSettings;

/// The window's state: the lists being edited (they replace the model's at OK), the list shown and the value typed to add.
pub(crate) struct MepSettingsDialog { pub draft: MepSettings, pub tab: usize, pub new_value: f64, pub new_text: String }

/// The four lists: name, unit, and the list in the settings.
/// The first four are numbers; the last two are names (the types of insulation and of duct lining).
const LISTS: [(&str, &str); 6] = [("Pipe Settings: Slopes", "%"), ("Pipe Settings: Sizes (diameters)", "mm"), ("Duct Settings: Widths", "mm"), ("Duct Settings: Heights", "mm"), ("Insulation Types", ""), ("Duct Lining Types", "")];
const NAME_TABS: usize = 4;

fn names_of(s: &mut MepSettings, tab: usize) -> &mut Vec<String> { if tab == NAME_TABS { &mut s.insulation_types } else { &mut s.lining_types } }

/// New, for a name: it joins the list unless it is empty or already there.
fn add_name(draft: &mut MepSettings, tab: usize, name: &str) {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 { return; }
    let list = names_of(draft, tab);
    if !list.iter().any(|n| n == name) { list.push(name.to_string()); }
}

fn list_of(s: &mut MepSettings, tab: usize) -> &mut Vec<f64> {
    match tab { 0 => &mut s.pipe_slopes_pct, 1 => &mut s.pipe_diameters_mm, 2 => &mut s.duct_widths_mm, _ => &mut s.duct_heights_mm }
}

/// New: the value joins the list, which is then tidied (a value out of range is dropped, the list is sorted, each value once).
fn add_to_list(draft: &mut MepSettings, tab: usize, value: f64) {
    list_of(draft, tab).push(value);
    let mut cleaned = draft.cleaned();
    let tidy = list_of(&mut cleaned, tab).clone();
    *list_of(draft, tab) = tidy;
}

impl Desktop {
    /// Opens the window with the model's lists.
    pub(crate) fn open_mep_settings(&mut self) {
        self.mep_settings_dialog = Some(MepSettingsDialog { draft: self.document.mep().settings.clone(), tab: 0, new_value: 1.0, new_text: String::new() });
    }

    /// OK: the lists are cleaned and replace the model's in one undoable step.
    pub(crate) fn apply_mep_settings(&mut self, draft: &MepSettings) {
        match self.document.set_mep_settings(draft) {
            Ok(true) => self.message = "Mechanical Settings changed; undo puts the old lists back".into(),
            Ok(false) => self.message = "Mechanical Settings: no change".into(),
            Err(e) => self.message = format!("Mechanical Settings rejected ({e:?})"),
        }
    }

    pub(crate) fn mep_settings_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.mep_settings_dialog.take() else { return };
        let (mut open, mut ok, mut cancel) = (true, false, false);
        egui::Window::new("Mechanical Settings").open(&mut open).default_width(420.0).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (i, (name, _)) in LISTS.iter().enumerate() { if ui.selectable_label(d.tab == i, *name).clicked() { d.tab = i; d.new_value = if i == 0 { 1.0 } else { 100.0 }; } }
            });
            ui.separator();
            let unit = LISTS[d.tab].1;
            let tab = d.tab;
            let mut remove = None;
            if tab >= NAME_TABS {
                egui::ScrollArea::vertical().max_height(220.0).id_salt("mep_settings_names").show(ui, |ui| {
                    let list = names_of(&mut d.draft, tab);
                    if list.is_empty() { ui.weak("The list is empty: nothing can be added until a type is made."); }
                    for (i, n) in list.iter().enumerate() {
                        ui.horizontal(|ui| { ui.label(n); if ui.small_button("Delete").clicked() { remove = Some(i); } });
                    }
                });
                if let Some(i) = remove { names_of(&mut d.draft, tab).remove(i); }
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut d.new_text).desired_width(180.0).hint_text("Type name"));
                    if ui.button("New").on_hover_text("Add the type to the list").clicked() {
                        let t = std::mem::take(&mut d.new_text);
                        add_name(&mut d.draft, tab, &t);
                    }
                });
                ui.small("A type already used on a duct or pipe stays on it when it is deleted here.");
            } else {
            egui::ScrollArea::vertical().max_height(220.0).id_salt("mep_settings_list").show(ui, |ui| {
                let list = list_of(&mut d.draft, tab);
                if list.is_empty() { ui.weak("The list is empty: the tools offer no values, but any value can still be typed."); }
                for (i, v) in list.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{v} {unit}"));
                        if ui.small_button("Delete").clicked() { remove = Some(i); }
                    });
                }
            });
            if let Some(i) = remove { list_of(&mut d.draft, tab).remove(i); }
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut d.new_value).speed(if tab == 0 { 0.05 } else { 1.0 }).suffix(format!(" {unit}")));
                if ui.button("New").on_hover_text("Add the value to the list").clicked() {
                    let v = d.new_value;
                    add_to_list(&mut d.draft, tab, v);
                }
            });
            ui.small("Values outside the range (slopes 0.01 to 100 %, sizes 10 to 3000 mm) are dropped; the lists are sorted when OK is pressed.");
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() { ok = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
                if ui.button("Defaults").on_hover_text("Put back the standard lists in this window").clicked() { d.draft = MepSettings::default(); }
            });
        });
        if ok { let draft = d.draft.clone(); self.apply_mep_settings(&draft); }
        if open && !ok && !cancel { self.mep_settings_dialog = Some(d); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_value_joins_its_list_sorted_once_and_a_bad_one_is_dropped() {
        let mut s = MepSettings::default();
        add_to_list(&mut s, 0, 0.75);
        add_to_list(&mut s, 0, 0.75);
        add_to_list(&mut s, 0, 500.0);
        add_to_list(&mut s, 0, f64::NAN);
        assert_eq!(s.pipe_slopes_pct, vec![0.2, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 10.0]);
        add_to_list(&mut s, 2, 450.0);
        assert!(s.duct_widths_mm.contains(&450.0) && s.duct_widths_mm.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(s.pipe_diameters_mm, MepSettings::default().pipe_diameters_mm, "the other lists are untouched");
    }

    #[test]
    fn a_new_type_name_joins_its_list_once_and_an_empty_or_long_one_does_not() {
        let mut s = MepSettings::default();
        add_name(&mut s, NAME_TABS, "  Rockwool ");
        add_name(&mut s, NAME_TABS, "Rockwool");
        add_name(&mut s, NAME_TABS, "   ");
        add_name(&mut s, NAME_TABS, &"x".repeat(61));
        add_name(&mut s, NAME_TABS + 1, "Acoustic liner");
        assert_eq!(s.insulation_types.last().map(String::as_str), Some("Rockwool"));
        assert_eq!(s.insulation_types.len(), MepSettings::default().insulation_types.len() + 1);
        assert_eq!(s.lining_types.last().map(String::as_str), Some("Acoustic liner"));
    }

    #[test]
    fn the_dialog_changes_the_lists_in_one_undoable_step_and_the_tools_offer_them() {
        let mut app = Desktop::sample();
        let before = app.document.mep().settings.clone();
        app.run_command("mep_settings", "MEP Settings");
        let d = app.mep_settings_dialog.as_mut().expect("the window opens");
        assert_eq!(d.draft, before);
        d.draft.pipe_slopes_pct = vec![3.0, 0.5];
        d.draft.pipe_diameters_mm = vec![110.0];
        let draft = d.draft.clone();
        app.apply_mep_settings(&draft);
        assert_eq!(app.document.mep().settings.pipe_slopes_pct, vec![0.5, 3.0], "{}", app.message);
        assert_eq!(app.document.mep().settings.pipe_diameters_mm, vec![110.0]);
        app.apply_mep_settings(&draft);
        assert!(app.message.contains("no change"), "{}", app.message);
        app.undo_document();
        assert_eq!(app.document.mep().settings, before);
        app.redo_document();
        assert_eq!(app.document.mep().settings.pipe_slopes_pct, vec![0.5, 3.0]);
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use crate::Tool;

    /// The text of everything one frame drew.
    fn drawn(app: &mut Desktop, ctx: &egui::Context) -> String {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1400.0, 800.0));
        let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
            egui::CentralPanel::default().show(ui, |ui| { crate::panels::options(app, ui); });
            app.mep_settings_window(ui.ctx());
        });
        out.textures_delta.clear();
        let mut text = String::new();
        fn walk(s: &egui::Shape, text: &mut String) {
            match s { egui::Shape::Text(t) => { text.push_str(t.galley.text()); text.push('\n'); } egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, text)), _ => {} }
        }
        for c in &out.shapes { walk(&c.shape, &mut text); }
        text
    }

    #[test]
    fn the_options_bar_and_the_window_show_the_lists_of_the_model() {
        let ctx = egui::Context::default();
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Pipe);
        app.run_opts.slope = crate::mep_tool::SlopeMode::Down;
        let text = drawn(&mut app, &ctx);
        for want in ["Place Pipe", "Diameter (mm)", "Automatically Connect", "Inherit Elevation", "Inherit Size", "Tag on Placement", "Horizontal Justification", "Vertical Justification", "Slope", "Show Slope Tooltip", "▾"] {
            assert!(text.contains(want), "the options bar shows '{want}':\n{text}");
        }
        // a duct has no slope, and has a height field with its own list
        app.choose_tool(Tool::Duct);
        let text = drawn(&mut app, &ctx);
        assert!(text.contains("Place Duct") && text.contains("Height (mm)") && !text.contains("Show Slope Tooltip"), "{text}");
        // the window lists the values of the model, and a new list shows
        app.open_mep_settings();
        app.mep_settings_dialog.as_mut().unwrap().draft.pipe_slopes_pct = vec![0.75, 3.5];
        let _ = drawn(&mut app, &ctx); // a new window is laid out on its first frame and shown on the next
        let text = drawn(&mut app, &ctx);
        assert!(text.contains("Mechanical Settings") && text.contains("0.75 %") && text.contains("3.5 %") && text.contains("Pipe Settings: Slopes"), "{text}");
        assert!(!text.contains("0.25 %"));
    }

    #[test]
    fn the_lists_go_out_in_the_ifc_file_and_come_back() {
        let mut app = Desktop::sample();
        let mut s = app.document.mep().settings.clone();
        s.pipe_slopes_pct = vec![0.8, 2.5];
        s.duct_widths_mm = vec![125.0, 315.0];
        app.apply_mep_settings(&s);
        let out = crate::export::export_ifc_as(&app.document, "P", 1, "2026-10-19T00:00:00", crate::export::Schema::Ifc4).unwrap();
        let mut back = openbim_document::Document::new();
        crate::import::import_ifc(&mut back, &out.text).unwrap();
        assert_eq!(back.mep().settings, app.document.mep().settings);
        // the saved file keeps them too
        let lines = openbim_document::native::records(&app.document);
        assert_eq!(openbim_document::native::parse_mep(&lines).unwrap().settings, app.document.mep().settings);
    }
}
