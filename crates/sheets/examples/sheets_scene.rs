//! Writes the office sheet set as PDF and DXF into the folder given as the first argument and prints the
//! facts an independent checker needs (spikes/s13_sheets_check/check_sheets.py).
//! Usage: cargo run -q --release -p openbim-sheets --example sheets_scene -- OUTDIR

use std::collections::BTreeMap;

use openbim_sheets::dxf::write_dxf;
use openbim_sheets::import::expected_columns;
use openbim_sheets::model::{Cloud, Params, PrintContext, Sheet, SheetSet, Viewport};
use openbim_sheets::paper::{Margins, PaperSize};
use openbim_sheets::pdf::write_pdf;
use openbim_sheets::render::{render_all, LineWeightTable, ModelPrim, Prim, ViewContents};
use openbim_sheets::titleblock::TitleBlockDef;
use openbim_sheets::visibility::{Category, CategoryKind, Discipline, Override, ViewLibrary, ViewSettings};

fn p(kv: &[(&str, &str)]) -> Params {
    kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn mp(a: [f64; 2], b: [f64; 2], cat: &str, w: u8) -> ModelPrim {
    ModelPrim { a, b, category: cat.into(), params: Params::new(), link: None, weight: w }
}

fn main() {
    let out = std::env::args().nth(1).expect("output folder");
    std::fs::create_dir_all(&out).unwrap();
    let m = Margins { left: 20.0, top: 10.0, right: 10.0, bottom: 10.0 };
    let mut set = SheetSet::new(p(&[("Project Name", "Office Tower"), ("Facility", "Block B")])).unwrap();
    set.add_titleblock(TitleBlockDef::generic("Generic A1", PaperSize::iso_a(1).unwrap(), m).unwrap()).unwrap();
    set.add_titleblock(TitleBlockDef::generic("Generic A3", PaperSize::iso_a(3).unwrap(), m).unwrap()).unwrap();
    let r1 = set.add_revision("2026-10-01", "Issued for tender").unwrap();
    set.add_revision("2026-10-10", "Duct reroute").unwrap();
    set.issue_revision(r1).unwrap();

    let mut lib = ViewLibrary::new();
    for (n, d) in [
        ("Walls", Discipline::Architectural),
        ("Doors", Discipline::Architectural),
        ("Columns", Discipline::Structural),
        ("Ducts", Discipline::Mechanical),
        ("Pipes", Discipline::Plumbing),
        ("Cable Trays", Discipline::Electrical),
        ("Sprinklers", Discipline::FireProtection),
        ("Grids", Discipline::Coordination),
    ] {
        lib.add_category(Category { name: n.into(), discipline: d, kind: CategoryKind::Model }).unwrap();
    }
    let mut va = ViewSettings::new("GF Architectural", Discipline::Architectural);
    va.categories.insert("Grids".into(), Override { visible: Some(true), ..Default::default() });
    lib.add_view(va).unwrap();
    lib.add_view(ViewSettings::new("GF Mechanical", Discipline::Mechanical)).unwrap();
    lib.add_view(ViewSettings::new("GF Coordination", Discipline::Coordination)).unwrap();

    let mut lines = Vec::new();
    let c = [[0.0, 0.0], [20.0, 0.0], [20.0, 12.0], [0.0, 12.0]];
    for i in 0..4 {
        lines.push(mp(c[i], c[(i + 1) % 4], "Walls", 8));
    }
    lines.push(mp([10.0, 0.0], [10.0, 12.0], "Doors", 5));
    lines.push(mp([5.0, 5.0], [5.4, 5.0], "Columns", 9));
    lines.push(mp([2.0, 3.0], [18.0, 3.0], "Ducts", 6));
    lines.push(mp([2.0, 6.0], [18.0, 6.0], "Pipes", 4));
    lines.push(mp([2.0, 9.0], [18.0, 9.0], "Cable Trays", 4));
    lines.push(mp([2.0, 10.5], [18.0, 10.5], "Sprinklers", 3));
    lines.push(mp([-3.0, 6.0], [23.0, 6.0], "Grids", 2));
    // a line that runs far outside every frame, to see clipping in the output
    lines.push(mp([-400.0, 1.0], [400.0, 1.0], "Grids", 2));
    let mut contents = ViewContents::new();
    for v in ["GF Architectural", "GF Mechanical", "GF Coordination"] {
        contents.insert(v.to_string(), lines.clone());
    }

    let f1 = set.titleblock("Generic A1").unwrap().free_rect;
    let f3 = set.titleblock("Generic A3").unwrap().free_rect;
    let vp = |v: &str, f: openbim_sheets::paper::Rect, scale: u32, title: Option<&str>| Viewport {
        view: v.into(),
        centre: f.centre(),
        size: [f.width() - 20.0, f.height() - 20.0],
        scale,
        model_centre: [10.0, 6.0],
        title: title.map(String::from),
    };
    let mut a = Sheet::new("A-101", "Ground floor plan", "Generic A1");
    a.params = p(&[("Drawn By", "WD"), ("Drawn Date", "2026-10-02")]);
    a.viewports.push(vp("GF Architectural", f1, 100, None));
    set.add_sheet(a).unwrap();
    let mut mm = Sheet::new("M-101", "Étage 0 – ventilation", "Generic A1");
    mm.params = p(&[("Drawn By", "KS")]);
    mm.viewports.push(vp("GF Mechanical", f1, 100, None));
    mm.clouds.push(Cloud { revision: 1, outline: vec![[100.0, 100.0], [200.0, 100.0], [200.0, 160.0], [100.0, 160.0]] });
    mm.clouds.push(Cloud { revision: 2, outline: vec![[300.0, 100.0], [380.0, 100.0], [380.0, 150.0]] });
    set.add_sheet(mm).unwrap();
    let mut cc = Sheet::new("C-001", "Coordination overview", "Generic A3");
    cc.viewports.push(vp("GF Coordination", f3, 200, Some("Overview")));
    set.add_sheet(cc).unwrap();

    let lw = LineWeightTable::uniform(vec![0.05, 0.09, 0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1.0, 1.4, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0]).unwrap();
    let ctx = PrintContext { plot_date: "2026-10-15".into(), file_path: "office.obim".into() };
    let sheets = render_all(&set, &lib, &contents, &lw, &ctx).unwrap();
    let pdf = write_pdf(&sheets, "Office set");
    std::fs::write(format!("{out}/office.pdf"), &pdf.bytes).unwrap();
    for s in &sheets {
        std::fs::write(format!("{out}/{}.dxf", s.number), write_dxf(s)).unwrap();
    }

    for w in &pdf.warnings {
        println!("WARNING {w}");
    }
    for s in sheets.iter() {
        let mut per_layer: BTreeMap<String, usize> = BTreeMap::new();
        let (mut texts, mut lines_n) = (0, 0);
        for pr in &s.prims {
            match pr {
                Prim::Line { layer, .. } => {
                    lines_n += 1;
                    *per_layer.entry(layer.clone()).or_default() += 1;
                }
                Prim::Text { .. } => texts += 1,
            }
        }
        println!("SHEET\t{}\t{}\t{}\t{}\t{}\t{}", s.number, s.name, s.size.width_mm, s.size.height_mm, lines_n, texts);
        for (l, n) in per_layer {
            println!("LAYER\t{}\t{}\t{}", s.number, l, n);
        }
    }
    for sh in set.sheets() {
        for v in &sh.viewports {
            println!("VIEWPORT\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", sh.number, v.view, v.centre[0], v.centre[1], v.size[0], v.size[1], v.scale, v.model_centre[0], v.model_centre[1], 0);
        }
    }
    println!("COLUMNS\t{}", expected_columns(&set).join("\t"));
}
