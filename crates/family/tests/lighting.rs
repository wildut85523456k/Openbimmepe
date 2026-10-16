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
