use openbim_family::light::*;
use openbim_family::*;

fn lum(l: LightingDef) -> Family {
    Family {
        name: "L".into(),
        category: Category::LightFixture,
        lighting: Some(l),
        ..Default::default()
    }
}
fn full() -> LightingDef {
    LightingDef {
        photometry: Some("a.ies".into()),
        flux_lm: Some(2000.0),
        power_w: Some(20.0),
        cct_k: Some(3000.0),
        cri: Some(90.0),
        dimming: Some(1.0),
        ..Default::default()
    }
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
    let by_catalogue = LightingDef {
        manufacturer: Some("M".into()),
        article: Some("1".into()),
        flux_lm: Some(1.0),
        power_w: Some(1.0),
        ..Default::default()
    };
    assert!(by_catalogue.missing_for_calculation().is_empty());
    assert_eq!(Mounting::from_name("suspended"), Some(Mounting::Suspended));
    assert_eq!(Mounting::Free.ifc(), "FREESTANDING");
}

#[test]
fn lighting_data_is_checked() {
    let f = full();
    assert!(why(LightingDef {
        flux_lm: Some(0.0),
        ..f.clone()
    })
    .contains("flux"));
    assert!(why(LightingDef {
        power_w: Some(f64::NAN),
        ..f.clone()
    })
    .contains("power"));
    assert!(why(LightingDef {
        cct_k: Some(-1.0),
        ..f.clone()
    })
    .contains("temperature"));
    assert!(why(LightingDef {
        cri: Some(101.0),
        ..f.clone()
    })
    .contains("rendering"));
    assert!(why(LightingDef {
        dimming: Some(1.5),
        ..f.clone()
    })
    .contains("dimming"));
    assert!(why(LightingDef {
        photometry: Some("a.txt".into()),
        ..f.clone()
    })
    .contains(".ies"));
    assert!(why(LightingDef {
        photometry: Some("dir/a.ies".into()),
        ..f.clone()
    })
    .contains("folder"));
    assert!(why(LightingDef {
        article: Some("  ".into()),
        ..f.clone()
    })
    .contains("empty"));
    assert!(why(LightingDef {
        manufacturer: Some("a\nb".into()),
        ..f
    })
    .contains("control"));
    // lighting data on a family that is not a luminaire
    let not = Family {
        name: "D".into(),
        category: Category::Door,
        lighting: Some(full()),
        ..Default::default()
    };
    assert!(matches!(not.compile(), Err(FamilyError::BadMeta(_))));
}

#[test]
fn a_photometric_file_fills_the_lighting_data() {
    use std::f64::consts::PI;
    let v: Vec<String> = (0..=6).map(|k| (k * 30).to_string()).collect();
    let file = format!("IESNA:LM-63-2002\n[MANUFAC] Maker\n[LUMCAT] ART-9\n[LUMINAIRE] Light\nTILT=NONE\n1 -1 1 7 1 1 2 0.6 0.6 0\n1 1 40\n{}\n0\n{}\n", v.join(" "), ["100"; 7].join(" "));
    let p = openbim_photo::parse(file.as_bytes(), "a.ies").unwrap();
    let base = LightingDef {
        ip: Some("IP44".into()),
        dimming: Some(0.5),
        mounting: Mounting::Suspended,
        cct_k: Some(3000.0),
        ..Default::default()
    };
    let l = LightingDef::from_photometry(&p, "a.ies", base);
    assert_eq!(
        (
            l.photometry.as_deref(),
            l.manufacturer.as_deref(),
            l.article.as_deref()
        ),
        (Some("a.ies"), Some("Maker"), Some("ART-9"))
    );
    assert!((l.flux_lm.unwrap() - 400.0 * PI).abs() < 1e-6 && l.power_w == Some(40.0));
    // what the file does not hold stays as the author typed it
    assert_eq!(
        (l.ip.as_deref(), l.dimming, l.mounting, l.cct_k),
        (Some("IP44"), Some(0.5), Mounting::Suspended, Some(3000.0))
    );
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
        w.start_file(*n, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(b).unwrap();
    }
    w.finish().unwrap().into_inner()
}

#[test]
fn each_photometric_variant_becomes_a_type_with_its_own_lighting() {
    use std::f64::consts::PI;
    let z = zip_of(&[
        ("panel/Panel 3000K.ies", ies_file(100.0, 30.0, "Maker")),
        ("panel/Panel 4000K.IES", ies_file(120.0, 32.0, "Maker")),
        ("panel/readme.txt", b"hi".to_vec()),
        ("panel/broken.ies", b"nonsense".to_vec()),
    ]);
    let imports = openbim_photo::import(&z, "panel.zip");
    assert_eq!(imports.len(), 3);
    let mut f = lum(LightingDef {
        mounting: Mounting::Surface,
        ip: Some("IP20".into()),
        ..Default::default()
    });
    let r = types_from_imports(&mut f, &imports).unwrap();
    assert_eq!(
        r.added
            .iter()
            .map(|a| (a.type_name.as_str(), a.file_name.as_str(), a.import))
            .collect::<Vec<_>>(),
        vec![
            ("Panel 3000K", "Panel 3000K.ies", 0),
            ("Panel 4000K", "Panel 4000K.IES", 1)
        ]
    );
    assert_eq!(r.warnings.len(), 1);
    assert!(
        r.warnings[0].starts_with("broken.ies: not used"),
        "{:?}",
        r.warnings
    );
    let c = f.compile().unwrap();
    assert_eq!(c.type_names(), vec!["Panel 3000K", "Panel 4000K"]);
    let a = c.lighting_for(Some("Panel 3000K")).unwrap();
    let b = c.lighting_for(Some("Panel 4000K")).unwrap();
    assert!(
        (a.flux_lm.unwrap() - 4.0 * PI * 100.0).abs() < 1e-6
            && (b.flux_lm.unwrap() - 4.0 * PI * 120.0).abs() < 1e-6
    );
    assert!(
        a.flux_lm.unwrap() < b.flux_lm.unwrap()
            && a.power_w == Some(30.0)
            && b.power_w == Some(32.0)
    );
    assert_eq!(
        (a.photometry.as_deref(), b.photometry.as_deref()),
        (Some("Panel 3000K.ies"), Some("Panel 4000K.IES"))
    );
    // what files do not state comes from the family
    assert_eq!(
        (a.mounting, a.ip.as_deref()),
        (Mounting::Surface, Some("IP20"))
    );
    // no type, or a type without data of its own: the family's data
    assert_eq!(c.lighting_for(None).unwrap().power_w, None);
    assert_eq!(c.lighting_for(Some("nope")).unwrap().power_w, None);
}

#[test]
fn variant_names_that_are_taken_get_a_number_and_a_bad_family_is_left_alone() {
    let z = zip_of(&[
        ("a/P.ies", ies_file(100.0, 30.0, "M")),
        ("b/P.ies", ies_file(110.0, 31.0, "M")),
    ]);
    let imports = openbim_photo::import(&z, "z.zip");
    let mut f = lum(LightingDef::default());
    f.types.push(TypeDef {
        name: "P".into(),
        ..Default::default()
    });
    let r = types_from_imports(&mut f, &imports).unwrap();
    assert_eq!(
        r.added
            .iter()
            .map(|a| (a.type_name.as_str(), a.file_name.as_str()))
            .collect::<Vec<_>>(),
        vec![("P (2)", "P.ies"), ("P (3)", "P (2).ies")]
    );
    assert_eq!(r.warnings.len(), 2);
    assert_eq!(f.types.len(), 3);
    let c = f.compile().unwrap();
    assert_eq!(
        c.lighting_for(Some("P (3)")).unwrap().photometry.as_deref(),
        Some("P (2).ies")
    );
    // a family that is not a luminaire cannot hold lighting types
    let mut door = Family {
        name: "D".into(),
        category: Category::Door,
        ..Default::default()
    };
    door.types.push(TypeDef {
        name: "x".into(),
        lighting: Some(LightingDef::default()),
        ..Default::default()
    });
    assert!(matches!(door.compile(), Err(FamilyError::BadMeta(_))));
    // a type's lighting is checked like the family's
    let mut bad = lum(LightingDef::default());
    bad.types.push(TypeDef {
        name: "x".into(),
        lighting: Some(LightingDef {
            flux_lm: Some(-1.0),
            ..Default::default()
        }),
        ..Default::default()
    });
    assert!(matches!(bad.compile(), Err(FamilyError::BadMeta(_))));
    // nothing readable: no types, only the warnings
    let junk = openbim_photo::import(&zip_of(&[("x.ies", b"junk".to_vec())]), "z.zip");
    let mut g = lum(LightingDef::default());
    let r = types_from_imports(&mut g, &junk).unwrap();
    assert!(r.added.is_empty() && r.warnings.len() == 1 && g.types.is_empty());
}

fn png() -> Vec<u8> {
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend([0u8; 8]);
    v
}
fn jpg() -> Vec<u8> {
    vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 0]
}

#[test]
fn pictures_and_data_sheets_are_judged_by_their_content_and_their_names() {
    assert_eq!(attachment_kind(&png()), Some(AttachmentKind::Png));
    assert_eq!(attachment_kind(&jpg()), Some(AttachmentKind::Jpeg));
    assert_eq!(attachment_kind(b"%PDF-1.7\n"), Some(AttachmentKind::Pdf));
    assert_eq!(attachment_kind(b"%PDF"), None);
    assert_eq!(attachment_kind(&[0xFF, 0xD8]), None);
    assert_eq!(attachment_kind(b""), None);
    assert!(check_attachment("picture", "p.png", &png()).is_ok());
    assert!(check_attachment("picture", "P.JPG", &jpg()).is_ok());
    assert!(check_attachment("picture", "p.jpeg", &jpg()).is_ok());
    assert!(check_attachment("datasheet", "d.pdf", b"%PDF-1.4").is_ok());
    // a picture that is really a PDF, or named for the wrong kind
    assert!(check_attachment("picture", "p.png", b"%PDF-1.4")
        .unwrap_err()
        .contains("PNG or JPEG"));
    assert!(check_attachment("picture", "p.png", &jpg())
        .unwrap_err()
        .contains("does not match"));
    assert!(check_attachment("picture", "p.jpg", &png())
        .unwrap_err()
        .contains("does not match"));
    assert!(check_attachment("datasheet", "d.pdf", &png())
        .unwrap_err()
        .contains("PDF"));
    assert!(check_attachment("datasheet", "d.png", b"%PDF-1.4")
        .unwrap_err()
        .contains(".pdf"));
    assert!(check_attachment("picture", "a/p.png", &png())
        .unwrap_err()
        .contains("folder"));
    assert!(check_attachment("picture", " ", &png())
        .unwrap_err()
        .contains("empty"));
    assert!(check_attachment("photometry", "p.png", &png())
        .unwrap_err()
        .contains("role"));
    // the names in the lighting data
    let f = full();
    let ok = LightingDef {
        picture: Some("p.PNG".into()),
        datasheet: Some("d.Pdf".into()),
        ..f.clone()
    };
    assert!(lum(ok).compile().is_ok());
    assert!(why(LightingDef {
        picture: Some("p.gif".into()),
        ..f.clone()
    })
    .contains("picture"));
    assert!(why(LightingDef {
        picture: Some("a/p.png".into()),
        ..f.clone()
    })
    .contains("folder"));
    assert!(why(LightingDef {
        datasheet: Some("d.png".into()),
        ..f.clone()
    })
    .contains("data sheet"));
    assert!(why(LightingDef {
        datasheet: Some("a\\d.pdf".into()),
        ..f.clone()
    })
    .contains("folder"));
    assert!(why(LightingDef {
        picture: Some(" ".into()),
        ..f.clone()
    })
    .contains("empty"));
    // the photometry import keeps a picture and a data sheet that were already set
    let base = LightingDef {
        picture: Some("p.png".into()),
        datasheet: Some("d.pdf".into()),
        ..Default::default()
    };
    let p = openbim_photo::parse(
        b"IESNA:LM-63-2002\nTILT=NONE\n1 -1 1 3 1 1 2 0.6 0.6 0\n1 1 10\n0 45 90\n0\n5 5 5\n",
        "a.ies",
    )
    .unwrap();
    let l = LightingDef::from_photometry(&p, "a.ies", base);
    assert_eq!(
        (l.picture.as_deref(), l.datasheet.as_deref()),
        (Some("p.png"), Some("d.pdf"))
    );
}

fn named_ies(name: &str, cd: f64, watts: f64) -> openbim_photo::Imported {
    let v: Vec<String> = (0..=6).map(|k| (k * 30).to_string()).collect();
    let data = format!("IESNA:LM-63-2002\n[MANUFAC] M\nTILT=NONE\n1 -1 1 7 1 1 2 0.6 0.6 0\n1 1 {watts}\n{}\n0\n{}\n", v.join(" "), vec![cd.to_string(); 7].join(" ")).into_bytes();
    openbim_photo::Imported {
        name: name.into(),
        result: openbim_photo::parse(&data, name),
        data,
    }
}

#[test]
fn a_picker_lists_every_file_and_only_the_ticked_ones_become_types() {
    let imports = vec![
        named_ies("dir/A.ies", 100.0, 20.0),
        openbim_photo::Imported {
            name: "junk.ies".into(),
            data: b"nonsense".to_vec(),
            result: openbim_photo::parse(b"nonsense", "junk.ies"),
        },
        named_ies("B.ies", 200.0, 40.0),
        named_ies("C.ies", 300.0, 60.0),
    ];
    let rows = variant_rows(&imports);
    assert_eq!(
        rows.iter()
            .map(|r| (r.import, r.file.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, "A.ies"), (1, "junk.ies"), (2, "B.ies"), (3, "C.ies")]
    );
    assert!(
        rows[0].unusable.is_none()
            && rows[0].power_w == Some(20.0)
            && rows[0].flux_lm.unwrap() > 0.0
    );
    assert!(rows[1].unusable.is_some() && rows[1].power_w.is_none() && rows[1].flux_lm.is_none());
    assert!(rows[3].flux_lm.unwrap() > rows[2].flux_lm.unwrap());
    // tick C and A (in that order, A twice): types come in import order, once each
    let mut f = Family {
        name: "P".into(),
        category: Category::LightFixture,
        ..Default::default()
    };
    let r = types_from_selection(&mut f, &imports, &[3, 0, 0]).unwrap();
    assert_eq!(
        r.added
            .iter()
            .map(|t| (t.type_name.as_str(), t.import))
            .collect::<Vec<_>>(),
        vec![("A", 0), ("C", 3)]
    );
    assert_eq!(f.types.len(), 2);
    assert_eq!(f.types[1].lighting.as_ref().unwrap().power_w, Some(60.0));
    // a ticked file that cannot be read is reported and adds no type
    let mut g = Family {
        name: "P".into(),
        category: Category::LightFixture,
        ..Default::default()
    };
    let r = types_from_selection(&mut g, &imports, &[1, 2]).unwrap();
    assert_eq!((r.added.len(), r.warnings.len(), g.types.len()), (1, 1, 1));
    assert!(r.warnings[0].contains("junk.ies"));
    // nothing ticked, or a row that does not exist: refused, family untouched
    let mut h = Family {
        name: "P".into(),
        category: Category::LightFixture,
        ..Default::default()
    };
    assert!(
        matches!(types_from_selection(&mut h, &imports, &[]), Err(FamilyError::BadMeta(w)) if w.contains("no file"))
    );
    assert!(
        matches!(types_from_selection(&mut h, &imports, &[0, 4]), Err(FamilyError::BadMeta(w)) if w.contains("not in the import"))
    );
    assert!(h.types.is_empty());
    // the all-files call is the same as ticking every row
    let (mut x, mut y) = (h.clone(), h.clone());
    let a = types_from_imports(&mut x, &imports).unwrap();
    let b = types_from_selection(&mut y, &imports, &[0, 1, 2, 3]).unwrap();
    assert_eq!(a, b);
    assert_eq!(x.types.len(), y.types.len());
}
