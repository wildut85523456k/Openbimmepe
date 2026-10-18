use crate::{Desktop, ElementId, SampleElement, Tool};
use eframe::egui;
use openbim_document::{FamilySpec, RebarSpec, TrussSpec, BeamSpec, BeamSystemSpec, Category, ColumnSpec, Command, FootingSpec, WallFootingSpec, Geometry, Section, SlabSpec, Usage};

/// The state of the export window.
pub(crate) struct ExportDialog { pub path: String, pub schema: crate::export::Schema, pub own_data: bool }

/// The state of the import window.
pub(crate) struct ImportDialog { pub path: String }

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

    pub(crate) fn select_element(&mut self, id: Option<ElementId>) {
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
            if matches!(el.category, Category::BeamSystem | Category::Truss) { (self.spacing_mm, self.angle_deg) = (el.spacing_m * 1000.0, el.angle_deg); }
            if el.category == Category::Truss { self.material_pick = el.material; }
            if el.category == Category::Family {
                (self.angle_deg, self.family_type) = (el.angle_deg, el.type_name.clone());
                self.family_values = crate::export::instance_values_with(&el.family, &el.type_name, &el.overrides, &el.texts).unwrap_or_default();
                self.family_texts = crate::export::instance_texts(&el.family, &el.type_name, &el.overrides, &el.texts).unwrap_or_default();
            }
            if el.category == Category::Rebar { (self.spacing_mm, self.bar_count) = (el.spacing_m * 1000.0, f64::from(el.count)); }
            if el.category == Category::Slab { (self.material_pick, self.slab_kind_pick) = (el.material, el.slab_kind); }
            if matches!(el.category, Category::Beam | Category::Column | Category::BeamSystem) {
                (self.usage_pick, self.material_pick) = (el.usage, el.material);
                (self.section_pick, self.web_mm, self.flange_mm) = match el.section {
                    Section::Rectangle => (0, self.web_mm, self.flange_mm),
                    Section::Circle => (1, self.web_mm, self.flange_mm),
                    Section::IShape { web_m, flange_m } => (2, web_m * 1000.0, flange_m * 1000.0),
                };
            }
            if let Some(Geometry::Linear { start, end, .. } | Geometry::Grid { start, end }) = el.geometry {
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
        let size = matches!(el.category, Category::Wall | Category::Beam | Category::Column | Category::Duct | Category::Pipe | Category::Slab | Category::Footing | Category::WallFooting | Category::BeamSystem | Category::Truss | Category::Rebar)
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
            Some(Geometry::Linear { start, end, .. } | Geometry::Grid { start, end }) =>
                [start[0], start[1], end[0], end[1]].iter().zip(self.axis_mm)
                    .any(|(a, b)| differs(a * 1000.0, b)),
            _ => false,
        };
        let system = el.category == Category::BeamSystem && (differs(el.spacing_m * 1000.0, self.spacing_mm) || differs(el.angle_deg, self.angle_deg) || el.usage != self.usage_pick || el.material != self.material_pick);
        let truss = el.category == Category::Truss && (differs(el.spacing_m * 1000.0, self.spacing_mm) || el.material != self.material_pick);
        let family = el.category == Category::Family && (differs(el.angle_deg, self.angle_deg) || el.type_name != self.family_type || self.family_values_changed(el));
        let rebar = el.category == Category::Rebar && (differs(el.spacing_m * 1000.0, self.spacing_mm) || differs(f64::from(el.count), self.bar_count));
        let kind = system || truss || rebar || family || (el.category == Category::Beam && el.usage != self.usage_pick) || (matches!(el.category, Category::Beam | Category::Column | Category::Slab) && el.material != self.material_pick)
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
                ui.text_edit_singleline(&mut dialog.path);
                if ui.button("Browse...").clicked() {
                    let start = std::path::Path::new(&dialog.path);
                    let mut fd = rfd::FileDialog::new().set_title("Export IFC").add_filter("IFC", &["ifc"]);
                    if let Some(name) = start.file_name().and_then(|n| n.to_str()) { fd = fd.set_file_name(name); }
                    if let Some(dir) = start.parent().filter(|d| d.is_dir() && !d.as_os_str().is_empty()) { fd = fd.set_directory(dir); }
                    if let Some(p) = fd.save_file() { dialog.path = p.display().to_string(); }
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
            go = ui.button("Export").clicked();
        });
        if go { self.write_ifc(&dialog.path, dialog.schema, dialog.own_data); } else if open { self.export_dialog = Some(dialog); }
    }

    /// Opens the import window.
    pub(crate) fn import_ifc_file(&mut self) {
        self.import_dialog = Some(ImportDialog { path: String::new() });
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
                    if let Some(p) = rfd::FileDialog::new().set_title("Import IFC").add_filter("IFC (IFC2X3, IFC4, IFC 4.3)", &["ifc"]).pick_file() { dialog.path = p.display().to_string(); }
                }
            });
            if !dialog.path.trim().is_empty() { ui.small(crate::import::describe_file(dialog.path.trim())); }
            ui.small("Reads IFC2X3, IFC4 and IFC 4.3. A file written by this app with its own data comes back exactly; any other file is read from its geometry.");
            go = ui.add_enabled(!dialog.path.trim().is_empty(), egui::Button::new("Import")).clicked();
        });
        if go { self.read_ifc(dialog.path.trim()); } else if open { self.import_dialog = Some(dialog); }
    }

    /// Reads the file at `path` into the document, as one undoable step.
    pub(crate) fn read_ifc(&mut self, path: &str) {
        self.message = match std::fs::read(path) {
            Err(e) => format!("Could not read {path}: {e}"),
            Ok(bytes) => match crate::import::import_ifc(&mut self.document, &String::from_utf8_lossy(&bytes)) {
                Ok(report) => { self.sync_duct_controls(); self.selected = None; report.text() }
                Err(e) => format!("Import failed, nothing changed: {e}"),
            },
        };
    }

    /// Writes the document to `path` in `schema` and says what was and was not written.
    pub(crate) fn write_ifc(&mut self, path: &str, schema: crate::export::Schema, own_data: bool) {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        match crate::export::export_ifc_with(&self.document, "Project1", 1, &crate::export::iso_time(secs), schema, own_data) {
            Ok(out) => match std::fs::write(path, &out.text) {
                Ok(()) => self.message = format!("Exported {path} ({}): {} walls, {} beams, {} columns, {} slabs. {}", schema.name(), out.walls, out.beams, out.columns, out.slabs, out.warnings.join("; ")),
                Err(e) => self.message = format!("Could not write {path}: {e}"),
            },
            Err(e) => self.message = format!("IFC export failed: {e}"),
        }
    }

    /// The level of the plan view on show.
    pub(crate) fn plan_level(&self) -> ElementId {
        match self.view { crate::View::Level(id) if self.document.get(ElementId(id)).is_some() => ElementId(id), crate::View::UpperPlan => ElementId(11), _ => ElementId(10) }
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
    pub(crate) fn slab_click(&mut self, level: ElementId, at: [f64; 2], closing: bool) {
        if closing && self.slab_points.len() >= 3 { return self.finish_slab(level); }
        if self.slab_points.last() == Some(&at) { return; }
        self.slab_points.push(at);
        self.message = format!("Slab: {} corners; Enter closes the outline", self.slab_points.len());
    }

    /// Lays the slab drawn so far on `level` with the thickness, top offset, kind and material of the options bar.
    pub(crate) fn finish_slab(&mut self, level: ElementId) {
        let m = |v: f64| v / 1000.0;
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
        let spec = SlabSpec { outline: self.slab_points.clone(), thickness_m: m(self.slab_thickness_mm), top_offset_m: m(self.slab_offset_mm),
            kind: self.slab_kind_pick, material: self.material_pick };
        match self.document.place_slab(level, &spec) {
            Ok(id) => { self.slab_points.clear(); self.message = format!("{} {} placed; undo removes it", self.slab_kind_pick.name(), id.0); }
            Err(error) => self.message = format!("Slab rejected ({error:?}); Backspace takes a corner back"),
        }
    }

    /// Whether the instance values in the Properties differ from those of the selected family instance.
    fn family_values_changed(&self, el: &openbim_document::Element) -> bool {
        let Some(now) = crate::export::instance_values_with(&el.family, &el.type_name, &el.overrides, &el.texts) else { return false };
        let Some(texts) = crate::export::instance_texts(&el.family, &el.type_name, &el.overrides, &el.texts) else { return false };
        if el.type_name == self.family_type && (texts.len() != self.family_texts.len() || texts.iter().zip(&self.family_texts).any(|(a, b)| a.0 != b.0 || a.1 != b.1)) { return true; }
        // a changed type brings its own values, so only a same-type edit counts
        el.type_name == self.family_type && now.len() == self.family_values.len()
            && now.iter().zip(&self.family_values).any(|(a, b)| a.0 == b.0 && (a.1 - b.1).abs() > 1e-9 || !b.1.is_finite())
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
    /// finish is, so the core choices give the same lines as the finish ones.
    pub(crate) const WALL_LOCATIONS: [&'static str; 6] = ["Wall Centerline", "Core Centerline", "Finish Face: Exterior", "Finish Face: Interior", "Core Face: Exterior", "Core Face: Interior"];

    /// The axis of a wall drawn from `a` to `b` along the Location Line: the exterior face is on the left of the
    /// direction of drawing (draw clockwise and it faces out), so a line drawn along it has the axis half a
    /// thickness to its right, and one drawn along the interior face has it half a thickness to its left.
    pub(crate) fn wall_axis(&self, a: [f64; 2], b: [f64; 2]) -> ([f64; 2], [f64; 2]) {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dy);
        if len < 1e-9 { return (a, b); }
        let half = self.width_mm / 2000.0;
        let k = match self.wall_loc { 2 | 4 => -half, 3 | 5 => half, _ => 0.0 };
        let (nx, ny) = (-dy / len * k, dx / len * k);
        ([a[0] + nx, a[1] + ny], [b[0] + nx, b[1] + ny])
    }

    /// A pick of the Wall tool in the plan of `level`: the first starts a wall, each next one draws it (along the
    /// Location Line of the options bar) and, with Chain on, starts the next. The walls of a chain meet at the
    /// corners of their lines, and a pick on the first point closes the loop. Esc ends the chain.
    pub(crate) fn wall_click(&mut self, level: ElementId, point: [f64; 2]) {
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
        match self.document.place_wall_moving(level, start, end, m(self.width_mm), m(self.height_mm), m(self.elevation_mm), &moves) {
            Ok(id) => {
                self.wall_len.clear();
                if closing || !self.wall_chain_on { self.wall_pts.clear(); self.wall_ids.clear(); self.message = format!("Wall {} placed; undo removes it", id.0); }
                else { self.wall_pts.push(point); self.wall_ids.push(id); self.message = format!("Wall {} placed; pick the next point, Esc ends the chain; undo removes it", id.0); }
            }
            Err(error) => self.message = format!("Wall rejected: {error:?}"),
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
        match self.document.place_grid(start, point, "") {
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
                self.view = crate::View::Level(id.0);
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
        if matches!(el.category, Category::Wall | Category::Beam | Category::Column | Category::Duct | Category::Pipe) {
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
            Some(Geometry::Grid { .. }) => Some(Geometry::Grid { start, end }),
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
        let batch = crate::view3d::batch(&app.document);
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
        let batch = crate::view3d::batch(&app.document);
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
        let before = crate::view3d::batch(&app.document).elements.len();
        app.place_beam_from_controls();
        let id = app.selected_id().expect("the new beam is selected");
        assert!(app.tool == Tool::Select);
        let el = app.document.get(id).unwrap();
        assert_eq!(el.category, Category::Beam);
        assert!(!app.has_pending_edit(), "the controls show the beam");
        let batch = crate::view3d::batch(&app.document);
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
        let batch = crate::view3d::batch(&app.document);
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
        assert!((volume(&crate::view3d::batch(&app.document)) - 4.8).abs() < 1e-3);
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
            let batch = crate::view3d::batch(&app.document);
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
            let batch = crate::view3d::batch(&app.document);
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
        let batch = crate::view3d::batch(&app.document);
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
        let batch = crate::view3d::batch(&app.document);
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
            let batch = crate::view3d::batch(&app.document);
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
        assert_eq!(out.text.matches("IFCOPENINGELEMENT(").count(), 1);
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
        assert!(app.message.contains("Restored"), "{}", app.message);
        app.read_ifc(dir.join("missing.ifc").to_str().unwrap());
        assert!(app.message.contains("Could not read"), "{}", app.message);
        std::fs::write(dir.join("bad.ifc"), "not an ifc").unwrap();
        app.read_ifc(dir.join("bad.ifc").to_str().unwrap());
        assert!(app.message.contains("Import failed"), "{}", app.message);
        assert_eq!(app.document.elements().count(), want);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
