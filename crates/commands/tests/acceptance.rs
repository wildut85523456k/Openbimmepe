//! AUTO-001 acceptance tests (05-backlog.md): "Ribbon uses it; unit-mismatch and
//! label-lookup tests".
#![forbid(unsafe_code)]

use openbim_commands::{builtin, by_symbol, lookup, Dimension, ParameterId, Quantity, Resolve};

/// Rule 7: every value is a quantity, an SI number plus its dimension, and a
/// dimension mismatch is an error.
#[test]
fn a_dimension_mismatch_is_an_error() {
    let length = Quantity::of(*by_symbol("m").unwrap(), 3.0);
    let area = Quantity::of(*by_symbol("m²").unwrap(), 2.0);
    let other_length = Quantity::of(*by_symbol("mm").unwrap(), 3000.0);

    // A length and an area cannot be combined, in either order.
    assert!(length.checked_add(area).is_err());
    assert!(area.checked_sub(length).is_err());
    assert!(!length.same_dimension(area));

    // The same dimension combines, across units: 3 m + 3000 mm = 6 m.
    let sum = length
        .checked_add(other_length)
        .expect("the same dimension adds");
    assert!((sum.si() - 6.0).abs() < 1e-9);
    assert_eq!(sum.dimension(), Dimension::Length);

    // A node that shows both dimensions reports them, not a silent mix.
    let mismatch = length.checked_add(area).unwrap_err();
    assert_eq!(mismatch.left, Dimension::Length);
    assert_eq!(mismatch.right, Dimension::Area);
}

/// Rule 8: a parameter has a stable ID with English and French labels; a label is
/// accepted with a warning.
#[test]
fn a_label_looks_up_the_stable_id() {
    let registry = builtin();

    // The stable ID, exactly.
    let exact = registry.resolve("duct.width").expect("the ID is known");
    assert!(matches!(exact, Resolve::Id(p) if p.id == ParameterId::new("duct.width")));

    // An English label, accepted with a warning.
    let en = registry
        .resolve("Width")
        .expect("the English label is accepted");
    match en {
        Resolve::ByLabel { parameter, label } => {
            assert_eq!(parameter.id.as_str(), "element.width");
            assert_eq!(label, "Width");
        }
        Resolve::Id(_) => panic!("a label must be reported as a label, not as an ID"),
    }

    // A French label, accepted with a warning.
    let fr = registry
        .resolve("Largeur")
        .expect("the French label is accepted");
    match fr {
        Resolve::ByLabel { parameter, .. } => assert_eq!(parameter.id.as_str(), "element.width"),
        Resolve::Id(_) => panic!("a French label must be reported as a label"),
    }

    // A renamed label keeps working: the ID is what is stored, the label is only a
    // spelling. Both spellings name the same stable parameter.
    assert_eq!(
        registry.resolve("Width").unwrap().parameter().id,
        ParameterId::new("element.width")
    );
    assert_eq!(
        registry.resolve("Largeur").unwrap().parameter().id,
        ParameterId::new("element.width")
    );

    // An unknown name is refused.
    assert!(registry.resolve("Largeur du conduit").is_err());
}

/// Rule 9: the ribbon, keymap, graph nodes and MCP tools are generated from the
/// command registry, so the commands the ribbon shows are registered here.
#[test]
fn the_ribbon_commands_are_registered() {
    for id in [
        "automation_editor",
        "automation_player",
        "automation_mcp",
        "automation_snapshot",
    ] {
        let def =
            lookup(id).unwrap_or_else(|| panic!("the ribbon shows {id}, so it must be registered"));
        assert!(
            !def.en.is_empty() && !def.fr.is_empty(),
            "{id} needs an English and a French label"
        );
        assert!(!def.icon.is_empty(), "{id} needs an icon");
    }
}

/// The wire form is `{"value": <number>, "unit": "<symbol>"}`, matching the MCP
/// exchange (rule 7).
#[test]
fn a_quantity_travels_as_a_value_and_a_unit() {
    let q = Quantity::parse("6 m/s").unwrap();
    assert_eq!(
        serde_json::to_string(&q).unwrap(),
        r#"{"value":6.0,"unit":"m/s"}"#
    );
    assert_eq!(
        serde_json::from_str::<Quantity>(r#"{"value":6.0,"unit":"m/s"}"#)
            .unwrap()
            .si(),
        6.0
    );
}
