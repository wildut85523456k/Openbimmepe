mod common;
use common::*;
use openbim_sheets::dxf::{aci, write_dxf};
use openbim_sheets::model::Sheet;
use openbim_sheets::pdf::{text_width_mm, write_pdf};
use openbim_sheets::render::*;
use openbim_sheets::SheetError;

fn all() -> Vec<RenderedSheet> {
    render_all(&office_set(), &views(), &contents(), &lw(), &ctx()).unwrap()
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

#[test]
fn revision_cloud_bulges_outward_for_either_winding() {
    let sq = vec![[100.0, 100.0], [200.0, 100.0], [200.0, 160.0], [100.0, 160.0]];
    let mut rev = sq.clone();
    rev.reverse();
    for outline in [sq, rev] {
        let pts = cloud_polyline(&outline, 8.0);
        // edges of 100, 60, 100 and 60 mm are cut into 13, 8, 13 and 8 bumps of 8 points each, plus the start
        assert_eq!(pts.len(), 42 * 8 + 1);
        let (f, l) = (pts[0], pts[pts.len() - 1]);
        assert!((f[0] - l[0]).abs() < 1e-9 && (f[1] - l[1]).abs() < 1e-9, "the outline closes");
        for p in &pts {
            let strictly_inside = p[0] > 100.0 + 1e-9 && p[0] < 200.0 - 1e-9 && p[1] > 100.0 + 1e-9 && p[1] < 160.0 - 1e-9;
            assert!(!strictly_inside, "{p:?} bulges inwards");
        }
        let top = pts.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
        assert!(top > 160.0 + 1.0 && top < 160.0 + 4.5, "a 60/8 = 7.5 mm chord bulges by half its length, got {top}");
    }
}

#[test]
fn revision_table_lists_oldest_first_and_tags_sit_in_the_clouds() {
    let s = render_sheet(&office_set(), &views(), &contents(), &lw(), "M-101", &ctx()).unwrap();
    let t: Vec<_> = s
        .prims
        .iter()
        .filter_map(|p| if let Prim::Text { at, text, .. } = p { Some((*at, text.clone())) } else { None })
        .collect();
    let at = |text: &str| t.iter().find(|x| x.1 == text).unwrap_or_else(|| panic!("no '{text}'")).0;
    // table origin (651, 80), rows 5 mm: first row text baseline 80 + 5 + 1.4, description column starts 54 mm in
    assert_eq!(at("Issued for tender"), [706.0, 86.4]);
    assert_eq!(at("Duct reroute"), [706.0, 91.4]);
    assert_eq!(at("2026-10-01"), [670.0, 86.4]);
    // header row at the bottom
    assert_eq!(at("Description"), [706.0, 81.4]);
    // tag numbers at the cloud centroids: (150, 130) and the triangle's (360, 116.67)
    let tags: Vec<_> = t.iter().filter(|x| (x.1 == "1" || x.1 == "2") && x.0[1] > 100.0 && x.0[1] < 140.0 && x.0[0] < 600.0).collect();
    assert_eq!(tags.len(), 2);
    assert!(tags.iter().any(|x| x.1 == "1" && (x.0[0] - 150.0).abs() < 1e-9 && (x.0[1] - 128.3).abs() < 1e-9));
    assert!(s.prims.iter().any(|p| matches!(p, Prim::Line { layer, .. } if layer == "REVISION")));
}

#[test]
fn a_full_revision_table_reports_what_does_not_fit() {
    let mut set = office_set();
    for i in 0..8 {
        set.add_revision("2026-11-01", &format!("extra {i}")).unwrap();
    }
    let mut m = set.sheet("M-101").unwrap().clone();
    for seq in 3..=10 {
        m.clouds.push(openbim_sheets::model::Cloud { revision: seq, outline: vec![[100.0, 200.0], [150.0, 200.0], [150.0, 230.0]] });
    }
    set.replace_sheet(m).unwrap();
    let s = render_sheet(&set, &views(), &contents(), &lw(), "M-101", &ctx()).unwrap();
    assert!(s.warnings.iter().any(|w| w.contains("4 older revisions")), "{:?}", s.warnings);
    let has = |d: &str| s.prims.iter().any(|p| matches!(p, Prim::Text { text, .. } if text == d));
    assert!(!has("Issued for tender") && has("extra 7"), "the table keeps the newest six");
}

#[test]
fn missing_fields_are_reported_not_invented() {
    let a = render_sheet(&office_set(), &views(), &contents(), &lw(), "A-101", &ctx()).unwrap();
    let mut miss: Vec<String> = a.warnings.iter().filter_map(|w| w.strip_prefix("sheet A-101: field '")).filter_map(|w| w.strip_suffix("' has no value")).map(String::from).collect();
    miss.sort();
    assert_eq!(miss, vec!["Approval Date", "Approved By", "Area", "Check Date", "Checked By", "Design Date", "Designed By", "Drawing Type"]);
}

#[test]
fn unknown_sheet_view_and_missing_content_are_handled() {
    let set = office_set();
    assert!(matches!(render_sheet(&set, &views(), &contents(), &lw(), "ZZ", &ctx()), Err(SheetError::Unknown { .. })));
    let mut c = contents();
    c.remove("GF Mechanical");
    let s = render_sheet(&set, &views(), &c, &lw(), "M-101", &ctx()).unwrap();
    assert!(s.warnings.iter().any(|w| w.contains("has no content")));
    let mut set2 = office_set();
    let fr = set2.titleblock("Generic A1").unwrap().free_rect;
    let mut sh = Sheet::new("N-1", "n", "Generic A1");
    sh.viewports.push(openbim_sheets::model::Viewport { view: "Undefined view".into(), centre: fr.centre(), size: [50.0, 50.0], scale: 100, model_centre: [0.0, 0.0], title: None });
    set2.add_sheet(sh).unwrap();
    assert!(render_sheet(&set2, &views(), &contents(), &lw(), "N-1", &ctx()).is_err());
}

#[test]
fn text_widths_come_from_the_helvetica_metrics() {
    assert!((text_width_mm("Hello", 10.0, false) - 22.78).abs() < 1e-9);
    assert!((text_width_mm("Hello", 10.0, true) - 24.45).abs() < 1e-9);
    assert_eq!(text_width_mm("é", 10.0, false), text_width_mm("e", 10.0, false));
    assert_eq!(text_width_mm("", 10.0, false), 0.0);
    assert!((text_width_mm("日", 10.0, false) - 5.56).abs() < 1e-9, "anything unknown counts as 556");
}

#[test]
fn pdf_structure_pages_and_sizes() {
    let sheets = all();
    let out = write_pdf(&sheets, "Office set");
    let b = &out.bytes;
    assert!(b.starts_with(b"%PDF-1.4\n"));
    assert!(b.ends_with(b"%%EOF\n"));
    // startxref points at the table, and every entry points at its object
    let sx = find(b, b"startxref\n").unwrap() + 10;
    let off: usize = std::str::from_utf8(&b[sx..sx + 10]).unwrap().lines().next().unwrap().parse().unwrap();
    assert!(b[off..].starts_with(b"xref\n0 12\n"), "5 fixed objects + 2 per sheet + the free entry");
    let table = &b[off + 10..];
    for n in 0..12usize {
        let line = std::str::from_utf8(&table[n * 20..n * 20 + 20]).unwrap();
        if n == 0 {
            assert_eq!(line, "0000000000 65535 f \n");
            continue;
        }
        let o: usize = line[..10].parse().unwrap();
        assert!(b[o..].starts_with(format!("{n} 0 obj\n").as_bytes()), "object {n}");
    }
    // three pages with true paper sizes in points
    assert!(find(b, b"/Count 3").is_some());
    assert!(find(b, b"/MediaBox [0 0 2383.937 1683.78]").is_some(), "A1 landscape");
    assert!(find(b, b"/MediaBox [0 0 1190.551 841.89]").is_some(), "A3 landscape");
    // every line is one stroke, every text one Tj
    let lines: usize = sheets.iter().map(|s| s.prims.iter().filter(|p| matches!(p, Prim::Line { .. })).count()).sum();
    let texts: usize = sheets.iter().map(|s| s.prims.iter().filter(|p| matches!(p, Prim::Text { .. })).count()).sum();
    assert_eq!(b.windows(3).filter(|w| w == b" S\n").count(), lines);
    assert_eq!(b.windows(8).filter(|w| w == b") Tj ET\n").count(), texts);
    // WinAnsi: É is C9 and the en dash is 96
    assert!(find(b, b"(\xC9tage 0 \x96 ventilation) Tj").is_some());
    assert!(out.warnings.iter().all(|w| !w.contains("printed as '?'")), "{:?}", out.warnings);
}

#[test]
fn pdf_escapes_text_and_reports_unprintable_characters() {
    let mut sheets = all();
    sheets.truncate(1);
    sheets[0].prims.push(Prim::Text { at: [50.0, 50.0], height: 3.0, align: openbim_sheets::titleblock::Align::Left, text: "a(b)c\\ 日本".into(), bold: false, layer: "X".into() });
    let out = write_pdf(&sheets, "T (1)");
    assert!(find(&out.bytes, b"(a\\(b\\)c\\\\ ??) Tj").is_some());
    assert!(find(&out.bytes, b"/Title (T \\(1\\))").is_some());
    assert!(out.warnings.iter().any(|w| w.contains("printed as '?'") && w.contains('日')));
}

#[test]
fn pdf_alignment_uses_the_text_width() {
    let mut sheets = all();
    sheets.truncate(1);
    sheets[0].prims.clear();
    for (al, name) in [(openbim_sheets::titleblock::Align::Centre, "c"), (openbim_sheets::titleblock::Align::Right, "r"), (openbim_sheets::titleblock::Align::Left, "l")] {
        sheets[0].prims.push(Prim::Text { at: [100.0, 50.0], height: 10.0, align: al, text: "Hello".into(), bold: false, layer: name.into() });
    }
    let s = String::from_utf8_lossy(&write_pdf(&sheets, "t").bytes).to_string();
    let pt = |mm: f64| format!("{:.3}", mm * 72.0 / 25.4).trim_end_matches('0').trim_end_matches('.').to_string();
    // width 22.78 mm: centre starts 11.39 mm left of 100, right 22.78 mm left
    assert!(s.contains(&format!("{} {} Td (Hello)", pt(100.0 - 11.39), pt(50.0))));
    assert!(s.contains(&format!("{} {} Td (Hello)", pt(100.0 - 22.78), pt(50.0))));
    assert!(s.contains(&format!("{} {} Td (Hello)", pt(100.0), pt(50.0))));
}

#[test]
fn dxf_is_complete_and_layered_by_weight() {
    let sheets = all();
    let s = &sheets[0];
    let d = write_dxf(s);
    assert!(d.starts_with("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n"));
    assert!(d.ends_with("0\nENDSEC\n0\nEOF\n"));
    let n_lines = s.prims.iter().filter(|p| matches!(p, Prim::Line { .. })).count();
    let n_texts = s.prims.iter().filter(|p| matches!(p, Prim::Text { .. })).count();
    assert_eq!(d.matches("\n0\nLINE\n").count(), n_lines);
    assert_eq!(d.matches("\n0\nTEXT\n").count(), n_texts);
    // walls have weight number 8 = 0.70 mm
    assert!(d.contains("\n8\nA-WALLS_W070\n"));
    assert!(d.contains("\n2\nA-WALLS_W070\n70\n0\n62\n7\n"), "layer table entry with colour 7");
    assert!(d.contains("\n2\nS-COLUMNS_W100\n70\n0\n62\n8\n"), "halftone is grey 8");
    // the layer count in the table header matches the entries
    let declared: usize = d.split("0\nTABLE\n2\nLAYER\n70\n").nth(1).unwrap().lines().next().unwrap().parse().unwrap();
    assert_eq!(d.matches("0\nLAYER\n2\n").count(), declared);
    // right aligned text carries justification 2 and its alignment point
    assert!(d.contains("72\n2\n11\n830.0000\n21\n58.0000\n"));
    assert!(d.contains("72\n1\n11\n"));
}

#[test]
fn colour_index_mapping() {
    assert_eq!(aci([0, 0, 0]), 7);
    assert_eq!(aci([127, 127, 127]), 8);
    assert_eq!(aci([128, 128, 128]), 8);
    assert_eq!(aci([200, 200, 200]), 9);
    assert_eq!(aci([255, 0, 0]), 1);
    assert_eq!(aci([250, 240, 10]), 2);
    assert_eq!(aci([10, 250, 10]), 3);
    assert_eq!(aci([0, 200, 200]), 4);
    assert_eq!(aci([10, 10, 250]), 5);
    assert_eq!(aci([250, 10, 250]), 6);
}

#[test]
fn rendering_is_deterministic() {
    let a = write_pdf(&all(), "t").bytes;
    let b = write_pdf(&all(), "t").bytes;
    assert_eq!(a, b);
}

#[test]
fn custom_orientation_reaches_titleblock_pdf_and_dxf() {
    use openbim_sheets::model::SheetSet;
    use openbim_sheets::paper::{Margins, PaperSize};
    use openbim_sheets::titleblock::TitleBlockDef;

    // All ISO sizes in both orientations, including the reported A0 portrait case.
    for (short, long) in [(841.0, 1189.0), (594.0, 841.0), (420.0, 594.0), (297.0, 420.0), (210.0, 297.0)] {
        for (width, height) in [(short, long), (long, short)] {
            let paper = PaperSize::custom("Custom", width, height).unwrap();
            let tb = TitleBlockDef::generic("Custom block", paper, Margins::all(10.0)).unwrap();
            assert_eq!((tb.page().width(), tb.page().height()), (width, height));
            tb.validate().unwrap();
            let mut set = SheetSet::default();
            set.add_titleblock(tb).unwrap();
            set.add_sheet(Sheet::new("P-001", "Orientation", "Custom block")).unwrap();
            let s = render_sheet(&set, &views(), &contents(), &lw(), "P-001", &ctx()).unwrap();
            assert_eq!((s.size.width_mm, s.size.height_mm), (width, height));
            // The rendered border must follow the same dimensions, not just page metadata.
            assert!(s.prims.iter().any(|p| matches!(p, Prim::Line { a, b, .. }
                if a[0] == width - 10.0 && b[0] == width - 10.0
                    && a[1].max(b[1]) == height - 10.0)));
            let pdf = write_pdf(std::slice::from_ref(&s), "Orientation");
            let pdf = String::from_utf8_lossy(&pdf.bytes);
            let pt = |mm: f64| format!("{:.3}", mm * 72.0 / 25.4).trim_end_matches('0').trim_end_matches('.').to_string();
            assert!(pdf.contains(&format!("/MediaBox [0 0 {} {}]", pt(width), pt(height))));
            let dxf = write_dxf(&s);
            assert!(dxf.contains(&format!("9\n$EXTMAX\n10\n{width:.4}\n20\n{height:.4}\n30\n0.0\n")));
            assert!(dxf.contains(&format!("10\n{:.4}\n20\n{:.4}\n", width - 10.0, height - 10.0)));
        }
    }
}
