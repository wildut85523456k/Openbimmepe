//! Sheets, as Revit's sheet views have them: a sheet is a piece of paper of a standard size with a title block and
//! viewports, each showing a view (a plan, an elevation, a section, the 3D view) or a schedule at the scale of the
//! view. A view is on one sheet only. The sheets are saved with the model; what a viewport shows is drawn from the
//! model whenever it is asked for.
use crate::{Command, Document, ElementId};

pub const MAX_SHEETS: usize = 500;
pub const MAX_VIEWPORTS: usize = 64;
/// the longest number, name or title-block text
pub const MAX_TEXT: usize = 60;
/// viewport places are within this many millimetres of the sheet's corner
pub const MAX_PLACE_MM: f64 = 5000.0;

/// The sheet sizes of the ISO series, which Cambodia and Revit's metric title blocks use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaperSize { A0, A1, A2, A3, A4 }

impl PaperSize {
    pub const ALL: [PaperSize; 5] = [PaperSize::A0, PaperSize::A1, PaperSize::A2, PaperSize::A3, PaperSize::A4];
    pub fn name(self) -> &'static str { match self { PaperSize::A0 => "A0", PaperSize::A1 => "A1", PaperSize::A2 => "A2", PaperSize::A3 => "A3", PaperSize::A4 => "A4" } }
    pub fn code(self) -> &'static str { self.name() }
    pub fn from_code(c: &str) -> Option<PaperSize> { Self::ALL.into_iter().find(|s| s.code() == c) }
    /// width and height in millimetres, the long side across unless `portrait`
    pub fn dims_mm(self, portrait: bool) -> (f64, f64) {
        let (w, h) = match self { PaperSize::A0 => (1189.0, 841.0), PaperSize::A1 => (841.0, 594.0), PaperSize::A2 => (594.0, 420.0), PaperSize::A3 => (420.0, 297.0), PaperSize::A4 => (297.0, 210.0) };
        if portrait { (h, w) } else { (w, h) }
    }
}

/// The texts of the title block that belong to the sheet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TitleBlock {
    pub project: String,
    pub client: String,
    pub date: String,
    pub drawn_by: String,
    pub checked_by: String,
}

impl TitleBlock {
    fn texts(&self) -> [&String; 5] { [&self.project, &self.client, &self.date, &self.drawn_by, &self.checked_by] }
}

/// A view or schedule placed on a sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct Viewport {
    /// the number of the viewport on its sheet
    pub id: u64,
    /// the view it shows: a view key (`p10`, `mech`, `3d`, `e0`, `s5`, `m7_2`) or `sc` and the number of a schedule
    pub view: String,
    /// where the middle of the view is, in millimetres from the left and from the top of the sheet
    pub x_mm: f64,
    pub y_mm: f64,
    /// the title; empty: the name of the view
    pub title: String,
    pub show_title: bool,
}

/// A sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetDef {
    pub id: u64,
    pub number: String,
    pub name: String,
    pub size: PaperSize,
    pub portrait: bool,
    pub block: TitleBlock,
    pub viewports: Vec<Viewport>,
}

/// The number of the schedule a view reference points to.
pub fn schedule_ref(view: &str) -> Option<u64> { view.strip_prefix("sc").and_then(|n| n.parse().ok()) }

/// The reference of a schedule.
pub fn schedule_view(id: u64) -> String { format!("sc{id}") }

/// The element a view reference depends on (the section line or the elevation marker), if it has one.
pub fn view_element(view: &str) -> Option<ElementId> {
    if view.starts_with("sc") { return None; }
    if let Some(n) = view.strip_prefix('s') { return n.parse().ok().map(ElementId); }
    let n = view.strip_prefix('m')?;
    n.split('_').next()?.parse().ok().map(ElementId)
}

fn good_text(s: &str) -> bool { s.chars().count() <= MAX_TEXT && !s.chars().any(char::is_control) }

fn good_view_ref(v: &str) -> bool { !v.is_empty() && v.len() <= 24 && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') }

impl SheetDef {
    pub fn check(&self) -> Result<(), String> {
        if self.number.trim() != self.number || self.number.is_empty() || self.number.chars().count() > 20 || !good_text(&self.number) { return Err("a sheet number is 1 to 20 characters".into()); }
        if self.name.trim() != self.name || self.name.is_empty() || !good_text(&self.name) { return Err(format!("a sheet name is 1 to {MAX_TEXT} characters")); }
        if !self.block.texts().iter().all(|t| good_text(t)) { return Err(format!("the title block texts are at most {MAX_TEXT} characters")); }
        if self.viewports.len() > MAX_VIEWPORTS { return Err(format!("a sheet has at most {MAX_VIEWPORTS} views")); }
        let mut ids = std::collections::BTreeSet::new();
        let mut views = std::collections::BTreeSet::new();
        for v in &self.viewports {
            if !good_view_ref(&v.view) { return Err("a viewport shows a view that cannot be named".into()); }
            if !v.x_mm.is_finite() || !v.y_mm.is_finite() || v.x_mm.abs() > MAX_PLACE_MM || v.y_mm.abs() > MAX_PLACE_MM { return Err("a viewport is too far from the sheet".into()); }
            if !good_text(&v.title) { return Err(format!("a view title is at most {MAX_TEXT} characters")); }
            if !ids.insert(v.id) { return Err("two viewports have the same number".into()); }
            if !views.insert(v.view.clone()) { return Err("a view is on a sheet once".into()); }
        }
        Ok(())
    }

    pub fn viewport(&self, id: u64) -> Option<&Viewport> { self.viewports.iter().find(|v| v.id == id) }

    /// The sheet's size in millimetres.
    pub fn dims_mm(&self) -> (f64, f64) { self.size.dims_mm(self.portrait) }

    /// "A101 - Floor Plan"
    pub fn label(&self) -> String { format!("{} - {}", self.number, self.name) }
}

impl Document {
    pub fn sheets(&self) -> &[SheetDef] { &self.mep().sheets }

    pub fn sheet(&self, id: u64) -> Option<&SheetDef> { self.mep().sheets.iter().find(|s| s.id == id) }

    pub fn next_sheet_id(&self) -> u64 { self.mep().sheets.iter().map(|s| s.id + 1).max().unwrap_or(1) }

    /// The sheet a view or schedule is on.
    pub fn sheet_of_view(&self, view: &str) -> Option<&SheetDef> { self.mep().sheets.iter().find(|s| s.viewports.iter().any(|v| v.view == view)) }

    /// The number Revit offers for a new sheet: the last number counted up ("A101", then "A102"), the first free one.
    pub fn next_sheet_number(&self) -> String {
        let last = self.mep().sheets.last().map_or("A100", |s| s.number.as_str());
        let digits = last.chars().rev().take_while(char::is_ascii_digit).count();
        let (stem, n) = last.split_at(last.len() - digits);
        let mut n: u64 = n.parse().unwrap_or(100);
        loop {
            n += 1;
            let candidate = format!("{stem}{n}");
            if candidate.chars().count() <= 20 && !self.mep().sheets.iter().any(|s| s.number.eq_ignore_ascii_case(&candidate)) { return candidate; }
            if n > 99_999_999 { return format!("S{}", self.next_sheet_id()); }
        }
    }

    /// New Sheet: a sheet of this size with the title block texts of the last sheet (the project and client do not
    /// change from sheet to sheet). Gives the commands and the number of the sheet.
    pub fn new_sheet_commands(&self, size: PaperSize, portrait: bool, number: &str, name: &str) -> Result<(Vec<Command>, u64), String> {
        let number = if number.trim().is_empty() { self.next_sheet_number() } else { number.trim().to_string() };
        let name = if name.trim().is_empty() { "Unnamed".to_string() } else { name.trim().to_string() };
        let mut block = self.mep().sheets.last().map(|s| s.block.clone()).unwrap_or_default();
        block.drawn_by.clear();
        block.checked_by.clear();
        let def = SheetDef { id: self.next_sheet_id(), number, name, size, portrait, block, viewports: Vec::new() };
        let id = def.id;
        Ok((self.set_sheet_commands(&def)?, id))
    }

    /// Saves `def`: a new sheet (its number is not in use) or a changed one. Sheet numbers are unique, as in Revit.
    pub fn set_sheet_commands(&self, def: &SheetDef) -> Result<Vec<Command>, String> {
        def.check()?;
        let mut data = self.mep().clone();
        if data.sheets.iter().any(|s| s.number.eq_ignore_ascii_case(&def.number) && s.id != def.id) { return Err(format!("there is a sheet numbered {} already", def.number)); }
        // a view is on one sheet only
        if let Some(v) = def.viewports.iter().find(|v| data.sheets.iter().any(|s| s.id != def.id && s.viewports.iter().any(|w| w.view == v.view))) {
            let on = data.sheets.iter().find(|s| s.id != def.id && s.viewports.iter().any(|w| w.view == v.view)).map_or(String::new(), SheetDef::label);
            return Err(format!("that view is on sheet {on} already; a view is on one sheet only"));
        }
        if let Some(v) = def.viewports.iter().find(|v| schedule_ref(&v.view).is_some_and(|s| !data.schedules.iter().any(|d| d.id == s))) { return Err(format!("{} is not a schedule of this model", v.view)); }
        match data.sheets.iter().position(|s| s.id == def.id) {
            Some(i) => { if data.sheets[i] == *def { return Err("nothing changes".into()); } data.sheets[i] = def.clone(); }
            None => { if data.sheets.len() >= MAX_SHEETS { return Err(format!("a model has at most {MAX_SHEETS} sheets")); } data.sheets.push(def.clone()); }
        }
        Ok(vec![Command::SetMep(Box::new(data))])
    }

    pub fn delete_sheet_commands(&self, id: u64) -> Result<Vec<Command>, String> {
        let mut data = self.mep().clone();
        let at = data.sheets.iter().position(|s| s.id == id).ok_or("there is no such sheet")?;
        data.sheets.remove(at);
        Ok(vec![Command::SetMep(Box::new(data))])
    }

    /// Puts a view on a sheet with its middle at (`x_mm`, `y_mm`). Gives the commands and the number of the viewport.
    pub fn place_viewport_commands(&self, sheet: u64, view: &str, x_mm: f64, y_mm: f64) -> Result<(Vec<Command>, u64), String> {
        let mut def = self.sheet(sheet).ok_or("there is no such sheet")?.clone();
        let id = def.viewports.iter().map(|v| v.id + 1).max().unwrap_or(1);
        def.viewports.push(Viewport { id, view: view.to_string(), x_mm, y_mm, title: String::new(), show_title: true });
        Ok((self.set_sheet_commands(&def)?, id))
    }

    /// Takes a view off its sheet (Revit's Remove From Sheet).
    pub fn remove_viewport_commands(&self, sheet: u64, viewport: u64) -> Result<Vec<Command>, String> {
        let mut def = self.sheet(sheet).ok_or("there is no such sheet")?.clone();
        let at = def.viewports.iter().position(|v| v.id == viewport).ok_or("there is no such view on the sheet")?;
        def.viewports.remove(at);
        self.set_sheet_commands(&def)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScheduleKind;

    fn run(d: &mut Document, c: Vec<Command>) { d.transact(d.revision(), &c).unwrap(); }

    #[test]
    fn paper_sizes_are_the_iso_series() {
        assert_eq!(PaperSize::A1.dims_mm(false), (841.0, 594.0));
        assert_eq!(PaperSize::A3.dims_mm(true), (297.0, 420.0));
        for s in PaperSize::ALL {
            assert_eq!(PaperSize::from_code(s.code()), Some(s));
            let (a, b) = s.dims_mm(false);
            assert!((a / b - std::f64::consts::SQRT_2).abs() < 0.01, "{s:?} keeps the ratio of the series");
        }
        // each size is half of the one before
        for w in PaperSize::ALL.windows(2) { assert!((w[0].dims_mm(false).0 / 2.0 - w[1].dims_mm(false).1).abs() < 2.0); }
    }

    #[test]
    fn view_references_name_a_schedule_or_an_element() {
        assert_eq!(schedule_ref("sc12"), Some(12));
        assert_eq!(schedule_ref("s12"), None);
        assert_eq!(schedule_ref("p10"), None);
        assert_eq!(view_element("s12"), Some(ElementId(12)));
        assert_eq!(view_element("m7_2"), Some(ElementId(7)));
        assert_eq!(view_element("sc12"), None);
        assert_eq!(view_element("p10"), None);
        assert_eq!(view_element("3d"), None);
        assert_eq!(schedule_ref(&schedule_view(5)), Some(5));
    }

    #[test]
    fn a_sheet_is_made_numbered_on_from_the_last_one_changed_and_undone() {
        let mut d = Document::spatial_sample();
        assert_eq!(d.next_sheet_number(), "A101");
        let steps = d.history_mark();
        let (c, a) = d.new_sheet_commands(PaperSize::A1, false, "", "").unwrap();
        run(&mut d, c);
        assert_eq!(d.history_mark(), steps + 1);
        assert_eq!((d.sheet(a).unwrap().number.as_str(), d.sheet(a).unwrap().name.as_str()), ("A101", "Unnamed"));
        assert_eq!(d.next_sheet_number(), "A102");
        // the title block texts of the last sheet go on to the next
        let mut s = d.sheet(a).unwrap().clone();
        s.block.project = "Villa Sokha".into();
        s.block.drawn_by = "WD".into();
        { let c = d.set_sheet_commands(&s).unwrap(); run(&mut d, c); }
        let (c, b) = d.new_sheet_commands(PaperSize::A3, true, "", "Sections").unwrap();
        run(&mut d, c);
        let sb = d.sheet(b).unwrap();
        assert_eq!((sb.number.as_str(), sb.block.project.as_str(), sb.block.drawn_by.as_str()), ("A102", "Villa Sokha", ""));
        assert_eq!(sb.dims_mm(), (297.0, 420.0));
        // a number is in use once, whatever its case
        assert!(d.new_sheet_commands(PaperSize::A1, false, "a101", "x").unwrap_err().contains("numbered"));
        assert!(d.set_sheet_commands(&s).unwrap_err().contains("nothing changes"));
        d.undo().unwrap();
        assert!(d.sheet(b).is_none());
        d.redo().unwrap();
        assert!(d.sheet(b).is_some());
        { let c = d.delete_sheet_commands(b).unwrap(); run(&mut d, c); }
        assert!(d.sheet(b).is_none());
        assert!(d.delete_sheet_commands(b).is_err());
        // a number that counts on from a number without digits
        let mut e = Document::spatial_sample();
        { let c = e.new_sheet_commands(PaperSize::A4, false, "COVER", "Cover").unwrap().0; run(&mut e, c); }
        assert_eq!(e.next_sheet_number(), "COVER101");
    }

    #[test]
    fn a_view_is_on_one_sheet_only_and_a_schedule_must_exist() {
        let mut d = Document::spatial_sample();
        let (c, a) = d.new_sheet_commands(PaperSize::A1, false, "", "Plans").unwrap();
        run(&mut d, c);
        let (c, b) = d.new_sheet_commands(PaperSize::A1, false, "", "More plans").unwrap();
        run(&mut d, c);
        let (c, v) = d.place_viewport_commands(a, "p10", 300.0, 250.0).unwrap();
        run(&mut d, c);
        assert_eq!(d.sheet(a).unwrap().viewport(v).unwrap().view, "p10");
        assert_eq!(d.sheet_of_view("p10").map(|s| s.id), Some(a));
        assert!(d.sheet_of_view("p11").is_none());
        let err = d.place_viewport_commands(b, "p10", 100.0, 100.0).unwrap_err();
        assert!(err.contains("one sheet only"), "{err}");
        assert!(d.place_viewport_commands(a, "p10", 100.0, 100.0).is_err(), "nor twice on the same sheet");
        assert!(d.place_viewport_commands(a, "sc9", 100.0, 100.0).unwrap_err().contains("not a schedule"));
        // a schedule goes on a sheet; deleting the schedule takes its view off
        let (c, sid) = d.new_schedule_commands(ScheduleKind::Walls, "").unwrap();
        run(&mut d, c);
        let (c, _) = d.place_viewport_commands(b, &schedule_view(sid), 200.0, 200.0).unwrap();
        run(&mut d, c);
        assert_eq!(d.sheet(b).unwrap().viewports.len(), 1);
        { let c = d.delete_schedule_commands(sid).unwrap(); run(&mut d, c); }
        assert!(d.sheet(b).unwrap().viewports.is_empty(), "the schedule is gone from its sheet");
        // taking a view off
        { let c = d.remove_viewport_commands(a, v).unwrap(); run(&mut d, c); }
        assert!(d.sheet_of_view("p10").is_none());
        assert!(d.remove_viewport_commands(a, v).is_err());
    }

    #[test]
    fn a_view_of_an_element_that_goes_leaves_the_sheet() {
        let mut d = Document::spatial_sample();
        let level = ElementId(10);
        let section = d.place_section(level, [0.0, 1.0], [5.0, 1.0], 3.0).unwrap();
        let (c, a) = d.new_sheet_commands(PaperSize::A1, false, "", "Sections").unwrap();
        run(&mut d, c);
        let key = format!("s{}", section.0);
        { let c = d.place_viewport_commands(a, &key, 400.0, 300.0).unwrap().0; run(&mut d, c); }
        { let c = d.place_viewport_commands(a, "p10", 600.0, 300.0).unwrap().0; run(&mut d, c); }
        assert_eq!(d.sheet(a).unwrap().viewports.len(), 2);
        d.transact(d.revision(), &[Command::Delete { id: section }]).unwrap();
        let views: Vec<&str> = d.sheet(a).unwrap().viewports.iter().map(|v| v.view.as_str()).collect();
        assert_eq!(views, vec!["p10"], "the section is gone from the sheet");
    }

    #[test]
    fn bad_sheets_are_refused_by_the_commands_and_by_the_model() {
        let d = Document::spatial_sample();
        let ok = SheetDef { id: 1, number: "A101".into(), name: "Plan".into(), size: PaperSize::A1, portrait: false, block: TitleBlock::default(), viewports: vec![] };
        assert!(d.set_sheet_commands(&ok).is_ok());
        let bad = |f: &dyn Fn(&mut SheetDef)| { let mut s = ok.clone(); f(&mut s); d.set_sheet_commands(&s).is_err() };
        assert!(bad(&|s| s.number = String::new()));
        assert!(bad(&|s| s.number = " A1".into()));
        assert!(bad(&|s| s.number = "A".repeat(21)));
        assert!(bad(&|s| s.name = String::new()));
        assert!(bad(&|s| s.name = "x\ny".into()));
        assert!(bad(&|s| s.name = "n".repeat(61)));
        assert!(bad(&|s| s.block.client = "c".repeat(61)));
        let vp = |id: u64, view: &str| Viewport { id, view: view.into(), x_mm: 100.0, y_mm: 100.0, title: String::new(), show_title: true };
        assert!(bad(&|s| s.viewports = vec![vp(1, "p10"), vp(1, "p11")]), "two viewports with one number");
        assert!(bad(&|s| s.viewports = vec![vp(1, "p10"), vp(2, "p10")]));
        assert!(bad(&|s| s.viewports = vec![vp(1, "bad view")]));
        assert!(bad(&|s| s.viewports = vec![Viewport { x_mm: f64::NAN, ..vp(1, "p10") }]));
        assert!(bad(&|s| s.viewports = vec![Viewport { y_mm: 1e9, ..vp(1, "p10") }]));
        assert!(bad(&|s| s.viewports = (0..65).map(|i| vp(i, &format!("p{i}"))).collect()));
        // the model refuses what the commands would have
        let mut data = d.mep().clone();
        data.sheets.push(SheetDef { number: String::new(), ..ok.clone() });
        let mut d2 = d.clone();
        assert!(d2.transact(d2.revision(), &[Command::SetMep(Box::new(data))]).is_err());
        let mut data = d.mep().clone();
        data.sheets = vec![ok.clone(), SheetDef { id: 2, ..ok.clone() }];
        assert!(d2.transact(d2.revision(), &[Command::SetMep(Box::new(data))]).is_err(), "the same number twice");
    }

    #[test]
    fn sheets_read_back_from_the_records_with_all_their_texts() {
        let mut d = Document::spatial_sample();
        let (c, a) = d.new_sheet_commands(PaperSize::A2, true, "E-101", "Plan; \"Level 1\", north | main").unwrap();
        run(&mut d, c);
        let mut s = d.sheet(a).unwrap().clone();
        s.block = TitleBlock { project: "Villa = 7".into(), client: "Mr. Sok, Mrs. Dara".into(), date: "2026-10-19".into(), drawn_by: "W.D.".into(), checked_by: "é ü 日本".into() };
        s.viewports.push(Viewport { id: 3, view: "p10".into(), x_mm: 123.456, y_mm: 78.9, title: "Ground; floor, plan".into(), show_title: false });
        s.viewports.push(Viewport { id: 4, view: "mech".into(), x_mm: -5.0, y_mm: 0.5, title: String::new(), show_title: true });
        { let c = d.set_sheet_commands(&s).unwrap(); run(&mut d, c); }
        let back = crate::native::parse_mep(&crate::native::mep_records(d.mep())).unwrap();
        assert_eq!(back.sheets, d.mep().sheets);
        // a sheet with nothing on it reads back too
        let (c, b) = d.new_sheet_commands(PaperSize::A4, false, "", "").unwrap();
        run(&mut d, c);
        let back = crate::native::parse_mep(&crate::native::mep_records(d.mep())).unwrap();
        assert_eq!(back.sheets.len(), 2);
        assert_eq!(back.sheets.iter().find(|s| s.id == b).unwrap().viewports.len(), 0);
    }
}
