mod common;
use common::*;
use openbim_sheets::export::{ExportCommand, ExportFormat, ExportSource};
use openbim_sheets::model::SheetSet;
use openbim_sheets::profile::PrintProfile;
use openbim_sheets::render::ViewContents;
use openbim_sheets::SheetError;

fn run(format: ExportFormat, numbers: &[&str], set: &SheetSet, content: &ViewContents)
    -> Result<openbim_sheets::export::ExportBundle, SheetError>
{
    let profile = PrintProfile::from_json(include_str!("data/synthetic-print-profile.json"))?;
    ExportCommand {
        format, sheets: numbers.iter().map(|s| (*s).into()).collect(), title: "Command test".into(),
    }.execute(ExportSource {
        set, views: &views(), contents: content, profile: &profile, context: &ctx(),
    })
}

#[test]
fn selected_sheets_keep_request_order_and_do_not_mutate_inputs() {
    let set = office_set();
    let before = format!("{set:?}");
    let content = contents();
    let pdf = run(ExportFormat::Pdf, &["C-001", "A-101"], &set, &content).unwrap();
    assert_eq!(pdf.files.len(), 1);
    assert_eq!(pdf.files[0].sheets, ["C-001", "A-101"]);
    let text = String::from_utf8_lossy(&pdf.files[0].bytes);
    assert!(text.starts_with("%PDF-1.4"));
    assert!(text.contains("/Count 2"));
    let a3 = text.find("/MediaBox [0 0 1190.551 841.89]").unwrap();
    let a1 = text.find("/MediaBox [0 0 2383.937 1683.78]").unwrap();
    assert!(a3 < a1, "page order must match the explicit selection");
    let dxf = run(ExportFormat::Dxf, &["C-001", "A-101"], &set, &content).unwrap();
    assert_eq!(dxf.files.len(), 2);
    assert_eq!(dxf.files[0].name, "sheet-0001.dxf");
    assert_eq!(dxf.files[0].sheets, ["C-001"]);
    assert_eq!(dxf.files[1].sheets, ["A-101"]);
    let text = String::from_utf8_lossy(&dxf.files[0].bytes);
    assert!(text.contains("$EXTMAX\n10\n420.0000\n20\n297.0000"));
    assert!(text.contains("0\nLINE\n"), "model and sheet geometry must be encoded");
    assert_eq!(format!("{set:?}"), before);
}

#[test]
fn invalid_selection_and_disconnected_content_fail_for_both_formats() {
    let set = office_set();
    for format in [ExportFormat::Pdf, ExportFormat::Dxf] {
        assert!(run(format, &[], &set, &contents()).is_err());
        assert!(matches!(run(format, &["A-101", "A-101"], &set, &contents()),
            Err(SheetError::Duplicate { what: "selected sheet", .. })));
        assert!(matches!(run(format, &["A-101", "missing"], &set, &contents()),
            Err(SheetError::Unknown { what: "sheet", .. })));
        assert!(matches!(run(format, &["A-101"], &set, &ViewContents::new()),
            Err(SheetError::Unknown { what: "view contents", .. })));
        let mut explicitly_empty = contents();
        explicitly_empty.get_mut("GF Architectural").unwrap().clear();
        assert!(run(format, &["A-101"], &set, &explicitly_empty).is_ok(),
            "a connected but intentionally empty view is allowed");
    }
}

#[test]
fn font_and_render_warnings_reach_the_caller() {
    let mut set = office_set();
    let mut sheet = set.sheet("A-101").unwrap().clone();
    sheet.name = "Plan 漢".into();
    set.replace_sheet(sheet).unwrap();
    let result = run(ExportFormat::Pdf, &["A-101"], &set, &contents()).unwrap();
    assert!(result.warnings.iter().any(|w| w.contains("PDF font")));
    assert!(result.warnings.iter().any(|w| w.contains("Designed By")),
        "title-block field warnings must not be dropped");
}
