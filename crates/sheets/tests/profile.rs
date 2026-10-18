mod common;
use common::*;
use openbim_sheets::model::{Sheet, SheetSet, Viewport};
use openbim_sheets::pdf::write_pdf;
use openbim_sheets::profile::*;
use openbim_sheets::render::{Prim, PrintMode};
use serde_json::{json, Value};

const FIXTURE: &str = include_str!("data/synthetic-print-profile.json");

#[test]
fn synthetic_profile_loads_ten_papers_and_roundtrips_without_changing_values() {
    let profile = PrintProfile::from_json(FIXTURE).unwrap();
    assert_eq!(profile.papers.len(), 10);
    assert_eq!(profile.mode, PrintMode::BlackLines);
    let tb = profile.generic_titleblock("A0 portrait", "Loaded A0").unwrap();
    assert_eq!((tb.size.width_mm, tb.size.height_mm), (841.0, 1189.0));
    assert_eq!(tb.margins.left, 10.0);
    let pens = profile.line_weights().unwrap();
    assert_eq!(pens.paper_mm(4), 0.18);
    assert_eq!(pens.model_mm(4, 100), 0.35);
    assert_eq!(pens.model_mm(4, 200), 0.245);
    assert_eq!(PrintProfile::from_json(&profile.to_json().unwrap()).unwrap(), profile);
    assert!(profile.generic_titleblock("unknown", "X").is_err());
}

#[test]
fn media_comparison_reports_matches_differences_missing_names_and_orientation() {
    let profile = PrintProfile::from_json(FIXTURE).unwrap();
    let source: Vec<_> = profile.papers.iter().map(|p| SourceMedia { name: p.name.clone(), width_mm: p.width_mm, height_mm: p.height_mm }).collect();
    let report = profile.compare_media(&source, 0.001).unwrap();
    assert_eq!(report.len(), 10);
    assert!(report.iter().all(|r| r.status == MediaStatus::Match && r.difference_mm == Some([0.0, 0.0])));
    let source = vec![
        SourceMedia { name: "A0 portrait".into(), width_mm: 841.0, height_mm: 1188.8 },
        SourceMedia { name: "A1 portrait".into(), width_mm: 841.0, height_mm: 594.0 },
        SourceMedia { name: "Unmapped".into(), width_mm: 500.0, height_mm: 600.0 },
    ];
    let report = profile.compare_media(&source, 0.01).unwrap();
    assert_eq!(report[0].status, MediaStatus::Different);
    assert!((report[0].difference_mm.unwrap()[1] - 0.2).abs() < 1e-9);
    assert_eq!(report[1].status, MediaStatus::Different, "orientation must not be silently normalized");
    assert_eq!(report[2].status, MediaStatus::Missing);
    assert_eq!(report[2].source_size_mm, [500.0, 600.0]);
    assert_eq!(report[2].profile_size_mm, None);
    assert!(profile.compare_media(&source, -1.0).is_err());
    assert!(profile.compare_media(&source, f64::NAN).is_err());
    assert!(profile.compare_media(&[source[0].clone(), source[0].clone()], 0.01).is_err());
}

#[test]
fn profile_pens_and_mode_drive_real_sheet_output() {
    let profile = PrintProfile::from_json(FIXTURE).unwrap();
    let tb = profile.generic_titleblock("A0 portrait", "Loaded").unwrap();
    let free = tb.free_rect;
    let mut set = SheetSet::default();
    set.add_titleblock(tb).unwrap();
    let mut sheet = Sheet::new("P-001", "Profile export", "Loaded");
    sheet.viewports.push(Viewport { view: "GF Architectural".into(), centre: free.centre(), size: [free.width() - 10.0, free.height() - 20.0], scale: 100, model_centre: [10.0, 6.0], title: None });
    set.add_sheet(sheet).unwrap();
    let rendered = profile.render_all(&set, &views(), &contents(), &ctx()).unwrap();
    assert!(rendered[0].prims.iter().any(|p| matches!(p, Prim::Line { layer, rgb, weight_mm, .. } if layer == "A-WALLS" && *rgb == [0; 3] && *weight_mm == 0.35)));
    assert!(rendered[0].prims.iter().any(|p| matches!(p, Prim::Line { layer, rgb, .. } if layer == "S-COLUMNS" && *rgb == [127; 3])));
    let pdf = write_pdf(&rendered, "Loaded profile");
    let text = String::from_utf8_lossy(&pdf.bytes);
    assert!(text.contains("/MediaBox [0 0 2383.937 3370.394]"));
    assert_eq!(profile.papers[1].height_mm, 1189.0, "export must not mutate the profile");
}

#[test]
fn malformed_or_ambiguous_profiles_are_rejected() {
    let base: Value = serde_json::from_str(FIXTURE).unwrap();
    let mut cases = Vec::new();
    for (pointer, value) in [
        ("/schema_version", json!(2)),
        ("/mode", json!("sepia")),
        ("/name", json!("")),
        ("/papers", json!([])),
        ("/papers/0/width_mm", json!(49)),
        ("/papers/0/margins_mm/left", json!(-1)),
        ("/papers/0/margins_mm/right", json!(5000)),
        ("/paper_pens_mm", json!([0.25])),
        ("/paper_pens_mm/0", json!(0)),
        ("/model_pen_tables", json!([])),
        ("/model_pen_tables/0/scale", json!(0)),
        ("/model_pen_tables/1/scale", json!(100)),
        ("/model_pen_tables/0/widths_mm/1", json!(11)),
    ] {
        let mut candidate = base.clone();
        *candidate.pointer_mut(pointer).unwrap() = value;
        cases.push(candidate);
    }
    let mut unknown = base.clone();
    unknown["unexpected"] = json!(true);
    cases.push(unknown);
    let mut nested_unknown = base.clone();
    nested_unknown["papers"][0]["margins_mm"]["unit"] = json!("inch");
    cases.push(nested_unknown);
    let mut duplicate = base.clone();
    duplicate["papers"][1]["name"] = duplicate["papers"][0]["name"].clone();
    cases.push(duplicate);
    let mut missing = base.clone();
    missing.as_object_mut().unwrap().remove("mode");
    cases.push(missing);
    for candidate in cases {
        assert!(PrintProfile::from_json(&candidate.to_string()).is_err(), "{candidate}");
    }
    assert!(PrintProfile::from_json(&" ".repeat(1_048_577)).is_err());
    assert!(PrintProfile::from_json("{").is_err());
    let mut typed = PrintProfile::from_json(FIXTURE).unwrap();
    typed.papers[0].margins_mm.left = f64::INFINITY;
    assert!(typed.validate().is_err());
    assert!(typed.to_json().is_err());
    assert!(typed.render_all(&office_set(), &views(), &contents(), &ctx()).is_err());
}
