use openbim_family::expr::{Expr, ExprError, MAX_DEPTH, MAX_LEN};
use openbim_family::library::{all, door, handle, window};
use openbim_family::*;
use proptest::prelude::*;

fn ev(s: &str) -> Result<f64, ExprError> { Expr::parse(s, &|_| None)?.eval(&[]) }
fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())) }

fn bbox(i: &Instance) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::MAX; 3]; let mut hi = [f64::MIN; 3];
    for (_, b) in &i.solids { for k in 0..3 { lo[k] = lo[k].min(b.min[k]); hi[k] = hi[k].max(b.max[k]); } }
    (lo, hi)
}
fn no_overlaps(i: &Instance) -> bool {
    for a in 0..i.solids.len() { for b in a + 1..i.solids.len() { if i.solids[a].1.overlaps(&i.solids[b].1, 1e-12) { return false; } } }
    true
}
fn total_volume(i: &Instance) -> f64 { i.solids.iter().map(|(_, b)| b.volume()).sum() }
/// Compile a family together with the handle family it may nest.
fn cd(f: &Family) -> Result<Compiled, FamilyError> { compile_library(&[handle(), f.clone()]).map(|mut v| v.remove(1)) }
fn cdoor() -> Compiled { all().unwrap()[1].clone() }
/// The instance without the parts of nested families.
fn frame_parts(c: &Compiled, i: &Instance) -> Instance {
    let keep: Vec<usize> = (0..i.solids.len()).filter(|&k| !c.solid_name(i.solids[k].0).contains('/')).collect();
    Instance { solids: keep.iter().map(|&k| i.solids[k]).collect(), materials: keep.iter().map(|&k| i.materials[k]).collect(), ..i.clone() }
}
fn req<'a>(o: Vec<(&'a str, f64)>) -> Request<'a> { Request { type_name: None, overrides: o, ..Default::default() } }

// ------------------------------------------------------------------ formula language
#[test]
fn formulas_follow_ordinary_arithmetic() {
    assert_eq!(ev("1 + 2 * 3").unwrap(), 7.0);
    assert_eq!(ev("(1 + 2) * 3").unwrap(), 9.0);
    assert_eq!(ev("10 - 4 - 3").unwrap(), 3.0);
    assert_eq!(ev("100 / 5 / 4").unwrap(), 5.0);
    assert_eq!(ev("-2 * 3").unwrap(), -6.0);
    assert_eq!(ev("- - 4").unwrap(), 4.0);
    assert_eq!(ev("1e-3 * 2").unwrap(), 0.002);
    assert_eq!(ev("2.5e2").unwrap(), 250.0);
    assert_eq!(ev("3 > 2 && 2 > 1").unwrap(), 1.0);
    assert_eq!(ev("3 < 2 || 2 > 1").unwrap(), 1.0);
    assert_eq!(ev("!(3 < 2)").unwrap(), 1.0);
    assert_eq!(ev("1 + 1 == 2").unwrap(), 1.0);
    assert_eq!(ev("1 + 1 != 2").unwrap(), 0.0);
    assert_eq!(ev("min(3, 4) + max(3, 4)").unwrap(), 7.0);
    assert_eq!(ev("abs(-3)").unwrap(), 3.0);
    assert_eq!(ev("sqrt(16)").unwrap(), 4.0);
    assert_eq!(ev("floor(-2.5)").unwrap(), -3.0);
    assert_eq!(ev("ceil(-2.5)").unwrap(), -2.0);
    assert_eq!(ev("round(2.5)").unwrap(), 3.0);
    assert_eq!(ev("round(-2.5)").unwrap(), -3.0);
    assert_eq!(ev("clamp(5, 0, 3)").unwrap(), 3.0);
    assert_eq!(ev("if(1 > 0, 7, 8)").unwrap(), 7.0);
}

#[test]
fn bad_formulas_are_errors_not_crashes() {
    assert!(matches!(ev("1 / 0"), Err(ExprError::DivByZero)));
    assert!(matches!(ev("sqrt(-1)"), Err(ExprError::Domain(_))));
    assert!(matches!(ev("clamp(1, 3, 0)"), Err(ExprError::Domain(_))));
    assert!(matches!(ev("1e308 * 10"), Err(ExprError::NotFinite)));
    assert!(matches!(ev("1e999"), Err(ExprError::NotFinite)));
    assert!(matches!(ev("1 +"), Err(ExprError::Syntax(_))));
    assert!(matches!(ev("(1"), Err(ExprError::Syntax(_))));
    assert!(matches!(ev("1 2"), Err(ExprError::Syntax(_))));
    assert!(matches!(ev("1e"), Err(ExprError::Syntax(_))));
    assert!(matches!(ev("1.2.3"), Err(ExprError::Syntax(_))));
    assert!(matches!(ev("$"), Err(ExprError::Syntax(_))));
    assert!(matches!(ev("nope(1)"), Err(ExprError::UnknownFunction(_))));
    assert!(matches!(ev("min(1)"), Err(ExprError::Arity { .. })));
    assert!(matches!(ev("x + 1"), Err(ExprError::UnknownName(_))));
    assert!(matches!(ev(""), Err(ExprError::Syntax(_))));
}

#[test]
fn logic_short_circuits_so_guards_work() {
    assert_eq!(ev("0 && 1 / 0").unwrap(), 0.0);
    assert_eq!(ev("1 || 1 / 0").unwrap(), 1.0);
    assert_eq!(ev("if(1, 5, 1 / 0)").unwrap(), 5.0);
    assert_eq!(ev("if(0, 1 / 0, 6)").unwrap(), 6.0);
    assert!(matches!(ev("1 && 1 / 0"), Err(ExprError::DivByZero)));
}

#[test]
fn hostile_formula_sizes_are_refused() {
    assert!(matches!(ev(&"1+".repeat(MAX_LEN) ), Err(ExprError::TooLong)));
    let deep = format!("{}1{}", "(".repeat(MAX_DEPTH + 5), ")".repeat(MAX_DEPTH + 5));
    assert!(matches!(ev(&deep), Err(ExprError::TooDeep)));
    let negs = format!("{}1", "-".repeat(MAX_DEPTH + 5));
    assert!(matches!(ev(&negs), Err(ExprError::TooDeep)));
    let many = (0..400).map(|_| "1").collect::<Vec<_>>().join("+");
    assert!(many.len() < MAX_LEN);
    assert!(matches!(ev(&many), Err(ExprError::TooManyNodes)));
    let ok = (0..100).map(|_| "1").collect::<Vec<_>>().join("+");
    assert_eq!(ev(&ok).unwrap(), 100.0);
}

proptest! {
    #[test]
    fn random_formula_text_never_panics(s in "[0-9a-z_ ()+*/<>=!&|,.\\-]{0,200}") {
        let _ = ev(&s);
        let f = Family { name: "g".into(), values: vec![ValueDef::input("a", 1.0, 0.0, 2.0, Scope::Type), ValueDef::formula("b", &s)], ..Default::default() };
        if let Ok(c) = cd(&f) { let _ = c.solve(&Request::default()); }
    }
}

// ------------------------------------------------------------------ door and window
#[test]
fn door_default_matches_hand_calculation() {
    let c = cdoor();
    let i = c.solve(&Request::default()).unwrap();
    assert_eq!(i.solids.len(), 8, "four frame parts and the four parts of the handle");
    let f = frame_parts(&c, &i);
    assert_eq!(f.solids.len(), 4);
    assert_eq!(i.opening, Some([0.9, 2.1, 0.0]));
    let (lo, hi) = bbox(&f);
    assert!(close(lo[0], -0.45) && close(hi[0], 0.45) && close(lo[1], -0.1) && close(hi[1], 0.1) && close(lo[2], 0.0) && close(hi[2], 2.1));
    // jambs 2 x (0.05 x 0.2 x 2.1) + head (0.8 x 0.2 x 0.05) + leaf (0.8 x 0.04 x 2.05 - wait: leaf height = 2.1 - 0.05)
    let expected = 2.0 * 0.05 * 0.2 * 2.1 + 0.8 * 0.2 * 0.05 + 0.8 * 0.04 * (2.1 - 0.05);
    assert!(close(total_volume(&f), expected), "{} vs {expected}", total_volume(&f));
    assert!(no_overlaps(&i));
}

#[test]
fn types_and_instance_overrides_layer_correctly() {
    let c = cdoor();
    let a = c.solve(&Request { type_name: Some("Type A"), overrides: vec![], ..Default::default() }).unwrap();
    assert_eq!(a.opening.unwrap()[0], 0.8);
    let b = c.solve(&Request { type_name: Some("Type B"), overrides: vec![("wall_t", 0.3)], ..Default::default() }).unwrap();
    assert_eq!(b.opening.unwrap()[..2], [1.2, 2.4]);
    assert!(close(c.value(&b, "y_back").unwrap(), 0.15));
    assert!(matches!(c.solve(&Request { type_name: Some("Nope"), overrides: vec![], ..Default::default() }), Err(FamilyError::UnknownType(_))));
    assert!(matches!(c.solve(&req(vec![("width", 1.0)])), Err(FamilyError::WrongScope(_))), "type values cannot change on one instance");
    assert!(matches!(c.solve(&req(vec![("x_left", 1.0)])), Err(FamilyError::NotAnInput(_))));
    assert!(matches!(c.solve(&req(vec![("zzz", 1.0)])), Err(FamilyError::UnknownValue(_))));
    assert!(matches!(c.solve(&req(vec![("wall_t", 5.0)])), Err(FamilyError::OutOfRange { .. })));
    assert!(matches!(c.solve(&req(vec![("wall_t", f64::NAN)])), Err(FamilyError::NotFinite(_))));
    assert!(matches!(c.solve(&req(vec![("wall_t", f64::INFINITY)])), Err(FamilyError::NotFinite(_))));
    // a type that breaks a rule is refused at solve time with the rule's name
    let mut f = door();
    f.types.push(TypeDef { name: "Bad".into(), values: vec![("frame".into(), 0.2), ("width".into(), 0.6)], ..Default::default() });
    let c2 = cd(&f).unwrap();
    assert_eq!(c2.solve(&Request { type_name: Some("Bad"), overrides: vec![], ..Default::default() }), Err(FamilyError::CheckFailed("opening wider than four frames".into())));
}

#[test]
fn window_mullions_are_evenly_spaced_and_counted() {
    let c = window().compile().unwrap();
    let i = c.solve(&Request { type_name: Some("Type A"), overrides: vec![("mullions", 3.0)], ..Default::default() }).unwrap();
    assert_eq!(i.solids.iter().filter(|(n, _)| c.solid_name(*n) == "mullion_front").count(), 3);
    assert_eq!(i.solids.iter().filter(|(n, _)| c.solid_name(*n) == "mullion_back").count(), 3);
    let mut centres: Vec<f64> = i.solids.iter().filter(|(n, _)| c.solid_name(*n) == "mullion_front").map(|(_, b)| (b.min[0] + b.max[0]) / 2.0).collect();
    centres.sort_by(|a, b| a.total_cmp(b));
    let (inner_l, cell) = (-0.9 + 0.05, (1.8 - 0.1) / 4.0);
    for (k, c) in centres.iter().enumerate() { assert!(close(*c, inner_l + cell * (k as f64 + 1.0)), "{centres:?}"); }
    assert!(no_overlaps(&i));
    let none = c.solve(&req(vec![("mullions", 0.0)])).unwrap();
    assert_eq!(none.solids.len(), 5);
    assert!(matches!(c.solve(&req(vec![("mullions", 2.5)])), Err(FamilyError::NotWhole { .. })));
    assert!(matches!(c.solve(&req(vec![("mullions", 21.0)])), Err(FamilyError::OutOfRange { .. })));
    let mut f = window();
    f.types.push(TypeDef { name: "Tiny".into(), values: vec![("width".into(), 0.3), ("height".into(), 0.5)], ..Default::default() });
    let c = cd(&f).unwrap();
    assert!(matches!(c.solve(&Request { type_name: Some("Tiny"), overrides: vec![("mullions", 20.0)], ..Default::default() }), Err(FamilyError::ArrayTooDense { .. }) | Err(FamilyError::CheckFailed(_))));
}

proptest! {
    #[test]
    fn door_resizes_without_breaking_constraints(
        w in 0.5f64..3.0, h in 1.5f64..3.5, fr in 0.02f64..0.2, lt in 0.02f64..0.1, wt in 0.05f64..1.0) {
        let mut f = door();
        f.types.push(TypeDef { name: "t".into(), values: vec![("width".into(), w), ("height".into(), h), ("frame".into(), fr), ("leaf_t".into(), lt)], ..Default::default() });
        let c = cd(&f).unwrap();
        let r = c.solve(&Request { type_name: Some("t"), overrides: vec![("wall_t", wt)], ..Default::default() });
        let valid = w > 4.0 * fr && h > 4.0 * fr && lt < wt && w - 2.0 * fr > 0.35;
        prop_assert_eq!(r.is_ok(), valid, "{:?}", r);
        if let Ok(i) = r {
            prop_assert!(no_overlaps(&i));
            let fi = frame_parts(&c, &i);
            let (lo, hi) = bbox(&fi);
            prop_assert!(close(lo[0], -w / 2.0) && close(hi[0], w / 2.0) && close(lo[2], 0.0) && close(hi[2], h));
            prop_assert!(close(lo[1], -wt / 2.0) && close(hi[1], wt / 2.0));
            prop_assert_eq!(i.opening.unwrap(), [w, h, 0.0]);
            // the frame keeps its size whatever the opening is
            for (n, b) in &i.solids { if c.solid_name(*n) == "jamb_left" || c.solid_name(*n) == "jamb_right" { prop_assert!(close(b.max[0] - b.min[0], fr)); } }
            let leaf = i.solids.iter().find(|(n, _)| c.solid_name(*n) == "leaf").unwrap().1;
            prop_assert!(close(leaf.min[1], -leaf.max[1]) && close(leaf.max[1] - leaf.min[1], lt));
            let expected = 2.0 * fr * wt * h + (w - 2.0 * fr) * wt * fr + (w - 2.0 * fr) * lt * (h - fr);
            prop_assert!(close(total_volume(&fi), expected));
        }
    }

    #[test]
    fn window_resizes_without_breaking_constraints(
        w in 0.3f64..4.0, h in 0.3f64..3.0, fr in 0.02f64..0.2, gt in 0.004f64..0.05, mw in 0.01f64..0.1,
        sill in 0.0f64..2.0, n in 0u32..=20, wt in 0.05f64..1.0) {
        let mut f = window();
        f.types.push(TypeDef { name: "t".into(), values: vec![("width".into(), w), ("height".into(), h), ("frame".into(), fr), ("glass_t".into(), gt), ("mullion_w".into(), mw)], ..Default::default() });
        let c = cd(&f).unwrap();
        let r = c.solve(&Request { type_name: Some("t"), overrides: vec![("sill", sill), ("mullions", n as f64), ("wall_t", wt)], ..Default::default() });
        let inner = w - 2.0 * fr;
        let cell = inner / (n as f64 + 1.0);
        let narrowest = cell - if n == 0 { 0.0 } else if n == 1 { mw / 2.0 } else { mw };
        let valid = w > 4.0 * fr && h > 4.0 * fr && gt < wt && (n == 0 || mw < cell) && narrowest >= 0.01;
        prop_assert_eq!(r.is_ok(), valid, "{:?}", r);
        if let Ok(i) = r {
            prop_assert!(no_overlaps(&i));
            let (lo, hi) = bbox(&i);
            prop_assert!(close(lo[0], -w / 2.0) && close(hi[0], w / 2.0) && close(lo[2], sill) && close(hi[2], sill + h));
            prop_assert_eq!(i.opening.unwrap(), [w, h, sill]);
            let mut cs: Vec<f64> = i.solids.iter().filter(|(nm, _)| c.solid_name(*nm) == "mullion_front").map(|(_, b)| (b.min[0] + b.max[0]) / 2.0).collect();
            cs.sort_by(|a, b| a.total_cmp(b));
            prop_assert_eq!(cs.len(), n as usize);
            for pair in cs.windows(2) { prop_assert!(close(pair[1] - pair[0], cell)); }
            if let Some(first) = cs.first() { prop_assert!(close(first - (-w / 2.0 + fr), cell)); }
            let glass = (inner) * gt * (h - 2.0 * fr);
            let frame_vol = 2.0 * fr * wt * h + 2.0 * inner * wt * fr;
            let bars = 2.0 * n as f64 * mw * ((wt - gt) / 2.0) * (h - 2.0 * fr);
            prop_assert!(close(total_volume(&i), glass + frame_vol + bars));
        }
    }
}

// ------------------------------------------------------------------ structure of a family
fn two(a: &str, b: &str) -> Family {
    Family { name: "t".into(), values: vec![ValueDef::formula("a", a), ValueDef::formula("b", b)], ..Default::default() }
}

#[test]
fn loops_unknown_names_and_duplicates_are_refused_when_compiling() {
    assert!(matches!(two("b + 1", "a + 1").compile(), Err(FamilyError::Cycle(c)) if c == vec!["a".to_string(), "b".to_string()]));
    assert!(matches!(two("a + 1", "1").compile(), Err(FamilyError::Cycle(_))), "a value cannot depend on itself");
    assert!(matches!(two("zzz", "1").compile(), Err(FamilyError::Formula { err: ExprError::UnknownName(_), .. })));
    let mut f = two("1", "2"); f.values[1].name = "a".into();
    assert!(matches!(cd(&f), Err(FamilyError::DuplicateName(_))));
    let mut f = two("1", "2"); f.values[1].name = "9bad".into();
    assert!(matches!(cd(&f), Err(FamilyError::BadName(_))));
    // chain a <- b <- c in reverse listing order is fine
    let f = Family { name: "t".into(), values: vec![ValueDef::formula("c", "b * 2"), ValueDef::formula("b", "a + 1 m"), ValueDef::input("a", 1.0, 0.0, 5.0, Scope::Type)], ..Default::default() };
    let c = cd(&f).unwrap();
    assert_eq!(c.value(&c.solve(&Request::default()).unwrap(), "c"), Some(4.0));
}

#[test]
fn inputs_and_solids_are_validated() {
    let bad = |v: ValueDef| Family { name: "t".into(), values: vec![v], ..Default::default() };
    assert!(matches!(bad(ValueDef::input("a", 5.0, 0.0, 1.0, Scope::Type)).compile(), Err(FamilyError::BadInput { .. })));
    assert!(matches!(bad(ValueDef::input("a", 0.5, 1.0, 0.0, Scope::Type)).compile(), Err(FamilyError::BadInput { .. })));
    assert!(matches!(bad(ValueDef::input("a", f64::NAN, 0.0, 1.0, Scope::Type)).compile(), Err(FamilyError::BadInput { .. })));
    assert!(matches!(bad(ValueDef::count("a", 0.5, 0.0, 1.0, Scope::Type)).compile(), Err(FamilyError::BadInput { .. })));
    let mut f = door();
    f.solids[0].x = ("y_front".into(), "x_in_left".into());
    assert!(matches!(cd(&f), Err(FamilyError::WrongAxis { .. })));
    let mut f = door();
    f.solids[0].z.1 = "nowhere".into();
    assert!(matches!(cd(&f), Err(FamilyError::UnknownPlane { .. })));
    let mut f = door();
    f.types.push(TypeDef { name: "x".into(), values: vec![("x_left".into(), 1.0)], ..Default::default() });
    assert!(matches!(cd(&f), Err(FamilyError::NotAnInput(_))));
    let mut f = door();
    f.types.push(TypeDef { name: "x".into(), values: vec![("nope".into(), 1.0)], ..Default::default() });
    assert!(matches!(cd(&f), Err(FamilyError::UnknownValue(_))));
}

#[test]
fn formula_failures_at_solve_time_name_the_value() {
    let f = Family { name: "t".into(), values: vec![ValueDef::input("w", 0.0, 0.0, 5.0, Scope::Type), ValueDef::formula("r", "10 / w"), ValueDef::formula("safe", "if(w > 0, 10 / w, 0)")], ..Default::default() };
    let c = cd(&f).unwrap();
    assert!(matches!(c.solve(&Request::default()), Err(FamilyError::Formula { name, err: ExprError::DivByZero }) if name == "r"));
    let f = Family { name: "t".into(), values: vec![ValueDef::input("w", 0.0, 0.0, 5.0, Scope::Type), ValueDef::formula("safe", "if(w > 0, 10 / w, 0)")], ..Default::default() };
    let c = cd(&f).unwrap();
    assert_eq!(c.value(&c.solve(&Request::default()).unwrap(), "safe"), Some(0.0));
}

#[test]
fn a_solid_with_no_thickness_is_refused() {
    let mut f = door();
    f.values.push(ValueDef::plane("z_zero", Axis::Z, "z_floor"));
    f.solids.push(SolidDef::new("flat", ("x_left", "x_right"), ("y_front", "y_back"), ("z_floor", "z_zero")));
    let c = cd(&f).unwrap();
    assert!(matches!(c.solve(&Request::default()), Err(FamilyError::Degenerate { solid }) if solid == "flat"));
    // hidden solids are not built
    f.solids.last_mut().unwrap().visible = Some("0".into());
    assert!(cd(&f).unwrap().solve(&Request::default()).is_ok());
}

// ------------------------------------------------------------------ predictability
proptest! {
    #[test]
    fn listing_order_does_not_change_the_answer(keys in proptest::collection::vec(any::<u32>(), 20), n in 0u32..5) {
        let base = window();
        let mut idx: Vec<usize> = (0..base.values.len()).collect();
        idx.sort_by_key(|&i| keys[i % keys.len()]);
        let mut shuffled = base.clone();
        shuffled.values = idx.iter().map(|&i| base.values[i].clone()).collect();
        let (c1, c2) = (base.compile().unwrap(), shuffled.compile().unwrap());
        let r = Request { type_name: None, overrides: vec![("mullions", n as f64), ("sill", 0.5)], ..Default::default() };
        let (a, b) = (c1.solve(&r).unwrap(), c2.solve(&r).unwrap());
        prop_assert_eq!(&a.solids, &b.solids);
        prop_assert_eq!(a.opening, b.opening);
        for v in &base.values { prop_assert_eq!(c1.value(&a, &v.name), c2.value(&b, &v.name)); }
    }
}

#[test]
fn parallel_batches_match_serial_and_keep_order() {
    let c = window().compile().unwrap();
    let reqs: Vec<Request> = (0..4000).map(|k| Request { type_name: if k % 3 == 0 { Some("Type A") } else { None }, overrides: vec![("mullions", (k % 23) as f64), ("sill", (k % 7) as f64 * 0.3)], ..Default::default() }).collect();
    let serial: Vec<_> = reqs.iter().map(|r| c.solve(r)).collect();
    for threads in [1usize, 2, 8] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        let par = pool.install(|| c.solve_many(&reqs));
        assert_eq!(par, serial, "threads {threads}");
    }
    assert!(serial.iter().any(|r| r.is_err()) && serial.iter().any(|r| r.is_ok()), "batch should mix valid and refused placements");
}

#[test]
fn realistic_nesting_is_accepted() {
    // twelve levels of parentheses and a 150-term sum are ordinary engineering formulas
    let nested = format!("{}1{}", "(1 + ".repeat(12), ")".repeat(12));
    assert_eq!(ev(&nested).unwrap(), 13.0);
    let sum = (0..150).map(|_| "0.5").collect::<Vec<_>>().join(" + ");
    assert_eq!(ev(&sum).unwrap(), 75.0);
    assert_eq!(ev("min(max(1, 2), if(1 > 0, clamp(5, 0, 3), 9)) * ((2 + 3) * (4 - (1 + 1)))").unwrap(), 20.0);
}

#[test]
fn comparisons_and_clamp_are_exact_at_the_edges() {
    for (e, want) in [("2 < 2", 0.0), ("1 < 2", 1.0), ("3 < 2", 0.0), ("2 <= 2", 1.0), ("1 <= 2", 1.0), ("3 <= 2", 0.0),
                      ("2 > 2", 0.0), ("3 > 2", 1.0), ("1 > 2", 0.0), ("2 >= 2", 1.0), ("3 >= 2", 1.0), ("1 >= 2", 0.0),
                      ("2 == 2", 1.0), ("2 == 3", 0.0), ("2 != 2", 0.0), ("2 != 3", 1.0)] {
        assert_eq!(ev(e).unwrap(), want, "{e}");
    }
    for (e, want) in [("clamp(1, 0, 3)", 1.0), ("clamp(-1, 0, 3)", 0.0), ("clamp(5, 0, 3)", 3.0), ("clamp(2, 2, 2)", 2.0), ("clamp(0, 0, 3)", 0.0), ("clamp(3, 0, 3)", 3.0)] {
        assert_eq!(ev(e).unwrap(), want, "{e}");
    }
}

#[test]
fn inputs_below_their_minimum_and_bad_repeat_counts_are_refused() {
    let c = cdoor();
    assert!(matches!(c.solve(&req(vec![("wall_t", 0.04)])), Err(FamilyError::OutOfRange { .. })));
    assert!(c.solve(&req(vec![("wall_t", 0.05)])).is_ok(), "the minimum itself is allowed");
    assert!(c.solve(&req(vec![("wall_t", 1.0)])).is_ok(), "the maximum itself is allowed");
    // a repeat count that comes from a formula is checked too
    let fam = |count: &str| Family {
        name: "t".into(),
        values: vec![
            ValueDef::plane("x0", Axis::X, "0"), ValueDef::plane("x1", Axis::X, "1"),
            ValueDef::plane("y0", Axis::Y, "0"), ValueDef::plane("y1", Axis::Y, "1"),
            ValueDef::plane("z0", Axis::Z, "0"), ValueDef::plane("z1", Axis::Z, "1"),
        ],
        solids: vec![SolidDef::new("bars", ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).array(Axis::X, count, "0.01")],
        ..Default::default()
    };
    for bad in ["2.5", "-1", "1001", "0.5 + 1"] {
        assert!(matches!(fam(bad).compile().unwrap().solve(&Request::default()), Err(FamilyError::ArrayCount { .. })), "{bad}");
    }
    assert_eq!(fam("3").compile().unwrap().solve(&Request::default()).unwrap().solids.len(), 3);
    assert_eq!(fam("0").compile().unwrap().solve(&Request::default()).unwrap().solids.len(), 0);
}

#[test]
fn hostile_families_are_bounded() {
    // 10,000 chained values: compiled and solved in dependency order without recursion
    let mut values = vec![ValueDef::input("v0", 1.0, 0.0, 10.0, Scope::Type)];
    for k in 1..10_000 { values.push(ValueDef::formula(&format!("v{k}"), &format!("v{} + 1 m", k - 1))); }
    let t = std::time::Instant::now();
    let c = Family { name: "chain".into(), values, ..Default::default() }.compile().unwrap();
    let i = c.solve(&Request::default()).unwrap();
    assert_eq!(c.value(&i, "v9999"), Some(10_000.0));
    assert!(t.elapsed().as_secs_f64() < 5.0, "{:?}", t.elapsed());
    // one more value than allowed
    let too_many: Vec<ValueDef> = (0..=10_000).map(|k| ValueDef::formula(&format!("v{k}"), "1")).collect();
    assert!(matches!(Family { name: "x".into(), values: too_many, ..Default::default() }.compile(), Err(FamilyError::TooMany(_))));
    // many repeated solids would make a huge model: refused
    let mut f = Family {
        name: "t".into(),
        values: vec![ValueDef::plane("x0", Axis::X, "0"), ValueDef::plane("x1", Axis::X, "1"), ValueDef::plane("y0", Axis::Y, "0"), ValueDef::plane("y1", Axis::Y, "1"), ValueDef::plane("z0", Axis::Z, "0"), ValueDef::plane("z1", Axis::Z, "1")],
        ..Default::default()
    };
    for k in 0..30 { f.solids.push(SolidDef::new(&format!("s{k}"), ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).array(Axis::X, "1000", "0.0001")); }
    assert!(matches!(cd(&f).unwrap().solve(&Request::default()), Err(FamilyError::TooMany(_))));
    f.solids.truncate(10);
    assert_eq!(cd(&f).unwrap().solve(&Request::default()).unwrap().solids.len(), 10_000);
}
