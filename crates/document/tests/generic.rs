//! Generic models: what a file holds that the app has no element for, kept as a prism with the file's words.

use openbim_document::{native, Category, Command, Document};

fn rect(x: f64, y: f64, w: f64, d: f64) -> Vec<[f64; 2]> { vec![[x, y], [x + w, y], [x + w, y + d], [x, y + d]] }
fn texts() -> Vec<(String, String)> { vec![("IfcClass".into(), "IFCPUMP".into()), ("PredefinedType".into(), "CIRCULATOR".into()), ("Pset_PumpCommon.Reference".into(), "P-01".into())] }

#[test]
fn a_generic_model_is_a_prism_on_a_level_that_keeps_what_the_source_said() {
    let mut d = Document::new();
    let l = d.place_level("Level 1", 0.0).unwrap();
    let id = d.place_generic(l, "Pump 1", rect(1.0, 1.0, 1.2, 0.8), 0.1, 0.9, texts()).unwrap();
    let e = d.get(id).unwrap();
    assert_eq!(e.category, Category::Generic);
    // its top is 1.0 above the level, and it is 0.9 tall: it stands from 0.1
    assert!((e.elevation_m - 1.0).abs() < 1e-12 && (e.height_m - 0.9).abs() < 1e-12 && (e.width_m - 0.9).abs() < 1e-12);
    assert_eq!(e.texts, texts());
    let m = d.mesh(id).unwrap();
    let (lo, hi) = m.vertices.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v[2]), b.max(v[2])));
    assert!((lo - 0.1).abs() < 1e-9 && (hi - 1.0).abs() < 1e-9, "{lo} {hi}");
}

#[test]
fn a_generic_model_without_a_shape_or_with_too_much_text_is_refused() {
    let mut d = Document::new();
    let l = d.place_level("Level 1", 0.0).unwrap();
    assert!(d.place_generic(l, "No height", rect(0.0, 0.0, 1.0, 1.0), 0.0, 0.0, vec![]).is_err());
    assert!(d.place_generic(l, "Too tall", rect(0.0, 0.0, 1.0, 1.0), 0.0, 250.0, vec![]).is_err());
    assert!(d.place_generic(l, "Two points", vec![[0.0, 0.0], [1.0, 0.0]], 0.0, 1.0, vec![]).is_err());
    assert!(d.place_generic(l, "Bow tie", vec![[0.0, 0.0], [1.0, 1.0], [1.0, 0.0], [0.0, 1.0]], 0.0, 1.0, vec![]).is_err());
    let many: Vec<(String, String)> = (0..65).map(|i| (format!("k{i}"), "v".into())).collect();
    assert!(d.place_generic(l, "Many", rect(0.0, 0.0, 1.0, 1.0), 0.0, 1.0, many).is_err());
    assert!(d.place_generic(l, "Long", rect(0.0, 0.0, 1.0, 1.0), 0.0, 1.0, vec![("k".into(), "v".repeat(257))]).is_err());
    assert_eq!(d.elements().filter(|e| e.category == Category::Generic).count(), 0, "a refused one leaves nothing behind");
    // 64 entries of 256 bytes fit
    let ok: Vec<(String, String)> = (0..64).map(|i| (format!("k{i}"), "v".repeat(256))).collect();
    assert!(d.place_generic(l, "Full", rect(0.0, 0.0, 1.0, 1.0), 0.0, 1.0, ok).is_ok());
}

#[test]
fn a_generic_model_takes_no_family_and_keeps_to_its_own_rules_when_edited() {
    let mut d = Document::new();
    let l = d.place_level("Level 1", 0.0).unwrap();
    let id = d.place_generic(l, "Pump", rect(0.0, 0.0, 1.0, 1.0), 0.0, 1.0, texts()).unwrap();
    assert!(d.transact(d.revision(), &[Command::SetFamily { id, family: "Door".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0 }]).is_err());
    assert!(d.transact(d.revision(), &[Command::SetMaterial { id, material: openbim_document::Material::Concrete }]).is_err());
    // its outline and its texts can be edited
    d.transact(d.revision(), &[Command::SetOutline { id, outline: rect(0.0, 0.0, 2.0, 1.0) }, Command::SetFamilyTexts { id, texts: vec![("IfcClass".into(), "IFCTANK".into())] }]).unwrap();
    assert_eq!(d.get(id).unwrap().texts, vec![("IfcClass".to_string(), "IFCTANK".to_string())]);
    // a slab does not take texts
    let slab = d.place_slab(l, &openbim_document::SlabSpec { outline: rect(5.0, 5.0, 2.0, 2.0), thickness_m: 0.2, top_offset_m: 0.0, kind: openbim_document::SlabKind::Floor, material: openbim_document::Material::Unspecified }).unwrap();
    assert!(d.transact(d.revision(), &[Command::SetFamilyTexts { id: slab, texts: texts() }]).is_err());
}

#[test]
fn a_generic_model_is_saved_and_read_back_whole() {
    let mut d = Document::new();
    let l = d.place_level("Level 1", 0.0).unwrap();
    let id = d.place_generic(l, "Pump | 1; a=b", rect(1.0, 1.0, 1.2, 0.8), 0.1, 0.9, texts()).unwrap();
    let lines = native::records(&d);
    let (elements, _) = native::parse_records(&lines).unwrap();
    let mut back = Document::new();
    back.transact(0, &native::restore_commands(elements, None)).unwrap();
    let (a, b) = (d.get(id).unwrap(), back.get(id).unwrap());
    assert_eq!((a.category, &a.name, &a.outline, a.elevation_m, a.height_m, &a.texts, a.geometry), (b.category, &b.name, &b.outline, b.elevation_m, b.height_m, &b.texts, b.geometry));
}
