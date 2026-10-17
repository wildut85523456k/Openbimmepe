use openbim_family::shared::*;
use openbim_family::*;
use proptest::prelude::*;

fn sp(id: u128, group: &str, def: ValueDef) -> SharedParam {
    SharedParam {
        id,
        group: group.into(),
        description: "d".into(),
        def,
    }
}

fn set() -> SharedSet {
    SharedSet {
        params: vec![
            sp(
                1,
                "Dimensions",
                ValueDef::input("wall_t", 0.2, 0.05, 1.0, Scope::Instance),
            ),
            sp(
                2,
                "Fire",
                ValueDef::choice("fire_rating", &["none", "r30", "r60"], "none", Scope::Type),
            ),
            sp(3, "Identity", ValueDef::text("mark", "", Scope::Instance)),
            sp(
                4,
                "Identity",
                ValueDef::flag("is_external", false, Scope::Instance),
            ),
            sp(
                5,
                "Finish",
                ValueDef::material("finish", "Paint", Scope::Type),
            ),
            sp(
                6,
                "Layout",
                ValueDef::count("leaves", 1.0, 1.0, 4.0, Scope::Instance),
            ),
        ],
    }
}

fn fam(name: &str, shared: &[&str], extra: Vec<ValueDef>) -> Family {
    let mut values = extra;
    values.push(ValueDef::input("w", 1.0, 0.5, 2.0, Scope::Type));
    values.push(ValueDef::plane("x0", Axis::X, "0"));
    values.push(ValueDef::plane("x1", Axis::X, "w"));
    values.push(ValueDef::plane("y0", Axis::Y, "0"));
    values.push(ValueDef::plane("y1", Axis::Y, "1"));
    values.push(ValueDef::plane("z0", Axis::Z, "0"));
    values.push(ValueDef::plane("z1", Axis::Z, "1"));
    Family {
        name: name.into(),
        shared: shared.iter().map(|s| s.to_string()).collect(),
        values,
        solids: vec![SolidDef::new("b", ("x0", "x1"), ("y0", "y1"), ("z0", "z1"))],
        ..Default::default()
    }
}

#[test]
fn two_families_share_one_definition() {
    let s = set();
    let lib = compile_library_shared(
        &[
            fam("A", &["wall_t", "fire_rating", "leaves"], vec![]),
            fam("B", &["wall_t", "mark"], vec![]),
        ],
        &s,
    )
    .unwrap();
    assert_eq!(
        lib[0].shared_ids().to_vec(),
        vec![
            ("wall_t".to_string(), 1u128),
            ("fire_rating".to_string(), 2),
            ("leaves".to_string(), 6)
        ]
    );
    assert_eq!(
        lib[1].shared_ids().to_vec(),
        vec![("wall_t".to_string(), 1u128), ("mark".to_string(), 3)]
    );
    for c in &lib {
        let i = c.solve(&Request::default()).unwrap();
        assert_eq!(c.value(&i, "wall_t"), Some(0.2));
        // same range in both families
        for bad in [0.04, 1.01] {
            let r = c.solve(&Request {
                overrides: vec![("wall_t", bad)],
                ..Default::default()
            });
            assert!(matches!(r, Err(FamilyError::OutOfRange { .. })), "{bad}");
        }
        assert!(c
            .solve(&Request {
                overrides: vec![("wall_t", 0.05)],
                ..Default::default()
            })
            .is_ok());
        assert!(c
            .solve(&Request {
                overrides: vec![("wall_t", 1.0)],
                ..Default::default()
            })
            .is_ok());
    }
    // shared parameters come before the family's own inputs
    let i = lib[0].solve(&Request::default()).unwrap();
    let names: Vec<&str> = lib[0].input_values(&i).iter().map(|(n, _)| *n).collect();
    assert_eq!(names, vec!["wall_t", "leaves", "w"]);
    // kinds and scopes are kept
    assert_eq!(lib[0].input_scope("fire_rating"), Some(Scope::Type));
    assert_eq!(lib[0].input_scope("wall_t"), Some(Scope::Instance));
    let i = lib[0]
        .solve(&Request {
            overrides: vec![("leaves", 2.0)],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(lib[0].value(&i, "leaves"), Some(2.0));
    assert!(lib[0]
        .solve(&Request {
            overrides: vec![("leaves", 1.5)],
            ..Default::default()
        })
        .is_err());
    let i = lib[1]
        .solve(&Request {
            texts: vec![("mark", "D-01")],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(lib[1].text(&i, "mark"), Some("D-01"));
    // a family can use a shared name in formulas
    let mut f = fam("C", &["wall_t"], vec![]);
    f.values.push(ValueDef::formula("twice", "wall_t * 2"));
    let c = compile_library_shared(&[f], &s).unwrap().remove(0);
    let i = c.solve(&Request::default()).unwrap();
    assert!((c.value(&i, "twice").unwrap() - 0.4).abs() < 1e-12);
}

#[test]
fn naming_errors() {
    let s = set();
    assert_eq!(
        compile_library_shared(&[fam("A", &["nope"], vec![])], &s).err(),
        Some(FamilyError::UnknownShared("nope".into()))
    );
    // no set given at all
    assert_eq!(
        compile_library(&[fam("A", &["wall_t"], vec![])]).err(),
        Some(FamilyError::UnknownShared("wall_t".into()))
    );
    // a family that defines the same name itself, or lists it twice
    assert_eq!(
        compile_library_shared(
            &[fam(
                "A",
                &["wall_t"],
                vec![ValueDef::input("wall_t", 0.1, 0.0, 1.0, Scope::Type)]
            )],
            &s
        )
        .err(),
        Some(FamilyError::DuplicateName("wall_t".into()))
    );
    assert_eq!(
        compile_library_shared(&[fam("A", &["wall_t", "wall_t"], vec![])], &s).err(),
        Some(FamilyError::DuplicateName("wall_t".into()))
    );
    // a family without shared names compiles the same with or without a set
    assert!(compile_library_shared(&[fam("A", &[], vec![])], &s).is_ok());
    assert!(compile_library(&[fam("A", &[], vec![])]).is_ok());
}

#[test]
fn set_checks() {
    assert!(set().check().is_ok());
    let mut s = set();
    s.params.push(sp(
        9,
        "g",
        ValueDef::input("wall_t", 0.1, 0.0, 1.0, Scope::Type),
    ));
    assert_eq!(
        s.check().err(),
        Some(FamilyError::DuplicateName("wall_t".into()))
    );
    let mut s = set();
    s.params.push(sp(
        1,
        "g",
        ValueDef::input("other", 0.1, 0.0, 1.0, Scope::Type),
    ));
    assert!(matches!(s.check(), Err(FamilyError::Shared { .. })));
    let mut s = set();
    s.params.push(sp(
        0,
        "g",
        ValueDef::input("other", 0.1, 0.0, 1.0, Scope::Type),
    ));
    assert!(matches!(s.check(), Err(FamilyError::Shared { .. })));
    let mut s = set();
    s.params.push(sp(9, "g", ValueDef::formula("calc", "1")));
    assert!(matches!(s.check(), Err(FamilyError::Shared { .. })));
    let mut s = set();
    s.params
        .push(sp(9, "g", ValueDef::plane("p", Axis::X, "1")));
    assert!(matches!(s.check(), Err(FamilyError::Shared { .. })));
    // invalid definitions are found without any family using them
    for bad in [
        ValueDef::input("a", 5.0, 0.0, 1.0, Scope::Type),
        ValueDef::input("a", 0.5, 1.0, 0.0, Scope::Type),
        ValueDef::choice("a", &["x"], "y", Scope::Type),
        ValueDef::input("1bad", 0.5, 0.0, 1.0, Scope::Type),
    ] {
        let mut s = set();
        s.params.push(sp(9, "g", bad));
        assert!(s.check().is_err());
    }
    // group and description text
    let mut s = set();
    s.params[0].group = "x".repeat(64);
    assert!(s.check().is_ok());
    s.params[0].group = "x".repeat(65);
    assert!(matches!(s.check(), Err(FamilyError::BadText { .. })));
    s.params[0].group = "a\tb".into();
    assert!(matches!(s.check(), Err(FamilyError::BadText { .. })));
    let mut s = set();
    s.params[0].description = "y".repeat(256);
    assert!(s.check().is_ok());
    s.params[0].description = "y".repeat(257);
    assert!(matches!(s.check(), Err(FamilyError::BadText { .. })));
    s.params[0].description = "line\nbreak".into();
    assert!(matches!(s.check(), Err(FamilyError::BadText { .. })));
    // size limit
    let many = |n: usize| SharedSet {
        params: (0..n)
            .map(|i| {
                sp(
                    i as u128 + 1,
                    "g",
                    ValueDef::flag(&format!("f{i}"), false, Scope::Type),
                )
            })
            .collect(),
    };
    assert!(many(10_000).check().is_ok());
    assert_eq!(
        many(10_001).check().err(),
        Some(FamilyError::TooMany("shared parameters"))
    );
    // compile_library_shared checks the set first
    let mut s = set();
    s.params[0].id = 0;
    assert!(compile_library_shared(&[fam("A", &[], vec![])], &s).is_err());
}

#[test]
fn text_roundtrip_is_exact() {
    let mut s = set();
    s.params.push(sp(
        0xdeadbeef_00000000_00000000_12345678,
        "G",
        ValueDef::input("tricky", 0.1 + 0.2, -1e-9, 1.0e9, Scope::Type),
    ));
    let t = s.to_text();
    assert!(t.starts_with("# id\tname"));
    let back = SharedSet::from_text(&t).unwrap();
    assert_eq!(back.to_text(), t);
    assert_eq!(back.params.len(), 7);
    for (a, b) in s.params.iter().zip(&back.params) {
        assert_eq!(
            (a.id, &a.group, &a.description, &a.def.name),
            (b.id, &b.group, &b.description, &b.def.name)
        );
    }
    match &back.params[6].def.kind {
        ValueKind::Input { default, .. } => assert_eq!(*default, 0.1 + 0.2),
        _ => panic!(),
    }
    match &back.params[5].def.kind {
        ValueKind::Input { integer, .. } => assert!(*integer),
        _ => panic!(),
    }
    match &back.params[0].def.kind {
        ValueKind::Input { integer, .. } => assert!(!*integer),
        _ => panic!(),
    }
    // comments, blank lines and Windows line ends are accepted
    let crlf = t.replace('\n', "\r\n") + "\r\n\r\n# end\r\n";
    assert_eq!(SharedSet::from_text(&crlf).unwrap().to_text(), t);
    assert_eq!(SharedSet::from_text("").unwrap().params.len(), 0);
}

#[allow(clippy::too_many_arguments)]
fn line(
    id: &str,
    name: &str,
    kind: &str,
    scope: &str,
    d: &str,
    mn: &str,
    mx: &str,
    o: &str,
) -> String {
    format!("{id}\t{name}\t{kind}\t{scope}\t{d}\t{mn}\t{mx}\t{o}\tg\tdesc")
}
const ID: &str = "00000000000000000000000000000001";

#[test]
fn text_errors_name_the_line() {
    let ok = line(ID, "a", "number", "type", "1", "0", "2", "");
    assert!(SharedSet::from_text(&ok).is_ok());
    let cases: Vec<(String, usize)> = vec![
        (format!("# c\n\n{}\tx", ok), 3),
        (line("123", "a", "number", "type", "1", "0", "2", ""), 1),
        (
            line(
                "0000000000000000000000000000000g",
                "a",
                "number",
                "type",
                "1",
                "0",
                "2",
                "",
            ),
            1,
        ),
        (
            line(
                "0000000000000000000000000000000+",
                "a",
                "number",
                "type",
                "1",
                "0",
                "2",
                "",
            ),
            1,
        ),
        (
            line(
                "+0000000000000000000000000000001",
                "a",
                "number",
                "type",
                "1",
                "0",
                "2",
                "",
            ),
            1,
        ),
        (line(ID, "1a", "number", "type", "1", "0", "2", ""), 1),
        (line(ID, "a", "number", "global", "1", "0", "2", ""), 1),
        (line(ID, "a", "number", "type", "x", "0", "2", ""), 1),
        (line(ID, "a", "number", "type", "1", "NaN", "2", ""), 1),
        (line(ID, "a", "number", "type", "1", "0", "inf", ""), 1),
        (line(ID, "a", "count", "type", "1", "0", "", ""), 1),
        (line(ID, "a", "flag", "type", "yes", "", "", ""), 1),
        (line(ID, "a", "wibble", "type", "1", "", "", ""), 1),
        (
            format!("{ok}\n{}", line(ID, "b", "flag", "type", "0", "", "", "")),
            0,
        ), // duplicate identity: whole-set check
    ];
    for (text, ln) in cases {
        let e = SharedSet::from_text(&text)
            .err()
            .unwrap_or_else(|| panic!("accepted: {text}"));
        assert_eq!(e.0, ln, "{text}: {:?}", e);
    }
    // flag spellings
    for (d, v) in [("0", false), ("false", false), ("1", true), ("true", true)] {
        let s = SharedSet::from_text(&line(ID, "a", "flag", "instance", d, "", "", "")).unwrap();
        assert!(
            matches!(s.params[0].def.kind, ValueKind::Flag { default, scope: Scope::Instance } if default == v)
        );
    }
    // a choice whose default is not an option is refused by the whole-set check
    let e = SharedSet::from_text(&line(ID, "a", "choice", "type", "z", "", "", "x|y")).unwrap_err();
    assert_eq!(e.0, 0);
    assert!(SharedSet::from_text(&line(ID, "a", "choice", "type", "y", "", "", "x|y")).is_ok());
    // 10,001 lines
    let big: String = (0..10_001)
        .map(|i| {
            line(
                &format!("{:032x}", i + 1),
                &format!("f{i}"),
                "flag",
                "type",
                "0",
                "",
                "",
                "",
            ) + "\n"
        })
        .collect();
    assert_eq!(
        SharedSet::from_text(&big).unwrap_err(),
        (10_001, FamilyError::TooMany("shared parameters"))
    );
}

proptest! {
    #[test]
    fn prop_text_roundtrip(items in prop::collection::vec((1u128..u128::MAX, "[a-z][a-z0-9_]{0,10}", 0.0f64..100.0, 0.0f64..100.0, 0.0f64..1.0, any::<bool>()), 0..12)) {
        let mut seen = std::collections::HashSet::new();
        let mut ids = std::collections::HashSet::new();
        let mut s = SharedSet::default();
        for (id, name, a, b, f, count) in items {
            if !seen.insert(name.clone()) || !ids.insert(id) { continue; }
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            let d = lo + (hi - lo) * f;
            let def = if count { ValueDef::input(&name, d, lo, hi, Scope::Type) } else { ValueDef::input(&name, d, lo, hi, Scope::Instance) };
            s.params.push(sp(id, "grp", def));
        }
        prop_assume!(s.check().is_ok());
        let t = s.to_text();
        let back = SharedSet::from_text(&t).unwrap();
        prop_assert_eq!(back.to_text(), t);
    }
}
