use openbim_family::library::{self, fan_coil};
use openbim_family::mep::*;
use openbim_family::*;
use proptest::prelude::*;

fn close(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 }
fn close3(a: [f64; 3], b: [f64; 3]) -> bool { (0..3).all(|k| close(a[k], b[k])) }

fn solved(over: &[(&str, f64)]) -> Instance {
    let c = fan_coil().compile().unwrap();
    c.solve(&Request { overrides: over.to_vec(), ..Default::default() }).unwrap()
}
fn conn<'a>(i: &'a Instance, n: &str) -> &'a Connector { i.connectors.iter().find(|c| c.name == n).unwrap() }

#[test]
fn fan_coil_connectors_match_hand_figures() {
    let i = solved(&[]);
    assert_eq!(i.connectors.len(), 6);
    let s = conn(&i, "supply");
    assert!(close3(s.pos, [0.0, 0.3, 0.125]) && s.dir == [0.0, 1.0, 0.0]);
    assert_eq!(s.size, Size::Rect { width: 0.4, height: 0.2 });
    assert!(close(s.flow, 0.1) && s.flow_dir == FlowDir::Out && s.system == System::SupplyAir && s.domain == Domain::Duct);
    let r = conn(&i, "return");
    assert!(close3(r.pos, [0.0, -0.3, 0.125]) && r.dir == [0.0, -1.0, 0.0] && r.flow_dir == FlowDir::In);
    let w = conn(&i, "chw_in");
    assert!(close3(w.pos, [0.5, 0.15, 0.25 / 3.0]) && w.size == Size::Round { diameter: 0.022 } && close(w.flow, 0.0003));
    let o = conn(&i, "chw_out");
    assert!(close3(o.pos, [0.5, -0.15, 0.25 / 3.0]) && o.system == System::ChilledWaterReturn && o.flow_dir == FlowDir::Out);
    let c = conn(&i, "condensate");
    assert!(close3(c.pos, [0.5, 0.0, 0.25 / 6.0]) && c.system == System::Condensate && c.flow == 0.0);
    let p = conn(&i, "power");
    assert!(close3(p.pos, [0.5, 0.25, 1.0 / 6.0]) && p.size == Size::Electrical { voltage: 230.0, poles: 1 } && close(p.flow, 80.0));
    assert_eq!(p.pressure_drop, 0.0);
}

#[test]
fn connectors_follow_the_family_size_and_can_be_hidden() {
    let i = solved(&[("length", 2.0), ("depth", 1.0), ("height", 0.4), ("with_return_duct", 0.0)]);
    assert_eq!(i.connectors.len(), 5);
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
        (Category::Generic, "IfcBuildingElementProxy"), (Category::Door, "IfcDoor"), (Category::Window, "IfcWindow"), (Category::Furniture, "IfcFurniture"),
        (Category::AirTerminal, "IfcAirTerminal"), (Category::Fan, "IfcFan"), (Category::Damper, "IfcDamper"), (Category::Pump, "IfcPump"),
        (Category::Boiler, "IfcBoiler"), (Category::Chiller, "IfcChiller"), (Category::Coil, "IfcCoil"), (Category::UnitaryEquipment, "IfcUnitaryEquipment"),
        (Category::Tank, "IfcTank"), (Category::Valve, "IfcValve"), (Category::SanitaryTerminal, "IfcSanitaryTerminal"), (Category::LightFixture, "IfcLightFixture"),
        (Category::DistributionBoard, "IfcElectricDistributionBoard"), (Category::Outlet, "IfcOutlet"), (Category::SwitchingDevice, "IfcSwitchingDevice"),
        (Category::Sprinkler, "IfcFireSuppressionTerminal"), (Category::DuctFitting, "IfcDuctFitting"), (Category::PipeFitting, "IfcPipeFitting"), (Category::CableFitting, "IfcCableCarrierFitting"),
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
        (SupplyAir, "VENTILATION"), (ReturnAir, "VENTILATION"), (OutsideAir, "VENTILATION"), (ExhaustAir, "EXHAUST"), (ChilledWaterSupply, "CHILLEDWATER"),
        (ChilledWaterReturn, "CHILLEDWATER"), (HeatingWaterSupply, "HEATING"), (HeatingWaterReturn, "HEATING"), (CondenserWaterSupply, "CONDENSERWATER"),
        (CondenserWaterReturn, "CONDENSERWATER"), (DomesticCold, "DOMESTICCOLDWATER"), (DomesticHot, "DOMESTICHOTWATER"), (Sanitary, "WASTEWATER"), (Vent, "VENT"),
        (StormWater, "STORMWATER"), (Condensate, "DRAINAGE"), (FireProtection, "FIREPROTECTION"), (Gas, "GAS"), (Power, "ELECTRICAL"), (Data, "DATA"), (Other, "NOTDEFINED"),
    ];
    for (s, n) in all { assert_eq!(s.ifc_system(), n); }
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
    assert!(matches!(compile_library(&[library::handle(), d]), Err(FamilyError::BadMeta(_))));
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
    f.classification = vec![("Uniclass".into(), "XX_00".into()), ("OmniClass".into(), "YY 11".into())];
    let c = f.compile().unwrap();
    assert_eq!(c.classification().len(), 2);
    assert_eq!(c.classification()[1], ("OmniClass".to_string(), "YY 11".to_string()));
    for bad in [vec![("".to_string(), "x".to_string())], vec![("a".to_string(), "  ".to_string())], vec![("a".to_string(), "1".to_string()), ("a".to_string(), "2".to_string())]] {
        let mut f = fan_coil();
        f.classification = bad;
        assert!(matches!(f.compile(), Err(FamilyError::BadMeta(_))));
    }
    let mut f = fan_coil();
    f.classification = (0..9).map(|i| (format!("s{i}"), "c".to_string())).collect();
    assert_eq!(f.compile().err(), Some(FamilyError::TooMany("classification entries")));
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

fn with_conn(c: ConnectorDef) -> Family {
    let mut f = fan_coil();
    f.connectors = vec![c];
    f
}
fn duct(name: &str) -> ConnectorDef {
    ConnectorDef::new(name, Domain::Duct, System::SupplyAir, FlowDir::Out, ("0", "0", "0"), Dir::PlusY, ShapeDef::Round("0.2".into()))
}

#[test]
fn connector_definition_errors() {
    use Domain::*;
    // duplicate and bad names
    let mut f = fan_coil();
    f.connectors = vec![duct("a"), duct("a")];
    assert_eq!(f.compile().err(), Some(FamilyError::DuplicateName("a".into())));
    assert_eq!(with_conn(duct("1bad")).compile().err(), Some(FamilyError::BadName("1bad".into())));
    // system must belong to the domain
    let wrong = ConnectorDef::new("p", Pipe, System::SupplyAir, FlowDir::In, ("0", "0", "0"), Dir::PlusX, ShapeDef::Round("0.02".into()));
    assert!(matches!(with_conn(wrong).compile(), Err(FamilyError::Connector { .. })));
    // shape must fit the domain
    for (d, s, sh) in [
        (Pipe, System::DomesticCold, ShapeDef::Rect("0.1".into(), "0.1".into())),
        (CableTray, System::Power, ShapeDef::Round("0.1".into())),
        (Electrical, System::Power, ShapeDef::Round("0.1".into())),
        (Duct, System::SupplyAir, ShapeDef::Electrical { voltage: "230".into(), poles: "1".into() }),
        (Conduit, System::Power, ShapeDef::Rect("0.1".into(), "0.1".into())),
    ] {
        let c = ConnectorDef::new("x", d, s, FlowDir::In, ("0", "0", "0"), Dir::PlusX, sh);
        assert!(matches!(with_conn(c).compile(), Err(FamilyError::Connector { .. })), "{d:?}");
    }
    // allowed shapes
    for (d, s, sh) in [
        (Duct, System::SupplyAir, ShapeDef::Rect("0.3".into(), "0.2".into())),
        (Duct, System::ReturnAir, ShapeDef::Round("0.3".into())),
        (Pipe, System::Gas, ShapeDef::Round("0.03".into())),
        (CableTray, System::Data, ShapeDef::Rect("0.3".into(), "0.1".into())),
        (Conduit, System::Power, ShapeDef::Round("0.02".into())),
        (Electrical, System::Other, ShapeDef::Electrical { voltage: "400".into(), poles: "3".into() }),
    ] {
        let c = ConnectorDef::new("x", d, s, FlowDir::Both, ("0", "0", "0"), Dir::MinusZ, sh);
        assert!(with_conn(c).compile().is_ok(), "{d:?} {s:?}");
    }
    // electrical has no pressure drop
    let e = ConnectorDef::new("e", Electrical, System::Power, FlowDir::In, ("0", "0", "0"), Dir::PlusX, ShapeDef::Electrical { voltage: "230".into(), poles: "1".into() }).pressure_drop("1");
    assert!(matches!(with_conn(e).compile(), Err(FamilyError::Connector { .. })));
    // formulas: unknown value, text value
    assert!(matches!(with_conn(duct("a").flow("nope")).compile(), Err(FamilyError::Formula { .. })));
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
    f.plan = (0..200).map(|_| PlanDef::line(("0", "0"), ("1", "0"))).collect();
    assert!(f.compile().is_ok());
    f.plan = (0..201).map(|_| PlanDef::line(("0", "0"), ("1", "0"))).collect();
    assert_eq!(f.compile().err(), Some(FamilyError::TooMany("plan symbol lines")));
}

fn run(c: ConnectorDef) -> Result<Instance, FamilyError> {
    with_conn(c).compile().unwrap().solve(&Request::default())
}

#[test]
fn connector_value_errors_at_solve_time() {
    use Domain::*;
    let sized = |s: &str| ConnectorDef::new("a", Duct, System::SupplyAir, FlowDir::Out, ("0", "0", "0"), Dir::PlusY, ShapeDef::Round(s.into()));
    assert!(run(sized("0.1")).is_ok());
    assert!(run(sized("100")).is_ok());
    for bad in ["0", "-0.1", "100.001", "1/0"] { assert!(run(sized(bad)).is_err(), "{bad}"); }
    let rect = ConnectorDef::new("a", Duct, System::SupplyAir, FlowDir::Out, ("0", "0", "0"), Dir::PlusY, ShapeDef::Rect("0.2".into(), "0".into()));
    assert!(matches!(run(rect), Err(FamilyError::Connector { .. })));
    // position
    let at = |x: &str| ConnectorDef::new("a", Duct, System::SupplyAir, FlowDir::Out, (x, "0", "0"), Dir::PlusY, ShapeDef::Round("0.1".into()));
    assert!(run(at("100")).is_ok() && run(at("-100")).is_ok());
    assert!(matches!(run(at("100.5")), Err(FamilyError::Connector { .. })));
    assert!(matches!(run(at("-100.5")), Err(FamilyError::Connector { .. })));
    // flow and pressure drop
    assert!(run(duct("a").flow("0")).is_ok());
    assert!(matches!(run(duct("a").flow("-0.001")), Err(FamilyError::Connector { .. })));
    assert!(matches!(run(duct("a").pressure_drop("-1")), Err(FamilyError::Connector { .. })));
    assert!(run(duct("a").pressure_drop("250")).unwrap().connectors[0].pressure_drop == 250.0);
    // electrical
    let el = |v: &str, p: &str| ConnectorDef::new("e", Electrical, System::Power, FlowDir::In, ("0", "0", "0"), Dir::PlusX, ShapeDef::Electrical { voltage: v.into(), poles: p.into() });
    assert!(run(el("230", "1")).is_ok() && run(el("1000000", "4")).is_ok());
    for (v, p) in [("230", "0"), ("230", "5"), ("230", "2.5"), ("0", "1"), ("-5", "1"), ("1000001", "1")] { assert!(matches!(run(el(v, p)), Err(FamilyError::Connector { .. })), "{v} {p}"); }
    // visible_if hides
    assert!(run(duct("a").visible_if("0")).unwrap().connectors.is_empty());
    assert_eq!(run(duct("a").visible_if("1")).unwrap().connectors.len(), 1);
}

#[test]
fn plan_symbol_errors() {
    let with = |p: PlanDef| { let mut f = fan_coil(); f.plan = vec![p]; f.compile().unwrap().solve(&Request::default()) };
    assert!(with(PlanDef::line(("0", "0"), ("1", "1"))).is_ok());
    assert!(matches!(with(PlanDef::line(("1", "1"), ("1", "1"))), Err(FamilyError::Plan { index: 0, .. })));
    assert!(matches!(with(PlanDef::line(("0", "0"), ("100.5", "1"))), Err(FamilyError::Plan { .. })));
    assert!(with(PlanDef::line(("0", "0"), ("100", "1"))).is_ok());
    assert!(matches!(with(PlanDef::circle(("0", "0"), "0")), Err(FamilyError::Plan { .. })));
    assert!(matches!(with(PlanDef::circle(("0", "0"), "-1")), Err(FamilyError::Plan { .. })));
    let c = with(PlanDef::circle(("0.1", "0.2"), "0.3")).unwrap();
    assert_eq!(c.plan, vec![PlanShape::Circle { c: [0.1, 0.2], r: 0.3 }]);
    assert!(with(PlanDef::circle(("0", "0"), "1").visible_if("0")).unwrap().plan.is_empty());
    assert!(with(PlanDef::line(("1", "1"), ("1", "1")).visible_if("0")).unwrap().plan.is_empty());
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
    let w = conn(&i, "chw_in").world(&Pose { x: 10.0, y: 5.0, z: 3.0, angle: 90.0 });
    // (0.5, 0.15) turned 90 degrees counter-clockwise is (-0.15, 0.5); direction +X becomes +Y
    assert!(close3(w.pos, [9.85, 5.5, 3.0 + 0.25 / 3.0]));
    assert!(close3(w.dir, [0.0, 1.0, 0.0]));
    let w = conn(&i, "supply").world(&Pose { x: 0.0, y: 0.0, z: 0.0, angle: 180.0 });
    assert!(close3(w.pos, [0.0, -0.3, 0.125]) && close3(w.dir, [0.0, -1.0, 0.0]));
    // vertical directions are not turned
    let up = ConnectorDef::new("u", Domain::Pipe, System::DomesticCold, FlowDir::In, ("0", "0", "0"), Dir::PlusZ, ShapeDef::Round("0.02".into()));
    let u = run(up).unwrap().connectors[0].world(&Pose { x: 0.0, y: 0.0, z: 1.0, angle: 37.0 });
    assert_eq!(u.dir, [0.0, 0.0, 1.0]);
    assert!(close3(u.pos, [0.0, 0.0, 1.0]));
}

fn facing(c: &Connector) -> Connector {
    let mut m = c.clone();
    m.dir = [-c.dir[0], -c.dir[1], -c.dir[2]];
    m.flow_dir = match c.flow_dir { FlowDir::In => FlowDir::Out, FlowDir::Out => FlowDir::In, FlowDir::Both => FlowDir::Both };
    m
}

#[test]
fn joining_rules() {
    let i = solved(&[]);
    let a = conn(&i, "supply").world(&Pose { x: 4.0, y: 2.0, z: 3.0, angle: 30.0 });
    let b = facing(&a);
    assert_eq!(mate(&a, &b, 0.001, Resize::Strict), Ok(()));
    let mut x = b.clone(); x.domain = Domain::Pipe;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Domain));
    let mut x = b.clone(); x.system = System::ReturnAir;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::System));
    x.system = System::Other;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Ok(()));
    assert_eq!(mate(&x, &a, 0.001, Resize::Strict), Ok(()));
    let mut x = b.clone(); x.flow_dir = FlowDir::Out;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::FlowDirection));
    x.flow_dir = FlowDir::Both;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Ok(()));
    let mut y = a.clone(); y.flow_dir = FlowDir::In; let mut z = b.clone(); z.flow_dir = FlowDir::In;
    assert_eq!(mate(&y, &z, 0.001, Resize::Strict), Err(MateError::FlowDirection));
    // position: just inside and just outside the tolerance
    let mut x = b.clone(); x.pos[0] += 0.0009;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Ok(()));
    x.pos[0] += 0.0002;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Position));
    let mut x = b.clone(); x.pos[2] -= 0.01;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Position));
    // facing: same direction, or sideways
    let mut x = b.clone(); x.dir = a.dir;
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Facing));
    x.dir = [0.0, 0.0, 1.0];
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Facing));
    // a tiny turn is still a refusal
    let (s, c) = (0.001f64.sin(), 0.001f64.cos());
    x.dir = [-(c * a.dir[0] - s * a.dir[1]), -(s * a.dir[0] + c * a.dir[1]), 0.0];
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Facing));
    // sizes
    let mut x = b.clone(); x.size = Size::Rect { width: 0.5, height: 0.2 };
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Size));
    assert_eq!(mate(&a, &x, 0.001, Resize::Allow), Ok(()));
    x.size = Size::Rect { width: 0.4, height: 0.25 };
    assert_eq!(mate(&a, &x, 0.001, Resize::Strict), Err(MateError::Size));
    x.size = Size::Round { diameter: 0.4 };
    assert_eq!(mate(&a, &x, 0.001, Resize::Allow), Err(MateError::ShapeKind));
    // round to round
    let p = conn(&i, "chw_in").world(&Pose { x: 0.0, y: 0.0, z: 0.0, angle: 0.0 });
    let mut q = facing(&p); q.size = Size::Round { diameter: 0.028 };
    assert_eq!(mate(&p, &q, 0.001, Resize::Strict), Err(MateError::Size));
    assert_eq!(mate(&p, &q, 0.001, Resize::Allow), Ok(()));
    // electrical
    let e = conn(&i, "power").world(&Pose { x: 0.0, y: 0.0, z: 0.0, angle: 0.0 });
    let mut f = facing(&e); f.flow_dir = FlowDir::Out;
    assert_eq!(mate(&e, &f, 0.001, Resize::Strict), Ok(()));
    f.size = Size::Electrical { voltage: 400.0, poles: 1 };
    assert_eq!(mate(&e, &f, 0.001, Resize::Allow), Err(MateError::Voltage));
    f.size = Size::Electrical { voltage: 230.0, poles: 3 };
    assert_eq!(mate(&e, &f, 0.001, Resize::Allow), Err(MateError::Poles));
}

#[test]
fn a_duct_stub_family_joins_the_fan_coil_after_placement() {
    // a straight duct stub whose inlet is at its origin facing -Y
    let stub = Family {
        name: "Duct stub".into(),
        values: vec![ValueDef::input("w", 0.4, 0.1, 1.0, Scope::Type), ValueDef::input("h", 0.2, 0.1, 1.0, Scope::Type)],
        connectors: vec![ConnectorDef::new("in", Domain::Duct, System::SupplyAir, FlowDir::In, ("0", "0", "0"), Dir::MinusY, ShapeDef::Rect("w".into(), "h".into()))],
        category: Category::DuctFitting,
        ..Default::default()
    };
    let fc = solved(&[]);
    let s = conn(&fc, "supply").world(&Pose { x: 20.0, y: 10.0, z: 2.8, angle: 90.0 });
    // fan coil turned 90 degrees: its +Y outlet now faces -X at x = 20 - 0.3
    assert!(close3(s.pos, [19.7, 10.0, 2.925]) && close3(s.dir, [-1.0, 0.0, 0.0]));
    let si = stub.compile().unwrap().solve(&Request::default()).unwrap();
    // the stub must face +X so its inlet (facing -Y in its own frame) faces +X after a turn of 270 degrees... check by trying angles
    let mut ok = 0;
    for a in [0.0, 90.0, 180.0, 270.0] {
        let t = si.connectors[0].world(&Pose { x: 19.7, y: 10.0, z: 2.925, angle: a });
        if mate(&s, &t, 1e-6, Resize::Strict).is_ok() { ok += 1; assert_eq!(a, 90.0); }
    }
    assert_eq!(ok, 1);
}

#[test]
fn solving_many_is_deterministic() {
    let c = fan_coil().compile().unwrap();
    let reqs: Vec<Request> = (0..200).map(|i| Request { overrides: vec![("length", 0.5 + i as f64 * 0.01)], ..Default::default() }).collect();
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
            Power | Data => vec![CableTray, Conduit, Electrical],
            Other => doms.to_vec(),
            _ => vec![Pipe],
        }
    };
    for s in [SupplyAir, ReturnAir, ExhaustAir, OutsideAir, ChilledWaterSupply, ChilledWaterReturn, HeatingWaterSupply, HeatingWaterReturn, CondenserWaterSupply, CondenserWaterReturn, DomesticCold, DomesticHot, Sanitary, Vent, StormWater, Condensate, FireProtection, Gas, Power, Data, Other] {
        for d in doms {
            assert_eq!(s.domain_ok(d), allowed(s).contains(&d), "{s:?} {d:?}");
        }
    }
    // every direction can be used by a compiled connector and comes back unchanged
    for d in [Dir::PlusX, Dir::MinusX, Dir::PlusY, Dir::MinusY, Dir::PlusZ, Dir::MinusZ] {
        let c = ConnectorDef::new("a", Domain::Duct, System::OutsideAir, FlowDir::In, ("0", "0", "0"), d, ShapeDef::Round("0.1".into()));
        assert_eq!(run(c).unwrap().connectors[0].dir, d.vec());
    }
}

fn stub() -> Family {
    Family {
        name: "Stub".into(),
        connectors: vec![ConnectorDef::new("p", Domain::Pipe, System::DomesticCold, FlowDir::Out, ("0.1", "0", "0.2"), Dir::PlusX, ShapeDef::Round("0.05".into())).flow("0.001")],
        plan: vec![PlanDef::line(("0", "0"), ("1", "0")), PlanDef::circle(("0.5", "0"), "0.1")],
        ..Default::default()
    }
}
fn parent(nd: NestedDef) -> Vec<Compiled> {
    let p = Family { name: "Parent".into(), nested: vec![nd], connectors: vec![duct("own")], ..Default::default() };
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
    assert_eq!(i.plan, vec![PlanShape::Line([1.0, 2.0, 2.0, 2.0]), PlanShape::Circle { c: [1.5, 2.0], r: 0.1 }]);
    // turned 90 degrees about the origin
    let lib = parent(NestedDef::new("n", "Stub", ("1", "2", "0.5")).rotate_about("90", "0", "0"));
    let i = lib[1].solve(&Request::default()).unwrap();
    let c = &i.connectors[1];
    assert!(close3(c.pos, [-2.0, 1.1, 0.7]), "{:?}", c.pos);
    assert!(close3(c.dir, [0.0, 1.0, 0.0]));
    assert_eq!(i.plan[0], PlanShape::Line([-2.0, 1.0, -2.0, 2.0]));
    match i.plan[1] { PlanShape::Circle { c, r } => { assert!(close(c[0], -2.0) && close(c[1], 1.5) && close(r, 0.1)); } _ => panic!() }
    // a turn about a point that is not the origin
    let lib = parent(NestedDef::new("n", "Stub", ("0", "0", "0")).rotate_about("180", "1", "1"));
    let i = lib[1].solve(&Request::default()).unwrap();
    assert!(close3(i.connectors[1].pos, [1.9, 2.0, 0.2]) && close3(i.connectors[1].dir, [-1.0, 0.0, 0.0]));
}

#[test]
fn repeated_and_hidden_nested_families() {
    let lib = parent(NestedDef::new("n", "Stub", ("_i", "0", "0")).repeat("3"));
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
    let mid = Family { name: "Mid".into(), nested: vec![NestedDef::new("s", "Stub", ("1", "0", "0"))], ..Default::default() };
    let top = Family { name: "Top".into(), nested: vec![NestedDef::new("m", "Mid", ("0", "5", "0")).rotate_about("90", "0", "0")], ..Default::default() };
    let lib = compile_library(&[stub(), mid, top]).unwrap();
    let i = lib[2].solve(&Request::default()).unwrap();
    assert_eq!(i.connectors.len(), 1);
    assert_eq!(i.connectors[0].name, "m/s/p");
    // stub connector (0.1,0,0.2) -> in Mid (1.1,0) -> in Top shifted (1.1,5) -> turned 90: (-5, 1.1)
    assert!(close3(i.connectors[0].pos, [-5.0, 1.1, 0.2]) && close3(i.connectors[0].dir, [0.0, 1.0, 0.0]));
}

#[test]
fn total_limits_with_nesting() {
    let three = Family {
        name: "Three".into(),
        connectors: (0..3).map(|i| duct(&format!("c{i}"))).collect(),
        plan: (0..5).map(|_| PlanDef::line(("0", "0"), ("1", "0"))).collect(),
        ..Default::default()
    };
    let big = |n: &str| compile_library(&[three.clone(), Family { name: "P".into(), nested: vec![NestedDef::new("n", "Three", ("0", "0", "0")).repeat(n)], ..Default::default() }]).unwrap();
    assert_eq!(big("666")[1].solve(&Request::default()).unwrap().connectors.len(), 1998);
    assert_eq!(big("667")[1].solve(&Request::default()).err(), Some(FamilyError::TooMany("connectors in one placement")));
    let p = Family { name: "Q".into(), nested: vec![NestedDef::new("n", "Five", ("0", "0", "0")).repeat("1000"), NestedDef::new("m", "Five", ("0", "0", "0"))], ..Default::default() };
    let five = Family { name: "Five".into(), plan: (0..5).map(|_| PlanDef::line(("0", "0"), ("1", "0"))).collect(), ..Default::default() };
    let lib = compile_library(&[five.clone(), p]).unwrap();
    assert_eq!(lib[1].solve(&Request::default()).err(), Some(FamilyError::TooMany("plan symbol lines in one placement")));
    let p = Family { name: "Q".into(), nested: vec![NestedDef::new("n", "Five", ("0", "0", "0")).repeat("1000")], ..Default::default() };
    let lib = compile_library(&[five, p]).unwrap();
    assert_eq!(lib[1].solve(&Request::default()).unwrap().plan.len(), 5000);
}

#[test]
fn nested_direction_turns_both_components_and_limit_is_exact() {
    let up = Family {
        name: "Up".into(),
        connectors: vec![ConnectorDef::new("q", Domain::Pipe, System::DomesticCold, FlowDir::In, ("0", "0", "0"), Dir::PlusY, ShapeDef::Round("0.05".into()))],
        ..Default::default()
    };
    let p = Family { name: "P".into(), nested: vec![NestedDef::new("u", "Up", ("0", "0", "0")).rotate_about("90", "0", "0")], ..Default::default() };
    let lib = compile_library(&[up, p]).unwrap();
    assert!(close3(lib[1].solve(&Request::default()).unwrap().connectors[0].dir, [-1.0, 0.0, 0.0]));
    let four = Family { name: "Four".into(), connectors: (0..4).map(|i| duct(&format!("c{i}"))).collect(), ..Default::default() };
    let with = |n: &str| compile_library(&[four.clone(), Family { name: "P".into(), nested: vec![NestedDef::new("n", "Four", ("0", "0", "0")).repeat(n)], ..Default::default() }]).unwrap();
    assert_eq!(with("500")[1].solve(&Request::default()).unwrap().connectors.len(), 2000);
    assert!(with("501")[1].solve(&Request::default()).is_err());
}

// ---------- connector upgrade: aim, up/roll, power factor ----------

fn rect(name: &str, d: Dir, w: &str, h: &str) -> ConnectorDef {
    ConnectorDef::new(name, Domain::Duct, System::SupplyAir, FlowDir::Out, ("0", "0", "0"), d, ShapeDef::Rect(w.into(), h.into()))
}
fn inn(mut c: Connector) -> Connector { c.flow_dir = FlowDir::In; c }
fn at_pose(c: &Connector, x: f64, ang: f64) -> Connector { c.world(&Pose { x, y: 0.0, z: 0.0, angle: ang }) }

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
    assert!(matches!(run(duct("a").aim("0", "91")), Err(FamilyError::Connector { .. })));
    assert!(matches!(run(duct("a").aim("0", "-90.5")), Err(FamilyError::Connector { .. })));
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
    let r = run(rect("r", Dir::PlusY, "0.4", "0.2").roll("90")).unwrap().connectors[0].up;
    assert!(close3(r, [1.0, 0.0, 0.0]));
    let r = run(rect("r", Dir::PlusY, "0.4", "0.2").roll("180")).unwrap().connectors[0].up;
    assert!(close3(r, [0.0, 0.0, -1.0]));
    // up is always square to dir and unit length, also for aimed connectors
    let i = run(rect("r", Dir::PlusX, "0.4", "0.2").aim("30", "50").roll("33")).unwrap();
    let c = &i.connectors[0];
    assert!(close(c.dir[0] * c.up[0] + c.dir[1] * c.up[1] + c.dir[2] * c.up[2], 0.0));
    assert!(close(c.up.iter().map(|v| v * v).sum::<f64>(), 1.0));
    // roll only for rectangular connectors; power factor only for electrical ones
    assert!(matches!(with_conn(duct("a").roll("10")).compile(), Err(FamilyError::Connector { .. })));
    assert!(matches!(with_conn(duct("a").power_factor("0.9")).compile(), Err(FamilyError::Connector { .. })));
    // placing the family turns up with it
    let w = at_pose(&run(rect("r", Dir::PlusY, "0.4", "0.2")).unwrap().connectors[0], 1.0, 90.0);
    assert!(close3(w.up, [0.0, 0.0, 1.0]) && close3(w.dir, [-1.0, 0.0, 0.0]));
}

#[test]
fn rectangular_mate_follows_orientation() {
    let a = run(rect("a", Dir::PlusY, "0.4", "0.2")).unwrap().connectors[0].clone();
    let face = |ang: f64| -> Connector { at_pose(&inn(run(rect("b", Dir::MinusY, "0.4", "0.2")).unwrap().connectors[0].clone()), 0.0, ang) };
    // the same way up
    assert_eq!(mate(&a, &face(0.0), 1e-6, Resize::Strict), Ok(()));
    // turned a quarter about its own axis: widths and heights are crossed, so equal sizes no longer fit
    let b90 = inn(run(rect("b", Dir::MinusY, "0.4", "0.2").roll("90")).unwrap().connectors[0].clone());
    assert_eq!(mate(&a, &b90, 1e-6, Resize::Strict), Err(MateError::Size));
    assert_eq!(mate(&a, &b90, 1e-6, Resize::Allow), Ok(()));
    // crossed but with swapped sizes it fits
    let sw = inn(run(rect("b", Dir::MinusY, "0.2", "0.4").roll("90")).unwrap().connectors[0].clone());
    assert_eq!(mate(&a, &sw, 1e-6, Resize::Strict), Ok(()));
    // upside down is parallel, so aligned
    let b180 = inn(run(rect("b", Dir::MinusY, "0.4", "0.2").roll("180")).unwrap().connectors[0].clone());
    assert_eq!(mate(&a, &b180, 1e-6, Resize::Strict), Ok(()));
    // a 45 degree roll cannot line up
    let b45 = inn(run(rect("b", Dir::MinusY, "0.4", "0.2").roll("45")).unwrap().connectors[0].clone());
    assert_eq!(mate(&a, &b45, 1e-6, Resize::Allow), Err(MateError::Orientation));
}

#[test]
fn power_factor_apparent_power_and_current() {
    let el = |v: &str, p: &str| ConnectorDef::new("e", Domain::Electrical, System::Power, FlowDir::In, ("0", "0", "0"), Dir::PlusX, ShapeDef::Electrical { voltage: v.into(), poles: p.into() }).flow("9000");
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
        assert!(matches!(run(el("230", "1").power_factor(bad)), Err(FamilyError::Connector { .. })), "{bad}");
    }
    assert!(run(el("230", "1").power_factor("1")).is_ok());
    assert_eq!(run(duct("a")).unwrap().connectors[0].current(), None);
}

#[test]
fn type_names_come_in_the_authors_order() {
    let all = library::all().unwrap();
    let door = all.iter().find(|c| c.name() == "Door, single flush").unwrap();
    assert_eq!(door.type_names(), vec!["Type A", "Type B"]);
    let fc = all.iter().find(|c| c.name() == "Fan coil unit, ducted").unwrap();
    assert!(fc.type_names().is_empty());
}
