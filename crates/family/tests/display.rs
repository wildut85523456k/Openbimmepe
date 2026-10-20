use openbim_family::display::*;
use openbim_family::*;

#[test]
fn values_are_shown_in_the_unit_chosen_for_their_kind() {
    let u = DisplayUnits::metric();
    assert_eq!(u.format(1.2, LENGTH, 3), "1200 mm");
    assert_eq!(u.format(0.0254, LENGTH, 3), "25.4 mm");
    assert_eq!(u.format(12.5, AREA, 2), "12.5 m2");
    assert_eq!(u.format(0.5, ANGLE, 1), "0.5 deg");
    assert_eq!(u.format(0.0125, FLOW, 2), "12.5 L/s");
    assert_eq!(u.format(1500.0, POWER, 2), "1.5 kW");
    // a kind with no choice uses the first unit of the table that measures it; a kind without a unit is a plain number
    assert_eq!(u.format(7200.0, TIME, 1), "7200 s");
    assert_eq!(u.format(2.5, NUMBER, 2), "2.5");
    assert_eq!(DisplayUnits::default().format(2.0, LENGTH, 2), "2000 mm");
}

#[test]
fn choosing_a_unit_changes_what_is_shown_and_what_a_bare_number_means() {
    let mut u = DisplayUnits::metric();
    u.set(LENGTH, "ft").unwrap();
    assert_eq!(u.format(0.3048, LENGTH, 3), "1 ft");
    assert!(
        (u.parse("10", LENGTH).unwrap() - 3.048).abs() < 1e-12,
        "a bare number is in the unit shown"
    );
    u.set(LENGTH, "m").unwrap();
    assert!((u.parse("10", LENGTH).unwrap() - 10.0).abs() < 1e-12);
    u.set(FLOW, "m3/h").unwrap();
    assert_eq!(u.format(1.0 / 3600.0 * 450.0, FLOW, 2), "450 m3/h");
    assert_eq!(u.symbol(LENGTH), Some("m"));
}

#[test]
fn typed_values_take_their_own_unit_when_they_have_one() {
    let u = DisplayUnits::metric();
    assert!((u.parse("50", LENGTH).unwrap() - 0.05).abs() < 1e-12);
    assert!((u.parse("50 mm", LENGTH).unwrap() - 0.05).abs() < 1e-12);
    assert!((u.parse("2.5m", LENGTH).unwrap() - 2.5).abs() < 1e-12);
    assert!((u.parse("  1e3 mm ", LENGTH).unwrap() - 1.0).abs() < 1e-12);
    assert!((u.parse("-2e-1 m", LENGTH).unwrap() + 0.2).abs() < 1e-12);
    assert!((u.parse("30 deg", ANGLE).unwrap() - 30.0).abs() < 1e-12);
    assert!((u.parse("1 rad", ANGLE).unwrap() - 180.0 / std::f64::consts::PI).abs() < 1e-9);
    assert!((u.parse("36 m3/h", FLOW).unwrap() - 0.01).abs() < 1e-12);
    assert!((u.parse("2 kPa", PRESSURE).unwrap() - 2000.0).abs() < 1e-9);
    assert!((u.parse("0.8 W/m2K", U_VALUE).unwrap() - 0.8).abs() < 1e-12);
}

#[test]
fn what_cannot_be_read_is_refused_with_a_reason() {
    let u = DisplayUnits::metric();
    assert!(matches!(
        u.parse("5 furlongs", LENGTH),
        Err(UnitError::Unknown(_))
    ));
    assert!(matches!(
        u.parse("5 kg", LENGTH),
        Err(UnitError::WrongKind { .. })
    ));
    assert!(matches!(
        u.parse("abc", LENGTH),
        Err(UnitError::NotANumber(_))
    ));
    assert!(matches!(u.parse("", LENGTH), Err(UnitError::NotANumber(_))));
    assert!(matches!(
        u.parse("inf", LENGTH),
        Err(UnitError::NotANumber(_)) | Err(UnitError::Unknown(_))
    ));
    assert!(matches!(
        u.parse("NaN mm", LENGTH),
        Err(UnitError::NotANumber(_)) | Err(UnitError::Unknown(_))
    ));
    let mut u = u;
    assert!(matches!(
        u.set(LENGTH, "kg"),
        Err(UnitError::WrongKind { .. })
    ));
    assert!(matches!(u.set(LENGTH, "nope"), Err(UnitError::Unknown(_))));
    // a refused choice changes nothing
    assert_eq!(u.symbol(LENGTH), Some("mm"));
}

#[test]
fn every_unit_of_the_table_round_trips_and_is_known_to_the_formula_language() {
    let u = DisplayUnits::default();
    for (sym, factor, dim) in UNITS {
        let text = format!("3 {sym}");
        let v = u
            .parse(&text, *dim)
            .unwrap_or_else(|e| panic!("{text}: {e}"));
        assert!((v - 3.0 * factor).abs() < 1e-9 * factor.max(1.0), "{sym}");
        // shown in that unit, it reads back the same
        let mut c = DisplayUnits::default();
        c.set(*dim, sym).unwrap();
        let back = c.parse(&c.format(v, *dim, 9), *dim).unwrap();
        assert!((back - v).abs() < 1e-8 * v.abs().max(1.0), "{sym}");
    }
}
