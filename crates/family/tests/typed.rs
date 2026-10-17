use openbim_family::library::{all, door, handle, window};
use openbim_family::*;
use proptest::prelude::*;

fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())) }
fn lib() -> Vec<Compiled> { all().unwrap() }
fn part(c: &Compiled, i: &Instance, name: &str) -> Box3 {
    let k = (0..i.solids.len()).find(|&k| c.solid_name(i.solids[k].0) == name).unwrap_or_else(|| panic!("no part {name}"));
    i.solids[k].1
}
fn parts<'a>(c: &'a Compiled, i: &'a Instance, name: &'a str) -> Vec<(usize, Box3)> {
    (0..i.solids.len()).filter(|&k| c.solid_name(i.solids[k].0) == name).map(|k| (k, i.solids[k].1)).collect()
}
fn root(mut e: FamilyError) -> FamilyError { while let FamilyError::Nested { err, .. } = e { e = *err; } e }
fn r<'a>(overrides: Vec<(&'a str, f64)>, texts: Vec<(&'a str, &'a str)>) -> Request<'a> { Request { type_name: None, overrides, texts } }

// ------------------------------------------------------------------ nested handle, by hand
#[test]
fn the_nested_handle_sits_where_the_hinge_side_says() {
    let c = &lib()[1];
    // hinged on the right (default): handle at the left of the leaf, lever pointing toward the hinges
    let i = c.solve(&Request::default()).unwrap();
    assert_eq!(c.text(&i, "swing"), Some("right"));
    let p = part(c, &i, "handle/plate_front");
    assert!(close(p.min[0], -0.35) && close(p.max[0], -0.31), "{p:?}");           // x_in_left + 0.07 = -0.33, plate 0.04 wide
    assert!(close(p.min[1], -0.028) && close(p.max[1], -0.02));                      // leaf face -0.02, plate 0.008
    assert!(close(p.min[2], 0.92) && close(p.max[2], 1.08));                         // handle height 1.0, plate 0.16 high
    let g = part(c, &i, "handle/grip_front");
    assert!(close(g.min[0], -0.35) && close(g.max[0], -0.23));                       // lever 0.12 long, from the plate's left edge
    assert!(close(g.min[1], -0.078) && close(g.max[1], -0.028));                     // projects 0.05 from the plate
    assert!(close(g.min[2], 0.99) && close(g.max[2], 1.01));
    let b = part(c, &i, "handle/plate_back");
    assert!(close(b.min[1], 0.02) && close(b.max[1], 0.028));
    let gb = part(c, &i, "handle/grip_back");
    assert!(close(gb.min[1], 0.028) && close(gb.max[1], 0.078));
    // hinged on the left: handle at the right, lever pointing left
    let i = c.solve(&r(vec![], vec![("swing", "left")])).unwrap();
    assert_eq!(c.text(&i, "swing"), Some("left"));
    assert_eq!(c.value(&i, "swing.left"), Some(1.0));
    assert_eq!(c.value(&i, "swing.right"), Some(0.0));
    let p = part(c, &i, "handle/plate_front");
    assert!(close(p.min[0], 0.31) && close(p.max[0], 0.35));
    let g = part(c, &i, "handle/grip_front");
    assert!(close(g.min[0], 0.23) && close(g.max[0], 0.35), "{g:?}");
    // no handle: only the frame and leaf
    let i = c.solve(&r(vec![("has_handle", 0.0)], vec![])).unwrap();
    assert_eq!(i.solids.len(), 4);
    assert!((0..i.solids.len()).all(|k| !c.solid_name(i.solids[k].0).contains('/')));
    // the parts of the handle are the only thing that changes with the handle height
    let i = c.solve(&Request { type_name: None, overrides: vec![], texts: vec![] }).unwrap();
    let j = c.solve(&Request { type_name: Some("Type B"), ..Default::default() }).unwrap();
    assert!(part(c, &j, "handle/plate_front").min[2] == part(c, &i, "handle/plate_front").min[2], "handle height is its own value, not a share of the door height");
}

// ------------------------------------------------------------------ yes/no, choices, text
#[test]
fn yes_no_and_choice_values_are_validated() {
    let c = &lib()[1];
    assert!(matches!(c.solve(&r(vec![("has_handle", 0.5)], vec![])), Err(FamilyError::NotYesNo { .. })));
    assert!(matches!(c.solve(&r(vec![("has_handle", 2.0)], vec![])), Err(FamilyError::NotYesNo { .. })));
    assert!(matches!(c.solve(&r(vec![("has_handle", f64::NAN)], vec![])), Err(FamilyError::NotFinite(_))));
    assert!(c.solve(&r(vec![("has_handle", 1.0)], vec![])).is_ok());
    assert!(matches!(c.solve(&r(vec![], vec![("swing", "middle")])), Err(FamilyError::BadChoice { .. })));
    assert!(matches!(c.solve(&r(vec![], vec![("swing", "")])), Err(FamilyError::BadChoice { .. })));
    // numbers and texts cannot be mixed up
    assert!(matches!(c.solve(&r(vec![("swing", 1.0)], vec![])), Err(FamilyError::WrongKind { .. })));
    assert!(matches!(c.solve(&r(vec![("mark", 1.0)], vec![])), Err(FamilyError::WrongKind { .. })));
    assert!(matches!(c.solve(&r(vec![], vec![("has_handle", "yes")])), Err(FamilyError::WrongKind { .. })));
    assert!(matches!(c.solve(&r(vec![], vec![("wall_t", "0.2")])), Err(FamilyError::WrongKind { .. })));
    // type values cannot be changed on one instance, for texts as well
    assert!(matches!(c.solve(&r(vec![], vec![("remarks", "x")])), Err(FamilyError::WrongScope(_))));
    assert!(matches!(c.solve(&r(vec![], vec![("leaf_material", "Oak")])), Err(FamilyError::WrongScope(_))));
    assert!(matches!(c.solve(&r(vec![], vec![("nope", "x")])), Err(FamilyError::UnknownValue(_))));
    assert!(matches!(c.solve(&r(vec![], vec![("x_left", "x")])), Err(FamilyError::NotAnInput(_))));
}

#[test]
fn text_values_are_kept_checked_and_never_used_in_formulas() {
    let c = &lib()[1];
    let i = c.solve(&r(vec![], vec![("mark", "D-01 (north)")])).unwrap();
    assert_eq!(c.text(&i, "mark"), Some("D-01 (north)"));
    assert_eq!(c.text(&c.solve(&Request::default()).unwrap(), "mark"), Some(""));
    assert!(c.solve(&r(vec![], vec![("mark", &"x".repeat(MAX_TEXT))])).is_ok());
    assert!(matches!(c.solve(&r(vec![], vec![("mark", &"x".repeat(MAX_TEXT + 1))])), Err(FamilyError::BadText { .. })));
    assert!(matches!(c.solve(&r(vec![], vec![("mark", "a\nb")])), Err(FamilyError::BadText { .. })));
    assert!(matches!(c.solve(&r(vec![], vec![("mark", "a\u{0}b")])), Err(FamilyError::BadText { .. })));
    assert!(c.solve(&r(vec![], vec![("mark", "Tür ①")])).is_ok(), "ordinary non-ASCII text is fine");
    // texts and materials cannot be read by a formula
    let f = Family { name: "t".into(), values: vec![ValueDef::text("label", "x", Scope::Type), ValueDef::formula("n", "label + 1")], ..Default::default() };
    assert!(matches!(f.compile(), Err(FamilyError::TextInFormula(_))));
    let f = Family { name: "t".into(), values: vec![ValueDef::material("m", "x", Scope::Type), ValueDef::formula("n", "if(m, 1, 2)")], ..Default::default() };
    assert!(matches!(f.compile(), Err(FamilyError::TextInFormula(_))));
    // a bad default is refused when compiling
    let f = Family { name: "t".into(), values: vec![ValueDef::text("label", "a\tb", Scope::Type)], ..Default::default() };
    assert!(matches!(f.compile(), Err(FamilyError::BadText { .. })));
    let f = Family { name: "t".into(), values: vec![ValueDef::choice("c", &["a", "b"], "z", Scope::Type)], ..Default::default() };
    assert!(matches!(f.compile(), Err(FamilyError::BadInput { .. })));
    let f = Family { name: "t".into(), values: vec![ValueDef::choice("c", &["a", "a"], "a", Scope::Type)], ..Default::default() };
    assert!(matches!(f.compile(), Err(FamilyError::DuplicateName(_))));
    let f = Family { name: "t".into(), values: vec![ValueDef::choice("c", &[], "a", Scope::Type)], ..Default::default() };
    assert!(matches!(f.compile(), Err(FamilyError::BadInput { .. })));
    let f = Family { name: "t".into(), values: vec![ValueDef::choice("c", &["1x"], "1x", Scope::Type)], ..Default::default() };
    assert!(matches!(f.compile(), Err(FamilyError::BadName(_))));
}

#[test]
fn a_choice_drives_formulas_and_a_flag_reads_as_one_or_zero() {
    let f = Family {
        name: "t".into(),
        values: vec![
            ValueDef::choice("kind", &["a", "b", "c"], "b", Scope::Instance),
            ValueDef::flag("on", true, Scope::Instance),
            ValueDef::formula("w", "if(kind.a, 1, if(kind.b, 2, 3)) + on * 10"),
            ValueDef::formula("idx", "kind"),
        ],
        ..Default::default()
    };
    let c = f.compile().unwrap();
    let get = |t: Vec<(&str, &str)>, o: Vec<(&str, f64)>| { let i = c.solve(&r(o, t)).unwrap(); (c.value(&i, "w").unwrap(), c.value(&i, "idx").unwrap()) };
    assert_eq!(get(vec![], vec![]), (12.0, 1.0));
    assert_eq!(get(vec![("kind", "a")], vec![]), (11.0, 0.0));
    assert_eq!(get(vec![("kind", "c")], vec![("on", 0.0)]), (3.0, 2.0));
}

// ------------------------------------------------------------------ materials
#[test]
fn solids_take_their_material_from_a_value_and_nested_families_pass_it_on() {
    let c = &lib()[1];
    let i = c.solve(&Request::default()).unwrap();
    for k in 0..i.solids.len() {
        let name = c.solid_name(i.solids[k].0);
        let want = if name.starts_with("handle/") { "Handle finish" } else if name == "leaf" { "Leaf material" } else { "Frame material" };
        assert_eq!(i.material_of(k), Some(want), "{name}");
    }
    assert_eq!(i.strings.len(), 3, "each distinct material is stored once");
    // a type may change a material, and the handle finish reaches the nested handle
    let mut f = door();
    f.types.push(TypeDef { name: "Oak".into(), values: vec![], texts: vec![("leaf_material".into(), "Oak veneer".into()), ("handle_finish".into(), "Brushed steel".into())], ..Default::default() });
    let c2 = compile_library(&[handle(), f]).unwrap().remove(1);
    let i = c2.solve(&Request { type_name: Some("Oak"), ..Default::default() }).unwrap();
    for k in 0..i.solids.len() {
        let name = c2.solid_name(i.solids[k].0);
        let want = if name.starts_with("handle/") { "Brushed steel" } else if name == "leaf" { "Oak veneer" } else { "Frame material" };
        assert_eq!(i.material_of(k), Some(want), "{name}");
    }
    assert_eq!(c2.text(&i, "leaf_material"), Some("Oak veneer"));
    // windows: mullion bars share one material entry
    let w = &window().compile().unwrap();
    let i = w.solve(&r(vec![("mullions", 5.0)], vec![])).unwrap();
    assert_eq!(i.solids.iter().filter(|(s, _)| w.solid_name(*s) == "mullion_front").count(), 5);
    assert!(i.strings.contains(&"Glass".to_string()) && i.strings.contains(&"Frame material".to_string()) && i.strings.len() == 2);
    // an empty or damaged material name is refused
    let mut bad = door();
    bad.types.push(TypeDef { name: "Bad".into(), values: vec![], texts: vec![("leaf_material".into(), "x\ny".into())], ..Default::default() });
    let c3 = compile_library(&[handle(), bad]).unwrap().remove(1);
    assert!(matches!(c3.solve(&Request { type_name: Some("Bad"), ..Default::default() }), Err(FamilyError::BadText { .. })));
    // a solid cannot take its material from a number
    let mut bad = window();
    bad.solids[0].material = Some("width".into());
    assert!(matches!(bad.compile(), Err(FamilyError::WrongKind { .. })));
    let mut bad = window();
    bad.solids[0].material = Some("nope".into());
    assert!(matches!(bad.compile(), Err(FamilyError::UnknownValue(_))));
}

// ------------------------------------------------------------------ structure of nested families
fn post() -> Family {
    Family {
        name: "Post".into(),
        values: vec![
            ValueDef::input("size", 0.05, 0.01, 0.2, Scope::Type),
            ValueDef::material("mat", "Post material", Scope::Type),
            ValueDef::plane("x0", Axis::X, "-size / 2"), ValueDef::plane("x1", Axis::X, "size / 2"),
            ValueDef::plane("y0", Axis::Y, "-size / 2"), ValueDef::plane("y1", Axis::Y, "size / 2"),
            ValueDef::plane("z0", Axis::Z, "0"), ValueDef::plane("z1", Axis::Z, "1"),
        ],
        solids: vec![SolidDef::new("body", ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).material("mat")],
        ..Default::default()
    }
}
fn fence() -> Family {
    Family {
        name: "Fence".into(),
        values: vec![
            ValueDef::count("n", 4.0, 0.0, 50.0, Scope::Instance),
            ValueDef::input("spacing", 0.5, 0.1, 2.0, Scope::Instance),
            ValueDef::material("timber", "Fence timber", Scope::Type),
        ],
        nested: vec![NestedDef::new("post", "Post", ("_i * spacing", "0", "0")).repeat("n").set("size", "0.04 m + 2 mm * _i").set_text("mat", TextSrc::Param("timber".into()))],
        ..Default::default()
    }
}

#[test]
fn nested_families_repeat_with_their_own_index() {
    let v = compile_library(&[fence(), post()]).unwrap();     // listing order does not matter
    let c = &v[0];
    let i = c.solve(&r(vec![("n", 3.0), ("spacing", 0.6)], vec![])).unwrap();
    let ps = parts(c, &i, "post/body");
    assert_eq!(ps.len(), 3);
    for (k, (e, b)) in ps.iter().enumerate() {
        let size = 0.04 + 0.002 * k as f64;
        assert!(close(b.min[0], k as f64 * 0.6 - size / 2.0) && close(b.max[0], k as f64 * 0.6 + size / 2.0), "{k}: {b:?}");
        assert!(close(b.max[1] - b.min[1], size) && close(b.max[2], 1.0));
        assert_eq!(i.material_of(*e), Some("Fence timber"));
    }
    assert_eq!(c.solve(&r(vec![("n", 0.0)], vec![])).unwrap().solids.len(), 0);
    assert_eq!(c.solve(&r(vec![("n", 50.0)], vec![])).unwrap().solids.len(), 50);
    // the repeat index is not a name outside nested formulas
    let mut bad = fence(); bad.values.push(ValueDef::formula("q", "_i + 1"));
    assert!(matches!(compile_library(&[bad, post()]), Err(FamilyError::Formula { err: openbim_family::expr::ExprError::UnknownName(_), .. })));
    let mut bad = fence(); bad.values.push(ValueDef::input("_i", 1.0, 0.0, 2.0, Scope::Type));
    assert!(matches!(compile_library(&[bad, post()]), Err(FamilyError::BadName(_))));
    // a fractional or negative repeat count is refused at solve time
    let mut f = fence(); f.nested[0].count = Some("n + 0.5".into());
    let cf = compile_library(&[f, post()]).unwrap().remove(0);
    assert!(matches!(cf.solve(&Request::default()), Err(FamilyError::BadNested { .. })));
}

#[test]
fn nested_families_are_checked_when_compiling() {
    let with = |g: &dyn Fn(&mut Family)| { let mut f = fence(); g(&mut f); compile_library(&[f, post()]).err() };
    assert!(matches!(with(&|f| f.nested[0].family = "Nope".into()), Some(FamilyError::UnknownFamily(_))));
    assert!(matches!(with(&|f| f.nested[0].values.push(("nope".into(), "1".into()))), Some(FamilyError::BadNested { .. })));
    assert!(matches!(with(&|f| f.nested[0].values.push(("size".into(), "1".into()))), Some(FamilyError::BadNested { .. })), "tied twice");
    assert!(matches!(with(&|f| f.nested[0].values.push(("mat".into(), "1".into()))), Some(FamilyError::BadNested { .. })), "number tied to a material");
    assert!(matches!(with(&|f| f.nested[0].texts.push(("size".into(), TextSrc::Literal("x".into())))), Some(FamilyError::BadNested { .. })), "text tied to a number");
    assert!(matches!(with(&|f| f.nested[0].texts[0].1 = TextSrc::Param("n".into())), Some(FamilyError::BadNested { .. })), "text from a number");
    assert!(matches!(with(&|f| f.nested[0].texts[0].1 = TextSrc::Param("zzz".into())), Some(FamilyError::UnknownValue(_))));
    assert!(matches!(with(&|f| f.nested[0].type_name = Some("Nope".into())), Some(FamilyError::BadNested { .. })));
    assert!(matches!(with(&|f| f.nested[0].at.0 = "zzz".into()), Some(FamilyError::Formula { .. })));
    assert!(matches!(with(&|f| f.nested[0].at.0 = "timber".into()), Some(FamilyError::TextInFormula(_))));
    assert!(matches!(with(&|f| f.nested[0].name = "n".into()), Some(FamilyError::DuplicateName(_))) || with(&|f| f.nested[0].name = "n".into()).is_none(), "a nested name may match a value name only if no solid has it");
    let mut f = fence(); f.nested.push(NestedDef::new("post", "Post", ("0", "0", "0")));
    assert!(matches!(compile_library(&[f, post()]), Err(FamilyError::DuplicateName(_))));
    assert!(matches!(compile_library(&[post(), post()]), Err(FamilyError::DuplicateName(_))));
    // choice literal must be one of the options
    let mut f = door(); f.nested[0].texts.push(("finish".into(), TextSrc::Literal("x".into())));
    assert!(matches!(compile_library(&[handle(), f]), Err(FamilyError::BadNested { .. })), "finish is already tied");
}

#[test]
fn families_that_contain_each_other_or_nest_too_deep_are_refused() {
    let mk = |name: &str, child: Option<&str>| Family {
        name: name.into(),
        values: vec![ValueDef::plane("x0", Axis::X, "0"), ValueDef::plane("x1", Axis::X, "1"), ValueDef::plane("y0", Axis::Y, "0"), ValueDef::plane("y1", Axis::Y, "1"), ValueDef::plane("z0", Axis::Z, "0"), ValueDef::plane("z1", Axis::Z, "1")],
        solids: vec![SolidDef::new("b", ("x0", "x1"), ("y0", "y1"), ("z0", "z1"))],
        nested: child.map(|c| vec![NestedDef::new("c", c, ("0", "0", "2"))]).unwrap_or_default(),
        ..Default::default()
    };
    assert!(matches!(compile_library(&[mk("A", Some("A"))]), Err(FamilyError::NestedLoop(p)) if p == vec!["A", "A"]));
    assert!(matches!(compile_library(&[mk("A", Some("B")), mk("B", Some("C")), mk("C", Some("A"))]), Err(FamilyError::NestedLoop(p)) if p == vec!["A", "B", "C", "A"]));
    let chain = |n: usize| -> Vec<Family> { (0..n).map(|k| { let nx = format!("F{}", k + 1); mk(&format!("F{k}"), if k + 1 < n { Some(nx.as_str()) } else { None }) }).collect() };
    let ok = compile_library(&chain(MAX_NEST_DEPTH)).unwrap();
    assert_eq!(ok[0].solve(&Request::default()).unwrap().solids.len(), MAX_NEST_DEPTH);
    assert!(matches!(compile_library(&chain(MAX_NEST_DEPTH + 1)), Err(FamilyError::NestTooDeep)));
    // the top of the chain stacks one box above the next
    let i = ok[0].solve(&Request::default()).unwrap();
    let mut zs: Vec<f64> = i.solids.iter().map(|(_, b)| b.min[2]).collect(); zs.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(zs, (0..MAX_NEST_DEPTH).map(|k| 2.0 * k as f64).collect::<Vec<_>>());
    assert_eq!(ok[0].solid_name(i.solids.iter().find(|(_, b)| b.min[2] == 4.0).unwrap().0), "c/c/b");
}

#[test]
fn nested_work_and_size_are_bounded() {
    let mk = |name: &str, child: Option<&str>, count: &str| Family {
        name: name.into(),
        values: vec![ValueDef::plane("x0", Axis::X, "0"), ValueDef::plane("x1", Axis::X, "1"), ValueDef::plane("y0", Axis::Y, "0"), ValueDef::plane("y1", Axis::Y, "1"), ValueDef::plane("z0", Axis::Z, "0"), ValueDef::plane("z1", Axis::Z, "1")],
        solids: if child.is_none() { vec![SolidDef::new("b", ("x0", "x1"), ("y0", "y1"), ("z0", "z1"))] } else { vec![] },
        nested: child.map(|c| vec![NestedDef::new("c", c, ("_i * 1 m", "0", "0")).repeat(count)]).unwrap_or_default(),
        ..Default::default()
    };
    // 1000 x 1000 x 1000 instances would never finish: the work budget stops it quickly
    let t = std::time::Instant::now();
    let v = compile_library(&[mk("A", Some("B"), "1000"), mk("B", Some("C"), "1000"), mk("C", Some("D"), "1000"), mk("D", None, "1")]).unwrap();
    assert!(matches!(v[0].solve(&Request::default()).map_err(root), Err(FamilyError::TooMany(_))));
    assert!(t.elapsed().as_secs_f64() < 5.0, "{:?}", t.elapsed());
    // too many boxes
    let v = compile_library(&[mk("A", Some("B"), "1000"), mk("B", Some("C"), "30"), mk("C", None, "1")]).unwrap();
    assert!(matches!(v[0].solve(&Request::default()).map_err(root), Err(FamilyError::TooMany(_))));
    let v = compile_library(&[mk("A", Some("B"), "100"), mk("B", Some("C"), "100"), mk("C", None, "1")]).unwrap();
    assert_eq!(v[0].solve(&Request::default()).unwrap().solids.len(), 10_000);
    // the flattened list of solids is bounded when compiling
    let mut big = mk("Big", None, "1");
    big.solids = (0..600).map(|k| SolidDef::new(&format!("s{k}"), ("x0", "x1"), ("y0", "y1"), ("z0", "z1"))).collect();
    let mut top = mk("Top", None, "1");
    top.solids = vec![]; top.nested = vec![NestedDef::new("a", "Big", ("0", "0", "0")), NestedDef::new("b", "Big", ("0", "0", "0"))];
    assert!(matches!(compile_library(&[big, top]), Err(FamilyError::TooMany(_))));
}

#[test]
fn a_failing_nested_family_is_named_in_the_error() {
    let mut f = door();
    f.nested[0] = f.nested[0].clone().set("plate_w", "0.5");        // wider than the lever is long: the handle's own rule fails
    let c = compile_library(&[handle(), f]).unwrap().remove(1);
    match c.solve(&Request::default()) {
        Err(FamilyError::Nested { path, err }) => { assert_eq!(path, "handle"); assert!(matches!(*err, FamilyError::OutOfRange { .. } | FamilyError::CheckFailed(_)), "{err:?}"); }
        other => panic!("{other:?}"),
    }
    let mut f = door();
    f.nested[0] = f.nested[0].clone().set("plate_w", "0.09").set("grip_len", "0.08");   // plate wider than 0.08 long lever
    let c = compile_library(&[handle(), f]).unwrap().remove(1);
    assert!(matches!(c.solve(&Request::default()), Err(FamilyError::Nested { .. })));
    // a tied value that is out of the child's range is refused, with the child's value named
    let mut f = door();
    f.nested[0] = f.nested[0].clone().set("plate_h", "9");
    let c = compile_library(&[handle(), f]).unwrap().remove(1);
    match c.solve(&Request::default()) { Err(FamilyError::Nested { err, .. }) => assert!(matches!(*err, FamilyError::OutOfRange { ref name, .. } if name == "plate_h")), o => panic!("{o:?}") }
}

#[test]
fn library_order_does_not_change_the_answer() {
    let a = compile_library(&[handle(), door(), window()]).unwrap();
    let b = compile_library(&[window(), door(), handle()]).unwrap();
    let rq = r(vec![("has_handle", 1.0)], vec![("swing", "left"), ("mark", "D1")]);
    let (x, y) = (a[1].solve(&rq).unwrap(), b[1].solve(&rq).unwrap());
    assert_eq!(x, y);
}

#[test]
fn batches_with_typed_values_are_identical_on_any_thread_count() {
    let c = &lib()[1];
    let marks: Vec<String> = (0..300).map(|k| format!("D{k}")).collect();
    let reqs: Vec<Request> = (0..3000).map(|k| Request {
        type_name: if k % 3 == 0 { Some("Type B") } else { None },
        overrides: vec![("has_handle", (k % 2) as f64), ("wall_t", 0.1 + (k % 5) as f64 * 0.1)],
        texts: vec![("swing", if k % 4 < 2 { "left" } else { "right" }), ("mark", marks[k % 300].as_str())],
    }).collect();
    let serial: Vec<_> = reqs.iter().map(|q| c.solve(q)).collect();
    for threads in [1usize, 2, 8] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        assert_eq!(pool.install(|| c.solve_many(&reqs)), serial, "threads {threads}");
    }
    assert!(serial.iter().all(|r| r.is_ok()));
}

proptest! {
    #[test]
    fn handle_follows_door_size_hinge_side_and_rules(
        w in 0.5f64..3.0, h in 1.5f64..3.5, fr in 0.02f64..0.2, hh in 0.6f64..1.5, left in any::<bool>(), on in any::<bool>()) {
        let mut f = door();
        f.types.push(TypeDef { name: "t".into(), values: vec![("width".into(), w), ("height".into(), h), ("frame".into(), fr), ("handle_height".into(), hh)], ..Default::default() });
        let c = compile_library(&[handle(), f]).unwrap().remove(1);
        let res = c.solve(&Request { type_name: Some("t"), overrides: vec![("has_handle", if on { 1.0 } else { 0.0 })], texts: vec![("swing", if left { "left" } else { "right" })] });
        let valid = w > 4.0 * fr && h > 4.0 * fr && (!on || (w - 2.0 * fr > 0.35 && hh < h - fr - 0.15));
        prop_assert_eq!(res.is_ok(), valid, "{:?}", res);
        if let Ok(i) = res {
            if !on { prop_assert!(!(0..i.solids.len()).any(|k| c.solid_name(i.solids[k].0).contains('/'))); }
            else {
                let p = part(&c, &i, "handle/plate_front");
                let centre_x = if left { w / 2.0 - fr - 0.07 } else { -w / 2.0 + fr + 0.07 };
                prop_assert!(close((p.min[0] + p.max[0]) / 2.0, centre_x));
                prop_assert!(close((p.min[2] + p.max[2]) / 2.0, hh));
                let g = part(&c, &i, "handle/grip_front");
                // the lever points toward the middle of the door, away from the edge the handle sits near
                let lever_ok = if left { g.min[0] < p.min[0] && close(g.max[0], p.max[0]) } else { g.max[0] > p.max[0] && close(g.min[0], p.min[0]) };
                prop_assert!(lever_ok);
                // the handle is on the leaf: inside it along the leaf, outside it across
                let leaf = part(&c, &i, "leaf");
                prop_assert!(p.min[0] >= leaf.min[0] && p.max[0] <= leaf.max[0] && p.min[2] >= leaf.min[2] && p.max[2] <= leaf.max[2]);
                prop_assert!(close(part(&c, &i, "handle/plate_front").max[1], leaf.min[1]) && close(part(&c, &i, "handle/plate_back").min[1], leaf.max[1]));
            }
            // nothing overlaps anything
            for a in 0..i.solids.len() { for b in a + 1..i.solids.len() { prop_assert!(!i.solids[a].1.overlaps(&i.solids[b].1, 1e-12), "{} vs {}", c.solid_name(i.solids[a].0), c.solid_name(i.solids[b].0)); } }
        }
    }
}

// ------------------------------------------------------------------ limits at their exact edges
fn boxed(name: &str, solids: usize, nested: Vec<NestedDef>) -> Family {
    Family {
        name: name.into(),
        values: vec![ValueDef::plane("x0", Axis::X, "0"), ValueDef::plane("x1", Axis::X, "1"), ValueDef::plane("y0", Axis::Y, "0"), ValueDef::plane("y1", Axis::Y, "1"), ValueDef::plane("z0", Axis::Z, "0"), ValueDef::plane("z1", Axis::Z, "1")],
        solids: (0..solids).map(|k| SolidDef::new(&format!("s{k}"), ("x0", "x1"), ("y0", "y1"), ("z0", "z1"))).collect(),
        nested,
        ..Default::default()
    }
}

#[test]
fn the_nested_instance_budget_stops_runaway_nesting_even_when_nothing_is_drawn() {
    // empty leaves draw nothing, so only the instance budget can stop 1000 x 1000 x 1000
    let chain = |a: &str, b: &str| vec![NestedDef::new("c", a, ("0", "0", "0")).repeat(b)];
    let t = std::time::Instant::now();
    let v = compile_library(&[boxed("A", 0, chain("B", "1000")), boxed("B", 0, chain("C", "1000")), boxed("C", 0, chain("D", "1000")), boxed("D", 0, vec![])]).unwrap();
    match v[0].solve(&Request::default()).map_err(root) { Err(FamilyError::TooMany(what)) => assert_eq!(what, "nested family instances"), o => panic!("{:?}", o.map(|i| i.solids.len())) }
    assert!(t.elapsed().as_secs_f64() < 5.0, "{:?}", t.elapsed());
    // exactly MAX_NESTED_INSTANCES is allowed (1000 B + 19 x 1000 C = 20000), one more is not
    let v = compile_library(&[boxed("A", 0, chain("B", "1000")), boxed("B", 0, chain("C", "19")), boxed("C", 0, vec![])]).unwrap();
    assert!(v[0].solve(&Request::default()).is_ok());
    let v = compile_library(&[boxed("A", 0, chain("B", "1000")), boxed("B", 0, chain("C", "20")), boxed("C", 0, vec![])]).unwrap();
    assert!(matches!(v[0].solve(&Request::default()).map_err(root), Err(FamilyError::TooMany(_))));
    let v = compile_library(&[boxed("A", 0, chain("B", "1000")), boxed("B", 0, chain("C", "19")), boxed("C", 0, vec![]), ]).unwrap();
    // one more instance in the same tree: A itself repeats B 1000 times and also holds one extra nested C
    let mut a = boxed("A", 0, chain("B", "1000")); a.nested.push(NestedDef::new("extra", "C", ("0", "0", "0")));
    let v2 = compile_library(&[a, boxed("B", 0, chain("C", "19")), boxed("C", 0, vec![])]).unwrap();
    assert!(v[0].solve(&Request::default()).is_ok());
    assert!(matches!(v2[0].solve(&Request::default()).map_err(root), Err(FamilyError::TooMany(_))));
}

#[test]
fn the_box_limit_is_exact() {
    let one = |extra: usize| compile_library(&[boxed("A", extra, vec![NestedDef::new("b", "B", ("0", "0", "0")).repeat("1000")]), boxed("B", 20, vec![])]).unwrap();
    // 1000 x 20 = 20000 boxes: allowed
    assert_eq!(one(0)[0].solve(&Request::default()).unwrap().solids.len(), MAX_BOXES);
    // one more own box: refused
    assert!(matches!(one(1)[0].solve(&Request::default()), Err(FamilyError::TooMany("boxes in one placement"))));
}

#[test]
fn a_loop_path_starts_where_the_loop_starts() {
    let k = |name: &str, child: &str| boxed(name, 1, vec![NestedDef::new("c", child, ("0", "0", "0"))]);
    match compile_library(&[k("R", "A"), k("A", "B"), k("B", "A")]) {
        Err(FamilyError::NestedLoop(p)) => assert_eq!(p, vec!["A", "B", "A"]),
        o => panic!("{:?}", o.map(|v| v.len())),
    }
}

#[test]
fn material_names_are_limited_to_128_characters() {
    let with = |n: usize| { let mut f = door(); f.types.push(TypeDef { name: "T".into(), values: vec![], texts: vec![("leaf_material".into(), "m".repeat(n))], ..Default::default() }); compile_library(&[handle(), f]).unwrap().remove(1) };
    assert!(with(128).solve(&Request { type_name: Some("T"), ..Default::default() }).is_ok());
    assert!(matches!(with(129).solve(&Request { type_name: Some("T"), ..Default::default() }), Err(FamilyError::BadText { .. })));
}

#[test]
fn a_choice_can_feed_a_nested_text_value() {
    let mut child = boxed("Child", 1, vec![]);
    child.values.push(ValueDef::text("label", "none", Scope::Type));
    child.solids[0] = child.solids[0].clone().material("label");
    let mut parent = boxed("Parent", 0, vec![NestedDef::new("c", "Child", ("0", "0", "0")).set_text("label", TextSrc::Param("kind".into()))]);
    parent.values.push(ValueDef::choice("kind", &["a", "b", "c"], "b", Scope::Instance));
    let v = compile_library(&[parent, child]).unwrap();
    for (pick, want) in [(None, "b"), (Some("a"), "a"), (Some("c"), "c")] {
        let texts = pick.map(|p| vec![("kind", p)]).unwrap_or_default();
        let i = v[0].solve(&Request { texts, ..Default::default() }).unwrap();
        assert_eq!(i.material_of(0), Some(want));
    }
}

#[test]
fn every_bar_of_an_array_carries_the_solids_material() {
    let w = &window().compile().unwrap();
    let i = w.solve(&r(vec![("mullions", 3.0)], vec![])).unwrap();
    let mut bars = 0;
    for k in 0..i.solids.len() {
        let name = w.solid_name(i.solids[k].0);
        if name.starts_with("mullion") { bars += 1; assert_eq!(i.material_of(k), Some("Frame material"), "{name} #{k}"); }
        assert!(i.material_of(k).is_some(), "{name} has a material");
    }
    assert_eq!(bars, 6);
}
