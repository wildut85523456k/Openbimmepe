mod common;
use common::*;
use openbim_sheets::model::{Cloud, Sheet, SheetSet, Viewport};
use openbim_sheets::SheetError;

#[test]
fn sheets_are_checked_when_added() {
    let mut s = office_set();
    // duplicate number
    assert!(matches!(s.add_sheet(Sheet::new("A-101", "again", "Generic A1")), Err(SheetError::Duplicate { .. })));
    // forbidden characters, empty, too long, surrounding space
    for n in ["A/101", "A:1", "", " A1", "A1 ", "a*b", &"x".repeat(65), "tab\t"] {
        assert!(s.add_sheet(Sheet::new(n, "n", "Generic A1")).is_err(), "{n:?}");
    }
    assert!(s.add_sheet(Sheet::new("N-1", "", "Generic A1")).is_err());
    assert!(matches!(s.add_sheet(Sheet::new("N-2", "n", "Nope")), Err(SheetError::Unknown { .. })));
    assert_eq!(s.sheets().len(), 3);
}

#[test]
fn viewports_must_lie_in_the_free_area_and_have_sane_scales() {
    let mut s = office_set();
    let fr = s.titleblock("Generic A1").unwrap().free_rect;
    let ok = Viewport { view: "GF Architectural".into(), centre: fr.centre(), size: [100.0, 100.0], scale: 50, model_centre: [0.0, 0.0], title: None };
    let mut sh = Sheet::new("N-1", "n", "Generic A1");
    sh.viewports.push(ok.clone());
    s.add_sheet(sh).unwrap();
    for bad in [
        Viewport { centre: [10.0, 10.0], ..ok.clone() },
        Viewport { size: [fr.width() + 1.0, 100.0], ..ok.clone() },
        Viewport { scale: 0, ..ok.clone() },
        Viewport { scale: 2_000_000, ..ok.clone() },
        Viewport { size: [0.0, 10.0], ..ok.clone() },
        Viewport { size: [f64::NAN, 10.0], ..ok.clone() },
        Viewport { model_centre: [f64::INFINITY, 0.0], ..ok.clone() },
        Viewport { view: "".into(), ..ok.clone() },
    ] {
        let mut sh = Sheet::new("N-2", "n", "Generic A1");
        sh.viewports.push(bad);
        assert!(s.add_sheet(sh).is_err());
    }
}

#[test]
fn clouds_need_a_known_revision_and_stay_inside_the_border() {
    let mut s = office_set();
    let mk = |rev: u32, pts: Vec<[f64; 2]>| {
        let mut sh = Sheet::new("N-1", "n", "Generic A1");
        sh.clouds.push(Cloud { revision: rev, outline: pts });
        sh
    };
    let tri = vec![[100.0, 100.0], [150.0, 100.0], [150.0, 150.0]];
    assert!(s.add_sheet(mk(9, tri.clone())).is_err());
    assert!(s.add_sheet(mk(1, vec![[1.0, 1.0], [2.0, 2.0]])).is_err());
    assert!(s.add_sheet(mk(1, vec![[1.0, 1.0], [150.0, 100.0], [150.0, 150.0]])).is_err(), "a point in the margin is outside the border");
    assert!(s.add_sheet(mk(1, vec![[f64::NAN, 100.0], [150.0, 100.0], [150.0, 150.0]])).is_err());
    s.add_sheet(mk(1, tri)).unwrap();
}

#[test]
fn computed_fields() {
    let s = office_set();
    let a = s.sheet("A-101").unwrap();
    let m = s.sheet("M-101").unwrap();
    let c = s.sheet("C-001").unwrap();
    let x = ctx();
    assert_eq!(s.field(a, &x, "Sheet Index").as_deref(), Some("1"));
    assert_eq!(s.field(m, &x, "Sheet Index").as_deref(), Some("2"));
    assert_eq!(s.field(c, &x, "sheet count").as_deref(), Some("3"));
    assert_eq!(s.field(a, &x, "Sheet Size").as_deref(), Some("A1"));
    assert_eq!(s.field(c, &x, "Sheet Size").as_deref(), Some("A3"));
    assert_eq!(s.field(a, &x, "Scale").as_deref(), Some("1:100"));
    assert_eq!(s.field(c, &x, "Scale").as_deref(), Some("1:200"));
    assert_eq!(s.field(a, &x, "Plot Date").as_deref(), Some("2026-10-15"));
    assert_eq!(s.field(a, &x, "File Path").as_deref(), Some("office.obim"));
    assert_eq!(s.field(a, &x, "Nonsense"), None);
}

#[test]
fn scale_text_follows_the_viewports() {
    let mut s = office_set();
    let fr = s.titleblock("Generic A1").unwrap().free_rect;
    let mk = |scale| Viewport { view: "GF Architectural".into(), centre: fr.centre(), size: [50.0, 50.0], scale, model_centre: [0.0, 0.0], title: None };
    let mut none = Sheet::new("N-0", "no views", "Generic A1");
    s.add_sheet(none.clone()).unwrap();
    assert_eq!(s.scale_text(&none), "");
    none.viewports = vec![mk(50), mk(50)];
    assert_eq!(s.scale_text(&none), "1:50");
    none.viewports = vec![mk(50), mk(100)];
    assert_eq!(s.scale_text(&none), "As indicated");
}

#[test]
fn current_revision_is_the_latest_issued_one_with_a_cloud_on_the_sheet() {
    let mut s = office_set();
    let m = s.sheet("M-101").unwrap().clone();
    let x = ctx();
    // revision 1 is issued, 2 is not, both have clouds on M-101
    assert_eq!(s.field(&m, &x, "Current Revision").as_deref(), Some("1"));
    assert_eq!(s.field(&m, &x, "Current Revision Date").as_deref(), Some("2026-10-01"));
    assert_eq!(s.field(&m, &x, "Current Revision Description").as_deref(), Some("Issued for tender"));
    assert_eq!(s.revisions_on(&m).iter().map(|r| r.seq).collect::<Vec<_>>(), vec![1, 2]);
    s.issue_revision(2).unwrap();
    assert_eq!(s.field(&m, &x, "Current Revision").as_deref(), Some("2"));
    // a sheet without clouds has none
    let a = s.sheet("A-101").unwrap();
    assert_eq!(s.field(a, &x, "Current Revision").as_deref(), Some(""));
    assert!(s.issue_revision(77).is_err());
}

#[test]
fn parameters_sheet_before_project_and_names_ignore_case() {
    let mut s = office_set();
    let x = ctx();
    assert_eq!(s.field(s.sheet("A-101").unwrap(), &x, "drawn by").as_deref(), Some("WD"));
    assert_eq!(s.field(s.sheet("A-101").unwrap(), &x, " Project Name ").as_deref(), Some("Office Tower"));
    let mut sh = Sheet::new("N-1", "n", "Generic A1");
    sh.params = params(&[("Project Name", "Override")]);
    s.add_sheet(sh).unwrap();
    assert_eq!(s.field(s.sheet("N-1").unwrap(), &x, "Project Name").as_deref(), Some("Override"));
}

#[test]
fn project_and_parameter_limits() {
    assert!(SheetSet::new(params(&[("", "x")])).is_err());
    assert!(SheetSet::new(params(&[("a", "tab\t")])).is_err());
    assert!(SheetSet::new(params(&[("a", &"x".repeat(2000))])).is_err());
    let mut s = office_set();
    assert!(s.add_revision("bad\tdate", "d").is_err());
    let dup = a1();
    assert!(matches!(s.add_titleblock(dup), Err(SheetError::Duplicate { .. })));
}

#[test]
fn replace_keeps_the_position() {
    let mut s = office_set();
    let mut m = s.sheet("M-101").unwrap().clone();
    m.name = "Renamed".into();
    s.replace_sheet(m).unwrap();
    assert_eq!(s.sheets()[1].number, "M-101");
    assert_eq!(s.sheets()[1].name, "Renamed");
    assert!(s.replace_sheet(Sheet::new("Z-9", "z", "Generic A1")).is_err());
}
