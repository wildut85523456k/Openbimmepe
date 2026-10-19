mod common;
use common::*;
use openbim_sheets::import::*;
use openbim_sheets::model::SheetSet;
use openbim_sheets::SheetError;

const BASIC: &[u8] = include_bytes!("data/basic.xlsx");
const BAD: &[u8] = include_bytes!("data/bad_rows.xlsx");
const AMBIG: &[u8] = include_bytes!("data/ambiguous.xlsx");
const NONUM: &[u8] = include_bytes!("data/no_number.xlsx");
const EMPTY: &[u8] = include_bytes!("data/empty.xlsx");
const UPDATE: &[u8] = include_bytes!("data/update.xlsx");
const OFFSET: &[u8] = include_bytes!("data/offset.xlsx");
const TBCOL: &[u8] = include_bytes!("data/with_titleblock_column.xlsx");

fn blank() -> SheetSet {
    let mut s = SheetSet::new(params(&[("Project Name", "Office Tower")])).unwrap();
    s.add_titleblock(a1()).unwrap();
    s.add_titleblock(a3()).unwrap();
    s
}

fn target<'a>(r: &'a ImportReport, header: &str) -> &'a Target {
    &r.columns.iter().find(|c| c.header == header).unwrap_or_else(|| panic!("no column {header}")).target
}

#[test]
fn columns_match_names_ignoring_case_spaces_and_punctuation() {
    let mut s = blank();
    let r = import_xlsx(&mut s, &views(), BASIC, &ImportOptions::default()).unwrap();
    assert_eq!(*target(&r, "Sheet No."), Target::Number);
    assert_eq!(*target(&r, "NAME"), Target::Name);
    assert_eq!(*target(&r, "Paper"), Target::Size);
    assert_eq!(*target(&r, "drawn_by"), Target::Field("Drawn By".into()));
    assert_eq!(*target(&r, "design date"), Target::Field("Design Date".into()));
    assert_eq!(*target(&r, "Scale"), Target::Scale);
    assert_eq!(*target(&r, "View"), Target::View);
    assert_eq!(*target(&r, "Extra Col"), Target::Ignored);
    assert_eq!(*target(&r, "Current Revision"), Target::Ignored, "computed by the program");
    assert!(r.warnings.iter().any(|w| w.contains("'Extra Col' matches no sheet property")));
    assert!(r.warnings.iter().any(|w| w.contains("'Current Revision' is computed")));
}

#[test]
fn rows_become_sheets_with_values_params_and_viewports() {
    let mut s = blank();
    let r = import_xlsx(&mut s, &views(), BASIC, &ImportOptions::default()).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert!(r.applied);
    assert_eq!(r.created, vec!["101", "M-101", "C-001", "S-1"]);
    assert!(r.updated.is_empty());
    assert_eq!(s.sheets().len(), 4);
    let a = s.sheet("101").unwrap();
    assert_eq!(a.name, "Ground floor plan");
    assert_eq!(a.titleblock, "Generic A1", "a numeric cell 101 is the text 101");
    assert_eq!(a.params.get("Drawn By").map(String::as_str), Some("WD"));
    assert_eq!(a.params.get("Design Date").map(String::as_str), Some("2026-10-02"), "a date cell");
    assert_eq!(a.viewports.len(), 1);
    assert_eq!((a.viewports[0].view.as_str(), a.viewports[0].scale), ("GF Architectural", 100));
    let fr = s.titleblock("Generic A1").unwrap().free_rect;
    assert_eq!(a.viewports[0].centre, fr.centre());
    assert!(a.viewports[0].frame().inside(&fr));
    let m = s.sheet("M-101").unwrap();
    assert_eq!(m.name, "Étage 0 – ventilation");
    assert_eq!(m.titleblock, "Generic A1", "'iso a1' is A1");
    assert_eq!(m.viewports[0].scale, 50, "a bare number is a scale denominator");
    let c = s.sheet("C-001").unwrap();
    assert_eq!(c.titleblock, "Generic A3");
    assert!(c.viewports.is_empty(), "undefined view: no viewport");
    assert!(r.warnings.iter().any(|w| w.contains("'No Such View' is not defined")));
    assert_eq!(c.params.get("Design Date").map(String::as_str), Some("2026-10-03"), "text kept as it is");
    assert!(s.sheet("S-1").unwrap().viewports.is_empty());
    assert!(!a.params.contains_key("Extra Col"));
}

#[test]
fn the_imported_set_prints() {
    let mut s = blank();
    import_xlsx(&mut s, &views(), BASIC, &ImportOptions::default()).unwrap();
    let out = render_all(&s);
    assert_eq!(out.len(), 4);
    let texts: Vec<String> = out[0].prims.iter().filter_map(|p| if let openbim_sheets::render::Prim::Text { text, .. } = p { Some(text.clone()) } else { None }).collect();
    assert!(texts.iter().any(|t| t == "Sheet 1 of 4"));
    assert!(texts.iter().any(|t| t == "WD"));
}

fn render_all(s: &SheetSet) -> Vec<openbim_sheets::render::RenderedSheet> {
    openbim_sheets::render::render_all(s, &views(), &contents(), &lw(), &ctx()).unwrap()
}

#[test]
fn unmatched_columns_can_be_kept_as_parameters() {
    let mut s = blank();
    let o = ImportOptions { unmatched: Unmatched::AsParameters, ..Default::default() };
    let r = import_xlsx(&mut s, &views(), BASIC, &o).unwrap();
    assert_eq!(*target(&r, "Extra Col"), Target::Param("Extra Col".into()));
    assert_eq!(s.sheet("101").unwrap().params.get("Extra Col").map(String::as_str), Some("x"));
    assert!(!s.sheet("S-1").unwrap().params.contains_key("Extra Col"), "an empty cell sets nothing");
    // a computed field is never kept as a parameter
    assert_eq!(*target(&r, "Current Revision"), Target::Ignored);
}

#[test]
fn another_worksheet_can_be_chosen() {
    let mut s = blank();
    let o = ImportOptions { worksheet: Some("Second".into()), default_titleblock: Some("Generic A3".into()), ..Default::default() };
    let r = import_xlsx(&mut s, &views(), BASIC, &o).unwrap();
    assert_eq!(r.created, vec!["Z-1"]);
    assert_eq!(s.sheet("Z-1").unwrap().titleblock, "Generic A3");
    let o = ImportOptions { worksheet: Some("Nope".into()), ..Default::default() };
    assert!(matches!(import_xlsx(&mut s, &views(), BASIC, &o), Err(SheetError::Unknown { .. })));
}

#[test]
fn bad_rows_are_reported_with_their_spreadsheet_row_and_nothing_is_applied() {
    let mut s = blank();
    let o = ImportOptions { mode: Mode::CreateOnly, ..Default::default() };
    let r = import_xlsx(&mut s, &views(), BAD, &o).unwrap();
    let rows: Vec<usize> = r.errors.iter().map(|e| e.row).collect();
    assert_eq!(rows, vec![2, 4, 5, 6, 7, 8, 9], "{:#?}", r.errors);
    let msg = |row: usize| r.errors.iter().find(|e| e.row == row).unwrap().message.clone();
    assert!(msg(2).contains("forbidden characters"));
    assert!(msg(4).contains("already exists"));
    assert!(msg(5).contains("no title block for size"));
    assert!(msg(6).contains("sheet name is empty"));
    assert!(msg(7).contains("not like 1:100"));
    assert!(msg(8).contains("sheet number is empty"));
    assert!(msg(9).contains("too long"));
    assert!(!r.applied);
    assert!(r.created.is_empty());
    assert!(s.sheets().is_empty(), "atomic: the good row is not applied either");
}

#[test]
fn non_atomic_import_applies_the_good_rows() {
    let mut s = blank();
    let o = ImportOptions { mode: Mode::CreateOnly, atomic: false, ..Default::default() };
    let r = import_xlsx(&mut s, &views(), BAD, &o).unwrap();
    assert!(r.applied);
    assert_eq!(r.created, vec!["B-1"]);
    assert_eq!(s.sheets().len(), 1);
    assert_eq!(s.sheets()[0].number, "B-1");
}

#[test]
fn a_repeated_number_updates_in_create_or_update_mode() {
    let mut s = blank();
    let r = import_xlsx(&mut s, &views(), BAD, &ImportOptions { atomic: false, ..Default::default() }).unwrap();
    // row 4 repeats B-1: it updates the sheet created by row 3
    assert_eq!(r.created, vec!["B-1"]);
    assert_eq!(r.updated, vec!["B-1"]);
    assert_eq!(s.sheet("B-1").unwrap().name, "duplicate of the row above");
}

#[test]
fn update_changes_only_the_filled_cells() {
    let mut s = office_set();
    let r = import_xlsx(&mut s, &views(), UPDATE, &ImportOptions::default()).unwrap();
    // N-9 has no title block, size or default and there are two title blocks
    assert_eq!(r.errors.len(), 1, "{:?}", r.errors);
    assert_eq!(r.errors[0].row, 3);
    assert!(r.errors[0].message.contains("no title block, size or default"));
    assert!(!r.applied);
    assert_eq!(s.sheet("A-101").unwrap().name, "Ground floor plan", "atomic: nothing changed");

    let o = ImportOptions { default_titleblock: Some("Generic A3".into()), ..Default::default() };
    let r = import_xlsx(&mut s, &views(), UPDATE, &o).unwrap();
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(r.updated, vec!["A-101"]);
    assert_eq!(r.created, vec!["N-9"]);
    let a = s.sheet("A-101").unwrap();
    assert_eq!(a.name, "Ground floor plan (revised)");
    assert_eq!(a.params.get("Drawn By").map(String::as_str), Some("WD"), "an empty cell leaves the value");
    assert_eq!(a.params.get("Checked By").map(String::as_str), Some("RM"));
    assert_eq!(a.viewports.len(), 1, "viewports are kept");
    assert_eq!(s.sheets()[0].number, "A-101", "update keeps the position");
    assert_eq!(s.sheet("N-9").unwrap().titleblock, "Generic A3");
}

#[test]
fn a_title_block_column_and_field_precedence() {
    let mut s = blank();
    let r = import_xlsx(&mut s, &views(), TBCOL, &ImportOptions::default()).unwrap();
    assert_eq!(r.errors.len(), 1);
    assert!(r.errors[0].message.contains("no title block called 'Generic A0'"));
    let o = ImportOptions { atomic: false, ..Default::default() };
    let r = import_xlsx(&mut s, &views(), TBCOL, &o).unwrap();
    assert_eq!(r.created, vec!["T-1"]);
    let t = s.sheet("T-1").unwrap();
    assert_eq!(t.titleblock, "Generic A3");
    assert_eq!(t.params.get("Project Name").map(String::as_str), Some("Override project"));
    let f = s.field(t, &ctx(), "Project Name").unwrap();
    assert_eq!(f, "Override project", "the sheet value comes before the project value");
    assert_eq!(s.field(t, &ctx(), "Approved By").as_deref(), Some("PM"));
}

#[test]
fn files_that_cannot_work_are_refused_whole() {
    let mut s = blank();
    let o = ImportOptions::default();
    assert!(matches!(import_xlsx(&mut s, &views(), AMBIG, &o), Err(SheetError::Import(m)) if m.contains("both fill the same field")));
    assert!(matches!(import_xlsx(&mut s, &views(), NONUM, &o), Err(SheetError::Import(m)) if m.contains("no column holds the sheet number")));
    assert!(matches!(import_xlsx(&mut s, &views(), EMPTY, &o), Err(SheetError::Import(m)) if m.contains("empty")));
    assert!(matches!(import_xlsx(&mut s, &views(), b"not a workbook", &o), Err(SheetError::Import(_))));
    assert!(matches!(import_xlsx(&mut s, &views(), &[], &o), Err(SheetError::Import(_))));
    let big = vec![0u8; MAX_BYTES + 1];
    assert!(matches!(import_xlsx(&mut s, &views(), &big, &o), Err(SheetError::Import(m)) if m.contains("larger than")));
    assert!(s.sheets().is_empty());
}

#[test]
fn expected_columns_lists_what_a_file_should_contain() {
    let cols = expected_columns(&blank());
    assert_eq!(&cols[..5], ["Sheet Number", "Sheet Name", "Sheet Size", "Scale", "View"]);
    for f in ["Designed By", "Drawn By", "Checked By", "Approved By", "Project Name", "Drawing Type", "Facility", "Area"] {
        assert!(cols.iter().any(|c| c == f), "{f}");
    }
    for computed in ["Sheet Index", "Sheet Count", "Plot Date", "File Path", "Current Revision"] {
        assert!(!cols.iter().any(|c| c == computed), "{computed} is computed, not a column");
    }
    let mut lower: Vec<String> = cols.iter().map(|c| c.to_lowercase()).collect();
    lower.sort();
    lower.dedup();
    assert_eq!(lower.len(), cols.len(), "no column twice");
}

#[test]
fn the_header_need_not_start_in_cell_a1() {
    let mut s = blank();
    let r = import_xlsx(&mut s, &views(), OFFSET, &ImportOptions { atomic: false, ..Default::default() }).unwrap();
    assert_eq!(r.created, vec!["O-1"]);
    assert_eq!(r.errors.len(), 1);
    assert_eq!(r.errors[0].row, 5, "the bad row is row 5 of the spreadsheet");
    assert_eq!(s.sheet("O-1").unwrap().titleblock, "Generic A1");
}
