//! Formulas are checked for units when a family is compiled (issue 10).
use openbim_family::*;

fn fam(values: Vec<ValueDef>) -> Family {
    Family { name: "t".into(), values, ..Default::default() }
}
fn width() -> ValueDef {
    ValueDef::input("Width", 0.9, 0.1, 5.0, Scope::Type)
}
/// a family with the input Width = 0.9 m and one calculated value `r`
fn with(formula: &str) -> Result<Compiled, FamilyError> {
    fam(vec![width(), ValueDef::formula("r", formula)]).compile()
}
fn units_error(formula: &str) -> String {
    match with(formula) {
        Err(FamilyError::Formula { err: ExprError::Units(m), .. }) => m,
        other => panic!("{formula}: {:?}", other.map(|_| ())),
    }
}
fn dim_of(formula: &str) -> Dim {
    with(formula).unwrap_or_else(|e| panic!("{formula}: {e}")).value_dim("r").unwrap()
}
fn value(formula: &str) -> f64 {
    let c = with(formula).unwrap_or_else(|e| panic!("{formula}: {e}"));
    c.value(&c.solve(&Request::default()).unwrap(), "r").unwrap()
}

#[test]
fn the_four_formulas_of_the_issue_are_refused() {
    assert!(units_error("Width + 30 deg").contains("cannot add a length and an angle"));
    assert!(units_error("Width - 50").contains("bare number"));
    // Width * Width is an area, and a plane needs a length
    assert_eq!(dim_of("Width * Width"), AREA);
    let e = fam(vec![width(), ValueDef::plane("x0", Axis::X, "Width * Width")]).compile().err().unwrap();
    assert!(matches!(e, FamilyError::Formula { err: ExprError::Units(ref m), .. } if m.contains("needs a length, not an area")), "{e:?}");
    assert!(units_error("sin(Width)").contains("needs an angle"));
}

#[test]
fn the_same_ideas_written_with_units_are_accepted_and_keep_their_value() {
    assert!((value("Width - 50 mm") - 0.85).abs() < 1e-12);
    assert!((value("Width + 3 cm") - 0.93).abs() < 1e-12);
    assert!((value("Width + 1 ft") - (0.9 + 0.3048)).abs() < 1e-12);
    assert!((value("sin(30 deg)") - 0.5).abs() < 1e-12);
    assert!((value("sin(1.5707963267948966 rad)") - 1.0).abs() < 1e-12);
    assert!((value("Width * cos(60 deg)") - 0.45).abs() < 1e-12);
    assert!((value("Width * Width / Width") - 0.9).abs() < 1e-12);
}

#[test]
fn dimensions_follow_the_arithmetic() {
    assert_eq!(dim_of("Width"), LENGTH);
    assert_eq!(dim_of("Width * 2"), LENGTH);
    assert_eq!(dim_of("-Width"), LENGTH);
    assert_eq!(dim_of("Width * Width * Width"), VOLUME);
    assert_eq!(dim_of("Width / Width"), NUMBER);
    assert_eq!(dim_of("Width * Width / Width"), LENGTH);
    assert_eq!(dim_of("sqrt(Width * Width)"), LENGTH);
    assert_eq!(dim_of("1 / Width"), Dim { len: -1, ..NUMBER });
    assert_eq!(dim_of("30 deg"), ANGLE);
    assert_eq!(dim_of("atan(Width / Width)"), ANGLE);
    assert_eq!(dim_of("atan2(Width, 2 * Width)"), ANGLE);
    assert_eq!(dim_of("asin(0.5)"), ANGLE);
    assert_eq!(dim_of("Width > 1 m"), NUMBER);
    assert_eq!(dim_of("min(Width, 2 m)"), LENGTH);
    assert_eq!(dim_of("max(Width, 0)"), LENGTH); // a plain 0 fits any dimension
    assert_eq!(dim_of("clamp(Width, 0.5 m, 2 m)"), LENGTH);
    assert_eq!(dim_of("if(Width > 1 m, 2 m, Width)"), LENGTH);
    assert_eq!(dim_of("abs(Width - 2 m)"), LENGTH);
    assert_eq!(dim_of("round(Width / 1 mm)"), NUMBER);
    assert_eq!(dim_of("pi()"), NUMBER);
    assert_eq!(dim_of("2 * pi() * Width"), LENGTH);
}

#[test]
fn what_is_refused() {
    assert!(units_error("Width + Width * Width").contains("cannot add a length and an area"));
    assert!(units_error("Width < 30 deg").contains("cannot compare a length and an angle"));
    assert!(units_error("Width > 2").contains("bare number"));
    assert!(units_error("min(Width, 2)").contains("bare number"));
    assert!(units_error("if(Width > 1 m, 2, Width)").contains("bare number"));
    assert!(units_error("sqrt(Width)").contains("no whole dimension"));
    assert!(units_error("cos(Width / Width)").contains("needs an angle"));
    assert!(units_error("asin(Width)").contains("needs a number"));
    assert!(units_error("if(Width, 1 m, 2 m)").contains("yes/no or a number"));
    assert!(units_error("!Width").contains("yes/no or a number"));
    assert!(units_error("Width * Width * Width * Width * Width * Width * Width").contains("beyond what a formula may build"));
    assert!(units_error("Width && Width").contains("yes/no or a number"));
    assert!(units_error("atan2(Width, 30 deg)").contains("cannot compare"));
}

#[test]
fn a_bare_number_has_one_safe_reading_and_one_refused() {
    // alone, it takes the dimension of the place and is read in metres (or degrees)
    assert!((value("0.25") - 0.25).abs() < 1e-12); // a number, until a place asks for a length
    let c = fam(vec![ValueDef::plane("x0", Axis::X, "0.25"), ValueDef::plane("x1", Axis::X, "1")]).compile().unwrap();
    assert_eq!(c.value_dim("x0"), Some(LENGTH));
    // a plain zero fits next to anything
    assert!((value("Width + 0") - 0.9).abs() < 1e-12);
    assert!((value("Width - 0") - 0.9).abs() < 1e-12);
    // but a number next to a quantity is ambiguous and refused, whichever side it is on
    assert!(units_error("50 + Width").contains("bare number"));
    assert!(units_error("Width - 50").contains("bare number"));
    // and a number made of numbers is still only numbers
    assert!(units_error("Width + (1 + 2)").contains("bare number"));
}

#[test]
fn inputs_say_what_they_measure() {
    let f = fam(vec![
        ValueDef::input("a_length", 1.0, 0.0, 5.0, Scope::Type),
        ValueDef::count("a_count", 2.0, 0.0, 9.0, Scope::Type),
        ValueDef::angle("an_angle", 30.0, 0.0, 90.0, Scope::Type),
        ValueDef::number("a_ratio", 0.5, 0.0, 1.0, Scope::Type),
        ValueDef::input("an_area", 1.0, 0.0, 9.0, Scope::Type).with_dim(AREA),
        ValueDef::flag("a_flag", true, Scope::Type),
        ValueDef::choice("a_choice", &["x", "y"], "x", Scope::Type),
        ValueDef::formula("total", "a_length * a_count"),
        ValueDef::formula("turned", "an_angle * a_ratio"),
        ValueDef::formula("step", "a_length / a_count").with_dim(LENGTH),
    ]);
    let c = f.compile().unwrap();
    for (n, d) in [("a_length", LENGTH), ("a_count", NUMBER), ("an_angle", ANGLE), ("a_ratio", NUMBER), ("an_area", AREA), ("a_flag", NUMBER), ("a_choice", NUMBER), ("total", LENGTH), ("step", LENGTH)] {
        assert_eq!(c.value_dim(n), Some(d), "{n}");
    }
    assert_eq!(c.value_dim("turned"), Some(ANGLE));
    assert_eq!(c.value_dim("nothing"), None);
    // a dimension on a flag or a choice is a mistake, a number dimension is not
    assert!(fam(vec![ValueDef::flag("f", true, Scope::Type).with_dim(LENGTH)]).compile().is_err());
    assert!(fam(vec![ValueDef::flag("f", true, Scope::Type).with_dim(NUMBER)]).compile().is_ok());
}

#[test]
fn a_declared_dimension_must_be_what_the_formula_gives() {
    let ok = fam(vec![width(), ValueDef::formula("a", "Width * Width").with_dim(AREA)]).compile();
    assert!(ok.is_ok());
    let e = fam(vec![width(), ValueDef::formula("a", "Width * Width").with_dim(LENGTH)]).compile().err().unwrap();
    assert!(matches!(e, FamilyError::Formula { ref name, err: ExprError::Units(ref m) } if name == "a" && m.contains("needs a length, not an area")), "{e:?}");
}

#[test]
fn places_that_use_a_formula_check_its_dimension() {
    let solid = |x1: &str, vis: Option<&str>| {
        let mut f = fam(vec![width(), ValueDef::angle("turn", 10.0, 0.0, 90.0, Scope::Type), ValueDef::plane("x0", Axis::X, "0"), ValueDef::plane("x1", Axis::X, x1), ValueDef::plane("y0", Axis::Y, "0"), ValueDef::plane("y1", Axis::Y, "1"), ValueDef::plane("z0", Axis::Z, "0"), ValueDef::plane("z1", Axis::Z, "1")]);
        let mut s = SolidDef::new("b", ("x0", "x1"), ("y0", "y1"), ("z0", "z1"));
        if let Some(v) = vis {
            s = s.visible_if(v);
        }
        f.solids.push(s);
        f.compile()
    };
    assert!(solid("Width", None).is_ok());
    assert!(solid("Width * Width", None).is_err());
    assert!(solid("turn", None).is_err());
    assert!(solid("Width", Some("Width > 1 m")).is_ok());
    assert!(solid("Width", Some("Width")).is_err()); // a length is not a condition
    // an opening, a check
    let mut f = fam(vec![width()]);
    f.opening = Some(OpeningDef { width: "Width".into(), height: "2.1".into(), sill: "0".into(), rise: None });
    assert!(f.compile().is_ok());
    f.opening = Some(OpeningDef { width: "Width".into(), height: "2.1 deg".into(), sill: "0".into(), rise: None });
    assert!(f.compile().is_err());
    let mut f = fam(vec![width()]);
    f.checks.push(CheckDef { name: "wide".into(), expr: "Width > 500 mm".into() });
    assert!(f.compile().is_ok());
    f.checks.push(CheckDef { name: "bad".into(), expr: "Width".into() });
    assert!(f.compile().is_err());
}

#[test]
fn a_nested_family_gets_values_of_the_dimension_its_inputs_have() {
    let child = Family { name: "Child".into(), values: vec![ValueDef::input("size", 0.1, 0.01, 1.0, Scope::Instance), ValueDef::angle("turn", 0.0, 0.0, 90.0, Scope::Instance)], ..Default::default() };
    let parent = |size: &str, turn: &str| Family {
        name: "P".into(),
        values: vec![width()],
        nested: vec![NestedDef::new("n", "Child", ("0", "0", "0")).set("size", size).set("turn", turn)],
        ..Default::default()
    };
    assert!(compile_library(&[child.clone(), parent("Width / 2", "45 deg")]).is_ok());
    assert!(compile_library(&[child.clone(), parent("45 deg", "45 deg")]).is_err()); // an angle for a length
    assert!(compile_library(&[child.clone(), parent("Width / 2", "Width")]).is_err()); // a length for an angle
    assert!(compile_library(&[child.clone(), parent("Width / 2", "45")]).is_ok()); // alone, a number is degrees there
    // and the repeat index is a plain number, so it has to be multiplied by a unit to become a length
    let rep = |at: &str| Family { name: "R".into(), nested: vec![NestedDef::new("n", "Child", (at, "0", "0")).repeat("3")], ..Default::default() };
    assert!(compile_library(&[child.clone(), rep("_i * 1 m")]).is_ok());
    assert!(compile_library(&[child, rep("_i")]).is_err());
}

#[test]
fn the_library_still_compiles_and_says_its_units() {
    let lib = library::all().unwrap();
    let window = lib.iter().find(|c| c.name() == "Window, fixed").unwrap_or_else(|| &lib[2]);
    assert_eq!(window.value_dim("width"), Some(LENGTH));
    assert_eq!(window.value_dim("window_area"), Some(AREA));
    assert_eq!(window.value_dim("mullions"), Some(NUMBER));
    // u-values per area and per length (W/m2K and W/mK), so that the window's own u-value works out as W/m2K
    assert_eq!(window.value_dim("ug"), Some(U_VALUE));
    assert_eq!(window.value_dim("psi"), Some(Dim { len: 1, time: -3, mass: 1, temp: -1, ..NUMBER }));
    assert_eq!(window.value_dim("u_window"), Some(U_VALUE));
    let door = &lib[1];
    assert_eq!(door.value_dim("open_angle"), Some(ANGLE));
}

#[test]
fn shared_parameters_carry_their_unit_through_the_text_file() {
    let id = |n: u128| format!("{n:032x}");
    let text = format!(
        "{}\ta_len\tlength\ttype\t1\t0\t9\t\tG\td\n{}\told_style\tnumber\ttype\t1\t0\t9\t\tG\td\n{}\tan_ang\tangle\ttype\t30\t0\t90\t\tG\td\n{}\ta_area\tarea\ttype\t1\t0\t9\t\tG\td\n{}\ta_vol\tvolume\ttype\t1\t0\t9\t\tG\td\n{}\ta_ratio\tratio\ttype\t0.5\t0\t1\t\tG\td\n{}\ta_count\tcount\ttype\t2\t0\t9\t\tG\td\n",
        id(1), id(2), id(3), id(4), id(5), id(6), id(7)
    );
    let set = SharedSet::from_text(&text).unwrap();
    let f = Family { name: "t".into(), shared: ["a_len", "old_style", "an_ang", "a_area", "a_vol", "a_ratio", "a_count"].iter().map(|s| s.to_string()).collect(), values: vec![ValueDef::formula("both", "a_len * old_style / a_ratio")], ..Default::default() };
    let c = compile_library_shared(std::slice::from_ref(&f), &set).unwrap().remove(0);
    for (n, d) in [("a_len", LENGTH), ("old_style", LENGTH), ("an_ang", ANGLE), ("a_area", AREA), ("a_vol", VOLUME), ("a_ratio", NUMBER), ("a_count", NUMBER), ("both", AREA)] {
        assert_eq!(c.value_dim(n), Some(d), "{n}");
    }
    // written back, "number" is spelled length, and everything else survives
    let again = SharedSet::from_text(&set.to_text()).unwrap();
    assert!(set.to_text().contains("old_style\tlength"));
    let c2 = compile_library_shared(&[f], &again).unwrap().remove(0);
    for n in ["a_len", "old_style", "an_ang", "a_area", "a_vol", "a_ratio", "a_count", "both"] {
        assert_eq!(c.value_dim(n), c2.value_dim(n), "{n}");
    }
    assert_eq!(again.to_text(), set.to_text());
    // a dimension that has no word in the file is refused
    let mut odd = set.clone();
    odd.params[0].def = odd.params[0].def.clone().with_dim(Dim { len: -2, ..NUMBER });
    assert!(odd.check().is_err());
}

#[test]
fn time_mass_temperature_and_what_is_built_from_them_have_dimensions_and_units() {
    // each unit scales to the program's own: seconds, kilograms, kelvin, pascals, watts
    assert!((value("2 h / 1 s") - 7200.0).abs() < 1e-9);
    assert!((value("1 min / 1 s") - 60.0).abs() < 1e-9);
    assert!((value("1 t / 1 kg") - 1000.0).abs() < 1e-9);
    assert!((value("500 g / 1 kg") - 0.5).abs() < 1e-12);
    assert!((value("2 kPa / 1 Pa") - 2000.0).abs() < 1e-9);
    assert!((value("1 bar / 1 kPa") - 100.0).abs() < 1e-9);
    assert!((value("3 kW / 1 W") - 3000.0).abs() < 1e-9);
    assert!((value("2 kJ / 1 J") - 2000.0).abs() < 1e-9);
    assert!((value("36 L / 1 s / (1 m * 1 m * 1 m / 1 s)") - 0.036).abs() < 1e-12);
    // the dimensions
    assert_eq!(dim_of("2 h"), TIME);
    assert_eq!(dim_of("3 kg"), MASS);
    assert_eq!(dim_of("5 K"), TEMPERATURE);
    assert_eq!(dim_of("10 L / 1 s"), FLOW);
    assert_eq!(dim_of("1 N"), FORCE);
    assert_eq!(dim_of("1 Pa"), PRESSURE);
    assert_eq!(dim_of("1 N / (1 m * 1 m)"), PRESSURE);
    assert_eq!(dim_of("1 W * 1 s"), ENERGY);
    assert_eq!(dim_of("1 J / 1 s"), POWER);
    assert_eq!(dim_of("1 W / (1 m * 1 m) / 1 K"), U_VALUE);
    assert_eq!(dim_of("sqrt(4 kg * 1 kg)"), MASS);
}

#[test]
fn quantities_of_different_kinds_are_not_mixed() {
    assert!(units_error("1 kg + 1 s").contains("cannot add a mass and a time"));
    assert!(units_error("1 kW + 1 kJ").contains("cannot add a power and an energy"));
    assert!(units_error("2 h + 5").contains("bare number"));
    assert!(units_error("1 Pa - 1 K").contains("cannot subtract a pressure and a temperature difference"));
    // odd powers cannot be square-rooted
    assert!(units_error("sqrt(1 kg)").contains("no whole dimension"));
    // a dimension that has no name of its own is spelled with its powers
    let mixed = Dim { len: 1, time: -1, ..NUMBER };
    assert_eq!(Dim::from_code("pressure"), Some(PRESSURE));
    assert_eq!(mixed.code(), None);
}

#[test]
fn the_new_dimensions_go_through_the_shared_parameter_file() {
    let id = |n: u128| format!("{n:032x}");
    let text = format!("{}	flow_rate	flow	type	1	0	9		G	d
{}	dp	pressure	type	1	0	9		G	d
{}	u	u_value	type	1	0	9		G	d
", id(1), id(2), id(3));
    let set = SharedSet::from_text(&text).unwrap();
    assert_eq!(SharedSet::from_text(&set.to_text()).unwrap().to_text(), set.to_text());
    assert!(set.to_text().contains("\tflow\t") && set.to_text().contains("\tpressure\t") && set.to_text().contains("\tu_value\t"));
}
