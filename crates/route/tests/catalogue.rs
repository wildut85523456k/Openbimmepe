use openbim_family::mep::{Domain, System};
use openbim_place::penetrations::RunSize;
use openbim_route::catalogue::*;
use openbim_route::snaps::*;
use openbim_route::*;

fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())) }
fn round(d: f64) -> RunSize { RunSize::Round { diameter: d } }
fn off() -> Snaps { Snaps::new(SnapSettings::off()) }
fn list(v: &[f64]) -> AngleSet { AngleSet::List(v.to_vec()) }
use FittingClass::*;

fn types() -> Vec<FittingType> {
    let p = Domain::Pipe;
    vec![
        FittingType::new("Elbow S", Elbow, p).code("E-S").sizes(0.0, 0.05).angles(list(&[90.0, 45.0])).radius(1.0),
        FittingType::new("Elbow L", Elbow, p).code("E-L").sizes(0.05, 0.2).angles(list(&[90.0, 45.0, 22.5])).radius(1.5),
        FittingType::new("Elbow XL", Elbow, p).code("E-XL").sizes(0.2, 0.5).angles(list(&[90.0])).radius(2.0),
        FittingType::new("Elbow return", Elbow, p).system(System::ChilledWaterReturn).sizes(0.0, 1.0).angles(AngleSet::Any).radius(3.0),
        FittingType::new("Tee", Tee, p).code("T").sizes(0.03, 0.2).branch_sizes(0.02, 0.1).angles(list(&[90.0])).tee_lengths(1.0, 1.0),
        FittingType::new("Tee XL", Tee, p).code("T-XL").sizes(0.2, 0.5).branch_sizes(0.02, 0.5).angles(list(&[90.0, 45.0])).tee_lengths(1.5, 2.0),
        FittingType::new("Reducer", Transition, p).code("R").sizes(0.02, 0.3).transition(20.0, 0.15),
        FittingType::new("Reducer XL", Transition, p).code("R-XL").sizes(0.3, 0.5).transition(15.0, 0.2),
    ]
}
fn prefs() -> RoutingPrefs {
    RoutingPrefs {
        domain: Domain::Pipe,
        system: System::ChilledWaterSupply,
        label: "CHWS".into(),
        elbow_angles: AngleSet::Any,
        bend_radius_factor: 99.0, // not used when there is a catalogue
        min_straight: 0.0,
        junction: Junction::Tee,
        tee_angles: AngleSet::Any,
        tee_half_factor: 99.0,
        branch_stub_factor: 99.0,
        pick_tolerance: 0.02,
        transition_angle_deg: 99.0,
        min_transition_len: 99.0,
        connector_stub: 0.5,
        margin: 0.0,
        catalogue: Some(Catalogue::new(types())),
    }
}
fn bend(d: f64, theta_deg: f64) -> Result<Route, RouteError> {
    let t = theta_deg.to_radians();
    let pts = [[0.0, 0.0, 0.0], [5.0, 0.0, 0.0], [5.0 + 3.0 * t.cos(), 3.0 * t.sin(), 0.0]];
    route_path(&pts, &[round(d), round(d)], &prefs(), &off())
}

#[test]
fn the_elbow_is_picked_by_size_and_its_radius_sets_the_trim() {
    // 0.04: Elbow S, radius 1.0 x 0.04, a right angle trims by the radius
    let r = bend(0.04, 90.0).unwrap();
    assert_eq!((r.fittings[0].type_name.as_str(), r.fittings[0].part_code.as_str()), ("Elbow S", "E-S"));
    assert!(close(r.fittings[0].radius, 0.04) && close(r.runs[0].b[0], 5.0 - 0.04));
    // 0.05 is in both S and L: the first in the list wins
    assert_eq!(bend(0.05, 90.0).unwrap().fittings[0].type_name, "Elbow S");
    // 0.06: Elbow L, radius 1.5 x 0.06
    let r = bend(0.06, 90.0).unwrap();
    assert_eq!(r.fittings[0].type_name, "Elbow L");
    assert!(close(r.fittings[0].radius, 0.09) && close(r.runs[0].b[0], 5.0 - 0.09));
    // 0.3: Elbow XL, radius 2.0 x 0.3
    let r = bend(0.3, 90.0).unwrap();
    assert_eq!((r.fittings[0].type_name.as_str(), r.fittings[0].part_code.as_str()), ("Elbow XL", "E-XL"));
    assert!(close(r.fittings[0].radius, 0.6));
    // a 45 degree bend uses the half-angle tangent of that radius
    let r = bend(0.1, 45.0).unwrap();
    assert!(close(r.fittings[0].trim[0], 0.15 * 22.5f64.to_radians().tan()));
}

#[test]
fn angles_exist_only_where_the_catalogue_has_them_for_that_size() {
    // 22.5 exists for 0.05 to 0.2 only
    assert!(bend(0.1, 22.5).is_ok());
    assert!(matches!(bend(0.04, 22.5), Err(RouteError::BendNotAvailable { vertex: 1, .. })));
    assert!(matches!(bend(0.3, 45.0), Err(RouteError::BendNotAvailable { .. })));
    // 0.6 is beyond every elbow: no fitting at all
    assert_eq!(bend(0.6, 90.0).err(), Some(RouteError::NoFitting { class: Elbow, vertex: 1 }));
    // the size ranges are inclusive at both ends
    assert!(bend(0.2, 90.0).is_ok() && bend(0.5, 90.0).is_ok());
    assert!(bend(0.5 + 1e-6, 90.0).is_err());
}

#[test]
fn a_type_for_one_system_is_used_only_for_that_system() {
    let mut p = prefs();
    p.system = System::ChilledWaterReturn;
    let pts = [[0.0, 0.0, 0.0], [5.0, 0.0, 0.0], [5.0, 3.0, 0.0]];
    // the return elbow is first in the list that fits (it is listed after S/L/XL, so for 0.1 the L elbow still wins)
    let r = route_path(&pts, &[round(0.1), round(0.1)], &p, &off()).unwrap();
    assert_eq!(r.fittings[0].type_name, "Elbow L");
    // 60 degrees only exists for the return system (any angle), not for the supply
    let pts60 = [[0.0, 0.0, 0.0], [5.0, 0.0, 0.0], [6.5, 2.598076211353316, 0.0]];
    let r = route_path(&pts60, &[round(0.1), round(0.1)], &p, &off()).unwrap();
    assert_eq!(r.fittings[0].type_name, "Elbow return");
    assert!(close(r.fittings[0].radius, 0.3));
    assert!(matches!(route_path(&pts60, &[round(0.1), round(0.1)], &prefs(), &off()), Err(RouteError::BendNotAvailable { .. })));
    // a type for no particular system serves any system of its domain
    let mut q = prefs();
    q.system = System::DomesticCold;
    assert!(route_path(&[[0.0, 0.0, 0.0], [5.0, 0.0, 0.0], [5.0, 3.0, 0.0]], &[round(0.1), round(0.1)], &q, &off()).is_ok());
}

#[test]
fn the_first_matching_type_wins_so_the_order_is_the_authors_preference() {
    let a = FittingType::new("A", Elbow, Domain::Pipe).radius(1.0);
    let b = FittingType::new("B", Elbow, Domain::Pipe).radius(2.0);
    let mut p = prefs();
    p.catalogue = Some(Catalogue::new(vec![a.clone(), b.clone()]));
    let pts = [[0.0, 0.0, 0.0], [5.0, 0.0, 0.0], [5.0, 3.0, 0.0]];
    assert_eq!(route_path(&pts, &[round(0.1), round(0.1)], &p, &off()).unwrap().fittings[0].type_name, "A");
    p.catalogue = Some(Catalogue::new(vec![b, a]));
    let r = route_path(&pts, &[round(0.1), round(0.1)], &p, &off()).unwrap();
    assert_eq!(r.fittings[0].type_name, "B");
    assert!(close(r.fittings[0].radius, 0.2));
}

#[test]
fn shapes_and_domains_are_matched() {
    let rect = RunSize::Rect { width: 0.4, height: 0.2, roll: 0.0 };
    let cat = Catalogue::new(vec![
        FittingType::new("Round elbow", Elbow, Domain::Duct).shape(ShapeFilter::Round).radius(1.0),
        FittingType::new("Rect elbow", Elbow, Domain::Duct).shape(ShapeFilter::Rect).radius(1.5),
    ]);
    assert_eq!(cat.pick_elbow(Domain::Duct, System::SupplyAir, round(0.3), 90.0).unwrap().name, "Round elbow");
    let e = cat.pick_elbow(Domain::Duct, System::SupplyAir, rect, 90.0).unwrap();
    assert_eq!(e.name, "Rect elbow");
    // the larger dimension (0.4) is what is compared with the range
    let cat2 = Catalogue::new(vec![FittingType::new("Small rect", Elbow, Domain::Duct).shape(ShapeFilter::Rect).sizes(0.0, 0.3)]);
    assert_eq!(cat2.pick_elbow(Domain::Duct, System::SupplyAir, rect, 90.0).err(), Some(PickError::NoType));
    // a pipe catalogue is no use for ducts
    assert_eq!(Catalogue::new(types()).pick_elbow(Domain::Duct, System::SupplyAir, round(0.1), 90.0).err(), Some(PickError::NoType));
}

#[test]
fn the_transition_is_picked_by_size_and_sets_its_length() {
    let pts = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [6.0, 0.0, 0.0]];
    let r = route_path(&pts, &[round(0.1), round(0.2)], &prefs(), &off()).unwrap();
    // Reducer: 20 degrees included, delta 0.1: L = 0.1 / (2 tan 10) = 0.2836, longer than the 0.15 minimum
    let l = 0.1 / (2.0 * 10f64.to_radians().tan());
    assert_eq!((r.fittings[0].type_name.as_str(), r.fittings[0].part_code.as_str()), ("Reducer", "R"));
    assert!(close(r.runs[1].a[0], 2.0 + l));
    // a small change takes the type's minimum length
    let r = route_path(&pts, &[round(0.1), round(0.11)], &prefs(), &off()).unwrap();
    assert!(close(r.runs[1].a[0], 2.15));
    // large sizes use the large reducer
    let r = route_path(&pts, &[round(0.35), round(0.45)], &prefs(), &off()).unwrap();
    assert_eq!(r.fittings[0].type_name, "Reducer XL");
    assert!(close(r.runs[1].a[0], 2.0 + (0.1 / (2.0 * 7.5f64.to_radians().tan())).max(0.2)));
    // one end in each range: no single reducer covers both
    assert_eq!(route_path(&pts, &[round(0.25), round(0.35)], &prefs(), &off()).err(), Some(RouteError::NoFitting { class: Transition, vertex: 1 }));
    // nothing for 0.01
    assert_eq!(route_path(&pts, &[round(0.01), round(0.02)], &prefs(), &off()).err(), Some(RouteError::NoFitting { class: Transition, vertex: 1 }));
}

fn main_run(d: f64) -> openbim_place::penetrations::Run {
    openbim_place::penetrations::Run::round("main", Domain::Pipe, System::ChilledWaterSupply, [0.0, 0.0, 1.0], [6.0, 0.0, 1.0], d)
}

#[test]
fn tees_are_picked_by_run_size_branch_size_and_angle() {
    let go = |m: f64, br: f64, pts: [[f64; 3]; 2]| branch_from_run(&main_run(m), &pts, &[round(br)], &prefs(), &off());
    let drop = [[3.0, 0.0, 1.0], [3.0, 2.0, 1.0]];
    let b = go(0.1, 0.05, drop).unwrap();
    assert_eq!((b.fitting.type_name.as_str(), b.fitting.part_code.as_str()), ("Tee", "T"));
    assert!(close(b.main[0].b[0], 3.0 - 0.1) && close(b.route.runs[0].a[1], 0.1));
    let b = go(0.3, 0.05, drop).unwrap();
    assert_eq!(b.fitting.type_name, "Tee XL");
    assert!(close(b.main[0].b[0], 3.0 - 0.45) && close(b.route.runs[0].a[1], 0.6));
    // a branch too big for the small tee, and no tee for the run size
    assert_eq!(go(0.1, 0.15, drop).err(), Some(RouteError::NoBranchSize));
    assert_eq!(go(0.6, 0.05, drop).err(), Some(RouteError::NoFitting { class: Tee, vertex: 0 }));
    // 45 degrees: only the large tee has it
    let diag = [[3.0, 0.0, 1.0], [4.0, 1.0, 1.0]];
    assert!(matches!(go(0.1, 0.05, diag), Err(RouteError::TeeAngleNotAvailable { .. })));
    assert!(go(0.3, 0.05, diag).is_ok());
    // a tap picks from the taps: there are none in this catalogue
    let mut p = prefs();
    p.junction = Junction::Tap;
    assert_eq!(branch_from_run(&main_run(0.1), &drop, &[round(0.05)], &p, &off()).err(), Some(RouteError::NoFitting { class: Tap, vertex: 0 }));
    let mut c = types();
    c.push(FittingType::new("Saddle", Tap, Domain::Pipe).code("S").tee_lengths(0.0, 0.5));
    p.catalogue = Some(Catalogue::new(c));
    let b = branch_from_run(&main_run(0.1), &drop, &[round(0.05)], &p, &off()).unwrap();
    assert_eq!((b.fitting.type_name.as_str(), b.main.len()), ("Saddle", 1));
    assert!(close(b.route.runs[0].a[1], 0.05));
}

#[test]
fn a_route_from_a_connector_uses_the_catalogue_for_the_transition() {
    use openbim_family::mep::{Connector, FlowDir, Size};
    let c = Connector { name: "c".into(), domain: Domain::Pipe, system: System::ChilledWaterSupply, flow_dir: FlowDir::Out, pos: [0.0, 0.0, 1.0], dir: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], size: Size::Round { diameter: 0.05 }, flow: 0.0, pressure_drop: 0.0, power_factor: 1.0 };
    let r = route_from_connector(&c, &[[4.0, 0.0, 1.0]], round(0.1), &prefs(), &off()).unwrap();
    // stub 0.5; Reducer: delta 0.05 at 20 degrees = 0.1418 < minimum 0.15
    assert_eq!(r.fittings[0].type_name, "Reducer");
    assert!(close(r.runs[1].a[0], 0.5 + 0.15));
}

#[test]
fn the_plain_fields_and_an_equal_catalogue_route_identically() {
    let mut plain = prefs();
    plain.catalogue = None;
    plain.elbow_angles = list(&[90.0, 45.0]);
    plain.bend_radius_factor = 1.5;
    plain.tee_angles = list(&[90.0]);
    plain.tee_half_factor = 1.0;
    plain.branch_stub_factor = 1.0;
    plain.transition_angle_deg = 30.0;
    plain.min_transition_len = 0.1;
    let mut cat = plain.clone();
    cat.catalogue = Some(Catalogue::new(vec![
        FittingType::new("Elbow", Elbow, Domain::Pipe).angles(list(&[90.0, 45.0])).radius(1.5),
        FittingType::new("Tee", Tee, Domain::Pipe).angles(list(&[90.0])).tee_lengths(1.0, 1.0),
        FittingType::new("Tap", Tap, Domain::Pipe).angles(list(&[90.0])).tee_lengths(1.0, 1.0),
        FittingType::new("Transition", Transition, Domain::Pipe).transition(30.0, 0.1),
    ]));
    let pts = [[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [3.0, 3.0, 0.0], [6.0, 6.0, 0.0], [9.0, 6.0, 0.0]];
    let sz = [round(0.1), round(0.1), round(0.1), round(0.2)];
    assert_eq!(route_path(&pts, &sz, &plain, &off()), route_path(&pts, &sz, &cat, &off()));
    let d = [[3.0, 0.0, 1.0], [3.0, 2.0, 1.0]];
    assert_eq!(branch_from_run(&main_run(0.1), &d, &[round(0.05)], &plain, &off()), branch_from_run(&main_run(0.1), &d, &[round(0.05)], &cat, &off()));
}

#[test]
fn a_catalogue_that_is_not_valid_is_refused() {
    let ok = || FittingType::new("X", Elbow, Domain::Pipe);
    let bad = |f: &dyn Fn(&mut FittingType)| { let mut t = ok(); f(&mut t); Catalogue::new(vec![t]).check() };
    assert_eq!(Catalogue::new(vec![ok()]).check(), Ok(()));
    assert_eq!(bad(&|t| t.name = String::new()), Err(CatalogueError::BadName));
    assert_eq!(bad(&|t| t.name = "x".repeat(65)), Err(CatalogueError::BadName));
    assert_eq!(bad(&|t| t.name = "a\nb".into()), Err(CatalogueError::BadName));
    assert_eq!(bad(&|t| t.part_code = "x".repeat(65)), Err(CatalogueError::BadName));
    assert_eq!(bad(&|t| t.part_code = "\t".into()), Err(CatalogueError::BadName));
    assert_eq!(bad(&|t| t.name = "x".repeat(64)), Ok(()));
    assert_eq!(bad(&|t| { t.size_min = 0.2; t.size_max = 0.1; }), Err(CatalogueError::BadRange));
    assert_eq!(bad(&|t| t.size_min = -0.1), Err(CatalogueError::BadRange));
    assert_eq!(bad(&|t| t.size_max = f64::NAN), Err(CatalogueError::BadRange));
    assert_eq!(bad(&|t| t.size_max = 1000.1), Err(CatalogueError::BadRange));
    assert_eq!(bad(&|t| { t.branch_min = 0.5; t.branch_max = 0.1; }), Err(CatalogueError::BadRange));
    assert_eq!(bad(&|t| t.radius_factor = 0.0), Err(CatalogueError::BadDimension));
    assert_eq!(bad(&|t| t.half_factor = -1.0), Err(CatalogueError::BadDimension));
    assert_eq!(bad(&|t| t.stub_factor = f64::INFINITY), Err(CatalogueError::BadDimension));
    assert_eq!(bad(&|t| t.min_len = -0.1), Err(CatalogueError::BadDimension));
    assert_eq!(bad(&|t| t.transition_angle_deg = 0.0), Err(CatalogueError::BadDimension));
    assert_eq!(bad(&|t| t.transition_angle_deg = 180.0), Err(CatalogueError::BadDimension));
    assert_eq!(bad(&|t| t.angles = AngleSet::List(vec![])), Err(CatalogueError::BadAngles));
    assert_eq!(bad(&|t| t.angles = AngleSet::List(vec![0.0])), Err(CatalogueError::BadAngles));
    assert_eq!(bad(&|t| t.angles = AngleSet::List(vec![180.0])), Err(CatalogueError::BadAngles));
    assert_eq!(bad(&|t| t.angles = AngleSet::List(vec![45.0; 65])), Err(CatalogueError::BadAngles));
    assert_eq!(Catalogue::new(vec![ok(), ok()]).check(), Err(CatalogueError::DuplicateName));
    let many: Vec<FittingType> = (0..=MAX_TYPES).map(|i| FittingType::new(&format!("t{i}"), Elbow, Domain::Pipe)).collect();
    assert_eq!(Catalogue::new(many).check(), Err(CatalogueError::TooMany));
    let max: Vec<FittingType> = (0..MAX_TYPES).map(|i| FittingType::new(&format!("t{i}"), Elbow, Domain::Pipe)).collect();
    assert_eq!(Catalogue::new(max).check(), Ok(()));
    // and the router refuses to use one
    let mut p = prefs();
    p.catalogue = Some(Catalogue::new(vec![ok(), ok()]));
    assert_eq!(route_path(&[[0.0; 3], [1.0, 0.0, 0.0]], &[round(0.1)], &p, &off()).err(), Some(RouteError::BadCatalogue(CatalogueError::DuplicateName)));
    // an empty catalogue is valid, and then no fitting exists
    p.catalogue = Some(Catalogue::default());
    assert!(route_path(&[[0.0; 3], [1.0, 0.0, 0.0]], &[round(0.1)], &p, &off()).is_ok());
    assert_eq!(route_path(&[[0.0; 3], [5.0, 0.0, 0.0], [5.0, 3.0, 0.0]], &[round(0.1), round(0.1)], &p, &off()).err(), Some(RouteError::NoFitting { class: Elbow, vertex: 1 }));
}

#[test]
fn size_ranges_include_both_ends_exactly() {
    let cat = Catalogue::new(vec![FittingType::new("Only", Elbow, Domain::Pipe).sizes(0.1, 0.2)]);
    let pick = |d: f64| cat.pick_elbow(Domain::Pipe, System::ChilledWaterSupply, round(d), 90.0).map(|t| t.name.clone());
    assert_eq!(pick(0.1), Ok("Only".to_string()));
    assert_eq!(pick(0.2), Ok("Only".to_string()));
    assert_eq!(pick(0.15), Ok("Only".to_string()));
    assert_eq!(pick(0.0999), Err(PickError::NoType));
    assert_eq!(pick(0.2001), Err(PickError::NoType));
    // branch ranges too
    let t = Catalogue::new(vec![FittingType::new("T", Tee, Domain::Pipe).branch_sizes(0.05, 0.1)]);
    let pj = |b: f64| t.pick_junction(Tee, Domain::Pipe, System::ChilledWaterSupply, round(0.2), round(b), 90.0).map(|x| x.name.clone());
    assert_eq!(pj(0.05), Ok("T".to_string()));
    assert_eq!(pj(0.1), Ok("T".to_string()));
    assert_eq!(pj(0.0499), Err(PickError::NoBranchSize));
    assert_eq!(pj(0.1001), Err(PickError::NoBranchSize));
}
