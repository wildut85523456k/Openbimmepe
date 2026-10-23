//! Tests for the IFC import: the STEP (ISO 10303-21) parser and the IFC model reader.
//! The key test is a round-trip: a scene exported by the writer is read back by the reader,
//! proving the product can open and parse IFC.
use openbim_arch::{Level, Levels};
use openbim_geom::WallChain;
use openbim_ifc::reader;
use openbim_ifc::step_read;
use openbim_ifc::*;

fn levels() -> Levels {
    Levels::new(vec![Level {
        name: "L0".into(),
        elevation: 0.0,
    }])
    .unwrap()
}
fn scene(walls: Vec<WallItem>) -> Scene {
    Scene {
        project_name: "P".into(),
        project_key: 1,
        timestamp: "t".into(),
        levels: levels(),
        walls,
        wall_penetrations: vec![],
        firestops: Default::default(),
        slabs: vec![],
        roofs: vec![],
        railings: vec![],
        stairs: vec![],
        columns: vec![],
        beams: vec![],
        materials: Default::default(),
        shafts: vec![],
        floor_penetrations: vec![],
        slab_voids: vec![],
        networks: vec![],
        families: vec![],
        items: vec![],
        rooms: vec![],
        room_lighting: Default::default(),
        wall_types: vec![],
        types: vec![],
        member_info: Default::default(),
        native: vec![],
        wall_joins: vec![],
        georef: None,
        room_settings: Default::default(),
        separation_lines: vec![],
        pset_map: Default::default(), grids: vec![], system_defs: vec![], assemblies: vec![],
    }
}
fn wall(id: u64) -> WallItem {
    WallItem {
        id,
        name: "W".into(),
        chain: WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.2, 3.0),
        wall_type: None,
        room_bounding: true,
    }
}

/// A minimal but valid IFC4 file: a project, site, building and one storey, no elements.
const MINIMAL_IFC4: &str = r#"ISO-10303-21;
HEADER;
FILE_DESCRIPTION((''),'2;1');
FILE_NAME('t.ifc','2026-01-01T00:00:00',(''),(''),'','','');
FILE_SCHEMA(('IFC4'));
ENDSEC;
DATA;
#1=IFCPERSON($,$,'Test',$,$,$,$,$);
#2=IFCORGANIZATION($,'TestOrg',$,$,$);
#3=IFCPERSONANDORGANIZATION(#1,#2,$);
#4=IFCAPPLICATION(#2,'1.0','TestApp','Test');
#5=IFCOWNERHISTORY(#3,#4,$,.ADDED.,$,$,$,0);
#6=IFCCARTESIANPOINT((0.,0.,0.));
#7=IFCDIRECTION((0.,0.,1.));
#8=IFCAXIS2PLACEMENT3D(#6,$,#7);
#9=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,#8,$);
#10=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);
#11=IFCUNITASSIGNMENT((#10));
#12=IFCPROJECT('0000000040000000000001',#5,'P','',$,$,$,(#9),#11);
#13=IFCSITE('0000000040000000000002',#5,'S','',$,$,#8,$,$,.ELEMENT.,$,$,$,$,$);
#14=IFCBUILDING('0000000040000000000003',#5,'B','',$,$,#8,$,$,.ELEMENT.,$,$,$);
#15=IFCBUILDINGSTOREY('0000000040000000000004',#5,'L0','',$,$,#8,$,.ELEMENT.,0.);
#16=IFCRELAGGREGATES('0000000040000000000010',#5,$,$,#12,(#13));
#17=IFCRELAGGREGATES('0000000040000000000011',#5,$,$,#13,(#14));
#18=IFCRELAGGREGATES('0000000040000000000012',#5,$,$,#14,(#15));
ENDSEC;
END-ISO-10303-21;
"#;

#[test]
fn the_step_parser_reads_a_minimal_ifc4_file() {
    let f = step_read::parse(MINIMAL_IFC4).unwrap();
    assert_eq!(f.schema, "IFC4");
    assert!(f.entities.len() >= 18, "entities: {}", f.entities.len());
    let proj = f
        .entities
        .values()
        .find(|e| e.class == "IFCPROJECT")
        .expect("a project");
    assert_eq!(
        proj.args.first().and_then(|v| v.as_str()),
        Some("0000000040000000000001")
    );
}

#[test]
fn the_step_parser_rejects_a_non_step_file() {
    assert!(matches!(
        step_read::parse("this is not an IFC file"),
        Err(step_read::ReadError::NotStep)
    ));
}

#[test]
fn the_reader_reads_the_storey_from_a_minimal_file() {
    let got = reader::read(MINIMAL_IFC4).unwrap();
    assert_eq!(got.schema, "IFC4");
    assert_eq!(got.storeys.len(), 1);
    assert_eq!(got.storeys[0].name, "L0");
    assert_eq!(got.storeys[0].elevation, 0.0);
    assert!(
        got.members.is_empty(),
        "no building elements in the minimal file"
    );
}

#[test]
fn exported_ifc_round_trips_through_the_reader() {
    let text = export(&scene(vec![wall(5)])).unwrap().text;
    let got = reader::read(&text).unwrap();
    assert_eq!(got.schema, "IFC4");
    // the level comes back as a storey
    assert_eq!(got.storeys.len(), 1);
    assert_eq!(got.storeys[0].name, "L0");
    assert_eq!(got.storeys[0].elevation, 0.0);
    // the wall comes back as an IFCWALL member, in that storey, with its GlobalId
    assert_eq!(got.members.len(), 1);
    let w = &got.members[0];
    assert_eq!(w.class, "IFCWALL");
    assert_eq!(w.name, "W");
    assert!(!w.guid.is_empty());
    assert_eq!(w.storey, Some(0));
    // the wall's extruded body: a 4 m long, 0.2 m thick footprint, extruded 3 m up
    let body = w.body.as_ref().expect("the wall has an extruded body");
    assert!(
        matches!(body.section, reader::Section::Polygon),
        "section: {:?}",
        body.section
    );
    assert!(
        (body.dir[2] - 1.0).abs() < 1e-9,
        "extruded vertically: {:?}",
        body.dir
    );
    assert!(
        (body.depth - 3.0).abs() < 1e-9,
        "the wall is 3 m high: {}",
        body.depth
    );
    assert_eq!(body.outline.len(), 4);
    let xs: Vec<f64> = body.outline.iter().map(|p| p[0]).collect();
    let (xmin, xmax) = xs
        .iter()
        .cloned()
        .fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
    assert!(
        (xmin - 0.0).abs() < 1e-9 && (xmax - 4.0).abs() < 1e-9,
        "x extent 0..4: {:?}",
        xs
    );
    // a clean round-trip leaves nothing out
    assert!(got.skipped.is_empty(), "skipped: {:?}", got.skipped);
}

#[test]
fn the_reader_keeps_the_wall_global_id_stable_across_exports() {
    let a = reader::read(&export(&scene(vec![wall(5)])).unwrap().text).unwrap();
    let b = reader::read(&export(&scene(vec![wall(5)])).unwrap().text).unwrap();
    assert_eq!(a.members[0].guid, b.members[0].guid);
    assert_eq!(a.members[0].guid, "0000000040000000000005");
}
fn step(data: &str) -> String {
    format!("ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n{data}\nENDSEC;\nEND-ISO-10303-21;")
}

#[test]
fn malformed_string_escapes_are_errors_not_panics() {
    for text in [
        r"\X\0é",
        r"\X2\000é\X0\",
        r"\X2\0041",
        r"\X2\001\X0\",
        r"\X\GG",
        r"\X2\D800\X0\",
    ] {
        assert!(
            step_read::parse(&step(&format!("#1=IFCLABEL('{text}');"))).is_err(),
            "{text}"
        );
    }
}

#[test]
fn step_ids_are_positive_unique_u32_including_references_and_complex_entities() {
    for data in [
        "#4294967297=IFCX();",
        "#1=IFCX(#4294967297);",
        "#0=IFCX();",
        "#1=IFCX(#0);",
        "#1=IFCX();#1=IFCY();",
        "#1=(IFCX()IFCY());#1=IFCZ();",
    ] {
        assert!(step_read::parse(&step(data)).is_err(), "{data}");
    }
    assert!(step_read::parse(&step("#4294967295=IFCX(#4294967295);")).is_ok());
}

#[test]
fn step_requires_section_and_file_terminators() {
    for suffix in [
        "",
        "END",
        "ENDSEC;",
        "ENDSEC;END-ISO-10303-21",
        "ENDSEC;END-ISO-10303-21;garbage",
    ] {
        assert!(
            step_read::parse(&format!("ISO-10303-21;DATA;#1=IFCX();{suffix}")).is_err(),
            "{suffix}"
        );
    }
}

// Synthetic fixture, authored for F1-R1; not third-party interoperability evidence.
fn length_fixture(scale: f64, prefix: &str, profile: &str) -> String {
    step(&format!(
        r#"
#1=IFCSIUNIT(*,.LENGTHUNIT.,{prefix},.METRE.);
#2=IFCUNITASSIGNMENT((#1));
#3=IFCPROJECT('p',$,'P',$,$,$,$,(),#2);
#4=IFCCARTESIANPOINT(({x},{y},{z}));
#5=IFCAXIS2PLACEMENT3D(#4,$,$);
#6=IFCLOCALPLACEMENT($,#5);
#7=IFCCARTESIANPOINT(({a},{b}));
#8=IFCAXIS2PLACEMENT2D(#7,$);
#9={profile};
#10=IFCDIRECTION((0.,0.,1.));
#11=IFCEXTRUDEDAREASOLID(#9,#5,#10,{depth});
#12=IFCSHAPEREPRESENTATION($,'Body','SweptSolid',(#11));
#13=IFCPRODUCTDEFINITIONSHAPE($,$,(#12));
#14=IFCBEAM('b',$,'B',$,$,#6,#13,$,.BEAM.);
#15=IFCBUILDINGSTOREY('s',$,'L',$,$,#6,$,$,.ELEMENT.,{z});
#16=IFCMATERIAL('Steel');
#17=IFCMATERIALLAYER(#16,{thickness},$,$,$,$,$);
#18=IFCMATERIALLAYERSET((#17),'Layers',$);
#19=IFCBEAMTYPE('t',$,'Type',$,$,(),$,$,$,.BEAM.);
#20=IFCRELASSOCIATESMATERIAL('r',$,$,$,(#19),#18);
#21=IFCPROPERTYSINGLEVALUE('Number',$,IFCREAL(42.),$);
#22=IFCPROPERTYSET('ps',$,'Test',$,(#21));
#23=IFCRELDEFINESBYPROPERTIES('rp',$,$,$,(#14),#22);
"#,
        x = scale,
        y = 2.0 * scale,
        z = 3.0 * scale,
        a = 0.25 * scale,
        b = 0.5 * scale,
        depth = 4.0 * scale,
        thickness = 0.02 * scale
    ))
}

#[test]
fn metres_and_millimetres_have_equivalent_canonical_lengths() {
    let a = reader::read(&length_fixture(
        1.0,
        "$",
        "IFCRECTANGLEPROFILEDEF(.AREA.,$,#8,2.,1.)",
    ))
    .unwrap();
    let b = reader::read(&length_fixture(
        1000.0,
        ".MILLI.",
        "IFCRECTANGLEPROFILEDEF(.AREA.,$,#8,2000.,1000.)",
    ))
    .unwrap();
    let (a_body, b_body) = (
        a.members[0].body.as_ref().unwrap(),
        b.members[0].body.as_ref().unwrap(),
    );
    assert!((a.storeys[0].elevation - b.storeys[0].elevation).abs() < 1e-5);
    assert!((a.types[0].layers[0].thickness - b.types[0].layers[0].thickness).abs() < 1e-5);
    assert_eq!(a_body.section, b_body.section);
    assert!((a_body.depth - b_body.depth).abs() < 1e-5);
    for (a, b) in a_body
        .outline
        .iter()
        .flatten()
        .zip(b_body.outline.iter().flatten())
    {
        assert!((a - b).abs() < 1e-5);
    }
    assert_eq!(a_body.dir, b_body.dir);
    assert_eq!(b.members[0].psets["Test"]["Number"].num(), Some(42.0));
}

#[test]
fn unsupported_or_missing_length_units_are_explicit_errors() {
    let text = length_fixture(1.0, "$", "IFCRECTANGLEPROFILEDEF(.AREA.,$,#8,2.,1.)");
    for bad in [
        text.replace("$,.METRE.", ".CENTI.,.METRE."),
        text.replace(
            "IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.)",
            "IFCCONVERSIONBASEDUNIT($,.LENGTHUNIT.,'foot',#99)",
        ),
        text.replace("IFCUNITASSIGNMENT((#1))", "IFCUNITASSIGNMENT(())"),
    ] {
        assert!(reader::read(&bad).is_err());
    }
}

#[test]
fn short_or_nonfinite_points_are_diagnosed_without_panicking() {
    let text = length_fixture(1.0, "$", "IFCRECTANGLEPROFILEDEF(.AREA.,$,#8,2.,1.)");
    for coords in ["()", "(1.)", "(1.,2.)", "(1.,2.,1.E999)", "(1.,'bad',3.)"] {
        let bad = text.replace(
            "IFCCARTESIANPOINT((1,2,3))",
            &format!("IFCCARTESIANPOINT({coords})"),
        );
        assert_ne!(text, bad);
        assert!(reader::read(&bad).is_err(), "{coords}");
    }
}

#[test]
fn boolean_results_are_not_imported_as_uncut_operands() {
    let text = length_fixture(1.0, "$", "IFCRECTANGLEPROFILEDEF(.AREA.,$,#8,2.,1.)");
    for class in ["IFCBOOLEANRESULT", "IFCBOOLEANCLIPPINGRESULT"] {
        let bad = text.replace("(#11));", "(#24));").replace(
            "ENDSEC;\nEND-ISO",
            &format!("#24={class}(.DIFFERENCE.,#11,#11);\nENDSEC;\nEND-ISO"),
        );
        assert!(reader::read(&bad).is_err());
    }
}

#[test]
fn i_profile_retains_its_reentrant_outline() {
    let got = reader::read(&length_fixture(
        1.0,
        "$",
        "IFCISHAPEPROFILEDEF(.AREA.,$,#8,2.,3.,0.2,0.3)",
    ))
    .unwrap();
    let body = got.members[0].body.as_ref().unwrap();
    assert_eq!(body.outline.len(), 12);
    let area: f64 = body
        .outline
        .iter()
        .zip(body.outline.iter().cycle().skip(1))
        .take(12)
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f64>()
        .abs()
        / 2.0;
    assert!((area - (2.0 * 2.0 * 0.3 + 0.2 * (3.0 - 0.6))).abs() < 1e-9);
}

#[test]
fn null_placement_unsupported_products_are_counted() {
    let got = reader::read(&step("#10=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);#11=IFCUNITASSIGNMENT((#10));#12=IFCPROJECT('p',$,'P',$,$,$,$,(),#11);#1=IFCOPENINGELEMENT('o',$,'O',$,$,$,$,$,$);#2=IFCDOOR('d',$,'D',$,$,$,$,$,$);#3=IFCSPACE('s',$,'S',$,$,$,$,$,$);")).unwrap();
    for class in ["IFCOPENINGELEMENT", "IFCDOOR", "IFCSPACE"] {
        assert_eq!(got.skipped.get(class), Some(&1));
    }
    assert_eq!(got.diagnostics.len(), 3);
    for (id, class) in [(1, "IFCOPENINGELEMENT"), (2, "IFCDOOR"), (3, "IFCSPACE")] {
        assert!(got
            .diagnostics
            .iter()
            .any(|d| d.contains(&format!("#{id} {class}"))));
    }
}

#[test]
fn header_and_comment_truncation_are_not_hidden_by_token_search() {
    for text in [
        "ISO-10303-21;HEADER;FILE_SCHEMA(('IFC4'));DATA;ENDSEC;END-ISO-10303-21;",
        "ISO-10303-21;HEADER;FILE_NAME('DATA;ENDSEC;END-ISO-10303-21;');",
        "ISO-10303-21;DATA;ENDSEC;END-ISO-10303-21;/*unterminated",
    ] {
        assert!(step_read::parse(text).is_err(), "{text}");
    }
}

#[test]
fn malformed_shift_escapes_also_never_panic() {
    for text in [r"\S\é", r"\S\"] {
        assert!(step_read::parse(&step(&format!("#1=IFCLABEL('{text}');"))).is_err());
    }
}

#[test]
fn complex_entities_cannot_bypass_reference_or_termination_validation() {
    for data in [
        "#1=(IFCX(#4294967297)IFCY());",
        "#1=(IFCX();",
        "#1=(IFCX('\\X\\0é'));",
        "#1=(IFCX(#0));",
    ] {
        assert!(step_read::parse(&step(data)).is_err(), "{data}");
    }
    let got = step_read::parse(&step("#1=(IFCX('a;b')/* ; */IFCY(#4294967295));")).unwrap();
    assert_eq!(got.complex, 1);
}

#[test]
fn units_scale_circle_i_and_polygon_profiles_and_nested_placements() {
    for (metres, mm) in [
        (
            "IFCCIRCLEPROFILEDEF(.AREA.,$,#8,0.5)",
            "IFCCIRCLEPROFILEDEF(.AREA.,$,#8,500.)",
        ),
        (
            "IFCISHAPEPROFILEDEF(.AREA.,$,#8,2.,3.,0.2,0.3)",
            "IFCISHAPEPROFILEDEF(.AREA.,$,#8,2000.,3000.,200.,300.)",
        ),
        (
            "IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#24)",
            "IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#24)",
        ),
    ] {
        let fixture = |scale, prefix, profile| {
            length_fixture(scale, prefix, profile)
                .replace(
                    "#6=IFCLOCALPLACEMENT($,#5);",
                    "#6=IFCLOCALPLACEMENT(#25,#5);#25=IFCLOCALPLACEMENT($,#5);",
                )
                .replace(
                    "ENDSEC;\nEND-ISO",
                    "#24=IFCPOLYLINE((#7,#26,#27,#7));\nENDSEC;\nEND-ISO",
                )
        };
        let a = fixture(1.0, "$", metres).replace(
            "#24=IFCPOLYLINE",
            "#26=IFCCARTESIANPOINT((1.,0.));#27=IFCCARTESIANPOINT((0.,1.));#24=IFCPOLYLINE",
        );
        let b = fixture(1000.0, ".MILLI.", mm).replace(
            "#24=IFCPOLYLINE",
            "#26=IFCCARTESIANPOINT((1000.,0.));#27=IFCCARTESIANPOINT((0.,1000.));#24=IFCPOLYLINE",
        );
        let (a, b) = (reader::read(&a).unwrap(), reader::read(&b).unwrap());
        let (a, b) = (
            a.members[0].body.as_ref().unwrap(),
            b.members[0].body.as_ref().unwrap(),
        );
        assert_eq!(a.section, b.section);
        assert_eq!(a.outline.len(), b.outline.len());
        for (a, b) in a.outline.iter().flatten().zip(b.outline.iter().flatten()) {
            assert!((a - b).abs() < 1e-5);
        }
        assert!((a.depth - b.depth).abs() < 1e-5);
        assert_eq!(a.dir, b.dir);
    }
}

#[test]
fn valid_unicode_and_delimiter_text_in_headers_survive() {
    let text = step("#1=IFCLABEL('é\\X2\\D83DDE00\\X0\\ \\X\\E9');").replace(
        "FILE_SCHEMA",
        "FILE_NAME('DATA;ENDSEC;');/* DATA; */FILE_SCHEMA",
    );
    let got = step_read::parse(&text).unwrap();
    assert_eq!(got.schema, "IFC4");
    assert_eq!(got.get(1).unwrap().args[0].as_str(), Some("é😀 é"));
}

#[test]
fn writer_literal_backslashes_survive_headers_and_data() {
    for value in [
        r"C:\X\GG.ifc",
        r"C:\X2\D800\X0\file.ifc",
        r"C:\S\file.ifc",
        r"literal \X\41 and \X2\0041\X0\",
        r"two \\ and trailing \",
    ] {
        let mut writer = openbim_ifc::step::Step::default();
        let id = writer.add(format!("IFCLABEL({})", openbim_ifc::step::string(value)));
        // Isolate DATA decoding from header acceptance, then exercise both together.
        for name in ["safe.ifc", value] {
            let text = writer.finish(name, "t", false);
            let got = step_read::parse(&text).unwrap_or_else(|e| panic!("{value:?}: {e}"));
            assert_eq!(got.get(id).unwrap().args[0].as_str(), Some(value));
        }
    }
}

#[test]
fn nonsharp_i_profile_attributes_are_explicitly_rejected() {
    for (optional, attribute) in [
        ("0.1,$,$", "FilletRadius"),
        ("$,0.1,$", "FlangeEdgeRadius"),
        ("$,$,0.1", "FlangeSlope"),
        ("$,$,-0.1", "FlangeSlope"),
    ] {
        let text = length_fixture(
            1.0,
            "$",
            &format!("IFCISHAPEPROFILEDEF(.AREA.,$,#8,2.,3.,0.2,0.3,{optional})"),
        );
        let err = reader::read(&text).expect_err("must not silently substitute a sharp I-profile");
        assert!(matches!(err, reader::Error::Invalid(_)));
        let message = err.to_string();
        assert!(message.contains("#9 IFCISHAPEPROFILEDEF"), "{message}");
        assert!(message.contains(attribute), "{message}");
    }
}

#[test]
fn sharp_i_profile_null_and_zero_attributes_preserve_geometry() {
    let fixture = |optional| {
        length_fixture(
            1.0,
            "$",
            &format!("IFCISHAPEPROFILEDEF(.AREA.,$,#8,2.,3.,0.2,0.3{optional})"),
        )
    };
    let plain = reader::read(&fixture("")).unwrap();
    for optional in [",$,$,$", ",0.,0.,0.", ",$,0.,-0."] {
        let got = reader::read(&fixture(optional)).unwrap();
        assert_eq!(plain.members[0].body, got.members[0].body);
        assert!(got.diagnostics.is_empty());
        assert!(got.skipped.is_empty());
    }
}
