use openbim_family::expr::Expr;

fn ev(src: &str) -> Result<f64, String> { Expr::parse(src, &|_| None).map_err(|e| e.to_string()).and_then(|e| e.eval(&[]).map_err(|e| e.to_string())) }
fn close(a: f64, b: f64) -> bool { (a - b).abs() < 1e-12 }

#[test]
fn angles_are_in_degrees_with_exact_values_at_quarter_turns() {
    for (src, want) in [("sin(0)", 0.0), ("sin(90)", 1.0), ("sin(180)", 0.0), ("sin(270)", -1.0), ("sin(360)", 0.0), ("sin(-90)", -1.0), ("sin(450)", 1.0),
                        ("cos(0)", 1.0), ("cos(90)", 0.0), ("cos(180)", -1.0), ("cos(270)", 0.0), ("cos(-180)", -1.0), ("cos(720)", 1.0),
                        ("tan(0)", 0.0), ("tan(180)", 0.0), ("tan(45)", 1.0), ("tan(-45)", -1.0), ("tan(135)", -1.0)] {
        let got = ev(src).unwrap();
        assert!(close(got, want), "{src} = {got}, want {want}");
    }
    // exact, not just close: a test for equality with zero must work
    assert_eq!(ev("cos(90)").unwrap(), 0.0);
    assert_eq!(ev("sin(180)").unwrap(), 0.0);
    assert_eq!(ev("if(cos(90) == 0, 1, 2)").unwrap(), 1.0);
    // values from tables
    assert!(close(ev("sin(30)").unwrap(), 0.5) && close(ev("cos(60)").unwrap(), 0.5) && close(ev("cos(45)").unwrap(), std::f64::consts::FRAC_1_SQRT_2));
    assert!(close(ev("sin(60)").unwrap(), 3f64.sqrt() / 2.0) && close(ev("tan(60)").unwrap(), 3f64.sqrt()));
    // a very large angle keeps its precision because it is reduced first: 1e9 degrees = 1e9 mod 360 = 280 degrees
    assert!(close(ev("sin(1000000000)").unwrap(), (280f64).to_radians().sin()));
    assert!(close(ev("cos(1000000000)").unwrap(), (280f64).to_radians().cos()));
}

#[test]
fn inverse_functions_return_degrees_in_the_right_quadrant() {
    for (src, want) in [("asin(1)", 90.0), ("asin(-1)", -90.0), ("asin(0.5)", 30.0), ("acos(1)", 0.0), ("acos(-1)", 180.0), ("acos(0.5)", 60.0), ("atan(1)", 45.0), ("atan(-1)", -45.0),
                        ("atan2(1, 1)", 45.0), ("atan2(1, -1)", 135.0), ("atan2(-1, -1)", -135.0), ("atan2(-1, 1)", -45.0), ("atan2(1, 0)", 90.0), ("atan2(-1, 0)", -90.0), ("atan2(0, -1)", 180.0), ("atan2(0, 1)", 0.0)] {
        let got = ev(src).unwrap();
        assert!(close(got, want), "{src} = {got}, want {want}");
    }
    // an arch: the angle at the springing of a segmental arch with span 1.2 and rise 0.3 is 2 atan(rise / (span / 2)) ...
    // radius = (half^2 + rise^2) / (2 rise) = (0.36 + 0.09) / 0.6 = 0.75; half angle = asin(0.6 / 0.75) = 53.13 degrees
    assert!(close(ev("(0.6 * 0.6 + 0.3 * 0.3) / (2 * 0.3)").unwrap(), 0.75));
    assert!(close(ev("asin(0.6 / 0.75)").unwrap(), 53.13010235415598));
    assert!(close(ev("pi()").unwrap(), std::f64::consts::PI) && close(ev("2 * pi() * 0.75").unwrap(), 1.5 * std::f64::consts::PI));
}

#[test]
fn domains_and_arity_are_refused() {
    for bad in ["tan(90)", "tan(270)", "tan(-90)", "tan(450)", "asin(1.0000001)", "asin(-2)", "acos(1.1)", "acos(-1.5)", "atan2(0, 0)"] { assert!(ev(bad).is_err(), "{bad}"); }
    for bad in ["sin()", "sin(1, 2)", "atan2(1)", "pi(1)", "cos(1,2,3)", "asin()"] { assert!(ev(bad).is_err(), "{bad}"); }
    assert!(ev("tan(89.9999)").is_ok());
    assert!(ev("asin(1)").is_ok() && ev("asin(-1)").is_ok() && ev("acos(1)").is_ok() && ev("acos(-1)").is_ok());
    assert!(ev("sin(1 / 0)").is_err());
}

#[test]
fn tangent_refuses_within_a_hair_of_a_pole_and_cosine_survives_huge_angles() {
    // cos(90.00000000005 degrees) is about -8.7e-13: closer to the pole than 1e-12, so refused; a little further away is fine
    assert!(ev("tan(90.00000000005)").is_err());
    assert!(ev("tan(89.99999999995)").is_err());
    let t = ev("tan(90.0000001)").unwrap();
    assert!(t < -1e8 && t > -1e9, "{t}");
    // 1e17 degrees: adding 90 before reducing would round to a multiple of 16; reducing first keeps the right answer
    let r = 1e17f64.rem_euclid(360.0);
    assert!(close(ev("cos(100000000000000000)").unwrap(), r.to_radians().cos()), "{r}");
    assert!(close(ev("sin(100000000000000000)").unwrap(), r.to_radians().sin()));
}
