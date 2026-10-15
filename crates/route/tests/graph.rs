use openbim_family::mep::{Connector, Domain, FlowDir, Size, System};
use openbim_place::penetrations::{Run, RunSize};
use openbim_route::graph::*;
use openbim_route::snaps::*;
use openbim_route::*;
use proptest::prelude::*;

fn round(d: f64) -> RunSize {
    RunSize::Round { diameter: d }
}
fn prefs() -> RoutingPrefs {
    RoutingPrefs {
        domain: Domain::Pipe,
        system: System::ChilledWaterSupply,
        label: "CHWS".into(),
        elbow_angles: AngleSet::List(vec![90.0, 45.0]),
        bend_radius_factor: 1.5,
        min_straight: 0.05,
        junction: Junction::Tee,
        tee_angles: AngleSet::List(vec![90.0, 45.0]),
        tee_half_factor: 1.0,
        branch_stub_factor: 1.0,
        pick_tolerance: 0.02,
        transition_angle_deg: 30.0,
        min_transition_len: 0.1,
        connector_stub: 0.3,
        margin: 0.0,
        catalogue: None,
    }
}
fn off() -> Snaps {
    Snaps::new(SnapSettings::off())
}
fn conn(pos: [f64; 3], dir: [f64; 3], d: f64) -> Connector {
    Connector { name: "c".into(), domain: Domain::Pipe, system: System::ChilledWaterSupply, flow_dir: FlowDir::Out, pos, dir, up: [0.0, 0.0, 1.0], size: Size::Round { diameter: d }, flow: 0.0, pressure_drop: 0.0, power_factor: 1.0 }
}
fn seg(i: usize) -> NodeRef {
    NodeRef::Segment(i)
}
fn fit(i: usize) -> NodeRef {
    NodeRef::Fitting(i)
}
fn pr(n: NodeRef, port: usize) -> PortRef {
    PortRef { node: n, port }
}

/// connector -> run -> elbow -> run, with the free end at the far side
fn l_network() -> Network {
    let c = conn([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], 0.05);
    let r = route_from_connector(&c, &[[3.0, 0.0, 1.0], [3.0, 2.0, 1.0]], round(0.05), &prefs(), &off()).unwrap();
    let mut n = Network::new(100);
    n.add_route(&r).unwrap();
    n.attach_equipment(7, &c, 0, End::Start).unwrap();
    n
}

#[test]
fn an_l_route_from_equipment_is_linked_end_to_end() {
    let n = l_network();
    // the connector leaves a 0.3 stub, then runs to the elbow at x=3: pieces = stub, run to elbow, run after
    assert_eq!(n.fittings.len(), n.segments.len() - 1);
    n.validate().unwrap();
    let e = fit(0);
    // each elbow leg meets one run end, the other ends are linked in order
    for k in 0..2 {
        assert!(n.link_of(pr(e, k)).is_some(), "leg {k}");
    }
    assert_eq!(n.link_of(pr(NodeRef::Equipment(0), 0)), Some(pr(seg(0), 0)));
    let open = n.open_ports();
    assert_eq!(open, vec![pr(seg(n.segments.len() - 1), 1)]);
    assert_eq!(n.components().len(), 1);
    let p = n.path(NodeRef::Equipment(0), seg(n.segments.len() - 1)).unwrap();
    assert_eq!(*p.first().unwrap(), NodeRef::Equipment(0));
    assert_eq!(*p.last().unwrap(), seg(n.segments.len() - 1));
    assert!(p.contains(&e));
    // element numbers count up from the base
    assert_eq!(n.segments[0].id, 100);
    assert_eq!(n.next_id(), 100 + (n.segments.len() + n.fittings.len()) as u64);
}

#[test]
fn a_size_change_links_runs_of_the_two_sizes_through_the_transition() {
    let r = route_path(&[[0.0, 0.0, 1.0], [2.0, 0.0, 1.0], [4.0, 0.0, 1.0]], &[round(0.1), round(0.05)], &prefs(), &off()).unwrap();
    let mut n = Network::new(1);
    n.add_route(&r).unwrap();
    n.validate().unwrap();
    assert_eq!((n.segments.len(), n.fittings.len()), (2, 1));
    let t = fit(0);
    let (a, b) = (n.link_of(pr(t, 0)).unwrap(), n.link_of(pr(t, 1)).unwrap());
    assert_eq!(a, pr(seg(0), 1));
    assert_eq!(b, pr(seg(1), 0));
    assert_eq!(n.port_size(a), Some(round(0.1)));
    assert_eq!(n.port_size(b), Some(round(0.05)));
    assert_eq!(n.open_ports().len(), 2);
}

fn main_route() -> Route {
    route_path(&[[0.0, 0.0, 1.0], [4.0, 0.0, 1.0]], &[round(0.1)], &prefs(), &off()).unwrap()
}

#[test]
fn a_tee_splits_the_run_and_keeps_what_was_joined_to_its_far_end() {
    let mut n = Network::new(1);
    n.add_route(&main_route()).unwrap();
    let far = conn([4.0, 0.0, 1.0], [-1.0, 0.0, 0.0], 0.1);
    n.attach_equipment(9, &far, 0, End::End).unwrap();
    let b = branch_from_run(&n.segments[0].run, &[[2.0, 0.0, 1.0], [2.0, 2.0, 1.0]], &[round(0.05)], &prefs(), &off()).unwrap();
    let added = n.add_branch(0, &b).unwrap();
    n.validate().unwrap();
    // main run + its second half + the branch run
    assert_eq!(n.segments.len(), 3);
    assert_eq!(added.fittings, vec![0]);
    // the equipment now hangs on the second half
    let half2 = added.segments[0];
    assert_eq!(n.link_of(pr(NodeRef::Equipment(0), 0)), Some(pr(seg(half2), 1)));
    // the tee's three legs are all linked: first half, second half, branch
    let legs: Vec<NodeRef> = (0..3).map(|k| n.link_of(pr(fit(0), k)).unwrap().node).collect();
    assert_eq!(legs[0], seg(0));
    assert_eq!(legs[1], seg(half2));
    assert!(matches!(legs[2], NodeRef::Segment(s) if s != 0 && s != half2));
    assert_eq!(n.components().len(), 1);
    // free ends: the main start and the branch end
    assert_eq!(n.open_ports().len(), 2);
}

#[test]
fn a_tap_keeps_the_run_whole_and_is_recorded_as_a_tapping() {
    let mut p = prefs();
    p.junction = Junction::Tap;
    let mut n = Network::new(1);
    n.add_route(&main_route()).unwrap();
    let b = branch_from_run(&n.segments[0].run, &[[1.5, 0.0, 1.0], [1.5, 2.0, 1.0]], &[round(0.05)], &p, &off()).unwrap();
    n.add_branch(0, &b).unwrap();
    n.validate().unwrap();
    assert_eq!(n.segments.len(), 2);
    assert_eq!(n.taps.len(), 1);
    assert!((n.taps[0].distance - 1.5).abs() < 1e-9);
    assert_eq!(n.taps[0].segment, 0);
    assert!(n.neighbours(seg(0)).contains(&fit(0)));
    assert_eq!(n.components().len(), 1);
    assert_eq!(n.path(seg(0), seg(1)).unwrap().len(), 3);
    // the main run's two ends stay free
    assert!(n.open_ports().contains(&pr(seg(0), 0)) && n.open_ports().contains(&pr(seg(0), 1)));
}

#[test]
fn a_tap_before_a_tee_follows_the_half_it_sits_on() {
    let mut p = prefs();
    p.junction = Junction::Tap;
    let mut n = Network::new(1);
    n.add_route(&main_route()).unwrap();
    let tap = branch_from_run(&n.segments[0].run, &[[3.0, 0.0, 1.0], [3.0, 2.0, 1.0]], &[round(0.05)], &p, &off()).unwrap();
    n.add_branch(0, &tap).unwrap();
    let tee = branch_from_run(&n.segments[0].run, &[[1.0, 0.0, 1.0], [1.0, 2.0, 1.0]], &[round(0.05)], &prefs(), &off()).unwrap();
    n.add_branch(0, &tee).unwrap();
    n.validate().unwrap();
    // the tap at x=3 now sits on the second half, at its own distance along that half
    let t = n.taps[0];
    assert_eq!(t.segment, 2);
    let half = &n.segments[2].run;
    assert!((t.distance - (3.0 - half.a[0])).abs() < 1e-9, "{t:?}");
}

#[test]
fn failures_leave_the_network_as_it_was() {
    let mut n = Network::new(1);
    n.add_route(&main_route()).unwrap();
    let before = n.clone();
    // a branch made from some other run
    let other = Run::round("x", Domain::Pipe, System::ChilledWaterSupply, [0.0, 5.0, 1.0], [4.0, 5.0, 1.0], 0.1);
    let b = branch_from_run(&other, &[[2.0, 5.0, 1.0], [2.0, 7.0, 1.0]], &[round(0.05)], &prefs(), &off()).unwrap();
    assert_eq!(n.add_branch(0, &b), Err(GraphError::MainMismatch));
    assert_eq!(n, before);
    // a connector that is not at the run end
    let c = conn([0.5, 0.0, 1.0], [1.0, 0.0, 0.0], 0.1);
    assert_eq!(n.attach_equipment(1, &c, 0, End::Start), Err(GraphError::ConnectorMismatch));
    // facing the wrong way
    let c = conn([0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], 0.1);
    assert_eq!(n.attach_equipment(1, &c, 0, End::Start), Err(GraphError::ConnectorMismatch));
    // wrong size / wrong system
    let c = conn([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], 0.2);
    assert!(matches!(n.attach_equipment(1, &c, 0, End::Start), Err(GraphError::SizeMismatch(..))));
    let mut c = conn([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], 0.1);
    c.system = System::DomesticCold;
    assert_eq!(n.attach_equipment(1, &c, 0, End::Start), Err(GraphError::ConnectorMismatch));
    assert_eq!(n, before);
    // a port can be used once
    let c = conn([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], 0.1);
    n.attach_equipment(1, &c, 0, End::Start).unwrap();
    assert!(matches!(n.attach_equipment(2, &c, 0, End::Start), Err(GraphError::PortTaken(_))));
    assert_eq!(n.equipment.len(), 1);
}

#[test]
fn links_are_checked_for_size_place_and_direction() {
    let mut n = Network::new(1);
    n.add_route(&main_route()).unwrap();
    let r2 = route_path(&[[4.0, 0.0, 1.0], [6.0, 0.0, 1.0]], &[round(0.05)], &prefs(), &off()).unwrap();
    n.add_route(&r2).unwrap();
    // meet in place but the sizes differ
    assert!(matches!(n.link(pr(seg(0), 1), pr(seg(1), 0)), Err(GraphError::SizeMismatch(..))));
    // same size, but apart
    let r3 = route_path(&[[9.0, 0.0, 1.0], [11.0, 0.0, 1.0]], &[round(0.1)], &prefs(), &off()).unwrap();
    n.add_route(&r3).unwrap();
    assert!(matches!(n.link(pr(seg(0), 1), pr(seg(2), 0)), Err(GraphError::NotMeeting(..))));
    assert!(matches!(n.link(pr(seg(0), 1), pr(seg(0), 0)), Err(GraphError::SelfLink(_))));
    assert!(matches!(n.link(pr(seg(0), 5), pr(seg(2), 0)), Err(GraphError::NoSuchPort(_))));
    // different system
    let mut p2 = prefs();
    p2.system = System::DomesticCold;
    let r4 = route_path(&[[4.0, 0.0, 1.0], [6.0, 0.0, 1.0]], &[round(0.1)], &p2, &off()).unwrap();
    n.add_route(&r4).unwrap();
    assert!(matches!(n.link(pr(seg(0), 1), pr(seg(3), 0)), Err(GraphError::SystemMismatch(..))));
}

#[test]
fn saved_form_reads_back_the_same_and_is_deterministic() {
    let mut n = l_network();
    let long = n.segments[1].run.clone();
    let mid = [(long.a[0] + long.b[0]) / 2.0, (long.a[1] + long.b[1]) / 2.0, 1.0];
    let b = branch_from_run(&long, &[mid, [mid[0] + 1.0, mid[1], 1.0]], &[round(0.05)], &{ let mut p = prefs(); p.junction = Junction::Tap; p }, &off()).unwrap();
    n.add_branch(1, &b).unwrap();
    let bytes = n.to_bytes();
    assert_eq!(bytes, n.to_bytes());
    let m = Network::from_bytes(&bytes).unwrap();
    assert_eq!(m, n);
    assert_eq!(m.to_bytes(), bytes);
}

#[test]
fn damaged_records_are_refused_not_trusted() {
    let n = l_network();
    let bytes = n.to_bytes();
    // every shorter prefix fails
    for k in 0..bytes.len() {
        assert!(Network::from_bytes(&bytes[..k]).is_err(), "prefix {k}");
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(Network::from_bytes(&extra).is_err());
    let mut bad = bytes.clone();
    bad[0] = b'X';
    assert!(Network::from_bytes(&bad).is_err());
    bad = bytes.clone();
    bad[4] = 9;
    assert!(Network::from_bytes(&bad).is_err());
    // flipping any single byte must not panic; when it still reads, the result must validate
    for k in 0..bytes.len() {
        for flip in [0x01u8, 0x80, 0xff] {
            let mut b = bytes.clone();
            b[k] ^= flip;
            if let Ok(m) = Network::from_bytes(&b) {
                m.validate().unwrap();
            }
        }
    }
    // a link that points at a port that does not exist cannot be added
    let mut m = n.clone();
    assert!(m.link(pr(seg(0), 0), pr(seg(1), 7)).is_err());
}

fn walk(steps: &[(u8, f64)]) -> Vec<[f64; 3]> {
    let dirs = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, -1.0, 0.0]];
    let mut p = vec![[0.0, 0.0, 1.0]];
    for (d, l) in steps {
        let last = *p.last().unwrap();
        let v = dirs[(*d % 4) as usize];
        p.push([last[0] + v[0] * l, last[1] + v[1] * l, last[2]]);
    }
    p
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]
    #[test]
    fn any_routed_path_becomes_one_valid_chain(steps in prop::collection::vec((0u8..4, 1.0f64..6.0), 1..9)) {
        let pts = walk(&steps);
        let sizes = vec![round(0.1); pts.len() - 1];
        let Ok(r) = route_path(&pts, &sizes, &prefs(), &Snaps::new(SnapSettings { plan: SnapMode::Increment(90.0), vertical: SnapMode::Off })) else { return Ok(()); };
        let mut n = Network::new(1);
        n.add_route(&r).unwrap();
        n.validate().unwrap();
        prop_assert_eq!(n.links().len(), 2 * n.fittings.len());
        prop_assert_eq!(n.components().len(), 1);
        prop_assert_eq!(n.open_ports().len(), 2);
        prop_assert_eq!(Network::from_bytes(&n.to_bytes()).unwrap(), n.clone());
        // the path from one end to the other visits every piece exactly once
        let (first, last) = (seg(0), seg(n.segments.len() - 1));
        prop_assert_eq!(n.path(first, last).unwrap().len(), n.segments.len() + n.fittings.len());
    }
}

#[test]
fn two_ports_at_one_place_must_face_each_other() {
    let mut n = Network::new(1);
    n.add_route(&main_route()).unwrap();
    // a run that starts at the main run's end but comes back along it: both ends point the same way
    let back = Run::round("b", Domain::Pipe, System::ChilledWaterSupply, [4.0, 0.0, 1.0], [2.0, 0.0, 1.0], 0.1);
    n.segments.push(NetSegment { id: 50, run: back });
    assert!(matches!(n.link(pr(seg(0), 1), pr(seg(1), 0)), Err(GraphError::NotMeeting(..))));
}

#[test]
fn a_record_with_a_run_of_no_length_is_refused() {
    let mut n = Network::new(1);
    n.segments.push(NetSegment { id: 1, run: Run::round("z", Domain::Pipe, System::ChilledWaterSupply, [1.0, 1.0, 1.0], [1.0, 1.0, 1.0], 0.1) });
    assert!(matches!(Network::from_bytes(&n.to_bytes()), Err(GraphError::Corrupt(_))));
}
