//! Schedules/Quantities, as Revit's Schedule views: New Schedule/Quantities (a category and a name), the Schedule
//! Properties window (Fields, Filter, Sorting/Grouping, Formatting), the schedule view itself with a row for each
//! element (or each kind of element), headers, footers and grand totals that follow the model as it changes, a click on
//! a row selecting its elements, and export to CSV. The definitions are saved with the model.
use crate::{Desktop, View};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use openbim_document::{
    field_info, schedule_fields, Command, ElementId, FieldType, FilterOp, RowKind, ScheduleDef, ScheduleField, ScheduleFilter, ScheduleKind, ScheduleSort, ScheduleTable,
    ScheduleValue, MAX_FILTERS, MAX_SORTS,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab { Fields, Filter, Sorting, Formatting }

/// New Schedule/Quantities: the category and the name.
pub(crate) struct NewSchedule {
    pub kind: ScheduleKind,
    pub name: String,
}

/// Schedule Properties: the definition being edited and what is selected in its lists.
pub(crate) struct Props {
    pub draft: ScheduleDef,
    pub tab: Tab,
    pub available: Option<&'static str>,
    pub scheduled: Option<usize>,
    pub formatting: Option<usize>,
}

/// The table of the schedule on show, kept until the model or the definition changes.
struct Cache {
    revision: u64,
    def: ScheduleDef,
    table: ScheduleTable,
}

#[derive(Default)]
pub(crate) struct ScheduleUi {
    pub new: Option<NewSchedule>,
    pub props: Option<Props>,
    pub rename: Option<(u64, String)>,
    cache: Option<Cache>,
}

/// The plan view that shows the level `level`.
pub(crate) fn view_of_level(level: ElementId) -> View {
    match level.0 { 10 => View::Plan, 11 => View::UpperPlan, id => View::Level(id) }
}

impl Desktop {
    /// The Schedules/Quantities button.
    pub(crate) fn open_new_schedule(&mut self) {
        self.schedules.new = Some(NewSchedule { kind: ScheduleKind::Walls, name: String::new() });
    }

    pub(crate) fn open_schedule_properties(&mut self, id: u64, tab: Tab) {
        match self.document.schedule(id) {
            Some(def) => self.schedules.props = Some(Props { draft: def.clone(), tab, available: None, scheduled: None, formatting: None }),
            None => self.message = "That schedule is gone".into(),
        }
    }

    fn apply_schedule(&mut self, ops: Result<Vec<Command>, String>, done: &str) -> bool {
        match ops {
            Ok(ops) => match self.document.transact(self.document.revision(), &ops) {
                Ok(_) => { self.message = format!("{done}; undo takes it back"); true }
                Err(e) => { self.message = format!("Schedule rejected ({e:?})"); false }
            },
            Err(e) => { self.message = format!("Schedule: {e}"); false }
        }
    }

    pub(crate) fn new_schedule_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.schedules.new.take() else { return };
        let (mut open, mut ok, mut cancel) = (true, false, false);
        let hint = self.document.default_schedule_name(d.kind);
        egui::Window::new("New Schedule/Quantities").id(egui::Id::unique("new_schedule_window")).open(&mut open).default_width(340.0).show(ctx, |ui| {
            ui.label("Category");
            egui::ScrollArea::vertical().id_salt("new_schedule_kinds").max_height(400.0).show(ui, |ui| {
                for k in ScheduleKind::ALL {
                    let r = ui.selectable_label(d.kind == k, k.name());
                    test_hit!(self, r, "schedule_new:{}", k.name());
                    if r.clicked() { d.kind = k; }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut d.name).hint_text(hint.as_str()).desired_width(220.0));
            });
            ui.horizontal(|ui| {
                let r = ui.button("OK");
                test_hit!(self, r, "schedule_new:ok");
                if r.clicked() { ok = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
            });
        });
        let mut keep = open && !cancel;
        if ok && self.confirm_new_schedule(&d) { keep = false; }
        if keep { self.schedules.new = Some(d); }
    }

    /// OK of New Schedule/Quantities: the schedule is made, shown, and its properties open on the Fields tab (as in Revit).
    pub(crate) fn confirm_new_schedule(&mut self, d: &NewSchedule) -> bool {
        match self.document.new_schedule_commands(d.kind, &d.name) {
            Ok((ops, id)) => {
                let name = match &ops[0] { Command::SetMep(m) => m.schedules.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default(), _ => String::new() };
                if !self.apply_schedule(Ok(ops), &format!("Schedule \"{name}\" made")) { return false; }
                self.view = View::Schedule(id);
                self.selected = None;
                self.open_schedule_properties(id, Tab::Fields);
                true
            }
            Err(e) => { self.message = format!("Schedule: {e}"); false }
        }
    }

    pub(crate) fn rename_schedule_window(&mut self, ctx: &egui::Context) {
        let Some((id, mut name)) = self.schedules.rename.take() else { return };
        let (mut open, mut ok, mut cancel) = (true, false, false);
        egui::Window::new("Rename Schedule").id(egui::Id::unique("rename_schedule_window")).open(&mut open).show(ctx, |ui| {
            ui.add(egui::TextEdit::singleline(&mut name).desired_width(240.0));
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() { ok = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
            });
        });
        let mut keep = open && !cancel;
        if ok {
            match self.document.schedule(id).cloned() {
                Some(mut def) => {
                    def.name = name.trim().to_string();
                    let ops = self.document.set_schedule_commands(&def);
                    if self.apply_schedule(ops, &format!("Schedule renamed \"{}\"", def.name)) { keep = false; }
                }
                None => keep = false,
            }
        }
        if keep { self.schedules.rename = Some((id, name)); }
    }

    pub(crate) fn duplicate_schedule(&mut self, id: u64) {
        match self.document.duplicate_schedule_commands(id) {
            Ok((ops, new_id)) => {
                if self.apply_schedule(Ok(ops), "Schedule duplicated") { self.view = View::Schedule(new_id); }
            }
            Err(e) => self.message = format!("Schedule: {e}"),
        }
    }

    pub(crate) fn delete_schedule(&mut self, id: u64) {
        let name = self.document.schedule(id).map(|s| s.name.clone()).unwrap_or_default();
        let ops = self.document.delete_schedule_commands(id);
        if self.apply_schedule(ops, &format!("Schedule \"{name}\" deleted")) && self.view == View::Schedule(id) { self.view = View::Plan; }
    }

    /// Writes the schedule as CSV to `path`.
    pub(crate) fn export_schedule_csv(&mut self, id: u64, path: &std::path::Path) {
        let Some(def) = self.document.schedule(id) else { return };
        let csv = self.document.schedule_table(def).to_csv();
        self.message = match std::fs::write(path, csv) {
            Ok(()) => format!("Schedule \"{}\" written to {}", def.name, path.display()),
            Err(e) => format!("Could not write {}: {e}", path.display()),
        };
    }

    /// The Schedule Properties window.
    pub(crate) fn schedule_props_window(&mut self, ctx: &egui::Context) {
        let Some(mut p) = self.schedules.props.take() else { return };
        let kind = p.draft.kind;
        let (mut open, mut ok, mut apply, mut cancel) = (true, false, false, false);
        egui::Window::new(format!("Schedule Properties - {}", p.draft.name)).id(egui::Id::unique("schedule_props_window")).open(&mut open).default_width(560.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (t, n) in [(Tab::Fields, "Fields"), (Tab::Filter, "Filter"), (Tab::Sorting, "Sorting/Grouping"), (Tab::Formatting, "Formatting")] { ui.selectable_value(&mut p.tab, t, n); }
            });
            ui.separator();
            match p.tab {
                Tab::Fields => fields_tab(ui, &mut p, kind),
                Tab::Filter => filter_tab(ui, &mut p, kind),
                Tab::Sorting => sorting_tab(ui, &mut p, kind),
                Tab::Formatting => formatting_tab(ui, &mut p, kind),
            }
            ui.separator();
            match p.draft.check() {
                Ok(()) => { ui.weak(format!("{} fields, {} filters", p.draft.fields.len(), p.draft.filters.len())); }
                Err(e) => { ui.colored_label(ui.visuals().warn_fg_color, e); }
            }
            ui.horizontal(|ui| {
                let r = ui.button("OK"); test_hit!(self, r, "schedule_props:ok"); if r.clicked() { ok = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
                if ui.button("Apply").clicked() { apply = true; }
            });
        });
        let mut keep = open && !cancel;
        if ok || apply {
            let before = self.document.schedule(p.draft.id).cloned();
            if before.as_ref() == Some(&p.draft) {
                if ok { keep = false; }
            } else {
                let ops = self.document.set_schedule_commands(&p.draft);
                if self.apply_schedule(ops, &format!("Schedule \"{}\" changed", p.draft.name)) && ok { keep = false; }
            }
        }
        if keep { self.schedules.props = Some(p); }
    }

    /// The schedule view: its toolbar and its table.
    pub(crate) fn show_schedule(&mut self, ui: &mut egui::Ui, id: u64) {
        let Some(def) = self.document.schedule(id).cloned() else {
            self.view = View::Plan;
            self.message = "The schedule is gone: back to the plan".into();
            return;
        };
        let revision = self.document.revision();
        if !self.schedules.cache.as_ref().is_some_and(|c| c.revision == revision && c.def == def) {
            let table = self.document.schedule_table(&def);
            self.schedules.cache = Some(Cache { revision, def: def.clone(), table });
        }
        let Some(cache) = self.schedules.cache.take() else { return };
        let table = &cache.table;
        ui.horizontal_wrapped(|ui| {
            ui.heading(&def.name);
            ui.label(egui::RichText::new(def.kind.name()).weak());
            ui.separator();
            if ui.button("Properties…").on_hover_text("Fields, Filter, Sorting/Grouping, Formatting").clicked() { self.open_schedule_properties(id, Tab::Fields); }
            let mut itemize = def.itemize;
            if ui.checkbox(&mut itemize, "Itemize every instance").clicked() {
                let mut d = def.clone();
                d.itemize = itemize;
                let ops = self.document.set_schedule_commands(&d);
                self.apply_schedule(ops, "Schedule changed");
            }
            if ui.button("Show in model").on_hover_text("Open the plan of the level of the selected row").clicked() { self.show_schedule_selection_in_model(); }
            if ui.button("Export CSV…").clicked() {
                let mut fd = crate::dialog::Question::new().set_title("Export schedule").add_filter("CSV", &["csv"]);
                fd = fd.set_file_name(format!("{}.csv", def.name));
                if let Some(p) = fd.ask_save() { self.export_schedule_csv(id, &p); }
            }
            if ui.button("Duplicate").clicked() { self.duplicate_schedule(id); }
            if ui.button("Rename…").clicked() { self.schedules.rename = Some((id, def.name.clone())); }
            if ui.button("Delete").clicked() { self.delete_schedule(id); }
        });
        let listed: usize = table.rows.iter().filter(|r| r.kind == RowKind::Data).map(|r| r.ids.len().max(1)).sum();
        ui.weak(format!("{} line(s), {} element(s) listed; the table follows the model as it changes", table.rows.iter().filter(|r| r.kind == RowKind::Data).count(), listed));
        ui.separator();
        let selected = self.selected_ids();
        let mut clicked: Option<Vec<ElementId>> = None;
        let cols = table.headings.len();
        let mut tb = TableBuilder::new(ui).striped(true).resizable(true).sense(egui::Sense::click()).vscroll(true);
        for i in 0..cols { tb = tb.column(if i == 0 { Column::auto().at_least(80.0).resizable(true) } else { Column::auto().at_least(60.0).resizable(true) }); }
        tb.header(22.0, |mut header| {
            for h in &table.headings { header.col(|ui| { ui.strong(h); }); }
        }).body(|body| {
            body.rows(20.0, table.rows.len(), |mut row| {
                let r = &table.rows[row.index()];
                let on = r.kind == RowKind::Data && !r.ids.is_empty() && r.ids.iter().all(|i| selected.contains(i));
                row.set_selected(on);
                // the title of a header, footer or grand total goes in the first column with nothing of its own
                let title_col = if r.kind == RowKind::Data || r.kind == RowKind::Blank { None } else { r.cells.iter().position(|c| matches!(c, ScheduleValue::Blank)) };
                for c in 0..r.cells.len() {
                    row.col(|ui| {
                        let text = if title_col == Some(c) { r.label.clone() } else { table.text(r, c) };
                        let numeric = table.types[c].numeric() && table.types[c] != FieldType::Id;
                        let t = if r.kind == RowKind::Data { egui::RichText::new(text) } else { egui::RichText::new(text).strong() };
                        if numeric && title_col != Some(c) { ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(t); }); } else { ui.label(t); }
                    });
                }
                if row.response().clicked() && r.kind == RowKind::Data && !r.ids.is_empty() { clicked = Some(r.ids.clone()); }
            });
        });
        self.schedules.cache = Some(cache);
        if let Some(ids) = clicked {
            let n = ids.len();
            self.select_many(ids);
            self.message = format!("{n} element(s) selected from the schedule; Show in model opens their plan");
        }
    }

    /// Show in model: the plan of the level of the first element selected.
    fn show_schedule_selection_in_model(&mut self) {
        let Some(first) = self.selected_ids().into_iter().next() else { self.message = "Select a row of the schedule first".into(); return };
        match self.document.level_of(first) {
            Some(level) => { self.view = view_of_level(level); self.message = format!("{} shown in its plan", self.document.get(first).map_or("", |e| e.name.as_str())); }
            None => self.message = "That element is not on a level".into(),
        }
    }

    /// The schedules in the Project Browser: each opens on a double click; the menu has Properties, Rename, Duplicate and Delete.
    pub(crate) fn schedule_browser_items(&mut self, ui: &mut egui::Ui, query: &str) {
        let list: Vec<(u64, String, ScheduleKind)> = self.document.mep().schedules.iter().map(|s| (s.id, s.name.clone(), s.kind)).collect();
        if list.is_empty() { ui.label(egui::RichText::new("No schedules yet: View > Schedules/Quantities").weak().italics()); return; }
        for (id, name, kind) in list {
            if !query.is_empty() && !name.to_lowercase().contains(query) && !kind.name().to_lowercase().contains(query) { continue; }
            let active = self.view == View::Schedule(id);
            let r = ui.selectable_label(active, egui::RichText::new(&name)).on_hover_text(format!("{} schedule: double-click to open", kind.name()));
            if r.double_clicked() { self.view = View::Schedule(id); self.selected = None; }
            r.context_menu(|ui| {
                if ui.button("Open").clicked() { self.view = View::Schedule(id); self.selected = None; ui.close(); }
                if ui.button("Properties…").clicked() { self.open_schedule_properties(id, Tab::Fields); ui.close(); }
                if ui.button("Rename…").clicked() { self.schedules.rename = Some((id, name.clone())); ui.close(); }
                if ui.button("Duplicate").clicked() { self.duplicate_schedule(id); ui.close(); }
                if ui.button("Delete").clicked() { self.delete_schedule(id); ui.close(); }
            });
        }
    }
}

fn fields_tab(ui: &mut egui::Ui, p: &mut Props, kind: ScheduleKind) {
    let all = schedule_fields(kind);
    let free: Vec<_> = all.iter().filter(|f| !p.draft.fields.iter().any(|x| x.key == f.key)).collect();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label("Available fields");
            egui::ScrollArea::vertical().id_salt("sched_avail").max_height(240.0).min_scrolled_width(190.0).show(ui, |ui| {
                for f in &free {
                    let r = ui.selectable_label(p.available == Some(f.key), f.label);
                    if r.clicked() { p.available = Some(f.key); }
                    if r.double_clicked() { p.draft.fields.push(ScheduleField::new(f.key)); p.available = None; }
                }
            });
        });
        ui.vertical(|ui| {
            ui.add_space(40.0);
            let pick = free.iter().find(|f| p.available == Some(f.key)).map(|f| f.key);
            if ui.add_enabled(pick.is_some(), egui::Button::new("Add →")).clicked() {
                if let Some(k) = pick { p.draft.fields.push(ScheduleField::new(k)); p.scheduled = Some(p.draft.fields.len() - 1); p.available = None; }
            }
            if ui.add_enabled(p.scheduled.is_some(), egui::Button::new("← Remove")).clicked() {
                if let Some(i) = p.scheduled.take() {
                    if i < p.draft.fields.len() {
                        let gone = p.draft.fields.remove(i).key;
                        // a filter or a sort on a field that is no longer there goes with it
                        p.draft.filters.retain(|f| f.field != gone);
                        p.draft.sorts.retain(|s| s.field != gone);
                    }
                }
            }
            if ui.add_enabled(p.scheduled.is_some_and(|i| i > 0), egui::Button::new("Move up")).clicked() {
                if let Some(i) = p.scheduled { p.draft.fields.swap(i, i - 1); p.scheduled = Some(i - 1); }
            }
            if ui.add_enabled(p.scheduled.is_some_and(|i| i + 1 < p.draft.fields.len()), egui::Button::new("Move down")).clicked() {
                if let Some(i) = p.scheduled { p.draft.fields.swap(i, i + 1); p.scheduled = Some(i + 1); }
            }
        });
        ui.vertical(|ui| {
            ui.label("Scheduled fields (in order)");
            egui::ScrollArea::vertical().id_salt("sched_fields").max_height(240.0).min_scrolled_width(190.0).show(ui, |ui| {
                for (i, f) in p.draft.fields.iter().enumerate() {
                    let label = field_info(kind, &f.key).map_or(f.key.clone(), |x| x.label.to_string());
                    let label = if f.hidden { format!("{label} (hidden)") } else { label };
                    if ui.selectable_label(p.scheduled == Some(i), label).clicked() { p.scheduled = Some(i); }
                }
            });
        });
    });
    ui.weak("Choose a field and Add, or double-click it.");
}

fn field_combo(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, kind: ScheduleKind, draft_fields: &[ScheduleField], current: &mut String) {
    let text = if current.is_empty() { "(none)".to_string() } else { field_info(kind, current).map_or(current.clone(), |i| i.label.to_string()) };
    egui::ComboBox::from_id_salt(id).selected_text(text).width(170.0).show_ui(ui, |ui| {
        ui.selectable_value(current, String::new(), "(none)");
        for f in draft_fields { if let Some(i) = field_info(kind, &f.key) { ui.selectable_value(current, f.key.clone(), i.label); } }
    });
}

fn filter_tab(ui: &mut egui::Ui, p: &mut Props, kind: ScheduleKind) {
    ui.label(format!("Filter by (up to {MAX_FILTERS}); a field that is hidden can still filter"));
    let fields = p.draft.fields.clone();
    let mut remove: Option<usize> = None;
    for i in 0..p.draft.filters.len() {
        let mut f = p.draft.filters[i].clone();
        ui.horizontal(|ui| {
            field_combo(ui, ("filter_field", i), kind, &fields, &mut f.field);
            let ty = field_info(kind, &f.field).map_or(FieldType::Text, |x| x.ty);
            if !f.op.applies(ty) { f.op = FilterOp::Equal; }
            egui::ComboBox::from_id_salt(("filter_op", i)).selected_text(f.op.name()).width(170.0).show_ui(ui, |ui| {
                for o in FilterOp::ALL { if o.applies(ty) { ui.selectable_value(&mut f.op, o, o.name()); } }
            });
            ui.add(egui::TextEdit::singleline(&mut f.value).desired_width(110.0).hint_text(if ty.numeric() { format!("number{}", if ty.unit().is_empty() { String::new() } else { format!(" in {}", ty.unit()) }) } else { "text".into() }));
            if ui.small_button("✕").on_hover_text("Remove this filter").clicked() { remove = Some(i); }
        });
        p.draft.filters[i] = f;
    }
    // a filter with no field is not saved
    if let Some(i) = remove { p.draft.filters.remove(i); }
    if ui.add_enabled(p.draft.filters.len() < MAX_FILTERS && !fields.is_empty(), egui::Button::new("Add filter")).clicked() {
        p.draft.filters.push(ScheduleFilter { field: fields[0].key.clone(), op: FilterOp::Equal, value: String::new() });
    }
}

fn sorting_tab(ui: &mut egui::Ui, p: &mut Props, kind: ScheduleKind) {
    let fields = p.draft.fields.clone();
    let mut remove: Option<usize> = None;
    for i in 0..p.draft.sorts.len() {
        let mut s = p.draft.sorts[i].clone();
        ui.horizontal(|ui| {
            ui.label(if i == 0 { "Sort by:" } else { "Then by:" });
            field_combo(ui, ("sort_field", i), kind, &fields, &mut s.field);
            ui.selectable_value(&mut s.descending, false, "Ascending");
            ui.selectable_value(&mut s.descending, true, "Descending");
            ui.checkbox(&mut s.header, "Header");
            ui.checkbox(&mut s.footer, "Footer");
            ui.checkbox(&mut s.blank, "Blank line");
            if ui.small_button("✕").on_hover_text("Remove this level").clicked() { remove = Some(i); }
        });
        p.draft.sorts[i] = s;
    }
    if let Some(i) = remove { p.draft.sorts.remove(i); }
    if ui.add_enabled(p.draft.sorts.len() < MAX_SORTS && !fields.is_empty(), egui::Button::new("Add sort level")).clicked() {
        let unused = fields.iter().find(|f| !p.draft.sorts.iter().any(|s| s.field == f.key)).map(|f| f.key.clone());
        if let Some(k) = unused { p.draft.sorts.push(ScheduleSort::new(&k)); }
    }
    ui.separator();
    ui.checkbox(&mut p.draft.grand_totals, "Grand totals");
    ui.checkbox(&mut p.draft.itemize, "Itemize every instance");
    ui.weak("A footer shows the group's count and the totals of the fields that calculate totals; with Itemize off, elements that look alike are one line.");
}

fn formatting_tab(ui: &mut egui::Ui, p: &mut Props, kind: ScheduleKind) {
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label("Fields");
            egui::ScrollArea::vertical().id_salt("sched_fmt_fields").max_height(240.0).min_scrolled_width(190.0).show(ui, |ui| {
                for (i, f) in p.draft.fields.iter().enumerate() {
                    let label = field_info(kind, &f.key).map_or(f.key.clone(), |x| x.label.to_string());
                    if ui.selectable_label(p.formatting == Some(i), label).clicked() { p.formatting = Some(i); }
                }
            });
        });
        ui.vertical(|ui| {
            let Some(i) = p.formatting.filter(|i| *i < p.draft.fields.len()) else { ui.weak("Choose a field"); return };
            let info = field_info(kind, &p.draft.fields[i].key);
            let f = &mut p.draft.fields[i];
            ui.horizontal(|ui| {
                ui.label("Heading");
                ui.add(egui::TextEdit::singleline(&mut f.heading).hint_text(info.map_or("", |x| x.label)).desired_width(180.0));
            });
            ui.checkbox(&mut f.hidden, "Hidden field");
            let summable = info.is_some_and(|x| x.ty.summable());
            ui.add_enabled(summable, egui::Checkbox::new(&mut f.totals, "Calculate totals"));
            if !summable { f.totals = false; }
            if let Some(x) = info { if !x.ty.unit().is_empty() { ui.weak(format!("Shown in {}", x.ty.unit())); } }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Pos2, Rect, Vec2};

    fn frames(app: &mut Desktop, n: usize, f: &dyn Fn(&mut Desktop, &mut egui::Ui)) {
        let ctx = egui::Context::default();
        for _ in 0..n {
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 800.0))), ..Default::default() }, |ui| f(app, ui));
            out.textures_delta.clear();
        }
    }

    #[test]
    fn a_schedule_is_made_shown_followed_with_the_model_exported_deleted_and_undone() {
        let mut app = Desktop::sample();
        app.run_command("schedule", "Schedules/Quantities");
        assert!(app.schedules.new.is_some());
        let mut d = app.schedules.new.take().unwrap();
        d.kind = ScheduleKind::Walls;
        assert!(app.confirm_new_schedule(&d), "{}", app.message);
        let id = app.document.mep().schedules[0].id;
        assert_eq!(app.view, View::Schedule(id));
        assert!(app.schedules.props.as_ref().is_some_and(|p| p.tab == Tab::Fields), "Revit goes on to Schedule Properties");
        assert_eq!(app.document.schedule(id).unwrap().name, "Wall Schedule");
        // the view draws, and its table is the model's walls
        let ui_frames = |app: &mut Desktop| frames(app, 2, &|app, ui| app.show_schedule(ui, id));
        ui_frames(&mut app);
        let rows = |app: &Desktop| app.schedules.cache.as_ref().map(|c| c.table.rows.iter().filter(|r| r.kind == RowKind::Data).count());
        assert_eq!(rows(&app), Some(2), "one line for the walls of each level");
        // the table follows the model: another wall on level 1 adds to the first line, and a new type makes a line
        let before = app.schedules.cache.as_ref().unwrap().table.clone();
        app.document.place_wall(ElementId(10), [20.0, 20.0], [25.0, 20.0], 0.3, 3.0, 0.0).unwrap();
        ui_frames(&mut app);
        let after = app.schedules.cache.as_ref().unwrap().table.clone();
        assert_ne!(before, after);
        assert_eq!(rows(&app), Some(3), "a wall of another thickness is another type");
        // a click on a row selects its elements; Show in model opens their plan
        let ids = after.rows.iter().find(|r| r.kind == RowKind::Data && r.ids.len() == 5).map(|r| r.ids.clone()).expect("the five walls of level 1");
        app.select_many(ids.clone());
        app.view = View::Schedule(id);
        app.show_schedule_selection_in_model();
        assert_eq!(app.view, View::Plan);
        app.view = View::Schedule(id);
        // all four tabs of Schedule Properties draw
        for tab in [Tab::Fields, Tab::Filter, Tab::Sorting, Tab::Formatting] {
            app.open_schedule_properties(id, tab);
            if let Some(p) = app.schedules.props.as_mut() { p.draft.filters.push(ScheduleFilter { field: "length".into(), op: FilterOp::Greater, value: "1000".into() }); p.formatting = Some(0); p.scheduled = Some(0); }
            frames(&mut app, 2, &|app, ui| { let c = ui.ctx().clone(); app.schedule_props_window(&c); });
            assert!(app.schedules.props.is_some());
        }
        // Apply makes a change in one undo step
        let steps = app.document.history_mark();
        let mut def = app.document.schedule(id).unwrap().clone();
        def.itemize = true;
        def.name = "Muros".into();
        let ops = app.document.set_schedule_commands(&def);
        assert!(app.apply_schedule(ops, "changed"));
        assert_eq!(app.document.history_mark(), steps + 1);
        assert_eq!(app.document.schedule(id).unwrap().name, "Muros");
        ui_frames(&mut app);
        assert_eq!(rows(&app), Some(7), "every wall now has its line");
        // CSV
        let dir = std::env::temp_dir().join(format!("sched-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("walls.csv");
        app.export_schedule_csv(id, &path);
        let csv = std::fs::read_to_string(&path).unwrap();
        assert!(csv.starts_with("Type,Base Constraint,Length (mm)"), "{csv}");
        assert_eq!(csv.lines().count(), 1 + 7 + 1, "the headings, a line for each wall and the grand total: {csv}");
        std::fs::remove_dir_all(&dir).ok();
        // the Project Browser lists it
        app.schedules.props = None;
        frames(&mut app, 2, &|app, ui| app.schedule_browser_items(ui, ""));
        // deleting it leaves the view, and undo brings it back
        app.delete_schedule(id);
        assert_eq!(app.view, View::Plan);
        assert!(app.document.schedule(id).is_none());
        app.undo_document();
        assert!(app.document.schedule(id).is_some());
    }

    #[test]
    fn schedules_are_saved_with_the_model_go_out_in_the_ifc_file_and_the_import_does_not_double_them() {
        let mut app = Desktop::sample();
        for (k, n) in [(ScheduleKind::Walls, "Walls, \"A\"; 1"), (ScheduleKind::Doors, "")] {
            let d = NewSchedule { kind: k, name: n.into() };
            assert!(app.confirm_new_schedule(&d), "{}", app.message);
        }
        let mut def = app.document.mep().schedules[0].clone();
        def.filters.push(ScheduleFilter { field: "length".into(), op: FilterOp::LessEqual, value: "7500.5".into() });
        def.fields[2].heading = "Long, \"one\"".into();
        def.fields[2].hidden = true;
        def.sorts = vec![ScheduleSort { field: "level".into(), descending: true, header: true, footer: true, blank: true }];
        let ops = app.document.set_schedule_commands(&def);
        assert!(app.apply_schedule(ops, "changed"));
        // the file
        let back = crate::file::document_from(&crate::file::file_text(&app.document)).unwrap();
        assert_eq!(back.mep().schedules, app.document.mep().schedules);
        assert_eq!(back.schedule_table(&back.mep().schedules[0].clone()), app.document.schedule_table(&app.document.mep().schedules[0].clone()));
        // IFC
        let out = crate::export::export_ifc_as(&app.document, "P", 1, "2026-10-19T00:00:00", crate::export::Schema::Ifc4).unwrap();
        let mut again = openbim_document::Document::new();
        crate::import::import_ifc(&mut again, &out.text).unwrap();
        assert_eq!(again.mep().schedules, app.document.mep().schedules);
        crate::import::import_ifc(&mut app.document, &out.text).unwrap();
        assert_eq!(app.document.mep().schedules.len(), 2);
    }
}
