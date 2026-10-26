//! Compact Revit-style palettes around the existing sample document.
use crate::{ribbon, Desktop, ElementId, View};
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

#[derive(Clone, Copy)]
enum Field { Count, Spacing, Angle, Thickness, Width, Height, Diameter, Web, Flange, Elevation, Slope, X1, Y1, X2, Y2 }

fn dimensions(category: openbim_document::Category, section: u8) -> &'static [Field] {
    use openbim_document::Category as C;
    match category {
        C::Slab => &[Field::Width, Field::Elevation],
        C::Stair | C::Opening => &[Field::Width, Field::Height, Field::Elevation],
        C::Section => &[Field::Width],
        C::Elevation => &[Field::Width, Field::Height],
        C::Family => &[Field::Angle, Field::Elevation],
        C::Rebar => &[Field::Count, Field::Width, Field::Height, Field::Elevation, Field::Spacing],
        C::Truss => &[Field::Spacing, Field::Width, Field::Height, Field::Elevation],
        C::Railing => &[Field::Height, Field::Width, Field::Spacing, Field::Elevation],
        C::BeamSystem => &[Field::Spacing, Field::Angle, Field::Width, Field::Height, Field::Elevation],
        C::Footing => &[Field::Width, Field::Height, Field::Thickness, Field::Elevation],
        C::WallFooting => &[Field::Width, Field::Height, Field::Elevation],
        C::Wall => &[Field::Width, Field::Height, Field::Elevation],
        C::Column => match section {
            1 => &[Field::Diameter, Field::Elevation],
            2 => &[Field::Width, Field::Height, Field::Web, Field::Flange, Field::Elevation],
            _ => &[Field::Width, Field::Height, Field::Elevation],
        },
        C::Beam => match section {
            1 => &[Field::Diameter, Field::Elevation, Field::Slope],
            2 => &[Field::Width, Field::Height, Field::Web, Field::Flange, Field::Elevation, Field::Slope],
            _ => &[Field::Width, Field::Height, Field::Elevation, Field::Slope],
        },
        C::Duct => &[Field::Width, Field::Height, Field::Elevation, Field::Slope],
        C::Pipe => &[Field::Width, Field::Elevation, Field::Slope],
        C::Level => &[Field::Elevation], _ => &[],
    }
}

/// The usage and the material of a beam.
fn usage_material_combos(app: &mut Desktop, ui: &mut egui::Ui, usage: bool) {
    ui.horizontal(|ui| {
        if usage {
            ui.label("Usage");
            egui::ComboBox::from_id_salt("beam_usage").selected_text(app.usage_pick.name()).show_ui(ui, |ui| {
                for u in openbim_document::Usage::ALL { ui.selectable_value(&mut app.usage_pick, u, u.name()); }
            });
        }
        ui.label("Material");
        egui::ComboBox::from_id_salt("beam_material").selected_text(app.material_pick.name()).show_ui(ui, |ui| {
            for m in openbim_document::Material::ALL { ui.selectable_value(&mut app.material_pick, m, m.name()); }
        });
    });
}

/// The section of a beam: rectangle, circle or I-shape.
fn section_combo(app: &mut Desktop, ui: &mut egui::Ui) {
    let names = ["Rectangle", "Circle", "I-shape"];
    ui.horizontal(|ui| {
        ui.label("Section");
        egui::ComboBox::from_id_salt("beam_section").selected_text(names[usize::from(app.section_pick.min(2))]).show_ui(ui, |ui| {
            for (i, n) in names.iter().enumerate() { ui.selectable_value(&mut app.section_pick, i as u8, *n); }
        });
    });
}

fn field_name(field: Field, category: openbim_document::Category) -> &'static str {
    use openbim_document::Category as C;
    match field {
        Field::Thickness => "Thickness (mm)",
        Field::Count => "Bottom Bars",
        Field::Angle if category == C::Family => "Rotation (deg)",
        Field::Elevation if category == C::Family => "Offset (mm)",
        Field::Width if category == C::Rebar => "Bar Diameter (mm)",
        Field::Height if category == C::Rebar => "Stirrup Diameter (mm)",
        Field::Elevation if category == C::Rebar => "Cover (mm)",
        Field::Spacing if category == C::Rebar => "Stirrup Spacing (mm)",
        Field::Spacing if category == C::Truss => "Panel Length (mm)",
        Field::Height if category == C::Railing => "Railing Height (mm)",
        Field::Width if category == C::Railing => "Rail and Post Size (mm)",
        Field::Spacing if category == C::Railing => "Baluster Spacing (mm)",
        Field::Elevation if category == C::Railing => "Base Offset (mm)",
        Field::Width if category == C::Truss => "Member Size (mm)",
        Field::Height if category == C::Truss => "Truss Depth (mm)",
        Field::Elevation if category == C::Truss => "Bottom Chord Elevation (mm)",
        Field::Spacing => "Spacing (mm)", Field::Angle => "Angle (deg)",
        Field::Height if category == C::BeamSystem => "Beam Depth (mm)",
        Field::Width if category == C::BeamSystem => "Beam Width (mm)",
        Field::Elevation if category == C::BeamSystem => "Beam Centre Elevation (mm)",
        Field::Height if category == C::Footing => "Length (mm)",
        Field::Height if category == C::WallFooting => "Depth (mm)",
        Field::Elevation if category == C::Footing || category == C::WallFooting => "Top Offset (mm)",
        Field::Width if category == C::Section => "View Depth (mm)",
        Field::Width if category == C::Elevation => "View Width (mm)",
        Field::Height if category == C::Elevation => "View Depth (mm)",
        Field::Width if category == C::Stair || category == C::Opening => "Width (mm)",
        Field::Height if category == C::Opening => "Height (mm)",
        Field::Elevation if category == C::Opening => "Sill Height (mm)",
        Field::Height if category == C::Stair => "Total Rise (mm)",
        Field::Elevation if category == C::Stair => "Base Offset (mm)",
        Field::Width if category == C::Wall || category == C::Slab => "Thickness (mm)",
        Field::Elevation if category == C::Slab => "Top Offset (mm)",
        Field::Width if category == C::Pipe => "Diameter (mm)",
        Field::Height if category == C::Beam => "Depth (mm)",
        Field::Width if category == C::Beam => "Width (mm)",
        Field::Diameter => "Diameter (mm)", Field::Web => "Web Thickness (mm)", Field::Flange => "Flange Thickness (mm)",
        Field::Elevation if category == C::Beam => "Centre Elevation (mm)",
        Field::Elevation if category == C::Column => "Base Offset (mm)",
        Field::Width => "Width (mm)", Field::Height => "Height (mm)",
        Field::Elevation if category == C::Level => "Elevation (mm)",
        Field::Elevation if category == C::Wall => "Base Offset (mm)",
        Field::Elevation => "Middle Elevation (mm)", Field::Slope => "Slope (%)",
        Field::X1 => "Start X (mm)", Field::Y1 => "Start Y (mm)",
        Field::X2 => "End X (mm)", Field::Y2 => "End Y (mm)",
    }
}

fn editor(app: &mut Desktop, ui: &mut egui::Ui, field: Field) {
    let value = match field {
        Field::Count => &mut app.bar_count, Field::Spacing => &mut app.spacing_mm, Field::Angle => &mut app.angle_deg,
        Field::Thickness => &mut app.footing_thickness_mm, Field::Width | Field::Diameter => &mut app.width_mm, Field::Web => &mut app.web_mm, Field::Flange => &mut app.flange_mm, Field::Height => &mut app.height_mm,
        Field::Elevation => &mut app.elevation_mm, Field::Slope => &mut app.slope_percent,
        Field::X1 => &mut app.axis_mm[0], Field::Y1 => &mut app.axis_mm[1],
        Field::X2 => &mut app.axis_mm[2], Field::Y2 => &mut app.axis_mm[3],
    };
    let editor = egui::DragValue::new(value).speed(if matches!(field, Field::Slope) { 0.1 } else { 1.0 });
    ui.add(if matches!(field, Field::Width | Field::Height | Field::Diameter | Field::Web | Field::Flange | Field::Thickness) { editor.range(1.0..=100_000.0) } else { editor });
}

fn edit_group(app: &mut Desktop, ui: &mut egui::Ui, name: &str, query: &str,
    category: openbim_document::Category, fields: &[Field]) {
    if fields.is_empty() || (!matches(query, name) && !fields.iter().any(|f| matches(query, field_name(*f, category)))) { return; }
    egui::CollapsingHeader::new(RichText::new(name).strong()).id_salt(("edit", name))
        .default_open(true).open(if query.is_empty() { None } else { Some(true) }).show(ui, |ui| {
            egui::Grid::new(("editable_rows", name)).num_columns(2).striped(true)
                .spacing([8.0, 4.0]).show(ui, |ui| {
                    for field in fields {
                        let label = field_name(*field, category);
                        if !matches(query, name) && !matches(query, label) { continue; }
                        ui.add_sized([ui.available_width() * 0.46, 18.0], egui::Label::new(label));
                        editor(app, ui, *field);
                        ui.end_row();
                    }
                });
        });
    ui.separator();
}

fn category_info(category: openbim_document::Category) -> (&'static str, &'static str, &'static str) {
    use openbim_document::Category as C;
    match category {
        C::Beam => ("Structural Framing (1)", "Rectangular Beam", "beam"),
        C::Column => ("Structural Columns (1)", "Rectangular Column", "structural_column"),
        C::Footing => ("Structural Foundations (1)", "Isolated Footing", "foundation"),
        C::WallFooting => ("Structural Foundations (1)", "Strip Footing", "footing"),
        C::Slab => ("Floors (1)", "Generic Slab", "structural_floor"),
        C::Family => ("Specialty Equipment (1)", "Family Instance", "component"),
        C::Rebar => ("Structural Rebar (1)", "Beam Rebar", "rebar"),
        C::Truss => ("Structural Trusses (1)", "Warren Truss", "truss"), C::Railing => ("Railings (1)", "Railing", "railing"),
        C::BeamSystem => ("Structural Framing (1)", "Beam System", "beam"),
        C::Wall => ("Walls (1)", "Basic Wall", "wall"), C::Duct => ("Ducts (1)", "Rectangular Duct", "duct"),
        C::Pipe => ("Pipes (1)", "Round Pipe", "pipe"), C::Level => ("Levels (1)", "Level Datum", "level"),
        C::Type => ("Types", "Type", "component"), C::Tag => ("Tags (1)", "Tag", "tag"), C::Dimension => ("Dimensions (1)", "Aligned Dimension", "dimension"), C::Room => ("Rooms (1)", "Room", "room"), C::Stair => ("Stairs (1)", "Stair", "stair"), C::Opening => ("Openings (1)", "Opening", "opening"), C::Section => ("Views (1)", "Section", "section"), C::Elevation => ("Views (1)", "Elevation", "elevation"),
        C::Generic => ("Generic Models (1)", "Generic Model", "component"), C::Grid => ("Grids (1)", "Straight Grid", "grid"), C::Equipment => ("Equipment (1)", "Equipment", "ahu"),
    }
}

/// The type of the selected element: none, or one of the types of its kind.
fn type_combo(app: &mut Desktop, ui: &mut egui::Ui, el: &openbim_document::Element) {
    let types: Vec<(openbim_document::ElementId, String, usize)> = app.document.types_for(el.category).iter().map(|t| (t.id, t.name.clone(), app.document.instances_of(t.id).len())).collect();
    let current = el.type_id.and_then(|t| types.iter().find(|x| x.0 == t)).map_or("— no type —".to_string(), |t| t.1.clone());
    let mut choice = None;
    egui::ComboBox::from_id_salt("property_type").width(180.0).selected_text(current).show_ui(ui, |ui| {
        if ui.selectable_label(el.type_id.is_none(), "— no type —").clicked() { choice = Some(None); }
        for (id, name, n) in &types {
            if ui.selectable_label(el.type_id == Some(*id), format!("{name}  ({n})")).clicked() { choice = Some(Some(*id)); }
        }
    });
    if let Some(c) = choice { if c != el.type_id { app.choose_type(c); } }
}

/// A name and a button that make a type of the selected element's values, and what editing does when it has a type.
fn type_maker(app: &mut Desktop, ui: &mut egui::Ui, el: &openbim_document::Element) {
    if let Some(t) = el.type_id {
        let n = app.document.instances_of(t).len();
        ui.weak(format!("Edits to its size, section, material or bars change the type and its {n} element{}.", if n == 1 { "" } else { "s" }));
    }
    if let (Some(t), openbim_document::Category::Wall) = (el.type_id, el.category) {
        if ui.small_button("Edit Type…").on_hover_text("Name, height and layers of the wall type").clicked() { app.open_type_editor(t); }
    }
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.type_name_input).hint_text("New type name").desired_width(120.0));
        if ui.add_enabled(!app.type_name_input.trim().is_empty(), egui::Button::new("Make type").small()).clicked() { app.make_type(); }
    });
}

/// The family to place and its type, from the library.
fn family_combos(app: &mut Desktop, ui: &mut egui::Ui) {
    let lib = crate::export::library();
    // while a door/window/component command is placing, offer only that kind of family, as Revit's Type Selector does
    let lib: Vec<_> = lib.iter().filter(|f| app.family_kind.is_none_or(|k| f.category() == k)).collect();
    ui.label("Family");
    egui::ComboBox::from_id_salt("family_pick").selected_text(app.family_pick.clone()).show_ui(ui, |ui| {
        for f in lib { if ui.selectable_value(&mut app.family_pick, f.name().to_string(), f.name()).clicked() { app.family_type.clear(); } }
    });
    family_type_combo(app, ui, &app.family_pick.clone());
}

/// The type of the family `family` (the family's own values when none is chosen).
fn family_type_combo(app: &mut Desktop, ui: &mut egui::Ui, family: &str) {
    let Some(f) = crate::export::library().iter().find(|f| f.name() == family) else { return };
    let names = f.type_names();
    if names.is_empty() { return; }
    ui.label("Type");
    let combo = egui::ComboBox::from_id_salt("family_type").selected_text(if app.family_type.is_empty() { "Default" } else { app.family_type.as_str() }).show_ui(ui, |ui| {
        ui.selectable_value(&mut app.family_type, String::new(), "Default");
        for n in names {
            let r = ui.selectable_value(&mut app.family_type, n.to_string(), n);
            test_hit!(app, r, "family_type:{n}");
        }
    });
    test_hit!(app, combo.response, "family_type");
}

/// The kind and the material of a slab.
/// The slope of a selected roof, and which edges of its outline define it (Revit's "Defines Slope" of each line).
fn roof_group(app: &mut Desktop, ui: &mut egui::Ui, el: &openbim_document::Element) {
    ui.separator();
    ui.strong("Roof");
    let mut degrees = el.slope_ratio.unwrap_or(0.0).atan().to_degrees();
    let mut edges = el.slope_edges.clone();
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Slope (°)");
        changed |= ui.add(egui::DragValue::new(&mut degrees).speed(0.5).range(0.0..=78.0)).changed();
    });
    ui.label("Defines slope:");
    ui.horizontal_wrapped(|ui| {
        for (i, e) in edges.iter_mut().enumerate() { changed |= ui.checkbox(e, format!("{}", i + 1)).changed(); }
    });
    if changed {
        let ops = [openbim_document::Command::SetRoof { id: el.id, slope_ratio: degrees.to_radians().tan(), edges }];
        match app.document.transact(app.document.revision(), &ops) {
            Ok(_) => app.message = "Roof changed; undo takes it back".into(),
            Err(_) => app.message = "On an outline with a reflex corner (an L, a T) every edge has to slope; a gable end there is not made yet".into(),
        }
    }
    if let Some(f) = app.document.roof_figures(el.id) {
        let (s, p, r) = (format!("{:.2}", f.surface_m2), format!("{:.2}", f.plan_m2), format!("{:.3}", f.rise_m));
        read_only_group(ui, "Derived Geometry", "", &[("Surface Area (m²)", &s), ("Plan Area (m²)", &p), ("Rise above Eaves (m)", &r)]);
    }
}

/// A selected shaft: its base and top levels and offsets (Revit's Base Constraint, Base Offset, Top Constraint, Top Offset).
fn shaft_group(app: &mut Desktop, ui: &mut egui::Ui, el: &openbim_document::Element) {
    let Some(openbim_document::Geometry::Shaft { level, top, base_offset_m, top_offset_m }) = el.geometry else { return };
    ui.separator();
    ui.strong("Constraints");
    let name = |id| app.document.get(id).map_or("Missing level".to_string(), |l| l.name.clone());
    let (base_name, top_name) = (name(level), name(top));
    let (mut base, mut up) = (base_offset_m * 1000.0, top_offset_m * 1000.0);
    let mut changed = false;
    ui.label(format!("Base Constraint: {base_name}"));
    ui.horizontal(|ui| { ui.label("Base Offset (mm)"); changed |= ui.add(egui::DragValue::new(&mut base).speed(5.0)).changed(); });
    ui.label(format!("Top Constraint: {top_name}"));
    ui.horizontal(|ui| { ui.label("Top Offset (mm)"); changed |= ui.add(egui::DragValue::new(&mut up).speed(5.0)).changed(); });
    if changed {
        let ops = [openbim_document::Command::SetGeometry { id: el.id, geometry: openbim_document::Geometry::Shaft { level, top, base_offset_m: base / 1000.0, top_offset_m: up / 1000.0 } }];
        match app.document.transact(app.document.revision(), &ops) {
            Ok(_) => app.message = "Shaft changed; undo takes it back".into(),
            Err(_) => app.message = "The top of a shaft must be above its base".into(),
        }
    }
    let cut = app.document.elements().filter(|e| e.category == openbim_document::Category::Slab && app.document.shafts_through(e.id).contains(&el.id)).count().to_string();
    let area = format!("{:.3}", openbim_geom::profile::polygon_area(&el.outline).abs());
    read_only_group(ui, "Derived Geometry", "", &[("Area (m²)", &area), ("Floors and Ceilings Cut", &cut)]);
}

fn slab_combos(app: &mut Desktop, ui: &mut egui::Ui) {
    ui.label("Kind");
    egui::ComboBox::from_id_salt("slab_kind").selected_text(app.slab_kind_pick.name()).show_ui(ui, |ui| {
        for k in openbim_document::SlabKind::ALL { ui.selectable_value(&mut app.slab_kind_pick, k, k.name()); }
    });
    usage_material_combos(app, ui, false);
}

pub fn options(app: &mut Desktop, ui: &mut egui::Ui) {
    if app.tool == crate::Tool::Tag {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Tag by Category");
            ui.separator();
            ui.checkbox(&mut app.tag_leader, "Leader");
            ui.weak("click an element in a plan, elevation or section view; the tag says its type, or its name when it has none, and stays in that view");
        });
        return;
    }
    if app.tool == crate::Tool::Level {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Level");
            ui.separator();
            ui.label("Elevation (mm)");
            let r = ui.add(egui::DragValue::new(&mut app.level_elev_mm).speed(10.0));
            test_hit!(app, r, "level:elevation");
            ui.label("Name");
            let r = ui.add(egui::TextEdit::singleline(&mut app.level_name).hint_text("Level N").desired_width(110.0));
            test_hit!(app, r, "level:name");
            let r = ui.button("Place Level");
            test_hit!(app, r, "level:place");
            if r.clicked() { app.level_click(); }
        });
        // the levels of the project, lowest first: renaming and deleting them is here, beside placing one (A6). The
        // draft of a name being typed is held by the app, so it is not rebuilt from the document every frame (R4).
        let levels = app.levels();
        ui.horizontal_wrapped(|ui| {
            ui.label("Levels:");
            for (id, name, elevation) in levels {
                if app.level_rename_id == Some(id) {
                    let field = ui.add(egui::TextEdit::singleline(&mut app.level_rename_name).desired_width(110.0));
                    test_hit!(app, field, "level:rename_field");
                    let ok = ui.small_button("OK");
                    test_hit!(app, ok, "level:rename_ok");
                    if (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ok.clicked() {
                        app.commit_level_rename();
                    }
                    if ui.small_button("Cancel").clicked() { app.cancel_level_rename(); }
                } else {
                    ui.label(&name);
                    let r = ui.small_button("Rename");
                    test_hit!(app, r, "level:rename:{name}");
                    if r.clicked() { app.start_level_rename(id); }
                }
                let editing = app.level_elev_edit.as_ref().is_some_and(|(e, _)| *e == id);
                if editing {
                    let mut text = app.level_elev_edit.as_ref().map(|(_, t)| t.clone()).unwrap_or_default();
                    let field = ui.add(egui::TextEdit::singleline(&mut text).desired_width(70.0));
                    test_hit!(app, field, "level:elevation_field");
                    ui.weak("mm");
                    if let Some((_, t)) = app.level_elev_edit.as_mut() { *t = text; }
                    let ok = ui.small_button("OK");
                    test_hit!(app, ok, "level:elevation_ok");
                    if (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ok.clicked() { app.commit_level_elevation(); }
                    if ui.small_button("Cancel").clicked() { app.level_elev_edit = None; }
                } else {
                    let r = ui.small_button(format!("{elevation:.3} m")).on_hover_text("Change the elevation; what stands on the level moves with it");
                    test_hit!(app, r, "level:elevation:{name}");
                    if r.clicked() { app.start_level_elevation(id); }
                }
                if ui.small_button("Delete").on_hover_text("A level that elements stand on cannot be deleted").clicked() {
                    app.delete_level(id);
                }
            }
        });
        return;
    }
    if app.tool == crate::Tool::Grid {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Grid");
            ui.separator();
            ui.label(format!("Next name: {}", app.document.next_grid_name()));
        });
        return;
    }
    if matches!(app.tool, crate::Tool::Align | crate::Tool::Trim | crate::Tool::Offset) {
        ui.horizontal_wrapped(|ui| {
            ui.strong(match app.tool { crate::Tool::Align => "Modify | Align", crate::Tool::Trim => "Modify | Trim/Extend to Corner", _ => "Modify | Offset" });
            ui.separator();
            if app.tool == crate::Tool::Offset {
                ui.label("Offset (mm)");
                ui.add(egui::DragValue::new(&mut app.offset_mm).speed(10.0).range(1.0..=100000.0));
                ui.checkbox(&mut app.offset_copy, "Copy");
            }
            if let Some((id, _)) = app.edit_ref { if let Some(e) = app.document.get(id) { ui.label(format!("first: {}", e.name)); } }
        });
        return;
    }
    if matches!(app.tool, crate::Tool::Move | crate::Tool::Copy | crate::Tool::Rotate | crate::Tool::Mirror) {
        ui.horizontal_wrapped(|ui| {
            ui.strong(match app.tool { crate::Tool::Move => "Modify | Move", crate::Tool::Copy => "Modify | Copy", crate::Tool::Rotate => "Modify | Rotate", _ => "Modify | Mirror" });
            ui.separator();
            if app.tool == crate::Tool::Mirror { ui.checkbox(&mut app.mirror_copy, "Copy"); }
            if app.tool == crate::Tool::Move { ui.checkbox(&mut app.move_disjoin, "Disjoin").on_hover_text("Move the element alone: its joins to pipes, ducts and fittings are broken first and nothing follows"); }
            let what = match app.selected_id().and_then(|id| app.document.get(id)) { Some(e) => e.name.clone(), None => "nothing selected: click an element".to_string() };
            ui.label(what);
            if !app.edit_num.is_empty() { ui.label(format!("typed: {}", app.edit_num)); }
        });
        return;
    }
    if app.tool == crate::Tool::Wall {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Wall");
            ui.separator();
            ui.label("Location Line");
            let combo = egui::ComboBox::from_id_salt("wall_loc").selected_text(Desktop::WALL_LOCATIONS[app.wall_loc]).show_ui(ui, |ui| {
                for (i, name) in Desktop::WALL_LOCATIONS.iter().enumerate() {
                    let r = ui.selectable_value(&mut app.wall_loc, i, *name);
                    test_hit!(app, r, "wall_loc:{name}");
                }
            });
            test_hit!(app, combo.response, "wall_loc");
            ui.label("Draw");
            egui::ComboBox::from_id_salt("wall_mode").selected_text(Desktop::WALL_MODES[app.wall_mode]).show_ui(ui, |ui| {
                for (i, name) in Desktop::WALL_MODES.iter().enumerate() { if ui.selectable_value(&mut app.wall_mode, i, *name).clicked() { app.wall_pts.clear(); app.wall_ids.clear(); } }
            });
            if matches!(app.wall_mode, 3 | 4) {
                ui.label("Sides");
                ui.add(egui::DragValue::new(&mut app.wall_sides).range(crate::wall_shapes::MIN_SIDES..=crate::wall_shapes::MAX_SIDES));
            }
            ui.label("Type");
            let types: Vec<(ElementId, String, f64, f64)> = app.document.types_for(openbim_document::Category::Wall).iter().map(|t| (t.id, t.name.clone(), t.width_m, t.height_m)).collect();
            let shown = app.wall_type.and_then(|t| types.iter().find(|x| x.0 == t)).map_or("Custom".to_string(), |t| t.1.clone());
            egui::ComboBox::from_id_salt("wall_type").selected_text(shown).show_ui(ui, |ui| {
                if ui.selectable_label(app.wall_type.is_none(), "Custom").clicked() { app.wall_type = None; }
                for (id, name, w, h) in &types {
                    if ui.selectable_label(app.wall_type == Some(*id), format!("{name}  ({:.0} x {:.0})", w * 1000.0, h * 1000.0)).clicked() { app.wall_type = Some(*id); app.width_mm = w * 1000.0; app.height_mm = h * 1000.0; }
                }
            });
            ui.label("Thickness (mm)");
            if ui.add(egui::DragValue::new(&mut app.width_mm).speed(5.0).range(10.0..=2000.0)).changed() { app.wall_type = None; }
            ui.label("Height (mm)");
            if ui.add(egui::DragValue::new(&mut app.height_mm).speed(10.0).range(100.0..=30000.0)).changed() { app.wall_type = None; }
            ui.add(egui::TextEdit::singleline(&mut app.wall_type_name).hint_text("new type name").desired_width(110.0));
            if ui.button("Add type").on_hover_text("A wall type with this thickness and height").clicked() {
                let name = app.wall_type_name.clone();
                match app.document.add_wall_type(&name, app.width_mm / 1000.0, app.height_mm / 1000.0) {
                    Ok(id) => { app.wall_type = Some(id); app.wall_type_name.clear(); app.message = format!("Wall type {name} added"); }
                    Err(_) => app.message = "A wall type needs a name that is not used yet".into(),
                }
            }
            if ui.add_enabled(app.wall_type.is_some(), egui::Button::new("Edit Type…")).on_hover_text("The layers of the wall type").clicked() {
                if let Some(t) = app.wall_type { app.open_type_editor(t); }
            }
            ui.label("Base offset (mm)");
            ui.add(egui::DragValue::new(&mut app.elevation_mm).speed(10.0));
            ui.checkbox(&mut app.wall_chain_on, "Chain");
            ui.checkbox(&mut app.wall_flip, "Flip (Space)");
        });
        return;
    }
    if app.tool == crate::Tool::WallFooting {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Wall Footing");
            ui.separator();
            usage_material_combos(app, ui, false);
            let cat = openbim_document::Category::WallFooting;
            for field in dimensions(cat, 0).iter().copied() {
                ui.label(field_name(field, cat));
                if matches!(field, Field::Elevation) { ui.add(egui::DragValue::new(&mut app.slab_offset_mm).speed(1.0)); } else { editor(app, ui, field); }
            }
        });
        return;
    }
    if app.tool == crate::Tool::Footing {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Isolated Footing");
            ui.separator();
            usage_material_combos(app, ui, false);
            let cat = openbim_document::Category::Footing;
            for field in dimensions(cat, 0).iter().copied() {
                if matches!(field, Field::Elevation) { ui.label(field_name(field, cat)); ui.add(egui::DragValue::new(&mut app.slab_offset_mm).speed(1.0)); continue; }
                ui.label(field_name(field, cat));
                editor(app, ui, field);
            }
        });
        return;
    }
    if app.tool == crate::Tool::Family {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Component");
            ui.separator();
            family_combos(app, ui);
            ui.label("Rotation (deg)");
            ui.add(egui::DragValue::new(&mut app.angle_deg).speed(1.0).range(-360.0..=360.0));
            ui.label("Offset (mm)");
            ui.add(egui::DragValue::new(&mut app.slab_offset_mm).speed(1.0));
        });
        return;
    }
    if app.tool == crate::Tool::Rebar {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Rebar");
            ui.separator();
            for field in dimensions(openbim_document::Category::Rebar, 0).iter().copied() {
                ui.label(field_name(field, openbim_document::Category::Rebar));
                editor(app, ui, field);
            }
        });
        return;
    }
    if app.tool == crate::Tool::Truss {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Truss");
            ui.separator();
            usage_material_combos(app, ui, false);
            for field in dimensions(openbim_document::Category::Truss, 0).iter().copied() {
                ui.label(field_name(field, openbim_document::Category::Truss));
                editor(app, ui, field);
            }
        });
        return;
    }
    if app.tool == crate::Tool::EndPoint {
        ui.horizontal_wrapped(|ui| app.end_point_options(ui));
    }
    if matches!(app.tool, crate::Tool::ElecNode | crate::Tool::Cable) {
        ui.horizontal_wrapped(|ui| app.elec_options(ui));
    }
    if matches!(app.tool, crate::Tool::Duct | crate::Tool::Pipe) {
        ui.horizontal_wrapped(|ui| app.run_options(ui));
        return;
    }
    if app.tool == crate::Tool::BeamSystem {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Beam System");
            ui.separator();
            section_combo(app, ui);
            usage_material_combos(app, ui, true);
            for field in dimensions(openbim_document::Category::BeamSystem, 0).iter().copied() {
                ui.label(field_name(field, openbim_document::Category::BeamSystem));
                editor(app, ui, field);
            }
            ui.label(format!("{} corners", app.slab_points.len()));
            if ui.add_enabled(app.slab_points.len() >= 3, egui::Button::new("Finish")).clicked() { app.finish_slab(app.plan_level()); }
            if ui.add_enabled(!app.slab_points.is_empty(), egui::Button::new("Clear")).clicked() { app.slab_points.clear(); }
        });
        return;
    }
    if app.tool == crate::Tool::VerticalOpening {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Create Vertical Opening");
            ui.separator();
            let level = app.plan_level();
            match app.opening_host.and_then(|h| app.document.get(h)) {
                Some(h) => { ui.label(format!("Host: {}", h.name)); }
                None => { ui.label("Host: click a floor, roof or ceiling"); }
            }
            ui.label(format!("{} corners", app.slab_points.len()));
            if ui.add_enabled(app.slab_points.len() >= 3 && app.opening_host.is_some(), egui::Button::new("Finish")).clicked() { app.finish_slab(level); }
            if ui.add_enabled(!app.slab_points.is_empty(), egui::Button::new("Clear")).clicked() { app.slab_points.clear(); }
            if ui.add_enabled(app.opening_host.is_some(), egui::Button::new("Pick another host")).clicked() { app.opening_host = None; app.opening_cands.clear(); app.slab_points.clear(); }
        });
        return;
    }
    if app.tool == crate::Tool::Shaft {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Create Shaft Opening");
            ui.separator();
            let level = app.plan_level();
            ui.label(format!("Base Constraint: {}", app.document.get(level).map_or("-", |l| l.name.as_str())));
            ui.label("Base Offset (mm)");
            ui.add(egui::DragValue::new(&mut app.shaft_base_mm).speed(5.0));
            let base_z = app.document.get(level).map_or(0.0, |l| l.elevation_m);
            let above: Vec<(openbim_document::ElementId, String)> = app.document.elements().filter(|e| e.geometry == Some(openbim_document::Geometry::Level) && e.elevation_m > base_z + 1e-6)
                .map(|e| (e.id, e.name.clone())).collect();
            let chosen = app.shaft_top.filter(|t| above.iter().any(|a| a.0 == *t)).or_else(|| app.document.level_above(level));
            ui.label("Top Constraint");
            egui::ComboBox::from_id_salt("shaft_top").selected_text(chosen.and_then(|c| above.iter().find(|a| a.0 == c)).map_or("No level above", |a| a.1.as_str())).show_ui(ui, |ui| {
                for (id, name) in &above { if ui.selectable_label(chosen == Some(*id), format!("Up to level: {name}")).clicked() { app.shaft_top = Some(*id); } }
            });
            ui.label("Top Offset (mm)");
            ui.add(egui::DragValue::new(&mut app.shaft_top_mm).speed(5.0));
            ui.label(format!("{} corners", app.slab_points.len()));
            if ui.add_enabled(app.slab_points.len() >= 3, egui::Button::new("Finish")).clicked() { app.finish_slab(level); }
            if ui.add_enabled(!app.slab_points.is_empty(), egui::Button::new("Clear")).clicked() { app.slab_points.clear(); }
        });
        return;
    }
    if app.tool == crate::Tool::WallOpening {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Wall Opening");
            ui.separator();
            ui.label("Sill Height (mm)");
            ui.add(egui::DragValue::new(&mut app.opening_sill_mm).speed(5.0).range(0.0..=20000.0));
            ui.label("Head Height (mm)");
            ui.add(egui::DragValue::new(&mut app.opening_head_mm).speed(5.0).range(50.0..=20000.0));
            ui.label(if app.opening_from.is_some() { "Click the other end on the same wall" } else { "Click one end on a wall" });
        });
        return;
    }
    if app.tool == crate::Tool::Stair {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Stair");
            ui.separator();
            ui.label("Shape");
            let shown = openbim_document::StairShape::ALL.iter().find(|(s, _)| *s == app.stair_shape).map_or("Straight", |(_, n)| *n);
            let combo = egui::ComboBox::from_id_salt("stair_shape").selected_text(shown).show_ui(ui, |ui| {
                for (shape, name) in openbim_document::StairShape::ALL {
                    let r = ui.selectable_value(&mut app.stair_shape, shape, name);
                    test_hit!(app, r, "stair_shape:{name}");
                }
            });
            test_hit!(app, combo.response, "stair_shape");
            ui.label("Railing");
            let shown = openbim_document::RailingSides::ALL.iter().find(|(s, _)| *s == app.stair_railing).map_or("Both sides", |(_, n)| *n);
            egui::ComboBox::from_id_salt("stair_railing").selected_text(shown).show_ui(ui, |ui| {
                for (sides, name) in openbim_document::RailingSides::ALL { ui.selectable_value(&mut app.stair_railing, sides, name); }
            });
            ui.label("Width (mm)");
            ui.add(egui::DragValue::new(&mut app.stair_width_mm).speed(5.0).range(200.0..=10000.0));
            ui.label("Max Riser Height (mm)");
            ui.add(egui::DragValue::new(&mut app.stair_riser_mm).speed(1.0).range(60.0..=400.0));
            ui.label("Tread Depth (mm)");
            ui.add(egui::DragValue::new(&mut app.stair_tread_mm).speed(1.0).range(100.0..=1000.0));
            ui.label("Base Offset (mm)");
            ui.add(egui::DragValue::new(&mut app.elevation_mm).speed(5.0));
            if app.document.level_above(app.plan_level()).is_none() {
                ui.label("Height (mm)");
                ui.add(egui::DragValue::new(&mut app.stair_rise_mm).speed(5.0).range(200.0..=50000.0));
            }
        });
        return;
    }
    if app.tool == crate::Tool::Railing {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Railing");
            ui.separator();
            ui.label("Height (mm)");
            ui.add(egui::DragValue::new(&mut app.height_mm).speed(5.0).range(100.0..=5000.0));
            ui.label("Rail and Post Size (mm)");
            ui.add(egui::DragValue::new(&mut app.width_mm).speed(1.0).range(10.0..=500.0));
            ui.label("Baluster Spacing (mm, 0 for none)");
            ui.add(egui::DragValue::new(&mut app.spacing_mm).speed(5.0).range(0.0..=2000.0));
            ui.label("Base Offset (mm)");
            ui.add(egui::DragValue::new(&mut app.elevation_mm).speed(5.0));
            ui.label(format!("{} points", app.slab_points.len()));
            if ui.add_enabled(app.slab_points.len() >= 2, egui::Button::new("Finish")).clicked() { app.finish_slab(app.plan_level()); }
            if ui.add_enabled(!app.slab_points.is_empty(), egui::Button::new("Clear")).clicked() { app.slab_points.clear(); }
        });
        return;
    }
    if app.tool == crate::Tool::Slab {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Slab");
            ui.separator();
            slab_combos(app, ui);
            ui.label("Thickness (mm)");
            ui.add(egui::DragValue::new(&mut app.slab_thickness_mm).speed(1.0).range(1.0..=5000.0));
            let roof = app.slab_kind_pick == openbim_document::SlabKind::Roof;
            let ceiling = app.slab_kind_pick == openbim_document::SlabKind::Ceiling;
            ui.label(if roof { "Base Offset (mm)" } else if ceiling { "Height Offset from Level (mm)" } else { "Top Offset (mm)" });
            ui.add(egui::DragValue::new(&mut app.slab_offset_mm).speed(1.0));
            if ceiling { ui.checkbox(&mut app.ceiling_auto, "Automatic"); }
            if roof {
                ui.label("Slope (°)");
                ui.add(egui::DragValue::new(&mut app.roof_slope_deg).speed(0.5).range(0.0..=78.0));
            }
            ui.label(format!("{} corners", app.slab_points.len()));
            if ui.add_enabled(app.slab_points.len() >= 3, egui::Button::new("Finish")).clicked() { app.finish_slab(app.plan_level()); }
            if ui.add_enabled(!app.slab_points.is_empty(), egui::Button::new("Clear")).clicked() { app.slab_points.clear(); }
        });
        return;
    }
    if app.tool == crate::Tool::Column {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Structural Column");
            ui.separator();
            section_combo(app, ui);
            usage_material_combos(app, ui, false);
            let cat = openbim_document::Category::Column;
            for field in dimensions(cat, app.section_pick).iter().copied().filter(|f| !matches!(f, Field::Elevation)) {
                ui.label(field_name(field, cat));
                editor(app, ui, field);
            }
        });
        return;
    }
    if app.tool == crate::Tool::Beam {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Place Structural Framing");
            ui.separator();
            let cat = openbim_document::Category::Beam;
            let levels = app.levels();
            let current = app.beam_level.and_then(|l| levels.iter().find(|x| x.0 == l)).or(levels.first()).cloned();
            ui.label("Level");
            egui::ComboBox::from_id_salt("beam_level").selected_text(current.map_or("-".into(), |l| l.1)).show_ui(ui, |ui| {
                for (id, name, _) in &levels { ui.selectable_value(&mut app.beam_level, Some(*id), name); }
            });
            section_combo(app, ui);
            usage_material_combos(app, ui, true);
            if app.usage_pick == openbim_document::Usage::Brace {
                ui.label("Rise (mm)");
                ui.add(egui::DragValue::new(&mut app.brace_rise_mm).speed(10.0));
            }
            let mut fields = dimensions(cat, app.section_pick).to_vec();
            if app.usage_pick == openbim_document::Usage::Brace { fields.retain(|f| !matches!(f, Field::Slope)); }
            fields.extend([Field::X1, Field::Y1, Field::X2, Field::Y2]);
            for field in fields {
                ui.label(field_name(field, cat));
                editor(app, ui, field);
            }
            if ui.button("Place Beam").clicked() { app.place_beam_from_controls(); }
        });
        return;
    }
    let element = app.selected_id().and_then(|id| app.document.get(id)).cloned();
    ui.horizontal_wrapped(|ui| {
        let Some(el) = element else { ui.strong("Modify | Select"); return; };
        ui.strong(format!("Modify | {}", category_info(el.category).0));
        ui.separator();
        let fields = if el.category == openbim_document::Category::Grid {
            &[Field::X1, Field::Y1, Field::X2, Field::Y2][..]
        } else if matches!(el.geometry, Some(openbim_document::Geometry::Shaft { .. })) || openbim_document::Document::is_vertical_opening(&el) { &[][..] } else { dimensions(el.category, app.section_pick) };
        for field in fields { ui.label(field_name(*field, el.category)); editor(app, ui, *field); }
        let pending = app.has_pending_edit();
        let apply = ui.add_enabled(pending, egui::Button::new("Apply"));
        test_hit!(app, apply, "props:Apply");
        if apply.clicked() {
            if el.id == crate::ElementId(1) { app.apply_duct_edit(); } else { app.apply_selected_edit(); }
        }
        if ui.add_enabled(pending, egui::Button::new("Reset")).clicked() { app.sync_duct_controls(); }
    });
}

pub fn properties(app: &mut Desktop, ui: &mut egui::Ui) {
    use openbim_document::Geometry;
    title_bar(ui, "Properties", &mut app.show_properties);
    let element = app.selected_id().and_then(|id| app.document.get(id)).cloned();
    let (category, family_type, icon) = element.as_ref().map(|el| category_info(el.category))
        .unwrap_or(("View Properties", "Current View", "view"));
    ui.horizontal(|ui| {
        ui.add(ribbon::icon_image(ui, icon, app.dark, Vec2::splat(40.0)));
        ui.vertical(|ui| {
            ui.label(RichText::new(category).strong());
            match &element {
                Some(el) if openbim_document::Element::can_have_type(el.category) => type_combo(app, ui, el),
                _ => { egui::ComboBox::from_id_salt("property_type").width(180.0)
                    .selected_text(family_type).show_ui(ui, |ui| {
                        ui.label(family_type); ui.weak("No additional types loaded");
                    }); }
            }
        });
    });
    match &element {
        Some(el) if openbim_document::Element::can_have_type(el.category) => type_maker(app, ui, el),
        _ => if ui.small_button("Edit Type").clicked() { app.message = "Type editing is not implemented yet".into(); },
    }
    ui.add(egui::TextEdit::singleline(&mut app.property_query).hint_text("Search parameters").desired_width(f32::INFINITY));
    let query = app.property_query.trim().to_lowercase();
    ui.separator();
    egui::ScrollArea::vertical().id_salt("property_parameters")
        .max_height((ui.available_height() - 50.0).max(20.0)).auto_shrink([false, false]).show(ui, |ui| {
            if let Some(el) = &element {
                ui.push_id(el.id.0, |ui| {
                    if el.category != openbim_document::Category::Type && (query.is_empty() || "pinned constraints".contains(&query)) {
                        let mut pinned = el.pinned;
                        if ui.checkbox(&mut pinned, "Pinned").on_hover_text("Pin (PN): it cannot be moved or deleted until it is unpinned").changed() { app.pin_selected(pinned); }
                    }
                    if let Some(Geometry::Point { level, top, .. }) = el.geometry {
                        let name = |id| app.document.get(id).map_or("Missing level", |l| l.name.as_str());
                        read_only_group(ui, "Constraints", &query, &[("Base Level", name(level)), ("Top Level", name(top))]);
                    }
                    if matches!(el.category, openbim_document::Category::Slab | openbim_document::Category::Footing) && el.category == openbim_document::Category::Footing { usage_material_combos(app, ui, false); }
                    if el.category == openbim_document::Category::Room { room_identity(app, ui, el); }
                    if el.category == openbim_document::Category::Family {
                        read_only_group(ui, "Identity Data", &query, &[("Family", &el.family)]);
                        family_type_combo(app, ui, &el.family);
                        // the instance's own type of a family: its parameters are edited here, once for every
                        // instance of it, rather than one instance at a time
                        if let Some(type_id) = el.type_id {
                            let name = app.document.get(type_id).map_or("Missing type".into(), |t| t.name.clone());
                            read_only_group(ui, "Type of a Family", &query, &[("Type", &name)]);
                            if !app.family_type_values.is_empty() || !app.family_type_texts.is_empty() {
                                egui::CollapsingHeader::new(RichText::new("Type Parameters").strong()).id_salt("family_type_params").default_open(true).show(ui, |ui| {
                                    egui::Grid::new("family_type_rows").num_columns(2).striped(true).show(ui, |ui| {
                                        for (name, value) in app.family_type_values.iter_mut() {
                                            ui.label(name.replace('_', " "));
                                            ui.add(egui::DragValue::new(value).speed(0.01));
                                            ui.end_row();
                                        }
                                        for (name, value, options) in app.family_type_texts.iter_mut() {
                                            ui.label(name.replace('_', " "));
                                            if options.is_empty() { ui.text_edit_singleline(value); } else {
                                                egui::ComboBox::from_id_salt(("family_type_text", name.as_str())).selected_text(value.clone()).show_ui(ui, |ui| {
                                                    for o in options.iter() { ui.selectable_value(value, o.clone(), o); }
                                                });
                                            }
                                            ui.end_row();
                                        }
                                    });
                                    ui.small("A type is shared: this changes every instance of it. An instance's own value in Instance Parameters still wins.");
                                    if ui.button("Apply to the type").clicked() { app.apply_family_type(); }
                                    if ui.button("Table of types…").on_hover_text("Every type of this family in one table").clicked() { let f = el.family.clone(); app.open_family_types(&f); }
                                });
                            }
                        }
                        if !app.family_texts.is_empty() {
                            egui::CollapsingHeader::new(RichText::new("Instance Choices and Texts").strong()).id_salt("family_texts").default_open(true).show(ui, |ui| {
                                egui::Grid::new("family_text_rows").num_columns(2).striped(true).show(ui, |ui| {
                                    for (name, value, options) in app.family_texts.iter_mut() {
                                        ui.label(name.replace('_', " "));
                                        if options.is_empty() { ui.text_edit_singleline(value); } else {
                                            egui::ComboBox::from_id_salt(("family_text", name.as_str())).selected_text(value.clone()).show_ui(ui, |ui| {
                                                for o in options.iter() { ui.selectable_value(value, o.clone(), o); }
                                            });
                                        }
                                        ui.end_row();
                                    }
                                });
                            });
                        }
                        if !app.family_values.is_empty() {
                            egui::CollapsingHeader::new(RichText::new("Instance Parameters").strong()).id_salt("family_values").default_open(true).show(ui, |ui| {
                                egui::Grid::new("family_value_rows").num_columns(2).striped(true).show(ui, |ui| {
                                    for (name, value) in app.family_values.iter_mut() {
                                        if !matches(&query, name) && !matches(&query, "instance parameters") { continue; }
                                        ui.label(name.replace('_', " "));
                                        ui.add(egui::DragValue::new(value).speed(0.01));
                                        ui.end_row();
                                    }
                                });
                            });
                        }
                        if let Some(Geometry::InWall { host, along_m }) = el.geometry {
                            let name = app.document.get(host).map_or("Missing wall", |w| w.name.as_str());
                            let along = format!("{:.0}", along_m * 1000.0);
                            read_only_group(ui, "Constraints", &query, &[("Host Wall", name), ("Distance Along Wall (mm)", &along)]);
                        }
                    }
                    if let Some(Geometry::OnMember { host }) = el.geometry {
                        let name = app.document.get(host).map_or("Missing beam", |w| w.name.as_str());
                        read_only_group(ui, "Constraints", &query, &[("Host Beam", name)]);
                        let bars = app.document.rebar_bars(el).map_or(0, |b| b.len()).to_string();
                        read_only_group(ui, "Derived Geometry", &query, &[("Bars and Stirrups", &bars)]);
                    }
                    if el.category == openbim_document::Category::Beam {
                        for r in app.document.rebars_of(el.id) {
                            let label = format!("Select {}", app.document.get(r).map_or("rebar", |e| e.name.as_str()));
                            if ui.button(label).clicked() { app.select_element(Some(r)); }
                        }
                    }
                    if let Some(Geometry::OnWall { host }) = el.geometry {
                        let name = app.document.get(host).map_or("Missing wall", |w| w.name.as_str());
                        read_only_group(ui, "Constraints", &query, &[("Host Wall", name)]);
                    }
                    if el.category == openbim_document::Category::WallFooting { usage_material_combos(app, ui, false); }
                    if let Some(Geometry::Pad { level, .. } | Geometry::Area { level }) = el.geometry {
                        let level_name = app.document.get(level).map_or("Missing level", |l| l.name.as_str());
                        read_only_group(ui, "Constraints", &query, &[("Level", level_name)]);
                    }
                    if let Some(Geometry::Linear { level, .. }) = el.geometry {
                        let level_name = app.document.get(level).map_or("Missing level", |l| l.name.as_str());
                        read_only_group(ui, "Constraints", &query, &[("Reference Level", level_name)]);
                    }
                    if el.is_roof() { roof_group(app, ui, el); }
                    if let Some(f) = app.document.stair_figures(el.id) {
                        let (r, t, rh, td, run) = (f.risers.to_string(), f.treads.to_string(), format!("{:.1}", f.riser_m * 1000.0), format!("{:.1}", f.tread_m * 1000.0), format!("{:.3}", f.run_m));
                        read_only_group(ui, "Derived Geometry", &query, &[("Number of Risers", &r), ("Number of Treads", &t), ("Actual Riser Height (mm)", &rh), ("Actual Tread Depth (mm)", &td), ("Run Length (m)", &run)]);
                    }
                    if el.category == openbim_document::Category::Slab {
                        slab_combos(app, ui);
                        let area = openbim_geom::profile::polygon_area(&el.outline).abs();
                        let (corners, area) = (el.outline.len().to_string(), format!("{area:.3}"));
                        read_only_group(ui, "Derived Geometry", &query, &[("Corners", &corners), ("Area (m²)", &area)]);
                    }
                    if let (openbim_document::Category::Grid, Some(Geometry::Grid { start, end })) = (el.category, el.geometry) {
                        if let Some(arc) = openbim_document::grid_arc::arc(start, end, el.bulge) {
                            let (radius, sweep, len) = (format!("{:.3}", arc.radius), format!("{:.2}", arc.sweep.to_degrees()), format!("{:.3}", arc.length()));
                            read_only_group(ui, "Arc", &query, &[("Radius (m)", &radius), ("Sweep (°, + counter-clockwise)", &sweep), ("Length (m)", &len)]);
                        }
                    }
                    if el.category == openbim_document::Category::Generic {
                        // what a file brought in: shown as it came, not edited here
                        let (height, base, area) = (format!("{:.3}", el.height_m), format!("{:.3}", el.elevation_m - el.height_m), format!("{:.3}", openbim_geom::profile::polygon_area(&el.outline).abs()));
                        read_only_group(ui, "Derived Geometry", &query, &[("Height (m)", &height), ("Base above level (m)", &base), ("Plan area (m²)", &area)]);
                        let rows: Vec<(&str, &str)> = el.texts.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                        read_only_group(ui, "Source (IFC)", &query, &rows);
                    }
                    if matches!(el.category, openbim_document::Category::Beam | openbim_document::Category::Column | openbim_document::Category::BeamSystem) { section_combo(app, ui); usage_material_combos(app, ui, el.category != openbim_document::Category::Column); }
                    if el.category == openbim_document::Category::Truss { usage_material_combos(app, ui, false); }
                    if matches!(el.category, openbim_document::Category::BeamSystem | openbim_document::Category::Truss) {
                        let n = app.document.system_beams(el.id).len().to_string();
                        read_only_group(ui, "Derived Geometry", &query, &[("Beams", &n)]);
                    }
                    if let (openbim_document::Category::Beam, Some(p)) = (el.category, el.parent) {
                        let name = app.document.get(p).map_or("Missing system", |s| s.name.as_str());
                        read_only_group(ui, "Constraints", &query, &[("Beam System", name)]);
                        if ui.button("Select system").clicked() { app.select_element(Some(p)); }
                    }
                    if openbim_document::Document::is_vertical_opening(el) {
                        let name = el.parent.and_then(|p| app.document.get(p)).map_or("Missing host", |h| h.name.as_str());
                        read_only_group(ui, "Constraints", &query, &[("Host", name)]);
                        if let Some(p) = el.parent { if ui.button("Select host").clicked() { app.select_element(Some(p)); } }
                    } else if matches!(el.geometry, Some(Geometry::Shaft { .. })) { shaft_group(app, ui, el); } else {
                        edit_group(app, ui, "Dimensions", &query, el.category, dimensions(el.category, app.section_pick));
                    }
                    if el.category == openbim_document::Category::Elevation {
                        // the views of the marker: tick the ones it holds, open one with its button
                        for (k, (_, name)) in crate::section_ui::FACINGS.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let mut on = el.count >> k & 1 == 1;
                                if ui.checkbox(&mut on, *name).on_hover_text("A view of the marker looking this way").changed() { app.toggle_marker_view(el.id, k); }
                                if on && ui.small_button("Open").clicked() { app.view = View::Marked(el.id.0, k as u8); app.selected = None; }
                            });
                        }
                    }
                    if el.category == openbim_document::Category::Section && ui.button("Flip direction").on_hover_text("Look to the other side of the line (Space)").clicked() { app.flip_selected_section(); }
                    if el.category == openbim_document::Category::Section && ui.button("Open view").on_hover_text("Go to the view of this section line").clicked() {
                        app.view = View::Section(el.id.0);
                        app.selected = None;
                    }
                    if el.category == openbim_document::Category::Beam {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut app.join_pick[0], "Join at start");
                            ui.checkbox(&mut app.join_pick[1], "Join at end");
                        });
                        if let Some(ext) = app.document.beam_extent(el) {
                            let (cut, a, b) = (format!("{:.1}", ext.cut_length() * 1000.0), format!("{:.1}", ext.cut_start * 1000.0), format!("{:.1}", ext.cut_end * 1000.0));
                            read_only_group(ui, "Joins", &query, &[("Cut Length (mm)", &cut), ("Cut at Start (mm)", &a), ("Cut at End (mm)", &b)]);
                        }
                    }
                    if let Some(Geometry::Linear { start, end, level }) = el.geometry {
                        let length = (end[0] - start[0]).hypot(end[1] - start[1]);
                        let start_z = app.document.get(level).map_or(0.0, |l| l.elevation_m) + el.elevation_m;
                        let length_text = format!("{:.1}", length * 1000.0);
                        let end_text = format!("{:.1}", (start_z + length * el.slope_ratio.unwrap_or(0.0)) * 1000.0);
                        read_only_group(ui, "Derived Geometry", &query,
                            &[("Run Length (mm)", &length_text), ("End Elevation (mm)", &end_text)]);
                        if matches!(el.category, openbim_document::Category::Duct | openbim_document::Category::Pipe) && (query.is_empty() || "constraints offset start end".contains(&query)) {
                            let level_z = app.document.get(level).map_or(0.0, |l| l.elevation_m);
                            let ends = [("Start Offset (mm)", true, (start_z - level_z) * 1000.0), ("End Offset (mm)", false, (start_z - level_z + length * el.slope_ratio.unwrap_or(0.0)) * 1000.0)];
                            egui::CollapsingHeader::new(RichText::new("Run Ends").strong()).id_salt("run_ends").default_open(true).show(ui, |ui| {
                                for (label, at_start, cur) in ends {
                                    ui.horizontal(|ui| {
                                        ui.label(label);
                                        // edited on a copy; one change is made when the drag ends or Enter is pressed
                                        let key = ui.id().with(("run_end", el.id.0, at_start));
                                        let mut v = ui.data(|d| d.get_temp::<f64>(key)).unwrap_or(cur);
                                        let r = ui.add(egui::DragValue::new(&mut v).speed(1.0).max_decimals(1)).on_hover_text("Height of the middle of the run at this end, above its level. A joined end cannot be changed here: move what it is joined to");
                                        if r.drag_stopped() || r.lost_focus() {
                                            ui.data_mut(|d| d.remove::<f64>(key));
                                            if (v - cur).abs() > 0.05 { app.set_run_end_offset(el.id, at_start, v); }
                                        } else if r.dragged() || r.has_focus() || r.changed() {
                                            ui.data_mut(|d| d.insert_temp(key, v));
                                        } else {
                                            ui.data_mut(|d| d.remove::<f64>(key));
                                        }
                                    });
                                }
                            });
                        }
                    }
                    if matches!(el.category, openbim_document::Category::Duct | openbim_document::Category::Pipe) {
                        let rows = app.insulation_rows(el.id);
                        if !rows.is_empty() {
                            let rows: Vec<(&str, &str)> = rows.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
                            read_only_group(ui, "Insulation", &query, &rows);
                        }
                    }
                    if matches!(el.geometry, Some(Geometry::Linear { .. } | Geometry::Grid { .. })) {
                        edit_group(app, ui, "Location", &query, el.category, &[Field::X1, Field::Y1, Field::X2, Field::Y2]);
                    }
                    let id = el.id.0.to_string();
                    read_only_group(ui, "Identity Data", &query, &[("Name", &el.name), ("Element ID", &id)]);
                });
            } else {
                let view_label = level_view_name(app, view_name(app.view), app.view);
                read_only_group(ui, "Graphics", &query, &[("View Name", view_label.as_str()), ("Projection", "Orthographic")]);
            }
        });
    ui.separator();
    if element.is_some() {
        let pending = app.has_pending_edit();
        ui.horizontal(|ui| {
            if ui.add_enabled(pending, egui::Button::new("Apply")).clicked() { app.apply_selected_edit(); }
            if ui.add_enabled(pending, egui::Button::new("Reset")).clicked() { app.sync_duct_controls(); }
            if pending { ui.weak("Pending changes"); }
        });
    }
    ui.label(RichText::new("In memory / not saved").small().weak());
}

/// The name of a level view: the level's own name, so renaming a level renames its plan views with it (A2). The
/// const table's name is the fallback for a project that has no such level.
pub(crate) fn level_view_name(app: &Desktop, fallback: &str, view: View) -> String {
    let name = match view {
        View::Plan => app.levels().first().map(|level| level.1.clone()),
        View::UpperPlan => app.levels().get(1).map(|level| level.1.clone()),
        View::Level(id) => app.document.get(ElementId(id))
            .filter(|el| el.geometry == Some(openbim_document::Geometry::Level)).map(|el| el.name.clone()),
        _ => None,
    };
    name.unwrap_or_else(|| fallback.to_string())
}

pub fn view_name(view: View) -> &'static str {
    match view { View::Plan => "Level 1", View::UpperPlan => "Level 2", View::ThreeD => "{3D}", View::Mechanical => "M-101 - Mechanical", View::Level(_) => "Level plan", View::Elevation(_) | View::Section(_) | View::Marked(..) => "Elevation / section", View::Schedule(_) => "Schedule", View::Sheet(_) => "Sheet" }
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
        folder("Floor Plans", &[view("Level 1", View::Plan), view("Level 2", View::UpperPlan), view("M-101 - Mechanical", View::Mechanical)]),
        folder("Ceiling Plans", &[]),
        folder("3D Views", &[view("{3D}", View::ThreeD)]),
        folder("Elevations (Building Elevation)", &[view("East", View::Elevation(0)), view("North", View::Elevation(1)), view("South", View::Elevation(2)), view("West", View::Elevation(3))]),
        folder("Sections (Building Section)", &[]), folder("Legends", &[]),
    ]),
    folder("Schedules/Quantities", &[]), folder("Sheets (all)", &[]),
    folder("Families", &[]), folder("Groups", &[]), folder("Model Links", &[]),
];

fn node_matches(app: &Desktop, node: &Node, query: &str) -> bool {
    // a level view is searched by the name it shows, which is the level's own (A2/R1)
    let shown = node.view.map_or_else(|| node.name.to_string(), |view| level_view_name(app, node.name, view));
    let added_level = node.name == "Floor Plans" && app.levels().iter().skip(2).any(|l| matches(query, &l.1));
    matches(query, node.name) || matches(query, &shown) || added_level || node.children.iter().any(|child| node_matches(app, child, query))
}

fn tree_node(app: &mut Desktop, ui: &mut egui::Ui, node: &Node, parent: &str, query: &str) {
    if !node_matches(app, node, query) { return; }
    let path = format!("{parent}/{}", node.name);
    if let Some(view) = node.view {
        let active = app.view == view;
        let color = if active { ui.visuals().hyperlink_color } else { ui.visuals().text_color() };
        let label = RichText::new(level_view_name(app, node.name, view)).color(color);
        let label = if active { label.strong() } else { label };
        let response = ui.selectable_label(app.browser_selected == path, label)
            .on_hover_text("Double-click to open view");
        test_hit!(app, response, "view:{}", level_view_name(app, node.name, view));
        if response.clicked() { app.browser_selected = path; }
        response.context_menu(|ui| app.add_to_sheet_menu(ui, crate::view_ui::view_key(view)));
        if response.double_clicked() {
            app.view = view;
            app.selected = None;
            app.message = format!("Opened {}", level_view_name(app, node.name, view));
        }
    } else {
        egui::CollapsingHeader::new(RichText::new(node.name).strong())
            .id_salt(&path).default_open(node.name == "Views (all)" || !node.children.is_empty())
            .open(if query.is_empty() { None } else { Some(true) })
            .show(ui, |ui| {
                if node.name == "Schedules/Quantities" {
                    app.schedule_browser_items(ui, query);
                } else if node.name == "Sheets (all)" {
                    app.sheet_browser_items(ui, query);
                } else if node.children.is_empty() {
                    ui.label(RichText::new("No items in sample").weak().italics());
                } else {
                    // A matching parent reveals its entire subtree.
                    let child_query = if matches(query, node.name) { "" } else { query };
                    for child in node.children { tree_node(app, ui, child, &path, child_query); }
                    // the plans of the levels after the first two belong with the other floor plans (A2/R1)
                    if node.name == "Floor Plans" { level_plans(app, ui, child_query); }
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
            // the section lines of the model, each opening its view
            let sections: Vec<(ElementId, String)> = app.document.elements().filter(|e| e.category == openbim_document::Category::Section).map(|e| (e.id, e.name.clone())).collect();
            for (id, name) in sections {
                if !matches(&query, &name) { continue; }
                let active = app.view == View::Section(id.0);
                let r = ui.selectable_label(active, RichText::new(format!("{name} (section)"))).on_hover_text("Open the view of this section line");
                test_hit!(app, r, "view:{name}");
                if r.clicked() { app.view = View::Section(id.0); app.selected = None; }
            }
            // the views of the elevation markers placed in plans
            let marked: Vec<(ElementId, String, u32)> = app.document.elements().filter(|e| e.category == openbim_document::Category::Elevation).map(|e| (e.id, e.name.clone(), e.count)).collect();
            for (id, name, views) in marked {
                for (k, (_, side)) in crate::section_ui::FACINGS.iter().enumerate() {
                    let label = format!("{name} - {side}");
                    if views >> k & 1 == 0 || !matches(&query, &label) { continue; }
                    let active = app.view == View::Marked(id.0, k as u8);
                    if ui.selectable_label(active, RichText::new(format!("{label} (elevation)"))).on_hover_text("Open this view of the elevation marker").clicked() { app.view = View::Marked(id.0, k as u8); app.selected = None; }
                }
            }
            if !TREE.iter().any(|node| node_matches(app, node, &query)) { ui.weak("No matching items"); }
        });
}

/// The plans of the levels after the first two (whose plans are the fixed Level 1 and Level 2 entries): listed in
/// the Floor Plans folder like the others, and opened by a double click as they are (A2/R1).
fn level_plans(app: &mut Desktop, ui: &mut egui::Ui, query: &str) {
    let added: Vec<(ElementId, String)> = app.levels().into_iter().skip(2).map(|l| (l.0, l.1)).collect();
    for (id, name) in added {
        if !matches(query, &name) { continue; }
        let view = View::Level(id.0);
        let active = app.view == view;
        let color = if active { ui.visuals().hyperlink_color } else { ui.visuals().text_color() };
        let label = RichText::new(&name).color(color);
        let path = format!("project/Views (all)/Floor Plans/{name}");
        let r = ui.selectable_label(app.browser_selected == path, if active { label.strong() } else { label }).on_hover_text("Double-click to open view");
        test_hit!(app, r, "view:{name}");
        if r.clicked() { app.browser_selected = path; }
        r.context_menu(|ui| app.add_to_sheet_menu(ui, crate::view_ui::view_key(view)));
        if r.double_clicked() { app.view = view; app.selected = None; app.message = format!("Opened {name}"); }
    }
}

/// Number, Name and Department of the selected room, editable in place and applied as one undoable step (B21). The
/// draft is held in the UI memory keyed by the room and its revision, so it is re-read after an undo or another edit.
fn room_identity(app: &mut Desktop, ui: &mut egui::Ui, el: &openbim_document::Element) {
    let key = egui::Id::new(("room_identity", el.id.0, el.revision));
    let seed = (app.document.room_text(el.id, "Number"), el.name.clone(), app.document.room_text(el.id, "Department"));
    let mut draft = ui.data_mut(|d| d.get_temp::<(String, String, String)>(key)).unwrap_or(seed);
    egui::CollapsingHeader::new(RichText::new("Identity Data").strong()).id_salt("room_identity").default_open(true).show(ui, |ui| {
        egui::Grid::new("room_identity_rows").num_columns(2).striped(true).show(ui, |ui| {
            ui.label("Number"); let r = ui.add(egui::TextEdit::singleline(&mut draft.0).desired_width(120.0)); test_hit!(app, r, "room:Number"); ui.end_row();
            ui.label("Name"); let r = ui.add(egui::TextEdit::singleline(&mut draft.1).desired_width(120.0)); test_hit!(app, r, "room:Name"); ui.end_row();
            ui.label("Department"); let r = ui.add(egui::TextEdit::singleline(&mut draft.2).desired_width(120.0)); test_hit!(app, r, "room:Department"); ui.end_row();
        });
        let apply = ui.small_button("Apply").on_hover_text("Sets Number, Name and Department in one undoable step");
        test_hit!(app, apply, "room:Apply");
        if apply.clicked() {
            app.message = match app.document.set_room_data(el.id, &draft.0, &draft.1, &draft.2) {
                Ok(()) => "Room updated; undo restores it".into(),
                Err(_) => "Room not changed: it needs a name, and its number must be new on its level".into(),
            };
        }
    });
    ui.data_mut(|d| d.insert_temp(key, draft));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SampleElement;

    #[test]
    fn a_door_command_offers_only_door_families() {
        let mut app = Desktop::sample();
        app.run_command("door", "Door");
        assert_eq!(app.family_kind, Some(openbim_family::Category::Door));
        let offered: Vec<_> = crate::export::library().iter().filter(|f| app.family_kind.is_none_or(|k| f.category() == k)).collect();
        assert!(!offered.is_empty() && offered.iter().all(|f| f.category() == openbim_family::Category::Door), "{:?}", offered.iter().map(|f| f.name()).collect::<Vec<_>>());
        // leaving the tool offers every family again
        app.choose_tool(crate::Tool::Select);
        assert_eq!(app.family_kind, None);
    }

    #[test]
    fn browser_search_keeps_ancestors_of_matching_views() {
        let app = Desktop::sample();
        assert!(node_matches(&app, &TREE[0], "m-101"));
        assert!(node_matches(&app, &TREE[0], "{3d}"));
        assert!(!node_matches(&app, &TREE[1], "m-101"));
        assert!(!TREE.iter().any(|node| node_matches(&app, node, "no such view")));
        // a renamed level is found by the name its view shows (R1)
        assert!(node_matches(&app, &TREE[0].children[0].children[0], "level 1"), "the tree's first plan is Level 1");
    }

    #[test]
    fn selection_edit_targets_duct_and_is_undoable() {
        let mut app = Desktop::sample();
        let initial = app.document.revision();
        app.select_element(Some(crate::ElementId(2)));
        app.width_mm = 600.0;
        app.apply_duct_edit();
        assert_eq!(app.document.revision(), initial);
        app.select_element(Some(crate::ElementId(1)));
        app.width_mm = 600.0;
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
        for selection in [None, Some(SampleElement::Duct), Some(SampleElement::Model(crate::ElementId(20))), Some(SampleElement::Model(crate::ElementId(10))), Some(SampleElement::Model(crate::ElementId(100)))] {
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
