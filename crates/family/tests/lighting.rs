use openbim_family::light::*;
use openbim_family::*;

fn lum(l: LightingDef) -> Family {
    Family { name: "L".into(), category: Category::LightFixture, lighting: Some(l), ..Default::default() }
}
fn full() -> LightingDef {
    LightingDef { photometry: Some("a.ies".into()), flux_lm: Some(2000.0), power_w: Some(20.0), cct_k: Some(3000.0), cri: Some(90.0), dimming: Some(1.0), ..Default::default() }
}
fn why(l: LightingDef) -> String {
    match lum(l).compile() {
        Err(FamilyError::BadMeta(w)) => w.to_string(),
        Err(FamilyError::BadText { why, .. }) => why.to_string(),
        other => panic!("{:?}", other.map(|_| ())),
    }
}

#[test]
fn a_luminaire_family_keeps_its_lighting_data() {
    let c = lum(full()).compile().unwrap();
    assert_eq!(c.lighting().unwrap().flux_lm, Some(2000.0));
    assert!(c.lighting().unwrap().missing_for_calculation().is_empty());
    // a family without the data reports it
    let bare = lum(LightingDef::default()).compile().unwrap();
    assert_eq!(bare.lighting().unwrap().missing_for_calculation().len(), 3);
    // manufacturer and article can stand in for the photometric file
    let by_catalogue = LightingDef { manufacturer: Some("M".into()), article: Some("1".into()), flux_lm: Some(1.0), power_w: Some(1.0), ..Default::default() };
    assert!(by_catalogue.missing_for_calculation().is_empty());
    assert_eq!(Mounting::from_name("suspended"), Some(Mounting::Suspended));
    assert_eq!(Mounting::Free.ifc(), "FREESTANDING");
}

#[test]
fn lighting_data_is_checked() {
    let f = full();
    assert!(why(LightingDef { flux_lm: Some(0.0), ..f.clone() }).contains("flux"));
    assert!(why(LightingDef { power_w: Some(f64::NAN), ..f.clone() }).contains("power"));
    assert!(why(LightingDef { cct_k: Some(-1.0), ..f.clone() }).contains("temperature"));
    assert!(why(LightingDef { cri: Some(101.0), ..f.clone() }).contains("rendering"));
    assert!(why(LightingDef { dimming: Some(1.5), ..f.clone() }).contains("dimming"));
    assert!(why(LightingDef { photometry: Some("a.txt".into()), ..f.clone() }).contains(".ies"));
    assert!(why(LightingDef { photometry: Some("dir/a.ies".into()), ..f.clone() }).contains("folder"));
    assert!(why(LightingDef { article: Some("  ".into()), ..f.clone() }).contains("empty"));
    assert!(why(LightingDef { manufacturer: Some("a\nb".into()), ..f }).contains("control"));
    // lighting data on a family that is not a luminaire
    let not = Family { name: "D".into(), category: Category::Door, lighting: Some(full()), ..Default::default() };
    assert!(matches!(not.compile(), Err(FamilyError::BadMeta(_))));
}

#[test]
fn a_photometric_file_fills_the_lighting_data() {
    use std::f64::consts::PI;
    let v: Vec<String> = (0..=6).map(|k| (k * 30).to_string()).collect();
    let file = format!("IESNA:LM-63-2002\n[MANUFAC] Maker\n[LUMCAT] ART-9\n[LUMINAIRE] Light\nTILT=NONE\n1 -1 1 7 1 1 2 0.6 0.6 0\n1 1 40\n{}\n0\n{}\n", v.join(" "), ["100"; 7].join(" "));
    let p = openbim_photo::parse(file.as_bytes(), "a.ies").unwrap();
    let base = LightingDef { ip: Some("IP44".into()), dimming: Some(0.5), mounting: Mounting::Suspended, cct_k: Some(3000.0), ..Default::default() };
    let l = LightingDef::from_photometry(&p, "a.ies", base);
    assert_eq!((l.photometry.as_deref(), l.manufacturer.as_deref(), l.article.as_deref()), (Some("a.ies"), Some("Maker"), Some("ART-9")));
    assert!((l.flux_lm.unwrap() - 400.0 * PI).abs() < 1e-6 && l.power_w == Some(40.0));
    // what the file does not hold stays as the author typed it
    assert_eq!((l.ip.as_deref(), l.dimming, l.mounting, l.cct_k), (Some("IP44"), Some(0.5), Mounting::Suspended, Some(3000.0)));
    assert!(lum(l).compile().is_ok());
}

fn ies_file(cd: f64, watts: f64, maker: &str) -> Vec<u8> {
    let v: Vec<String> = (0..=6).map(|k| (k * 30).to_string()).collect();
    format!("IESNA:LM-63-2002\n[MANUFAC] {maker}\n[LUMCAT] ART-{watts}\nTILT=NONE\n1 -1 1 7 1 1 2 0.6 0.6 0\n1 1 {watts}\n{}\n0\n{}\n", v.join(" "), vec![cd.to_string(); 7].join(" ")).into_bytes()
}

fn zip_of(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    use std::io::Write;
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (n, b) in files {
        w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(b).unwrap();
    }
    w.finish().unwrap().into_inner()
}

#[test]
fn each_photometric_variant_becomes_a_type_with_its_own_lighting() {
    use std::f64::consts::PI;
    let z = zip_of(&[("panel/Panel 3000K.ies", ies_file(100.0, 30.0, "Maker")), ("panel/Panel 4000K.IES", ies_file(120.0, 32.0, "Maker")), ("panel/readme.txt", b"hi".to_vec()), ("panel/broken.ies", b"nonsense".to_vec())]);
    let imports = openbim_photo::import(&z, "panel.zip");
    assert_eq!(imports.len(), 3);
    let mut f = lum(LightingDef { mounting: Mounting::Surface, ip: Some("IP20".into()), ..Default::default() });
    let r = types_from_imports(&mut f, &imports).unwrap();
    assert_eq!(r.added.iter().map(|a| (a.type_name.as_str(), a.file_name.as_str(), a.import)).collect::<Vec<_>>(), vec![("Panel 3000K", "Panel 3000K.ies", 0), ("Panel 4000K", "Panel 4000K.IES", 1)]);
    assert_eq!(r.warnings.len(), 1);
    assert!(r.warnings[0].starts_with("broken.ies: not used"), "{:?}", r.warnings);
    let c = f.compile().unwrap();
    assert_eq!(c.type_names(), vec!["Panel 3000K", "Panel 4000K"]);
    let a = c.lighting_for(Some("Panel 3000K")).unwrap();
    let b = c.lighting_for(Some("Panel 4000K")).unwrap();
    assert!((a.flux_lm.unwrap() - 4.0 * PI * 100.0).abs() < 1e-6 && (b.flux_lm.unwrap() - 4.0 * PI * 120.0).abs() < 1e-6);
    assert!(a.flux_lm.unwrap() < b.flux_lm.unwrap() && a.power_w == Some(30.0) && b.power_w == Some(32.0));
    assert_eq!((a.photometry.as_deref(), b.photometry.as_deref()), (Some("Panel 3000K.ies"), Some("Panel 4000K.IES")));
    // what files do not state comes from the family
    assert_eq!((a.mounting, a.ip.as_deref()), (Mounting::Surface, Some("IP20")));
    // no type, or a type without data of its own: the family's data
    assert_eq!(c.lighting_for(None).unwrap().power_w, None);
    assert_eq!(c.lighting_for(Some("nope")).unwrap().power_w, None);
}

#[test]
fn variant_names_that_are_taken_get_a_number_and_a_bad_family_is_left_alone() {
    let z = zip_of(&[("a/P.ies", ies_file(100.0, 30.0, "M")), ("b/P.ies", ies_file(110.0, 31.0, "M"))]);
    let imports = openbim_photo::import(&z, "z.zip");
    let mut f = lum(LightingDef::default());
    f.types.push(TypeDef { name: "P".into(), ..Default::default() });
    let r = types_from_imports(&mut f, &imports).unwrap();
    assert_eq!(r.added.iter().map(|a| (a.type_name.as_str(), a.file_name.as_str())).collect::<Vec<_>>(), vec![("P (2)", "P.ies"), ("P (3)", "P (2).ies")]);
    assert_eq!(r.warnings.len(), 2);
    assert_eq!(f.types.len(), 3);
    let c = f.compile().unwrap();
    assert_eq!(c.lighting_for(Some("P (3)")).unwrap().photometry.as_deref(), Some("P (2).ies"));
    // a family that is not a luminaire cannot hold lighting types
    let mut door = Family { name: "D".into(), category: Category::Door, ..Default::default() };
    door.types.push(TypeDef { name: "x".into(), lighting: Some(LightingDef::default()), ..Default::default() });
    assert!(matches!(door.compile(), Err(FamilyError::BadMeta(_))));
    // a type's lighting is checked like the family's
    let mut bad = lum(LightingDef::default());
    bad.types.push(TypeDef { name: "x".into(), lighting: Some(LightingDef { flux_lm: Some(-1.0), ..Default::default() }), ..Default::default() });
    assert!(matches!(bad.compile(), Err(FamilyError::BadMeta(_))));
    // nothing readable: no types, only the warnings
    let junk = openbim_photo::import(&zip_of(&[("x.ies", b"junk".to_vec())]), "z.zip");
    let mut g = lum(LightingDef::default());
    let r = types_from_imports(&mut g, &junk).unwrap();
    assert!(r.added.is_empty() && r.warnings.len() == 1 && g.types.is_empty());
}
