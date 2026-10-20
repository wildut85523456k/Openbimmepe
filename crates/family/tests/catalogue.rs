use openbim_family::catalogue::*;
use openbim_family::library::{self, fan_coil};
use openbim_family::shared::*;
use openbim_family::*;

const CAT: &str = "# fan coil range\nfamily\tFan coil unit, ducted\ntype\tair_flow\tduct_w\tcasing\nFCU-S\t0.08\t0.3\tPainted steel\nFCU-L\t0.2\t\tGalvanised\nFCU-X\t\t\t\n";

fn applied() -> Vec<Family> {
    let mut fams = vec![fan_coil()];
    Catalogue::from_text(CAT)
        .unwrap()
        .apply(&mut fams, None)
        .unwrap();
    fams
}

#[test]
fn types_from_a_catalogue_drive_the_solver() {
    let fams = applied();
    assert_eq!(fams[0].types.len(), 3);
    let c = fams[0].compile().unwrap();
    let s = c
        .solve(&Request {
            type_name: Some("FCU-S"),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(c.value(&s, "air_flow"), Some(0.08));
    assert_eq!(c.value(&s, "duct_w"), Some(0.3));
    let supply = s.connectors.iter().find(|x| x.name == "supply").unwrap();
    assert_eq!(
        supply.size,
        Size::Rect {
            width: 0.3,
            height: 0.2
        }
    );
    assert_eq!(supply.flow, 0.08);
    assert_eq!(s.material_of(0), Some("Painted steel"));
    // blank cell: the family's own default stays
    let l = c
        .solve(&Request {
            type_name: Some("FCU-L"),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(c.value(&l, "duct_w"), Some(0.4));
    assert_eq!(c.value(&l, "air_flow"), Some(0.2));
    assert_eq!(l.material_of(0), Some("Galvanised"));
    let x = c
        .solve(&Request {
            type_name: Some("FCU-X"),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(c.value(&x, "air_flow"), Some(0.1));
    assert_eq!(x.material_of(0), Some("Casing"));
}

use openbim_family::mep::Size;

#[test]
fn numbers_out_of_range_are_found_when_the_type_is_used() {
    let mut fams = vec![fan_coil()];
    Catalogue::from_text("family\tFan coil unit, ducted\ntype\tair_flow\nBIG\t99\n")
        .unwrap()
        .apply(&mut fams, None)
        .unwrap();
    let c = fams[0].compile().unwrap();
    assert!(matches!(
        c.solve(&Request {
            type_name: Some("BIG"),
            ..Default::default()
        }),
        Err(FamilyError::OutOfRange { .. })
    ));
}

#[test]
fn apply_errors_and_atomicity() {
    let run =
        |text: &str, fams: &mut Vec<Family>| Catalogue::from_text(text).unwrap().apply(fams, None);
    let mut f = vec![fan_coil()];
    assert_eq!(
        run("family\tNope\ntype\ta\nT\t1\n", &mut f),
        Err(CatError::UnknownFamily("Nope".into()))
    );
    assert_eq!(
        run("family\tFan coil unit, ducted\ntype\tnope\nT\t1\n", &mut f),
        Err(CatError::UnknownColumn {
            family: "Fan coil unit, ducted".into(),
            column: "nope".into()
        })
    );
    assert_eq!(
        run("family\tFan coil unit, ducted\ntype\tx0\nT\t1\n", &mut f),
        Err(CatError::NotAnInput {
            family: "Fan coil unit, ducted".into(),
            column: "x0".into()
        })
    );
    for bad in ["abc", "NaN", "inf", "-inf", "1e999", "1,5"] {
        let e = run(
            &format!("family\tFan coil unit, ducted\ntype\tair_flow\nT\t{bad}\n"),
            &mut f,
        );
        assert!(matches!(e, Err(CatError::BadNumber { .. })), "{bad}: {e:?}");
    }
    assert!(f[0].types.is_empty());
    // a type name that already exists
    let mut g = vec![library::window()];
    let e = run(
        "family\tWindow, fixed with mullions\ntype\twidth\nType A\t1\n",
        &mut g,
    );
    assert_eq!(
        e,
        Err(CatError::DuplicateType {
            family: "Window, fixed with mullions".into(),
            type_name: "Type A".into()
        })
    );
    assert_eq!(g[0].types.len(), 1);
    // all or nothing across tables
    let mut h = vec![fan_coil(), library::window()];
    let e = run("family\tFan coil unit, ducted\ntype\tair_flow\nOK\t0.1\nfamily\tWindow, fixed with mullions\ntype\twidth\nType A\t1\n", &mut h);
    assert!(e.is_err());
    assert!(
        h[0].types.is_empty(),
        "the first table must not be applied when the second fails"
    );
    // yes/no spellings
    let mut k = vec![library::door()];
    for (w, v) in [
        ("true", 1.0),
        ("yes", 1.0),
        ("1", 1.0),
        ("false", 0.0),
        ("no", 0.0),
        ("0", 0.0),
    ] {
        k[0].types.clear();
        run(
            &format!("family\tDoor, single flush\ntype\thas_handle\nT\t{w}\n"),
            &mut k,
        )
        .unwrap();
        assert_eq!(k[0].types[0].values, vec![("has_handle".to_string(), v)]);
    }
    // text cells go to text-kind inputs, numbers to number inputs
    k[0].types.clear();
    run(
        "family\tDoor, single flush\ntype\tswing\twidth\tremarks\nT\tleft\t0.9\tfire door\n",
        &mut k,
    )
    .unwrap();
    assert_eq!(
        k[0].types[0].texts,
        vec![
            ("swing".to_string(), "left".to_string()),
            ("remarks".to_string(), "fire door".to_string())
        ]
    );
    assert_eq!(k[0].types[0].values, vec![("width".to_string(), 0.9)]);
    // a number in a text column is just text; text in a number column is an error
    k[0].types.clear();
    assert!(matches!(
        run("family\tDoor, single flush\ntype\twidth\nT\tleft\n", &mut k),
        Err(CatError::BadNumber { .. })
    ));
}

#[test]
fn shared_parameter_columns() {
    let s = SharedSet {
        params: vec![
            SharedParam {
                id: 2,
                group: "g".into(),
                description: String::new(),
                def: ValueDef::text("mark", "", Scope::Type),
            },
            SharedParam {
                id: 1,
                group: "g".into(),
                description: String::new(),
                def: ValueDef::input("wall_t", 0.2, 0.05, 1.0, Scope::Type),
            },
        ],
    };
    let mut f = Family {
        name: "A".into(),
        shared: vec!["mark".into(), "wall_t".into()],
        ..Default::default()
    };
    let text = "family\tA\ntype\twall_t\nT1\t0.3\n";
    let mut fams = vec![f.clone()];
    Catalogue::from_text(text)
        .unwrap()
        .apply(&mut fams, Some(&s))
        .unwrap();
    assert_eq!(fams[0].types[0].values, vec![("wall_t".to_string(), 0.3)]);
    // without the set, or when the family does not use it, the column is unknown
    let mut fams = vec![f.clone()];
    assert!(matches!(
        Catalogue::from_text(text).unwrap().apply(&mut fams, None),
        Err(CatError::UnknownColumn { .. })
    ));
    f.shared.clear();
    let mut fams = vec![f];
    assert!(matches!(
        Catalogue::from_text(text)
            .unwrap()
            .apply(&mut fams, Some(&s)),
        Err(CatError::UnknownColumn { .. })
    ));
}

#[test]
fn text_errors_name_the_line() {
    let cases: Vec<(&str, usize)> = vec![
        ("T\t1\n", 1),
        ("type\ta\n", 1),
        ("family\n", 1),
        ("family\t\n", 1),
        ("family\tA\tB\n", 1),
        ("# c\n\nfamily\tA\nT\t1\n", 4),
        ("family\tA\ntype\n", 2),
        ("family\tA\nfoo\ta\n", 2),
        ("family\tA\tB\ntype\ta\n", 1),
        ("family\t\ntype\ta\n", 1),
        ("family\tA\ntype\t1a\n", 2),
        ("family\tA\ntype\ta\ta\n", 2),
        ("family\tA\ntype\ta\nT\n", 3),
        ("family\tA\ntype\ta\nT\t1\t2\n", 3),
        ("family\tA\ntype\ta\n\t1\n", 3),
        ("family\tA\ntype\ta\nT\t1\nT\t2\n", 4),
        ("family\tA\ntype\ta\nfamily\tA\ntype\ta\n", 3),
        ("family\tA\ntype\ta\nT\ta\u{1}b\n", 3),
        ("family\tA\n", 1),
        ("family\tA\ntype\ta\nfamily\tB\n", 3),
    ];
    for (t, ln) in cases {
        match Catalogue::from_text(t) {
            Err(CatError::Syntax { line, .. }) | Err(CatError::Text { line, .. }) => {
                assert_eq!(line, ln, "{t:?}")
            }
            other => panic!("{t:?}: {other:?}"),
        }
    }
    assert!(Catalogue::from_text("family\tA\ntype\ta\nT\t1\n").is_ok());
}

#[test]
fn limits() {
    let cols = |n: usize| {
        (0..n)
            .map(|i| format!("c{i}"))
            .collect::<Vec<_>>()
            .join("\t")
    };
    assert!(Catalogue::from_text(&format!("family\tA\ntype\t{}\n", cols(500))).is_ok());
    assert_eq!(
        Catalogue::from_text(&format!("family\tA\ntype\t{}\n", cols(501))).unwrap_err(),
        CatError::TooMany("columns")
    );
    let rows = |n: usize| (0..n).map(|i| format!("t{i}\t1\n")).collect::<String>();
    assert!(Catalogue::from_text(&format!("family\tA\ntype\ta\n{}", rows(10_000))).is_ok());
    assert_eq!(
        Catalogue::from_text(&format!("family\tA\ntype\ta\n{}", rows(10_001))).unwrap_err(),
        CatError::TooMany("types in one family")
    );
    let name = |n: usize| format!("family\tA\ntype\ta\n{}\t1\n", "n".repeat(n));
    assert!(Catalogue::from_text(&name(128)).is_ok());
    assert!(matches!(
        Catalogue::from_text(&name(129)),
        Err(CatError::Text { line: 3, .. })
    ));
    let cell = |n: usize| format!("family\tA\ntype\ta\nT\t{}\n", "c".repeat(n));
    assert!(Catalogue::from_text(&cell(256)).is_ok());
    assert!(matches!(
        Catalogue::from_text(&cell(257)),
        Err(CatError::Text { line: 3, .. })
    ));
    let fam = |n: usize| format!("family\t{}\ntype\ta\n", "f".repeat(n));
    assert!(Catalogue::from_text(&fam(128)).is_ok());
    assert!(matches!(
        Catalogue::from_text(&fam(129)),
        Err(CatError::Text { line: 1, .. })
    ));
}

#[test]
fn export_and_reload_give_the_same_types() {
    let fams = applied();
    let cat = Catalogue::export(&fams);
    let text = cat.to_text();
    let back = Catalogue::from_text(&text).unwrap();
    assert_eq!(back, cat);
    assert_eq!(back.to_text(), text);
    let mut fresh = vec![fan_coil()];
    back.apply(&mut fresh, None).unwrap();
    for (a, b) in fresh[0].types.iter().zip(&fams[0].types) {
        assert_eq!(
            (&a.name, &a.values, &a.texts),
            (&b.name, &b.values, &b.texts)
        );
    }
    // library families export too; families without types are left out
    let lib = vec![library::door(), library::window(), library::handle()];
    let c = Catalogue::export(&lib);
    assert_eq!(c.tables.len(), 2);
    assert_eq!(c.tables[0].columns, vec!["width", "height"]);
    assert_eq!(
        c.tables[0].rows[0],
        (
            "Type A".to_string(),
            vec!["0.8".to_string(), "".to_string()]
        )
    );
    assert_eq!(
        c.tables[0].rows[1],
        (
            "Type B".to_string(),
            vec!["1.2".to_string(), "2.4".to_string()]
        )
    );
    assert!(Catalogue::from_text(&c.to_text()).is_ok());
    // CRLF and comments
    let crlf = format!("# x\r\n{}", text.replace('\n', "\r\n"));
    assert_eq!(Catalogue::from_text(&crlf).unwrap(), cat);
}
