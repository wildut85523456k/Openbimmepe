#![allow(dead_code)]
use openbim_sheets::model::{Cloud, Params, PrintContext, Sheet, SheetSet, Viewport};
use openbim_sheets::paper::{Margins, PaperSize};
use openbim_sheets::render::{LineWeightTable, ModelPrim, ViewContents};
use openbim_sheets::titleblock::TitleBlockDef;
use openbim_sheets::visibility::{Category, CategoryKind, Discipline, Filter, Op, Override, Rule, ViewLibrary, ViewSettings};

pub const WEIGHTS: [f64; 16] = [0.05, 0.09, 0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1.0, 1.4, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0];

pub fn params(kv: &[(&str, &str)]) -> Params {
    kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

pub fn lw() -> LineWeightTable {
    LineWeightTable::uniform(WEIGHTS.to_vec()).unwrap()
}

pub fn a1() -> TitleBlockDef {
    TitleBlockDef::generic("Generic A1", PaperSize::iso_a(1).unwrap(), Margins { left: 20.0, top: 10.0, right: 10.0, bottom: 10.0 }).unwrap()
}

pub fn a3() -> TitleBlockDef {
    TitleBlockDef::generic("Generic A3", PaperSize::iso_a(3).unwrap(), Margins { left: 20.0, top: 10.0, right: 10.0, bottom: 10.0 }).unwrap()
}

pub fn library() -> ViewLibrary {
    let mut l = ViewLibrary::new();
    for (n, d, k) in [
        ("Walls", Discipline::Architectural, CategoryKind::Model),
        ("Doors", Discipline::Architectural, CategoryKind::Model),
        ("Columns", Discipline::Structural, CategoryKind::Model),
        ("Ducts", Discipline::Mechanical, CategoryKind::Model),
        ("Pipes", Discipline::Plumbing, CategoryKind::Model),
        ("Cable Trays", Discipline::Electrical, CategoryKind::Model),
        ("Sprinklers", Discipline::FireProtection, CategoryKind::Model),
        ("Grids", Discipline::Coordination, CategoryKind::Annotation),
    ] {
        l.add_category(Category { name: n.into(), discipline: d, kind: k }).unwrap();
    }
    l
}

fn mp(a: [f64; 2], b: [f64; 2], cat: &str, w: u8) -> ModelPrim {
    ModelPrim { a, b, category: cat.into(), params: Params::new(), link: None, weight: w }
}

/// A 20 m x 12 m office: walls, a column, a duct, a pipe, a cable tray, a sprinkler line and a grid line.
pub fn office_lines() -> Vec<ModelPrim> {
    let mut v = Vec::new();
    let c = [[0.0, 0.0], [20.0, 0.0], [20.0, 12.0], [0.0, 12.0]];
    for i in 0..4 {
        v.push(mp(c[i], c[(i + 1) % 4], "Walls", 8));
    }
    v.push(mp([10.0, 0.0], [10.0, 12.0], "Doors", 5));
    v.push(mp([5.0, 5.0], [5.4, 5.0], "Columns", 9));
    v.push(mp([2.0, 3.0], [18.0, 3.0], "Ducts", 6));
    v.push(mp([2.0, 6.0], [18.0, 6.0], "Pipes", 4));
    v.push(mp([2.0, 9.0], [18.0, 9.0], "Cable Trays", 4));
    v.push(mp([2.0, 10.5], [18.0, 10.5], "Sprinklers", 3));
    v.push(mp([-3.0, 6.0], [23.0, 6.0], "Grids", 2));
    v
}

pub fn views() -> ViewLibrary {
    let mut l = library();
    let mut f = ViewSettings::new("GF Architectural", Discipline::Architectural);
    f.categories.insert("Grids".into(), Override { visible: Some(true), ..Default::default() });
    l.add_view(f).unwrap();
    l.add_view(ViewSettings::new("GF Mechanical", Discipline::Mechanical)).unwrap();
    l.add_view(ViewSettings::new("GF Coordination", Discipline::Coordination)).unwrap();
    l
}

pub fn contents() -> ViewContents {
    let mut c = ViewContents::new();
    for v in ["GF Architectural", "GF Mechanical", "GF Coordination"] {
        c.insert(v.to_string(), office_lines());
    }
    c
}

pub fn office_set() -> SheetSet {
    let mut s = SheetSet::new(params(&[("Project Name", "Office Tower"), ("Facility", "Block B"), ("Contract Number", "C-77")])).unwrap();
    s.add_titleblock(a1()).unwrap();
    s.add_titleblock(a3()).unwrap();
    let r1 = s.add_revision("2026-10-01", "Issued for tender").unwrap();
    let _r2 = s.add_revision("2026-10-10", "Duct reroute").unwrap();
    s.issue_revision(r1).unwrap();
    let fr = s.titleblock("Generic A1").unwrap().free_rect;
    let vp = |view: &str, scale: u32| Viewport { view: view.into(), centre: fr.centre(), size: [fr.width() - 20.0, fr.height() - 20.0], scale, model_centre: [10.0, 6.0], title: None };
    let mut a = Sheet::new("A-101", "Ground floor plan", "Generic A1");
    a.params = params(&[("Drawn By", "WD"), ("Drawn Date", "2026-10-02")]);
    a.viewports.push(vp("GF Architectural", 100));
    s.add_sheet(a).unwrap();
    let mut m = Sheet::new("M-101", "Étage 0 – ventilation", "Generic A1");
    m.params = params(&[("Drawn By", "KS")]);
    m.viewports.push(vp("GF Mechanical", 100));
    m.clouds.push(Cloud { revision: 1, outline: vec![[100.0, 100.0], [200.0, 100.0], [200.0, 160.0], [100.0, 160.0]] });
    m.clouds.push(Cloud { revision: 2, outline: vec![[300.0, 100.0], [380.0, 100.0], [380.0, 150.0]] });
    s.add_sheet(m).unwrap();
    let f3 = s.titleblock("Generic A3").unwrap().free_rect;
    let mut c = Sheet::new("C-001", "Coordination overview", "Generic A3");
    c.viewports.push(Viewport { view: "GF Coordination".into(), centre: f3.centre(), size: [f3.width() - 10.0, f3.height() - 20.0], scale: 200, model_centre: [10.0, 6.0], title: Some("Overview".into()) });
    s.add_sheet(c).unwrap();
    s
}

pub fn ctx() -> PrintContext {
    PrintContext { plot_date: "2026-10-15".into(), file_path: "office.obim".into() }
}

pub fn filters_lib() -> ViewLibrary {
    let mut l = library();
    l.add_filter(Filter { name: "Fire rated".into(), categories: vec!["Walls".into(), "Doors".into()], rules: vec![Rule { param: "Fire Rating".into(), op: Op::Greater, value: "30".into() }] }).unwrap();
    l.add_filter(Filter { name: "Large ducts".into(), categories: vec!["Ducts".into()], rules: vec![Rule { param: "Width".into(), op: Op::Greater, value: "400".into() }] }).unwrap();
    l
}
