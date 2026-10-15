use openbim_family::mep::{Connector, Domain, FlowDir, Size, System};
use openbim_place::penetrations::RunSize;
use openbim_route::auto::*;
use openbim_route::graph::*;
use openbim_route::reroute::*;
use openbim_route::snaps::*;
use openbim_route::*;
use proptest::prelude::*;

fn round(d: f64) -> RunSize {
    RunSize::Round { diameter: d }
}
fn prefs(junction: Junction) -> RoutingPrefs {
    RoutingPrefs {
        domain: Domain::Pipe,
        system: System::ChilledWaterSupply,
        label: "CHWS".into(),
        elbow_angles: AngleSet::List(vec![90.0, 45.0]),
        bend_radius_factor: 1.5,
        min_straight: 0.05,
        junction,
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
fn conn(name: &str, pos: [f64; 3], dir: [f64; 3]) -> Connector {
    Connector { name: name.into(), domain: Domain::Pipe, system: System::ChilledWaterSupply, flow_dir: FlowDir::Out, pos, dir, up: [0.0, 0.0, 1.0], size: Size::Round { diameter: 0.05 }, flow: 0.0, pressure_drop: 0.0, power_factor: 1.0 }
}
const PX: [f64; 3] = [1.0, 0.0, 0.0];
const NX: [f64; 3] = [-1.0, 0.0, 0.0];

/// a main along x=6 and a unit at (2,0,1) joined to it by a branch from a tee at (6,0,1)
fn tee_network() -> Network {
    let p = prefs(Junction::Tee);
    let mut n = Network::new(100);
    let main = route_path(&[[6.0, -3.0, 1.0], [6.0, 3.0, 1.0]], &[round(0.05)], &p, &off()).unwrap();
    let a = n.add_route(&main).unwrap();
    let br = branch_from_run(&n.segments[a.segments[0]].run, &[[6.0, 0.0, 1.0], [2.0, 0.0, 1.0]], &[round(0.05)], &p, &off()).unwrap();
    let added = n.add_branch(a.segments[0], &br).unwrap();
    let last = *added.segments.last().unwrap();
    n.attach_equipment(1, &conn("c", [2.0, 0.0, 1.0], PX), last, End::End).unwrap();
    n.validate().unwrap();
    n
}

fn request<'a>(equipment: usize, c: &'a Connector, p: &'a RoutingPrefs, zones: &'a [Zone], o: &'a AutoOptions) -> RerouteRequest<'a> {
    RerouteRequest { equipment, connector: c, prefs: p, zones, from_own: &[], to_own: &[], opts: o, avoid_network: true }
}
fn ids(n: &Network) -> Vec<u64> {
    let mut v: Vec<u64> = n.segments.iter().map(|s| s.id).chain(n.fittings.iter().map(|f| f.id)).collect();
    v.sort();
    v
}
fn close3(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-6)
}

#[test]
fn a_moved_unit_is_joined_to_the_tee_again_and_the_rest_is_untouched() {
    let mut n = tee_network();
    let before = n.clone();
    let open_before = n.open_ports().len();
    let p = prefs(Junction::Tee);
    let o = AutoOptions::default();
    let moved = conn("c", [1.0, 3.0, 1.0], PX);
    let r = reroute_equipment(&mut n, &request(0, &moved, &p, &[], &o)).unwrap();
    n.validate().unwrap();
    assert_eq!(n.components().len(), 1);
    assert_eq!(n.open_ports().len(), open_before, "the two main ends are still the only open ports");
    assert_eq!(n.equipment[0].pos, moved.pos);
    assert_eq!(n.equipment[0].dir, moved.dir);
    assert_eq!(n.equipment[0].size, round(0.05));
    // the route starts at the new connector and ends in the tee's leg
    assert!(close3(r.route.route.runs[0].a, moved.pos));
    let leg = before.port_pos(before.chain_from_equipment(0).unwrap().end_port().unwrap()).unwrap();
    assert!(close3(r.route.route.runs.last().unwrap().b, leg));
    // everything off the chain is exactly as it was, by element number
    let chain = before.chain_from_equipment(0).unwrap();
    for (i, s) in before.segments.iter().enumerate() {
        if !chain.nodes.contains(&NodeRef::Segment(i)) {
            assert!(n.segments.iter().any(|t| t.id == s.id && t.run == s.run), "segment {}", s.id);
        }
    }
    for (i, f) in before.fittings.iter().enumerate() {
        if !chain.nodes.contains(&NodeRef::Fitting(i)) {
            assert!(n.fittings.iter().any(|t| t.id == f.id && t.fitting == f.fitting), "fitting {}", f.id);
        }
    }
    // element numbers are unique, the pipe kept its numbers, and the retired ones are gone
    let mut all = ids(&n);
    let len = all.len();
    all.dedup();
    assert_eq!(all.len(), len);
    let old_chain_ids: Vec<u64> = chain.nodes.iter().map(|x| match x {
        NodeRef::Segment(i) => before.segments[*i].id,
        NodeRef::Fitting(i) => before.fittings[*i].id,
        NodeRef::Equipment(_) => unreachable!(),
    }).collect();
    assert_eq!(r.kept + r.retired.len(), old_chain_ids.len());
    for id in &r.retired {
        assert!(!all.contains(id));
    }
    assert!(r.kept >= 1);
}

#[test]
fn rerouting_twice_gives_the_same_network() {
    let mut n = tee_network();
    let p = prefs(Junction::Tee);
    let o = AutoOptions::default();
    let moved = conn("c", [1.0, 3.0, 1.0], PX);
    reroute_equipment(&mut n, &request(0, &moved, &p, &[], &o)).unwrap();
    let once = n.clone();
    reroute_equipment(&mut n, &request(0, &moved, &p, &[], &o)).unwrap();
    assert_eq!(n, once);
}

#[test]
fn a_failed_reroute_leaves_the_network_as_it_was() {
    let mut n = tee_network();
    let before = n.clone();
    let p = prefs(Junction::Tee);
    let o = AutoOptions::default();
    let moved = conn("c", [1.0, 3.0, 1.0], PX);
    // a wall of obstacle between the new place and the tee that fills the whole search volume
    let wall = [Zone::hard("everything", [2.0, -20.0, -20.0], [4.0, 20.0, 20.0]).unwrap()];
    let e = reroute_equipment(&mut n, &request(0, &moved, &p, &wall, &o));
    assert!(matches!(e, Err(RerouteError::Auto(AutoError::NoPath))), "{e:?}");
    assert_eq!(n, before);
    // a connector of another system
    let mut duct = moved.clone();
    duct.domain = Domain::Duct;
    assert!(reroute_equipment(&mut n, &request(0, &duct, &p, &[], &o)).is_err());
    assert_eq!(n, before);
    // equipment that is not there
    assert_eq!(reroute_equipment(&mut n, &request(5, &moved, &p, &[], &o)), Err(RerouteError::Graph(GraphError::NoSuchEquipment(5))));
    assert_eq!(n, before);
}

#[test]
fn a_pipe_that_ends_in_the_open_cannot_be_rerouted() {
    let p = prefs(Junction::Tee);
    let c = conn("c", [0.0, 0.0, 1.0], PX);
    let r = route_from_connector(&c, &[[3.0, 0.0, 1.0], [3.0, 2.0, 1.0]], round(0.05), &p, &off()).unwrap();
    let mut n = Network::new(1);
    n.add_route(&r).unwrap();
    n.attach_equipment(1, &c, 0, End::Start).unwrap();
    let before = n.clone();
    let o = AutoOptions::default();
    let moved = conn("c", [0.0, 1.0, 1.0], PX);
    assert_eq!(reroute_equipment(&mut n, &request(0, &moved, &p, &[], &o)), Err(RerouteError::OpenEnd));
    assert_eq!(n, before);
}

#[test]
fn a_pipe_with_a_tap_on_it_is_not_taken_away() {
    let p = prefs(Junction::Tap);
    let c = conn("c", [0.0, 0.0, 1.0], PX);
    let r = route_from_connector(&c, &[[8.0, 0.0, 1.0]], round(0.05), &p, &off()).unwrap();
    let mut n = Network::new(1);
    n.add_route(&r).unwrap();
    n.attach_equipment(1, &c, 0, End::Start).unwrap();
    let br = branch_from_run(&n.segments[0].run, &[[4.0, 0.0, 1.0], [4.0, 3.0, 1.0]], &[round(0.05)], &p, &off()).unwrap();
    n.add_branch(0, &br).unwrap();
    n.validate().unwrap();
    let before = n.clone();
    let o = AutoOptions::default();
    let moved = conn("c", [0.0, 1.0, 1.0], PX);
    assert_eq!(reroute_equipment(&mut n, &request(0, &moved, &p, &[], &o)), Err(RerouteError::Graph(GraphError::TapOnChain(0))));
    assert_eq!(n, before);
}

#[test]
fn a_pipe_between_two_units_follows_the_one_that_moves() {
    let p = prefs(Junction::Tee);
    let o = AutoOptions::default();
    let (a, b) = (conn("a", [0.0, 0.0, 1.0], PX), conn("b", [6.0, 0.0, 1.0], NX));
    let r = auto_route(&AutoRequest { from: &a, to: &b, size: round(0.05), prefs: &p, zones: &[], from_own: &[], to_own: &[], opts: &o }).unwrap();
    let mut n = Network::new(1);
    n.add_route(&r.route).unwrap();
    n.attach_equipment(1, &a, 0, End::Start).unwrap();
    n.attach_equipment(2, &b, n.segments.len() - 1, End::End).unwrap();
    n.validate().unwrap();
    assert!(n.open_ports().is_empty());
    let moved = conn("a", [0.0, 4.0, 1.0], PX);
    let r2 = reroute_equipment(&mut n, &request(0, &moved, &p, &[], &o)).unwrap();
    n.validate().unwrap();
    assert!(n.open_ports().is_empty());
    assert_eq!(n.components().len(), 1);
    assert!(r2.route.cost > r.cost);
    // the other unit did not move
    assert_eq!(n.equipment[1].pos, b.pos);
}

#[test]
fn the_rest_of_the_network_is_kept_clear_of() {
    let mut n = tee_network();
    // another pipe straight across the way the unit would go
    let p = prefs(Junction::Tee);
    let o = AutoOptions::default();
    let other = route_path(&[[3.5, -2.0, 1.0], [3.5, 5.0, 1.0]], &[round(0.05)], &p, &off()).unwrap();
    n.add_route(&other).unwrap();
    let moved = conn("c", [1.0, 3.0, 1.0], PX);
    let r = reroute_equipment(&mut n, &request(0, &moved, &p, &[], &o)).unwrap();
    n.validate().unwrap();
    // the new centreline stays at least the two half sizes and the clearance from the other pipe
    let min = 0.025 + 0.025 + o.clearance;
    for run in &r.route.route.runs {
        for i in 0..=200 {
            let t = i as f64 / 200.0;
            let q = [0, 1, 2].map(|k| run.a[k] + (run.b[k] - run.a[k]) * t);
            if q[1] > -2.0 && q[1] < 5.0 {
                let d = ((q[0] - 3.5).powi(2) + (q[2] - 1.0).powi(2)).sqrt();
                assert!(d >= min - 1e-6, "{q:?}");
            }
        }
    }
}

#[test]
fn two_connectors_of_one_unit_move_together_or_not_at_all() {
    // a unit with a supply and a return, each joined to its own main by a tee
    let p = prefs(Junction::Tee);
    let mut n = Network::new(100);
    let mut mk = |x: f64, name: &str, y: f64, el: usize| {
        let main = route_path(&[[x, -3.0, 1.0], [x, 3.0, 1.0]], &[round(0.05)], &p, &off()).unwrap();
        let a = n.add_route(&main).unwrap();
        let br = branch_from_run(&n.segments[a.segments[0]].run, &[[x, y, 1.0], [2.0, y, 1.0]], &[round(0.05)], &p, &off()).unwrap();
        let added = n.add_branch(a.segments[0], &br).unwrap();
        n.attach_equipment(1, &conn(name, [2.0, y, 1.0], PX), *added.segments.last().unwrap(), End::End).unwrap();
        el
    };
    mk(6.0, "in", 0.3, 0);
    mk(7.0, "out", -0.3, 1);
    n.validate().unwrap();
    let o = AutoOptions::default();
    let (in2, out2) = (conn("in", [1.0, 3.0, 1.0], PX), conn("out", [1.0, 2.4, 1.0], PX));
    let reqs = [request(0, &in2, &p, &[], &o), request(1, &out2, &p, &[], &o)];
    let before = n.clone();
    let done = reroute_many(&mut n, &reqs).unwrap();
    assert_eq!(done.len(), 2);
    n.validate().unwrap();
    assert_eq!(n.components().len(), 2, "two mains, each with its branch");
    // the second pipe keeps clear of the first
    let rad = 0.025 + 0.025 + o.clearance;
    for q in done[1].route.route.runs.iter().flat_map(|r| (0..=100).map(move |i| [0, 1, 2].map(|k| r.a[k] + (r.b[k] - r.a[k]) * i as f64 / 100.0))) {
        for r in &done[0].route.route.runs {
            let (ab, aq): ([f64; 3], [f64; 3]) = ([0, 1, 2].map(|i| r.b[i] - r.a[i]), [0, 1, 2].map(|i| q[i] - r.a[i]));
            let t = ((0..3).map(|i| ab[i] * aq[i]).sum::<f64>() / (0..3).map(|i| ab[i] * ab[i]).sum::<f64>()).clamp(0.0, 1.0);
            let d = (0..3).map(|i| (aq[i] - ab[i] * t).powi(2)).sum::<f64>().sqrt();
            assert!(d >= rad - 1e-6, "{q:?} is {d} from the first pipe");
        }
    }
    // the second cannot be done (it starts inside an obstacle): nothing changes, and the error names it
    let mut n2 = before.clone();
    let block = [Zone::hard("post", [0.5, 2.3, 0.0], [1.3, 2.5, 2.0]).unwrap()];
    let bad = [request(0, &in2, &p, &[], &o), request(1, &out2, &p, &block, &o)];
    let e = reroute_many(&mut n2, &bad);
    assert!(matches!(e, Err((1, RerouteError::Auto(_)))), "{e:?}");
    assert_eq!(n2, before);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(40))]
    #[test]
    fn wherever_the_unit_goes_the_network_stays_whole(x in -4i32..9, y in -6i32..7, z in 0i32..4, d in 0usize..4) {
        let mut n = tee_network();
        let before = n.clone();
        let dirs = [PX, NX, [0.0, 1.0, 0.0], [0.0, -1.0, 0.0]];
        let moved = conn("c", [x as f64 * 0.5, y as f64 * 0.5, 0.5 + z as f64 * 0.5], dirs[d]);
        let p = prefs(Junction::Tee);
        let o = AutoOptions::default();
        match reroute_equipment(&mut n, &request(0, &moved, &p, &[], &o)) {
            Ok(r) => {
                prop_assert!(n.validate().is_ok());
                prop_assert_eq!(n.components().len(), 1);
                prop_assert_eq!(n.open_ports().len(), before.open_ports().len());
                prop_assert_eq!(n.equipment[0].pos, moved.pos);
                prop_assert!(r.kept >= 1);
                // the pieces off the chain are untouched
                for s in before.segments.iter().filter(|s| s.run.a[0] == 6.0 && s.run.b[0] == 6.0) {
                    prop_assert!(n.segments.iter().any(|t| t.id == s.id && t.run == s.run));
                }
            }
            Err(_) => prop_assert_eq!(n, before),
        }
    }
}

#[test]
fn the_graph_refuses_a_chain_swap_for_the_wrong_kind_of_connector() {
    let mut n = tee_network();
    let before = n.clone();
    let chain = n.chain_from_equipment(0).unwrap();
    let route = route_path(&[[2.0, 0.0, 1.0], [5.0, 0.0, 1.0]], &[round(0.05)], &prefs(Junction::Tee), &off()).unwrap();
    let mut duct = conn("c", [2.0, 0.0, 1.0], PX);
    duct.domain = Domain::Duct;
    assert_eq!(n.replace_chain(0, &duct, &chain, &route), Err(GraphError::EquipmentMismatch));
    assert_eq!(n, before);
    // a route that does not end at the junction leaves the network as it was
    let c = conn("c", [2.0, 0.0, 1.0], PX);
    assert!(n.replace_chain(0, &c, &chain, &route).is_err());
    assert_eq!(n, before);
}

#[test]
fn removing_pieces_renumbers_the_rest_and_keeps_their_links() {
    let mut n = tee_network();
    let chain = n.chain_from_equipment(0).unwrap();
    let keep: Vec<(u64, usize)> = n.segments.iter().enumerate().filter(|(i, _)| !chain.nodes.contains(&NodeRef::Segment(*i))).map(|(i, s)| (s.id, i)).collect();
    n.remove_nodes(&chain.nodes);
    assert_eq!(n.segments.len(), keep.len());
    assert!(n.segments.iter().map(|s| s.id).eq(keep.iter().map(|k| k.0)));
    // the tee's leg towards the main is still linked to the right halves, the leg towards the unit is open
    let tee = n.fittings.iter().position(|f| matches!(f.fitting.kind, FittingKind::Tee { .. })).unwrap();
    let linked = (0..3).filter(|&k| n.link_of(PortRef { node: NodeRef::Fitting(tee), port: k }).is_some()).count();
    assert_eq!(linked, 2);
    assert!(n.equipment.len() == 1 && n.link_of(PortRef { node: NodeRef::Equipment(0), port: 0 }).is_none());
}
