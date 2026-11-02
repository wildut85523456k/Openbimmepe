//! Family Types: the table of all the types of one family, as Revit's Family Types dialog and a type catalog show it.
//! One row per type (a pipe size, a valve model), one column per type input of the family (a diameter, a flow, a
//! material). Rows are edited in place, added (a copy of the last row) and deleted (while no element uses the type);
//! nothing changes in the model until Apply, and then the whole table is one undoable step, or refused whole with the
//! row that the family does not accept.
use crate::Desktop;
use eframe::egui;
use openbim_document::{Category, Document, ElementId, TypeRow};

/// One row of the table as typed so far: lengths in millimetres, everything else as the family keeps it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row { pub id: Option<ElementId>, pub name: String, pub nums: Vec<f64>, pub texts: Vec<String> }

/// A number column: the input's name and whether it is a length (shown in millimetres).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NumCol { pub name: String, pub length: bool }

/// A CSV row read but not yet put in the table: name, (column, value) numbers and (column, text) texts.
type StagedRow = (String, Vec<(usize, f64)>, Vec<(usize, String)>);

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct FamilyTypesDialog {
    pub family: String,
    pub nums: Vec<NumCol>,
    /// text, choice and material columns with the options of a choice
    pub texts: Vec<(String, Vec<String>)>,
    pub rows: Vec<Row>,
    pub removed: Vec<ElementId>,
    pub users: Vec<usize>,
    pub error: String,
    /// the CSV file typed or picked for Import
    pub csv_path: String,
    /// which row is selected for attaching an imported STEP mesh
    pub selected_row: Option<usize>,
    /// scale applied to an imported STEP mesh when attaching it to a type (1.0 = as imported)
    pub mesh_scale: f64,
}

impl FamilyTypesDialog {
    /// The table of `family` as the document has it. None for a family the library does not have.
    pub fn open(doc: &Document, family: &str) -> Option<Self> {
        let f = crate::export::library().iter().find(|f| f.name() == family)?;
        let mut d = FamilyTypesDialog { family: family.into(), mesh_scale: 1.0, ..Default::default() };
        let rows: Vec<&openbim_document::Element> = doc.types_for(Category::Family).into_iter().filter(|t| t.family == family).collect();
        // the columns come from the family itself, so a family with no type yet still has its table
        let (nums, texts) = crate::export::family_type_inputs_of(f, &[], &[])?;
        d.nums = nums.iter().map(|(n, _)| NumCol { name: n.clone(), length: f.value_dim(n) == Some(openbim_family::LENGTH) }).collect();
        d.texts = texts.iter().map(|(n, _, o)| (n.clone(), o.clone())).collect();
        for t in rows {
            let (n, x) = crate::export::family_type_inputs(doc, t.id, f);
            d.users.push(doc.instances_of(t.id).len());
            d.rows.push(d.row_of(Some(t.id), &t.name, &n, &x));
        }
        if d.rows.is_empty() { d.users.clear(); }
        Some(d)
    }

    fn row_of(&self, id: Option<ElementId>, name: &str, nums: &[(String, f64)], texts: &[(String, String, Vec<String>)]) -> Row {
        Row {
            id, name: name.into(),
            nums: self.nums.iter().map(|c| { let v = nums.iter().find(|(n, _)| *n == c.name).map_or(0.0, |x| x.1); if c.length { v * 1000.0 } else { v } }).collect(),
            texts: self.texts.iter().map(|(n, _)| texts.iter().find(|x| x.0 == *n).map_or(String::new(), |x| x.1.clone())).collect(),
        }
    }

    /// A new row after the last one: a copy of it (or of the family's defaults) under a name not used yet.
    pub fn add_row(&mut self) {
        let mut r = self.rows.last().cloned().unwrap_or_else(|| {
            let f = crate::export::library().iter().find(|f| f.name() == self.family);
            let (n, x) = f.and_then(|f| crate::export::family_type_inputs_of(f, &[], &[])).unwrap_or_default();
            self.row_of(None, "", &n, &x)
        });
        r.id = None;
        let mut k = self.rows.len() + 1;
        while self.rows.iter().any(|x| x.name == format!("Type {k}")) { k += 1; }
        r.name = format!("Type {k}");
        self.rows.push(r);
        self.users.resize(self.rows.len(), 0);
    }

    /// Reads a CSV (comma, semicolon or tab separated) into the table. The first line names the columns: the first is
    /// the type name, the others are inputs of the family, written as the table shows them (`pipe d (mm)` and
    /// `pipe_d` are the same column; `(m)` says metres). A row whose name is already in the table replaces that row's
    /// values; any other row is added; a column the CSV leaves out keeps the first row's value (the family's default
    /// for an empty table). A file with a column the family does not have, or a cell that is not a number, is
    /// refused whole and says where, so a half-read table never reaches the model. Returns (rows replaced, rows added).
    pub fn import_csv(&mut self, text: &str) -> Result<(usize, usize), String> {
        let text = text.trim_start_matches('\u{feff}');
        let mut lines = text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty() && !l.starts_with('#'));
        let (_, head) = lines.next().ok_or("the file is empty")?;
        let sep = ['\t', ';', ','].into_iter().find(|c| head.contains(*c)).ok_or("no separator found: use commas, semicolons or tabs")?;
        let cells = |l: &str| -> Vec<String> { l.split(sep).map(|c| c.trim().trim_matches('"').trim().to_string()).collect() };
        // which table column each CSV column fills, and whether the CSV gives it in metres
        let norm = |h: &str| -> (String, bool) {
            let h = h.to_lowercase();
            let metres = h.contains("(m)");
            let h = h.replace("(mm)", "").replace("(m)", "");
            (h.trim().replace(' ', "_"), metres)
        };
        let mut nums: Vec<(usize, usize, bool)> = Vec::new();
        let mut txts: Vec<(usize, usize)> = Vec::new();
        for (c, h) in cells(head).iter().enumerate().skip(1) {
            let (name, metres) = norm(h);
            if let Some(k) = self.nums.iter().position(|x| x.name.to_lowercase() == name) { nums.push((c, k, metres)); }
            else if let Some(k) = self.texts.iter().position(|x| x.0.to_lowercase() == name) { txts.push((c, k)); }
            else { return Err(format!("column {} ({h}) is not an input of {}", c + 1, self.family)); }
        }
        if nums.is_empty() && txts.is_empty() { return Err("the first line names no column of the family".into()); }
        let mut staged: Vec<StagedRow> = Vec::new();
        for (i, l) in lines {
            let c = cells(l);
            if c[0].is_empty() { return Err(format!("line {}: the type needs a name", i + 1)); }
            if staged.iter().any(|s| s.0 == c[0]) { return Err(format!("line {}: the name {} is used twice", i + 1, c[0])); }
            let (mut n, mut t) = (Vec::new(), Vec::new());
            for &(col, k, metres) in &nums {
                let cell = c.get(col).map(String::as_str).unwrap_or("");
                if cell.is_empty() { continue; }
                let v = match cell.to_lowercase().as_str() { "yes" | "true" => 1.0, "no" | "false" => 0.0, x => x.parse::<f64>().map_err(|_| format!("line {}, {}: {cell} is not a number", i + 1, self.nums[k].name))? };
                n.push((k, if metres && self.nums[k].length { v * 1000.0 } else { v }));
            }
            for &(col, k) in &txts { if let Some(cell) = c.get(col).filter(|x| !x.is_empty()) { t.push((k, cell.clone())); } }
            staged.push((c[0].clone(), n, t));
        }
        if staged.is_empty() { return Err("the file has a header but no rows".into()); }
        let (mut replaced, mut added) = (0, 0);
        for (name, n, t) in staged {
            let k = match self.rows.iter().position(|r| r.name == name) {
                Some(k) => { replaced += 1; k }
                None => { self.add_row(); added += 1; let k = self.rows.len() - 1; self.rows[k].name = name; k }
            };
            for (j, v) in n { self.rows[k].nums[j] = v; }
            for (j, v) in t { self.rows[k].texts[j] = v; }
        }
        self.error.clear();
        Ok((replaced, added))
    }

    /// Takes row `k` out of the table; a type that exists is deleted when the table is applied.
    pub fn remove_row(&mut self, k: usize) {
        if k >= self.rows.len() { return; }
        if let Some(id) = self.rows[k].id { self.removed.push(id); }
        self.rows.remove(k);
        if k < self.users.len() { self.users.remove(k); }
    }

    /// The rows as the document keeps them (metres).
    pub fn type_rows(&self) -> Vec<TypeRow> {
        self.rows.iter().map(|r| TypeRow {
            id: r.id, name: r.name.clone(),
            values: self.nums.iter().zip(&r.nums).map(|(c, v)| (c.name.clone(), if c.length { v / 1000.0 } else { *v })).collect(),
            texts: self.texts.iter().zip(&r.texts).filter(|(_, v)| !v.is_empty()).map(|((n, _), v)| (n.clone(), v.clone())).collect(),
        }).collect()
    }
}

impl Desktop {
    /// Opens the table of the types of `family`.
    pub(crate) fn open_family_types(&mut self, family: &str) {
        match FamilyTypesDialog::open(&self.document, family) {
            Some(d) => self.family_types_dialog = Some(d),
            None => self.message = format!("No family {family} in the library"),
        }
    }

    /// Applies the table: every row is checked against the family first, then all of it is one undoable step.
    pub(crate) fn attach_step_to_type(&mut self, d: &mut FamilyTypesDialog) {
        let Some(k) = d.selected_row else { return };
        if k >= d.rows.len() { return; }
        let Some(type_id) = d.rows[k].id else { self.message = "That type has not been saved yet; press Apply first".into(); return; };
        let Some(path) = crate::dialog::Question::new().set_title("Import STEP for type").add_filter("STEP", &["step", "stp"]).ask_open() else { return };
        match crate::step_import::import_step(&path.display().to_string()) {
            Ok(mut mesh) => {
                let scale = d.mesh_scale;
                if (scale - 1.0).abs() > 1e-9 {
                    for v in &mut mesh.vertices { v[0] *= scale; v[1] *= scale; v[2] *= scale; }
                }
                let verts = mesh.vertices.len();
                let tris = mesh.faces.len();
                self.document.set_display_mesh(type_id, Some(mesh));
                self.message = format!("Attached STEP ({} vertices, {} triangles, scale {:.2}) to type {}; instances of that type now show it", verts, tris, scale, self.document.get(type_id).map(|e| e.name.clone()).unwrap_or_default());
            }
            Err(e) => self.message = format!("STEP import failed: {e}"),
        }
    }

    /// Applies the table: every row is checked against the family first, then all of it is one undoable step.
    pub(crate) fn apply_family_types(&mut self, d: &mut FamilyTypesDialog) -> bool {
        match self.document.save_family_types(&d.family, &d.type_rows(), &d.removed) {
            Ok(_) => {
                self.message = format!("{}: {} types saved; undo puts the table back", d.family, d.rows.len());
                if let Some(fresh) = FamilyTypesDialog::open(&self.document, &d.family) { *d = fresh; }
                true
            }
            Err(why) => { d.error = why.clone(); self.message = format!("Family types refused, nothing changed: {why}"); false }
        }
    }

    pub(crate) fn family_types_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.family_types_dialog.take() else { return };
        let (mut open, mut apply, mut add, mut remove, mut import, mut attach_step) = (true, false, false, None, false, false);
        egui::Window::new(format!("Family Types: {}", d.family)).open(&mut open).default_width(640.0).show(ctx, |ui| {
            egui::ScrollArea::both().max_height(360.0).id_salt("family_types_table").show(ui, |ui| {
                egui::Grid::new("family_types_grid").striped(true).show(ui, |ui| {
                    ui.strong("Type name");
                    for c in &d.nums { ui.strong(format!("{}{}", c.name.replace('_', " "), if c.length { " (mm)" } else { "" })); }
                    for (n, _) in &d.texts { ui.strong(n.replace('_', " ")); }
                    ui.strong("");
                    ui.end_row();
                    let (nums, texts) = (d.nums.clone(), d.texts.clone());
                    for k in 0..d.rows.len() {
                        let used = d.users.get(k).copied().unwrap_or(0);
                        let r = &mut d.rows[k];
                        let selected = d.selected_row == Some(k);
                        if ui.selectable_label(selected, "●").clicked() { d.selected_row = Some(k); }
                        ui.add(egui::TextEdit::singleline(&mut r.name).desired_width(110.0));
                        for (v, c) in r.nums.iter_mut().zip(&nums) {
                            ui.add(egui::DragValue::new(v).speed(if c.length { 1.0 } else { 0.01 }));
                        }
                        for (v, (n, opts)) in r.texts.iter_mut().zip(&texts) {
                            if opts.is_empty() { ui.add(egui::TextEdit::singleline(v).desired_width(90.0)); } else {
                                egui::ComboBox::from_id_salt(("ft_choice", k, n.as_str())).selected_text(v.clone()).show_ui(ui, |ui| { for o in opts { ui.selectable_value(v, o.clone(), o); } });
                            }
                        }
                        if ui.add_enabled(used == 0, egui::Button::new("Delete")).on_disabled_hover_text(format!("{used} element{} use this type", if used == 1 { "" } else { "s" })).clicked() { remove = Some(k); }
                        ui.end_row();
                    }
                });
            });
            ui.horizontal(|ui| {
                if ui.button("Add type").on_hover_text("A copy of the last row").clicked() { add = true; }
                apply = ui.button("Apply").clicked();
            });
            ui.horizontal(|ui| {
                ui.label("CSV");
                ui.add(egui::TextEdit::singleline(&mut d.csv_path).desired_width(260.0));
                if ui.button("Browse...").clicked() {
                    if let Some(p) = crate::dialog::Question::new().set_title("Import types from CSV").add_filter("CSV or tab separated", &["csv", "tsv", "txt"]).ask_open() { d.csv_path = p.display().to_string(); }
                }
                import = ui.add_enabled(!d.csv_path.trim().is_empty(), egui::Button::new("Import CSV")).clicked();
            });
            ui.horizontal(|ui| {
                ui.label("Scale");
                ui.add(egui::DragValue::new(&mut d.mesh_scale).speed(0.01).range(0.01..=100.0));
                let enabled = d.selected_row.is_some_and(|k| k < d.rows.len() && d.rows[k].id.is_some());
                if ui.add_enabled(enabled, egui::Button::new("Import STEP for selected type")).clicked() { attach_step = true; }
                if !enabled { ui.small("select a saved type row first"); }
            });
            ui.small("CSV: the first line names the columns (type name first, then inputs as shown above), one line per type. Lengths are in millimetres. Nothing changes in the model until Apply; Apply is one undo step, and a row the family does not accept refuses the whole table.");
        });
        if add { d.add_row(); }
        if let Some(k) = remove { d.remove_row(k); }
        if import {
            let path = d.csv_path.trim().to_string();
            self.message = match std::fs::read_to_string(&path) {
                Err(e) => format!("Could not read {path}: {e}"),
                Ok(text) => match d.import_csv(&text) {
                    Ok((r, a)) => format!("CSV read: {r} types replaced, {a} added in the table; press Apply to save them"),
                    Err(why) => { d.error = why.clone(); format!("CSV refused, the table is as it was: {why}") }
                },
            };
        }
        if attach_step {
            self.attach_step_to_type(&mut d);
        }
        if apply { self.apply_family_types(&mut d); }
        if open { self.family_types_dialog = Some(d); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbim_document::FamilySpec;

    fn elbow_table(app: &Desktop) -> FamilyTypesDialog { FamilyTypesDialog::open(&app.document, "Pipe elbow").unwrap() }

    #[test]
    fn a_table_of_pipe_sizes_gives_the_elbow_a_type_and_a_model_for_each_size() {
        let mut app = Desktop::sample();
        let mut d = elbow_table(&app);
        assert!(d.rows.is_empty(), "no type yet");
        let col = |d: &FamilyTypesDialog, n: &str| d.nums.iter().position(|c| c.name == n).unwrap();
        assert!(d.nums[col(&d, "pipe_d")].length, "a diameter is a length, shown in mm");
        let sizes = [15.0, 25.0, 50.0, 100.0, 200.0];
        for s in sizes {
            d.add_row();
            let k = d.rows.len() - 1;
            d.rows[k].name = format!("DN{s}");
            let c = col(&d, "pipe_d");
            d.rows[k].nums[c] = s;
        }
        assert!(app.apply_family_types(&mut d), "{}", d.error);
        assert_eq!(app.document.types_for(Category::Family).iter().filter(|t| t.family == "Pipe elbow").count(), 5);
        // the whole table was one step: one undo removes all five, and redo brings them back
        app.document.undo().unwrap();
        assert!(app.document.types_for(Category::Family).iter().all(|t| t.family != "Pipe elbow"));
        app.document.redo().unwrap();
        assert_eq!(app.document.types_for(Category::Family).iter().filter(|t| t.family == "Pipe elbow").count(), 5);
        // each type is a different 3D model: the bend grows with the diameter, and so do the connectors
        let f = crate::export::library().iter().find(|f| f.name() == "Pipe elbow").unwrap();
        let mut widths = Vec::new();
        for s in sizes {
            let t = app.document.types_for(Category::Family).iter().find(|t| t.name == format!("DN{s}")).map(|t| t.id).unwrap();
            let spec = FamilySpec { family: "Pipe elbow".into(), type_name: String::new(), overrides: Vec::new(), angle_deg: 0.0, elevation_m: 1.0 };
            let el = app.document.place_family(ElementId(10), [2.0, 2.0], &spec).unwrap();
            app.document.assign_type(&[el], t).unwrap();
            let parts = openbim_document::request_parts(&app.document, app.document.get(el).unwrap(), f);
            let inst = f.solve(&parts.request(None)).unwrap();
            let (lo, hi) = inst.forms[0].bounds_local();
            widths.push(hi[0] - lo[0]);
            let c = crate::elec_ui::instance_connectors(&app.document, el);
            assert_eq!(c.len(), 2);
            assert_eq!(c[0].size, openbim_family::mep::Size::Round { diameter: s / 1000.0 }, "the connector takes the table's diameter");
            assert!((hi[1] - lo[1] - (1.5 * s / 1000.0 + s / 2000.0)).abs() < 1e-6, "the bend radius follows the diameter");
        }
        assert!(widths.windows(2).all(|w| w[1] > w[0]), "{widths:?}");
        // the table opens again with the five rows, lengths back in millimetres
        let d2 = elbow_table(&app);
        assert_eq!(d2.rows.len(), 5);
        let dn15 = d2.rows.iter().find(|r| r.name == "DN15").unwrap();
        assert!((dn15.nums[col(&d2, "pipe_d")] - 15.0).abs() < 1e-9);
    }

    #[test]
    fn a_row_the_family_does_not_accept_refuses_the_table_and_a_used_type_is_kept() {
        let mut app = Desktop::sample();
        let mut d = elbow_table(&app);
        d.add_row(); d.rows[0].name = "DN50".into(); d.rows[0].nums[0] = 50.0;
        assert!(app.apply_family_types(&mut d));
        let mut d = elbow_table(&app);
        d.add_row();
        d.rows[1].name = "Too small".into(); d.rows[1].nums[0] = 2.0; // 2 mm: under the family's 10 mm
        d.rows[0].nums[0] = 65.0; // a good change in the same table must not happen either
        let before = app.document.revision();
        assert!(!app.apply_family_types(&mut d));
        assert!(d.error.contains("row 2"), "{}", d.error);
        assert_eq!(app.document.revision(), before, "nothing changed");
        assert_eq!(elbow_table(&app).rows[0].nums[0], 50.0);
        // a type in use cannot be taken out of the table
        let t = app.document.types_for(Category::Family).iter().find(|t| t.name == "DN50").map(|t| t.id).unwrap();
        let spec = FamilySpec { family: "Pipe elbow".into(), type_name: String::new(), overrides: Vec::new(), angle_deg: 0.0, elevation_m: 1.0 };
        let el = app.document.place_family(ElementId(10), [2.0, 2.0], &spec).unwrap();
        app.document.assign_type(&[el], t).unwrap();
        let mut d = elbow_table(&app);
        assert_eq!(d.users, vec![1]);
        d.remove_row(0);
        assert!(!app.apply_family_types(&mut d));
        assert!(app.document.get(t).is_some(), "the type is still there");
        // two rows with one name are refused too
        let mut d = elbow_table(&app);
        d.add_row(); d.rows[1].name = "DN50".into();
        assert!(!app.apply_family_types(&mut d) && d.error.contains("twice"), "{}", d.error);
    }

    #[test]
    fn a_csv_fills_the_table_in_one_go_and_a_bad_file_changes_nothing() {
        let mut app = Desktop::sample();
        let mut d = elbow_table(&app);
        let csv = "Type name;pipe d (mm);radius factor\nDN15;15;1.5\nDN20;20;1.5\nDN25;25;1\n";
        assert_eq!(d.import_csv(csv), Ok((0, 3)));
        assert_eq!(d.rows.len(), 3);
        // metres, commas, quotes and a repeated name that replaces the row
        assert_eq!(d.import_csv("\"name\",\"pipe_d (m)\"\n\"DN20\",0.021\n"), Ok((1, 0)));
        assert!((d.rows[1].nums[0] - 21.0).abs() < 1e-9 && d.rows[1].nums[1] == 1.5);
        let before = d.clone();
        for bad in ["type,colour\nA,red\n", "type,pipe_d\nA,abc\n", "type,pipe_d\nA,1\nA,2\n", "type,pipe_d\n", ""] {
            assert!(d.import_csv(bad).is_err(), "{bad}");
            assert_eq!(d.rows, before.rows, "a refused file changes nothing: {bad}");
        }
        assert!(app.apply_family_types(&mut d), "{}", d.error);
        assert_eq!(app.document.types_for(Category::Family).iter().filter(|t| t.family == "Pipe elbow").count(), 3);
        let t = app.document.types_for(Category::Family).iter().find(|t| t.name == "DN25").map(|t| t.id).unwrap();
        assert!(app.document.family_type_values(t).contains(&("radius_factor".to_string(), 1.0)));
        assert!(app.document.family_type_values(t).iter().any(|(n, v)| n == "pipe_d" && (v - 0.025).abs() < 1e-9));
    }

    #[test]
    fn the_table_gives_the_45_degree_elbow_with_its_outlet_aimed_along_the_bend() {
        let mut app = Desktop::sample();
        let mut d = elbow_table(&app);
        assert_eq!(d.import_csv("type,pipe_d,angle\nDN50 x 90,50,90\nDN50 x 45,50,45\n"), Ok((0, 2)));
        assert!(app.apply_family_types(&mut d), "{}", d.error);
        let spec = FamilySpec { family: "Pipe elbow".into(), type_name: String::new(), overrides: Vec::new(), angle_deg: 0.0, elevation_m: 1.0 };
        let el = app.document.place_family(ElementId(10), [2.0, 2.0], &spec).unwrap();
        let t = app.document.types_for(Category::Family).iter().find(|t| t.name == "DN50 x 45").map(|t| t.id).unwrap();
        app.document.assign_type(&[el], t).unwrap();
        let c = crate::elec_ui::instance_connectors(&app.document, el);
        let (inlet, out) = (&c[0], &c[1]);
        let dot = inlet.dir[0] * out.dir[0] + inlet.dir[1] * out.dir[1];
        assert!((dot + 45f64.to_radians().cos()).abs() < 1e-6, "the two connectors face 135 degrees apart (a 45 degree turn): {:?} {:?}", inlet.dir, out.dir);
    }

    #[test]
    fn a_csv_gives_the_tee_and_the_reducer_a_type_for_each_pair_of_sizes_and_the_elbow_its_turn() {
        let mut app = Desktop::sample();
        let mut tee = FamilyTypesDialog::open(&app.document, "Pipe tee").unwrap();
        assert_eq!(tee.import_csv("type,pipe d (mm),branch d (mm),branch\nT50x50,50,50,left\nT50x32,50,32,up\nT80x50,80,50,down\n"), Ok((0, 3)));
        assert!(app.apply_family_types(&mut tee), "{}", tee.error);
        let mut red = FamilyTypesDialog::open(&app.document, "Pipe reducer").unwrap();
        assert_eq!(red.import_csv("type;pipe d (mm);pipe d2 (mm);length (mm);flat\nR50x32;50;32;80;concentric\nR50x32 ecc;50;32;80;bottom\n"), Ok((0, 2)));
        assert!(app.apply_family_types(&mut red), "{}", red.error);
        // a branch larger than the run is a row the family refuses, and the table says which
        let mut bad = FamilyTypesDialog::open(&app.document, "Pipe tee").unwrap();
        assert!(bad.import_csv("type,pipe d (mm),branch d (mm)\nT32x50,32,50\n").is_ok());
        assert!(!app.apply_family_types(&mut bad) && bad.error.contains("T32x50"), "{}", bad.error);
        // the elbow takes its turn from a choice column
        let mut el = elbow_table(&app);
        assert_eq!(el.import_csv("type,pipe_d,turn\nDN50 up,50,up\n"), Ok((0, 1)));
        assert!(app.apply_family_types(&mut el), "{}", el.error);
        let spec = FamilySpec { family: "Pipe elbow".into(), type_name: String::new(), overrides: Vec::new(), angle_deg: 0.0, elevation_m: 1.0 };
        let e = app.document.place_family(ElementId(10), [2.0, 2.0], &spec).unwrap();
        let t = app.document.types_for(Category::Family).iter().find(|t| t.name == "DN50 up").map(|t| t.id).unwrap();
        app.document.assign_type(&[e], t).unwrap();
        let c = crate::elec_ui::instance_connectors(&app.document, e);
        assert_eq!(c.len(), 2);
        assert!(c[1].dir[2] > 0.99, "an up elbow leaves upwards: {:?}", c[1].dir);
    }
}
