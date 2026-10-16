//! Compact Revit-style palettes around the existing sample document.
use crate::{ribbon, Desktop, SampleElement, View};
use eframe::egui::{self, RichText, Vec2};

fn title_bar(ui: &mut egui::Ui, title: &str, visible: &mut bool) {
    let fill = ui.visuals().widgets.inactive.bg_fill;
    egui::Frame::new().fill(fill).inner_margin(5.0).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).strong().size(12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("×").on_hover_text("Close palette").clicked() {
                    *visible = false;
                }
            });
        });
    });
}

fn palette_style(ui: &mut egui::Ui) {
    ui.style_mut().override_font_id = Some(egui::FontId::proportional(12.0));
    ui.spacing_mut().item_spacing = Vec2::new(3.0, 3.0);
    ui.spacing_mut().indent = 14.0;
    ui.spacing_mut().button_padding = Vec2::new(4.0, 2.0);
}

pub fn sidebar(app: &mut Desktop, ui: &mut egui::Ui) {
    palette_style(ui);
    if app.show_properties && app.show_browser {
        let height = ui.available_height();
        egui::Panel::top("properties_split")
            .default_size(height * 0.55)
            .min_size(90.0).max_size((height - 100.0).max(90.0))
            .resizable(true)
            .show(ui, |ui| properties(app, ui));
        egui::CentralPanel::default().frame(egui::Frame::new())
            .show(ui, |ui| browser(app, ui));
    } else if app.show_properties {
        properties(app, ui);
    } else if app.show_browser {
        browser(app, ui);
    }
}

fn matches(query: &str, text: &str) -> bool {
    text.to_lowercase().contains(query)
}

fn read_only_group(ui: &mut egui::Ui, name: &str, query: &str, rows: &[(&str, &str)]) {
    if !matches(query, name) && !rows.iter().any(|(key, value)|
        matches(query, key) || matches(query, value)) { return; }
    egui::CollapsingHeader::new(RichText::new(name).strong())
        .id_salt(("parameters", name)).default_open(true)
        .open(if query.is_empty() { None } else { Some(true) })
        .show(ui, |ui| {
            egui::Grid::new(("parameter_rows", name)).num_columns(2)
                .striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
                    for (key, value) in rows {
                        if matches(query, name) || matches(query, key) || matches(query, value) {
                            ui.add_sized([ui.available_width() * 0.46, 18.0], egui::Label::new(*key));
                            ui.label(RichText::new(*value).color(ui.visuals().weak_text_color()));
                            ui.end_row();
                        }
                    }
                });
        });
    ui.separator();
}

pub fn properties(app: &mut Desktop, ui: &mut egui::Ui) {
    title_bar(ui, "Properties", &mut app.show_properties);
    let duct_exists = app.document.get(crate::ElementId(1)).is_some();
    let (category, family_type, icon) = match app.selected {
        Some(SampleElement::Duct) if duct_exists => ("Ducts (1)", "Rectangular Duct", "duct"),
        Some(SampleElement::Ahu) => ("Mechanical Equipment (1)", "Sample AHU", "ahu"),
        Some(SampleElement::Room) => ("Rooms (1)", "Sample Room", "room"),
        _ => ("View Properties", "Current View", "view"),
    };
    ui.horizontal(|ui| {
        ui.add(ribbon::icon_image(ui, icon, app.dark, Vec2::splat(40.0)));
        ui.vertical(|ui| {
            ui.label(RichText::new(category).strong());
            // Only one sample type exists. The selector explicitly shows that
            // state rather than offering fictitious families or types.
            egui::ComboBox::from_id_salt("property_type").width(180.0)
                .selected_text(family_type).show_ui(ui, |ui| {
                    ui.label(family_type);
                    ui.weak("No additional types loaded");
                });
        });
    });
    ui.horizontal(|ui| {
        ui.weak("Instance parameters");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Edit Type").on_hover_text("Type editing is not connected yet").clicked() {
                app.message = "Type editing is not implemented in the sample document".into();
            }
        });
    });
    ui.add(egui::TextEdit::singleline(&mut app.property_query)
        .hint_text("Search parameters").desired_width(f32::INFINITY));
    ui.separator();
    let query = app.property_query.trim().to_lowercase();
    let editable = app.selected == Some(SampleElement::Duct) && duct_exists;
    let footer_height = if editable { 47.0 } else { 22.0 };
    egui::ScrollArea::vertical().id_salt("property_parameters")
        .max_height((ui.available_height() - footer_height).max(20.0))
        .auto_shrink([false, false]).show(ui, |ui| {
            ui.push_id(category, |ui| match app.selected {
                Some(SampleElement::Duct) if duct_exists => {
                    read_only_group(ui, "Constraints", &query, &[
                        ("Reference Level", "Level 1"), ("Justification", "Middle")]);
                    read_only_group(ui, "Mechanical", &query, &[
                        ("System Classification", "Supply Air"), ("System Name", "Supply Air 1 (sample)")]);
                    let names = ["Width (mm)", "Height (mm)", "Middle Elevation (mm)", "Slope (%)"];
                    if matches(&query, "Dimensions") || names.iter().any(|s| matches(&query, s)) {
                        egui::CollapsingHeader::new(RichText::new("Dimensions").strong())
                            .default_open(true).open(if query.is_empty() { None } else { Some(true) })
                            .show(ui, |ui| {
                                egui::Grid::new("duct_dimensions").num_columns(2).striped(true)
                                    .spacing([8.0, 4.0]).show(ui, |ui| {
                                        for (index, value) in [&mut app.width_mm, &mut app.height_mm,
                                            &mut app.elevation_mm, &mut app.slope_percent].into_iter().enumerate() {
                                            if !matches(&query, "Dimensions") && !matches(&query, names[index]) { continue; }
                                            ui.add_sized([ui.available_width() * 0.46, 18.0], egui::Label::new(names[index]));
                                            let editor = egui::DragValue::new(value).speed(if index == 3 { 0.1 } else { 1.0 });
                                            ui.add(if index < 2 { editor.range(1.0..=100_000.0) } else { editor });
                                            ui.end_row();
                                        }
                                    });
                            });
                        ui.separator();
                    }
                    read_only_group(ui, "Identity Data", &query, &[("Name", "Sample Supply Air Duct"), ("Element ID", "1")]);
                }
                Some(SampleElement::Ahu) => {
                    read_only_group(ui, "Mechanical", &query, &[("Category", "Mechanical Equipment"), ("System", "Supply Air (sample)")]);
                    read_only_group(ui, "Identity Data", &query, &[("Name", "Sample AHU"), ("Model status", "Illustration only")]);
                }
                Some(SampleElement::Room) => {
                    read_only_group(ui, "Identity Data", &query, &[("Name", "Sample Room"), ("Model status", "Illustration only")]);
                    read_only_group(ui, "Dimensions", &query, &[("Area", "Not calculated"), ("Volume", "Not calculated")]);
                }
                _ => {
                    let name = view_name(app.view);
                    read_only_group(ui, "Graphics", &query, &[("View Name", name), ("View Scale", "1:100 (sample)")]);
                    read_only_group(ui, "Identity Data", &query, &[("Model status", "Sample view")]);
                }
            });
        });
    ui.separator();
    if editable {
        let changed = app.document.get(crate::ElementId(1)).is_some_and(|duct|
            (duct.width_m * 1000.0 - app.width_mm).abs() > 1e-6 ||
            (duct.height_m * 1000.0 - app.height_mm).abs() > 1e-6 ||
            (duct.elevation_m * 1000.0 - app.elevation_mm).abs() > 1e-6 ||
            (duct.slope_ratio.unwrap_or(0.0) * 100.0 - app.slope_percent).abs() > 1e-6);
        ui.horizontal(|ui| {
            if ui.add_enabled(changed, egui::Button::new("Apply")).clicked() { app.apply_duct_edit(); }
            if ui.add_enabled(changed, egui::Button::new("Reset")).clicked() { app.sync_duct_controls(); }
            if changed { ui.weak("Pending changes"); }
        });
    }
    ui.label(RichText::new("Sample / in memory").small().weak());
}

pub fn view_name(view: View) -> &'static str {
    match view { View::Plan => "Level 1", View::ThreeD => "{3D}", View::Mechanical => "M-101 - Mechanical" }
}

struct Node {
    name: &'static str,
    view: Option<View>,
    children: &'static [Node],
}
const fn folder(name: &'static str, children: &'static [Node]) -> Node { Node { name, view: None, children } }
const fn view(name: &'static str, view: View) -> Node { Node { name, view: Some(view), children: &[] } }
const TREE: &[Node] = &[
    folder("Views (all)", &[
        folder("Floor Plans", &[view("Level 1", View::Plan), view("M-101 - Mechanical", View::Mechanical)]),
        folder("Ceiling Plans", &[]),
        folder("3D Views", &[view("{3D}", View::ThreeD)]),
        folder("Elevations", &[]), folder("Sections", &[]), folder("Legends", &[]),
    ]),
    folder("Schedules/Quantities", &[]), folder("Sheets (all)", &[]),
    folder("Families", &[]), folder("Groups", &[]), folder("Model Links", &[]),
];

fn node_matches(node: &Node, query: &str) -> bool {
    matches(query, node.name) || node.children.iter().any(|child| node_matches(child, query))
}

fn tree_node(app: &mut Desktop, ui: &mut egui::Ui, node: &Node, parent: &str, query: &str) {
    if !node_matches(node, query) { return; }
    let path = format!("{parent}/{}", node.name);
    if let Some(view) = node.view {
        let active = app.view == view;
        let color = if active { ui.visuals().hyperlink_color } else { ui.visuals().text_color() };
        let label = RichText::new(node.name).color(color);
        let label = if active { label.strong() } else { label };
        let response = ui.selectable_label(app.browser_selected == path, label)
            .on_hover_text("Double-click to open view");
        if response.clicked() { app.browser_selected = path; }
        if response.double_clicked() {
            app.view = view;
            app.selected = None;
            app.message = format!("Opened sample view: {}", node.name);
        }
    } else {
        egui::CollapsingHeader::new(RichText::new(node.name).strong())
            .id_salt(&path).default_open(node.name == "Views (all)" || !node.children.is_empty())
            .open(if query.is_empty() { None } else { Some(true) })
            .show(ui, |ui| {
                if node.children.is_empty() {
                    ui.label(RichText::new("No items in sample").weak().italics());
                } else {
                    // A matching parent reveals its entire subtree.
                    let child_query = if matches(query, node.name) { "" } else { query };
                    for child in node.children { tree_node(app, ui, child, &path, child_query); }
                }
            });
    }
}

pub fn browser(app: &mut Desktop, ui: &mut egui::Ui) {
    title_bar(ui, "Project Browser — Project1", &mut app.show_browser);
    ui.add(egui::TextEdit::singleline(&mut app.browser_query)
        .hint_text("Search views and categories").desired_width(f32::INFINITY));
    ui.separator();
    let query = app.browser_query.trim().to_lowercase();
    egui::ScrollArea::vertical().id_salt("project_tree").auto_shrink([false, false])
        .show(ui, |ui| {
            for node in TREE { tree_node(app, ui, node, "project", &query); }
            if !TREE.iter().any(|node| node_matches(node, &query)) { ui.weak("No matching items"); }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_search_keeps_ancestors_of_matching_views() {
        assert!(node_matches(&TREE[0], "m-101"));
        assert!(node_matches(&TREE[0], "{3d}"));
        assert!(!node_matches(&TREE[1], "m-101"));
        assert!(!TREE.iter().any(|node| node_matches(node, "no such view")));
    }

    #[test]
    fn selection_edit_targets_duct_and_is_undoable() {
        let mut app = Desktop::sample();
        let initial = app.document.revision();
        app.selected = Some(SampleElement::Ahu);
        app.width_mm = 600.0;
        app.apply_duct_edit();
        assert_eq!(app.document.revision(), initial);
        app.selected = Some(SampleElement::Duct);
        app.slope_percent = 2.0;
        app.apply_duct_edit();
        assert_eq!(app.document.get(crate::ElementId(1)).unwrap().width_m, 0.6);
        assert_eq!(app.document.get(crate::ElementId(1)).unwrap().slope_ratio, Some(0.02));
        app.undo_document();
        assert_eq!(app.width_mm, 200.0);
        assert_eq!(app.slope_percent, 0.0);
        assert_eq!(app.document.get(crate::ElementId(1)).unwrap().width_m, 0.2);
    }

    #[test]
    fn palettes_render_each_selection_and_single_palette_layout() {
        let ctx = egui::Context::default();
        let mut app = Desktop::sample();
        let revision = app.document.revision();
        for selection in [None, Some(SampleElement::Duct), Some(SampleElement::Ahu), Some(SampleElement::Room)] {
            app.selected = selection;
            for (properties, browser) in [(true, true), (true, false), (false, true)] {
                app.show_properties = properties;
                app.show_browser = browser;
                for dark in [false, true] {
                    app.dark = dark;
                    ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
                    let mut output = ctx.run_ui(egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(900.0, 550.0))),
                        ..Default::default()
                    }, |ui| {
                        egui::Panel::left("test_side").default_size(310.0)
                            .show(ui, |ui| sidebar(&mut app, ui));
                    });
                    assert!(!output.shapes.is_empty());
                    output.textures_delta.clear();
                }
            }
        }
        assert_eq!(app.document.revision(), revision, "rendering must not edit the model");
    }
}
