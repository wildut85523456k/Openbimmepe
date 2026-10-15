use openbim_family::expr::*;
use openbim_family::*;

fn ev(src: &str) -> Result<f64, ExprError> { Expr::parse(src, &|_| None)?.eval(&[]) }
fn ev_with(src: &str, names: &[&str], vals: &[f64]) -> Result<f64, ExprError> {
    Expr::parse(src, &|n| names.iter().position(|x| *x == n))?.eval(vals)
}
fn close(a: f64, b: f64) -> bool { (a - b).abs() < 1e-12 }

#[test]
fn units_turn_into_metres_and_degrees() {
    for (s, v) in [("1000 mm", 1.0), ("1000mm", 1.0), ("2.5 cm", 0.025), ("3 m", 3.0), ("2 km", 2000.0), ("12 in", 0.3048), ("1 ft", 0.3048), ("90 deg", 90.0), ("1 rad", 180.0 / std::f64::consts::PI), ("1e3 mm", 1.0), ("0.5m", 0.5)] {
        assert!(close(ev(s).unwrap(), v), "{s} = {:?}", ev(s));
    }
    // units bind to their number
    assert!(close(ev("1000 mm / 2").unwrap(), 0.5));
    assert!(close(ev("2 * 300 mm + 1 m").unwrap(), 1.6));
    assert!(close(ev("-500 mm").unwrap(), -0.5));
    assert!(close(ev("(10 cm)").unwrap(), 0.1));
    assert!(close(ev("max(100 mm, 0.2)").unwrap(), 0.2));
    // not a unit: still an error, with the old message
    assert!(matches!(ev("5 apples"), Err(ExprError::Syntax(_))));
    assert!(matches!(ev("5 mmm"), Err(ExprError::Syntax(_))));
    assert!(matches!(ev("5 mm mm"), Err(ExprError::Syntax(_))));
    // a name that looks like a unit is still a name when it is not straight after a number
    assert!(close(ev_with("2 * m", &["m"], &[7.0]).unwrap(), 14.0));
    assert!(close(ev_with("m + 1 mm", &["m"], &[7.0]).unwrap(), 7.001));
    assert!(matches!(ev("1e999 mm"), Err(ExprError::NotFinite)));
}

#[test]
fn names_may_have_spaces_and_accents() {
    let names = ["Rough Width", "Largeur_allège", "Width", "Épaisseur", "_x", "a.b c", "Hauteur de linteau"];
    let vals = [2.0, 3.0, 5.0, 7.0, 11.0, 13.0, 17.0];
    let e = |s: &str| ev_with(s, &names, &vals);
    assert_eq!(e("Rough Width * 2").unwrap(), 4.0);
    assert_eq!(e("Largeur_allège + Épaisseur").unwrap(), 10.0);
    // the longest known name wins
    assert_eq!(e("Width + Rough Width").unwrap(), 7.0);
    assert_eq!(e("Rough Width+Width").unwrap(), 7.0);
    assert_eq!(e("Hauteur de linteau - _x").unwrap(), 6.0);
    assert_eq!(e("a.b c").unwrap(), 13.0);
    assert_eq!(e("min(Rough Width, Width)").unwrap(), 2.0);
    assert_eq!(e("if(Rough Width > 1, Épaisseur, 0)").unwrap(), 7.0);
    // two names side by side are still a mistake
    assert!(matches!(e("Width Épaisseur"), Err(ExprError::Syntax(_))));
    assert!(matches!(e("Rough"), Err(ExprError::UnknownName(n)) if n == "Rough"));
    assert!(matches!(e("Rough Wid"), Err(ExprError::UnknownName(n)) if n == "Rough"));
    assert!(matches!(e("Rough  Width"), Err(ExprError::UnknownName(_)) | Err(ExprError::Syntax(_))));
    // a name starting with a digit is not a name
    assert!(e("2x").is_err());
}

#[test]
fn name_rules() {
    let ok = |n: &str| Family { name: "F".into(), values: vec![ValueDef::input(n, 1.0, 0.0, 2.0, Scope::Instance)], ..Default::default() }.compile().is_ok();
    for n in ["Rough Width", "Largeur_allège", "Épaisseur", "_x", "a", "A B C", "größe", "幅 1", "x2", "Width 2"] { assert!(ok(n), "{n}"); }
    for n in ["", " x", "x ", "a  b", "2x", "x-y", "x\ty", "a.b", "x(", "2 x", "x\u{0}"] { assert!(!ok(n), "{n:?}"); }
    assert!(ok(&"x".repeat(64)));
    assert!(!ok(&"x".repeat(65)));
    assert!(ok(&"é".repeat(64)), "64 characters even when each takes two bytes");
    assert!(!ok(&"é".repeat(65)));
    // a family that uses such names in formulas end to end
    let f = Family {
        name: "Fenêtre".into(),
        values: vec![
            ValueDef::input("Rough Width", 1.2, 0.1, 5.0, Scope::Instance),
            ValueDef::input("Largeur_allège", 0.1, 0.0, 1.0, Scope::Instance),
            ValueDef::formula("Clear Width", "Rough Width - 2 * Largeur_allège"),
            ValueDef::formula("Clear Width mm", "Clear Width / 1 mm"),
        ],
        ..Default::default()
    };
    let c = f.compile().unwrap();
    let i = c.solve(&Request::default()).unwrap();
    assert!(close(c.value(&i, "Clear Width").unwrap(), 1.0));
    assert!(close(c.value(&i, "Clear Width mm").unwrap(), 1000.0));
    let i = c.solve(&Request { overrides: vec![("Rough Width", 2.0)], ..Default::default() }).unwrap();
    assert!(close(c.value(&i, "Clear Width").unwrap(), 1.8));
}

#[test]
fn equality_and_the_revit_style_logic_functions() {
    assert_eq!(ev("3 = 3").unwrap(), 1.0);
    assert_eq!(ev("3 = 4").unwrap(), 0.0);
    assert_eq!(ev("3 == 3").unwrap(), 1.0);
    assert_eq!(ev("3 <> 4").unwrap(), 1.0);
    assert_eq!(ev("3 <> 3").unwrap(), 0.0);
    assert_eq!(ev("3 != 4").unwrap(), 1.0);
    assert_eq!(ev("3 <= 3").unwrap(), 1.0);
    assert_eq!(ev("3 < 3").unwrap(), 0.0);
    assert_eq!(ev("if(2 + 2 = 4, 10, 20)").unwrap(), 10.0);
    // and / or / not
    assert_eq!(ev("and(1, 1)").unwrap(), 1.0);
    assert_eq!(ev("and(1, 0)").unwrap(), 0.0);
    assert_eq!(ev("and(1, 1, 1, 0)").unwrap(), 0.0);
    assert_eq!(ev("and(1, 2, 3)").unwrap(), 1.0);
    assert_eq!(ev("or(0, 0)").unwrap(), 0.0);
    assert_eq!(ev("or(0, 0, 5)").unwrap(), 1.0);
    assert_eq!(ev("or(0, 1)").unwrap(), 1.0);
    assert_eq!(ev("not(0)").unwrap(), 1.0);
    assert_eq!(ev("not(3)").unwrap(), 0.0);
    assert_eq!(ev("not(and(1, 0))").unwrap(), 1.0);
    assert_eq!(ev("and(or(0, 1), not(0))").unwrap(), 1.0);
    assert_eq!(ev("if(and(2 > 1, 3 > 2), 7, 8)").unwrap(), 7.0);
    // the old spellings still work
    assert_eq!(ev("1 && 0 || 1").unwrap(), 1.0);
    assert_eq!(ev("!0").unwrap(), 1.0);
    // short circuit: the second condition is not evaluated when the first decides
    assert_eq!(ev_with("and(w > 0, 1 / w < 5)", &["w"], &[0.0]).unwrap(), 0.0);
    assert_eq!(ev_with("or(w = 0, 1 / w < 5)", &["w"], &[0.0]).unwrap(), 1.0);
    assert!(matches!(ev_with("and(1, 1 / w < 5)", &["w"], &[0.0]), Err(ExprError::DivByZero)));
    // arity
    assert!(matches!(ev("and(1)"), Err(ExprError::Arity { expected: 2, got: 1, .. })));
    assert!(matches!(ev("or()"), Err(ExprError::Arity { expected: 2, got: 0, .. })));
    assert!(matches!(ev("not()"), Err(ExprError::Arity { expected: 1, got: 0, .. })));
    assert!(matches!(ev("not(1, 2)"), Err(ExprError::Arity { expected: 1, got: 2, .. })));
    let sixteen = format!("and({})", vec!["1"; 16].join(", "));
    let seventeen = format!("and({})", vec!["1"; 17].join(", "));
    assert_eq!(ev(&sixteen).unwrap(), 1.0);
    assert!(ev(&seventeen).is_err());
    let sixteen = format!("or({})", vec!["0"; 16].join(", "));
    let seventeen = format!("or({})", vec!["0"; 17].join(", "));
    assert_eq!(ev(&sixteen).unwrap(), 0.0);
    assert!(ev(&seventeen).is_err());
    // a stray = is still an error where no comparison fits
    assert!(ev("= 3").is_err());
    assert!(ev("3 = ").is_err());
    assert!(ev("3 < > 4").is_err());
    // a name called and: the function and the value do not clash
    assert_eq!(ev_with("and + and(1, 1)", &["and"], &[4.0]).unwrap(), 5.0);
}

#[test]
fn longest_name_wins_and_long_names_work() {
    let names = ["Rough", "Rough Width", "x"];
    let vals = [1.0, 10.0, 100.0];
    assert_eq!(ev_with("Rough Width", &names, &vals).unwrap(), 10.0);
    assert_eq!(ev_with("Rough + Rough Width", &names, &vals).unwrap(), 11.0);
    assert_eq!(ev_with("Rough * 2", &names, &vals).unwrap(), 2.0);
    // a dot only joins when a letter follows
    assert!(matches!(ev_with("x.5", &names, &vals), Err(ExprError::Syntax(_))));
    assert!(matches!(ev_with("x. 5", &names, &vals), Err(ExprError::Syntax(_))));
    assert_eq!(ev_with("x .5", &names, &vals).map_err(|_| ()), Err(()));
    // the longest allowed name: 32 one-letter words
    let long = vec!["a"; 32].join(" ");
    assert_eq!(long.len(), 63);
    assert_eq!(ev_with(&long, &[long.as_str()], &[5.0]).unwrap(), 5.0);
    let long31 = vec!["a"; 31].join(" ");
    assert_eq!(ev_with(&format!("{long31} + 1"), &[long31.as_str()], &[5.0]).unwrap(), 6.0);
}
