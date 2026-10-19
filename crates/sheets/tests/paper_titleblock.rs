mod common;
use common::*;
use openbim_sheets::paper::{Margins, PaperSize, Rect};
use openbim_sheets::titleblock::{Align, Item, Template, TitleBlockDef};

#[test]
fn iso_and_ansi_sizes_are_exact() {
    let a1 = PaperSize::iso_a(1).unwrap();
    assert_eq!((a1.width_mm, a1.height_mm), (841.0, 594.0));
    let a4 = PaperSize::iso_a(4).unwrap();
    assert_eq!((a4.width_mm, a4.height_mm), (297.0, 210.0));
    assert!(PaperSize::iso_a(5).is_none());
    let d = PaperSize::ansi('d').unwrap();
    assert!((d.width_mm - 863.6).abs() < 1e-9 && (d.height_mm - 558.8).abs() < 1e-9);
    assert_eq!(PaperSize::iso_a(3).unwrap().portrait(), (297.0, 420.0));
}

#[test]
fn paper_names_parse() {
    for t in ["A1", "a1", " ISO A1 ", "iso  a1"] {
        assert_eq!(PaperSize::parse(t).unwrap().name, "A1", "{t}");
    }
    assert_eq!(PaperSize::parse("ANSI D").unwrap().name, "ANSI D");
    for t in ["", "A", "A9", "A10", "ANSI", "ANSI DD", "B1", "A1x"] {
        assert!(PaperSize::parse(t).is_none(), "{t}");
    }
}

#[test]
fn custom_sizes_are_checked_and_keep_orientation() {
    let p = PaperSize::custom("Roll", 500.0, 900.0).unwrap();
    assert_eq!((p.width_mm, p.height_mm), (500.0, 900.0));
    assert_eq!(p.portrait(), (500.0, 900.0));
    for (w, h) in [(900.0, 500.0), (500.0, 500.0), (50.0, 5000.0), (5000.0, 50.0)] {
        let p = PaperSize::custom("Custom", w, h).unwrap();
        assert_eq!((p.width_mm, p.height_mm), (w, h));
    }
    assert!(PaperSize::custom("x", 10.0, 900.0).is_err());
    assert!(PaperSize::custom("x", f64::NAN, 900.0).is_err());
    assert!(PaperSize::custom("", 500.0, 900.0).is_err());
    assert!(PaperSize::custom("x", 500.0, 9000.0).is_err());
}

#[test]
fn templates_find_fields_and_escape_braces() {
    let t = Template::new("Sheet {Sheet Index} of { Sheet Count } {{literal}}").unwrap();
    assert_eq!(t.fields(), vec!["Sheet Index", "Sheet Count"]);
    let (s, missing) = t.render(&|n| match n {
        "Sheet Index" => Some("2".into()),
        _ => None,
    });
    assert_eq!(s, "Sheet 2 of  {literal}");
    assert_eq!(missing, vec!["Sheet Count"]);
}

#[test]
fn bad_templates_are_refused() {
    for t in ["{open", "close}", "{}", "{ }", "{a{b}", "tab\there"] {
        assert!(Template::new(t).is_err(), "{t:?}");
    }
    assert!(Template::new(&"x".repeat(2000)).is_err());
    assert!(Template::new(&format!("{{{}}}", "n".repeat(65))).is_err());
}

#[test]
fn generic_title_block_has_the_usual_fields_and_a_clear_free_area() {
    for (tb, w, h) in [(a1(), 841.0, 594.0), (a3(), 420.0, 297.0)] {
        assert_eq!((tb.size.width_mm, tb.size.height_mm), (w, h));
        let fields = tb.fields();
        for f in ["Scale", "Sheet Size", "Sheet Number", "Current Revision", "Designed By", "Design Date", "Drawn By", "Checked By", "Approved By", "Approval Date", "Sheet Name", "Project Name", "Drawing Type", "Sheet Index", "Sheet Count", "File Path", "Plot Date"] {
            assert!(fields.iter().any(|x| x == f), "missing {f}");
        }
        // the free area is inside the border and clear of the right-hand title block column
        let b = tb.border();
        assert!(tb.free_rect.inside(&b));
        let panel = tb.items.iter().find_map(|i| if let Item::Placeholder { rect, label } = i { (label == "Owner logo").then_some(*rect) } else { None }).unwrap();
        assert!(tb.free_rect.x1 < panel.x0, "free area must end before the title block column");
        // nothing in the title block lies outside the sheet
        tb.validate().unwrap();
    }
}

#[test]
fn generic_title_block_refuses_small_paper_and_bad_margins() {
    let a5 = PaperSize::custom("small", 200.0, 100.0).unwrap();
    assert!(TitleBlockDef::generic("x", a5, Margins::all(10.0)).is_err());
    assert!(TitleBlockDef::generic("x", PaperSize::iso_a(1).unwrap(), Margins::all(-1.0)).is_err());
    assert!(TitleBlockDef::generic("x", PaperSize::iso_a(1).unwrap(), Margins::all(f64::NAN)).is_err());
    assert!(TitleBlockDef::generic("", PaperSize::iso_a(1).unwrap(), Margins::all(10.0)).is_err());
}

#[test]
fn validation_catches_items_off_the_sheet_and_bad_numbers() {
    let mut tb = a3();
    tb.items.push(Item::Line { a: [0.0, 0.0], b: [500.0, 10.0], weight: 3 });
    assert!(tb.validate().is_err());
    let mut tb = a3();
    tb.items.push(Item::Line { a: [0.0, 0.0], b: [10.0, 10.0], weight: 17 });
    assert!(tb.validate().is_err());
    let mut tb = a3();
    tb.items.push(Item::Text { at: [10.0, 10.0], height: 0.0, align: Align::Left, text: Template::new("x").unwrap(), bold: false });
    assert!(tb.validate().is_err());
    let mut tb = a3();
    tb.items.push(Item::Rect { rect: Rect { x0: 5.0, y0: 5.0, x1: 5.0, y1: 9.0 }, weight: 3 });
    assert!(tb.validate().is_err());
    let mut tb = a3();
    tb.free_rect = Rect::new(0.0, 0.0, 5.0, 5.0);
    assert!(tb.validate().is_err());
}
