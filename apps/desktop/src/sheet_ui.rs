//! Sheets, as Revit's View > Sheet Composition has them: New Sheet with a paper size and a title block, Place View
//! (plans, elevations, sections, the 3D view and schedules go on a sheet at the scale of the view, and a view is on
//! one sheet only), moving the views on the paper, Sheet Properties for the title block texts, and Print to PDF.
use crate::pdf::Pdf;
use crate::sheet_draw::{self, Drawing, Placed, Prim};
use crate::{Desktop, View};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use openbim_document::{ElementId, PaperSize, SheetDef};
use std::collections::BTreeMap;

/// The New Sheet window.
pub(crate) struct NewSheet {
    pub size: PaperSize,
    pub portrait: bool,
    pub number: String,
    pub name: String,
}

/// The Sheet Properties window.
pub(crate) struct Props {
    pub draft: SheetDef,
}

/// The Place View window: the sheet and the view, either of which may be given already.
pub(crate) struct AddView {
    pub sheet: Option<u64>,
    pub view: Option<String>,
}

/// The Print window.
pub(crate) struct PrintDialog {
    pub sheets: Vec<(u64, bool)>,
    pub black_and_white: bool,
}

struct Cache {
    revision: u64,
    sheet: u64,
    /// the drawing of each viewport, with the view it was made for
    drawings: BTreeMap<u64, (String, Result<Drawing, String>)>,
}

struct Drag {
    viewport: u64,
    /// where the pointer was on the paper when the drag began
    from: [f64; 2],
    /// how far it has moved, in millimetres
    by: [f64; 2],
}

#[derive(Default)]
pub(crate) struct SheetUi {
    pub new: Option<NewSheet>,
    pub props: Option<Props>,
    pub add: Option<AddView>,
    pub print: Option<PrintDialog>,
    cache: Option<Cache>,
    /// the viewport that is selected
    pub selected: Option<u64>,
    drag: Option<Drag>,
    pan: Vec2,
    zoom: Option<f32>,
    /// the title being typed for the selected viewport
    title_edit: Option<(u64, String)>,
}

/// The paper's size in pixels per millimetre and where its top left corner is, for a canvas of `rect`.
fn layout(rect: Rect, sheet: &SheetDef, pan: Vec2, zoom: f32) -> (f32, Pos2) {
    let (w, h) = sheet.dims_mm();
    let fit = ((rect.width() - 30.0).max(10.0) / w as f32).min((rect.height() - 30.0).max(10.0) / h as f32);
    let k = fit * zoom;
    (k, rect.center() + pan - Vec2::new(w as f32, h as f32) * k / 2.0)
}

fn rgb(c: [u8; 3]) -> Color32 { Color32::from_rgb(c[0], c[1], c[2]) }

/// Paints the shapes of a sheet: `k` pixels to the millimetre from `origin`.
fn paint(painter: &egui::Painter, prims: &[Prim], k: f32, origin: Pos2) {
    let at = |p: [f64; 2]| origin + Vec2::new(p[0] as f32, p[1] as f32) * k;
    let clip = painter.clip_rect().expand(40.0);
    let many = prims.len() > 15_000;
    for prim in prims {
        // what is outside the canvas is not painted
        if let Some(b) = prim.bounds() {
            let r = Rect::from_min_max(at([b[0], b[1]]), at([b[2], b[3]]));
            if !r.intersects(clip) { continue; }
        }
        let stroke = |c: [u8; 3], w: f64| Stroke::new((w as f32 * k).clamp(0.5, 8.0), rgb(c));
        match prim {
            Prim::Poly { rings, fill, stroke: edge } => {
                if let (Some(f), Some(outer)) = (fill, rings.first()) {
                    let holes: Vec<Vec<[f64; 2]>> = rings[1..].to_vec();
                    if outer.len() == 3 && holes.is_empty() {
                        painter.add(egui::Shape::convex_polygon(outer.iter().map(|p| at(*p)).collect(), rgb(*f), Stroke::NONE));
                    } else {
                        let mut mesh = egui::epaint::Mesh::default();
                        for (a, b, c) in crate::section_ui::triangles(outer, &holes) {
                            let base = mesh.vertices.len() as u32;
                            for q in [a, b, c] { mesh.colored_vertex(at(q), rgb(*f)); }
                            mesh.add_triangle(base, base + 1, base + 2);
                        }
                        painter.add(egui::Shape::mesh(mesh));
                    }
                }
                if let Some((c, w)) = edge {
                    if many && *w < 0.1 { continue; }
                    for ring in rings { painter.add(egui::Shape::closed_line(ring.iter().map(|p| at(*p)).collect(), stroke(*c, *w))); }
                }
            }
            Prim::Line { a, b, rgb: c, w, dash } => {
                let (pa, pb) = (at(*a), at(*b));
                match dash {
                    Some((d, g)) => { painter.extend(egui::Shape::dashed_line(&[pa, pb], stroke(*c, *w), (*d as f32 * k).max(1.0), (*g as f32 * k).max(1.0))); }
                    None => { painter.line_segment([pa, pb], stroke(*c, *w)); }
                }
            }
            Prim::Circle { c, r, fill, stroke: edge } => {
                let (centre, radius) = (at(*c), (*r as f32 * k).max(0.5));
                if let Some(f) = fill { painter.circle_filled(centre, radius, rgb(*f)); }
                if let Some((col, w)) = edge { painter.circle_stroke(centre, radius, stroke(*col, *w)); }
            }
            Prim::Text { at: p, size, text, align, rgb: c, bold, degrees } => {
                let px = (*size as f32 * k).clamp(2.0, 400.0);
                if px < 3.5 { continue; }
                let font = egui::FontId::proportional(px);
                let galley = painter.layout_no_wrap(text.clone(), font, rgb(*c));
                let w = galley.size().x;
                let back = match align { crate::pdf::Align::Left => 0.0, crate::pdf::Align::Center => w / 2.0, crate::pdf::Align::Right => w };
                let rad = (-*degrees as f32).to_radians();
                let (s, co) = rad.sin_cos();
                // the galley's top left, from the baseline point: back along the text and up by the ascent
                let ascent = px * 0.8;
                let base = at(*p);
                let top_left = base + Vec2::new(-back * co + ascent * s, -back * s - ascent * co);
                let _ = bold;
                painter.add(egui::epaint::TextShape::new(top_left, galley, rgb(*c)).with_angle(rad));
            }
        }
    }
}

impl Desktop {
    pub(crate) fn open_new_sheet(&mut self) {
        let size = self.document.sheets().last().map_or(PaperSize::A1, |s| s.size);
        let portrait = self.document.sheets().last().is_some_and(|s| s.portrait);
        self.sheets.new = Some(NewSheet { size, portrait, number: self.document.next_sheet_number(), name: "Unnamed".into() });
    }

    /// Makes the sheet and opens it. False (with a message) when the model refuses it.
    pub(crate) fn confirm_new_sheet(&mut self, d: &NewSheet) -> bool {
        match self.document.new_sheet_commands(d.size, d.portrait, &d.number, &d.name) {
            Ok((ops, id)) => match self.document.transact(self.document.revision(), &ops) {
                Ok(_) => {
                    self.view = View::Sheet(id);
                    self.selected = None;
                    self.sheets.selected = None;
                    self.message = format!("Sheet {} made; Place View puts views on it, undo takes it back", self.document.sheet(id).map_or(String::new(), SheetDef::label));
                    true
                }
                Err(e) => { self.message = format!("Sheet rejected ({e:?})"); false }
            },
            Err(e) => { self.message = format!("Sheet: {e}"); false }
        }
    }

    /// Saves a change to the sheets in one undo step. The drawings of the views are kept, because moving a view on the
    /// paper does not change what it shows.
    pub(crate) fn apply_sheet(&mut self, ops: Result<Vec<openbim_document::Command>, String>, done: &str) -> bool {
        let ops = match ops { Ok(o) => o, Err(e) => { self.message = format!("Sheet: {e}"); return false; } };
        let before = self.document.revision();
        match self.document.transact(before, &ops) {
            Ok(_) => {
                if let Some(c) = self.sheets.cache.as_mut() { if c.revision == before { c.revision = self.document.revision(); } }
                self.message = format!("{done}; undo takes it back");
                true
            }
            Err(e) => { self.message = format!("Sheet rejected ({e:?})"); false }
        }
    }

    /// The drawings of the viewports of a sheet, made when the model changed or a viewport is new.
    fn sheet_drawings(&mut self, sheet: u64) -> BTreeMap<u64, Result<Drawing, String>> {
        let Some(def) = self.document.sheet(sheet).cloned() else { return BTreeMap::new() };
        let rev = self.document.revision();
        if self.sheets.cache.as_ref().is_none_or(|c| c.revision != rev || c.sheet != sheet) {
            self.sheets.cache = Some(Cache { revision: rev, sheet, drawings: BTreeMap::new() });
        }
        let stale: Vec<&openbim_document::Viewport> = def.viewports.iter().filter(|v| self.sheets.cache.as_ref().is_none_or(|c| c.drawings.get(&v.id).is_none_or(|d| d.0 != v.view))).collect();
        if !stale.is_empty() {
            use rayon::prelude::*;
            let doc = &self.document;
            let made: Vec<(u64, String, Result<Drawing, String>)> = stale.par_iter().map(|v| (v.id, v.view.clone(), sheet_draw::view_drawing(doc, &v.view))).collect();
            if let Some(c) = self.sheets.cache.as_mut() { for (id, view, d) in made { c.drawings.insert(id, (view, d)); } }
        }
        let c = self.sheets.cache.as_ref();
        def.viewports.iter().filter_map(|v| c.and_then(|c| c.drawings.get(&v.id)).map(|d| (v.id, d.1.clone()))).collect()
    }

    /// The views and schedules that can be put on a sheet and are not on one: (view key, name).
    pub(crate) fn placeable_views(&self) -> Vec<(String, String)> {
        let mut keys: Vec<String> = Vec::new();
        for (id, _, _) in self.levels() { keys.push(format!("p{}", id.0)); }
        keys.push("mech".into());
        keys.push("3d".into());
        keys.extend((0..4).map(|k| format!("e{k}")));
        for e in self.document.elements() {
            match e.category {
                openbim_document::Category::Section => keys.push(format!("s{}", e.id.0)),
                openbim_document::Category::Elevation => keys.extend((0..4u8).filter(|k| e.count >> k & 1 == 1).map(|k| format!("m{}_{k}", e.id.0))),
                _ => {}
            }
        }
        keys.extend(self.document.mep().schedules.iter().map(|s| openbim_document::schedule_view(s.id)));
        keys.into_iter().filter(|k| self.document.sheet_of_view(k).is_none()).map(|k| { let n = sheet_draw::view_title(&self.document, &k); (k, n) }).collect()
    }

    pub(crate) fn open_add_view(&mut self, sheet: Option<u64>, view: Option<String>) {
        if self.document.sheets().is_empty() { self.message = "Make a sheet first: View > Sheet".into(); return; }
        let sheet = sheet.or(match self.view { View::Sheet(id) => Some(id), _ => None }).or_else(|| (self.document.sheets().len() == 1).then(|| self.document.sheets()[0].id));
        self.sheets.add = Some(AddView { sheet, view });
    }

    /// Puts a view on a sheet, in the middle of the drawing area. False (with a message) when it cannot.
    pub(crate) fn add_view_to_sheet(&mut self, sheet: u64, key: &str) -> bool {
        let Some(def) = self.document.sheet(sheet).cloned() else { self.message = "There is no such sheet".into(); return false };
        if let Some(on) = self.document.sheet_of_view(key) { self.message = format!("That view is on sheet {} already; a view is on one sheet only", on.label()); return false; }
        let drawing = match sheet_draw::view_drawing(&self.document, key) { Ok(d) => d, Err(e) => { self.message = format!("{} cannot be put on a sheet: {e}", sheet_draw::view_title(&self.document, key)); return false; } };
        let drawings = self.sheet_drawings(sheet);
        let (_, placed) = sheet_draw::compose(&self.document, &def, &drawings, None);
        let boxes: Vec<[f64; 4]> = placed.iter().map(|p| p.rect).collect();
        let (x, y) = sheet_draw::default_place(&def, &boxes, (drawing.w, drawing.h));
        let ops = self.document.place_viewport_commands(sheet, key, (x * 2.0).round() / 2.0, (y * 2.0).round() / 2.0).map(|r| r.0);
        let name = sheet_draw::view_title(&self.document, key);
        let ok = self.apply_sheet(ops, &format!("{name} put on sheet {}", def.label()));
        if ok { self.sheets.selected = self.document.sheet(sheet).and_then(|s| s.viewports.last().map(|v| v.id)); }
        ok
    }

    pub(crate) fn remove_selected_viewport(&mut self, sheet: u64) {
        let Some(vp) = self.sheets.selected else { return };
        let ops = self.document.remove_viewport_commands(sheet, vp);
        if self.apply_sheet(ops, "View taken off the sheet") { self.sheets.selected = None; }
    }

    /// Opens the view of the selected viewport (Revit's Activate View).
    pub(crate) fn open_viewport_view(&mut self, sheet: u64, vp: u64) {
        let Some(key) = self.document.sheet(sheet).and_then(|s| s.viewport(vp)).map(|v| v.view.clone()) else { return };
        match crate::view_ui::view_from_key(&key) {
            Some(v) => { self.view = v; self.selected = None; }
            None => self.message = "That view cannot be opened".into(),
        }
    }

    pub(crate) fn open_sheet_properties(&mut self, id: u64) {
        if let Some(s) = self.document.sheet(id) { self.sheets.props = Some(Props { draft: s.clone() }); }
    }

    pub(crate) fn delete_sheet(&mut self, id: u64) {
        let label = self.document.sheet(id).map_or(String::new(), SheetDef::label);
        let ops = self.document.delete_sheet_commands(id);
        if self.apply_sheet(ops, &format!("Sheet {label} deleted")) && self.view == View::Sheet(id) { self.view = View::Plan; }
    }

    pub(crate) fn open_print(&mut self) {
        if self.document.sheets().is_empty() { self.message = "Nothing to print: make a sheet first (View > Sheet)".into(); return; }
        let current = match self.view { View::Sheet(id) => Some(id), _ => None };
        self.sheets.print = Some(PrintDialog { sheets: self.document.sheets().iter().map(|s| (s.id, current.is_none_or(|c| c == s.id))).collect(), black_and_white: false });
    }

    /// Writes the sheets to one PDF file, a page each, in the order of their numbers.
    pub(crate) fn print_sheets(&mut self, ids: &[u64], black_and_white: bool, path: &std::path::Path) -> Result<usize, String> {
        let mut sheets: Vec<SheetDef> = ids.iter().filter_map(|i| self.document.sheet(*i).cloned()).collect();
        if sheets.is_empty() { return Err("no sheet is chosen".into()); }
        sheets.sort_by_key(|a| a.number.to_lowercase());
        let mut pdf = Pdf::new(&format!("{} sheets", sheets.len()));
        for s in &sheets { pdf.add(sheet_draw::sheet_page(&self.document, s, black_and_white)); }
        let n = pdf.pages();
        std::fs::write(path, pdf.finish()).map_err(|e| format!("could not write {}: {e}", path.display()))?;
        self.message = format!("{n} sheet(s) written to {}", path.display());
        Ok(n)
    }

    // -----------------------------------------------------------------------------------------------------------------
    // windows

    pub(crate) fn new_sheet_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.sheets.new.take() else { return };
        let (mut open, mut ok, mut cancel) = (true, false, false);
        egui::Window::new("New Sheet").id(egui::Id::new("new_sheet_window")).open(&mut open).collapsible(false).default_width(320.0).show(ctx, |ui| {
            egui::Grid::new("new_sheet_grid").num_columns(2).show(ui, |ui| {
                ui.label("Title block");
                egui::ComboBox::from_id_salt("new_sheet_size").selected_text({ let (w, h) = d.size.dims_mm(d.portrait); format!("{} {w:.0} x {h:.0} mm", d.size.name()) }).show_ui(ui, |ui| {
                    for s in PaperSize::ALL { let (w, h) = s.dims_mm(d.portrait); ui.selectable_value(&mut d.size, s, format!("{} {w:.0} x {h:.0} mm", s.name())); }
                });
                ui.end_row();
                ui.label("Orientation");
                ui.horizontal(|ui| { ui.selectable_value(&mut d.portrait, false, "Landscape"); ui.selectable_value(&mut d.portrait, true, "Portrait"); });
                ui.end_row();
                ui.label("Sheet number");
                ui.text_edit_singleline(&mut d.number);
                ui.end_row();
                ui.label("Sheet name");
                ui.text_edit_singleline(&mut d.name);
                ui.end_row();
            });
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() { ok = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
            });
        });
        let mut keep = open && !cancel;
        if ok && self.confirm_new_sheet(&d) { keep = false; }
        if keep { self.sheets.new = Some(d); }
    }

    pub(crate) fn sheet_props_window(&mut self, ctx: &egui::Context) {
        let Some(mut p) = self.sheets.props.take() else { return };
        let (mut open, mut ok, mut apply, mut cancel) = (true, false, false, false);
        egui::Window::new(format!("Sheet Properties - {}", p.draft.label())).id(egui::Id::new("sheet_props_window")).open(&mut open).default_width(380.0).show(ctx, |ui| {
            egui::Grid::new("sheet_props_grid").num_columns(2).show(ui, |ui| {
                ui.label("Sheet number");
                ui.text_edit_singleline(&mut p.draft.number);
                ui.end_row();
                ui.label("Sheet name");
                ui.text_edit_singleline(&mut p.draft.name);
                ui.end_row();
                ui.label("Paper");
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("sheet_props_size").selected_text(p.draft.size.name()).width(70.0).show_ui(ui, |ui| { for s in PaperSize::ALL { ui.selectable_value(&mut p.draft.size, s, s.name()); } });
                    ui.selectable_value(&mut p.draft.portrait, false, "Landscape");
                    ui.selectable_value(&mut p.draft.portrait, true, "Portrait");
                });
                ui.end_row();
                for (label, value) in [("Project name", &mut p.draft.block.project), ("Client name", &mut p.draft.block.client), ("Sheet issue date", &mut p.draft.block.date), ("Drawn by", &mut p.draft.block.drawn_by), ("Checked by", &mut p.draft.block.checked_by)] {
                    ui.label(label);
                    ui.text_edit_singleline(value);
                    ui.end_row();
                }
            });
            ui.weak("The project and client names are copied to the next sheet that is made.");
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() { ok = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
                if ui.button("Apply").clicked() { apply = true; }
            });
        });
        let mut keep = open && !cancel;
        if ok || apply {
            let ops = self.document.set_sheet_commands(&p.draft);
            let same = matches!(&ops, Err(e) if e.contains("nothing changes"));
            if same { if ok { keep = false; } }
            else if self.apply_sheet(ops, "Sheet properties changed") && ok { keep = false; }
        }
        if keep { self.sheets.props = Some(p); }
    }

    pub(crate) fn add_view_window(&mut self, ctx: &egui::Context) {
        let Some(mut a) = self.sheets.add.take() else { return };
        let (mut open, mut add, mut cancel) = (true, false, false);
        let sheets: Vec<(u64, String)> = self.document.sheets().iter().map(|s| (s.id, s.label())).collect();
        let views = self.placeable_views();
        egui::Window::new("Place View").id(egui::Id::new("add_view_window")).open(&mut open).collapsible(false).default_width(340.0).show(ctx, |ui| {
            egui::Grid::new("add_view_grid").num_columns(2).show(ui, |ui| {
                ui.label("Sheet");
                egui::ComboBox::from_id_salt("add_view_sheet").selected_text(a.sheet.and_then(|id| sheets.iter().find(|s| s.0 == id)).map_or("choose a sheet".to_string(), |s| s.1.clone())).show_ui(ui, |ui| {
                    for (id, label) in &sheets { ui.selectable_value(&mut a.sheet, Some(*id), label); }
                });
                ui.end_row();
                ui.label("View");
                let shown = a.view.as_ref().map_or("choose a view".to_string(), |k| views.iter().find(|v| &v.0 == k).map_or_else(|| format!("{} (already on a sheet)", sheet_draw::view_title(&self.document, k)), |v| v.1.clone()));
                egui::ComboBox::from_id_salt("add_view_view").selected_text(shown).width(220.0).show_ui(ui, |ui| {
                    for (k, n) in &views { ui.selectable_value(&mut a.view, Some(k.clone()), n); }
                });
                ui.end_row();
            });
            if views.is_empty() { ui.weak("Every view is on a sheet already."); }
            ui.weak("A view is on one sheet only. It goes on at the scale set in its View Control Bar.");
            ui.horizontal(|ui| {
                if ui.add_enabled(a.sheet.is_some() && a.view.is_some(), egui::Button::new("Add View to Sheet")).clicked() { add = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
            });
        });
        let mut keep = open && !cancel;
        if add {
            if let (Some(s), Some(k)) = (a.sheet, a.view.clone()) {
                if self.add_view_to_sheet(s, &k) { keep = false; self.view = View::Sheet(s); }
            }
        }
        if keep { self.sheets.add = Some(a); }
    }

    pub(crate) fn print_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.sheets.print.take() else { return };
        let (mut open, mut go, mut cancel) = (true, false, false);
        let labels: BTreeMap<u64, String> = self.document.sheets().iter().map(|s| (s.id, s.label())).collect();
        egui::Window::new("Print").id(egui::Id::new("print_window")).open(&mut open).collapsible(false).default_width(340.0).show(ctx, |ui| {
            ui.label("Print to PDF, one page for each sheet, at the size of its paper:");
            egui::ScrollArea::vertical().id_salt("print_sheets").max_height(220.0).show(ui, |ui| {
                for (id, on) in d.sheets.iter_mut() { ui.checkbox(on, labels.get(id).cloned().unwrap_or_default()); }
            });
            ui.horizontal(|ui| {
                if ui.small_button("Check All").clicked() { for s in d.sheets.iter_mut() { s.1 = true; } }
                if ui.small_button("Check None").clicked() { for s in d.sheets.iter_mut() { s.1 = false; } }
            });
            ui.horizontal(|ui| {
                ui.label("Color");
                ui.selectable_value(&mut d.black_and_white, false, "Color");
                ui.selectable_value(&mut d.black_and_white, true, "Black Lines");
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(d.sheets.iter().any(|s| s.1), egui::Button::new("Print…")).clicked() { go = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
            });
        });
        let mut keep = open && !cancel;
        if go {
            let ids: Vec<u64> = d.sheets.iter().filter(|s| s.1).map(|s| s.0).collect();
            let name = if let [one] = ids.as_slice() { self.document.sheet(*one).map_or("sheets".to_string(), |s| s.label()) } else { "Sheets".into() };
            let fd = rfd::FileDialog::new().set_title("Print to PDF").add_filter("PDF", &["pdf"]).set_file_name(format!("{name}.pdf"));
            if let Some(path) = fd.save_file() {
                if let Err(e) = self.print_sheets(&ids, d.black_and_white, &path) { self.message = format!("Print failed: {e}"); } else { keep = false; }
            }
        }
        if keep { self.sheets.print = Some(d); }
    }

    // -----------------------------------------------------------------------------------------------------------------
    // the sheet view

    pub(crate) fn show_sheet(&mut self, ui: &mut egui::Ui, id: u64) {
        let Some(def) = self.document.sheet(id).cloned() else {
            ui.label("This sheet is gone (it was deleted or the change undone). Open another view from the Project Browser.");
            return;
        };
        let drawings = self.sheet_drawings(id);
        let (w, h) = def.dims_mm();
        ui.horizontal_wrapped(|ui| {
            ui.strong(def.label());
            ui.weak(format!("{} {w:.0} x {h:.0} mm, {} view(s)", def.size.name(), def.viewports.len()));
            if ui.button("Place View…").clicked() { self.open_add_view(Some(id), None); }
            if ui.button("Sheet Properties…").clicked() { self.open_sheet_properties(id); }
            if ui.button("Print…").clicked() { self.open_print(); }
            let sel = self.sheets.selected.filter(|s| def.viewport(*s).is_some());
            if ui.add_enabled(sel.is_some(), egui::Button::new("Activate View")).on_hover_text("Open the view of the selected viewport").clicked() { if let Some(s) = sel { self.open_viewport_view(id, s); } }
            if ui.add_enabled(sel.is_some(), egui::Button::new("Remove From Sheet")).clicked() { self.remove_selected_viewport(id); }
            if ui.small_button("Fit").clicked() { self.sheets.pan = Vec2::ZERO; self.sheets.zoom = Some(1.0); }
        });
        // the selected viewport: its title, and whether it shows
        if let Some(vp) = self.sheets.selected.and_then(|s| def.viewport(s)).cloned() {
            if self.sheets.title_edit.as_ref().is_none_or(|t| t.0 != vp.id) { self.sheets.title_edit = Some((vp.id, vp.title.clone())); }
            ui.horizontal(|ui| {
                ui.label(format!("View {}: {}", vp.id, sheet_draw::view_title(&self.document, &vp.view)));
                let mut show = vp.show_title;
                let schedule = openbim_document::schedule_ref(&vp.view).is_some();
                if !schedule && ui.checkbox(&mut show, "Show Title").changed() {
                    let mut d = def.clone();
                    if let Some(v) = d.viewports.iter_mut().find(|v| v.id == vp.id) { v.show_title = show; }
                    let ops = self.document.set_sheet_commands(&d);
                    self.apply_sheet(ops, "Title changed");
                }
                if !schedule {
                    ui.label("Title");
                    if let Some((_, text)) = self.sheets.title_edit.as_mut() {
                        let r = ui.add(egui::TextEdit::singleline(text).desired_width(200.0).hint_text("the name of the view"));
                        if r.lost_focus() && *text != vp.title {
                            let t = text.clone();
                            let mut d = def.clone();
                            if let Some(v) = d.viewports.iter_mut().find(|v| v.id == vp.id) { v.title = t; }
                            let ops = self.document.set_sheet_commands(&d);
                            self.apply_sheet(ops, "Title changed");
                        }
                    }
                }
                if let Some(d) = drawings.get(&vp.id).and_then(|d| d.as_ref().ok()) { ui.weak(format!("{:.0} x {:.0} mm on the paper", d.w, d.h)); }
            });
        }
        let (response, painter) = ui.allocate_painter(ui.available_size().max(Vec2::splat(50.0)), Sense::click_and_drag());
        let rect = response.rect;
        painter.rect_filled(rect, 0.0, Color32::from_gray(if self.dark { 40 } else { 200 }));
        if response.dragged_by(egui::PointerButton::Middle) { self.sheets.pan += response.drag_delta(); }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            self.sheets.zoom = Some((self.sheets.zoom.unwrap_or(1.0) * (scroll * 0.001).exp()).clamp(0.2, 12.0));
        }
        let (k, origin) = layout(rect, &def, self.sheets.pan, self.sheets.zoom.unwrap_or(1.0));
        let to_paper = |p: Pos2| { let v = (p - origin) / k; [f64::from(v.x), f64::from(v.y)] };
        let moved = self.sheets.drag.as_ref().map(|d| (d.viewport, d.by[0], d.by[1]));
        let (prims, placed) = sheet_draw::compose(&self.document, &def, &drawings, moved);
        painter.rect_filled(Rect::from_min_size(origin + Vec2::new(3.0, 3.0), Vec2::new(w as f32, h as f32) * k), 0.0, Color32::from_black_alpha(60));
        paint(&painter, &prims, k, origin);
        let blue = ui.visuals().hyperlink_color;
        let at = |p: [f64; 2]| origin + Vec2::new(p[0] as f32, p[1] as f32) * k;
        let inside = |pl: &Placed, p: [f64; 2]| p[0] >= pl.rect[0] && p[0] <= pl.rect[2] && p[1] >= pl.rect[1] && p[1] <= pl.rect[3];
        let under = |p: [f64; 2]| placed.iter().rev().find(|pl| inside(pl, p)).map(|pl| pl.id);
        // the viewport that is selected
        if let Some(pl) = self.sheets.selected.and_then(|s| placed.iter().find(|p| p.id == s)) {
            painter.rect_stroke(Rect::from_min_max(at([pl.rect[0], pl.rect[1]]), at([pl.rect[2], pl.rect[3]])).expand(2.0), 0.0, Stroke::new(1.5, blue), egui::StrokeKind::Middle);
        }
        // the views that stand past the border
        let [al, at_, ar, ab] = [sheet_draw::MARGIN_MM, sheet_draw::MARGIN_MM, w - sheet_draw::MARGIN_MM, h - sheet_draw::MARGIN_MM];
        let outside = placed.iter().filter(|p| p.rect[0] < al - 0.5 || p.rect[1] < at_ - 0.5 || p.rect[2] > ar + 0.5 || p.rect[3] > ab + 0.5).count();
        if outside > 0 { painter.text(rect.left_bottom() + Vec2::new(8.0, -8.0), egui::Align2::LEFT_BOTTOM, format!("{outside} view(s) reach past the border"), egui::FontId::proportional(13.0), Color32::from_rgb(200, 90, 0)); }
        // press on a view and drag: it moves on the paper (Esc takes it back)
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                let paper = to_paper(p);
                if let Some(vp) = under(paper) { self.sheets.selected = Some(vp); self.sheets.drag = Some(Drag { viewport: vp, from: paper, by: [0.0, 0.0] }); }
            }
        }
        if let Some(d) = self.sheets.drag.as_mut() {
            if let Some(p) = response.interact_pointer_pos().or(response.hover_pos()) {
                let paper = to_paper(p);
                d.by = [((paper[0] - d.from[0]) * 2.0).round() / 2.0, ((paper[1] - d.from[1]) * 2.0).round() / 2.0];
            }
        }
        if self.sheets.drag.is_some() {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { self.sheets.drag = None; self.message = "Move cancelled".into(); }
            else if response.drag_stopped_by(egui::PointerButton::Primary) {
                if let Some(d) = self.sheets.drag.take() {
                    if d.by != [0.0, 0.0] {
                        let mut s = def.clone();
                        if let Some(v) = s.viewports.iter_mut().find(|v| v.id == d.viewport) { v.x_mm += d.by[0]; v.y_mm += d.by[1]; }
                        let ops = self.document.set_sheet_commands(&s);
                        self.apply_sheet(ops, "View moved on the sheet");
                    }
                }
            }
        }
        if !ui.input(|i| i.pointer.primary_down()) && self.sheets.drag.is_some() && !response.dragged() { self.sheets.drag = None; }
        if response.clicked() {
            if let Some(p) = response.interact_pointer_pos() { self.sheets.selected = under(to_paper(p)); }
        }
        if response.double_clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                let paper = to_paper(p);
                match under(paper) {
                    Some(vp) => self.open_viewport_view(id, vp),
                    None => { let tb = sheet_draw::title_block_box(&def); if paper[0] >= tb[0] && paper[0] <= tb[0] + tb[2] && paper[1] >= tb[1] && paper[1] <= tb[1] + tb[3] { self.open_sheet_properties(id); } }
                }
            }
        }
        if !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) { self.remove_selected_viewport(id); }
    }

    /// The Sheets folder of the Project Browser.
    pub(crate) fn sheet_browser_items(&mut self, ui: &mut egui::Ui, query: &str) {
        let list: Vec<(u64, String)> = self.document.sheets().iter().map(|s| (s.id, s.label())).collect();
        if list.is_empty() { ui.label(egui::RichText::new("No sheets yet: View > Sheet").weak().italics()); return; }
        for (id, label) in list {
            if !query.is_empty() && !label.to_lowercase().contains(query) { continue; }
            let active = self.view == View::Sheet(id);
            let r = ui.selectable_label(active, egui::RichText::new(&label)).on_hover_text("Double-click to open the sheet");
            if r.double_clicked() { self.view = View::Sheet(id); self.selected = None; }
            r.context_menu(|ui| {
                if ui.button("Open").clicked() { self.view = View::Sheet(id); self.selected = None; ui.close(); }
                if ui.button("Place View…").clicked() { self.open_add_view(Some(id), None); ui.close(); }
                if ui.button("Properties…").clicked() { self.open_sheet_properties(id); ui.close(); }
                if ui.button("Print…").clicked() { self.view = View::Sheet(id); self.open_print(); ui.close(); }
                if ui.button("Delete").clicked() { self.delete_sheet(id); ui.close(); }
            });
        }
    }

    /// "Add to Sheet…" for a view of the browser, by its key.
    pub(crate) fn add_to_sheet_menu(&mut self, ui: &mut egui::Ui, key: Option<String>) {
        let Some(key) = key else { return };
        if let Some(on) = self.document.sheet_of_view(&key) {
            ui.add_enabled(false, egui::Button::new(format!("On sheet {}", on.label())));
        } else if ui.button("Add to Sheet…").clicked() {
            self.open_add_view(None, Some(key));
            ui.close();
        }
    }
}

#[allow(dead_code)]
fn element_id(_: ElementId) {}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Pos2, Rect, Vec2};

    fn frames(app: &mut Desktop, n: usize, f: &dyn Fn(&mut Desktop, &mut egui::Ui)) {
        let ctx = egui::Context::default();
        for _ in 0..n {
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1300.0, 900.0))), ..Default::default() }, |ui| f(app, ui));
            out.textures_delta.clear();
        }
    }

    #[test]
    fn views_go_on_a_sheet_once_move_come_off_and_the_whole_is_one_undo_each() {
        let mut app = Desktop::sample();
        app.run_command("sheet", "Sheet");
        assert!(app.sheets.new.is_some());
        let d = app.sheets.new.take().unwrap();
        assert_eq!(d.number, "A101");
        assert!(app.confirm_new_sheet(&d), "{}", app.message);
        let id = app.document.sheets()[0].id;
        assert_eq!(app.view, View::Sheet(id));
        frames(&mut app, 2, &|app, ui| app.show_sheet(ui, id));
        // a plan, the 3D view and a schedule
        let steps = app.document.history_mark();
        assert!(app.add_view_to_sheet(id, "p10"), "{}", app.message);
        assert_eq!(app.document.history_mark(), steps + 1);
        assert!(!app.add_view_to_sheet(id, "p10"), "a view is on one sheet only");
        assert!(app.message.contains("one sheet only"));
        assert!(app.add_view_to_sheet(id, "3d"), "{}", app.message);
        let (ops, sc) = app.document.new_schedule_commands(openbim_document::ScheduleKind::Walls, "").unwrap();
        app.document.transact(app.document.revision(), &ops).unwrap();
        assert!(app.add_view_to_sheet(id, &openbim_document::schedule_view(sc)), "{}", app.message);
        assert_eq!(app.document.sheet(id).unwrap().viewports.len(), 3);
        assert!(!app.placeable_views().iter().any(|v| v.0 == "p10" || v.0 == "3d"), "what is on a sheet is not offered again");
        // the sheet draws, with the views and a viewport selected
        app.sheets.selected = Some(1);
        frames(&mut app, 2, &|app, ui| app.show_sheet(ui, id));
        // the viewports are on the paper, none of them on another
        let def = app.document.sheet(id).unwrap().clone();
        let drawings = app.sheet_drawings(id);
        let (prims, placed) = sheet_draw::compose(&app.document, &def, &drawings, None);
        assert!(!prims.is_empty());
        assert_eq!(placed.len(), 3);
        for p in &placed { assert!(p.rect[2] > p.rect[0] && p.rect[3] > p.rect[1], "{p:?}"); }
        // moving a viewport is one undo step and keeps the drawings
        let before = app.document.sheet(id).unwrap().viewports[0].x_mm;
        let mut moved = def.clone();
        moved.viewports[0].x_mm += 25.0;
        let steps = app.document.history_mark();
        let ops = app.document.set_sheet_commands(&moved);
        assert!(app.apply_sheet(ops, "moved"));
        assert_eq!(app.document.history_mark(), steps + 1);
        assert_eq!(app.document.sheet(id).unwrap().viewports[0].x_mm, before + 25.0);
        assert!(app.sheets.cache.as_ref().is_some_and(|c| c.revision == app.document.revision()), "the drawings are kept");
        // taking one off
        app.sheets.selected = Some(2);
        app.remove_selected_viewport(id);
        assert_eq!(app.document.sheet(id).unwrap().viewports.len(), 2);
        assert!(app.placeable_views().iter().any(|v| v.0 == "3d"), "and it can be put on again");
        // activate the view of a viewport
        app.open_viewport_view(id, 1);
        assert_eq!(app.view, View::Plan);
        // delete the sheet
        app.delete_sheet(id);
        assert!(app.document.sheets().is_empty());
        app.view = View::Sheet(id);
        frames(&mut app, 1, &|app, ui| app.show_sheet(ui, id));
    }

    #[test]
    fn the_windows_draw_and_properties_change_the_title_block_in_one_step() {
        let mut app = Desktop::sample();
        app.open_new_sheet();
        frames(&mut app, 2, &|app, ui| { let c = ui.ctx().clone(); app.new_sheet_window(&c); });
        assert!(app.sheets.new.is_some());
        let d = app.sheets.new.take().unwrap();
        assert!(app.confirm_new_sheet(&d));
        let id = app.document.sheets()[0].id;
        app.open_sheet_properties(id);
        frames(&mut app, 2, &|app, ui| { let c = ui.ctx().clone(); app.sheet_props_window(&c); });
        app.sheets.props.as_mut().unwrap().draft.block.project = "Villa Sokha".into();
        let steps = app.document.history_mark();
        let ops = app.document.set_sheet_commands(&app.sheets.props.as_ref().unwrap().draft);
        assert!(app.apply_sheet(ops, "x"));
        assert_eq!(app.document.history_mark(), steps + 1);
        assert_eq!(app.document.sheet(id).unwrap().block.project, "Villa Sokha");
        // Place View: with one sheet it is chosen, the view comes from the current view
        app.run_command("add_view", "Place View");
        assert_eq!(app.sheets.add.as_ref().and_then(|a| a.sheet), Some(id));
        frames(&mut app, 2, &|app, ui| { let c = ui.ctx().clone(); app.add_view_window(&c); });
        app.sheets.add = None;
        // a number in use is refused with a message
        app.open_new_sheet();
        let mut d = app.sheets.new.take().unwrap();
        d.number = "a101".into();
        assert!(!app.confirm_new_sheet(&d));
        assert!(app.message.contains("numbered"), "{}", app.message);
        // the browser lists the sheet and offers Add to Sheet on a view
        frames(&mut app, 2, &|app, ui| { app.sheet_browser_items(ui, ""); app.add_to_sheet_menu(ui, Some("p10".into())); });
        // print
        app.open_print();
        frames(&mut app, 2, &|app, ui| { let c = ui.ctx().clone(); app.print_window(&c); });
        assert_eq!(app.sheets.print.as_ref().map(|p| p.sheets.len()), Some(1));
    }

    #[test]
    fn the_sheets_print_to_a_pdf_page_each_in_the_order_of_their_numbers() {
        let mut app = Desktop::sample();
        for (size, number, name) in [(PaperSize::A1, "B200", "Second"), (PaperSize::A3, "A100", "First")] {
            let (ops, id) = app.document.new_sheet_commands(size, false, number, name).unwrap();
            app.document.transact(app.document.revision(), &ops).unwrap();
            assert!(app.add_view_to_sheet(id, if number == "A100" { "p10" } else { "mech" }), "{}", app.message);
        }
        let ids: Vec<u64> = app.document.sheets().iter().map(|s| s.id).collect();
        let dir = std::env::temp_dir().join(format!("openbim-print-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sheets.pdf");
        assert_eq!(app.print_sheets(&ids, false, &path), Ok(2));
        let bytes = std::fs::read(&path).unwrap();
        let text: String = bytes.iter().map(|&b| if b < 128 { b as char } else { '?' }).collect();
        assert!(text.starts_with("%PDF-1.4") && text.contains("/Count 2"));
        // A100 (A3, 420 x 297 mm) comes first, B200 (A1) second
        let a3 = text.find("/MediaBox [0 0 1190.551 841.89]").expect("an A3 page");
        let a1 = text.find("/MediaBox [0 0 2383.937 1683.78]").expect("an A1 page");
        assert!(a3 < a1);
        assert_eq!(app.print_sheets(&[], false, &path), Err("no sheet is chosen".into()));
        assert!(app.print_sheets(&ids, true, &dir.join("no such folder").join("x.pdf")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
