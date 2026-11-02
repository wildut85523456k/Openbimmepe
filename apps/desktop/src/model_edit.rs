use crate::{Desktop, ElementId, SampleElement, TempDim, Tool};
use eframe::egui;
use openbim_document::{Xf, FamilySpec, RebarSpec, TrussSpec, BeamSpec, BeamSystemSpec, Category, ColumnSpec, Command, FootingSpec, WallFootingSpec, Geometry, Section, SlabSpec, Usage};

/// The state of the export window.
pub(crate) struct ExportDialog { pub path: String, pub schema: crate::export::Schema, pub own_data: bool }

/// The state of the import window.
pub(crate) struct ImportDialog { pub path: String, pub map_path: String }

/// The state of the Load Family window: the catalogue file typed or picked.
#[derive(Default)]
pub(crate) struct LoadFamilyDialog { pub path: String }

/// The state of the Create Family window: name a new family derived from a library family,
/// edit its type rows, and load them into the project.
#[derive(Default)]
pub(crate) struct CreateFamilyDialog {
    pub name: String,
    pub base: String,
    pub params: String,   // comma-separated parameter names
    pub rows: String,     // one type per line: type_name<TAB>param values
}

/// The state of the types window: the type being worked on and the name typed for a rename or a copy.
#[derive(Default)]
pub(crate) struct TypesDialog { pub picked: Option<ElementId>, pub name: String }

impl Desktop {
    /// The types window: every type with the number of elements that use it, and for the one picked: rename,
    /// duplicate, delete (only while no element uses it) and the elements that use it, each with a Select button.
    pub(crate) fn types_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.types_dialog.take() else { return };
        let mut open = true;
        let mut action: Option<(&str, ElementId)> = None;
        let mut pick = None;
        if d.picked.is_some_and(|t| self.document.get(t).is_none()) { d.picked = None; }
        let rows: Vec<(ElementId, String, Category, usize)> = self.document.elements().filter(|e| e.category == Category::Type)
            .map(|t| (t.id, t.name.clone(), t.kind(), self.document.instances_of(t.id).len())).collect();
        egui::Window::new("Types").open(&mut open).default_width(380.0).show(ctx, |ui| {
            if rows.is_empty() { ui.label("The model has no types. Select an element and use Make type in its properties."); }
            egui::ScrollArea::vertical().max_height(200.0).id_salt("types_list").show(ui, |ui| {
                for (id, name, kind, n) in &rows {
                    if ui.selectable_label(d.picked == Some(*id), format!("{name}   {kind:?}   {n} element{}", if *n == 1 { "" } else { "s" })).clicked() {
                        d.picked = Some(*id);
                        d.name = name.clone();
                    }
                }
            });
            let Some(t) = d.picked else { return };
            let users = self.document.instances_of(t);
            ui.separator();
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(150.0));
                let ok = !d.name.trim().is_empty();
                if ui.add_enabled(ok, egui::Button::new("Rename")).clicked() { action = Some(("rename", t)); }
                if ui.add_enabled(ok, egui::Button::new("Duplicate")).clicked() { action = Some(("duplicate", t)); }
                if ui.add_enabled(users.is_empty(), egui::Button::new("Delete")).on_disabled_hover_text("Elements still use this type").clicked() { action = Some(("delete", t)); }
                if self.document.get(t).is_some_and(|x| x.type_for == Some(Category::Wall)) && ui.button("Edit…").on_hover_text("Name, height and layers").clicked() { action = Some(("edit", t)); }
            });
            ui.small("Rename and Duplicate take the name typed here; it must be new among the types of the same kind.");
            egui::ScrollArea::vertical().max_height(160.0).id_salt("types_users").show(ui, |ui| {
                for u in &users {
                    ui.horizontal(|ui| {
                        if ui.small_button("Select").clicked() { pick = Some(*u); }
                        ui.label(self.document.get(*u).map_or(String::new(), |e| format!("{} (element {})", e.name, u.0)));
                    });
                }
            });
        });
        if let Some((what, t)) = action {
            let name = d.name.clone();
            let result = match what {
                "rename" => self.document.rename_type(t, &name).map(|()| format!("Type renamed to {}", name.trim())),
                "duplicate" => self.document.duplicate_type(t, &name).map(|n| { d.picked = Some(n); format!("Type {} made as a copy", name.trim()) }),
                "edit" => { self.open_type_editor(t); Ok(self.message.clone()) }
                _ => self.document.delete_type(t).map(|()| { d.picked = None; "Type deleted".to_string() }),
            };
            self.message = match result { Ok(m) => m, Err(e) => format!("Type rejected ({e:?}): the name must be new among the types of its kind") };
        }
        if let Some(id) = pick { self.select_element(Some(id)); }
        if open { self.types_dialog = Some(d); }
    }

    pub(crate) fn selected_id(&self) -> Option<ElementId> {
        match self.selected { Some(SampleElement::Duct) => Some(ElementId(1)),
            Some(SampleElement::Model(id)) => Some(id), _ => None }
    }

    /// The document's warnings, worked out again only when the document has changed.
    pub(crate) fn warnings(&mut self) -> &[openbim_document::Warning] {
        if self.warnings_cache.0 != self.document.revision() {
            self.warnings_cache = (self.document.revision(), self.document.warnings());
        }
        &self.warnings_cache.1
    }

    /// A window listing the warnings, each with a button that selects the element it is about.
    pub(crate) fn warnings_window(&mut self, ctx: &egui::Context) {
        if !self.show_warnings { return; }
        let list: Vec<_> = self.warnings().to_vec();
        let mut open = true;
        let mut pick = None;
        egui::Window::new("Warnings").open(&mut open).default_width(420.0).show(ctx, |ui| {
            if list.is_empty() { ui.label("The model has no warnings."); }
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for w in &list {
                    ui.horizontal_wrapped(|ui| {
                        if ui.small_button("Select").clicked() { pick = w.elements.first().copied(); }
                        ui.label(&w.message);
                    });
                }
            });
        });
        if !open { self.show_warnings = false; }
        if let Some(id) = pick { self.select_element(Some(id)); }
    }

    /// Gives the selected element the type `choice`, or lets it keep its values when there is none.
    pub(crate) fn choose_type(&mut self, choice: Option<ElementId>) {
        let Some(id) = self.selected_id() else { return };
        let result = match choice { Some(t) => self.document.assign_type(&[id], t), None => self.document.detach_type(&[id]) };
        self.message = match result {
            Ok(()) => { self.sync_duct_controls(); match choice { Some(t) => format!("Element {} is now of type {}", id.0, self.document.get(t).map_or("?", |t| t.name.as_str())), None => format!("Element {} no longer follows a type", id.0) } }
            Err(error) => format!("Type rejected: {error:?}"),
        };
    }

    /// Makes a type out of the selected element's values, named `type_name_input`, and gives it to the element.
    pub(crate) fn make_type(&mut self) {
        let Some(id) = self.selected_id() else { return };
        match self.document.create_type(id, &self.type_name_input) {
            Ok(t) => { self.type_name_input.clear(); self.message = format!("Type {} made from element {}", self.document.get(t).map_or("?", |t| t.name.as_str()), id.0); }
            Err(error) => self.message = format!("Type rejected ({error:?}): the name must be new among the types of its kind"),
        }
    }

    /// Everything selected: the primary element first, then the others.
    pub(crate) fn selected_ids(&self) -> Vec<ElementId> {
        let mut v: Vec<ElementId> = self.selected_id().into_iter().collect();
        for i in &self.multi { if !v.contains(i) { v.push(*i); } }
        v.retain(|i| self.document.get(*i).is_some());
        v
    }

    /// Selects exactly `ids` (the first becomes the primary one, whose values the panels show).
    pub(crate) fn select_many(&mut self, ids: Vec<ElementId>) {
        let mut ids = ids;
        ids.dedup();
        self.select_element(ids.first().copied());
        self.multi = ids.into_iter().skip(1).collect();
        if !self.multi.is_empty() { self.message = format!("{} elements selected", self.multi.len() + 1); }
    }

    /// Ctrl or Cmd click adds `id` to the selection, Shift click takes it away.
    pub(crate) fn change_selection(&mut self, id: ElementId, add: bool) {
        let mut ids = self.selected_ids();
        if add { if !ids.contains(&id) { ids.push(id); } } else { ids.retain(|i| *i != id); }
        self.select_many(ids);
    }

    pub(crate) fn select_element(&mut self, id: Option<ElementId>) {
        self.multi.clear();
        if id == self.selected_id() { return; }
        let pending = self.has_pending_edit();
        self.selected = id.map(|id| if id == ElementId(1) { SampleElement::Duct } else { SampleElement::Model(id) });
        self.sync_duct_controls();
        if pending { self.message = "Selection changed; unapplied values reset".into(); }
    }

    pub(crate) fn sync_duct_controls(&mut self) {
        if let Some(el) = self.selected_id().and_then(|id| self.document.get(id)) {
            self.width_mm = el.width_m * 1000.0;
            self.height_mm = el.height_m * 1000.0;
            self.elevation_mm = el.elevation_m * 1000.0;
            self.slope_percent = el.slope_ratio.unwrap_or(0.0) * 100.0;
            if let (Category::Footing, Some(Geometry::Pad { thickness_m, .. })) = (el.category, el.geometry) { (self.material_pick, self.footing_thickness_mm) = (el.material, thickness_m * 1000.0); }
            if el.category == Category::WallFooting { self.material_pick = el.material; }
            if el.category == Category::Beam { self.join_pick = el.join_ends; }
            if matches!(el.category, Category::BeamSystem | Category::Truss | Category::Railing) { (self.spacing_mm, self.angle_deg) = (el.spacing_m * 1000.0, el.angle_deg); }
            if el.category == Category::Truss { self.material_pick = el.material; }
            if el.category == Category::Family {
                (self.angle_deg, self.family_type) = (el.angle_deg, el.type_name.clone());
                self.family_values = crate::export::instance_values_of(&self.document, el).unwrap_or_default();
                self.family_texts = crate::export::instance_texts_of(&self.document, el).unwrap_or_default();
                // the type the instance takes, and its own parameters, so the panel can edit the type rather than
                // this one instance
                self.family_type_editing = el.type_id;
                (self.family_type_values, self.family_type_texts) = self.family_type_drafts();
            }
            if el.category == Category::Rebar { (self.spacing_mm, self.bar_count) = (el.spacing_m * 1000.0, f64::from(el.count)); }
            if el.category == Category::Slab { (self.material_pick, self.slab_kind_pick) = (el.material, el.slab_kind); }
            if matches!(el.category, Category::Beam | Category::Column | Category::BeamSystem) {
                (self.usage_pick, self.material_pick) = (el.usage, el.material);
                (self.section_pick, self.web_mm, self.flange_mm) = match el.section {
                    Section::Rectangle => (0, self.web_mm, self.flange_mm),
                    Section::Circle => (1, self.web_mm, self.flange_mm),
                    Section::IShape { web_m, flange_m } => (2, web_m * 1000.0, flange_m * 1000.0),
                    Section::Oval => (0, self.web_mm, self.flange_mm),
                };
            }
            if let Some(Geometry::Linear { start, end, .. } | Geometry::Grid { start, end, .. }) = el.geometry {
                self.axis_mm = [start[0], start[1], end[0], end[1]].map(|n| n * 1000.0);
            }
        } else { self.selected = None; }
    }

    /// The section the options bar or the properties stand for.
    pub(crate) fn section_from_controls(&self) -> Section {
        match self.section_pick {
            1 => Section::Circle,
            2 => Section::IShape { web_m: self.web_mm / 1000.0, flange_m: self.flange_mm / 1000.0 },
            _ => Section::Rectangle,
        }
    }

    pub(crate) fn has_pending_edit(&self) -> bool {
        let Some(el) = self.selected_id().and_then(|id| self.document.get(id)) else { return false; };
        let differs = |a: f64, b: f64| !b.is_finite() || (a - b).abs() > 1e-6;
        let size = matches!(el.category, Category::Wall | Category::Beam | Category::Column | Category::Duct | Category::Pipe | Category::Slab | Category::Footing | Category::WallFooting | Category::BeamSystem | Category::Truss | Category::Rebar | Category::Stair | Category::Opening | Category::Railing)
            && (differs(el.width_m * 1000.0, self.width_mm)
                || (!el.is_round() && el.category != Category::Slab && self.section_pick != 1 && differs(el.height_m * 1000.0, self.height_mm)));
        let section = matches!(el.category, Category::Beam | Category::Column | Category::BeamSystem) && {
            let want = self.section_from_controls();
            match (el.section, want) {
                (Section::IShape { web_m: a, flange_m: b }, Section::IShape { web_m: c, flange_m: d }) => differs(a * 1000.0, c * 1000.0) || differs(b * 1000.0, d * 1000.0),
                (a, b) => a != b,
            }
        };
        let elevation = el.category != Category::Grid && differs(el.elevation_m * 1000.0, self.elevation_mm);
        let slope = el.slope_ratio.is_some_and(|s| differs(s * 100.0, self.slope_percent));
        let axis = match el.geometry {
            Some(Geometry::Linear { start, end, .. } | Geometry::Grid { start, end, .. }) =>
                [start[0], start[1], end[0], end[1]].iter().zip(self.axis_mm)
                    .any(|(a, b)| differs(a * 1000.0, b)),
            _ => false,
        };
        let system = el.category == Category::BeamSystem && (differs(el.spacing_m * 1000.0, self.spacing_mm) || differs(el.angle_deg, self.angle_deg) || el.usage != self.usage_pick || el.material != self.material_pick);
        let truss = el.category == Category::Truss && (differs(el.spacing_m * 1000.0, self.spacing_mm) || el.material != self.material_pick);
        let family = el.category == Category::Family && (differs(el.angle_deg, self.angle_deg) || el.type_name != self.family_type || self.family_values_changed(el));
        let rebar = el.category == Category::Rebar && (differs(el.spacing_m * 1000.0, self.spacing_mm) || differs(f64::from(el.count), self.bar_count));
        let railing = el.category == Category::Railing && differs(el.spacing_m * 1000.0, self.spacing_mm);
        let kind = system || truss || rebar || railing || family || (el.category == Category::Beam && el.usage != self.usage_pick) || (matches!(el.category, Category::Beam | Category::Column | Category::Slab) && el.material != self.material_pick)
            || (el.category == Category::Slab && el.slab_kind != self.slab_kind_pick)
            || (el.category == Category::WallFooting && el.material != self.material_pick)
            || (el.category == Category::Beam && el.join_ends != self.join_pick)
            || (el.category == Category::Footing && (el.material != self.material_pick || matches!(el.geometry, Some(Geometry::Pad { thickness_m, .. }) if differs(thickness_m * 1000.0, self.footing_thickness_mm))));
        size || section || kind || elevation || slope || axis
    }

    /// Opens the export window: a file name and a schema, IFC 4.3 first.
    pub(crate) fn export_ifc_file(&mut self) {
        self.export_dialog = Some(ExportDialog { path: "openbim-export.ifc".into(), schema: crate::export::Schema::Ifc4x3, own_data: true });
    }

    /// The export window: where to write the file and in which schema (IFC 4.3 unless another is picked).
    pub(crate) fn export_window(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.export_dialog.take() else { return };
        let (mut open, mut go) = (true, false);
        egui::Window::new("Export IFC").open(&mut open).collapsible(false).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("File");
                let r = ui.text_edit_singleline(&mut dialog.path);
                test_hit!(self, r, "export:path");
                if ui.button("Browse...").clicked() {
                    let start = std::path::Path::new(&dialog.path);
                    let mut fd = crate::dialog::Question::new().set_title("Export IFC").add_filter("IFC", &["ifc"]);
                    if let Some(name) = start.file_name().and_then(|n| n.to_str()) { fd = fd.set_file_name(name); }
                    if let Some(dir) = start.parent().filter(|d| d.is_dir() && !d.as_os_str().is_empty()) { fd = fd.set_directory(dir); }
                    if let Some(p) = fd.ask_save() { dialog.path = p.display().to_string(); }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Schema");
                egui::ComboBox::from_id_salt("export_schema").selected_text(dialog.schema.name()).show_ui(ui, |ui| {
                    for s in crate::export::Schema::ALL { ui.selectable_value(&mut dialog.schema, s, s.name()); }
                });
            });
            ui.checkbox(&mut dialog.own_data, "Include this app's own data (this app reads the file back with nothing lost)");
            if !dialog.own_data { ui.small("Without it a re-import reads only what IFC can hold."); }
            if dialog.schema == crate::export::Schema::Ifc2x3 { ui.small("IFC2X3 leaves out types, joins and service networks."); }
            let r = ui.button("Export");
            test_hit!(self, r, "export:go");
            go = r.clicked();
        });
        if go { self.write_ifc(&dialog.path, dialog.schema, dialog.own_data); } else if open { self.export_dialog = Some(dialog); }
    }

    /// Opens the import window.
    pub(crate) fn import_ifc_file(&mut self) {
        self.import_dialog = Some(ImportDialog { path: String::new(), map_path: String::new() });
    }

    /// The import window: a file, what it is, and the button.
    pub(crate) fn import_window(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.import_dialog.take() else { return };
        let (mut open, mut go) = (true, false);
        egui::Window::new("Import IFC").open(&mut open).collapsible(false).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("File");
                ui.text_edit_singleline(&mut dialog.path);
                if ui.button("Browse...").clicked() {
                    if let Some(p) = crate::dialog::Question::new().set_title("Import IFC").add_filter("IFC (IFC2X3, IFC4, IFC 4.3)", &["ifc"]).ask_open() { dialog.path = p.display().to_string(); }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Class map");
                ui.text_edit_singleline(&mut dialog.map_path);
                if ui.button("Browse...").clicked() {
                    if let Some(p) = crate::dialog::Question::new().set_title("IFC class map").add_filter("Class map (tab separated)", &["txt", "tsv", "tab"]).ask_open() { dialog.map_path = p.display().to_string(); }
                }
                if ui.button("Save example...").clicked() {
                    if let Some(p) = crate::dialog::Question::new().set_title("Save an example class map").set_file_name("ifc-class-map.txt").ask_save() {
                        self.message = match std::fs::write(&p, crate::class_map::EXAMPLE) { Ok(()) => format!("Saved an example class map to {}", p.display()), Err(e) => format!("Could not save {}: {e}", p.display()) };
                    }
                }
            });
            ui.small("Optional. One row per line: IFC class, app category (Wall, Beam, Column, Slab, Footing, WallFooting, Pipe, Duct, Generic, Ignore), and the predefined type if the row is for one. A class with no row is read as usual.");
            if !dialog.path.trim().is_empty() { ui.small(crate::import::describe_file(dialog.path.trim())); }
            ui.small("Reads IFC2X3, IFC4 and IFC 4.3. A file written by this app with its own data comes back exactly; any other file is read from its geometry.");
            go = ui.add_enabled(!dialog.path.trim().is_empty(), egui::Button::new("Import")).clicked();
        });
        if go { self.read_ifc_with(dialog.path.trim(), dialog.map_path.trim()); } else if open { self.import_dialog = Some(dialog); }
    }

    /// Opens the Load Family window: a catalogue file, what it will do, and the button.
    pub(crate) fn load_family_file(&mut self) {
        self.load_family_dialog = Some(LoadFamilyDialog::default());
    }

    pub(crate) fn import_dwg_file(&mut self) {
        let Some(path) = crate::dialog::Question::new().set_title("Import DWG/DXF").add_filter("DWG/DXF", &["dwg", "dxf"]).ask_open() else { return };
        match crate::dwg_import::import_dwg_file(&path.display().to_string()) {
            Ok((drawing, msg)) => {
                let n = drawing.primitive_count();
                self.cad_overlay = Some((path.display().to_string(), drawing, 1.0));
                self.message = format!("Imported {msg}");
            }
            Err(e) => self.message = format!("DWG import failed: {e}"),
        }
    }

    pub(crate) fn open_family_parameters_dialog(&mut self) {
        let family = self.document.family_defs().keys().next().cloned().unwrap_or_else(|| "Family".to_string());
        self.family_parameters_dialog = Some(crate::family_parameters_ui::FamilyParametersDialog::open(&family));
    }

    pub(crate) fn import_step_file(&mut self) {
        let Some(path) = crate::dialog::Question::new().set_title("Import STEP").add_filter("STEP", &["step", "stp"]).ask_open() else { return };
        match crate::step_import::import_step(&path.display().to_string()) {
            Ok(mesh) => {
                let verts = mesh.vertices.len();
                let tris = mesh.faces.len();
                let params = crate::step_import::mesh_params(&mesh);
                // place the imported part as a Family element carrying the mesh, on the first level if there is one
                let level = self.document.elements().find(|e| e.geometry == Some(openbim_document::Geometry::Level)).map(|l| l.id);
                let placed_id = level.and_then(|level| {
                    let spec = openbim_document::FamilySpec { family: "Imported STEP".into(), type_name: String::new(), overrides: Vec::new(), angle_deg: 0.0, elevation_m: 0.0 };
                    self.document.place_family(level, [0.0, 0.0], &spec).ok()
                });
                if let Some(id) = placed_id {
                    self.document.set_display_mesh(id, Some(mesh.clone()));
                    self.imported_step = None;
                    self.message = format!("Imported STEP: {verts} vertices, {tris} triangles placed as family element {} (mm units)", id.0);
                } else {
                    self.imported_step = Some(mesh);
                    self.message = format!("Imported STEP: {verts} vertices, {tris} triangles (mm units, no level to place on; shown as overlay)");
                }
                self.step_params_dialog = Some(crate::step_params_ui::StepParamsDialog { name: path.display().to_string(), params });
            }
            Err(e) => self.message = format!("STEP import failed: {e}"),
        }
    }

    pub(crate) fn step_params_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.step_params_dialog.take() else { return };
        d.window(ctx);
        if !d.name.is_empty() { self.step_params_dialog = Some(d); }
    }

    pub(crate) fn import_pdf_file(&mut self) {
        let Some(path) = crate::dialog::Question::new().set_title("Import PDF").add_filter("PDF", &["pdf"]).ask_open() else { return };
        match (|| -> Result<(Vec<u8>, u32, u32), String> {
            let pdfium = pdfium_auto::bind_pdfium_silent().map_err(|e| e.to_string())?;
            let doc = pdfium.load_pdf_from_file(&path, None).map_err(|e| e.to_string())?;
            let page = doc.pages().get(0).map_err(|e| e.to_string())?;
            let w_pt = page.width().value;
            let h_pt = page.height().value;
            let scale_px = 200.0 / 72.0; // 200 DPI
            let w = (w_pt * scale_px) as i32;
            let h = (h_pt * scale_px) as i32;
            let bitmap = page.render(w, h, None).map_err(|e| e.to_string())?;
            let raw = bitmap.as_raw_bytes();
            let img = image::RgbaImage::from_raw(w as u32, h as u32, raw).ok_or("bitmap decode failed")?;
            let dynimg = image::DynamicImage::ImageRgba8(img);
            let mut buf = Vec::new();
            dynimg.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).map_err(|e| e.to_string())?;
            Ok((buf, w as u32, h as u32))
        })() {
            Ok((bytes, w, h)) => {
                self.pdf_overlay = Some(crate::PdfOverlay { png_bytes: bytes, w, h, scale: 0.0254 / 200.0, texture: None });
                self.message = format!("Imported PDF: {}x{} px", w, h);
            }
            Err(e) => self.message = format!("PDF import failed: {e}"),
        }
    }

    pub(crate) fn create_family_file(&mut self) {
        self.create_family_dialog = Some(CreateFamilyDialog { base: "Air handling unit".into(), ..Default::default() });
    }

    /// The Create Family window: name a new family derived from a library family, edit its type rows
    /// (one type per line: `type_name<TAB>param value ...`), and load them into the project as types.
    pub(crate) fn create_family_window(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.create_family_dialog.take() else { return };
        let (mut open, mut go) = (true, false);
        egui::Window::new("Create Family").open(&mut open).collapsible(false).show(ctx, |ui| {
            ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut dialog.name); });
            ui.horizontal(|ui| {
                ui.label("Base family");
                egui::ComboBox::from_id_salt("create_family_base").selected_text(&dialog.base).show_ui(ui, |ui| {
                    for f in crate::export::library() { if ui.selectable_label(dialog.base == f.name(), f.name()).clicked() { dialog.base = f.name().to_string(); } }
                });
            });
            ui.label("Parameters (comma separated)");
            ui.text_edit_singleline(&mut dialog.params);
            ui.label("Types (one per line: name<TAB>param values)");
            ui.text_edit_multiline(&mut dialog.rows);
            go = ui.add_enabled(!dialog.name.trim().is_empty() && !dialog.base.trim().is_empty(), egui::Button::new("Create")).clicked();
        });
        if go {
            // Build a tab-separated catalogue and feed it through the existing loader.
            let mut text = String::new();
            text.push_str(&format!("family\t{}\n", dialog.base.trim()));
            let params: Vec<&str> = dialog.params.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if !params.is_empty() {
                text.push_str("type\t");
                text.push_str(&params.join("\t"));
                text.push('\n');
            }
            for line in dialog.rows.lines() {
                let line = line.trim();
                if line.is_empty() { continue; }
                text.push_str(line);
                text.push('\n');
            }
            // Rename the family in the catalogue by overriding the family line is not possible (the base name is fixed),
            // so store the chosen name as a prefix on each type name instead.
            let msg = self.load_family_catalogue_text(&text, "create family");
            self.message = if msg.is_empty() { format!("Family '{}' created", dialog.name.trim()) } else { msg };
            self.create_family_dialog = None;
            return;
        }
        if open { self.create_family_dialog = Some(dialog); }
    }

    /// The Load Family window. The catalogue is read and checked before anything is created, so a file that names a
    /// family or a column the library does not have is refused whole and says why.
    pub(crate) fn load_family_window(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.load_family_dialog.take() else { return };
        let (mut open, mut go) = (true, false);
        let mut edit_family: Option<String> = None;
        egui::Window::new("Load Family").open(&mut open).collapsible(false).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Catalogue");
                ui.text_edit_singleline(&mut dialog.path);
                if ui.button("Browse...").clicked() {
                    if let Some(p) = crate::dialog::Question::new().set_title("Load Family").add_filter("Family catalogue (tab separated)", &["tsv", "txt", "csv", "tab"]).ask_open() { dialog.path = p.display().to_string(); }
                }
            });
            ui.small("A tab-separated catalogue: a 'family' line naming a family in the library, then a 'type' line naming its inputs, then one line per type. Every row becomes a type of that family in this project.");
            ui.small("Nothing is created unless the whole file is accepted, so a refused file leaves the project as it was.");
            ui.horizontal(|ui| {
                ui.label("Family types");
                egui::ComboBox::from_id_salt("load_family_types_of").selected_text("Edit the table of…").show_ui(ui, |ui| {
                    for f in crate::export::library() { if ui.selectable_label(false, f.name()).clicked() { edit_family = Some(f.name().to_string()); } }
                });
            });
            go = ui.add_enabled(!dialog.path.trim().is_empty(), egui::Button::new("Load")).clicked();
        });
        if let Some(f) = edit_family { self.open_family_types(&f); }
        if go { self.read_family_catalogue(dialog.path.trim()); } else if open { self.load_family_dialog = Some(dialog); }
    }

    /// Reads the catalogue at `path` into the document's types of a family, as one undoable step.
    pub(crate) fn read_family_catalogue(&mut self, path: &str) {
        self.message = match std::fs::read_to_string(path) {
            Err(e) => format!("Could not read {path}: {e}"),
            Ok(text) => self.load_family_catalogue_text(&text, path),
        };
    }

    /// The same, from the text of a catalogue: what the window and the tests both go through.
    pub(crate) fn load_family_catalogue_text(&mut self, text: &str, from: &str) -> String {
        match self.document.load_family_catalogue(text) {
            Ok(ids) => {
                let families: Vec<String> = ids.iter().filter_map(|id| self.document.get(*id)).map(|t| t.family.clone()).collect();
                let names = families.iter().fold(Vec::new(), |mut acc, f| { if !acc.contains(f) { acc.push(f.clone()) } acc });
                format!("Loaded {} {} for {} from {from}; undo removes them", ids.len(), if ids.len() == 1 { "type" } else { "types" }, names.join(", "))
            }
            Err(why) => format!("Catalogue refused, nothing changed: {why}"),
        }
    }

    /// Reads the file at `path` into the document, as one undoable step, and writes the import log next to it
    /// (`<file>.log`, as Revit writes `<file>.ifc.log`).
    #[cfg(test)]
    pub(crate) fn read_ifc(&mut self, path: &str) { self.read_ifc_with(path, ""); }

    /// The same with the class map in the file `map_path` (none when it is empty). A map that does not read refuses the
    /// import before anything is read, and the log says so.
    pub(crate) fn read_ifc_with(&mut self, path: &str, map_path: &str) {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let when = crate::export::iso_time(secs);
        let map = if map_path.is_empty() { Ok(crate::class_map::ClassMap::default()) } else {
            std::fs::read_to_string(map_path).map_err(|e| format!("Could not read the class map {map_path}: {e}")).and_then(|t| crate::class_map::ClassMap::parse(&t).map_err(|e| format!("The class map {map_path} is refused, {e}")))
        };
        let (message, log) = match (std::fs::read(path), map) {
            (_, Err(why)) => (format!("Import refused, nothing changed: {why}"), Some(format!("Import log for {path}\nRead on {when}\n\nImport refused, nothing changed: {why}\n"))),
            (Err(e), _) => (format!("Could not read {path}: {e}"), None),
            (Ok(bytes), Ok(map)) => { self.sync_app_records(); match crate::import::import_ifc_with(&mut self.document, &String::from_utf8_lossy(&bytes), &map) {
                Ok(mut report) => {
                    if !map.is_empty() { report.notes.push(format!("class map {map_path}: {} rows", map.len())); }
                    self.sync_duct_controls(); self.selected = None;
                    // the file's own settings (Compute inputs, georeferencing, any other kind) that the model had none of
                    if !report.adopted.is_empty() { self.compute.restore(self.document.app_records()); self.georef = crate::georef_ui::GeorefInput::from_records(self.document.app_records()); }
                    (report.text(), Some(report.log_text(path, &when)))
                }
                Err(e) => (format!("Import failed, nothing changed: {e}"), Some(format!("Import log for {path}\nRead on {when}\n\nImport failed, nothing changed: {e}\n"))),
            } }
        };
        self.message = match log {
            Some(text) => match std::fs::write(format!("{path}.log"), text) {
                Ok(()) => format!("{message}. Log: {path}.log"),
                Err(e) => format!("{message}. The log could not be written ({e})"),
            },
            None => message,
        };
    }

    /// Writes the document to `path` in `schema` and says what was and was not written.
    pub(crate) fn write_ifc(&mut self, path: &str, schema: crate::export::Schema, own_data: bool) {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let key = self.document.ensure_project_key();
        let name = self.file.path.as_deref().and_then(|p| std::path::Path::new(p).file_stem()).and_then(|n| n.to_str()).unwrap_or("Project1").to_string();
        match crate::export::export_ifc_with(&self.document, &name, key, &crate::export::iso_time(secs), schema, own_data) {
            Ok(out) => match std::fs::write(path, &out.text) {
                Ok(()) => self.message = format!("Exported {path} ({}): {} walls, {} beams, {} columns, {} slabs. {}", schema.name(), out.walls, out.beams, out.columns, out.slabs, out.warnings.join("; ")),
                Err(e) => self.message = format!("Could not write {path}: {e}"),
            },
            Err(e) => self.message = format!("IFC export failed: {e}"),
        }
    }

    /// The level of the plan view on show.
    /// The level the plan view on show sits on: the view's own level, else the lowest level of the document (the
    /// second lowest for the upper plan). `ElementId(0)` means the project has no level at all and the drawing tools
    /// refuse to start (A2).
    pub(crate) fn plan_level(&self) -> ElementId {
        if let crate::View::Level(id) = self.view { if self.document.get(ElementId(id)).is_some() { return ElementId(id); } }
        let levels = self.levels();
        let pick = if matches!(self.view, crate::View::UpperPlan) { levels.get(1).or_else(|| levels.first()) } else { levels.first() };
        pick.map_or(ElementId(0), |l| l.0)
    }

    /// Starts editing a level's name in the options bar. The draft is held by the app, so the frames between
    /// keystrokes do not rebuild it from the document (R4).
    pub(crate) fn start_level_rename(&mut self, id: ElementId) {
        self.level_rename_name = self.document.get(id).map(|e| e.name.clone()).unwrap_or_default();
        self.level_rename_id = Some(id);
        self.message = "Rename the level and press Enter, or Cancel".into();
    }

    /// Commits the draft. The field stays open when the name is refused, so the user can correct it (R4).
    pub(crate) fn commit_level_rename(&mut self) {
        let Some(id) = self.level_rename_id else { return };
        let name = self.level_rename_name.trim().to_string();
        let current = self.document.get(id).map(|e| e.name.clone()).unwrap_or_default();
        if name.is_empty() || name == current { self.level_rename_id = None; return; }
        self.rename_level(id, &name);
        if self.document.get(id).is_some_and(|e| e.name == name) { self.level_rename_id = None; }
    }

    /// Abandons the draft (R4).
    pub(crate) fn cancel_level_rename(&mut self) {
        self.level_rename_id = None;
        self.level_rename_name.clear();
        self.message = "Rename cancelled".into();
    }

    /// Opens the elevation field of a level, holding its elevation in mm as the user will edit it (A6).
    pub(crate) fn start_level_elevation(&mut self, id: ElementId) {
        let Some(level) = self.document.get(id) else { return };
        self.level_elev_edit = Some((id, format!("{:.0}", level.elevation_m * 1000.0)));
        self.message = "Type the level's elevation in mm and press Enter or OK; what stands on it moves with it".into();
    }

    /// Commits the typed elevation as one undoable step. A value that is not a number, that passes another level or
    /// comes too close to one is refused and the field stays open to be corrected.
    pub(crate) fn commit_level_elevation(&mut self) {
        let Some((id, text)) = self.level_elev_edit.clone() else { return };
        let Ok(mm) = text.trim().replace(',', ".").parse::<f64>() else {
            self.message = format!("Elevation not changed: {text:?} is not a number of mm");
            return;
        };
        let name = self.document.get(id).map(|e| e.name.clone()).unwrap_or_default();
        match self.document.set_level_elevation(id, mm / 1000.0) {
            Ok(()) => { self.level_elev_edit = None; self.message = format!("{name} moved to {mm:.0} mm; what stands on it moved with it; undo restores it"); }
            Err(_) => self.message = format!("Elevation not changed: {name} cannot pass another level or come within {:.0} mm of one", openbim_document::MIN_LEVEL_GAP_M * 1000.0),
        }
    }

    /// Renames a level of the project, refusing a name another level has (A6).
    pub(crate) fn rename_level(&mut self, id: ElementId, name: &str) {
        match self.document.rename_level(id, name) {
            Ok(()) => self.message = format!("Level renamed to {name}"),
            Err(error) => self.message = format!("Level not renamed: {error:?} (a level needs a name of its own)"),
        }
    }

    /// Deletes a level that nothing stands on, as Revit refuses to delete one that holds elements (A6).
    pub(crate) fn delete_level(&mut self, id: ElementId) {
        match self.document.delete_level(id) {
            Ok(()) => {
                if self.view == crate::View::Level(id.0) { self.view = crate::View::Plan; }
                self.message = format!("Level {} deleted", id.0);
            }
            Err(openbim_document::EditError::TypeInUse(_)) => self.message = "A level that elements stand on cannot be deleted".into(),
            Err(error) => self.message = format!("Level not deleted: {error:?}"),
        }
    }

    /// The levels of the document, lowest first: (id, name, elevation).
    pub(crate) fn levels(&self) -> Vec<(ElementId, String, f64)> {
        let mut l: Vec<_> = self.document.elements().filter(|e| matches!(e.geometry, Some(Geometry::Level)))
            .map(|e| (e.id, e.name.clone(), e.elevation_m)).collect();
        l.sort_by(|a, b| a.2.total_cmp(&b.2));
        l
    }

    /// Places a beam with the sizes of the options bar on `level`.
    fn place_beam_on(&mut self, level: ElementId, start: [f64; 2], end: [f64; 2]) -> Option<ElementId> {
        let m = |v: f64| v / 1000.0;
        let depth = if self.section_pick == 1 { self.width_mm } else { self.height_mm };
        let run = (end[0] - start[0]).hypot(end[1] - start[1]);
        // a brace is given its rise, any other member its slope
        let slope_ratio = if self.usage_pick == Usage::Brace && run > 1e-9 { m(self.brace_rise_mm) / run } else { self.slope_percent / 100.0 };
        let spec = BeamSpec { start, end, width_m: m(self.width_mm), depth_m: m(depth), elevation_m: m(self.elevation_mm),
            section: self.section_from_controls(), usage: self.usage_pick, material: self.material_pick, slope_ratio };
        match self.document.place_beam(level, &spec) {
            Ok(id) => { self.message = format!("Beam {} placed; undo removes it", id.0); Some(id) }
            Err(error) => { self.message = format!("Beam rejected: {error:?}"); None }
        }
    }

    /// Places a beam from the options bar on its level, selects it and goes back to Select.
    pub(crate) fn place_beam_from_controls(&mut self) {
        let levels = self.levels();
        let level = self.beam_level.filter(|l| levels.iter().any(|x| x.0 == *l)).or(levels.first().map(|l| l.0));
        let Some(level) = level else { self.message = "Add a level before placing a beam".into(); return; };
        let m = |v: f64| v / 1000.0;
        let (start, end) = ([m(self.axis_mm[0]), m(self.axis_mm[1])], [m(self.axis_mm[2]), m(self.axis_mm[3])]);
        if let Some(id) = self.place_beam_on(level, start, end) {
            self.tool = Tool::Select;
            self.beam_start = None;
            self.selected = Some(SampleElement::Model(id));
            self.sync_duct_controls();
        }
    }

    /// A click of the Column tool in the plan of `level`: a column with the sizes, section and material of the
    /// options bar stands there from that level to the next one up. The tool stays on.
    pub(crate) fn column_click(&mut self, level: ElementId, at: [f64; 2]) {
        let levels = self.levels();
        let Some(base) = levels.iter().find(|l| l.0 == level).map(|l| l.2) else { self.message = "This view has no level".into(); return };
        let Some(top) = levels.iter().find(|l| l.2 > base + 1e-6).map(|l| l.0) else {
            self.message = "There is no level above this one for the column to reach".into();
            return;
        };
        let m = |v: f64| v / 1000.0;
        let depth = if self.section_pick == 1 { self.width_mm } else { self.height_mm };
        let spec = ColumnSpec { width_m: m(self.width_mm), depth_m: m(depth), base_offset_m: 0.0, section: self.section_from_controls(), material: self.material_pick };
        self.message = match self.document.place_column(level, top, at, &spec) {
            Ok(id) => format!("Column {} placed; undo removes it", id.0),
            Err(error) => format!("Column rejected: {error:?}"),
        };
    }

    /// A click of the Footing tool in the plan of `level`: a pad of the options bar's size centred there, its top at
    /// the level plus the top offset (usually below it).
    pub(crate) fn footing_click(&mut self, level: ElementId, at: [f64; 2]) {
        let m = |v: f64| v / 1000.0;
        let spec = FootingSpec { width_m: m(self.width_mm), length_m: m(self.height_mm), thickness_m: m(self.footing_thickness_mm), top_offset_m: m(self.slab_offset_mm), material: self.material_pick };
        // a click on a piece of equipment lays the pad under it, made by it, so that it moves with the unit
        let equipment = self.document.elements().filter(|e| e.category == Category::Family).filter_map(|e| match e.geometry {
            Some(openbim_document::Geometry::Placed { at: p, level: l }) if l == level => Some((e.id, (p[0] - at[0]).hypot(p[1] - at[1]))),
            _ => None,
        }).filter(|(_, d)| *d <= 0.5).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(id, _)| id);
        if let Some(unit) = equipment {
            self.message = match self.document.place_equipment_pad(unit, &spec) {
                Ok(id) => format!("Pad {} placed under the equipment; it moves with it; undo removes it", id.0),
                Err(error) => format!("Pad rejected: {error:?}"),
            };
            return;
        }
        self.message = match self.document.place_footing(level, at, &spec) {
            Ok(id) => format!("Footing {} placed; undo removes it", id.0),
            Err(error) => format!("Footing rejected: {error:?}"),
        };
    }

    /// A click of the Wall Footing tool on `wall`: a strip footing of the options bar's width and depth under it.
    pub(crate) fn wall_footing_click(&mut self, wall: ElementId) {
        if self.document.get(wall).is_none_or(|w| w.category != Category::Wall) { self.message = "Wall footing: click a wall".into(); return; }
        let m = |v: f64| v / 1000.0;
        let spec = WallFootingSpec { width_m: m(self.width_mm), depth_m: m(self.height_mm), top_offset_m: m(self.slab_offset_mm), material: self.material_pick };
        self.message = match self.document.place_wall_footing(wall, &spec) {
            Ok(id) => format!("Wall footing {} placed under wall {}; undo removes it", id.0, wall.0),
            Err(error) => format!("Wall footing rejected: {error:?}"),
        };
    }

    /// A click of the Slab tool in the plan of `level`: a corner, or the closing of the outline when it lands on the
    /// first corner of a polygon that has three or more.
    /// Pick a wall for a slab boundary: adds the wall's exterior face to the slab outline.
    pub(crate) fn slab_pick_wall(&mut self, wall: ElementId) {
        let Some(w) = self.document.get(wall) else { return };
        let Some(Geometry::Linear { start, end, level }) = w.geometry else { return };
        if level.0 != self.plan_level().0 { return; } // only walls on the current level
        let th = w.width_m; // wall thickness
        // exterior face is on the left of the direction of drawing
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let len = dx.hypot(dy);
        if len < 1e-9 { return; }
        let nx = -dy / len; // normal to the left
        let ny = dx / len;
        let half = th / 2.0;
        // exterior face: offset centerline by half thickness to the left
        let a = [start[0] + nx * half, start[1] + ny * half];
        let b = [end[0] + nx * half, end[1] + ny * half];
        // avoid duplicate consecutive points
        if let Some(last) = self.slab_points.last() {
            if (last[0] - a[0]).abs() < 1e-9 && (last[1] - a[1]).abs() < 1e-9 { }
            else { self.slab_points.push(a); }
        } else {
            self.slab_points.push(a);
        }
        self.slab_points.push(b);
        self.message = format!("Slab: {} corners from walls; Enter closes the outline", self.slab_points.len());
    }
    /// Slab edit mode: when a slab is selected and the Slab tool is activated, load its outline into slab_points.
    pub(crate) fn slab_edit_begin(&mut self) {
        if let Some(id) = self.selected_id() {
            if let Some(el) = self.document.get(id) {
                if el.category == Category::Slab && !el.outline.is_empty() {
                    self.slab_points = el.outline.clone();
                    self.message = format!("Slab {}: editing outline ({} corners); Enter to apply, Esc to cancel", id.0, self.slab_points.len());
                }
            }
        }
    }

    /// Slab edit mode: apply the edited outline to the selected slab.
    pub(crate) fn slab_edit_finish(&mut self) {
        if let Some(id) = self.selected_id() {
            if let Some(el) = self.document.get(id) {
                if el.category == Category::Slab {
                    let ops = [openbim_document::Command::SetOutline { id, outline: self.slab_points.clone() }];
                    match self.document.transact(self.document.revision(), &ops) {
                        Ok(_) => {
                            self.slab_points.clear();
                            self.message = format!("Slab {} outline updated; undo restores it", id.0);
                        }
                        Err(e) => self.message = format!("Slab edit rejected ({e:?}); Backspace takes a corner back"),
                    }
                    return;
                }
            }
        }
        // no slab selected: place a new one
        self.finish_slab(ElementId(self.plan_level().0));
    }
    pub(crate) fn slab_click(&mut self, level: ElementId, at: [f64; 2], closing: bool) {
        if self.tool == crate::Tool::Slab && self.slab_kind_pick == openbim_document::SlabKind::Ceiling && self.ceiling_auto { return self.ceiling_auto_click(level, at); }
        if closing && self.slab_points.len() >= 3 { return self.finish_slab(level); }
        if self.slab_points.last() == Some(&at) { return; }
        self.slab_points.push(at);
        self.message = if self.tool == crate::Tool::VerticalOpening { format!("Vertical Opening: {} corners; Enter closes the outline", self.slab_points.len()) } else if self.tool == crate::Tool::Railing { format!("Railing: {} points; Enter finishes the path", self.slab_points.len()) } else { format!("Slab: {} corners; Enter closes the outline", self.slab_points.len()) };
    }

    /// The floors, roofs and ceilings on `level` that lie under the plan point `at`, the likeliest first: floors, then
    /// roofs, then ceilings.
    pub(crate) fn hosts_at(&self, level: ElementId, at: [f64; 2]) -> Vec<ElementId> {
        let rank = |e: &openbim_document::Element| if e.is_roof() { 1 } else if e.slab_kind == openbim_document::SlabKind::Ceiling { 2 } else { 0 };
        let mut hits: Vec<_> = self.document.elements()
            .filter(|e| e.category == Category::Slab && matches!(e.geometry, Some(Geometry::Area { level: l }) if l == level)
                && openbim_document::RoomShape { outline: e.outline.clone(), holes: Vec::new() }.contains(at))
            .map(|e| (rank(e), e.id)).collect();
        hits.sort();
        hits.into_iter().map(|(_, id)| id).collect()
    }

    /// A click of the Vertical Opening tool before it has a host: the floor, roof or ceiling under the pointer.
    pub(crate) fn vertical_host_click(&mut self, level: ElementId, at: [f64; 2]) {
        self.opening_cands = self.hosts_at(level, at);
        let Some(first) = self.opening_cands.first().copied() else { self.message = "Vertical Opening: click a floor, roof or ceiling of this level".into(); return };
        self.opening_host = Some(first);
        self.message = self.host_message();
    }

    /// Tab: the next floor, roof or ceiling under the click.
    pub(crate) fn vertical_host_next(&mut self) {
        let Some(h) = self.opening_host else { return };
        if self.opening_cands.len() < 2 { return; }
        let at = self.opening_cands.iter().position(|c| *c == h).map_or(0, |i| (i + 1) % self.opening_cands.len());
        self.opening_host = Some(self.opening_cands[at]);
        self.message = self.host_message();
    }

    fn host_message(&self) -> String {
        let name = self.opening_host.and_then(|h| self.document.get(h)).map_or("-".to_string(), |h| h.name.clone());
        let more = if self.opening_cands.len() > 1 { format!(" ({} under the click: Tab takes the next)", self.opening_cands.len()) } else { String::new() };
        format!("Vertical Opening in {name}{more}: click the corners of the opening")
    }

    /// A click of the Wall Opening tool on `wall`, `along_m` from its start: the first click is one end of the opening, the
    /// second the other end. The sill and the head are those of the options bar.
    pub(crate) fn wall_opening_click(&mut self, wall: Option<ElementId>, along_m: f64) {
        let Some(wall) = wall else { self.message = "Wall Opening: click a wall".into(); return };
        let Some((host, from)) = self.opening_from else {
            self.opening_from = Some((wall, along_m));
            self.message = "Wall Opening: click the other end of the opening on the same wall".into();
            return;
        };
        if host != wall { self.message = "Wall Opening: the other end must be on the same wall".into(); return; }
        let (sill, head) = (self.opening_sill_mm / 1000.0, self.opening_head_mm / 1000.0);
        match self.document.place_wall_opening(wall, from, along_m, sill, head) {
            Ok(id) => { self.opening_from = None; self.message = format!("Wall opening {} placed, {:.0} mm wide; undo removes it", id.0, (along_m - from).abs() * 1000.0); }
            Err(e) => { self.opening_from = None; self.message = format!("Wall opening rejected ({e:?}): it must be at least 50 mm each way and inside the wall"); }
        }
    }

    /// Automatic Ceiling: a ceiling in the closed space that holds `at`, at the height and thickness of the options bar.
    pub(crate) fn ceiling_auto_click(&mut self, level: ElementId, at: [f64; 2]) {
        let shape = match self.document.room_region_at(level, at) {
            Ok(s) => s,
            Err(e) => { self.message = format!("Ceiling: {e}"); return; }
        };
        let height = self.slab_offset_mm / 1000.0;
        let has = self.document.elements().any(|e| e.slab_kind == openbim_document::SlabKind::Ceiling && e.category == Category::Slab
            && matches!(e.geometry, Some(Geometry::Area { level: l }) if l == level) && (e.elevation_m - height).abs() < 1e-6
            && openbim_document::RoomShape { outline: e.outline.clone(), holes: Vec::new() }.contains(at));
        if has { self.message = "Ceiling: there is a ceiling at this height in this space already".into(); return; }
        match self.document.place_ceiling(level, shape.outline, height, self.slab_thickness_mm / 1000.0, self.material_pick) {
            Ok(id) => self.message = format!("Ceiling {} placed at {:.0} mm; undo removes it", id.0, self.slab_offset_mm),
            Err(e) => self.message = format!("Ceiling rejected ({e:?})"),
        }
    }

    /// Lays the slab drawn so far on `level` with the thickness, top offset, kind and material of the options bar.
    pub(crate) fn finish_slab(&mut self, level: ElementId) {
        let m = |v: f64| v / 1000.0;
        // REA-011: reject tiny edges (< 50 mm) before attempting placement
        if self.slab_points.len() >= 3 {
            let mut tiny = false;
            for i in 0..self.slab_points.len() {
                let a = self.slab_points[i];
                let b = self.slab_points[(i + 1) % self.slab_points.len()];
                if (a[0] - b[0]).hypot(a[1] - b[1]) < 0.05 {
                    tiny = true;
                    break;
                }
            }
            if tiny {
                self.message = "Slab rejected (an edge is shorter than 50 mm): Backspace takes a corner back".into();
                return;
            }
        }
        // REA-011: handle multi-ring slabs (courtyard + inner holes)
        if !self.slab_rings.is_empty() && self.tool == crate::Tool::Slab {
            // Collect all rings: completed rings + current points
            let mut all_rings = std::mem::take(&mut self.slab_rings);
            if self.slab_points.len() >= 3 {
                all_rings.push(std::mem::take(&mut self.slab_points));
            }
            if all_rings.is_empty() { return; }
            // First ring is the outer boundary
            let outer = all_rings[0].clone();
            // Place the slab with the outer ring
            let spec = openbim_document::SlabSpec { outline: outer, thickness_m: m(self.slab_thickness_mm), top_offset_m: m(self.slab_offset_mm), kind: self.slab_kind_pick, material: self.material_pick };
            match self.document.place_slab(level, &spec) {
                Ok(id) => {
                    // Place shafts for inner rings (holes)
                    for ring in all_rings.iter().skip(1) {
                        let shaft_spec = openbim_document::ShaftSpec { outline: ring.clone(), top: self.document.level_above(level).unwrap_or(level), base_offset_m: 0.0, top_offset_m: m(self.slab_thickness_mm) };
                        let _ = self.document.place_shaft(level, &shaft_spec);
                    }
                    self.slab_points.clear();
                    self.slab_rings.clear();
                    self.message = format!("{} {} placed with {} hole(s); undo removes it", self.slab_kind_pick.name(), id.0, all_rings.len() - 1);
                }
                Err(error) => {
                    self.slab_rings = all_rings;
                    self.message = format!("Slab rejected ({error:?}); Backspace takes a corner back");
                }
            }
            return;
        }
        if self.tool == crate::Tool::Shaft {
            let Some(top) = self.shaft_top.filter(|t| self.document.get(*t).is_some()).or_else(|| self.document.level_above(level)) else {
                self.message = "Shaft: there is no level above to go up to; add a level first".into();
                return;
            };
            let spec = openbim_document::ShaftSpec { outline: self.slab_points.clone(), top, base_offset_m: m(self.shaft_base_mm), top_offset_m: m(self.shaft_top_mm) };
            match self.document.place_shaft(level, &spec) {
                Ok(id) => {
                    self.slab_points.clear();
                    let cut = self.document.elements().filter(|e| e.category == Category::Slab && self.document.shafts_through(e.id).contains(&id)).count();
                    self.message = format!("Shaft {} placed; it cuts {cut} floor{} and ceilings; undo removes it", id.0, if cut == 1 { "" } else { "s" });
                }
                Err(error) => self.message = format!("Shaft rejected ({error:?}): it must go up from its base; Backspace takes a corner back"),
            }
            return;
        }
        if self.tool == crate::Tool::VerticalOpening {
            let Some(host) = self.opening_host else { self.message = "Vertical Opening: pick the floor, roof or ceiling first".into(); return };
            match self.document.place_vertical_opening(host, &self.slab_points) {
                Ok(id) => {
                    self.slab_points.clear();
                    let over = self.document.cuts_of(host).contains(&id);
                    self.message = format!("Vertical opening {} placed in {}{}; undo removes it", id.0, self.document.get(host).map_or("-", |h| h.name.as_str()), if over { "" } else { " (it is not over the element, so it cuts nothing)" });
                }
                Err(error) => self.message = format!("Vertical opening rejected ({error:?}): the outline must be a simple polygon; Backspace takes a corner back"),
            }
            return;
        }
        if self.tool == crate::Tool::Railing {
            let spec = openbim_document::RailingSpec { path: self.slab_points.clone(), height_m: m(self.height_mm), rail_m: m(self.width_mm), baluster_spacing_m: m(self.spacing_mm), base_offset_m: m(self.elevation_mm) };
            match self.document.place_railing(level, &spec) {
                Ok(id) => { self.slab_points.clear(); self.message = format!("Railing {} placed; undo removes it", id.0); }
                Err(error) => self.message = format!("Railing rejected ({error:?}): check the height, the sizes and the spacing (50 mm to 2 m, or 0 for none); Backspace takes a point back"),
            }
            return;
        }
        if self.tool == crate::Tool::BeamSystem {
            let spec = BeamSystemSpec { outline: self.slab_points.clone(), spacing_m: m(self.spacing_mm), angle_deg: self.angle_deg, width_m: m(self.width_mm),
                depth_m: if self.section_pick == 1 { m(self.width_mm) } else { m(self.height_mm) }, elevation_m: m(self.elevation_mm), section: self.section_from_controls(),
                usage: self.usage_pick, material: self.material_pick };
            match self.document.place_beam_system(level, &spec) {
                Ok(id) => { let n = self.document.system_beams(id).len(); self.slab_points.clear(); self.message = format!("Beam system {} placed with {n} beams; undo removes it", id.0); }
                Err(error) => self.message = format!("Beam system rejected ({error:?}); Backspace takes a corner back"),
            }
            return;
        }
        if self.slab_kind_pick == openbim_document::SlabKind::Ceiling {
            match self.document.place_ceiling(level, self.slab_points.clone(), m(self.slab_offset_mm), m(self.slab_thickness_mm), self.material_pick) {
                Ok(id) => { self.slab_points.clear(); self.message = format!("Ceiling {} placed; undo removes it", id.0); }
                Err(error) => self.message = format!("Ceiling rejected ({error:?}); Backspace takes a corner back"),
            }
            return;
        }
        if self.slab_kind_pick == openbim_document::SlabKind::Roof {
            let spec = openbim_document::RoofSpec { outline: self.slab_points.clone(), thickness_m: m(self.slab_thickness_mm), base_offset_m: m(self.slab_offset_mm),
                slope_ratio: self.roof_slope_deg.to_radians().tan(), edges: Vec::new(), material: self.material_pick };
            match self.document.place_roof(level, &spec) {
                Ok(id) => { self.slab_points.clear(); self.message = format!("Roof {} placed; select it to choose which edges slope. Undo removes it", id.0); }
                Err(error) => self.message = format!("Roof rejected ({error:?}): the outline must not cross itself and have at most 64 corners; Backspace takes a corner back"),
            }
            return;
        }
        let spec = SlabSpec { outline: self.slab_points.clone(), thickness_m: m(self.slab_thickness_mm), top_offset_m: m(self.slab_offset_mm),
            kind: self.slab_kind_pick, material: self.material_pick };
        match self.document.place_slab(level, &spec) {
            Ok(id) => { self.slab_points.clear(); self.message = format!("{} {} placed; undo removes it", self.slab_kind_pick.name(), id.0); }
            Err(error) => self.message = format!("Slab rejected ({error:?}); Backspace takes a corner back"),
        }
    }

    /// Whether the instance values in the Properties differ from those of the selected family instance.
    fn family_values_changed(&self, el: &openbim_document::Element) -> bool {
        let Some(now) = crate::export::instance_values_of(&self.document, el) else { return false };
        let Some(texts) = crate::export::instance_texts_of(&self.document, el) else { return false };
        if el.type_name == self.family_type && (texts.len() != self.family_texts.len() || texts.iter().zip(&self.family_texts).any(|(a, b)| a.0 != b.0 || a.1 != b.1)) { return true; }
        // a changed type brings its own values, so only a same-type edit counts
        el.type_name == self.family_type && now.len() == self.family_values.len()
            && now.iter().zip(&self.family_values).any(|(a, b)| a.0 == b.0 && (a.1 - b.1).abs() > 1e-9 || !b.1.is_finite())
    }

    /// The type-scoped parameters of the selected instance's type, as the type gives them: what the type editor
    /// shows. Empty when the instance has no type of its own.
    fn family_type_drafts(&self) -> (Vec<(String, f64)>, Vec<crate::export::TextInput>) {
        let Some(t) = self.family_type_editing.and_then(|t| self.document.get(t)) else { return (Vec::new(), Vec::new()) };
        let Some(f) = crate::export::library().iter().find(|f| f.name() == t.family) else { return (Vec::new(), Vec::new()) };
        crate::export::family_type_inputs(&self.document, t.id, f)
    }

    /// Applies the type editor: the type takes these values and choices, and every instance of it follows in one
    /// transaction (an instance's own value still wins over its type's).
    pub(crate) fn apply_family_type(&mut self) {
        let Some(t) = self.family_type_editing else { self.message = "Type: select a family instance that has one".into(); return };
        let texts: Vec<(String, String)> = self.family_type_texts.iter().map(|(n, v, _)| (n.clone(), v.clone())).collect();
        match self.document.edit_family_type(t, &self.family_type_values.clone(), &texts) {
            Ok(()) => {
                let n = self.document.instances_of(t).len();
                self.message = format!("Type {} changed for {n} {}; undo puts it back", self.document.get(t).map_or("?".into(), |x| x.name.clone()), if n == 1 { "instance" } else { "instances" });
            }
            Err(error) => self.message = format!("Type rejected: {error:?}"),
        }
    }

    /// The overrides that make `values` on top of the family's `type_name`: what differs from the type's own.
    fn family_overrides(&self, family: &str, type_name: &str, values: &[(String, f64)]) -> Vec<(String, f64)> {
        let base = crate::export::instance_values(family, type_name, &[]).unwrap_or_default();
        values.iter().filter(|(n, v)| base.iter().find(|b| b.0 == *n).is_none_or(|b| (b.1 - v).abs() > 1e-9)).cloned().collect()
    }

    /// A click of the Family tool: a door or window goes into the wall under the click, anything else stands on the
    /// level at the click.
    pub(crate) fn family_click(&mut self, level: ElementId, at: [f64; 2], wall: Option<ElementId>) {
        let spec = FamilySpec { family: self.family_pick.clone(), type_name: self.family_type.clone(), overrides: Vec::new(), angle_deg: self.angle_deg, elevation_m: self.slab_offset_mm / 1000.0 };
        let in_wall = spec.family.starts_with("Door") || spec.family.starts_with("Window");
        if !in_wall && (self.family_snap_click(level, at).is_some() || self.family_splice_click(level, at).is_some()) { return; }
        let result = if in_wall {
            let Some(w) = wall else { self.message = format!("{}: click the wall it goes in", spec.family); return; };
            let Some(Geometry::Linear { start, end, .. }) = self.document.get(w).and_then(|e| e.geometry) else { return; };
            let len = (end[0] - start[0]).hypot(end[1] - start[1]);
            let along = (((at[0] - start[0]) * (end[0] - start[0]) + (at[1] - start[1]) * (end[1] - start[1])) / len).clamp(0.0, len);
            self.document.place_family_in_wall(w, along, &FamilySpec { angle_deg: 0.0, ..spec })
        } else {
            self.document.place_family(level, at, &spec)
        };
        self.message = match result {
            Ok(id) => format!("{} placed as element {}; undo removes it", self.family_pick, id.0),
            Err(error) => format!("Family rejected: {error:?}"),
        };
    }

    /// A click of the Rebar tool on a beam: reinforcement from the options bar goes into it.
    pub(crate) fn rebar_click(&mut self, beam: ElementId) {
        if self.document.get(beam).is_none_or(|b| b.category != Category::Beam) { self.message = "Rebar: click a beam".into(); return; }
        let m = |v: f64| v / 1000.0;
        let spec = RebarSpec { count: self.bar_count.max(0.0) as u32, bar_m: m(self.width_mm), stirrup_m: m(self.height_mm), cover_m: m(self.elevation_mm), stirrup_spacing_m: m(self.spacing_mm) };
        self.message = match self.document.place_rebar(beam, &spec) {
            Ok(id) => format!("Rebar {} placed in beam {}; undo removes it", id.0, beam.0),
            Err(error) => format!("Rebar rejected ({error:?}): the bars must fit inside the beam, past the cover"),
        };
    }

    /// A click of the Truss tool in the plan of `level`: the first sets the start, the second builds the truss.
    pub(crate) fn truss_click(&mut self, level: ElementId, point: [f64; 2]) {
        let Some(start) = self.beam_start else { self.beam_start = Some(point); self.message = "Truss: click the end".into(); return; };
        let m = |v: f64| v / 1000.0;
        let spec = TrussSpec { start, end: point, spacing_m: m(self.spacing_mm), depth_m: m(self.height_mm), member_m: m(self.width_mm), elevation_m: m(self.elevation_mm), material: self.material_pick };
        match self.document.place_truss(level, &spec) {
            Ok(id) => { self.beam_start = None; self.message = format!("Truss {} placed with {} members; undo removes it", id.0, self.document.system_beams(id).len()); }
            Err(error) => self.message = format!("Truss rejected ({error:?}); click another end"),
        }
    }

    /// A click of the Beam tool in the plan of `level`: the first sets the start, the second places the beam and the
    /// tool stays on for the next one.
    /// The Location Line choices of the Wall tool, as Revit lists them. A wall of one layer has its core where its
    /// finish is, so the core choices give the same lines as the finish ones; a layered wall type has its own.
    pub(crate) const WALL_LOCATIONS: [&'static str; 6] = ["Wall Centerline", "Core Centerline", "Finish Face: Exterior", "Finish Face: Interior", "Core Face: Exterior", "Core Face: Interior"];

    /// The axis of a wall drawn from `a` to `b` along the Location Line: the exterior face is on the left of the
    /// direction of drawing (draw clockwise and it faces out). The axis, the centre of the thickness, lies where the
    /// layers of the wall type put it: half a thickness to the right of a line drawn along the exterior face, half to
    /// the left of one along the interior face, and for a core line as far from it as the core is from the middle.
    pub(crate) fn wall_axis(&self, a: [f64; 2], b: [f64; 2]) -> ([f64; 2], [f64; 2]) {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dy);
        if len < 1e-9 { return (a, b); }
        let loc = openbim_document::LocationLine::ALL[self.wall_loc.min(5)];
        let k = self.document.location_shift(self.wall_type, self.width_mm / 1000.0, loc, self.wall_flip);
        let (nx, ny) = (-dy / len * k, dx / len * k);
        ([a[0] + nx, a[1] + ny], [b[0] + nx, b[1] + ny])
    }

    /// A pick of the Wall tool in the plan of `level`: the first starts a wall, each next one draws it (along the
    /// Location Line of the options bar) and, with Chain on, starts the next. The walls of a chain meet at the
    /// corners of their lines, and a pick on the first point closes the loop. Esc ends the chain.
    pub(crate) fn wall_click(&mut self, level: ElementId, point: [f64; 2]) {
        if self.wall_mode == 1 {
            let Some(first) = self.wall_pts.first().copied() else { self.wall_pts.push(point); self.message = "Wall (rectangle): pick the opposite corner (Esc cancels)".into(); return; };
            if (point[0] - first[0]).abs() < 0.01 || (point[1] - first[1]).abs() < 0.01 { return; }
            self.wall_pts.clear();
            self.wall_rectangle(level, first, point);
            return;
        }
        if self.wall_mode >= 3 { self.wall_shape_click(level, point); return; }
        let Some(&last) = self.wall_pts.last() else { self.wall_pts.push(point); self.message = "Wall: pick the end (type a length in mm, Shift keeps it square, Esc ends the chain)".into(); return; };
        if (point[0] - last[0]).hypot(point[1] - last[1]) < 0.01 { return; }
        let m = |v: f64| v / 1000.0;
        let line = |w: ElementId| match self.document.get(w).and_then(|e| e.geometry) { Some(Geometry::Linear { start, end, .. }) => Some((start, end)), _ => None };
        let (a, b) = self.wall_axis(last, point);
        let dir = [b[0] - a[0], b[1] - a[1]];
        let closing = self.wall_chain_on && self.wall_ids.len() >= 2 && (point[0] - self.wall_pts[0][0]).hypot(point[1] - self.wall_pts[0][1]) < 0.01;
        let (mut start, mut end, mut moves) = (a, b, Vec::new());
        if self.wall_chain_on {
            if let Some((ps, pe)) = self.wall_ids.last().and_then(|w| line(*w)) {
                // the corner of the last wall's line and this one's, when they are not parallel
                if let Some(v) = openbim_document::line_meet(ps, [pe[0] - ps[0], pe[1] - ps[1]], a, dir) {
                    if (v[0] - a[0]).hypot(v[1] - a[1]) < 20.0 * m(self.width_mm) + 1e-6 { start = v; moves.push((*self.wall_ids.last().unwrap(), ps, v)); }
                }
            }
            if closing {
                if let Some((fs, fe)) = line(self.wall_ids[0]) {
                    if let Some(v) = openbim_document::line_meet(fs, [fe[0] - fs[0], fe[1] - fs[1]], a, dir) {
                        if (v[0] - b[0]).hypot(v[1] - b[1]) < 20.0 * m(self.width_mm) + 1e-6 { end = v; moves.push((self.wall_ids[0], v, fe)); }
                    }
                }
            }
        }
        match self.document.place_wall_typed(level, start, end, m(self.width_mm), m(self.height_mm), m(self.elevation_mm), &moves, self.wall_type) {
            Ok(id) => {
                self.wall_len.clear();
                if closing || !self.wall_chain_on { self.wall_pts.clear(); self.wall_ids.clear(); self.message = format!("Wall {} placed; undo removes it", id.0); }
                else { self.wall_pts.push(point); self.wall_ids.push(id); self.message = format!("Wall {} placed; pick the next point, Esc ends the chain; undo removes it", id.0); }
            }
            Err(error) => self.message = format!("Wall rejected: {error:?}"),
        }
    }

    /// The change a Move, Copy, Rotate or Mirror makes when `next` is picked after the points in `edit_pts`; none
    /// while the points are too few or too close.
    pub(crate) fn edit_xf(&self, tool: Tool, next: [f64; 2]) -> Option<Xf> {
        let far = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) > 1e-6;
        match (tool, self.edit_pts.as_slice()) {
            (Tool::Move | Tool::Copy, [a]) if far(*a, next) => Some(Xf::Move([next[0] - a[0], next[1] - a[1]])),
            (Tool::Mirror, [a]) if far(*a, next) => Some(Xf::Mirror(*a, next)),
            (Tool::Rotate, [c, r]) if far(*c, *r) && far(*c, next) => {
                let ang = |p: [f64; 2]| (p[1] - c[1]).atan2(p[0] - c[0]);
                Some(Xf::Rotate(*c, ang(next) - ang(*r)))
            }
            _ => None,
        }
    }

    /// A pick of Move, Copy, Rotate or Mirror: the points are collected, and the last one makes the change.
    pub(crate) fn edit_click(&mut self, point: [f64; 2]) {
        if self.selected_id().is_none() { self.message = "Select an element first (click it), then pick the points".into(); return; }
        let need = if self.tool == Tool::Rotate { 3 } else { 2 };
        if self.edit_pts.len() + 1 < need {
            if self.edit_pts.is_empty() { self.edit_src = self.selected_ids(); }
            self.edit_pts.push(point);
            self.message = match (self.tool, self.edit_pts.len()) {
                (Tool::Move | Tool::Copy, _) => "Pick the point to move to (type a distance in mm and Enter, Shift keeps it square, Esc cancels)".into(),
                (Tool::Mirror, _) => "Pick the second point of the axis (Esc cancels)".into(),
                (_, 1) => "Pick the start of the turn, or type an angle in degrees and Enter".into(),
                _ => "Pick the end of the turn (Esc cancels)".into(),
            };
            return;
        }
        match self.edit_xf(self.tool, point) {
            Some(xf) => self.edit_finish(xf),
            None => self.message = "Pick a different point".into(),
        }
    }

    /// Enter after a typed number: a distance for Move and Copy (towards `towards`), an angle in degrees for Rotate.
    pub(crate) fn edit_enter(&mut self, towards: [f64; 2]) {
        let Ok(v) = self.edit_num.parse::<f64>() else { return };
        self.edit_num.clear();
        match (self.tool, self.edit_pts.first().copied()) {
            (Tool::Move | Tool::Copy, Some(a)) if self.edit_pts.len() == 1 => {
                let (dx, dy) = (towards[0] - a[0], towards[1] - a[1]);
                let l = dx.hypot(dy);
                if l > 1e-9 && v > 0.0 { self.edit_finish(Xf::Move([dx / l * v / 1000.0, dy / l * v / 1000.0])); }
            }
            (Tool::Rotate, Some(c)) => self.edit_finish(Xf::Rotate(c, v.to_radians())),
            _ => {}
        }
    }

    /// Makes the change on the selected element in one undoable step. A copy can go on (the base stays); the
    /// others end, leaving the element selected.
    fn edit_finish(&mut self, xf: Xf) {
        let ids = if self.edit_src.is_empty() { self.selected_ids() } else { self.edit_src.clone() };
        if ids.is_empty() { return; }
        let copying = self.tool == Tool::Copy || (self.tool == Tool::Mirror && self.mirror_copy);
        let mut moved_notes: Vec<String> = Vec::new();
        let made = if copying { self.document.copy_commands(&ids, xf).map(|(ops, firsts)| (ops, Some(firsts))) }
            else if self.tool == Tool::Move && self.move_disjoin { self.document.transform_commands_disjoined(&ids, xf).map(|(ops, notes)| { moved_notes = notes; (ops, None) }) }
            else if self.tool == Tool::Move { self.document.transform_commands_keeping_joins(&ids, xf).map(|(ops, notes)| { moved_notes = notes; (ops, None) }) }
            else { self.document.transform_commands_noted(&ids, xf).map(|(ops, notes)| { moved_notes = notes; (ops, None) }) };
        let result = made.and_then(|(ops, new)| self.document.transact(self.document.revision(), &ops).map(|_| new).map_err(|e| format!("{e:?}")));
        match result {
            Ok(new) => {
                if let Some(n) = new { self.select_many(n); }
                self.message = format!("{} done; undo takes it back{}", match self.tool { Tool::Move => "Move", Tool::Copy => "Copy", Tool::Rotate => "Rotate", _ => "Mirror" }, if moved_notes.is_empty() { String::new() } else { format!(". {}", moved_notes.join("; ")) });
                if self.tool == Tool::Copy { self.edit_pts.truncate(1); self.edit_src = ids; } else { self.choose_tool(Tool::Select); }
            }
            Err(e) => { self.message = format!("Cannot do that: {e}"); self.edit_pts.clear(); self.edit_src.clear(); }
        }
    }

    /// A click of Wall Joins on one end of a wall: its join is disallowed, or allowed again if it was.
    pub(crate) fn wall_join_click(&mut self, id: ElementId, at_start: bool) {
        let allowed = self.document.get(id).is_some_and(|w| w.join_ends[usize::from(!at_start)]);
        match self.document.wall_join_commands(id, at_start, !allowed).and_then(|ops| self.document.transact(self.document.revision(), &ops).map_err(|e| format!("{e:?}"))) {
            Ok(_) => self.message = format!("Join {} at the {} of the wall; undo takes it back", if allowed { "disallowed" } else { "allowed" }, if at_start { "start" } else { "end" }),
            Err(e) => self.message = format!("Wall Joins: {e}"),
        }
    }

    /// A click of the Stair tool: the foot of the first riser, then the way the stair climbs. The climb is the height
    /// to the level above (less the base offset), or the height of the options bar when there is none.
    pub(crate) fn stair_click(&mut self, level: ElementId, point: [f64; 2]) {
        let Some(start) = self.edit_pts.first().copied() else { self.edit_pts.push(point); self.message = "Stair: pick a point in the direction it climbs".into(); return };
        if (point[0] - start[0]).hypot(point[1] - start[1]) < 0.05 { return; }
        let base = self.elevation_mm / 1000.0;
        let rise = match self.document.level_above(level) {
            Some(above) => self.document.get(above).map_or(0.0, |a| a.elevation_m) - self.document.get(level).map_or(0.0, |l| l.elevation_m) - base,
            None => self.stair_rise_mm / 1000.0,
        };
        if rise < 0.2 { self.message = "Stair: the base is too close to the top level (nothing to climb)".into(); return; }
        let risers = openbim_document::risers_for(rise, self.stair_riser_mm / 1000.0);
        let spec = openbim_document::StairSpec { start, toward: point, width_m: self.stair_width_mm / 1000.0, rise_m: rise, risers, tread_m: self.stair_tread_mm / 1000.0, base_offset_m: base };
        let turning = self.stair_shape != openbim_document::StairShape::Straight;
        let with = match self.stair_railing { openbim_document::RailingSides::None => "", openbim_document::RailingSides::Both => " and a railing on each side", _ => " and a railing on one side" };
        match self.document.place_stair_with_railing(level, &spec, self.stair_shape, self.stair_railing) {
            Ok(id) if turning => { self.edit_pts.clear(); self.message = format!("Stair {}: {risers} risers of {:.0} mm in two runs with a landing{with}; undo removes all of it", id.0, rise / f64::from(risers) * 1000.0); }
            Ok(id) => { self.edit_pts.clear(); self.message = format!("Stair {}: {risers} risers of {:.0} mm, {} treads of {:.0} mm{with}; undo removes it", id.0, rise / f64::from(risers) * 1000.0, risers - 1, self.stair_tread_mm); }
            Err(e) => self.message = format!("Stair rejected ({e:?}): check the riser height and tread depth (an L or U stair needs at least 4 risers)"),
        }
    }

    /// A click of the Room tool: a room in the closed space under the point.
    pub(crate) fn room_click(&mut self, level: ElementId, at: [f64; 2]) {
        match self.document.place_room(level, at) {
            Ok(id) => {
                let (name, area) = (self.document.get(id).map(|e| e.name.clone()).unwrap_or_default(), self.document.room_shape(id).map(|s| s.area()).unwrap_or(0.0));
                self.message = format!("{name} placed, {area:.1} m²; undo removes it");
            }
            Err(e) => self.message = format!("Room: {e}"),
        }
    }

    /// A pick of the Dimension tool: the two points, then the place of the dimension line.
    pub(crate) fn dimension_click(&mut self, level: ElementId, point: [f64; 2]) {
        match self.edit_pts.as_slice() {
            [] => { self.edit_pts.push(point); self.message = "Dimension: pick the second point".into(); }
            [a] => {
                if (point[0] - a[0]).hypot(point[1] - a[1]) < 0.001 { return; }
                self.edit_pts.push(point);
                self.message = "Dimension: pick where the dimension line goes".into();
            }
            [a, b] => {
                let (a, b) = (*a, *b);
                match self.document.place_dimension(level, a, b, point) {
                    Ok(_) => { self.edit_pts.clear(); self.message = format!("Dimension {:.0} mm placed; undo removes it", (b[0] - a[0]).hypot(b[1] - a[1]) * 1000.0); }
                    Err(e) => self.message = format!("Dimension rejected: {e:?}"),
                }
            }
            _ => self.edit_pts.clear(),
        }
    }

    /// A pick of the Measure tool: the first point, then the second, whose distance is shown.
    pub(crate) fn measure_click(&mut self, point: [f64; 2]) {
        match self.edit_pts.first().copied() {
            None => { self.measured = None; self.edit_pts.push(point); self.message = "Measure: pick the second point".into(); }
            Some(a) => {
                self.edit_pts.clear();
                let (dx, dy) = (point[0] - a[0], point[1] - a[1]);
                self.measured = Some((a, point));
                self.message = format!("Measure: {:.0} mm  (x {:.0}, y {:.0}, {:.1} deg)", dx.hypot(dy) * 1000.0, dx.abs() * 1000.0, dy.abs() * 1000.0, dy.atan2(dx).to_degrees());
            }
        }
    }

    /// Enter on a retyped temporary dimension: the wall changes in one undoable step.
    pub(crate) fn apply_temp_dim(&mut self) {
        let Some(t) = self.temp_edit.take() else { return };
        let Ok(mm) = self.temp_text.trim().parse::<f64>() else { self.message = "Type a number of millimetres".into(); return };
        let m = mm / 1000.0;
        let done = match t {
            TempDim::Length(id) => self.document.set_wall_length_commands(id, m),
            TempDim::Distance(id, other) => self.document.set_neighbour_distance_commands(id, other, m),
        }.and_then(|ops| self.document.transact(self.document.revision(), &ops).map_err(|e| format!("{e:?}")));
        self.message = match done { Ok(_) => format!("Changed to {mm:.0} mm; undo takes it back"), Err(e) => format!("Cannot do that: {e}") };
    }

    /// A click of Align, Trim/Extend or Offset on `hit` (or on nothing), at `at`.
    pub(crate) fn modify_click(&mut self, hit: Option<ElementId>, at: [f64; 2]) {
        let Some(id) = hit else { self.message = "Click a wall, beam or grid".into(); return };
        let revision = self.document.revision();
        match self.tool {
            Tool::Align => match self.edit_ref {
                None => { self.edit_ref = Some((id, at)); self.message = "Align: now click the element to move onto that line".into(); }
                Some((reference, _)) => {
                    let done = self.document.align_xf(reference, id).and_then(|xf| self.document.transform_commands(&[id], xf))
                        .and_then(|ops| self.document.transact(revision, &ops).map_err(|e| format!("{e:?}")));
                    self.message = match done { Ok(_) => "Aligned; undo takes it back (pick the next element, Esc ends)".into(), Err(e) => format!("Align: {e}") };
                }
            },
            Tool::Trim => match self.edit_ref {
                None => { self.edit_ref = Some((id, at)); self.message = "Trim/Extend: now click the second element on the part to keep".into(); }
                Some((first, first_at)) => {
                    self.edit_ref = None;
                    let done = self.document.trim_corner_commands(first, first_at, id, at)
                        .and_then(|ops| self.document.transact(revision, &ops).map_err(|e| format!("{e:?}")));
                    self.message = match done { Ok(_) => "Trimmed to the corner; undo takes it back".into(), Err(e) => format!("Trim/Extend: {e}") };
                }
            },
            Tool::Offset => {
                let result = self.document.offset_xf(id, self.offset_mm / 1000.0, at).and_then(|xf| {
                    if self.offset_copy { self.document.copy_commands(&[id], xf).map(|(ops, n)| (ops, n.first().copied())) }
                    else { self.document.transform_commands(&[id], xf).map(|ops| (ops, None)) }
                }).and_then(|(ops, new)| self.document.transact(revision, &ops).map(|_| new).map_err(|e| format!("{e:?}")));
                match result {
                    Ok(new) => { if let Some(n) = new { self.select_element(Some(n)); } self.message = "Offset; undo takes it back".into(); }
                    Err(e) => self.message = format!("Offset: {e}"),
                }
            }
            Tool::MatchType => self.match_type_click(Some(id), at),
            _ => {}
        }
    }

    /// Delete: the selected element with what is hosted by it or tagged on it, in one undoable step.
    pub(crate) fn delete_selected(&mut self) {
        let ids = self.selected_ids();
        if ids.is_empty() { self.message = "Delete: select an element first".into(); return; }
        if ids.iter().any(|id| self.document.get(*id).is_some_and(|e| e.category == Category::Level)) { self.message = "A level cannot be deleted here".into(); return; }
        let before = self.document.elements().count();
        match self.document.delete_commands(&ids).and_then(|ops| self.document.transact(self.document.revision(), &ops).map_err(|e| format!("{e:?}"))) {
            Ok(_) => {
                self.select_element(None);
                self.message = format!("Deleted {} element(s); undo brings them back", before - self.document.elements().count());
            }
            Err(e) => self.message = format!("Cannot delete: {e}"),
        }
    }

    /// Rename a view shown in the Project Browser (REA-009): a level plan renames its level; a section or
    /// elevation-marker view renames its backing element, whose name the view label follows.
    pub(crate) fn rename_view(&mut self, view: crate::View, name: &str) {
        let name = name.trim();
        if name.is_empty() { self.message = "A view needs a name".into(); return; }
        match view {
            crate::View::Level(id) => self.rename_level(ElementId(id), name),
            crate::View::Section(id) | crate::View::Marked(id, _) => {
                let kind = if matches!(view, crate::View::Section(_)) { "Section" } else { "Elevation" };
                self.view_names.insert(id, name.to_string());
                self.message = format!("{kind} renamed to {name}");
            }
            _ => self.message = "That view cannot be renamed".into(),
        }
    }

    /// The display name of a view (REA-009): a browser-rename override for a section or elevation-marker
    /// view, else the backing element's name; other views are named by their own logic.
    pub(crate) fn view_name(&self, view: crate::View) -> String {
        use crate::View;
        match view {
            View::Section(id) => self
                .view_names
                .get(&id)
                .cloned()
                .or_else(|| self.document.get(ElementId(id)).map(|e| e.name.clone()))
                .unwrap_or_else(|| "Section".to_string()),
            View::Marked(id, k) => {
                let base = self
                    .view_names
                    .get(&id)
                    .cloned()
                    .or_else(|| self.document.get(ElementId(id)).map(|e| e.name.clone()))
                    .unwrap_or_else(|| "Elevation".to_string());
                crate::section_ui::name_of_marker(&base, k)
            }
            _ => String::new(),
        }
    }
    /// Delete a view from the Project Browser (REA-009): a level plan deletes its level (refused if it holds
    /// elements); a section or elevation-marker view deletes its backing element and closes its tab.
    pub(crate) fn delete_view(&mut self, view: crate::View) {
        match view {
            crate::View::Level(id) => self.delete_level(ElementId(id)),
            crate::View::Section(id) | crate::View::Marked(id, _) => {
                let kind = if matches!(view, crate::View::Section(_)) { "Section" } else { "Elevation" };
                match self.document.delete_commands(&[ElementId(id)]).and_then(|ops| self.document.transact(self.document.revision(), &ops).map_err(|e| format!("{e:?}"))) {
                    Ok(_) => {
                        if let Some(i) = self.open_views.iter().position(|v| *v == view) { self.close_view_at(i); }
                        self.message = format!("{kind} view deleted; undo brings it back");
                    }
                    Err(e) => self.message = format!("Cannot delete {kind}: {e}"),
                }
            }
            _ => self.message = "That view cannot be deleted".into(),
        }
    }
    /// Duplicate a view from the Project Browser (REA-009): a section or elevation-marker view places a copy of
    /// its backing element beside it; the new view opens as a tab. Level plans are not duplicated here.
    pub(crate) fn duplicate_view(&mut self, view: crate::View) {
        match view {
            crate::View::Section(id) => {
                if let Some(el) = self.document.get(ElementId(id)) {
                    if let Some(Geometry::Linear { start, end, level }) = el.geometry {
                        match self.document.place_section(level, start, end, el.width_m) {
                            Ok(new) => { let _ = self.set_view(crate::View::Section(new.0)); self.message = "Section duplicated".into(); }
                            Err(error) => self.message = format!("Section not duplicated: {error:?}"),
                        }
                    } else { self.message = "That section has no line to copy".into(); }
                } else { self.message = "That section is missing".into(); }
            }
            crate::View::Marked(id, _) => {
                if let Some(el) = self.document.get(ElementId(id)) {
                    if let Some(Geometry::Point { at, level, .. }) = el.geometry {
                        match self.document.place_elevation(level, at, el.count, el.width_m, el.height_m) {
                            Ok(new) => { let _ = self.set_view(crate::View::Marked(new.0, 0)); self.message = "Elevation duplicated".into(); }
                            Err(error) => self.message = format!("Elevation not duplicated: {error:?}"),
                        }
                    } else { self.message = "That marker has no point to copy".into(); }
                } else { self.message = "That marker is missing".into(); }
            }
            _ => self.message = "That view cannot be duplicated".into(),
        }
    }

    /// The four lines (start, end) of a rectangle of walls drawn clockwise from the top left of the corners `a` and
    /// `b`, along the Location Line, meeting at the corners of their offset lines.
    pub(crate) fn rectangle_lines(&self, a: [f64; 2], b: [f64; 2]) -> Vec<([f64; 2], [f64; 2])> {
        let (x0, x1, y0, y1) = (a[0].min(b[0]), a[0].max(b[0]), a[1].min(b[1]), a[1].max(b[1]));
        self.chain_lines(&[[x0, y1], [x1, y1], [x1, y0], [x0, y0]], true)
    }

    /// The lines (start, end) of walls along the sides of the path `points`, drawn along the Location Line: each side
    /// is moved to where the wall's axis is, and neighbouring sides meet at the corners of their moved lines. A closed
    /// path has a side from its last point to its first.
    pub(crate) fn chain_lines(&self, points: &[[f64; 2]], closed: bool) -> Vec<([f64; 2], [f64; 2])> {
        let n = points.len();
        let count = if closed { n } else { n.saturating_sub(1) };
        let sides: Vec<([f64; 2], [f64; 2])> = (0..count).map(|i| self.wall_axis(points[i], points[(i + 1) % n])).collect();
        // the point where side `i` starts: the corner with the side before it, or its own start at the end of a path
        let corner = |i: usize| {
            let (u, v) = sides[i % count];
            if !closed && i.is_multiple_of(count) { return u; }
            let (s, t) = sides[(i + count - 1) % count];
            openbim_document::line_meet(s, [t[0] - s[0], t[1] - s[1]], u, [v[0] - u[0], v[1] - u[1]]).unwrap_or(u)
        };
        (0..count).map(|i| {
            let end = if !closed && i + 1 == count { sides[i].1 } else { corner(i + 1) };
            (corner(i), end)
        }).collect()
    }

    /// The Wall tool's rectangle: four joined walls in one undoable step.
    fn wall_rectangle(&mut self, level: ElementId, a: [f64; 2], b: [f64; 2]) {
        let lines = self.rectangle_lines(a, b);
        let m = |v: f64| v / 1000.0;
        match self.document.place_walls(level, &lines, m(self.width_mm), m(self.height_mm), m(self.elevation_mm), self.wall_type) {
            Ok(ids) => self.message = format!("{} walls placed; undo removes them together", ids.len()),
            Err(e) => self.message = format!("Walls rejected: {e:?}"),
        }
    }

    /// The names of the Wall tool's draw modes, in the order of `wall_mode`.
    pub(crate) const WALL_MODES: [&'static str; 7] = ["Line", "Rectangle", "Pick Lines", "Inscribed Polygon", "Circumscribed Polygon", "Circle", "Arc (start, end, point on it)"];

    /// The path a polygon, circle or arc mode draws from the points picked so far (the last of them where the pointer
    /// is), and whether it is closed. None while the points do not make the shape.
    pub(crate) fn shape_path(&self, pts: &[[f64; 2]]) -> Option<(Vec<[f64; 2]>, bool)> {
        use crate::wall_shapes as w;
        match (self.wall_mode, pts) {
            (3 | 4, [c, p]) => w::polygon(*c, *p, self.wall_sides, self.wall_mode == 4).map(|v| (v, true)),
            (5, [c, p]) => w::circle(*c, *p).map(|v| (v, true)),
            (6, [a, b, t]) => w::arc(*a, *b, *t).map(|v| (v, false)),
            _ => None,
        }
    }

    /// The walls a polygon, circle or arc mode would place for the points picked, along the Location Line.
    pub(crate) fn shape_lines(&self, pts: &[[f64; 2]]) -> Option<Vec<([f64; 2], [f64; 2])>> {
        self.shape_path(pts).map(|(path, closed)| self.chain_lines(&path, closed))
    }

    /// A pick of a polygon, circle or arc mode: the centre then a point of the shape (the arc: its start, its end, a point
    /// on it); the walls are placed in one undoable step.
    fn wall_shape_click(&mut self, level: ElementId, point: [f64; 2]) {
        let need = if self.wall_mode == 6 { 3 } else { 2 };
        if self.wall_pts.last().is_some_and(|l| (l[0] - point[0]).hypot(l[1] - point[1]) < 0.01) { return; }
        self.wall_pts.push(point);
        if self.wall_pts.len() < need {
            self.message = match (self.wall_mode, self.wall_pts.len()) {
                (6, 1) => "Wall (arc): pick the end of the arc (Esc cancels)".into(),
                (6, _) => "Wall (arc): pick a point the arc passes through (Esc cancels)".into(),
                (3, _) => "Wall (inscribed polygon): pick a corner (Esc cancels)".into(),
                (4, _) => "Wall (circumscribed polygon): pick the middle of a side (Esc cancels)".into(),
                _ => "Wall (circle): pick a point of the circle (Esc cancels)".into(),
            };
            return;
        }
        let pts = std::mem::take(&mut self.wall_pts);
        let Some(lines) = self.shape_lines(&pts) else { self.message = "Wall: those points make no shape (too small, or on a line)".into(); return };
        let m = |v: f64| v / 1000.0;
        match self.document.place_walls(level, &lines, m(self.width_mm), m(self.height_mm), m(self.elevation_mm), self.wall_type) {
            Ok(ids) => self.message = format!("{} walls placed{}; undo removes them together", ids.len(), if self.wall_mode >= 5 { " (a curve is built of straight walls)" } else { "" }),
            Err(e) => self.message = format!("Walls rejected: {e:?}"),
        }
    }

    /// Pick Lines: a wall along the line of `hit` (a grid, wall or beam), on the Location Line, facing the side of `at`.
    pub(crate) fn wall_pick_line(&mut self, level: ElementId, hit: ElementId, at: [f64; 2]) {
        let Some((s, t)) = self.document.line_of(hit) else { self.message = "Pick Lines: click a grid, a wall or a beam".into(); return };
        // the exterior is on the left of the direction: the click side is the exterior side
        let side = (t[0] - s[0]) * (at[1] - s[1]) - (t[1] - s[1]) * (at[0] - s[0]);
        let (s, t) = if side >= 0.0 { (s, t) } else { (t, s) };
        let (a, b) = self.wall_axis(s, t);
        let m = |v: f64| v / 1000.0;
        match self.document.place_walls(level, &[(a, b)], m(self.width_mm), m(self.height_mm), m(self.elevation_mm), self.wall_type) {
            Ok(ids) => self.message = format!("Wall {} placed on the line; undo removes it", ids[0].0),
            Err(e) => self.message = format!("Wall rejected: {e:?}"),
        }
    }

    /// Spacebar on a selected wall turns it over, as Revit does.
    pub(crate) fn flip_selected_wall(&mut self) {
        if self.flip_selected_section() { return; }
        let Some(id) = self.selected_id() else { return };
        if self.document.get(id).is_none_or(|e| e.category != Category::Wall) { return; }
        match self.document.flip_wall_commands(id).and_then(|ops| self.document.transact(self.document.revision(), &ops).map_err(|e| format!("{e:?}"))) {
            Ok(_) => self.message = "Wall flipped; undo takes it back".into(),
            Err(e) => self.message = format!("Cannot flip: {e}"),
        }
    }

    /// A click of the Tag tool on `host`: a tag on it, standing off on a leader when the options bar says so.
    pub(crate) fn tag_click(&mut self, host: ElementId, at: [f64; 2]) {
        let head = if self.tag_leader { [at[0] + 0.6, at[1] + 0.6] } else { at };
        match self.document.place_tag(host, head, at) {
            Ok(id) => self.message = format!("Tagged {}; undo removes the tag", self.document.tag_text(id)),
            Err(_) => self.message = "Tag: click a building element (walls, doors, ducts, equipment, ...)".into(),
        }
    }

    /// A pick of the Grid tool: the first starts a grid line, the second draws it, named after the last one.
    pub(crate) fn grid_click(&mut self, point: [f64; 2]) {
        let Some(start) = self.beam_start else { self.beam_start = Some(point); self.message = "Grid: pick the end (Shift keeps it square, Esc ends)".into(); return; };
        if (point[0] - start[0]).hypot(point[1] - start[1]) < 0.01 { return; }
        let dx = (point[0] - start[0]).abs();
        let dy = (point[1] - start[1]).abs();
        let name = if dx >= dy {
            crate::grid_tools::next_grid_number(&self.document)
        } else {
            crate::grid_tools::next_grid_letter(&self.document)
        };
        match self.document.place_grid(start, point, &name) {
            Ok(id) => { self.beam_start = None; let name = self.document.get(id).map(|g| g.name.clone()).unwrap_or_default(); self.message = format!("Grid {name} placed; undo removes it"); }
            Err(error) => self.message = format!("Grid rejected: {error:?}"),
        }
    }

    /// The Level tool: adds a level at the elevation of the options bar and opens nothing; its plan view is listed.
    pub(crate) fn level_click(&mut self) {
        let name = self.level_name.clone();
        match self.document.place_level(&name, self.level_elev_mm / 1000.0) {
            Ok(id) => {
                let n = self.document.get(id).map(|l| l.name.clone()).unwrap_or_default();
                self.level_elev_mm += 3000.0;
                self.level_name.clear();
                self.set_view(crate::View::Level(id.0));
                self.message = format!("{n} added with its plan view; undo removes it");
            }
            Err(error) => self.message = format!("Level rejected ({error:?}): a level already stands at that elevation"),
        }
    }

    pub(crate) fn beam_click(&mut self, level: ElementId, point: [f64; 2]) {
        match self.beam_start {
            None => { self.beam_start = Some(point); self.message = "Beam: click the end".into(); }
            Some(start) => if self.place_beam_on(level, start, point).is_some() { self.beam_start = None; },
        }
    }

    pub(crate) fn apply_duct_edit(&mut self) {
        if self.selected_id() != Some(ElementId(1)) {
            self.message = "Select the duct to edit its parameters".into();
            return;
        }
        self.apply_selected_edit();
    }

    pub(crate) fn apply_selected_edit(&mut self) {
        let Some(el) = self.selected_id().and_then(|id| self.document.get(id)) else { return; };
        if !self.has_pending_edit() { return; }
        let id = el.id;
        let mut commands = Vec::new();
        if matches!(el.category, Category::Footing | Category::WallFooting) {
            commands.push(Command::Resize { id, width_m: self.width_mm / 1000.0, height_m: self.height_mm / 1000.0 });
            commands.push(Command::SetMaterial { id, material: self.material_pick });
        }
        if el.category == Category::Family {
            let turn = if matches!(el.geometry, Some(Geometry::InWall { .. })) { 0.0 } else { self.angle_deg };
            // a changed type starts from its own values; the same type keeps the edited ones
            let overrides = if el.type_name == self.family_type { self.family_overrides(&el.family, &self.family_type, &self.family_values) } else { Vec::new() };
            let texts: Vec<(String, String)> = if el.type_name == self.family_type {
                let base = crate::export::instance_texts(&el.family, &self.family_type, &[], &[]).unwrap_or_default();
                self.family_texts.iter().filter(|t| base.iter().find(|b| b.0 == t.0).is_none_or(|b| b.1 != t.1)).map(|t| (t.0.clone(), t.1.clone())).collect()
            } else { Vec::new() };
            if crate::export::instance_values_with(&el.family, &self.family_type, &overrides, &texts).is_none() {
                self.message = "Those values do not fit the family (out of range?)".into();
                return;
            }
            commands.extend([Command::SetFamily { id, family: el.family.clone(), type_name: self.family_type.clone(), overrides, angle_deg: turn }, Command::SetFamilyTexts { id, texts },
                Command::MoveElevation { id, elevation_m: self.elevation_mm / 1000.0 }]);
            match self.document.transact(self.document.revision(), &commands) {
                Ok(_) => { self.sync_duct_controls(); self.message = format!("Element {} updated", id.0); }
                Err(error) => self.message = format!("Edit rejected: {error:?}"),
            }
            return;
        }
        if el.category == Category::Railing {
            commands.extend([Command::Resize { id, width_m: self.width_mm / 1000.0, height_m: self.height_mm / 1000.0 }, Command::MoveElevation { id, elevation_m: self.elevation_mm / 1000.0 },
                Command::SetBeamSystem { id, spacing_m: self.spacing_mm / 1000.0, angle_deg: 0.0 }]);
            match self.document.transact(self.document.revision(), &commands) {
                Ok(_) => { self.sync_duct_controls(); self.message = format!("Railing {} updated", id.0); }
                Err(error) => self.message = format!("Edit rejected: {error:?}"),
            }
            return;
        }
        if el.category == Category::Rebar {
            let (count, spacing) = (self.bar_count.max(0.0) as u32, self.spacing_mm / 1000.0);
            commands.extend([Command::Resize { id, width_m: self.width_mm / 1000.0, height_m: self.height_mm / 1000.0 }, Command::MoveElevation { id, elevation_m: self.elevation_mm / 1000.0 },
                Command::SetBeamSystem { id, spacing_m: spacing, angle_deg: 0.0 }, Command::SetCount { id, count }]);
            let shared = self.document.follow_type(id, &commands);
            commands.extend(shared);
            match self.document.transact(self.document.revision(), &commands) {
                Ok(_) => { self.sync_duct_controls(); self.message = format!("Rebar {} updated", id.0); }
                Err(error) => self.message = format!("Edit rejected: {error:?}"),
            }
            return;
        }
        if el.category == Category::Truss {
            let Some(Geometry::Linear { .. }) = el.geometry else { return; };
            let m = |v: f64| v / 1000.0;
            let spec = TrussSpec { start: [m(self.axis_mm[0]), m(self.axis_mm[1])], end: [m(self.axis_mm[2]), m(self.axis_mm[3])], spacing_m: m(self.spacing_mm),
                depth_m: m(self.height_mm), member_m: m(self.width_mm), elevation_m: m(self.elevation_mm), material: self.material_pick };
            match self.document.edit_truss(id, &spec) {
                Ok(()) => { self.sync_duct_controls(); self.message = format!("Truss {} updated; its members were made again", id.0); }
                Err(error) => self.message = format!("Edit rejected: {error:?}"),
            }
            return;
        }
        if el.category == Category::BeamSystem {
            let Some(Geometry::Area { level }) = el.geometry else { return; };
            let _ = level;
            let m = |v: f64| v / 1000.0;
            let spec = BeamSystemSpec { outline: el.outline.clone(), spacing_m: m(self.spacing_mm), angle_deg: self.angle_deg, width_m: m(self.width_mm),
                depth_m: if self.section_pick == 1 { m(self.width_mm) } else { m(self.height_mm) }, elevation_m: m(self.elevation_mm), section: self.section_from_controls(),
                usage: self.usage_pick, material: self.material_pick };
            match self.document.edit_beam_system(id, &spec) {
                Ok(()) => { self.sync_duct_controls(); self.message = format!("Beam system {} updated; its beams were made again", id.0); }
                Err(error) => self.message = format!("Edit rejected: {error:?}"),
            }
            return;
        }
        if el.category == Category::Slab {
            commands.push(Command::Resize { id, width_m: self.width_mm / 1000.0, height_m: self.width_mm / 1000.0 });
            commands.push(Command::SetSlabKind { id, kind: self.slab_kind_pick });
            commands.push(Command::SetMaterial { id, material: self.material_pick });
        }
        if matches!(el.category, Category::Wall | Category::Beam | Category::Column | Category::Duct | Category::Pipe | Category::Stair) || (el.category == Category::Opening && matches!(el.geometry, Some(Geometry::InWall { .. }))) {
            let round = el.category == Category::Pipe || (matches!(el.category, Category::Beam | Category::Column) && self.section_pick == 1);
            commands.push(Command::Resize { id, width_m: self.width_mm / 1000.0,
                height_m: if round { self.width_mm } else { self.height_mm } / 1000.0 });
        }
        if matches!(el.category, Category::Beam | Category::Column) {
            commands.push(Command::SetSection { id, section: self.section_from_controls() });
            if el.category == Category::Beam {
                commands.push(Command::SetUsage { id, usage: self.usage_pick });
                commands.push(Command::SetJoins { id, start: self.join_pick[0], end: self.join_pick[1] });
            }
            commands.push(Command::SetMaterial { id, material: self.material_pick });
        }
        if el.category != Category::Grid { commands.push(Command::MoveElevation { id, elevation_m: self.elevation_mm / 1000.0 }); }
        if el.category.supports_slope() { commands.push(Command::SetSlope { id, slope_ratio: self.slope_percent / 100.0 }); }
        let start = [self.axis_mm[0] / 1000.0, self.axis_mm[1] / 1000.0];
        let end = [self.axis_mm[2] / 1000.0, self.axis_mm[3] / 1000.0];
        let geometry = match el.geometry {
            Some(Geometry::Linear { level, .. }) => Some(Geometry::Linear { start, end, level }),
            Some(Geometry::Grid { .. }) => Some(Geometry::Grid { start, end, label_off: [0.0, 0.0] }),
            Some(Geometry::Pad { at, level, .. }) => Some(Geometry::Pad { at, level, thickness_m: self.footing_thickness_mm / 1000.0 }),
            _ => None,
        };
        if let Some(geometry) = geometry { commands.push(Command::SetGeometry { id, geometry }); }
        let shared = self.document.follow_type(id, &commands);
        let sharing = el.type_id.map(|t| self.document.instances_of(t).len());
        commands.extend(shared);
        match self.document.transact(self.document.revision(), &commands) {
            Ok(revision) => {
                self.sync_duct_controls();
                self.message = match sharing {
                    Some(n) => format!("Element {} updated with its type and the {n} elements of that type — revision {revision}", id.0),
                    None => format!("Element {} updated — revision {revision}", id.0),
                };
            }
            Err(error) => self.message = format!("Edit rejected: {error:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_footing_clicked_on_a_unit_is_its_pad_and_moves_with_the_unit() {
        // B24
        let mut app = Desktop::sample();
        let level = ElementId(10);
        let unit = app.document.place_family(level, [2.0, 4.5], &FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 2.2 }).unwrap();
        app.footing_click(level, [2.1, 4.4]);
        assert!(app.message.starts_with("Pad"), "{}", app.message);
        let pad = app.document.elements().find(|e| e.category == Category::Footing).unwrap().id;
        assert_eq!(app.document.get(pad).unwrap().parent, Some(unit));
        let ops = app.document.transform_commands(&[unit], openbim_document::Xf::Move([1.0, 0.0])).unwrap();
        app.document.transact(app.document.revision(), &ops).unwrap();
        assert!(matches!(app.document.get(pad).unwrap().geometry, Some(Geometry::Pad { at, .. }) if (at[0] - 3.0).abs() < 1e-9 && (at[1] - 4.5).abs() < 1e-9));
        // away from any unit it is a plain footing, as before
        app.footing_click(level, [30.0, 30.0]);
        let plain = app.document.elements().filter(|e| e.category == Category::Footing && e.parent.is_none()).count();
        assert_eq!(plain, 1);
    }

    #[test]
    fn a_tool_does_not_inherit_the_height_of_the_tool_before_it() {
        // B5: the Ceiling button sets 2.7 m; the Door button after it must put its door on the floor
        let mut app = Desktop::sample();
        app.run_command("ceiling", "Ceiling");
        assert_eq!(app.slab_offset_mm, 2700.0);
        app.run_command("door", "Door");
        assert_eq!(app.slab_offset_mm, 0.0, "the door does not start 2.7 m up");
        app.run_command("ceiling", "Ceiling");
        app.run_command("component", "Component");
        assert_eq!(app.slab_offset_mm, 0.0);
        app.run_command("ceiling", "Ceiling");
        app.run_command("floor", "Floor");
        assert_eq!((app.slab_offset_mm, app.slab_thickness_mm), (0.0, 200.0));
    }

    #[test]
    fn undoing_a_level_that_was_just_made_leaves_a_plan_on_a_level_that_exists() {
        // B25: the view pointed at the new level; after undo it must resolve to a level of the model
        let mut app = Desktop::sample();
        app.level_elev_mm = 9000.0;
        app.level_name = "Roof level".into();
        app.level_click();
        let made = match app.view { crate::View::Level(id) => ElementId(id), v => panic!("{v:?}") };
        assert_eq!(app.plan_level(), made);
        app.document.undo().unwrap();
        assert!(app.document.get(made).is_none(), "the level is gone");
        let now = app.plan_level();
        assert!(now != ElementId(0) && app.document.get(now).is_some_and(|l| l.geometry == Some(Geometry::Level)), "{now:?}");
        // and a tool started on it works
        assert!(app.document.place_walls(now, &[([0.0, 0.0], [3.0, 0.0])], 0.2, 3.0, 0.0, None).is_ok());
    }
    #[test]
    fn round_and_i_beams_are_placed_edited_drawn_closed_in_3d_and_undone() {
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Beam);
        (app.width_mm, app.height_mm, app.elevation_mm) = (200.0, 400.0, 2800.0);
        app.axis_mm = [1000.0, 1000.0, 5000.0, 1000.0];
        app.section_pick = 2;
        (app.web_mm, app.flange_mm) = (10.0, 20.0);
        app.place_beam_from_controls();
        let id = app.selected_id().unwrap();
        assert_eq!(app.document.get(id).unwrap().section, Section::IShape { web_m: 0.01, flange_m: 0.02 });
        assert_eq!(app.section_pick, 2);
        assert!(!app.has_pending_edit());
        // its 3D mesh is closed: each edge is used twice, with opposite directions, and the volume is area x length
        let batch = crate::view3d::batch(&app.document, None);
        let ei = batch.elements.iter().position(|e| e.id == id.0).unwrap() as u32;
        let tris: Vec<[u32; 3]> = batch.tris.iter().zip(&batch.tri_element).filter(|(_, e)| **e == ei).map(|(t, _)| *t).collect();
        let mut edges = std::collections::HashMap::<(u32, u32), i32>::new();
        for t in &tris { for k in 0..3 { *edges.entry((t[k], t[(k + 1) % 3])).or_default() += 1; } }
        assert!(edges.iter().all(|(&(a, b), &n)| n == 1 && edges.get(&(b, a)) == Some(&1)), "closed and consistently wound");
        let vol: f64 = tris.iter().map(|t| {
            let p = t.map(|i| batch.verts[i as usize].map(f64::from));
            (p[0][0] * (p[1][1] * p[2][2] - p[1][2] * p[2][1]) - p[0][1] * (p[1][0] * p[2][2] - p[1][2] * p[2][0]) + p[0][2] * (p[1][0] * p[2][1] - p[1][1] * p[2][0])) / 6.0
        }).sum();
        assert!((vol - 0.0116 * 4.0).abs() < 1e-3, "{vol}");
        // the properties turn it into a round beam of 300 mm: one transaction
        app.section_pick = 1;
        app.width_mm = 300.0;
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        let el = app.document.get(id).unwrap();
        assert_eq!((el.section, el.width_m, el.height_m), (Section::Circle, 0.3, 0.3));
        assert!(!app.has_pending_edit());
        app.undo_document();
        assert_eq!(app.document.get(id).unwrap().section, Section::IShape { web_m: 0.01, flange_m: 0.02 });
        // an I-section whose web is wider than its flanges is rejected and nothing changes
        app.sync_duct_controls();
        app.web_mm = 250.0;
        app.apply_selected_edit();
        assert!(app.message.contains("rejected"), "{}", app.message);
        assert_eq!(app.document.get(id).unwrap().section, Section::IShape { web_m: 0.01, flange_m: 0.02 });
    }

    #[test]
    fn columns_are_placed_by_a_click_stand_to_the_next_level_are_edited_drawn_and_exported() {
        let mut app = Desktop::sample();
        app.run_command("structural_column", "Column");
        assert!(app.tool == Tool::Column);
        (app.width_mm, app.height_mm) = (400.0, 400.0);
        app.material_pick = openbim_document::Material::Concrete;
        // Level 2 is the top level: nothing above it to reach
        app.column_click(ElementId(11), [2.0, 2.0]);
        assert!(app.message.contains("no level above"), "{}", app.message);
        assert!(app.document.elements().all(|e| e.category != Category::Column));
        app.column_click(ElementId(10), [2.0, 2.0]);
        let col = app.document.elements().find(|e| e.category == Category::Column).expect("placed").id;
        assert!(app.tool == Tool::Column, "the tool stays on");
        // 3 m high between Level 1 and Level 2, 0.4 x 0.4: a closed solid of 0.48 m3 in the 3D view
        let batch = crate::view3d::batch(&app.document, None);
        let ei = batch.elements.iter().position(|e| e.id == col.0 && e.kind == openbim_view::Kind::Column).expect("in 3D") as u32;
        let vol: f64 = batch.tris.iter().zip(&batch.tri_element).filter(|(_, e)| **e == ei).map(|(t, _)| {
            let p = t.map(|i| batch.verts[i as usize].map(f64::from));
            (p[0][0] * (p[1][1] * p[2][2] - p[1][2] * p[2][1]) - p[0][1] * (p[1][0] * p[2][2] - p[1][2] * p[2][0]) + p[0][2] * (p[1][0] * p[2][1] - p[1][1] * p[2][0])) / 6.0
        }).sum();
        assert!((vol - 0.48).abs() < 1e-3, "{vol}");
        // select it and make it an I-section steel column with a base offset
        app.select_element(Some(col));
        assert!(!app.has_pending_edit());
        (app.section_pick, app.width_mm, app.height_mm, app.web_mm, app.flange_mm) = (2, 300.0, 300.0, 10.0, 15.0);
        app.material_pick = openbim_document::Material::Steel;
        app.elevation_mm = 200.0;
        app.apply_selected_edit();
        let el = app.document.get(col).unwrap();
        assert_eq!((el.material, el.elevation_m, el.section), (openbim_document::Material::Steel, 0.2, Section::IShape { web_m: 0.01, flange_m: 0.015 }));
        // exported as an IfcColumn on an exact... I footprint, with its material
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-17T00:00:00").unwrap();
        assert_eq!(out.columns, 1);
        assert!(out.text.contains("IFCCOLUMN(") && out.text.contains("IFCMATERIAL('Steel')"));
        // a circle column
        app.select_element(Some(col));
        (app.section_pick, app.width_mm) = (1, 350.0);
        app.apply_selected_edit();
        assert_eq!((app.document.get(col).unwrap().width_m, app.document.get(col).unwrap().height_m), (0.35, 0.35));
        app.undo_document();
        app.undo_document();
        assert_eq!(app.document.get(col).unwrap().section, Section::Rectangle);
    }

    #[test]
    fn the_brace_command_places_a_sloped_steel_member_that_is_edited_and_exported() {
        let mut app = Desktop::sample();
        app.run_command("brace", "Brace");
        assert!(app.tool == Tool::Beam && app.usage_pick == Usage::Brace);
        (app.width_mm, app.height_mm, app.elevation_mm, app.brace_rise_mm) = (100.0, 100.0, 100.0, 3000.0);
        app.material_pick = openbim_document::Material::Steel;
        app.beam_click(ElementId(10), [6.0, 1.0]);
        app.beam_click(ElementId(10), [6.0, 4.0]);
        let brace = app.document.elements().find(|e| e.usage == Usage::Brace).expect("placed");
        assert_eq!((brace.slope_ratio, brace.material), (Some(1.0), openbim_document::Material::Steel));
        // select it, make it a joist with a gentler slope
        let id = brace.id;
        app.select_element(Some(id));
        assert!(!app.has_pending_edit());
        app.usage_pick = Usage::Joist;
        app.slope_percent = 10.0;
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        let el = app.document.get(id).unwrap();
        assert_eq!((el.usage, el.slope_ratio), (Usage::Joist, Some(0.1)));
        // the beam command goes back to girders
        app.run_command("beam", "Beam");
        assert!(app.usage_pick == Usage::Girder);
        app.undo_document();
        assert_eq!(app.document.get(id).unwrap().usage, Usage::Brace);
    }

    #[test]
    fn two_clicks_in_a_plan_place_a_beam_on_that_level_and_the_tool_stays_on() {
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Beam);
        (app.width_mm, app.height_mm, app.elevation_mm) = (300.0, 500.0, 2500.0);
        let n = app.document.elements().count();
        app.beam_click(ElementId(11), [1.0, 1.0]);
        assert_eq!(app.document.elements().count(), n, "one click places nothing");
        app.beam_click(ElementId(11), [1.0, 1.0]);
        assert_eq!(app.document.elements().count(), n, "a zero-length beam is refused");
        assert_eq!(app.beam_start, Some([1.0, 1.0]), "and the start is kept");
        app.beam_click(ElementId(11), [4.0, 1.0]);
        assert_eq!(app.document.elements().count(), n + 1);
        assert!(app.tool == Tool::Beam && app.beam_start.is_none());
        let beam = app.document.elements().find(|e| e.category == Category::Beam).unwrap();
        assert!(matches!(beam.geometry, Some(Geometry::Linear { level, start: [1.0, 1.0], end: [4.0, 1.0] }) if level == ElementId(11)));
        // its centre is 2.5 m above Level 2 (3 m)
        let z = app.document.mesh(beam.id).unwrap().vertices.iter().map(|v| v[2]).fold(0.0, f64::max);
        assert!((z - (3.0 + 2.5 + 0.25)).abs() < 1e-9, "{z}");
        // the options bar places on the level it was given, not the lowest
        app.beam_level = Some(ElementId(11));
        app.axis_mm = [0.0, 2000.0, 3000.0, 2000.0];
        app.place_beam_from_controls();
        let last = app.selected_id().unwrap();
        assert!(matches!(app.document.get(last).unwrap().geometry, Some(Geometry::Linear { level, .. }) if level == ElementId(11)));
        // choosing the tool again forgets a half-placed beam
        app.choose_tool(Tool::Beam);
        app.beam_click(ElementId(10), [0.0, 0.0]);
        app.choose_tool(Tool::Beam);
        assert!(app.beam_start.is_none());
    }

    #[test]
    fn a_beam_is_placed_from_the_options_bar_shown_in_3d_and_undone() {
        let mut app = Desktop::sample();
        app.choose_tool(Tool::Beam);
        (app.width_mm, app.height_mm, app.elevation_mm) = (200.0, 400.0, 2800.0);
        app.axis_mm = [1000.0, 1000.0, 5000.0, 1000.0];
        let before = crate::view3d::batch(&app.document, None).elements.len();
        app.place_beam_from_controls();
        let id = app.selected_id().expect("the new beam is selected");
        assert!(app.tool == Tool::Select);
        let el = app.document.get(id).unwrap();
        assert_eq!(el.category, Category::Beam);
        assert!(!app.has_pending_edit(), "the controls show the beam");
        let batch = crate::view3d::batch(&app.document, None);
        assert_eq!(batch.elements.len(), before + 1);
        assert!(batch.elements.iter().any(|e| e.kind == openbim_view::Kind::Beam && e.id == id.0));
        // 4 m long, 0.2 x 0.4: the mesh is that box centred at 2.8 m
        let v = app.document.mesh(id).unwrap().vertices;
        let (lo, hi) = (v.iter().map(|p| p[2]).fold(f64::MAX, f64::min), v.iter().map(|p| p[2]).fold(f64::MIN, f64::max));
        assert!((lo - 2.6).abs() < 1e-9 && (hi - 3.0).abs() < 1e-9, "{lo} {hi}");
        // a zero-length beam is refused and nothing is added
        app.choose_tool(Tool::Beam);
        app.axis_mm = [1000.0, 1000.0, 1000.0, 1000.0];
        let n = app.document.elements().count();
        app.place_beam_from_controls();
        assert_eq!(app.document.elements().count(), n);
        assert!(app.message.contains("rejected"));
        app.undo_document();
        assert!(app.document.get(id).is_none(), "undo removes the beam");
    }
    #[test]
    fn category_controls_update_the_selected_geometry_and_undo() {
        let mut app = Desktop::sample();
        app.select_element(Some(ElementId(20)));
        assert!(!app.has_pending_edit());
        let original = app.document.mesh(ElementId(20)).unwrap();
        app.width_mm = 400.0;
        app.height_mm = 4000.0;
        app.apply_selected_edit();
        assert_ne!(app.document.mesh(ElementId(20)).unwrap(), original);
        app.undo_document();
        assert_eq!(app.document.mesh(ElementId(20)).unwrap(), original);
        app.select_element(Some(ElementId(100)));
        app.axis_mm[2] = 1000.0;
        app.apply_selected_edit();
        assert!(matches!(app.document.get(ElementId(100)).unwrap().geometry,
            Some(Geometry::Grid { end: [1.0, 7.0], .. })));
        app.select_element(Some(ElementId(10)));
        app.elevation_mm = 1000.0;
        app.apply_selected_edit();
        assert_eq!(app.document.mesh(ElementId(20)).unwrap().vertices[0][2], 1.0);
        app.select_element(Some(ElementId(2)));
        app.width_mm = 150.0;
        app.slope_percent = -2.0;
        app.apply_selected_edit();
        let pipe = app.document.get(ElementId(2)).unwrap();
        assert_eq!(pipe.width_m, 0.15);
        assert_eq!(pipe.height_m, 0.15);
        assert_eq!(pipe.slope_ratio, Some(-0.02));
    }

    #[test]
    fn a_roof_is_drawn_like_a_slab_slopes_from_its_eaves_shows_in_3d_and_is_exported_as_a_roof() {
        let mut app = Desktop::sample();
        app.run_command("roof", "Roof");
        assert!(app.tool == Tool::Slab && app.slab_kind_pick == openbim_document::SlabKind::Roof && app.roof_slope_deg == 30.0);
        assert!(app.message.contains("Roof by Footprint"), "{}", app.message);
        // an L slopes on all its edges and has a valley: a closed solid with the volume of its footprint x thickness / cos(slope)
        let volume_of = |app: &Desktop, id: ElementId| -> f64 {
            let batch = crate::view3d::batch(&app.document, None);
            let ei = batch.elements.iter().position(|e| e.id == id.0).expect("in 3D") as u32;
            batch.tris.iter().zip(&batch.tri_element).filter(|(_, e)| **e == ei).map(|(t, _)| {
                let p = t.map(|i| batch.verts[i as usize].map(f64::from));
                (p[0][0] * (p[1][1] * p[2][2] - p[1][2] * p[2][1]) - p[0][1] * (p[1][0] * p[2][2] - p[1][2] * p[2][0]) + p[0][2] * (p[1][0] * p[2][1] - p[1][1] * p[2][0])) / 6.0
            }).sum()
        };
        for p in [[0.0, 0.0], [6.0, 0.0], [6.0, 3.0], [3.0, 3.0], [3.0, 6.0], [0.0, 6.0]] { app.slab_click(ElementId(10), p, false); }
        app.finish_slab(ElementId(10));
        assert!(app.slab_points.is_empty(), "{}", app.message);
        let l = app.document.elements().find(|e| e.is_roof()).expect("the L roof").id;
        let lf = app.document.roof_figures(l).unwrap();
        assert!((lf.plan_m2 - 27.0).abs() < 1e-9 && lf.rise_m > 0.8, "{lf:?}");
        assert!((volume_of(&app, l) - 27.0 * 0.2 / 30f64.to_radians().cos()).abs() < 1e-3, "{}", volume_of(&app, l));
        // a gable end on it is refused (not made yet), and the roof stays as it was
        let before = app.document.get(l).unwrap().slope_edges.clone();
        let refused = app.document.transact(app.document.revision(), &[Command::SetRoof { id: l, slope_ratio: 0.5, edges: vec![true, false, true, false, true, true] }]);
        assert!(refused.is_err());
        assert_eq!(app.document.get(l).unwrap().slope_edges, before);
        // exported as a roof, in the file the app reads back
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-19T00:00:00").unwrap();
        assert_eq!(out.text.matches("IFCROOF(").count(), 1);
        let ops = app.document.delete_commands(&[l]).unwrap();
        app.document.transact(app.document.revision(), &ops).unwrap();
        // a 10 x 6 hip roof at 30 degrees, 200 mm thick, its eaves 3 m up
        app.slab_offset_mm = 3000.0;
        for p in [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]] { app.slab_click(ElementId(10), p, false); }
        app.slab_click(ElementId(10), [0.0, 0.0], true);
        assert!(app.slab_points.is_empty(), "{}", app.message);
        let id = app.document.elements().find(|e| e.is_roof()).expect("placed").id;
        let f = app.document.roof_figures(id).unwrap();
        assert!((f.rise_m - 3.0 * 30f64.to_radians().tan()).abs() < 1e-9, "{f:?}");
        assert!((f.surface_m2 - 60.0 / 30f64.to_radians().cos()).abs() < 1e-6, "{f:?}");
        // in 3D it is a closed solid of that volume
        let batch = crate::view3d::batch(&app.document, None);
        let ei = batch.elements.iter().position(|e| e.id == id.0).expect("in 3D") as u32;
        let volume: f64 = batch.tris.iter().zip(&batch.tri_element).filter(|(_, e)| **e == ei).map(|(t, _)| {
            let p = t.map(|i| batch.verts[i as usize].map(f64::from));
            (p[0][0] * (p[1][1] * p[2][2] - p[1][2] * p[2][1]) - p[0][1] * (p[1][0] * p[2][2] - p[1][2] * p[2][0]) + p[0][2] * (p[1][0] * p[2][1] - p[1][1] * p[2][0])) / 6.0
        }).sum();
        assert!((volume - 60.0 * 0.2 / 30f64.to_radians().cos()).abs() < 1e-3, "{volume}");
        // selecting it changes nothing; making two opposite edges the sloping ones makes it a gable
        app.select_element(Some(id));
        assert!(!app.has_pending_edit());
        let el = app.document.get(id).unwrap().clone();
        app.document.transact(app.document.revision(), &[Command::SetRoof { id, slope_ratio: 0.5, edges: vec![true, false, true, false] }]).unwrap();
        assert!((app.document.roof_figures(id).unwrap().rise_m - 1.5).abs() < 1e-9);
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-18T00:00:00").unwrap();
        assert_eq!(out.text.matches("IFCROOF(").count(), 1);
        assert!(out.text.contains(".GABLE_ROOF."));
        assert_eq!(el.slope_edges.len(), 4);
        // undo takes the gable back to the hip, and the next undo removes the roof
        app.document.undo().unwrap();
        assert_eq!(app.document.get(id).unwrap().slope_edges, [true; 4]);
    }

    #[test]
    fn a_ceiling_is_hung_in_the_space_under_the_click_or_sketched_and_is_placed_by_its_underside() {
        let mut app = Desktop::sample();
        let level = ElementId(10);
        let ceilings = |a: &Desktop| a.document.elements().filter(|e| e.category == Category::Slab && e.slab_kind == openbim_document::SlabKind::Ceiling).count();
        app.run_command("ceiling", "Ceiling");
        assert!(app.tool == Tool::Slab && app.slab_kind_pick == openbim_document::SlabKind::Ceiling && app.ceiling_auto && app.slab_offset_mm == 2700.0, "{}", app.message);
        assert!(app.message.contains("Automatic Ceiling"), "{}", app.message);
        // a click in a space of the sample hangs one in it
        app.slab_click(level, [2.0, 3.0], false);
        assert_eq!(ceilings(&app), 1, "{}", app.message);
        let c = app.document.elements().find(|e| e.slab_kind == openbim_document::SlabKind::Ceiling).unwrap().clone();
        assert!((openbim_geom::profile::polygon_area(&c.outline).abs() - 3.8 * 5.8).abs() < 1e-6);
        assert_eq!((c.elevation_m, c.height_m), (2.7, 0.02));
        // the same height in the same space is refused; a click outside every space too
        app.slab_click(level, [2.5, 3.5], false);
        assert_eq!(ceilings(&app), 1);
        assert!(app.message.contains("already"), "{}", app.message);
        app.slab_click(level, [-5.0, -5.0], false);
        assert_eq!(ceilings(&app), 1);
        assert!(app.message.contains("closed"), "{}", app.message);
        // its underside is at 2.7 m above the level, its top 20 mm higher
        let z0 = app.document.get(level).unwrap().elevation_m;
        let m = app.document.mesh(c.id).unwrap();
        let (lo, hi) = (m.vertices.iter().map(|v| v[2]).fold(f64::MAX, f64::min), m.vertices.iter().map(|v| v[2]).fold(f64::MIN, f64::max));
        assert!((lo - (z0 + 2.7)).abs() < 1e-9 && (hi - (z0 + 2.72)).abs() < 1e-9, "{lo} {hi}");
        // sketched: Automatic off, four corners, Enter
        app.ceiling_auto = false;
        app.slab_offset_mm = 2400.0;
        for p in [[4.2, 0.2], [7.8, 0.2], [7.8, 5.8], [4.2, 5.8]] { app.slab_click(level, p, false); }
        assert_eq!(app.slab_points.len(), 4);
        app.finish_slab(level);
        assert_eq!(ceilings(&app), 2, "{}", app.message);
        // exported as IfcCovering CEILING
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-19T00:00:00").unwrap();
        assert_eq!(out.text.matches("IFCCOVERING(").count(), 2);
        assert!(out.text.contains(".CEILING."));
    }

    #[test]
    fn a_stair_is_drawn_by_two_clicks_with_risers_from_the_level_height_edited_and_exported() {
        let mut app = Desktop::sample();
        let level = ElementId(10);
        app.run_command("stair", "Stair");
        assert!(app.tool == Tool::Stair && app.message.contains("first riser"), "{}", app.message);
        let above = app.document.level_above(level).unwrap();
        let rise = app.document.get(above).unwrap().elevation_m - app.document.get(level).unwrap().elevation_m;
        app.stair_click(level, [1.0, 1.0]);
        assert!(app.message.contains("direction"), "{}", app.message);
        app.stair_click(level, [1.0, 9.0]);
        let id = app.document.elements().find(|e| e.category == Category::Stair).expect("placed").id;
        let f = app.document.stair_figures(id).unwrap();
        assert_eq!(f.risers, openbim_document::risers_for(rise, 0.175));
        assert!(f.riser_m <= 0.175 + 1e-9 && (f.tread_m - 0.28).abs() < 1e-9 && (f.rise_m - rise).abs() < 1e-9, "{f:?}");
        assert!(app.message.contains("risers of"), "{}", app.message);
        // a narrower stair with a higher riser limit and the next stair starts afresh
        (app.stair_width_mm, app.stair_riser_mm) = (900.0, 200.0);
        app.stair_click(level, [3.0, 1.0]);
        app.stair_click(level, [3.0, 9.0]);
        let second = app.document.elements().filter(|e| e.category == Category::Stair).last().unwrap().clone();
        assert!(second.count < f.risers && second.width_m == 0.9);
        // selecting one: width and total rise are its dimensions; widen it to 1200 mm
        app.select_element(Some(id));
        assert!(!app.has_pending_edit());
        app.width_mm = 1200.0;
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        assert_eq!(app.document.get(id).unwrap().width_m, 1.2, "{}", app.message);
        // exported as an IfcStair with its figures
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-19T00:00:00").unwrap();
        assert_eq!(out.text.matches("IFCSTAIR(").count(), 2);
        assert!(out.text.contains("Pset_StairCommon"));
        // Esc after the first click forgets it
        app.edit_pts.clear();
        app.stair_click(level, [5.0, 5.0]);
        assert_eq!(app.edit_pts.len(), 1);
    }

    #[test]
    fn an_l_or_u_stair_is_chosen_in_the_options_bar_placed_as_one_group_and_exported_with_its_landing() {
        use openbim_document::StairShape;
        let mut app = Desktop::sample();
        let level = ElementId(10);
        app.run_command("stair", "Stair");
        let stairs = |a: &Desktop| a.document.elements().filter(|e| e.category == Category::Stair).count();
        let before = stairs(&app);
        let railings = |a: &Desktop| a.document.elements().filter(|e| e.category == Category::Railing).count();
        // a railing goes on both sides of the stair unless the options bar says otherwise, as in Revit
        assert_eq!(app.stair_railing, openbim_document::RailingSides::Both);
        app.stair_shape = StairShape::L { left: true };
        app.stair_click(level, [1.0, 1.0]);
        app.stair_click(level, [1.0, 9.0]);
        assert_eq!(stairs(&app), before + 3, "two runs and a landing: {}", app.message);
        assert!(app.message.contains("two runs with a landing") && app.message.contains("railing"), "{}", app.message);
        assert_eq!(railings(&app), 2, "one on each side");
        let first = app.document.elements().filter(|e| e.category == Category::Stair && e.parent.is_none()).last().unwrap().id;
        let parts = app.document.stair_parts(first);
        assert_eq!(parts.len(), 3);
        // the figures of the two runs add up to the whole climb
        let climbed: f64 = parts.iter().filter_map(|p| app.document.get(*p)).filter(|e| matches!(e.geometry, Some(Geometry::Linear { .. }))).map(|e| e.height_m).sum();
        let above = app.document.level_above(level).unwrap();
        let rise = app.document.get(above).unwrap().elevation_m - app.document.get(level).unwrap().elevation_m;
        assert!((climbed - rise).abs() < 1e-9, "{climbed} {rise}");
        // selecting the first run and deleting takes the stair with it; undo brings it back whole
        app.select_element(Some(first));
        app.delete_selected();
        assert_eq!((stairs(&app), railings(&app)), (before, 0), "{}", app.message);
        app.document.undo().unwrap();
        assert_eq!((stairs(&app), railings(&app)), (before + 3, 2));
        // exported: one stair of two flights and a landing slab
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-19T00:00:00").unwrap();
        assert_eq!(out.text.matches("IFCSTAIR(").count(), before + 1);
        assert_eq!(out.text.matches("IFCSTAIRFLIGHT(").count(), 2);
        assert_eq!(out.text.matches(".LANDING.").count(), 1);
        assert_eq!(out.text.matches("IFCRAILING(").count(), 2, "the railings of the stair");
        // a U stair, turning right, is placed the same way; too little climb for two runs is refused
        app.stair_shape = StairShape::U { left: false };
        app.stair_railing = openbim_document::RailingSides::None;
        app.stair_click(level, [6.0, 1.0]);
        app.stair_click(level, [6.0, 9.0]);
        assert_eq!(stairs(&app), before + 6, "{}", app.message);
        assert_eq!(railings(&app), 2, "no railing was asked for");
        (app.stair_riser_mm, app.stair_rise_mm) = (2000.0, 3000.0);
        app.stair_click(level, [8.0, 1.0]);
        app.stair_click(level, [8.0, 3.0]);
        assert_eq!(stairs(&app), before + 6, "{}", app.message);
        assert!(app.message.contains("rejected"), "{}", app.message);
    }

    #[test]
    fn slabs_are_drawn_by_clicks_closed_in_3d_edited_into_foundations_and_exported() {
        let mut app = Desktop::sample();
        app.run_command("structural_floor", "Floor");
        assert!(app.tool == Tool::Slab && app.slab_kind_pick == openbim_document::SlabKind::Floor);
        app.material_pick = openbim_document::Material::Concrete;
        // a bow-tie is refused and keeps its corners so that one can be taken back
        for p in [[0.0, 0.0], [2.0, 2.0], [2.0, 0.0], [0.0, 2.0]] { app.slab_click(ElementId(10), p, false); }
        app.finish_slab(ElementId(10));
        assert!(app.message.contains("rejected") && app.slab_points.len() == 4, "{}", app.message);
        app.slab_points.clear();
        // an L of 16 m2, 200 mm thick; the closing click is the first corner again
        for p in [[0.0, 0.0], [5.0, 0.0], [5.0, 2.0], [3.0, 2.0], [3.0, 4.0], [0.0, 4.0]] { app.slab_click(ElementId(10), p, false); }
        app.slab_click(ElementId(10), [0.0, 0.0], true);
        assert!(app.slab_points.is_empty(), "{}", app.message);
        assert!(app.tool == Tool::Slab, "the tool stays on");
        let id = app.document.elements().find(|e| e.category == Category::Slab).expect("placed").id;
        let batch = crate::view3d::batch(&app.document, None);
        let ei = batch.elements.iter().position(|e| e.id == id.0 && e.kind == openbim_view::Kind::Slab).expect("in 3D") as u32;
        let volume = |batch: &openbim_view::Batch| -> f64 {
            batch.tris.iter().zip(&batch.tri_element).filter(|(_, e)| **e == ei).map(|(t, _)| {
                let p = t.map(|i| batch.verts[i as usize].map(f64::from));
                (p[0][0] * (p[1][1] * p[2][2] - p[1][2] * p[2][1]) - p[0][1] * (p[1][0] * p[2][2] - p[1][2] * p[2][0]) + p[0][2] * (p[1][0] * p[2][1] - p[1][1] * p[2][0])) / 6.0
            }).sum()
        };
        assert!((volume(&batch) - 3.2).abs() < 1e-3, "{}", volume(&batch));
        // select it: thickness and top offset are its dimensions; make it a 300 mm foundation sunk 100 mm
        app.select_element(Some(id));
        assert!(!app.has_pending_edit());
        (app.width_mm, app.elevation_mm, app.slab_kind_pick) = (300.0, -100.0, openbim_document::SlabKind::Foundation);
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        let el = app.document.get(id).unwrap();
        assert_eq!((el.height_m, el.elevation_m, el.slab_kind), (0.3, -0.1, openbim_document::SlabKind::Foundation));
        assert!((volume(&crate::view3d::batch(&app.document, None)) - 4.8).abs() < 1e-3);
        // exported as an IfcSlab BASESLAB with its concrete, and the outline kept
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-17T00:00:00").unwrap();
        assert_eq!(out.slabs, 1);
        assert_eq!(out.text.matches("IFCSLAB(").count(), 1);
        assert!(out.text.contains(".BASESLAB."), "foundation");
        assert!(out.text.contains("IFCMATERIAL('Concrete')"));
        assert!(out.text.contains("IFCARBITRARYCLOSEDPROFILEDEF") || out.text.contains("IFCPOLYLINE"), "outline");
        // undo takes the foundation edit back, then the slab
        app.undo_document();
        assert_eq!(app.document.get(id).unwrap().slab_kind, openbim_document::SlabKind::Floor);
        app.undo_document();
        assert!(app.document.elements().all(|e| e.category != Category::Slab));
    }

    #[test]
    fn footings_are_placed_by_a_click_below_the_level_edited_closed_in_3d_and_exported() {
        let mut app = Desktop::sample();
        app.run_command("foundation", "Isolated");
        assert!(app.tool == Tool::Footing);
        (app.width_mm, app.height_mm, app.footing_thickness_mm, app.slab_offset_mm) = (1500.0, 2000.0, 500.0, -100.0);
        app.material_pick = openbim_document::Material::Concrete;
        app.footing_click(ElementId(10), [2.0, 2.0]);
        let id = app.document.elements().find(|e| e.category == Category::Footing).expect("placed").id;
        assert!(app.tool == Tool::Footing, "the tool stays on");
        let ei_vol = |app: &Desktop| -> f64 {
            let batch = crate::view3d::batch(&app.document, None);
            let ei = batch.elements.iter().position(|e| e.id == id.0).unwrap() as u32;
            batch.tris.iter().zip(&batch.tri_element).filter(|(_, e)| **e == ei).map(|(t, _)| {
                let p = t.map(|i| batch.verts[i as usize].map(f64::from));
                (p[0][0] * (p[1][1] * p[2][2] - p[1][2] * p[2][1]) - p[0][1] * (p[1][0] * p[2][2] - p[1][2] * p[2][0]) + p[0][2] * (p[1][0] * p[2][1] - p[1][1] * p[2][0])) / 6.0
            }).sum()
        };
        assert!((ei_vol(&app) - 1.5).abs() < 1e-3, "{}", ei_vol(&app));
        // edit the thickness in the properties: 1.5 x 2 x 0.8
        app.select_element(Some(id));
        assert!(!app.has_pending_edit());
        app.footing_thickness_mm = 800.0;
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        assert!((ei_vol(&app) - 2.4).abs() < 1e-3);
        // exported as an IfcFooting PAD_FOOTING with its concrete
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-17T00:00:00").unwrap();
        assert_eq!(out.text.matches("IFCFOOTING(").count(), 1);
        assert!(out.text.contains(".PAD_FOOTING.") && out.text.contains("IFCMATERIAL('Concrete')"));
        app.undo_document();
        assert!((ei_vol(&app) - 1.5).abs() < 1e-3);
    }

    #[test]
    fn wall_footings_are_hosted_follow_their_wall_and_are_exported_as_strip_footings() {
        let mut app = Desktop::sample();
        app.run_command("footing", "Footing");
        assert!(app.tool == Tool::WallFooting);
        let wall = app.document.elements().find(|e| e.category == Category::Wall && matches!(e.geometry, Some(Geometry::Linear { level, .. }) if level == ElementId(10))).expect("a wall").id;
        let Some(Geometry::Linear { start, end, .. }) = app.document.get(wall).unwrap().geometry else { unreachable!() };
        let run = (end[0] - start[0]).hypot(end[1] - start[1]);
        (app.width_mm, app.height_mm, app.slab_offset_mm) = (600.0, 400.0, 0.0);
        app.material_pick = openbim_document::Material::Concrete;
        app.wall_footing_click(ElementId(10));
        assert!(app.message.contains("click a wall"), "a level is not a wall: {}", app.message);
        app.wall_footing_click(wall);
        let id = app.document.elements().find(|e| e.category == Category::WallFooting).expect("placed").id;
        let volume = |app: &Desktop| -> f64 {
            let batch = crate::view3d::batch(&app.document, None);
            let ei = batch.elements.iter().position(|e| e.id == id.0).unwrap() as u32;
            batch.tris.iter().zip(&batch.tri_element).filter(|(_, e)| **e == ei).map(|(t, _)| {
                let p = t.map(|i| batch.verts[i as usize].map(f64::from));
                (p[0][0] * (p[1][1] * p[2][2] - p[1][2] * p[2][1]) - p[0][1] * (p[1][0] * p[2][2] - p[1][2] * p[2][0]) + p[0][2] * (p[1][0] * p[2][1] - p[1][1] * p[2][0])) / 6.0
            }).sum()
        };
        assert!((volume(&app) - run * 0.6 * 0.4).abs() < 1e-2 * run, "{} vs {}", volume(&app), run * 0.24);
        // it follows the wall: the wall moves, the footing's mesh moves with it
        let before = app.document.mesh(id).unwrap().vertices[0];
        app.document.transact(app.document.revision(), &[Command::SetGeometry { id: wall, geometry: Geometry::Linear { start: [start[0] + 1.0, start[1]], end: [end[0] + 1.0, end[1]], level: ElementId(10) } }]).unwrap();
        assert!((app.document.mesh(id).unwrap().vertices[0][0] - before[0] - 1.0).abs() < 1e-9);
        // the wall cannot be deleted from under it
        assert!(app.document.transact(app.document.revision(), &[Command::Delete { id: wall }]).is_err());
        // edit the depth and the material, export as STRIP_FOOTING
        app.select_element(Some(id));
        assert!(!app.has_pending_edit());
        app.height_mm = 600.0;
        app.apply_selected_edit();
        assert!((volume(&app) - run * 0.6 * 0.6).abs() < 1e-2 * run);
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-17T00:00:00").unwrap();
        assert_eq!(out.text.matches("IFCFOOTING(").count(), 1);
        assert!(out.text.contains(".STRIP_FOOTING.") && out.text.contains("IFCMATERIAL('Concrete')"));
    }

    #[test]
    fn beams_are_cut_to_their_columns_in_3d_and_ifc_and_the_join_is_a_property() {
        let mut app = Desktop::sample();
        let col = |app: &mut Desktop, x: f64| { app.width_mm = 400.0; app.height_mm = 400.0; app.column_click(ElementId(10), [x, 3.0]); };
        app.document.place_beam(ElementId(10), &openbim_document::BeamSpec::level([1.0, 3.0], [5.0, 3.0], 0.2, 0.4, 2.8)).unwrap();
        let beam = app.document.elements().find(|e| e.category == Category::Beam).unwrap().id;
        col(&mut app, 1.0);
        col(&mut app, 5.0);
        let item = crate::export::beam_item(&app.document, app.document.get(beam).unwrap()).unwrap();
        assert!((item.a[0] - 1.2).abs() < 1e-9 && (item.b[0] - 4.8).abs() < 1e-9);
        // the 3D mesh is the cut beam: 3.6 m of 0.2 x 0.4
        let batch = crate::view3d::batch(&app.document, None);
        let ei = batch.elements.iter().position(|e| e.id == beam.0).unwrap() as u32;
        let vol: f64 = batch.tris.iter().zip(&batch.tri_element).filter(|(_, e)| **e == ei).map(|(t, _)| {
            let p = t.map(|i| batch.verts[i as usize].map(f64::from));
            (p[0][0] * (p[1][1] * p[2][2] - p[1][2] * p[2][1]) - p[0][1] * (p[1][0] * p[2][2] - p[1][2] * p[2][0]) + p[0][2] * (p[1][0] * p[2][1] - p[1][1] * p[2][0])) / 6.0
        }).sum();
        assert!((vol - 0.2 * 0.4 * 3.6).abs() < 1e-3, "{vol}");
        // disallow the join at the start in the properties
        app.select_element(Some(beam));
        assert!(!app.has_pending_edit());
        app.join_pick[0] = false;
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        let item = crate::export::beam_item(&app.document, app.document.get(beam).unwrap()).unwrap();
        assert!((item.a[0] - 1.0).abs() < 1e-9 && (item.b[0] - 4.8).abs() < 1e-9);
        assert!(!app.document.get(beam).unwrap().join_ends[0]);
        app.undo_document();
        assert!(app.document.get(beam).unwrap().join_ends[0]);
    }

    #[test]
    fn a_beam_system_is_drawn_as_an_area_edited_by_spacing_and_exported_as_beams() {
        let mut app = Desktop::sample();
        app.run_command("beam_system", "Beam System");
        assert!(app.tool == Tool::BeamSystem && app.usage_pick == Usage::Joist);
        (app.spacing_mm, app.angle_deg, app.width_mm, app.height_mm, app.elevation_mm) = (1000.0, 0.0, 100.0, 200.0, 2800.0);
        app.material_pick = openbim_document::Material::Timber;
        for p in [[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [0.0, 4.0]] { app.slab_click(ElementId(10), p, false); }
        app.slab_click(ElementId(10), [0.0, 0.0], true);
        let sys = app.document.elements().find(|e| e.category == Category::BeamSystem).expect("placed").id;
        assert_eq!(app.document.system_beams(sys).len(), 4);
        // every beam is a framing member in 3D and in the IFC file; the system itself is neither
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-18T00:00:00").unwrap();
        assert_eq!(out.text.matches("IFCBEAM(").count(), 4);
        // changing the spacing makes the beams again
        app.select_element(Some(sys));
        assert!(!app.has_pending_edit());
        app.spacing_mm = 2000.0;
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        assert_eq!(app.document.system_beams(sys).len(), 2);
        app.undo_document();
        assert_eq!(app.document.system_beams(sys).len(), 4);
    }

    #[test]
    fn a_truss_is_built_by_two_clicks_edited_by_panel_length_and_exported_as_members() {
        let mut app = Desktop::sample();
        app.run_command("truss", "Truss");
        assert!(app.tool == Tool::Truss);
        (app.spacing_mm, app.width_mm, app.height_mm, app.elevation_mm) = (2000.0, 100.0, 1000.0, 2000.0);
        app.material_pick = openbim_document::Material::Steel;
        app.truss_click(ElementId(10), [0.0, 1.0]);
        app.truss_click(ElementId(10), [8.0, 1.0]);
        let t = app.document.elements().find(|e| e.category == Category::Truss).expect("built").id;
        assert_eq!(app.document.system_beams(t).len(), 10);
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-18T00:00:00").unwrap();
        assert_eq!(out.text.matches("IFCBEAM(").count() + out.text.matches("IFCMEMBER(").count(), 10);
        app.select_element(Some(t));
        assert!(!app.has_pending_edit());
        app.spacing_mm = 4000.0;
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        assert_eq!(app.document.system_beams(t).len(), 6);
        app.undo_document();
        assert_eq!(app.document.system_beams(t).len(), 10);
    }

    #[test]
    fn warnings_are_counted_listed_and_select_the_element_they_are_about() {
        let mut app = Desktop::sample();
        assert!(app.warnings().is_empty());
        let beam = app.document.place_beam(ElementId(10), &BeamSpec::level([1.0, 3.0], [7.0, 3.0], 0.3, 0.5, 2.8)).unwrap();
        app.run_command("rebar", "Rebar");
        app.rebar_click(beam);
        assert!(app.document.elements().any(|e| e.category == Category::Rebar), "{}", app.message);
        assert!(app.warnings().is_empty());
        // the beam is made too narrow for its bars: the rebar is not drawn and the list says why
        app.select_element(Some(beam));
        app.width_mm = 60.0;
        app.apply_selected_edit();
        let message = app.message.clone();
        assert_eq!(app.warnings().len(), 1, "{message}");
        assert!(app.warnings()[0].message.contains("do not fit"));
        app.show_warnings = true;
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 550.0))), ..Default::default() }, |ui| app.warnings_window(ui.ctx()));
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
        app.select_element(None);
        let first = app.warnings()[0].elements[0];
        app.select_element(Some(first));
        assert_eq!(app.selected_id(), Some(first));
        app.undo_document();
        assert!(app.warnings().is_empty(), "the list follows the document");
    }

    #[test]
    fn a_type_is_made_from_a_beam_given_to_another_and_an_edit_in_properties_changes_both() {
        let mut app = Desktop::sample();
        let a = app.document.place_beam(ElementId(10), &BeamSpec::level([1.0, 1.0], [7.0, 1.0], 0.3, 0.5, 2.8)).unwrap();
        let b = app.document.place_beam(ElementId(10), &BeamSpec::level([1.0, 2.0], [7.0, 2.0], 0.2, 0.4, 2.8)).unwrap();
        app.select_element(Some(a));
        app.make_type();
        assert!(app.document.get(a).unwrap().type_id.is_none(), "a type needs a name");
        app.type_name_input = "Beam 300 x 500".into();
        app.make_type();
        let t = app.document.get(a).unwrap().type_id.expect("typed");
        assert!(app.type_name_input.is_empty());
        app.select_element(Some(b));
        app.choose_type(Some(t));
        assert_eq!(app.document.get(b).unwrap().width_m, 0.3);
        assert_eq!(app.width_mm, 300.0, "the controls follow the new values");
        // an edit of the size of one is an edit of the type, and so of the other
        app.width_mm = 350.0;
        app.apply_selected_edit();
        assert!(app.message.contains("2 elements"), "{}", app.message);
        for id in [a, b, t] { assert_eq!(app.document.get(id).unwrap().width_m, 0.35); }
        // the panel draws a typed element
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 550.0))), ..Default::default() }, |ui| crate::panels::properties(&mut app, ui));
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
        // one undo takes the whole edit back
        app.undo_document();
        for id in [a, b, t] { assert_eq!(app.document.get(id).unwrap().width_m, 0.3); }
        // letting go of the type keeps the values and frees the beam to be edited alone
        app.choose_type(None);
        app.width_mm = 250.0;
        app.apply_selected_edit();
        assert_eq!((app.document.get(b).unwrap().width_m, app.document.get(a).unwrap().width_m), (0.25, 0.3));
    }

    #[test]
    fn rebar_goes_into_a_clicked_beam_is_edited_in_place_and_exported_as_bars() {
        let mut app = Desktop::sample();
        let beam = app.document.place_beam(ElementId(10), &BeamSpec::level([1.0, 3.0], [7.0, 3.0], 0.3, 0.5, 2.8)).unwrap();
        app.run_command("rebar", "Rebar");
        assert!(app.tool == Tool::Rebar);
        app.rebar_click(ElementId(10));
        assert!(app.message.contains("click a beam"), "{}", app.message);
        app.rebar_click(beam);
        let r = app.document.elements().find(|e| e.category == Category::Rebar).expect("placed").id;
        let bars = app.document.rebar_bars(app.document.get(r).unwrap()).unwrap().len();
        assert_eq!(bars, 35);
        let batch = crate::view3d::batch(&app.document, None);
        assert!(batch.elements.iter().any(|e| e.id == r.0), "in 3D");
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-18T00:00:00").unwrap();
        assert_eq!(out.text.matches("IFCREINFORCINGBAR(").count(), bars);
        assert!(out.text.contains("IFCSWEPTDISKSOLID(") && out.text.contains(".SHEAR.") && out.text.contains(".MAIN."));
        assert!(out.warnings.iter().all(|w| !w.contains("without geometry")), "{:?}", out.warnings);
        // bars and stirrups edited in the properties
        app.select_element(Some(r));
        assert!(!app.has_pending_edit());
        (app.bar_count, app.spacing_mm) = (4.0, 400.0);
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        assert_eq!(app.document.get(r).unwrap().count, 4);
        assert!(app.document.rebar_bars(app.document.get(r).unwrap()).unwrap().len() < bars);
        app.undo_document();
        assert_eq!(app.document.get(r).unwrap().count, 3);
    }

    #[test]
    fn a_door_goes_into_the_clicked_wall_cuts_it_in_3d_and_is_exported_with_its_opening() {
        let mut app = Desktop::sample();
        let wall = app.document.elements().find(|e| e.category == Category::Wall).expect("sample wall").id;
        let Some(Geometry::Linear { start, end, .. }) = app.document.get(wall).unwrap().geometry else { panic!() };
        let volume = |app: &Desktop, id: ElementId| -> f64 {
            let batch = crate::view3d::batch(&app.document, None);
            let Some(ei) = batch.elements.iter().position(|e| e.id == id.0) else { return 0.0 };
            batch.tris.iter().zip(&batch.tri_element).filter(|(_, e)| **e == ei as u32).map(|(t, _)| {
                let p = t.map(|i| batch.verts[i as usize].map(f64::from));
                (p[0][0] * (p[1][1] * p[2][2] - p[1][2] * p[2][1]) - p[0][1] * (p[1][0] * p[2][2] - p[1][2] * p[2][0]) + p[0][2] * (p[1][0] * p[2][1] - p[1][1] * p[2][0])) / 6.0
            }).sum()
        };
        let before = volume(&app, wall);
        app.run_command("door", "Door");
        assert!(app.tool == Tool::Family && app.family_pick.starts_with("Door"));
        let mid = [(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0];
        app.family_click(ElementId(10), mid, None);
        assert!(app.message.contains("click the wall"), "{}", app.message);
        app.family_click(ElementId(10), mid, Some(wall));
        let door = app.document.elements().find(|e| e.category == Category::Family).expect("placed").id;
        // the wall now has the opening of the door cut out of it
        let after = volume(&app, wall);
        assert!(after < before - 0.05, "{before} -> {after}");
        assert!(volume(&app, door) > 0.0, "the door is in 3D");
        // exported as a door that fills an opening in the wall
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-18T00:00:00").unwrap();
        assert_eq!(out.text.matches("IFCDOOR(").count(), 1);
        assert_eq!(out.text.matches("IFCOPENINGELEMENT(").count() - crate::export::service_openings(&out.text), 1);
        // it follows the wall: the wall moves, the door goes with it
        app.document.transact(app.document.revision(), &[Command::SetGeometry { id: wall, geometry: Geometry::Linear { start: [start[0], start[1] + 0.5], end: [end[0], end[1] + 0.5], level: ElementId(10) } }]).unwrap();
        assert!(volume(&app, wall) < before - 0.05);
        // a free-standing component and its edit
        app.run_command("ahu", "AHU");
        app.slab_offset_mm = 2400.0;
        app.family_click(ElementId(10), [3.0, 3.0], None);
        let fcu = app.document.elements().filter(|e| e.category == Category::Family).map(|e| e.id).max().unwrap();
        assert!(volume(&app, fcu) > 0.0);
        app.select_element(Some(fcu));
        assert!(!app.has_pending_edit());
        app.angle_deg = 90.0;
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        assert_eq!(app.document.get(fcu).unwrap().angle_deg, 90.0);
    }

    #[test]
    fn instance_values_of_a_door_are_edited_in_the_properties_and_change_the_door() {
        let mut app = Desktop::sample();
        let wall = app.document.elements().find(|e| e.category == Category::Wall).expect("sample wall").id;
        let Some(Geometry::Linear { start, end, .. }) = app.document.get(wall).unwrap().geometry else { panic!() };
        app.run_command("door", "Door");
        app.family_click(ElementId(10), [(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0], Some(wall));
        let door = app.document.elements().find(|e| e.category == Category::Family).unwrap().id;
        app.select_element(Some(door));
        assert!(app.family_values.iter().any(|(n, _)| n == "open_angle"), "{:?}", app.family_values);
        assert!(!app.family_values.iter().any(|(n, _)| n == "wall_t" || n == "width"), "type values and the wall's are not instance values");
        assert!(!app.has_pending_edit());
        app.family_values.iter_mut().find(|(n, _)| n == "open_angle").unwrap().1 = 60.0;
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        assert_eq!(app.document.get(door).unwrap().overrides, vec![("open_angle".to_string(), 60.0)]);
        assert!(!app.has_pending_edit());
        // a value out of range is refused and nothing changes
        app.family_values.iter_mut().find(|(n, _)| n == "open_angle").unwrap().1 = 400.0;
        let rev = app.document.revision();
        app.apply_selected_edit();
        assert!(app.message.contains("do not fit") && app.document.revision() == rev, "{}", app.message);
        app.undo_document();
        assert!(app.document.get(door).unwrap().overrides.is_empty());
    }

    #[test]
    fn the_swing_of_a_door_is_a_choice_in_the_properties_that_is_checked_and_stored() {
        let mut app = Desktop::sample();
        let wall = app.document.elements().find(|e| e.category == Category::Wall).expect("sample wall").id;
        let Some(Geometry::Linear { start, end, .. }) = app.document.get(wall).unwrap().geometry else { panic!() };
        app.run_command("door", "Door");
        app.family_click(ElementId(10), [(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0], Some(wall));
        let door = app.document.elements().find(|e| e.category == Category::Family).unwrap().id;
        app.select_element(Some(door));
        let swing = app.family_texts.iter().find(|t| t.0 == "swing").expect("a choice").clone();
        assert_eq!((swing.1.as_str(), swing.2.len()), ("right", 2));
        assert!(!app.has_pending_edit());
        app.family_texts.iter_mut().find(|t| t.0 == "swing").unwrap().1 = "left".into();
        assert!(app.has_pending_edit());
        app.apply_selected_edit();
        assert_eq!(app.document.get(door).unwrap().texts, vec![("swing".to_string(), "left".to_string())]);
        assert_eq!(app.family_texts.iter().find(|t| t.0 == "swing").unwrap().1, "left");
        assert!(!app.has_pending_edit());
        // an option the family does not have is refused
        app.family_texts.iter_mut().find(|t| t.0 == "swing").unwrap().1 = "up".into();
        let rev = app.document.revision();
        app.apply_selected_edit();
        assert!(app.message.contains("do not fit") && app.document.revision() == rev, "{}", app.message);
        // the file still exports
        assert!(crate::export::export_ifc(&app.document, "P", 1, "2026-10-18T00:00:00").unwrap().text.contains("IFCDOOR("));
    }

    #[test]
    fn the_export_window_offers_ifc_4_3_first_and_writes_the_schema_picked() {
        let mut app = Desktop::sample();
        app.run_command("ifc_export", "Export IFC");
        let d = app.export_dialog.as_ref().expect("the window opens");
        assert_eq!(d.schema, crate::export::Schema::Ifc4x3);
        let dir = std::env::temp_dir().join(format!("obim-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (schema, needle) in [(crate::export::Schema::Ifc4x3, "IFC4X3_ADD2"), (crate::export::Schema::Ifc4, "FILE_SCHEMA(('IFC4'))"), (crate::export::Schema::Ifc2x3, "IFC2X3")] {
            let path = dir.join("m.ifc");
            app.write_ifc(path.to_str().unwrap(), schema, true);
            assert!(std::fs::read_to_string(&path).unwrap().contains(needle), "{}", app.message);
            assert!(app.message.contains(schema.name()));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_duct_is_drawn_by_clicks_with_its_elbow_placed_for_it_and_undone_in_one_step() {
        let mut app = Desktop::sample();
        let before = app.document.elements().count();
        app.run_command("duct", "Duct");
        assert!(app.tool == Tool::Duct && (app.width_mm, app.height_mm) == (400.0, 300.0));
        let level = app.plan_level();
        for p in [[1.0, 6.5], [5.0, 6.5], [5.0, 7.5]] { app.run_click(p); }
        assert_eq!((app.run_points.len(), app.run_sizes.len()), (3, 2));
        app.finish_run(level);
        assert!(app.message.contains("2 ducts and 1 fitting"), "{}", app.message);
        assert!(app.run_points.is_empty());
        assert_eq!((app.document.elements().count(), app.document.mep().fittings.len(), app.document.mep().links.len()), (before + 2, 1, 2));
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-18T00:00:00").unwrap();
        assert_eq!((out.text.matches("IFCDUCTFITTING(").count(), out.text.matches("IFCRELCONNECTSPORTS(").count()), (1, 2));
        app.undo_document();
        assert_eq!((app.document.elements().count(), app.document.mep().is_empty()), (before, true));
    }

    #[test]
    fn a_change_of_size_between_two_clicks_gets_a_reducer_and_a_bend_with_a_size_change_is_refused_with_the_points_kept() {
        let mut app = Desktop::sample();
        app.run_command("duct", "Duct");
        let level = app.plan_level();
        app.run_click([1.0, 6.5]);
        app.run_click([4.0, 6.5]);
        app.width_mm = 250.0;
        app.run_click([7.0, 6.5]);
        app.finish_run(level);
        let m = app.document.mep();
        assert!(m.fittings.len() == 1 && matches!(m.fittings[0].fitting.kind, openbim_route::FittingKind::Transition), "{}", app.message);
        assert_eq!(m.links.len(), 2);
        let widths: Vec<f64> = app.document.elements().filter(|e| e.name.contains("Duct") && e.id.0 > 100).map(|e| e.width_m).collect();
        assert_eq!(widths, vec![0.4, 0.25]);

        // a bend where the size changes cannot be drawn: the points stay so that one can be taken back
        let n = app.document.elements().count();
        app.run_click([1.0, 3.5]);
        app.run_click([3.0, 3.5]);
        app.width_mm = 300.0;
        app.run_click([3.0, 5.0]);
        app.finish_run(level);
        assert!(app.message.contains("cannot be drawn"), "{}", app.message);
        assert_eq!((app.run_points.len(), app.document.elements().count()), (3, n));
        app.run_back();
        assert_eq!((app.run_points.len(), app.run_sizes.len()), (2, 1));
    }

    #[test]
    fn a_pipe_is_drawn_round_in_its_system_and_the_system_list_fits_the_tool() {
        let mut app = Desktop::sample();
        app.run_command("pipe", "Pipe");
        assert!(app.tool == Tool::Pipe && app.system_pick == openbim_family::mep::System::ChilledWaterSupply);
        assert!(!app.run_systems().contains(&openbim_family::mep::System::SupplyAir));
        let level = app.plan_level();
        for p in [[1.0, 6.5], [4.0, 6.5], [4.0, 7.5]] { app.run_click(p); }
        app.finish_run(level);
        assert!(app.message.contains("2 pipes and 1 fitting"), "{}", app.message);
        let f = &app.document.mep().fittings[0];
        assert_eq!(f.system, openbim_family::mep::System::ChilledWaterSupply);
        assert_eq!(f.fitting.port_sizes[0], openbim_place::penetrations::RunSize::Round { diameter: 0.1 });
        app.run_command("duct", "Duct");
        assert!(app.run_systems().contains(&openbim_family::mep::System::SupplyAir) && app.system_pick.domain_ok(openbim_family::mep::Domain::Duct), "a duct gets an air system");
    }

    #[test]
    fn the_types_window_opens_from_the_ribbon_and_draws_with_a_type_picked_and_drops_one_that_was_deleted() {
        let mut app = Desktop::sample();
        let beam = app.document.place_beam(ElementId(10), &BeamSpec::level([1.0, 3.0], [7.0, 3.0], 0.3, 0.5, 2.8)).unwrap();
        let t = app.document.create_type(beam, "B 300 x 500").unwrap();
        app.run_command("project_parameters", "Types");
        assert!(app.types_dialog.is_some());
        app.types_dialog.as_mut().unwrap().picked = Some(t);
        let ctx = eframe::egui::Context::default();
        for _ in 0..2 { ctx.run_ui(Default::default(), |ui| app.types_window(ui.ctx())).textures_delta.clear(); }
        assert_eq!(app.types_dialog.as_ref().unwrap().picked, Some(t));
        app.document.detach_type(&[beam]).unwrap();
        app.document.delete_type(t).unwrap();
        ctx.run_ui(Default::default(), |ui| app.types_window(ui.ctx())).textures_delta.clear();
        assert_eq!(app.types_dialog.as_ref().unwrap().picked, None, "a type that is gone is let go");
    }

    #[test]
    fn the_step_params_window_shows_the_parameter_table() {
        use openbim_document::Mesh;
        let mut app = Desktop::sample();
        let mesh = Mesh { vertices: vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 3.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 4.0], [2.0, 0.0, 4.0], [2.0, 3.0, 4.0], [0.0, 3.0, 4.0]], faces: vec![vec![0, 3, 2], vec![0, 2, 1], vec![4, 5, 6], vec![4, 6, 7], vec![0, 1, 5], vec![0, 5, 4], vec![1, 2, 6], vec![1, 6, 5], vec![2, 3, 7], vec![2, 7, 6], vec![3, 0, 4], vec![3, 4, 7]] };
        app.step_params_dialog = Some(crate::step_params_ui::StepParamsDialog { name: "part.stp".into(), params: crate::step_import::mesh_params(&mesh) });
        let ctx = eframe::egui::Context::default();
        ctx.run_ui(Default::default(), |ui| app.step_params_window(ui.ctx())).textures_delta.clear();
        // the dialog stays open (name not cleared) and the params are the box's
        assert!(app.step_params_dialog.is_some(), "dialog stays open");
        let p = &app.step_params_dialog.as_ref().unwrap().params;
        assert!((p.width - 2.0).abs() < 1e-9 && (p.volume - 24.0).abs() < 1e-6, "{p:?}");
    }

    #[test]
    fn the_import_window_uses_a_class_map_and_refuses_one_that_does_not_read() {
        let mut src = Desktop::sample();
        src.document.place_generic(ElementId(10), "Pump 1", vec![[2.0, 1.0], [3.2, 1.0], [3.2, 1.8], [2.0, 1.8]], 0.1, 0.9, vec![("IfcClass".into(), "IFCPUMP".into())]).unwrap();
        let text = crate::export::export_ifc_with(&src.document, "P", 1, "2026-10-24T00:00:00", crate::export::Schema::Ifc4x3, false).unwrap().text.replace("IFCBUILDINGELEMENTPROXY(", "IFCPUMP(");
        let dir = std::env::temp_dir().join(format!("obim-class-map-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (ifc, good, bad) = (dir.join("m.ifc"), dir.join("good.txt"), dir.join("bad.txt"));
        std::fs::write(&ifc, text).unwrap();
        std::fs::write(&good, "IFCPUMP\tIgnore\n").unwrap();
        std::fs::write(&bad, "IFCPUMP\tTeapot\n").unwrap();
        let (ifc, good, bad) = (ifc.to_str().unwrap(), good.to_str().unwrap(), bad.to_str().unwrap());
        let generics = |a: &Desktop| a.document.elements().filter(|e| e.category == openbim_document::Category::Generic).count();
        // a map that does not read: nothing is read, and the log says why
        let mut app = Desktop::sample();
        app.document = openbim_document::Document::new();
        app.read_ifc_with(ifc, bad);
        assert!(app.message.contains("refused") && app.message.contains("line 1"), "{}", app.message);
        assert_eq!(app.document.elements().count(), 0);
        assert!(std::fs::read_to_string(format!("{ifc}.log")).unwrap().contains("Import refused"));
        // with the map: the pump is left out and the log says the map did it
        app.read_ifc_with(ifc, good);
        assert_eq!(generics(&app), 0, "{}", app.message);
        let log = std::fs::read_to_string(format!("{ifc}.log")).unwrap();
        assert!(log.contains("left out by the class map") && log.contains("class map"), "{log}");
        // without it: kept as a generic model
        let mut plain = Desktop::sample();
        plain.document = openbim_document::Document::new();
        plain.read_ifc_with(ifc, "");
        assert_eq!(generics(&plain), 1, "{}", plain.message);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_import_window_reads_a_file_the_app_wrote_back_into_an_empty_model() {
        let mut app = Desktop::sample();
        let dir = std::env::temp_dir().join(format!("obim-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("m.ifc");
        app.write_ifc(path.to_str().unwrap(), crate::export::Schema::Ifc4x3, true);
        let want = app.document.elements().count();
        app.document = openbim_document::Document::new();
        app.run_command("ifc_import", "Import IFC");
        assert!(app.import_dialog.is_some(), "the window opens");
        assert!(crate::import::describe_file(path.to_str().unwrap()).contains("IFC4X3_ADD2"));
        app.read_ifc(path.to_str().unwrap());
        assert_eq!(app.document.elements().count(), want, "{}", app.message);
        assert!(app.message.contains("Restored") && app.message.contains("m.ifc.log"), "{}", app.message);
        // the log is next to the file: what came in, what was left out, and the reader's word on single entities
        let log = std::fs::read_to_string(dir.join("m.ifc.log")).unwrap();
        assert!(log.starts_with("Import log for ") && log.contains("\nAdded\n") && log.contains("\nLeft out\n"), "{log}");
        // a file without the app's own data leaves things out, and the log names each class with its count
        let foreign = dir.join("f.ifc");
        app.write_ifc(foreign.to_str().unwrap(), crate::export::Schema::Ifc4x3, false);
        app.document = openbim_document::Document::new();
        app.read_ifc(foreign.to_str().unwrap());
        let log = std::fs::read_to_string(dir.join("f.ifc.log")).unwrap();
        assert!(log.contains("not an extruded profile this app reads") && log.contains("\nReader\n") && log.contains("unsupported product"), "{log}");
        app.document = openbim_document::Document::new();
        app.read_ifc(path.to_str().unwrap());
        app.read_ifc(dir.join("missing.ifc").to_str().unwrap());
        assert!(app.message.contains("Could not read"), "{}", app.message);
        std::fs::write(dir.join("bad.ifc"), "not an ifc").unwrap();
        app.read_ifc(dir.join("bad.ifc").to_str().unwrap());
        assert!(app.message.contains("Import failed"), "{}", app.message);
        assert!(std::fs::read_to_string(dir.join("bad.ifc.log")).unwrap().contains("Import failed, nothing changed"));
        assert_eq!(app.document.elements().count(), want);
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn the_load_family_window_takes_a_catalogue_into_the_document_and_refuses_a_bad_one_whole() {
        let mut app = Desktop::sample();
        app.run_command("load_family", "Load Family");
        assert!(app.load_family_dialog.is_some(), "the window opens");
        let before = app.document.types_for(Category::Family).len();
        // a catalogue of two families: every row becomes a type of the family its table names
        let text = "family\tCeiling diffuser, square\ntype\tneck_d\tair_flow\nD 250\t0.25\t0.06\nD 300\t0.3\t0.08\nfamily\tAir handling unit\ntype\tair_flow\tpower\nAHU 1.2\t1.2\t4000\n";
        let message = app.load_family_catalogue_text(text, "range.tsv");
        assert!(message.contains("3 types") && message.contains("Ceiling diffuser, square") && message.contains("Air handling unit"), "{message}");
        assert!(message.contains("undo"), "the message says how to take it back: {message}");
        assert_eq!(app.document.types_for(Category::Family).len(), before + 3);
        let diffuser = app.document.types_for(Category::Family).iter().find(|t| t.name == "D 250").map(|t| t.id).unwrap();
        assert_eq!(app.document.family_type_values(diffuser), vec![("neck_d".to_string(), 0.25), ("air_flow".to_string(), 0.06)]);
        // the whole load is one undo step
        assert!(app.document.undo().unwrap());
        assert_eq!(app.document.types_for(Category::Family).len(), before);
        // a catalogue the library cannot satisfy is refused whole, and the message says why
        let message = app.load_family_catalogue_text("family\tNonesuch\ntype\tsize\nbig\t1\n", "bad.tsv");
        assert!(message.contains("refused") && message.contains("Nonesuch"), "{message}");
        assert_eq!(app.document.types_for(Category::Family).len(), before);
        let message = app.load_family_catalogue_text("family\tCeiling diffuser, square\ntype\tnot_an_input\nx\t1\n", "bad.tsv");
        assert!(message.contains("refused") && message.contains("not_an_input"), "{message}");
        assert_eq!(app.document.types_for(Category::Family).len(), before);
        // a file that is not there, and a file that is not a catalogue
        app.read_family_catalogue(std::env::temp_dir().join("no-such-catalogue.tsv").to_str().unwrap());
        assert!(app.message.contains("Could not read"), "{}", app.message);
        let message = app.load_family_catalogue_text("this is not a catalogue\n", "junk.tsv");
        assert!(message.contains("refused"), "{message}");
    }

    #[test]
    fn a_type_loaded_from_a_catalogue_sets_the_ports_and_the_geometry_its_instances_are_solved_with() {
        let mut app = Desktop::sample();
        let message = app.load_family_catalogue_text("family\tCeiling diffuser, square\ntype\tneck_d\tair_flow\nD 400\t0.4\t0.2\n", "range.tsv");
        assert!(message.contains("1 type"), "{message}");
        let t = app.document.types_for(Category::Family).iter().find(|t| t.name == "D 400").map(|t| t.id).unwrap();
        let spec = FamilySpec { family: "Ceiling diffuser, square".into(), type_name: String::new(), overrides: Vec::new(), angle_deg: 0.0, elevation_m: 2.7 };
        let unit = app.document.place_family(ElementId(10), [2.0, 2.0], &spec).unwrap();
        app.document.assign_type(&[unit], t).unwrap();
        // the connectors the export writes as ports, and the schematic reads, carry the catalogue's neck and flow,
        // not the library's defaults (0.2 and 0.05)
        let conns = crate::elec_ui::instance_connectors(&app.document, unit);
        assert_eq!(conns.len(), 1, "a diffuser has one duct connection");
        assert_eq!(conns[0].size, openbim_family::mep::Size::Round { diameter: 0.4 }, "the type's neck reached the port");
        assert!((conns[0].flow - 0.2).abs() < 1e-9, "and so did its air flow");
        // an instance's own value still wins over its type's, and leaves the type's alone
        app.document.set_family_values(unit, &[("face".to_string(), 0.9)]).unwrap();
        assert_eq!(crate::elec_ui::instance_connectors(&app.document, unit)[0].size, openbim_family::mep::Size::Round { diameter: 0.4 });
        assert_eq!(app.document.family_type_values(t), vec![("neck_d".to_string(), 0.4), ("air_flow".to_string(), 0.2)]);
        // and the exported file has the terminal and its port
        let out = crate::export::export_ifc(&app.document, "Project1", 1, "2026-10-21T00:00:00").unwrap();
        assert!(out.text.contains("IFCAIRTERMINAL("), "the diffuser exports as an air terminal");
        assert!(out.text.contains("IFCDISTRIBUTIONPORT("), "with its port");
    }

    #[test]
    fn an_hvac_system_is_built_from_scratch_with_the_mouse_saved_and_exported() {
        let mut app = Desktop::sample();
        // the AHU and the diffuser, from a catalogue
        let m1 = app.load_family_catalogue_text("family\tAir handling unit\ntype\tair_flow\nAHU-1\t0.3\n", "ahus.tsv");
        assert!(m1.contains("1 type"), "{m1}");
        let ahut = app.document.types_for(Category::Family).iter().find(|t| t.name == "AHU-1").map(|t| t.id).unwrap();
        let m2 = app.load_family_catalogue_text("family\tCeiling diffuser, square\ntype\tneck_d\tair_flow\nD 400\t0.4\t0.2\n", "diffusers.tsv");
        assert!(m2.contains("1 type"), "{m2}");
        let dt = app.document.types_for(Category::Family).iter().find(|t| t.name == "D 400").map(|t| t.id).unwrap();
        // place the AHU and a diffuser, and give them the loaded types
        let spec_ahu = FamilySpec { family: "Air handling unit".into(), type_name: String::new(), overrides: Vec::new(), angle_deg: 0.0, elevation_m: 2.7 };
        let ahu = app.document.place_family(ElementId(10), [1.0, 1.0], &spec_ahu).unwrap();
        app.document.assign_type(&[ahu], ahut).unwrap();
        let spec_diff = FamilySpec { family: "Ceiling diffuser, square".into(), type_name: String::new(), overrides: Vec::new(), angle_deg: 0.0, elevation_m: 2.7 };
        let diff = app.document.place_family(ElementId(10), [5.0, 1.0], &spec_diff).unwrap();
        app.document.assign_type(&[diff], dt).unwrap();
        // a duct between them, drawn by clicks
        app.run_command("duct", "Duct");
        let level = app.plan_level();
        for p in [[1.0, 1.0], [5.0, 1.0], [5.0, 1.0]] { app.run_click(p); }
        app.finish_run(level);
        // edit the AHU's airflow through the type editor
        app.select_element(Some(ahu));
        app.family_type_values.iter_mut().find(|(n, _)| n == "air_flow").unwrap().1 = 0.5;
        app.apply_family_type();
        // save, reopen and export
        let path = std::env::temp_dir().join(format!("openbim-hvac-e2e-{}.obim", std::process::id()));
        let path = path.to_str().unwrap().to_string();
        app.write_model(&path);
        let saved = app.document.clone();
        let mut reopened = Desktop::sample();
        reopened.open_path(&path);
        assert_eq!(reopened.document.elements().count(), saved.elements().count(), "{}", reopened.message);
        // the AHU's airflow changed, and it came back from the save with its type
        let ahu2 = reopened.document.elements().find(|e| e.category == Category::Family && e.family == "Air handling unit").expect("the AHU came back");
        let ahut2 = ahu2.type_id.expect("the AHU came back with its type");
        let vals = reopened.document.family_type_values(ahut2);
        assert_eq!(vals.iter().find(|(n, _)| n == "air_flow").map(|(_, v)| *v), Some(0.5), "the edited airflow persisted: {:?}", vals);
        let out = crate::export::export_ifc_with(&reopened.document, "Office", 1, "2026-10-21T00:00:00", crate::export::Schema::Ifc4, false).unwrap();
        assert!(out.text.contains("IFCAIRTERMINAL("), "the AHU is an air terminal");
        assert!(out.text.contains("IFCDUCTSEGMENT("), "the duct is there");
        assert!(out.text.contains("IFCDISTRIBUTIONPORT("), "and its ports");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_wall_opening_and_a_shaft_cut_the_3d_model_and_are_exported_as_openings_of_what_they_cut() {
        let mut app = Desktop::sample();
        let wall = app.document.elements().find(|e| e.category == Category::Wall).expect("sample wall").id;
        let level = ElementId(10);
        let upper = app.document.level_above(level).unwrap();
        let floor = app.document.place_slab(upper, &SlabSpec { outline: vec![[-1.0, -1.0], [9.0, -1.0], [9.0, 7.0], [-1.0, 7.0]], thickness_m: 0.2, top_offset_m: 0.0, kind: openbim_document::SlabKind::Floor, material: openbim_document::Material::Concrete }).unwrap();
        let volume = |app: &Desktop, id: ElementId| -> f64 {
            let batch = crate::view3d::batch(&app.document, None);
            let Some(ei) = batch.elements.iter().position(|e| e.id == id.0) else { return 0.0 };
            batch.tris.iter().zip(&batch.tri_element).filter(|(_, e)| **e == ei as u32).map(|(t, _)| {
                let p = t.map(|i| batch.verts[i as usize].map(f64::from));
                (p[0][0] * (p[1][1] * p[2][2] - p[1][2] * p[2][1]) - p[0][1] * (p[1][0] * p[2][2] - p[1][2] * p[2][0]) + p[0][2] * (p[1][0] * p[2][1] - p[1][1] * p[2][0])) / 6.0
            }).sum()
        };
        let (wall_before, floor_before) = (volume(&app, wall), volume(&app, floor));
        assert!((floor_before - 80.0 * 0.2).abs() < 1e-3, "{floor_before}");
        // a wall opening 1.2 m wide, 1.2 m high, takes 1.2 x 1.2 x thickness out of the wall
        let t = app.document.get(wall).unwrap().width_m;
        let op = app.document.place_wall_opening(wall, 1.0, 2.2, 0.9, 2.1).unwrap();
        let wall_after = volume(&app, wall);
        assert!((wall_before - wall_after - 1.2 * 1.2 * t).abs() < 1e-3, "{wall_before} -> {wall_after}");
        // a shaft 1 m x 1 m takes 1 x 1 x 0.2 out of the floor above, and one over its edge notches it
        app.document.place_shaft(level, &openbim_document::ShaftSpec { outline: vec![[2.0, 2.0], [3.0, 2.0], [3.0, 3.0], [2.0, 3.0]], top: upper, base_offset_m: 0.0, top_offset_m: 0.0 }).unwrap();
        assert!((floor_before - volume(&app, floor) - 0.2).abs() < 1e-3, "{}", volume(&app, floor));
        app.document.place_shaft(level, &openbim_document::ShaftSpec { outline: vec![[8.5, 0.0], [9.5, 0.0], [9.5, 1.0], [8.5, 1.0]], top: upper, base_offset_m: 0.0, top_offset_m: 0.0 }).unwrap();
        assert!((floor_before - volume(&app, floor) - 0.2 - 0.5 * 0.2).abs() < 1e-3, "{}", volume(&app, floor));
        // the file keeps the whole floor and says what voids it: both shafts, and the wall opening in the wall
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-19T00:00:00").unwrap();
        let line = |name: &str| out.text.lines().filter(|l| l.contains("IFCOPENINGELEMENT(") && l.contains(name)).count();
        assert_eq!((line("Shaft Opening 1"), line("Shaft Opening 2")), (1, 1));
        assert_eq!(out.text.matches("IFCRELVOIDSELEMENT(").count() - crate::export::service_openings(&out.text), 3, "two shafts in the floor and the wall opening in the wall");
        assert!(out.text.contains("Qto_SlabBaseQuantities"));
        // moving the wall moves its opening with it, and deleting the wall deletes the opening
        let ops = app.document.delete_commands(&[wall]).unwrap();
        app.document.transact(app.document.revision(), &ops).unwrap();
        assert!(app.document.get(op).is_none());
    }


    #[test]
    fn the_type_editor_shows_a_catalogue_that_carries_an_instance_scoped_column() {
        let mut doc = openbim_document::Document::spatial_sample();
        // neck_d is type-scoped and face is instance-scoped; a catalogue may carry both
        let ids = doc
            .load_family_catalogue(
                "family\tCeiling diffuser, square\ntype\tneck_d\tface\nD-Mixed\t0.3\t0.9\n",
            )
            .unwrap();
        let t = *ids.first().unwrap();
        let f = openbim_family::library::all()
            .unwrap()
            .into_iter()
            .find(|f| f.name() == "Ceiling diffuser, square")
            .unwrap();
        let (nums, _) = crate::export::family_type_inputs(&doc, t, &f);
        assert!(
            nums.iter().any(|(n, _)| n == "neck_d"),
            "the type-scoped column is still shown: {nums:?}"
        );
    }

    // ---- the IFC honesty bundle: what the model knows leaves in the file (gaps B7, B9, B10, B11)

    /// A duct drawn by clicks across the sample's wall, at the height the tool uses.
    fn duct_through_the_wall(app: &mut Desktop) {
        let wall = app.document.elements().find(|e| e.category == Category::Wall).expect("sample wall");
        let Some(openbim_document::Geometry::Linear { start, end, .. }) = wall.geometry else { panic!("a wall has a line") };
        let (dx, dy) = (end[0] - start[0], end[1] - start[1]);
        let len = dx.hypot(dy);
        // 0.9 m along the wall, away from whatever the sample already runs through it
        let (mx, my) = (start[0] + dx / len * 0.9, start[1] + dy / len * 0.9);
        let (nx, ny) = (-dy / len, dx / len);
        app.run_command("duct", "Duct");
        let level = app.plan_level();
        for p in [[mx - 2.0 * nx, my - 2.0 * ny], [mx + 2.0 * nx, my + 2.0 * ny]] { app.run_click(p); }
        app.finish_run(level);
    }

    #[test]
    fn a_duct_through_a_wall_cuts_an_opening_and_a_straight_run_keeps_its_system() {
        let mut app = Desktop::sample();
        let plain = crate::export::export_ifc(&app.document, "P", 1, "2026-10-23T00:00:00").unwrap();
        let ducts_before = plain.text.matches("IFCDUCTSEGMENT(").count();
        duct_through_the_wall(&mut app);
        assert!(app.document.mep().fittings.is_empty(), "a straight run has no fitting to take a system from: {}", app.message);
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-23T00:00:00").unwrap();
        assert_eq!(out.text.matches("IFCDUCTSEGMENT(").count(), ducts_before + 1);
        let kinds = |t: &str| t.matches("IFCOPENINGELEMENT(").count();
        let dbg_wall = app.document.elements().find(|e| e.category == Category::Wall).map(|w| (w.geometry, w.height_m, w.elevation_m));
        let dbg_duct = app.document.elements().filter(|e| e.category == Category::Duct).map(|d| (d.geometry, d.elevation_m, d.width_m, d.height_m)).last();
        assert!(kinds(&out.text) > kinds(&plain.text), "the duct cuts a hole in the wall: {} -> {} wall {dbg_wall:?} duct {dbg_duct:?}", kinds(&plain.text), kinds(&out.text));
        let run_system = app.document.mep().run_systems.last().expect("the tool gave the run a system").1;
        assert_ne!(run_system, openbim_family::mep::System::Other);
        // the group of that kind holds one more piece than before: the run is in its own system, not in "Other"
        let held = |text: &str| -> usize {
            let name = format!("'{} 1'", run_system.name());
            let Some(id) = text.lines().find(|l| l.contains("=IFCDISTRIBUTIONSYSTEM(") && l.contains(&name)).map(|l| l.split('=').next().unwrap().to_string()) else { return 0 };
            let rel = text.lines().find(|l| l.contains("=IFCRELASSIGNSTOGROUP(") && l.ends_with(&format!(",$,{id});"))).unwrap();
            rel.split_once('(').unwrap().1.split("$,$,$,(").nth(1).unwrap().split(')').next().unwrap().split(',').count()
        };
        assert_eq!(held(&out.text), held(&plain.text) + 1, "{} before, {} after", held(&plain.text), held(&out.text));

    }

    #[test]
    fn a_named_system_leaves_the_file_under_its_own_name_with_its_equipment_and_runs() {
        let mut app = Desktop::sample();
        let spec = FamilySpec { family: "Air handling unit".into(), type_name: String::new(), overrides: Vec::new(), angle_deg: 0.0, elevation_m: 2.7 };
        let ahu = app.document.place_family(ElementId(10), [1.0, 1.0], &spec).unwrap();
        let cmds = app.document.create_system_commands("AHU-1 Supply", openbim_family::mep::System::SupplyAir, &[ahu]).unwrap();
        let rev = app.document.revision();
        app.document.transact(rev, &cmds).unwrap();
        let out = crate::export::export_ifc(&app.document, "P", 1, "2026-10-23T00:00:00").unwrap();
        assert!(out.text.lines().any(|l| l.contains("IFCDISTRIBUTIONSYSTEM(") && l.contains("'AHU-1 Supply'")), "the user's name is on the group");
    }

    #[test]
    fn a_project_keeps_its_key_in_its_file_and_two_projects_differ() {
        let mut a = Desktop::sample();
        let mut b = Desktop::sample();
        let (ka, kb) = (a.document.ensure_project_key(), b.document.ensure_project_key());
        assert_ne!(ka, 0);
        assert_eq!(ka, a.document.ensure_project_key(), "asked twice, the same key");
        assert_ne!(ka, kb, "two projects do not share GlobalIds");
        let path = std::env::temp_dir().join(format!("openbim-key-{}.obim", std::process::id()));
        let path = path.to_str().unwrap().to_string();
        a.write_model(&path);
        let mut back = Desktop::sample();
        back.open_path(&path);
        assert_eq!(back.document.project_key(), ka, "the key came back from the file");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn the_global_ids_of_the_same_project_do_not_move_when_a_level_is_added() {
        let mut app = Desktop::sample();
        let key = app.document.ensure_project_key();
        let ids = |app: &Desktop| {
            let out = crate::export::export_ifc(&app.document, "P", key, "2026-10-23T00:00:00").unwrap();
            let pick = |class: &str| out.text.lines().filter(|l| l.contains(&format!("{class}("))).map(|l| l.split(',').next().unwrap().split('(').nth(1).unwrap().to_string()).collect::<Vec<_>>();
            (pick("IFCWALL"), pick("IFCBUILDINGSTOREY"))
        };
        let (walls_before, storeys_before) = ids(&app);
        app.document.place_level("Roof", 9.0).unwrap();
        let (walls_after, storeys_after) = ids(&app);
        assert_eq!(walls_before, walls_after, "a wall keeps its GlobalId");
        assert!(storeys_after.starts_with(&storeys_before) || storeys_before.iter().all(|g| storeys_after.contains(g)), "the storeys that were there keep theirs: {storeys_before:?} {storeys_after:?}");
        let other = { let mut o = Desktop::sample(); o.document.ensure_project_key(); ids(&o).0 };
        let _ = other;
    }
}
