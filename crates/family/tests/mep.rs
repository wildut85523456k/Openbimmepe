use openbim_family::library::{self, fan_coil};
use openbim_family::mep::*;
use openbim_family::*;
use proptest::prelude::*;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}
fn close3(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|k| close(a[k], b[k]))
}

fn solved(over: &[(&str, f64)]) -> Instance {
    let c = fan_coil().compile().unwrap();
    c.solve(&Request {
        overrides: over.to_vec(),
        ..Default::default()
    })
    .unwrap()
}
fn conn<'a>(i: &'a Instance, n: &str) -> &'a Connector {
    i.connectors.iter().find(|c| c.name == n).unwrap()
}

#[test]
fn fan_coil_connectors_match_hand_figures() {
    let i = solved(&[]);
    assert_eq!(i.connectors.len(), 7);
    let s = conn(&i, "supply");
    assert!(close3(s.pos, [0.0, 0.3, 0.125]) && s.dir == [0.0, 1.0, 0.0]);
    assert_eq!(
        s.size,
        Size::Rect {
            width: 0.4,
            height: 0.2
        }
    );
    assert!(
        close(s.flow, 0.1)
            && s.flow_dir == FlowDir::Out
            && s.system == System::SupplyAir
            && s.domain == Domain::Duct
    );
    let r = conn(&i, "return");
    assert!(
        close3(r.pos, [0.0, -0.3, 0.125]) && r.dir == [0.0, -1.0, 0.0] && r.flow_dir == FlowDir::In
    );
    let w = conn(&i, "chw_in");
    assert!(
        close3(w.pos, [0.5, 0.15, 0.25 / 3.0])
            && w.size == Size::Round { diameter: 0.022 }
            && close(w.flow, 0.0003)
    );
    let o = conn(&i, "chw_out");
    assert!(
        close3(o.pos, [0.5, -0.15, 0.25 / 3.0])
            && o.system == System::ChilledWaterReturn
            && o.flow_dir == FlowDir::Out
    );
    let c = conn(&i, "condensate");
    assert!(
        close3(c.pos, [0.5, 0.0, 0.25 / 6.0]) && c.system == System::Condensate && c.flow == 0.0
    );
    let p = conn(&i, "power");
    assert!(
        close3(p.pos, [0.5, 0.25, 1.0 / 6.0])
            && p.size
                == Size::Electrical {
                    voltage: 230.0,
                    poles: 1
                }
            && close(p.flow, 80.0)
    );
    assert_eq!(p.pressure_drop, 0.0);
    // BMS link: depth 0.6 -> y = 0.3 - 0.12 = 0.18, height 0.25 -> z = 0.25 * 2 / 3
    let b = conn(&i, "bms");
    assert!(close3(b.pos, [0.5, 0.18, 0.5 / 3.0]) && b.dir == [1.0, 0.0, 0.0]);
    assert!(b.size == Size::Signal && b.system == System::Controls && b.domain == Domain::Electrical && b.flow_dir == FlowDir::Both);
    assert!(b.flow == 0.0 && b.current().is_none());
}

#[test]
fn fan_coil_has_the_specified_connector_set() {
    // the agreed set (15 Oct 2026): 2 chilled water, 2 air ductwork, 1 drainage, 1 power, 1 BMS
    let i = solved(&[]);
    let count = |f: &dyn Fn(&Connector) -> bool| i.connectors.iter().filter(|c| f(c)).count();
    assert_eq!(i.connectors.len(), 7);
    assert_eq!(count(&|c| matches!(c.system, System::ChilledWaterSupply | System::ChilledWaterReturn) && c.domain == Domain::Pipe), 2);
    assert_eq!(count(&|c| c.domain == Domain::Duct), 2);
    assert_eq!(count(&|c| c.system == System::Condensate), 1);
    assert_eq!(count(&|c| c.system == System::Power), 1);
    assert_eq!(count(&|c| c.system == System::Controls), 1);
    // no two connectors at the same point
    for (k, a) in i.connectors.iter().enumerate() {
        for b in &i.connectors[k + 1..] {
            let d = (0..3).map(|j| (a.pos[j] - b.pos[j]).powi(2)).sum::<f64>().sqrt();
            assert!(d > 0.01, "{} and {} are {d} m apart", a.name, b.name);
        }
    }
}

#[test]
fn signal_connectors() {
    use Domain::*;
    let sig = |d: Domain, s: System| ConnectorDef::new("s", d, s, FlowDir::Both, ("0", "0", "0"), Dir::PlusX, ShapeDef::Signal);
    let run = |c: ConnectorDef| Family { name: "F".into(), connectors: vec![c], ..Default::default() }.compile();
    // accepted on an electrical connector of a controls, data or other system
    for s in [System::Controls, System::Data, System::Other] {
        assert!(run(sig(Electrical, s)).is_ok(), "{s:?}");
    }
    // refused: power system, non-electrical domains, power or flow figures
    assert!(run(sig(Electrical, System::Power)).is_err());
    for d in [Duct, Pipe, CableTray, Conduit] {
        assert!(run(sig(d, System::Other)).is_err(), "{d:?}");
    }
    assert!(run(sig(Electrical, System::Controls).flow("10")).is_err());
    assert!(run(sig(Electrical, System::Controls).power_factor("0.9")).is_err());
    assert!(run(sig(Electrical, System::Controls).pressure_drop("1")).is_err());
    // the controls system takes the same domains as power and data, and maps to IFC CONTROL
    assert!(System::Controls.domain_ok(Electrical) && System::Controls.domain_ok(CableTray) && System::Controls.domain_ok(Conduit));
    assert!(!System::Controls.domain_ok(Duct) && !System::Controls.domain_ok(Pipe));
    assert_eq!(System::Controls.ifc_system(), "CONTROL");
    // mating: signal joins signal only; not a power connector at the same point
    let i = solved(&[]);
    let b = conn(&i, "bms").clone();
    let mut facing = b.clone();
    facing.dir = [-1.0, 0.0, 0.0];
    assert_eq!(mate(&b, &facing, 1e-6, Resize::Strict), Ok(()));
    let mut power = facing.clone();
    power.system = System::Controls;
    power.size = Size::Electrical { voltage: 230.0, poles: 1 };
    assert_eq!(mate(&b, &power, 1e-6, Resize::Strict), Err(MateError::ShapeKind));
    let mut data = facing;
    data.system = System::Data;
    assert_eq!(mate(&b, &data, 1e-6, Resize::Strict), Err(MateError::System));
}

#[test]
fn connectors_follow_the_family_size_and_can_be_hidden() {
    let i = solved(&[
        ("length", 2.0),
        ("depth", 1.0),
        ("height", 0.4),
        ("with_return_duct", 0.0),
    ]);
    assert_eq!(i.connectors.len(), 6);
    assert!(i.connectors.iter().all(|c| c.name != "return"));
    assert!(close3(conn(&i, "supply").pos, [0.0, 0.5, 0.2]));
    assert!(close(conn(&i, "chw_in").pos[0], 1.0));
}

#[test]
fn plan_symbol_is_solved_and_scales() {
    let i = solved(&[]);
    assert_eq!(i.plan.len(), 6);
    assert_eq!(i.plan[0], PlanShape::Line([-0.5, -0.3, 0.5, -0.3]));
    assert_eq!(i.plan[4], PlanShape::Line([-0.5, -0.3, 0.5, 0.3]));
    let j = solved(&[("length", 2.0)]);
    assert_eq!(j.plan[1], PlanShape::Line([1.0, -0.3, 1.0, 0.3]));
}

#[test]
fn categories_map_to_known_ifc_classes() {
    let expect = [
        (Category::Generic, "IfcBuildingElementProxy"),
        (Category::Door, "IfcDoor"),
        (Category::Window, "IfcWindow"),
        (Category::Furniture, "IfcFurniture"),
        (Category::AirTerminal, "IfcAirTerminal"),
        (Category::Fan, "IfcFan"),
        (Category::Damper, "IfcDamper"),
        (Category::Pump, "IfcPump"),
        (Category::Boiler, "IfcBoiler"),
        (Category::Chiller, "IfcChiller"),
        (Category::Coil, "IfcCoil"),
        (Category::UnitaryEquipment, "IfcUnitaryEquipment"),
        (Category::Tank, "IfcTank"),
        (Category::Valve, "IfcValve"),
        (Category::SanitaryTerminal, "IfcSanitaryTerminal"),
        (Category::LightFixture, "IfcLightFixture"),
        (Category::DistributionBoard, "IfcElectricDistributionBoard"),
        (Category::Outlet, "IfcOutlet"),
        (Category::SwitchingDevice, "IfcSwitchingDevice"),
        (Category::Sprinkler, "IfcFireSuppressionTerminal"),
        (Category::DuctFitting, "IfcDuctFitting"),
        (Category::PipeFitting, "IfcPipeFitting"),
        (Category::CableFitting, "IfcCableCarrierFitting"),
    ];
    assert_eq!(expect.len(), Category::ALL.len());
    for (c, n) in expect {
        assert!(Category::ALL.contains(&c));
        assert_eq!(c.ifc_class(), n);
    }
    assert_eq!(Category::Door.default_host(), Host::Wall);
    assert_eq!(Category::Window.default_host(), Host::Wall);
    assert_eq!(Category::AirTerminal.default_host(), Host::Ceiling);
    assert_eq!(Category::LightFixture.default_host(), Host::Ceiling);
    assert_eq!(Category::Sprinkler.default_host(), Host::Ceiling);
    assert_eq!(Category::Outlet.default_host(), Host::Wall);
    assert_eq!(Category::SwitchingDevice.default_host(), Host::Wall);
    assert_eq!(Category::Pump.default_host(), Host::Free);
}

#[test]
fn system_and_flow_names_for_ifc() {
    use System::*;
    let all = [
        (SupplyAir, "VENTILATION"),
        (ReturnAir, "VENTILATION"),
        (OutsideAir, "VENTILATION"),
        (ExhaustAir, "EXHAUST"),
        (ChilledWaterSupply, "CHILLEDWATER"),
        (ChilledWaterReturn, "CHILLEDWATER"),
        (HeatingWaterSupply, "HEATING"),
        (HeatingWaterReturn, "HEATING"),
        (CondenserWaterSupply, "CONDENSERWATER"),
        (CondenserWaterReturn, "CONDENSERWATER"),
        (DomesticCold, "DOMESTICCOLDWATER"),
        (DomesticHot, "DOMESTICHOTWATER"),
        (Sanitary, "WASTEWATER"),
        (Vent, "VENT"),
        (StormWater, "STORMWATER"),
        (Condensate, "DRAINAGE"),
        (FireProtection, "FIREPROTECTION"),
        (Gas, "GAS"),
        (Power, "ELECTRICAL"),
        (Data, "DATA"),
        (Other, "NOTDEFINED"),
    ];
    for (s, n) in all {
        assert_eq!(s.ifc_system(), n);
    }
    assert_eq!(FlowDir::In.ifc(), "SINK");
    assert_eq!(FlowDir::Out.ifc(), "SOURCE");
    assert_eq!(FlowDir::Both.ifc(), "SOURCEANDSINK");
}

#[test]
fn hosting_rules() {
    let lib = library::all().unwrap();
    let door = &lib[1];
    assert_eq!(door.host(), Host::Wall);
    assert_eq!(door.category(), Category::Door);
    assert_eq!(lib[2].host(), Host::Wall);
    assert_eq!(lib[2].category(), Category::Window);
    assert_eq!(lib[0].host(), Host::Free);
    assert_eq!(fan_coil().compile().unwrap().host(), Host::Free);
    let mut f = fan_coil();
    f.host = Some(Host::Ceiling);
    assert_eq!(f.compile().unwrap().host(), Host::Ceiling);
    // a door on a floor, or an opening cutter on a floor, is refused
    let mut d = library::door();
    d.host = Some(Host::Floor);
    assert!(matches!(
        compile_library(&[library::handle(), d]),
        Err(FamilyError::BadMeta(_))
    ));
    let mut w = library::window();
    w.category = Category::Furniture;
    w.host = Some(Host::Face);
    assert!(matches!(w.compile(), Err(FamilyError::BadMeta(_))));
    // door category without a wall host
    let mut g = fan_coil();
    g.category = Category::Door;
    assert_eq!(g.compile().unwrap().host(), Host::Wall);
    g.host = Some(Host::Free);
    assert!(matches!(g.compile(), Err(FamilyError::BadMeta(_))));
    g.category = Category::Window;
    assert!(matches!(g.compile(), Err(FamilyError::BadMeta(_))));
    g.host = Some(Host::Wall);
    assert_eq!(g.compile().unwrap().host(), Host::Wall);
    // a family with an opening and no stated host is hosted by a wall
    let mut o = fan_coil();
    o.opening = Some(OpeningDef::new("0.5", "0.5", "0"));
    assert_eq!(o.compile().unwrap().host(), Host::Wall);
}

#[test]
fn classification_rules() {
    let mut f = fan_coil();
    f.classification = vec![
        ("Uniclass".into(), "XX_00".into()),
        ("OmniClass".into(), "YY 11".into()),
    ];
    let c = f.compile().unwrap();
    assert_eq!(c.classification().len(), 2);
    assert_eq!(
        c.classification()[1],
        ("OmniClass".to_string(), "YY 11".to_string())
    );
    for bad in [
        vec![("".to_string(), "x".to_string())],
        vec![("a".to_string(), "  ".to_string())],
        vec![
            ("a".to_string(), "1".to_string()),
            ("a".to_string(), "2".to_string()),
        ],
    ] {
        let mut f = fan_coil();
        f.classification = bad;
        assert!(matches!(f.compile(), Err(FamilyError::BadMeta(_))));
    }
    let mut f = fan_coil();
    f.classification = (0..9).map(|i| (format!("s{i}"), "c".to_string())).collect();
    assert_eq!(
        f.compile().err(),
        Some(FamilyError::TooMany("classification entries"))
    );
    f.classification = (0..8).map(|i| (format!("s{i}"), "c".to_string())).collect();
    assert!(f.compile().is_ok());
    let mut f = fan_coil();
    f.classification = vec![("a".into(), "x\ny".into())];
    assert!(matches!(f.compile(), Err(FamilyError::BadText { .. })));
    f.classification = vec![("a".into(), "x".repeat(65))];
    assert!(matches!(f.compile(), Err(FamilyError::BadText { .. })));
    f.classification = vec![("a".into(), "x".repeat(64))];
    assert!(f.compile().is_ok());
}

/// Every family in the shipped library carries a Uniclass 2015 code, whatever it is: this is the
/// invariant, so a family added without a code fails here (docs/revit-style-ui/09).
#[test]
fn every_library_family_carries_a_uniclass_code() {
    let lib = library::all().expect("library compiles");
    assert!(!lib.is_empty(), "the library is empty");
    for fam in &lib {
        let codes: Vec<&(String, String)> = fam
            .classification()
            .iter()
            .filter(|(sys, _)| sys == "Uniclass")
            .collect();
        assert!(!codes.is_empty(), "{} carries no Uniclass code", fam.name());
        for (_, c) in codes {
            assert!(
                c.starts_with("Pr_") || c.starts_with("Ss_") || c.starts_with("EF_"),
                "{}: Uniclass code {} is not a Pr_/Ss_/EF_ code",
                fam.name(),
                c
            );
        }
    }
}

/// The code each library family carries is the one decided for it; adding a family without an entry
/// here fails this test (docs/revit-style-ui/09). The IFC export test in crates/ifc checks the codes
/// are written out of the app.
#[test]
fn library_families_carry_the_decided_uniclass_codes() {
    let decided: &[(&str, &str)] = &[
        ("Lever handle", "Pr_30_36_59"),
        ("Door, single flush", "Pr_30_59_24"),
        ("Window, fixed with mullions", "Pr_30_59_98"),
        ("Door, arched", "Pr_30_59_24"),
        ("Fan coil unit, ducted", "Pr_70_65_04_88"),
        ("Air handling unit", "Pr_60_65_03"),
        ("Ceiling diffuser, square", "Pr_70_65_04_02"),
        ("VAV terminal unit, single duct", "Pr_70_65_04_94"),
        ("Pump, end suction", "Pr_65_53"),
        ("Valve, globe", "Pr_65_54"),
        ("WC, close coupled", "Pr_40_20_93"),
        ("Washbasin, wall hung", "Pr_40_20_96"),
        ("Sprinkler head, pendent", "Pr_70_55_84"),
        ("Luminaire, recessed", "Pr_70_70_48"),
        ("Distribution board, 12-way", "Pr_60_70_22"),
        ("Pipe hanger, clevis", "Pr_20_85_47"),
        ("Pipe elbow", "Pr_65_52_63"),
        ("Pipe reducer", "Pr_65_52_63"),
        ("Pipe tee", "Pr_65_52_63"),
    ];
    let lib = library::all().expect("library compiles");
    assert_eq!(lib.len(), decided.len(), "a library family has no decided code");
    for (fam, (name, code)) in lib.iter().zip(decided) {
        assert_eq!(fam.name(), *name, "library order changed");
        let got = fam
            .classification()
            .iter()
            .find(|(sys, _)| sys == "Uniclass")
            .map(|(_, c)| c.as_str())
            .unwrap_or("");
        assert_eq!(got, *code, "{name}: wrong Uniclass code");
    }
}

/// A family's own code is its identity: unique, kept when the family is renamed, and independent of the Uniclass
/// group it belongs to. This table is decided once; a code is never reused or changed. A new family takes the next
/// number and its row is added here.
#[test]
fn library_families_carry_their_own_unique_codes_apart_from_uniclass() {
    let decided: &[(&str, &str)] = &[
        ("Lever handle", "FAM-0001"),
        ("Door, single flush", "FAM-0002"),
        ("Window, fixed with mullions", "FAM-0003"),
        ("Door, arched", "FAM-0004"),
        ("Fan coil unit, ducted", "FAM-0005"),
        ("Air handling unit", "FAM-0006"),
        ("Ceiling diffuser, square", "FAM-0007"),
        ("VAV terminal unit, single duct", "FAM-0008"),
        ("Pump, end suction", "FAM-0009"),
        ("Valve, globe", "FAM-0010"),
        ("WC, close coupled", "FAM-0011"),
        ("Washbasin, wall hung", "FAM-0012"),
        ("Sprinkler head, pendent", "FAM-0013"),
        ("Luminaire, recessed", "FAM-0014"),
        ("Distribution board, 12-way", "FAM-0015"),
        ("Pipe hanger, clevis", "FAM-0016"),
        ("Pipe elbow", "FAM-0017"),
        ("Pipe reducer", "FAM-0018"),
        ("Pipe tee", "FAM-0019"),
    ];
    let lib = library::all().expect("library compiles");
    assert_eq!(lib.len(), decided.len(), "a library family has no decided code");
    for (fam, (name, code)) in lib.iter().zip(decided) {
        assert_eq!((fam.name(), fam.code()), (*name, *code));
    }
    let mut seen = std::collections::BTreeSet::new();
    assert!(lib.iter().all(|f| seen.insert(f.code().to_string())), "a family code is used twice");
    // the two doors are one Uniclass group and two families
    let uniclass = |n: &str| lib.iter().find(|f| f.name() == n).unwrap().classification().iter().find(|(s, _)| s == "Uniclass").map(|(_, c)| c.clone());
    assert_eq!(uniclass("Door, single flush"), uniclass("Door, arched"));
    assert_ne!(lib.iter().find(|f| f.name() == "Door, single flush").unwrap().code(), lib.iter().find(|f| f.name() == "Door, arched").unwrap().code());
}

#[test]
fn a_family_code_that_is_not_a_plain_token_is_refused() {
    use openbim_family::Family;
    let mut f = library::families().remove(0);
    for bad in ["has space", "FAM/1", "é", &"X".repeat(33)] {
        f.code = bad.to_string();
        assert!(f.compile().is_err(), "{bad:?}");
    }
    for ok in ["", "FAM-0001", "mepe.fan_coil-2"] {
        f.code = ok.to_string();
        assert!(f.compile().is_ok(), "{ok:?}");
    }
    let _: Option<Family> = None;
}

fn with_conn(c: ConnectorDef) -> Family {
    let mut f = fan_coil();
    f.connectors = vec![c];
    f
}
fn duct(name: &str) -> ConnectorDef {
    ConnectorDef::new(
        name,
        Domain::Duct,
        System::SupplyAir,
        FlowDir::Out,
        ("0", "0", "0"),
        Dir::PlusY,
        ShapeDef::Round("0.2".into()),
    )
}

#[test]
fn connector_definition_errors() {
    use Domain::*;
    // duplicate and bad names
    let mut f = fan_coil();
    f.connectors = vec![duct("a"), duct("a")];
    assert_eq!(
        f.compile().err(),
        Some(FamilyError::DuplicateName("a".into()))
    );
    assert_eq!(
        with_conn(duct("1bad")).compile().err(),
        Some(FamilyError::BadName("1bad".into()))
    );
    // system must belong to the domain
    let wrong = ConnectorDef::new(
        "p",
        Pipe,
        System::SupplyAir,
        FlowDir::In,
        ("0", "0", "0"),
        Dir::PlusX,
        ShapeDef::Round("0.02".into()),
    );
    assert!(matches!(
        with_conn(wrong).compile(),
        Err(FamilyError::Connector { .. })
    ));
    // shape must fit the domain
    for (d, s, sh) in [
        (
            Pipe,
            System::DomesticCold,
            ShapeDef::Rect("0.1".into(), "0.1".into()),
        ),
        (CableTray, System::Power, ShapeDef::Round("0.1".into())),
        (Electrical, System::Power, ShapeDef::Round("0.1".into())),
        (
            Duct,
            System::SupplyAir,
            ShapeDef::Electrical {
                voltage: "230".into(),
                poles: "1".into(),
            },
        ),
        (
            Conduit,
            System::Power,
            ShapeDef::Rect("0.1".into(), "0.1".into()),
        ),
    ] {
        let c = ConnectorDef::new("x", d, s, FlowDir::In, ("0", "0", "0"), Dir::PlusX, sh);
        assert!(
            matches!(with_conn(c).compile(), Err(FamilyError::Connector { .. })),
            "{d:?}"
        );
    }
    // allowed shapes
    for (d, s, sh) in [
        (
            Duct,
            System::SupplyAir,
            ShapeDef::Rect("0.3".into(), "0.2".into()),
        ),
        (Duct, System::ReturnAir, ShapeDef::Round("0.3".into())),
        (Pipe, System::Gas, ShapeDef::Round("0.03".into())),
        (
            CableTray,
            System::Data,
            ShapeDef::Rect("0.3".into(), "0.1".into()),
        ),
        (Conduit, System::Power, ShapeDef::Round("0.02".into())),
        (
            Electrical,
            System::Other,
            ShapeDef::Electrical {
                voltage: "400".into(),
                poles: "3".into(),
            },
        ),
    ] {
        let c = ConnectorDef::new("x", d, s, FlowDir::Both, ("0", "0", "0"), Dir::MinusZ, sh);
        assert!(with_conn(c).compile().is_ok(), "{d:?} {s:?}");
    }
    // electrical has no pressure drop
    let e = ConnectorDef::new(
        "e",
        Electrical,
        System::Power,
        FlowDir::In,
        ("0", "0", "0"),
        Dir::PlusX,
        ShapeDef::Electrical {
            voltage: "230".into(),
            poles: "1".into(),
        },
    )
    .pressure_drop("1");
    assert!(matches!(
        with_conn(e).compile(),
        Err(FamilyError::Connector { .. })
    ));
    // formulas: unknown value, text value
    assert!(matches!(
        with_conn(duct("a").flow("nope")).compile(),
        Err(FamilyError::Formula { .. })
    ));
    let mut f = fan_coil();
    f.connectors = vec![duct("a").flow("casing")];
    assert!(matches!(f.compile(), Err(FamilyError::TextInFormula(_))));
    // count limits
    let mut f = fan_coil();
    f.connectors = (0..64).map(|i| duct(&format!("c{i}"))).collect();
    assert!(f.compile().is_ok());
    f.connectors = (0..65).map(|i| duct(&format!("c{i}"))).collect();
    assert_eq!(f.compile().err(), Some(FamilyError::TooMany("connectors")));
    let mut f = fan_coil();
    f.plan = (0..200)
        .map(|_| PlanDef::line(("0", "0"), ("1", "0")))
        .collect();
    assert!(f.compile().is_ok());
    f.plan = (0..201)
        .map(|_| PlanDef::line(("0", "0"), ("1", "0")))
        .collect();
    assert_eq!(
        f.compile().err(),
        Some(FamilyError::TooMany("plan symbol lines"))
    );
}

fn run(c: ConnectorDef) -> Result<Instance, FamilyError> {
    with_conn(c).compile().unwrap().solve(&Request::default())
}

#[test]
fn connector_value_errors_at_solve_time() {
    use Domain::*;
    let sized = |s: &str| {
        ConnectorDef::new(
            "a",
            Duct,
            System::SupplyAir,
            FlowDir::Out,
            ("0", "0", "0"),
            Dir::PlusY,
            ShapeDef::Round(s.into()),
        )
    };
    assert!(run(sized("0.1")).is_ok());
    assert!(run(sized("100")).is_ok());
    for bad in ["0", "-0.1", "100.001", "1/0"] {
        assert!(run(sized(bad)).is_err(), "{bad}");
    }
    let rect = ConnectorDef::new(
        "a",
        Duct,
        System::SupplyAir,
        FlowDir::Out,
        ("0", "0", "0"),
        Dir::PlusY,
        ShapeDef::Rect("0.2".into(), "0".into()),
    );
    assert!(matches!(run(rect), Err(FamilyError::Connector { .. })));
    // position
    let at = |x: &str| {
        ConnectorDef::new(
            "a",
            Duct,
            System::SupplyAir,
            FlowDir::Out,
            (x, "0", "0"),
            Dir::PlusY,
            ShapeDef::Round("0.1".into()),
        )
    };
    assert!(run(at("100")).is_ok() && run(at("-100")).is_ok());
    assert!(matches!(
        run(at("100.5")),
        Err(FamilyError::Connector { .. })
    ));
    assert!(matches!(
        run(at("-100.5")),
        Err(FamilyError::Connector { .. })
    ));
    // flow and pressure drop
    assert!(run(duct("a").flow("0")).is_ok());
    assert!(matches!(
        run(duct("a").flow("-0.001")),
        Err(FamilyError::Connector { .. })
    ));
    assert!(matches!(
        run(duct("a").pressure_drop("-1")),
        Err(FamilyError::Connector { .. })
    ));
    assert!(run(duct("a").pressure_drop("250")).unwrap().connectors[0].pressure_drop == 250.0);
    // electrical
    let el = |v: &str, p: &str| {
        ConnectorDef::new(
            "e",
            Electrical,
            System::Power,
            FlowDir::In,
            ("0", "0", "0"),
            Dir::PlusX,
            ShapeDef::Electrical {
                voltage: v.into(),
                poles: p.into(),
            },
        )
    };
    assert!(run(el("230", "1")).is_ok() && run(el("1000000", "4")).is_ok());
    for (v, p) in [
        ("230", "0"),
        ("230", "5"),
        ("230", "2.5"),
        ("0", "1"),
        ("-5", "1"),
        ("1000001", "1"),
    ] {
        assert!(
            matches!(run(el(v, p)), Err(FamilyError::Connector { .. })),
            "{v} {p}"
        );
    }
    // visible_if hides
    assert!(run(duct("a").visible_if("0"))
        .unwrap()
        .connectors
        .is_empty());
    assert_eq!(run(duct("a").visible_if("1")).unwrap().connectors.len(), 1);
}

#[test]
fn plan_symbol_errors() {
    let with = |p: PlanDef| {
        let mut f = fan_coil();
        f.plan = vec![p];
        f.compile().unwrap().solve(&Request::default())
    };
    assert!(with(PlanDef::line(("0", "0"), ("1", "1"))).is_ok());
    assert!(matches!(
        with(PlanDef::line(("1", "1"), ("1", "1"))),
        Err(FamilyError::Plan { index: 0, .. })
    ));
    assert!(matches!(
        with(PlanDef::line(("0", "0"), ("100.5", "1"))),
        Err(FamilyError::Plan { .. })
    ));
    assert!(with(PlanDef::line(("0", "0"), ("100", "1"))).is_ok());
    assert!(matches!(
        with(PlanDef::circle(("0", "0"), "0")),
        Err(FamilyError::Plan { .. })
    ));
    assert!(matches!(
        with(PlanDef::circle(("0", "0"), "-1")),
        Err(FamilyError::Plan { .. })
    ));
    let c = with(PlanDef::circle(("0.1", "0.2"), "0.3")).unwrap();
    assert_eq!(
        c.plan,
        vec![PlanShape::Circle {
            c: [0.1, 0.2],
            r: 0.3
        }]
    );
    assert!(with(PlanDef::circle(("0", "0"), "1").visible_if("0"))
        .unwrap()
        .plan
        .is_empty());
    assert!(with(PlanDef::line(("1", "1"), ("1", "1")).visible_if("0"))
        .unwrap()
        .plan
        .is_empty());
    let mut f = fan_coil();
    f.plan = vec![PlanDef::line(("nope", "0"), ("1", "1"))];
    assert!(matches!(f.compile(), Err(FamilyError::Formula { .. })));
    let mut f = fan_coil();
    f.plan = vec![PlanDef::circle(("0", "0"), "1").visible_if("zzz")];
    assert!(matches!(f.compile(), Err(FamilyError::Formula { .. })));
}

#[test]
fn placing_turns_and_moves_connectors() {
    let i = solved(&[]);
    let w = conn(&i, "chw_in").world(&Pose {
        x: 10.0,
        y: 5.0,
        z: 3.0,
        angle: 90.0,
    });
    // (0.5, 0.15) turned 90 degrees counter-clockwise is (-0.15, 0.5); direction +X becomes +Y
    assert!(close3(w.pos, [9.85, 5.5, 3.0 + 0.25 / 3.0]));
    assert!(close3(w.dir, [0.0, 1.0, 0.0]));
    let w = conn(&i, "supply").world(&Pose {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        angle: 180.0,
    });
    assert!(close3(w.pos, [0.0, -0.3, 0.125]) && close3(w.dir, [0.0, -1.0, 0.0]));
    // vertical directions are not turned
    let up = ConnectorDef::new(
        "u",
        Domain::Pipe,
        System::DomesticCold,
        FlowDir::In,
        ("0", "0", "0"),
        Dir::PlusZ,
        ShapeDef::Round("0.02".into()),
    );
    let u = run(up).unwrap().connectors[0].world(&Pose {
        x: 0.0,
        y: 0.0,
        z: 1.0,
        angle: 37.0,
    });
    assert_eq!(u.dir, [0.0, 0.0, 1.0]);
    assert!(close3(u.pos, [0.0, 0.0, 1.0]));
}

fn facing(c: &Connector) -> Connector {
    let mut m = c.clone();
    m.dir = [-c.dir[0], -c.dir[1], -c.dir[2]];
    m.flow_dir = match c.flow_dir {
        FlowDir::In => FlowDir::Out,
        FlowDir::Out => FlowDir::In,
        FlowDir::Both => FlowDir::Both,
    };
    m
}

#[test]
fn joining_rules() {
    let i = solved(&[]);
    let a = conn(&i, "supply").world(&Pose {
        x: 4.0,
        y: 2.0,
        z: 3.0,
        angle: 30.0,
    });
    let b = facing(&a);
    assert_eq!(mate(&a, &b, 0.001, Resize::Strict), Ok(()));
    let mut x = b.clone();
    x.domain = Domain::Pipe;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Domain));
    let mut x = b.clone();
    x.system = System::ReturnAir;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::System));
    x.system = System::Other;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Ok(()));
    assert_eq!(mate(&x, &a, 0.001, Resize::Strict), Ok(()));
    let mut x = b.clone();
    x.flow_dir = FlowDir::Out;
    assert_eq!(
        mate(&a, &x, 0.001, Resize::Strict),
        Err(MateError::FlowDirection)
    );
    x.flow_dir = FlowDir::Both;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Ok(()));
    let mut y = a.clone();
    y.flow_dir = FlowDir::In;
    let mut z = b.clone();
    z.flow_dir = FlowDir::In;
    assert_eq!(
        mate(&y, &z, 0.001, Resize::Strict),
        Err(MateError::FlowDirection)
    );
    // position: just inside and just outside the tolerance
    let mut x = b.clone();
    x.pos[0] += 0.0009;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Ok(()));
    x.pos[0] += 0.0002;
    assert_eq!(
        mate(&a, &x, 0.001, Resize::Strict),
        Err(MateError::Position)
    );
    let mut x = b.clone();
    x.pos[2] -= 0.01;
    assert_eq!(
        mate(&a, &x, 0.001, Resize::Strict),
        Err(MateError::Position)
    );
    // facing: same direction, or sideways
    let mut x = b.clone();
    x.dir = a.dir;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Facing));
    x.dir = [0.0, 0.0, 1.0];
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Facing));
    // a tiny turn is still a refusal
    let (s, c) = (0.001f64.sin(), 0.001f64.cos());
    x.dir = [
        -(c * a.dir[0] - s * a.dir[1]),
        -(s * a.dir[0] + c * a.dir[1]),
        0.0,
    ];
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Facing));
    // sizes
    let mut x = b.clone();
    x.size = Size::Rect {
        width: 0.5,
        height: 0.2,
    };
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Size));
    assert_eq!(mate(&a, &x, 0.001, Resize::Allow), Ok(()));
    x.size = Size::Rect {
        width: 0.4,
        height: 0.25,
    };
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Size));
    x.size = Size::Round { diameter: 0.4 };
    assert_eq!(
        mate(&a, &x, 0.001, Resize::Allow),
        Err(MateError::ShapeKind)
    );
    // round to round
    let p = conn(&i, "chw_in").world(&Pose {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        angle: 0.0,
    });
    let mut q = facing(&p);
    q.size = Size::Round { diameter: 0.028 };
    assert_eq!(mate(&p, &q, 0.001, Resize::Strict), Err(MateError::Size));
    assert_eq!(mate(&p, &q, 0.001, Resize::Allow), Ok(()));
    // electrical
    let e = conn(&i, "power").world(&Pose {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        angle: 0.0,
    });
    let mut f = facing(&e);
    f.flow_dir = FlowDir::Out;
    assert_eq!(mate(&e, &f, 0.001, Resize::Strict), Ok(()));
    f.size = Size::Electrical {
        voltage: 400.0,
        poles: 1,
    };
    assert_eq!(mate(&e, &f, 0.001, Resize::Allow), Err(MateError::Voltage));
    f.size = Size::Electrical {
        voltage: 230.0,
        poles: 3,
    };
    assert_eq!(mate(&e, &f, 0.001, Resize::Allow), Err(MateError::Poles));
}

#[test]
fn a_duct_stub_family_joins_the_fan_coil_after_placement() {
    // a straight duct stub whose inlet is at its origin facing -Y
    let stub = Family {
        name: "Duct stub".into(),
        values: vec![
            ValueDef::input("w", 0.4, 0.1, 1.0, Scope::Type),
            ValueDef::input("h", 0.2, 0.1, 1.0, Scope::Type),
        ],
        connectors: vec![ConnectorDef::new(
            "in",
            Domain::Duct,
            System::SupplyAir,
            FlowDir::In,
            ("0", "0", "0"),
            Dir::MinusY,
            ShapeDef::Rect("w".into(), "h".into()),
        )],
        category: Category::DuctFitting,
        ..Default::default()
    };
    let fc = solved(&[]);
    let s = conn(&fc, "supply").world(&Pose {
        x: 20.0,
        y: 10.0,
        z: 2.8,
        angle: 90.0,
    });
    // fan coil turned 90 degrees: its +Y outlet now faces -X at x = 20 - 0.3
    assert!(close3(s.pos, [19.7, 10.0, 2.925]) && close3(s.dir, [-1.0, 0.0, 0.0]));
    let si = stub.compile().unwrap().solve(&Request::default()).unwrap();
    // the stub must face +X so its inlet (facing -Y in its own frame) faces +X after a turn of 270 degrees... check by trying angles
    let mut ok = 0;
    for a in [0.0, 90.0, 180.0, 270.0] {
        let t = si.connectors[0].world(&Pose {
            x: 19.7,
            y: 10.0,
            z: 2.925,
            angle: a,
        });
        if mate(&s, &t, 1e-6, Resize::Strict).is_ok() {
            ok += 1;
            assert_eq!(a, 90.0);
        }
    }
    assert_eq!(ok, 1);
}

#[test]
fn solving_many_is_deterministic() {
    let c = fan_coil().compile().unwrap();
    let reqs: Vec<Request> = (0..200)
        .map(|i| Request {
            overrides: vec![("length", 0.5 + i as f64 * 0.01)],
            ..Default::default()
        })
        .collect();
    let a = c.solve_many(&reqs);
    let b = c.solve_many(&reqs);
    assert_eq!(a, b);
    assert!(a.iter().all(|r| r.is_ok()));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]
    #[test]
    fn placement_keeps_lengths_and_a_facing_copy_always_joins(x in -50.0f64..50.0, y in -50.0f64..50.0, z in -5.0f64..5.0, a in -720.0f64..720.0, len in 0.5f64..2.5, dep in 0.35f64..1.2) {
        let i = solved(&[("length", len), ("depth", dep)]);
        let p = Pose { x, y, z, angle: a };
        for c in &i.connectors {
            let w = c.world(&p);
            let r0 = (c.pos[0].powi(2) + c.pos[1].powi(2)).sqrt();
            let r1 = ((w.pos[0] - x).powi(2) + (w.pos[1] - y).powi(2)).sqrt();
            prop_assert!((r0 - r1).abs() < 1e-9);
            prop_assert!((w.pos[2] - z - c.pos[2]).abs() < 1e-12);
            let n = (w.dir[0].powi(2) + w.dir[1].powi(2) + w.dir[2].powi(2)).sqrt();
            prop_assert!((n - 1.0).abs() < 1e-9);
            prop_assert_eq!(w.dir[2], c.dir[2]);
            prop_assert_eq!(mate(&w, &facing(&w), 1e-9, Resize::Strict), Ok(()));
        }
    }
}

#[test]
fn port_type_names() {
    assert_eq!(Domain::Duct.ifc_port_type(), "DUCT");
    assert_eq!(Domain::Pipe.ifc_port_type(), "PIPE");
    assert_eq!(Domain::CableTray.ifc_port_type(), "CABLECARRIER");
    assert_eq!(Domain::Conduit.ifc_port_type(), "CABLECARRIER");
    assert_eq!(Domain::Electrical.ifc_port_type(), "CABLE");
}

#[test]
fn direction_vectors_and_system_domain_matrix() {
    assert_eq!(Dir::PlusX.vec(), [1.0, 0.0, 0.0]);
    assert_eq!(Dir::MinusX.vec(), [-1.0, 0.0, 0.0]);
    assert_eq!(Dir::PlusY.vec(), [0.0, 1.0, 0.0]);
    assert_eq!(Dir::MinusY.vec(), [0.0, -1.0, 0.0]);
    assert_eq!(Dir::PlusZ.vec(), [0.0, 0.0, 1.0]);
    assert_eq!(Dir::MinusZ.vec(), [0.0, 0.0, -1.0]);
    use Domain::*;
    use System::*;
    let doms = [Duct, Pipe, CableTray, Conduit, Electrical];
    let allowed = |s: System| -> Vec<Domain> {
        match s {
            SupplyAir | ReturnAir | ExhaustAir | OutsideAir => vec![Duct],
            Power | Data | Controls => vec![CableTray, Conduit, Electrical],
            Other => doms.to_vec(),
            _ => vec![Pipe],
        }
    };
    for s in [
        SupplyAir,
        ReturnAir,
        ExhaustAir,
        OutsideAir,
        ChilledWaterSupply,
        ChilledWaterReturn,
        HeatingWaterSupply,
        HeatingWaterReturn,
        CondenserWaterSupply,
        CondenserWaterReturn,
        DomesticCold,
        DomesticHot,
        Sanitary,
        Vent,
        StormWater,
        Condensate,
        FireProtection,
        Gas,
        Power,
        Data,
        Controls,
        Other,
    ] {
        for d in doms {
            assert_eq!(s.domain_ok(d), allowed(s).contains(&d), "{s:?} {d:?}");
        }
    }
    // every direction can be used by a compiled connector and comes back unchanged
    for d in [
        Dir::PlusX,
        Dir::MinusX,
        Dir::PlusY,
        Dir::MinusY,
        Dir::PlusZ,
        Dir::MinusZ,
    ] {
        let c = ConnectorDef::new(
            "a",
            Domain::Duct,
            System::OutsideAir,
            FlowDir::In,
            ("0", "0", "0"),
            d,
            ShapeDef::Round("0.1".into()),
        );
        assert_eq!(run(c).unwrap().connectors[0].dir, d.vec());
    }
}

fn stub() -> Family {
    Family {
        name: "Stub".into(),
        connectors: vec![ConnectorDef::new(
            "p",
            Domain::Pipe,
            System::DomesticCold,
            FlowDir::Out,
            ("0.1", "0", "0.2"),
            Dir::PlusX,
            ShapeDef::Round("0.05".into()),
        )
        .flow("0.001")],
        plan: vec![
            PlanDef::line(("0", "0"), ("1", "0")),
            PlanDef::circle(("0.5", "0"), "0.1"),
        ],
        ..Default::default()
    }
}
fn parent(nd: NestedDef) -> Vec<Compiled> {
    let p = Family {
        name: "Parent".into(),
        nested: vec![nd],
        connectors: vec![duct("own")],
        ..Default::default()
    };
    compile_library(&[stub(), p]).unwrap()
}

#[test]
fn nested_connectors_and_plan_move_with_the_child() {
    let lib = parent(NestedDef::new("n", "Stub", ("1", "2", "0.5")));
    let i = lib[1].solve(&Request::default()).unwrap();
    assert_eq!(i.connectors.len(), 2);
    assert_eq!(i.connectors[0].name, "own");
    let c = &i.connectors[1];
    assert_eq!(c.name, "n/p");
    assert!(close3(c.pos, [1.1, 2.0, 0.7]) && c.dir == [1.0, 0.0, 0.0] && close(c.flow, 0.001));
    assert_eq!(
        i.plan,
        vec![
            PlanShape::Line([1.0, 2.0, 2.0, 2.0]),
            PlanShape::Circle {
                c: [1.5, 2.0],
                r: 0.1
            }
        ]
    );
    // turned 90 degrees about the origin
    let lib = parent(NestedDef::new("n", "Stub", ("1", "2", "0.5")).rotate_about("90", "0", "0"));
    let i = lib[1].solve(&Request::default()).unwrap();
    let c = &i.connectors[1];
    assert!(close3(c.pos, [-2.0, 1.1, 0.7]), "{:?}", c.pos);
    assert!(close3(c.dir, [0.0, 1.0, 0.0]));
    assert_eq!(i.plan[0], PlanShape::Line([-2.0, 1.0, -2.0, 2.0]));
    match i.plan[1] {
        PlanShape::Circle { c, r } => {
            assert!(close(c[0], -2.0) && close(c[1], 1.5) && close(r, 0.1));
        }
        _ => panic!(),
    }
    // a turn about a point that is not the origin
    let lib = parent(NestedDef::new("n", "Stub", ("0", "0", "0")).rotate_about("180", "1", "1"));
    let i = lib[1].solve(&Request::default()).unwrap();
    assert!(
        close3(i.connectors[1].pos, [1.9, 2.0, 0.2])
            && close3(i.connectors[1].dir, [-1.0, 0.0, 0.0])
    );
}

#[test]
fn repeated_and_hidden_nested_families() {
    let lib = parent(NestedDef::new("n", "Stub", ("_i * 1 m", "0", "0")).repeat("3"));
    let i = lib[1].solve(&Request::default()).unwrap();
    let names: Vec<_> = i.connectors.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["own", "n#0/p", "n#1/p", "n#2/p"]);
    assert!(close(i.connectors[3].pos[0], 2.1));
    assert_eq!(i.plan.len(), 6);
    let lib = parent(NestedDef::new("n", "Stub", ("0", "0", "0")).visible_if("0"));
    let i = lib[1].solve(&Request::default()).unwrap();
    assert_eq!(i.connectors.len(), 1);
    assert!(i.plan.is_empty());
    // a family nested two levels deep passes its connectors all the way up
    let mid = Family {
        name: "Mid".into(),
        nested: vec![NestedDef::new("s", "Stub", ("1", "0", "0"))],
        ..Default::default()
    };
    let top = Family {
        name: "Top".into(),
        nested: vec![NestedDef::new("m", "Mid", ("0", "5", "0")).rotate_about("90", "0", "0")],
        ..Default::default()
    };
    let lib = compile_library(&[stub(), mid, top]).unwrap();
    let i = lib[2].solve(&Request::default()).unwrap();
    assert_eq!(i.connectors.len(), 1);
    assert_eq!(i.connectors[0].name, "m/s/p");
    // stub connector (0.1,0,0.2) -> in Mid (1.1,0) -> in Top shifted (1.1,5) -> turned 90: (-5, 1.1)
    assert!(
        close3(i.connectors[0].pos, [-5.0, 1.1, 0.2])
            && close3(i.connectors[0].dir, [0.0, 1.0, 0.0])
    );
}

#[test]
fn total_limits_with_nesting() {
    let three = Family {
        name: "Three".into(),
        connectors: (0..3).map(|i| duct(&format!("c{i}"))).collect(),
        plan: (0..5)
            .map(|_| PlanDef::line(("0", "0"), ("1", "0")))
            .collect(),
        ..Default::default()
    };
    let big = |n: &str| {
        compile_library(&[
            three.clone(),
            Family {
                name: "P".into(),
                nested: vec![NestedDef::new("n", "Three", ("0", "0", "0")).repeat(n)],
                ..Default::default()
            },
        ])
        .unwrap()
    };
    assert_eq!(
        big("666")[1]
            .solve(&Request::default())
            .unwrap()
            .connectors
            .len(),
        1998
    );
    assert_eq!(
        big("667")[1].solve(&Request::default()).err(),
        Some(FamilyError::TooMany("connectors in one placement"))
    );
    let p = Family {
        name: "Q".into(),
        nested: vec![
            NestedDef::new("n", "Five", ("0", "0", "0")).repeat("1000"),
            NestedDef::new("m", "Five", ("0", "0", "0")),
        ],
        ..Default::default()
    };
    let five = Family {
        name: "Five".into(),
        plan: (0..5)
            .map(|_| PlanDef::line(("0", "0"), ("1", "0")))
            .collect(),
        ..Default::default()
    };
    let lib = compile_library(&[five.clone(), p]).unwrap();
    assert_eq!(
        lib[1].solve(&Request::default()).err(),
        Some(FamilyError::TooMany("plan symbol lines in one placement"))
    );
    let p = Family {
        name: "Q".into(),
        nested: vec![NestedDef::new("n", "Five", ("0", "0", "0")).repeat("1000")],
        ..Default::default()
    };
    let lib = compile_library(&[five, p]).unwrap();
    assert_eq!(lib[1].solve(&Request::default()).unwrap().plan.len(), 5000);
}

#[test]
fn nested_direction_turns_both_components_and_limit_is_exact() {
    let up = Family {
        name: "Up".into(),
        connectors: vec![ConnectorDef::new(
            "q",
            Domain::Pipe,
            System::DomesticCold,
            FlowDir::In,
            ("0", "0", "0"),
            Dir::PlusY,
            ShapeDef::Round("0.05".into()),
        )],
        ..Default::default()
    };
    let p = Family {
        name: "P".into(),
        nested: vec![NestedDef::new("u", "Up", ("0", "0", "0")).rotate_about("90", "0", "0")],
        ..Default::default()
    };
    let lib = compile_library(&[up, p]).unwrap();
    assert!(close3(
        lib[1].solve(&Request::default()).unwrap().connectors[0].dir,
        [-1.0, 0.0, 0.0]
    ));
    let four = Family {
        name: "Four".into(),
        connectors: (0..4).map(|i| duct(&format!("c{i}"))).collect(),
        ..Default::default()
    };
    let with = |n: &str| {
        compile_library(&[
            four.clone(),
            Family {
                name: "P".into(),
                nested: vec![NestedDef::new("n", "Four", ("0", "0", "0")).repeat(n)],
                ..Default::default()
            },
        ])
        .unwrap()
    };
    assert_eq!(
        with("500")[1]
            .solve(&Request::default())
            .unwrap()
            .connectors
            .len(),
        2000
    );
    assert!(with("501")[1].solve(&Request::default()).is_err());
}

// ---------- connector upgrade: aim, up/roll, power factor ----------

fn rect(name: &str, d: Dir, w: &str, h: &str) -> ConnectorDef {
    ConnectorDef::new(
        name,
        Domain::Duct,
        System::SupplyAir,
        FlowDir::Out,
        ("0", "0", "0"),
        d,
        ShapeDef::Rect(w.into(), h.into()),
    )
}
fn inn(mut c: Connector) -> Connector {
    c.flow_dir = FlowDir::In;
    c
}
fn at_pose(c: &Connector, x: f64, ang: f64) -> Connector {
    c.world(&Pose {
        x,
        y: 0.0,
        z: 0.0,
        angle: ang,
    })
}

#[test]
fn aim_gives_arbitrary_directions() {
    let s = std::f64::consts::FRAC_1_SQRT_2;
    let i = run(duct("a").aim("45", "0")).unwrap();
    assert!(close3(i.connectors[0].dir, [s, s, 0.0]));
    let i = run(duct("a").aim("0", "45")).unwrap();
    assert!(close3(i.connectors[0].dir, [s, 0.0, s]));
    let i = run(duct("a").aim("90", "-90")).unwrap();
    assert!(close3(i.connectors[0].dir, [0.0, 0.0, -1.0]));
    // formulas may use family parameters, and the result is a unit vector
    let i = run(duct("a").aim("30 + 15", "10 deg")).unwrap();
    let d = i.connectors[0].dir;
    assert!(close(d[0] * d[0] + d[1] * d[1] + d[2] * d[2], 1.0));
    assert!(matches!(
        run(duct("a").aim("0", "91")),
        Err(FamilyError::Connector { .. })
    ));
    assert!(matches!(
        run(duct("a").aim("0", "-90.5")),
        Err(FamilyError::Connector { .. })
    ));
    // two 45 degree connectors facing each other mate
    let a = run(duct("a").aim("45", "0")).unwrap().connectors[0].clone();
    let b = inn(run(duct("a").aim("225", "0")).unwrap().connectors[0].clone());
    assert_eq!(mate(&a, &b, 1e-6, Resize::Strict), Ok(()));
    let c = inn(run(duct("a").aim("180", "0")).unwrap().connectors[0].clone());
    assert_eq!(mate(&a, &c, 1e-6, Resize::Strict), Err(MateError::Facing));
}

#[test]
fn up_defaults_and_roll() {
    let up = |d: Dir| run(rect("r", d, "0.4", "0.2")).unwrap().connectors[0].up;
    assert!(close3(up(Dir::PlusY), [0.0, 0.0, 1.0]));
    assert!(close3(up(Dir::MinusX), [0.0, 0.0, 1.0]));
    assert!(close3(up(Dir::PlusZ), [0.0, 1.0, 0.0]));
    assert!(close3(up(Dir::MinusZ), [0.0, 1.0, 0.0]));
    // rolled a quarter turn about +Y: up goes from +Z to +X (right hand about the axis)
    let r = run(rect("r", Dir::PlusY, "0.4", "0.2").roll("90"))
        .unwrap()
        .connectors[0]
        .up;
    assert!(close3(r, [1.0, 0.0, 0.0]));
    let r = run(rect("r", Dir::PlusY, "0.4", "0.2").roll("180"))
        .unwrap()
        .connectors[0]
        .up;
    assert!(close3(r, [0.0, 0.0, -1.0]));
    // up is always square to dir and unit length, also for aimed connectors
    let i = run(rect("r", Dir::PlusX, "0.4", "0.2")
        .aim("30", "50")
        .roll("33"))
    .unwrap();
    let c = &i.connectors[0];
    assert!(close(
        c.dir[0] * c.up[0] + c.dir[1] * c.up[1] + c.dir[2] * c.up[2],
        0.0
    ));
    assert!(close(c.up.iter().map(|v| v * v).sum::<f64>(), 1.0));
    // roll only for rectangular connectors; power factor only for electrical ones
    assert!(matches!(
        with_conn(duct("a").roll("10")).compile(),
        Err(FamilyError::Connector { .. })
    ));
    assert!(matches!(
        with_conn(duct("a").power_factor("0.9")).compile(),
        Err(FamilyError::Connector { .. })
    ));
    // placing the family turns up with it
    let w = at_pose(
        &run(rect("r", Dir::PlusY, "0.4", "0.2")).unwrap().connectors[0],
        1.0,
        90.0,
    );
    assert!(close3(w.up, [0.0, 0.0, 1.0]) && close3(w.dir, [-1.0, 0.0, 0.0]));
}

#[test]
fn rectangular_mate_follows_orientation() {
    let a = run(rect("a", Dir::PlusY, "0.4", "0.2")).unwrap().connectors[0].clone();
    let face = |ang: f64| -> Connector {
        at_pose(
            &inn(run(rect("b", Dir::MinusY, "0.4", "0.2"))
                .unwrap()
                .connectors[0]
                .clone()),
            0.0,
            ang,
        )
    };
    // the same way up
    assert_eq!(mate(&a, &face(0.0), 1e-6, Resize::Strict), Ok(()));
    // turned a quarter about its own axis: widths and heights are crossed, so equal sizes no longer fit
    let b90 = inn(run(rect("b", Dir::MinusY, "0.4", "0.2").roll("90"))
        .unwrap()
        .connectors[0]
        .clone());
    assert_eq!(mate(&a, &b90, 1e-6, Resize::Strict), Err(MateError::Size));
    assert_eq!(mate(&a, &b90, 1e-6, Resize::Allow), Ok(()));
    // crossed but with swapped sizes it fits
    let sw = inn(run(rect("b", Dir::MinusY, "0.2", "0.4").roll("90"))
        .unwrap()
        .connectors[0]
        .clone());
    assert_eq!(mate(&a, &sw, 1e-6, Resize::Strict), Ok(()));
    // upside down is parallel, so aligned
    let b180 = inn(run(rect("b", Dir::MinusY, "0.4", "0.2").roll("180"))
        .unwrap()
        .connectors[0]
        .clone());
    assert_eq!(mate(&a, &b180, 1e-6, Resize::Strict), Ok(()));
    // a 45 degree roll cannot line up
    let b45 = inn(run(rect("b", Dir::MinusY, "0.4", "0.2").roll("45"))
        .unwrap()
        .connectors[0]
        .clone());
    assert_eq!(
        mate(&a, &b45, 1e-6, Resize::Allow),
        Err(MateError::Orientation)
    );
}

#[test]
fn power_factor_apparent_power_and_current() {
    let el = |v: &str, p: &str| {
        ConnectorDef::new(
            "e",
            Domain::Electrical,
            System::Power,
            FlowDir::In,
            ("0", "0", "0"),
            Dir::PlusX,
            ShapeDef::Electrical {
                voltage: v.into(),
                poles: p.into(),
            },
        )
        .flow("9000")
    };
    let c = run(el("230", "1")).unwrap().connectors[0].clone();
    assert!(close(c.power_factor, 1.0) && close(c.apparent_power(), 9000.0));
    assert!((c.current().unwrap() - 9000.0 / 230.0).abs() < 1e-9);
    let c = run(el("230", "1").power_factor("0.9")).unwrap().connectors[0].clone();
    assert!(close(c.apparent_power(), 10000.0) && close(c.flow, 9000.0));
    let c = run(el("400", "3").power_factor("0.9")).unwrap().connectors[0].clone();
    assert!((c.current().unwrap() - 10000.0 / (3f64.sqrt() * 400.0)).abs() < 1e-9);
    let c = run(el("400", "4")).unwrap().connectors[0].clone();
    assert!((c.current().unwrap() - 9000.0 / (3f64.sqrt() * 400.0)).abs() < 1e-9);
    let c = run(el("400", "2")).unwrap().connectors[0].clone();
    assert!((c.current().unwrap() - 9000.0 / 400.0).abs() < 1e-9);
    for bad in ["0", "-0.5", "1.01"] {
        assert!(
            matches!(
                run(el("230", "1").power_factor(bad)),
                Err(FamilyError::Connector { .. })
            ),
            "{bad}"
        );
    }
    assert!(run(el("230", "1").power_factor("1")).is_ok());
    assert_eq!(run(duct("a")).unwrap().connectors[0].current(), None);
}

#[test]
fn type_names_come_in_the_authors_order() {
    let all = library::all().unwrap();
    let door = all
        .iter()
        .find(|c| c.name() == "Door, single flush")
        .unwrap();
    assert_eq!(door.type_names(), vec!["Type A", "Type B"]);
    let fc = all
        .iter()
        .find(|c| c.name() == "Fan coil unit, ducted")
        .unwrap();
    assert!(fc.type_names().is_empty());
}


fn ahu_solve(over: &[(&str, f64)], types: &[(&str, f64)]) -> Result<Instance, FamilyError> {
    library::ahu().compile().unwrap().solve(&Request {
        overrides: over.to_vec(),
        type_values: types.to_vec(),
        ..Default::default()
    })
}
fn ahu_solved(over: &[(&str, f64)]) -> Instance {
    ahu_solve(over, &[]).unwrap()
}
fn diffuser_solve(over: &[(&str, f64)], types: &[(&str, f64)]) -> Result<Instance, FamilyError> {
    library::ceiling_diffuser().compile().unwrap().solve(&Request {
        overrides: over.to_vec(),
        type_values: types.to_vec(),
        ..Default::default()
    })
}
fn diffuser_solved(over: &[(&str, f64)]) -> Instance {
    diffuser_solve(over, &[]).unwrap()
}

#[test]
fn ahu_connectors_match_hand_figures() {
    let i = ahu_solved(&[]);
    assert_eq!(i.connectors.len(), 9, "supply, return, chw in and out, hw in and out, condensate, power, bms");
    let s = conn(&i, "supply");
    assert!(close3(s.pos, [0.0, 0.6, 0.7]) && s.dir == [0.0, 1.0, 0.0]);
    assert_eq!(
        s.size,
        Size::Rect {
            width: 0.8,
            height: 0.5
        }
    );
    assert!(close(s.flow, 1.2) && s.flow_dir == FlowDir::Out && s.system == System::SupplyAir && s.domain == Domain::Duct);
    let r = conn(&i, "return");
    assert!(close3(r.pos, [0.0, -0.6, 0.7]) && r.dir == [0.0, -1.0, 0.0] && r.flow_dir == FlowDir::In && r.system == System::ReturnAir);
    let c = conn(&i, "chw_in");
    assert!(close3(c.pos, [1.25, 0.3, 0.35]) && c.size == Size::Round { diameter: 0.05 } && c.system == System::ChilledWaterSupply && c.flow_dir == FlowDir::In);
    let co = conn(&i, "chw_out");
    assert!(close(co.flow, 0.0006) && co.flow_dir == FlowDir::Out && co.system == System::ChilledWaterReturn);
    let h = conn(&i, "hw_in");
    assert!(close3(h.pos, [1.25, -0.3, 0.35]) && h.system == System::HeatingWaterSupply && close(h.flow, 0.0004) && h.flow_dir == FlowDir::In);
    assert_eq!(conn(&i, "hw_out").system, System::HeatingWaterReturn);
    assert!(close3(conn(&i, "condensate").pos, [1.25, 0.3, 0.175]) && conn(&i, "condensate").system == System::Condensate);
    let p = conn(&i, "power");
    assert_eq!(p.size, Size::Electrical { voltage: 400.0, poles: 3 });
    assert!(close(p.flow, 4000.0) && p.domain == Domain::Electrical && p.system == System::Power && p.flow_dir == FlowDir::In);
    let b = conn(&i, "bms");
    assert_eq!((b.size, b.flow_dir, b.system), (Size::Signal, FlowDir::Both, System::Controls));
    // the return duct can be turned off, as on the fan coil
    let without = ahu_solved(&[("with_return_duct", 0.0)]);
    assert!(without.connectors.iter().all(|c| c.name != "return"), "a unit with no return duct has no return connector");
}

#[test]
fn ahu_follows_its_own_sizes_and_flow() {
    let big = ahu_solve(&[("length", 4.0), ("depth", 2.0), ("height", 2.0)], &[("air_flow", 3.0), ("supply_duct_w", 1.2)]).unwrap();
    let s = conn(&big, "supply");
    assert!(close3(s.pos, [0.0, 1.0, 1.0]) && close(s.flow, 3.0));
    assert_eq!(
        s.size,
        Size::Rect {
            width: 1.2,
            height: 0.5
        }
    );
    assert!(close3(conn(&big, "chw_in").pos, [2.0, 0.5, 0.5]));
    // a duct that cannot fit the casing face is refused by the family's own check
    assert!(matches!(ahu_solve(&[("length", 1.0)], &[("supply_duct_w", 1.2)]), Err(FamilyError::CheckFailed(_))), "1.2 m duct on a 1 m face");
    assert!(matches!(ahu_solve(&[("length", 1.9)], &[("supply_duct_w", 2.0)]), Err(FamilyError::CheckFailed(_))), "2 m duct on a 1.9 m face");
    // a value of the wrong scope is refused whichever way it is passed
    assert!(matches!(ahu_solve(&[("air_flow", 2.0)], &[]), Err(FamilyError::WrongScope(_))), "a type value is not an instance override");
    assert!(matches!(ahu_solve(&[], &[("length", 3.0)]), Err(FamilyError::WrongScope(_))), "an instance value is not a type value");
}

#[test]
fn the_ceiling_diffuser_hangs_on_a_ceiling_and_takes_its_duct_from_above() {
    let i = diffuser_solved(&[]);
    assert_eq!(i.connectors.len(), 1, "a diffuser has one duct connection");
    let s = conn(&i, "supply");
    assert!(close3(s.pos, [0.0, 0.0, 0.15]) && s.dir == [0.0, 0.0, 1.0], "the drop joins it from above");
    assert_eq!(s.size, Size::Round { diameter: 0.2 });
    assert!(close(s.flow, 0.05) && close(s.pressure_drop, 30.0) && s.flow_dir == FlowDir::In && s.system == System::SupplyAir);
    let big = diffuser_solve(&[], &[("neck_d", 0.4), ("neck_h", 0.3), ("air_flow", 0.2)]).unwrap();
    let b = conn(&big, "supply");
    assert_eq!(b.size, Size::Round { diameter: 0.4 });
    assert!(close3(b.pos, [0.0, 0.0, 0.3]) && close(b.flow, 0.2));
    // a neck wider than the face is refused by the family's own check
    assert!(matches!(diffuser_solve(&[("face", 0.3)], &[("neck_d", 0.4)]), Err(FamilyError::CheckFailed(_))));
    // the type's values win over the library's own defaults (neck_d 0.2 and neck_h 0.15), and the face, which is the
    // instance's own value, is untouched by them
    let tall = diffuser_solve(&[("face", 0.8)], &[("neck_h", 0.25)]).unwrap();
    assert!(close3(conn(&tall, "supply").pos, [0.0, 0.0, 0.25]) && close3(conn(&tall, "supply").pos, [0.0, 0.0, 0.25]));
    assert_eq!(conn(&tall, "supply").size, Size::Round { diameter: 0.2 }, "the library default still stands for what the type does not set");
    // a value of the wrong scope is refused whichever way it is passed
    assert!(matches!(diffuser_solve(&[("neck_d", 0.2)], &[]), Err(FamilyError::WrongScope(_))), "a type value is not an instance override");
    assert!(matches!(diffuser_solve(&[], &[("face", 0.6)]), Err(FamilyError::WrongScope(_))), "an instance value is not a type value");
}

#[test]
fn the_library_carries_the_air_handling_unit_and_the_ceiling_diffuser() {
    let all = library::all().unwrap();
    let ahu = all.iter().find(|c| c.name() == "Air handling unit").unwrap();
    assert_eq!(ahu.category(), Category::UnitaryEquipment);
    assert_eq!(ahu.category().ifc_class(), "IfcUnitaryEquipment");
    assert_eq!(ahu.host(), Host::Floor, "an air handling unit stands on a floor");
    let d = all.iter().find(|c| c.name() == "Ceiling diffuser, square").unwrap();
    assert_eq!(d.category(), Category::AirTerminal);
    assert_eq!(d.category().ifc_class(), "IfcAirTerminal");
    assert_eq!(d.host(), Host::Ceiling, "a diffuser belongs on a ceiling");
    assert!(d.type_names().is_empty(), "the library ships no named types for either: they come from a catalogue");
    assert!(ahu.type_names().is_empty());
}


#[test]
fn the_type_layer_sits_between_the_librarys_named_type_and_the_instances_own_values() {
    // a family with one value of each scope and a named type of its own that sets both
    let f = Family {
        name: "Layered".into(),
        values: vec![
            ValueDef::input("size", 1.0, 0.1, 10.0, Scope::Instance),
            ValueDef::number("flow", 2.0, 0.1, 10.0, Scope::Type),
        ],
        types: vec![TypeDef {
            name: "T".into(),
            values: vec![("size".into(), 3.0), ("flow".into(), 4.0)],
            ..Default::default()
        }],
        ..Default::default()
    };
    let c = f.compile().unwrap();
    let v = |req: &Request| -> Vec<(String, f64)> {
        let i = c.solve(req).unwrap();
        c.input_values(&i).into_iter().map(|(n, x)| (n.to_string(), x)).collect()
    };
    // the library's own defaults, with nothing else asked for
    assert_eq!(v(&Request::default()), vec![("size".to_string(), 1.0), ("flow".to_string(), 2.0)]);
    // the named type of the library
    assert_eq!(v(&Request { type_name: Some("T"), ..Default::default() }), vec![("size".to_string(), 3.0), ("flow".to_string(), 4.0)]);
    // the caller's own type of the family wins over the library's named type, and the instance wins over both
    let r = Request { type_name: Some("T"), type_values: vec![("flow", 5.0)], overrides: vec![("size", 6.0)], ..Default::default() };
    assert_eq!(v(&r), vec![("size".to_string(), 6.0), ("flow".to_string(), 5.0)]);
    // and an unknown name is refused whatever layer it comes from
    assert!(matches!(c.solve(&Request { type_values: vec![("nope", 1.0)], ..Default::default() }), Err(FamilyError::UnknownValue(_))));
    assert!(matches!(c.solve(&Request { overrides: vec![("nope", 1.0)], ..Default::default() }), Err(FamilyError::UnknownValue(_))));
}

#[test]
fn the_pipe_elbow_outlet_follows_its_angle_and_ends_where_the_bend_ends() {
    let c = library::pipe_elbow().compile().unwrap();
    for angle in [90.0_f64, 45.0, 22.5] {
        let i = c.solve(&Request { type_values: vec![("angle", angle)], ..Default::default() }).unwrap();
        let (inlet, out) = (conn(&i, "in left"), conn(&i, "out left"));
        let r = 1.5 * 0.05;
        let a = angle.to_radians();
        assert!(close3(inlet.pos, [0.0, 0.0, 0.0]) && inlet.dir == [-1.0, 0.0, 0.0]);
        assert!(close3(out.pos, [r * a.sin(), r - r * a.cos(), 0.0]), "{angle}: {:?}", out.pos);
        assert!(close3(out.dir, [a.cos(), a.sin(), 0.0]), "{angle}: {:?}", out.dir);
        // the solid is the bend of that angle: Pappus, angle x centre-line radius x section area
        let v = i.forms[0].volume();
        let want = a * r * std::f64::consts::PI * 0.025 * 0.025;
        assert!((v - want).abs() < want * 1e-6, "{angle}: {v} vs {want}");
    }
}

#[test]
fn the_pipe_elbow_turns_left_right_up_and_down_and_each_outlet_is_where_the_bend_ends() {
    let c = library::pipe_elbow().compile().unwrap();
    let (r, rad) = (1.5 * 0.05, 0.025);
    for turn in ["left", "right", "up", "down"] {
        for angle in [90.0_f64, 45.0] {
            let i = c.solve(&Request { type_values: vec![("angle", angle)], type_texts: vec![("turn", turn)], ..Default::default() }).unwrap();
            assert_eq!(i.connectors.len(), 2, "{turn}: only the connectors of its own turn");
            assert_eq!(i.forms.len(), 1, "{turn}: only its own bend");
            let (inlet, out) = (conn(&i, &format!("in {turn}")), conn(&i, &format!("out {turn}")));
            let a = angle.to_radians();
            // the pipe runs along +x for left and down, along -x for right and up; the bend goes to the side it names
            let sx = if turn == "right" || turn == "up" { -1.0 } else { 1.0 };
            let (sy, sz) = (match turn { "left" => 1.0, "right" => -1.0, _ => 0.0 }, match turn { "up" => 1.0, "down" => -1.0, _ => 0.0 });
            let want_pos = [sx * r * a.sin(), (sy + sz * 0.0) * (r - r * a.cos()), sz * (r - r * a.cos())];
            assert!(close3(out.pos, want_pos), "{turn} {angle}: {:?} vs {want_pos:?}", out.pos);
            let want_dir = [sx * a.cos(), sy * a.sin(), sz * a.sin()];
            assert!(close3(out.dir, want_dir), "{turn} {angle}: {:?} vs {want_dir:?}", out.dir);
            assert!(close3(inlet.dir, [-sx, 0.0, 0.0]), "{turn}: the inlet faces away from the bend");
            // the bend is the solid between the two: its box holds the outlet's centre, and its end face is the outlet's
            let (lo, hi) = i.forms[0].bounds_local();
            for k in 0..3 { assert!(lo[k] - 1e-9 <= out.pos[k] && out.pos[k] <= hi[k] + 1e-9, "{turn} {angle} axis {k}: {lo:?} {hi:?} {:?}", out.pos); }
            let want = a * r * std::f64::consts::PI * rad * rad;
            assert!((i.forms[0].volume() - want).abs() < want * 1e-6, "{turn} {angle}");
        }
    }
}

#[test]
fn the_reducer_and_the_tee_carry_the_sizes_of_their_type_on_every_connector() {
    let red = library::pipe_reducer().compile().unwrap();
    let i = red.solve(&Request { type_values: vec![("pipe_d", 0.1), ("pipe_d2", 0.065), ("length", 0.15)], ..Default::default() }).unwrap();
    let (large, small) = (conn(&i, "large"), conn(&i, "small"));
    assert_eq!(large.size, Size::Round { diameter: 0.1 });
    assert_eq!(small.size, Size::Round { diameter: 0.065 });
    assert!(close3(small.pos, [0.15, 0.0, 0.0]) && small.dir == [1.0, 0.0, 0.0] && large.dir == [-1.0, 0.0, 0.0]);
    let (lo, hi) = i.forms[0].bounds_local();
    assert!(close3(lo, [0.0, -0.05, -0.05]) && close3(hi, [0.15, 0.05, 0.05]), "{lo:?} {hi:?}");
    // a small end as large as the large end is refused
    assert!(red.solve(&Request { type_values: vec![("pipe_d", 0.05), ("pipe_d2", 0.05)], ..Default::default() }).is_err());

    let tee = library::pipe_tee().compile().unwrap();
    let i = tee.solve(&Request { type_values: vec![("pipe_d", 0.08), ("branch_d", 0.05), ("leg", 0.12), ("branch_len", 0.1)], ..Default::default() }).unwrap();
    assert_eq!(i.connectors.len(), 3);
    assert_eq!(conn(&i, "run in").size, Size::Round { diameter: 0.08 });
    assert_eq!(conn(&i, "run out").size, Size::Round { diameter: 0.08 });
    let b = conn(&i, "branch left");
    assert_eq!(b.size, Size::Round { diameter: 0.05 });
    assert!(close3(b.pos, [0.0, 0.1, 0.0]) && b.dir == [0.0, 1.0, 0.0]);
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for f in &i.forms { let (l, h) = f.bounds_local(); for k in 0..3 { lo[k] = lo[k].min(l[k]); hi[k] = hi[k].max(h[k]); } }
    assert!(close3(lo, [-0.12, -0.04, -0.04]) && close3(hi, [0.12, 0.1, 0.04]), "{lo:?} {hi:?}");
    // the branch goes right, up or down as well
    for (side, dir, pos) in [("right", [0.0, -1.0, 0.0], [0.0, -0.1, 0.0]), ("up", [0.0, 0.0, 1.0], [0.0, 0.0, 0.1]), ("down", [0.0, 0.0, -1.0], [0.0, 0.0, -0.1])] {
        let i = tee.solve(&Request { type_values: vec![("pipe_d", 0.08), ("branch_d", 0.05), ("leg", 0.12), ("branch_len", 0.1)], type_texts: vec![("branch", side)], ..Default::default() }).unwrap();
        assert_eq!(i.connectors.len(), 3, "{side}");
        let b = conn(&i, &format!("branch {side}"));
        assert!(close3(b.pos, pos) && close3(b.dir, dir), "{side}: {:?} {:?}", b.pos, b.dir);
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for f in &i.forms { let (l, h) = f.bounds_local(); for k in 0..3 { lo[k] = lo[k].min(l[k]); hi[k] = hi[k].max(h[k]); } }
        for k in 0..3 { assert!(lo[k] - 1e-9 <= b.pos[k] && b.pos[k] <= hi[k] + 1e-9, "{side}: the branch end lies on the solid"); }
        assert_eq!(i.forms.len(), 2, "{side}: the run and its own branch only");
    }
    // a branch larger than the run is refused
    assert!(tee.solve(&Request { type_values: vec![("pipe_d", 0.05), ("branch_d", 0.08)], ..Default::default() }).is_err());
}

#[test]
fn the_eccentric_reducer_keeps_its_flat_side_straight() {
    let red = library::pipe_reducer().compile().unwrap();
    let (d, d2, l) = (0.1_f64, 0.065_f64, 0.15_f64);
    for (flat, want) in [("concentric", [0.0, 0.0]), ("bottom", [0.0, -0.0175]), ("top", [0.0, 0.0175]), ("left", [0.0175, 0.0]), ("right", [-0.0175, 0.0])] {
        let i = red.solve(&Request { type_values: vec![("pipe_d", d), ("pipe_d2", d2), ("length", l)], type_texts: vec![("flat", flat)], ..Default::default() }).unwrap();
        let small = conn(&i, "small");
        assert!(close3(small.pos, [l, want[0], want[1]]), "{flat}: {:?}", small.pos);
        assert!(small.dir == [1.0, 0.0, 0.0], "an eccentric reducer still leaves along the run");
        // the flat side: the solid reaches the large pipe's wall on that side all the way to the small end
        let (lo, hi) = i.forms[0].bounds_local();
        let (ylo, yhi, zlo, zhi) = (lo[1], hi[1], lo[2], hi[2]);
        match flat {
            "bottom" => assert!(close(zlo, -d / 2.0), "{flat}"),
            "top" => assert!(close(zhi, d / 2.0), "{flat}"),
            "left" => assert!(close(yhi, d / 2.0), "{flat}"),
            "right" => assert!(close(ylo, -d / 2.0), "{flat}"),
            _ => {}
        }
        // and the small end's wall on the flat side is in line with the large pipe's wall
        let wall = match flat { "bottom" => small.pos[2] - d2 / 2.0 + d / 2.0, "top" => d / 2.0 - (small.pos[2] + d2 / 2.0), "left" => d / 2.0 - (small.pos[1] + d2 / 2.0), "right" => small.pos[1] - d2 / 2.0 + d / 2.0, _ => 0.0 };
        assert!(close(wall, 0.0), "{flat}: the flat side runs straight, off by {wall}");
    }
}
