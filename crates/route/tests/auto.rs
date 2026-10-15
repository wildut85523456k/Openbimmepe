use openbim_family::mep::{Connector, Domain, FlowDir, Size, System};
use openbim_geom::WallChain;
use openbim_place::penetrations::RunSize;
use openbim_route::auto::*;
use openbim_route::graph::*;
use openbim_route::*;
use proptest::prelude::*;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

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
fn conn(pos: [f64; 3], dir: [f64; 3], d: f64) -> Connector {
    Connector { name: "c".into(), domain: Domain::Pipe, system: System::ChilledWaterSupply, flow_dir: FlowDir::Out, pos, dir, up: [0.0, 0.0, 1.0], size: Size::Round { diameter: d }, flow: 0.0, pressure_drop: 0.0, power_factor: 1.0 }
}
const PX: [f64; 3] = [1.0, 0.0, 0.0];
const NX: [f64; 3] = [-1.0, 0.0, 0.0];
const PY: [f64; 3] = [0.0, 1.0, 0.0];
const NY: [f64; 3] = [0.0, -1.0, 0.0];

#[allow(clippy::too_many_arguments)]
fn run_req<'a>(from: &'a Connector, to: &'a Connector, size: RunSize, p: &'a RoutingPrefs, zones: &'a [Zone], from_own: &'a [usize], to_own: &'a [usize], o: &'a AutoOptions) -> AutoRequest<'a> {
    AutoRequest { from, to, size, prefs: p, zones, from_own, to_own, opts: o }
}
fn go(from: &Connector, to: &Connector, zones: &[Zone], o: &AutoOptions) -> Result<AutoRoute, AutoError> {
    let p = prefs();
    auto_route(&run_req(from, to, round(0.05), &p, zones, &[], &[], o))
}
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}
fn close3(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| close(a[i], b[i]))
}
fn bends(ar: &AutoRoute) -> usize {
    ar.route.fittings.iter().filter(|f| matches!(f.kind, FittingKind::Elbow { .. })).count()
}
/// the centreline of every run, sampled
fn samples(ar: &AutoRoute) -> Vec<[f64; 3]> {
    let mut v = Vec::new();
    for r in &ar.route.runs {
        for i in 0..=100 {
            let t = i as f64 / 100.0;
            v.push([0, 1, 2].map(|k| r.a[k] + (r.b[k] - r.a[k]) * t));
        }
    }
    v
}
fn inside_open(p: [f64; 3], lo: [f64; 3], hi: [f64; 3], r: f64) -> bool {
    (0..3).all(|i| p[i] > lo[i] - r + 1e-6 && p[i] < hi[i] + r - 1e-6)
}

#[test]
fn facing_connectors_in_the_open_are_joined_by_one_straight_run() {
    let (a, b) = (conn([0.0, 0.0, 1.0], PX, 0.05), conn([5.0, 0.0, 1.0], NX, 0.05));
    let r = go(&a, &b, &[], &AutoOptions::default()).unwrap();
    assert_eq!(r.waypoints.len(), 2);
    assert!(close(r.cost, 5.0), "{}", r.cost);
    assert_eq!(bends(&r), 0);
    assert_eq!(r.route.runs.len(), 1);
    assert!(close3(r.route.runs[0].a, a.pos) && close3(r.route.runs[0].b, b.pos));
    assert!(r.crossed.is_empty());
}

#[test]
fn an_l_path_has_one_elbow_and_costs_length_plus_a_bend() {
    let (a, b) = (conn([0.0, 0.0, 1.0], PX, 0.05), conn([4.0, 3.0, 1.0], NY, 0.05));
    let r = go(&a, &b, &[], &AutoOptions::default()).unwrap();
    assert_eq!(r.waypoints.len(), 3);
    assert!(close3(r.waypoints[1], [4.0, 0.0, 1.0]));
    assert_eq!(bends(&r), 1);
    assert!(close(r.cost, 4.0 + 3.0 + 1.0), "{}", r.cost);
    // it ends going into the second connector the way that connector faces
    let last = r.route.runs.last().unwrap();
    assert!(close3(last.b, b.pos) && last.a[1] < last.b[1]);
}

#[test]
fn a_riser_costs_more_and_makes_two_bends() {
    let (a, b) = (conn([0.0, 0.0, 0.5], PX, 0.05), conn([4.0, 0.0, 3.5], NX, 0.05));
    let r = go(&a, &b, &[], &AutoOptions::default()).unwrap();
    assert_eq!(bends(&r), 2);
    assert!(close(r.cost, 4.0 + 3.0 * 1.5 + 2.0), "{}", r.cost);
    // with vertical moves off the same request is refused rather than flattened
    let o = AutoOptions { allow_vertical: false, ..Default::default() };
    assert!(matches!(go(&a, &b, &[], &o), Err(AutoError::BadInput(_))));
}

#[test]
fn a_box_in_the_way_is_gone_round_and_never_entered() {
    let (a, b) = (conn([0.0, 0.0, 1.0], PX, 0.05), conn([8.0, 0.0, 1.0], NX, 0.05));
    let z = Zone::hard("duct", [3.0, -1.0, 0.0], [4.0, 1.0, 3.0]).unwrap();
    let o = AutoOptions::default();
    let r = go(&a, &b, std::slice::from_ref(&z), &o).unwrap();
    assert!(bends(&r) >= 2 && r.cost > 8.0);
    let rad = 0.025 + o.clearance;
    for p in samples(&r) {
        assert!(!inside_open(p, [3.0, -1.0, 0.0], [4.0, 1.0, 3.0], rad), "{p:?}");
    }
    // every leg is along an axis
    for w in r.waypoints.windows(2) {
        assert_eq!((0..3).filter(|&i| !close(w[0][i], w[1][i])).count(), 1);
    }
    // blocking the whole search volume leaves no path
    let wall = Zone::hard("everything", [2.0, -10.0, -10.0], [2.5, 10.0, 10.0]).unwrap();
    let e = go(&a, &b, &[wall], &o);
    assert!(matches!(e, Err(AutoError::NoPath)), "{e:?}");
}

#[test]
fn the_ends_own_equipment_is_left_by_the_connector_and_avoided_after_that() {
    // a unit whose face the connector sits on; the target lies behind the unit
    let a = conn([0.0, 0.0, 1.0], PX, 0.05);
    // the target faces +x, so the straight way to it would pass through the unit itself
    let b = conn([-3.0, 0.0, 1.0], PX, 0.05);
    let body = Zone::hard("unit", [-0.6, -0.4, 0.5], [0.0, 0.4, 1.5]).unwrap();
    let o = AutoOptions::default();
    let p = prefs();
    let zones = [body.clone()];
    // not told it is the unit's own: the connector is inside the widened box, nothing can leave
    assert!(matches!(auto_route(&run_req(&a, &b, round(0.05), &p, &zones, &[], &[], &o)), Err(AutoError::NoPath)));
    let r = auto_route(&run_req(&a, &b, round(0.05), &p, &zones, &[0], &[], &o)).unwrap();
    let rad = 0.025 + o.clearance;
    for q in samples(&r) {
        if inside_open(q, [-0.6, -0.4, 0.5], [0.0, 0.4, 1.5], rad) {
            // only the straight lead out of the connector may be inside the widened box
            assert!(q[1].abs() < 1e-9 && close(q[2], 1.0) && q[0] >= -1e-9, "{q:?}");
        }
    }
    assert!(bends(&r) >= 2);
}

#[test]
fn a_wall_is_crossed_when_that_is_cheaper_and_gone_round_when_it_is_not() {
    let mut chain = WallChain::new(vec![[2.0, -1.0], [2.0, 1.0]], false, 0.2, 3.0);
    let (a, b) = (conn([0.0, 0.0, 1.0], PX, 0.05), conn([4.0, 0.0, 1.0], NX, 0.05));
    let o = AutoOptions::default();
    // cheap to cross: straight through, one crossing listed
    let walls = Zone::walls(&chain, 2.0, 1000.0).unwrap();
    let r = go(&a, &b, &walls, &o).unwrap();
    assert_eq!(r.crossed, vec!["wall segment 0".to_string()]);
    assert_eq!(bends(&r), 0);
    assert!(r.cost > 4.0 + 2.0 && r.cost < 4.0 + 2.0 + 0.5 * o.inside_factor, "{}", r.cost);
    // a fire partition costs far more: the path goes round the end of it instead
    chain.fire_rating_min = Some(60);
    let walls = Zone::walls(&chain, 2.0, 1000.0).unwrap();
    let r = go(&a, &b, &walls, &o).unwrap();
    assert!(r.crossed.is_empty() && bends(&r) >= 2, "{:?}", r.crossed);
    // never turning, and never running along, inside a wall: find the points inside the widened wall
    let rad = 0.025 + o.clearance;
    for p in samples(&r) {
        assert!(!((p[0] - 2.0).abs() < 0.1 + rad - 1e-6 && p[1].abs() < 1.0 + rad - 1e-6 && p[2] < 3.0 + rad), "{p:?}");
    }
}

#[test]
fn running_along_inside_a_wall_costs_more_than_stepping_beside_it() {
    // a wall along the way: the path stays off it
    let chain = WallChain::new(vec![[1.0, 0.0], [6.0, 0.0]], false, 0.2, 3.0);
    let walls = Zone::walls(&chain, 5.0, 5.0).unwrap();
    let (a, b) = (conn([0.0, 0.0, 1.0], PX, 0.05), conn([7.0, 0.5, 1.0], NX, 0.05));
    let r = go(&a, &b, &walls, &AutoOptions::default()).unwrap();
    // if it ran inside the wall it would have been charged ten times the length; it is not
    assert!(r.cost < 7.0 + 0.5 + 6.0, "{}", r.cost);
}

#[test]
fn bad_requests_are_refused() {
    let o = AutoOptions::default();
    let (a, b) = (conn([0.0, 0.0, 1.0], PX, 0.05), conn([5.0, 0.0, 1.0], NX, 0.05));
    // a connector that is not along an axis
    let skew = conn([5.0, 0.0, 1.0], [-1.0, 1.0, 0.0], 0.05);
    assert_eq!(go(&a, &skew, &[], &o), Err(AutoError::NotAxisAligned));
    // wrong domain
    let mut duct = b.clone();
    duct.domain = Domain::Duct;
    assert_eq!(go(&a, &duct, &[], &o), Err(AutoError::Route(RouteError::ConnectorMismatch)));
    // one point
    assert!(matches!(go(&a, &a, &[], &o), Err(AutoError::BadInput(_))));
    // options out of range
    for bad in [AutoOptions { pitch: 0.0, ..o.clone() }, AutoOptions { vertical_factor: 0.5, ..o.clone() }, AutoOptions { bend_cost: f64::NAN, ..o.clone() }, AutoOptions { max_expansions: 0, ..o.clone() }, AutoOptions { min_leg: Some(0.0), ..o.clone() }] {
        assert!(matches!(go(&a, &b, &[], &bad), Err(AutoError::BadInput(_))), "{bad:?}");
    }
    // own-zone numbers must exist
    let p = prefs();
    assert!(matches!(auto_route(&run_req(&a, &b, round(0.05), &p, &[], &[3], &[], &o)), Err(AutoError::BadInput(_))));
    // zones that make no sense
    assert!(Zone::hard("x", [0.0; 3], [0.0, 1.0, 1.0]).is_err());
    assert!(Zone::soft("x", &[[0.0, 0.0], [1.0, 0.0]], 0.0, 1.0, 1.0).is_err());
    assert!(Zone::soft("x", &[[0.0, 0.0], [2.0, 0.0], [1.0, 0.2], [2.0, 2.0], [0.0, 2.0]], 0.0, 1.0, 1.0).is_err(), "not convex");
    assert!(Zone::soft("x", &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], 1.0, 0.0, 1.0).is_err());
    assert!(Zone::soft("x", &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], 0.0, 1.0, -1.0).is_err());
    // a search that is too big or runs too long
    let big = AutoOptions { pitch: 0.005, search_margin: 1000.0, ..o.clone() };
    assert_eq!(go(&a, &b, &[], &big), Err(AutoError::TooLarge));
    let short = AutoOptions { max_expansions: 3, ..o.clone() };
    assert_eq!(go(&a, &b, &[], &short), Err(AutoError::TooLarge));
}

#[test]
fn the_result_joins_both_pieces_of_equipment_in_the_graph() {
    let o = AutoOptions::default();
    let (a, b) = (conn([0.0, 0.0, 1.0], PX, 0.05), conn([4.0, 3.0, 1.0], NY, 0.05));
    let r = go(&a, &b, &[], &o).unwrap();
    let mut n = Network::new(100);
    n.add_route(&r.route).unwrap();
    n.attach_equipment(1, &a, 0, End::Start).unwrap();
    n.attach_equipment(2, &b, n.segments.len() - 1, End::End).unwrap();
    n.validate().unwrap();
    assert!(n.open_ports().is_empty());
    assert_eq!(n.components().len(), 1);
    // a bigger pipe than the connectors: transitions on the stubs, still joined
    let p = prefs();
    let big = auto_route(&run_req(&a, &b, round(0.08), &p, &[], &[], &[], &o)).unwrap();
    let mut n = Network::new(100);
    n.add_route(&big.route).unwrap();
    n.attach_equipment(1, &a, 0, End::Start).unwrap();
    n.attach_equipment(2, &b, n.segments.len() - 1, End::End).unwrap();
    n.validate().unwrap();
    assert!(n.open_ports().is_empty());
    assert_eq!(big.route.fittings.iter().filter(|f| matches!(f.kind, FittingKind::Transition)).count(), 2);
}

#[test]
fn many_paths_at_once_give_what_each_gives_alone_and_a_sequence_keeps_apart() {
    let p = prefs();
    let o = AutoOptions::default();
    let zs = [Zone::hard("duct", [3.0, -1.0, 0.0], [4.0, 1.0, 3.0]).unwrap()];
    let cs: Vec<(Connector, Connector)> = (0..7).map(|i| (conn([0.0, i as f64 * 0.7, 1.0], PX, 0.05), conn([8.0, i as f64 * 0.7 + 0.3, 1.0], NX, 0.05))).collect();
    let reqs: Vec<AutoRequest> = cs.iter().map(|(a, b)| run_req(a, b, round(0.05), &p, &zs, &[], &[], &o)).collect();
    let many = auto_route_many(&reqs);
    for (rq, m) in reqs.iter().zip(&many) {
        assert_eq!(*m, auto_route(rq));
    }
    assert_eq!(many, auto_route_many(&reqs), "same every time");
    // two pipes asked for in a sequence: the second keeps clear of the first
    let (a1, b1) = (conn([0.0, 0.0, 1.0], PX, 0.05), conn([6.0, 0.0, 1.0], NX, 0.05));
    let (a2, b2) = (conn([0.0, 0.3, 1.0], PX, 0.05), conn([6.0, 0.3, 1.0], NX, 0.05));
    let rs = [run_req(&a1, &b1, round(0.05), &p, &[], &[], &[], &o), run_req(&a2, &b2, round(0.05), &p, &[], &[], &[], &o)];
    let seq = auto_route_sequence(&rs);
    let first = seq[0].as_ref().unwrap();
    assert_eq!(bends(first), 0);
    // alone, the second would also run straight; after the first it still ends at its connectors but cannot be closer than the widened first run
    let second = seq[1].as_ref().unwrap();
    assert!(close3(second.route.runs[0].a, a2.pos));
    let rad = 0.025 + 0.025 + o.clearance;
    for q in samples(second) {
        if q[0] > 0.31 && q[0] < 5.69 {
            assert!(((q[1] * q[1]) + (q[2] - 1.0).powi(2)).sqrt() >= rad - 1e-6, "{q:?}");
        }
    }
}

// ---------- the search against a plain reference ----------

type Ix = [i32; 3];
fn dirv(d: usize) -> Ix {
    let mut v = [0; 3];
    v[d / 2] = if d.is_multiple_of(2) { 1 } else { -1 };
    v
}

/// Dijkstra over (position, direction, run) on whole units of the pitch, written separately from the search. The costs are in
/// hundredths of a metre: a step is 10, a vertical step `vf`, a bend `bend`. Boxes are in units, widened by 3/4 of a unit.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn reference(start_dir: usize, goal: Ix, goal_dir: usize, boxes: &[(Ix, Ix)], vertical: bool, k: usize, margin: i32, vf: i64, bend: i64) -> Option<i64> {
    let lo = [goal[0].min(0) - margin, goal[1].min(0) - margin, if vertical { goal[2].min(0) - margin } else { 0 }];
    let hi = [goal[0].max(0) + margin, goal[1].max(0) + margin, if vertical { goal[2].max(0) + margin } else { 0 }];
    let blocked = |p: Ix, q: Ix| -> bool {
        boxes.iter().any(|(bl, bh)| {
            (0..3).all(|i| {
                let (mn, mx) = (p[i].min(q[i]), p[i].max(q[i]));
                mx >= bl[i] && mn <= bh[i]
            })
        })
    };
    let mut dist: HashMap<(Ix, usize, usize), i64> = HashMap::new();
    let mut heap = BinaryHeap::new();
    dist.insert(([0, 0, 0], start_dir, 0), 0);
    heap.push(Reverse((0i64, [0, 0, 0], start_dir, 0usize)));
    while let Some(Reverse((c, p, d, run))) = heap.pop() {
        if dist.get(&(p, d, run)).is_some_and(|&x| c > x) {
            continue;
        }
        if p == goal && d == goal_dir && run == k {
            return Some(c);
        }
        for nd in 0..6 {
            if nd != d && (nd == (d ^ 1) || run < k) {
                continue;
            }
            if !vertical && nd / 2 == 2 {
                continue;
            }
            let v = dirv(nd);
            let q = [p[0] + v[0], p[1] + v[1], p[2] + v[2]];
            if (0..3).any(|i| q[i] < lo[i] || q[i] > hi[i]) || blocked(p, q) {
                continue;
            }
            let nc = c + if nd / 2 == 2 { vf } else { 10 } + if nd != d { bend } else { 0 };
            let nrun = if nd == d { (run + 1).min(k) } else { 1.min(k) };
            let e = dist.entry((q, nd, nrun)).or_insert(i64::MAX);
            if nc < *e {
                *e = nc;
                heap.push(Reverse((nc, q, nd, nrun)));
            }
        }
    }
    None
}

fn unit_pos(u: Ix) -> [f64; 3] {
    [u[0] as f64 * 0.1, u[1] as f64 * 0.1, u[2] as f64 * 0.1]
}
fn unit_dir(d: usize) -> [f64; 3] {
    let v = dirv(d);
    [v[0] as f64, v[1] as f64, v[2] as f64]
}

fn case() -> impl Strategy<Value = (usize, Ix, usize, Vec<(Ix, Ix)>)> {
    let boxes = proptest::collection::vec(((-4i32..14, -8i32..8, -2i32..4), (1i32..4, 1i32..4, 1i32..3)), 0..5);
    (0usize..6, (-12i32..16, -10i32..10, -3i32..4), 0usize..6, boxes).prop_map(|(d1, g, d2, bs)| {
        let bx = bs.into_iter().map(|((x, y, z), (a, b, c))| ([x, y, z], [x + a, y + b, z + c])).collect();
        (d1, [g.0, g.1, g.2], d2, bx)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(120))]
    #[test]
    fn the_cost_is_the_best_there_is_and_the_path_is_clear(c in case(), vertical in any::<bool>(), vfi in 0usize..3, bci in 0usize..4) {
        let (vf, bc) = ([10i64, 15, 30][vfi], [0i64, 50, 100, 300][bci]);
        let (d1, g, d2, bx) = c;
        let g = if vertical { g } else { [g[0], g[1], 0] };
        prop_assume!(g != [0, 0, 0]);
        prop_assume!(vertical || (d1 / 2 != 2 && d2 / 2 != 2));
        let (a, b) = (conn([0.0; 3], unit_dir(d1), 0.05), conn(unit_pos(g), unit_dir(d2), 0.05));
        let o = AutoOptions { allow_vertical: vertical, min_leg: Some(0.5), search_margin: 1.0, vertical_factor: vf as f64 / 10.0, bend_cost: bc as f64 / 100.0, ..Default::default() };
        let zones: Vec<Zone> = bx.iter().map(|(l, h)| Zone::hard("b", unit_pos(*l), unit_pos(*h)).unwrap()).collect();
        let expect = reference(d1, g, d2 ^ 1, &bx, vertical, 5, 10, vf, bc);
        let p = prefs();
        let got = auto_route(&run_req(&a, &b, round(0.05), &p, &zones, &[], &[], &o));
        match (expect, got) {
            (None, Err(AutoError::NoPath)) => {}
            (Some(c), Ok(r)) => {
                prop_assert!((r.cost * 100.0 - c as f64).abs() < 1e-6, "cost {} vs {}", r.cost * 100.0, c);
                // clear of every box, all legs along an axis, every leg between bends at least the shortest leg
                let rad = 0.025 + o.clearance;
                for q in samples(&r) {
                    for (l, h) in &bx {
                        prop_assert!(!inside_open(q, unit_pos(*l), unit_pos(*h), rad), "{q:?} in {l:?} {h:?}");
                    }
                }
                for w in r.waypoints.windows(2) {
                    let moved = (0..3).filter(|&i| !close(w[0][i], w[1][i])).count();
                    prop_assert_eq!(moved, 1);
                    let len: f64 = (0..3).map(|i| (w[1][i] - w[0][i]).abs()).sum();
                    prop_assert!(len >= 0.5 - 1e-9, "leg {len}");
                }
                prop_assert!(close3(r.waypoints[0], a.pos) && close3(*r.waypoints.last().unwrap(), b.pos));
            }
            (e, g) => prop_assert!(false, "reference {e:?}, search {:?}", g.map(|r| r.cost)),
        }
    }
}

#[test]
fn the_far_end_leaves_its_own_equipment_the_same_way() {
    // the target sits on the face of a unit, entered along its own connector
    let a = conn([0.0, 0.0, 1.0], PX, 0.05);
    let b = conn([6.0, 0.0, 1.0], NX, 0.05);
    let body = [Zone::hard("unit", [6.0, -0.4, 0.5], [6.8, 0.4, 1.5]).unwrap()];
    let o = AutoOptions::default();
    let p = prefs();
    assert!(matches!(auto_route(&run_req(&a, &b, round(0.05), &p, &body, &[], &[], &o)), Err(AutoError::NoPath)));
    let r = auto_route(&run_req(&a, &b, round(0.05), &p, &body, &[], &[0], &o)).unwrap();
    assert_eq!(bends(&r), 0);
    assert!(close(r.cost, 6.0));
}

#[test]
fn no_turn_is_made_inside_a_soft_zone() {
    // the target can only be entered going +y at x=2.5, which is inside a thick zone that spans everything
    let a = conn([0.0, 0.0, 1.0], PX, 0.05);
    let b = conn([2.5, 3.0, 1.0], NY, 0.05);
    let thick = Zone::soft("thick", &[[1.0, -10.0], [4.0, -10.0], [4.0, 10.0], [1.0, 10.0]], -10.0, 10.0, 0.0).unwrap();
    let o = AutoOptions { inside_factor: 1.0, search_margin: 0.3, ..Default::default() };
    assert_eq!(go(&a, &b, std::slice::from_ref(&thick), &o), Err(AutoError::NoPath));
    // without the zone it is an ordinary L
    assert!(go(&a, &b, &[], &o).is_ok());
}

#[test]
fn a_zone_the_equipment_stands_in_is_not_charged_on_the_way_out() {
    // a connector in a wall (a wall-mounted unit): the lead out through the wall is the unit's own
    let chain = WallChain::new(vec![[2.0, -2.0], [2.0, 2.0]], false, 0.2, 3.0);
    let walls = Zone::walls(&chain, 2.0, 2.0).unwrap();
    let a = conn([2.0, 0.0, 1.0], PX, 0.05);
    let b = conn([5.0, 0.0, 1.0], NX, 0.05);
    let o = AutoOptions::default();
    let p = prefs();
    let plain = auto_route(&run_req(&a, &b, round(0.05), &p, &walls, &[], &[], &o)).unwrap();
    let own = auto_route(&run_req(&a, &b, round(0.05), &p, &walls, &[0], &[], &o)).unwrap();
    assert!(close(own.cost, 3.0), "{}", own.cost);
    assert!(own.crossed.is_empty() && plain.cost > own.cost);
}

#[test]
fn a_later_pipe_goes_round_an_earlier_one_in_a_sequence() {
    let p = prefs();
    let o = AutoOptions::default();
    let (a1, b1) = (conn([0.0, 0.0, 1.0], PX, 0.05), conn([6.0, 0.0, 1.0], NX, 0.05));
    let (a2, b2) = (conn([3.0, -3.0, 1.0], PY, 0.05), conn([3.0, 3.0, 1.0], NY, 0.05));
    let rs = [run_req(&a1, &b1, round(0.05), &p, &[], &[], &[], &o), run_req(&a2, &b2, round(0.05), &p, &[], &[], &[], &o)];
    let alone = auto_route(&rs[1]).unwrap();
    assert!(close(alone.cost, 6.0));
    let seq = auto_route_sequence(&rs);
    let second = seq[1].as_ref().unwrap();
    assert!(second.cost > 6.0 + 1.0, "{}", second.cost);
    // the centrelines keep the two widened half sizes apart
    let rad = 0.025 + 0.025 + o.clearance;
    let first = seq[0].as_ref().unwrap();
    for q in samples(second) {
        for r in &first.route.runs {
            let (ab, aq): ([f64; 3], [f64; 3]) = ([0, 1, 2].map(|i| r.b[i] - r.a[i]), [0, 1, 2].map(|i| q[i] - r.a[i]));
            let t = ((0..3).map(|i| ab[i] * aq[i]).sum::<f64>() / (0..3).map(|i| ab[i] * ab[i]).sum::<f64>()).clamp(0.0, 1.0);
            let d = (0..3).map(|i| (aq[i] - ab[i] * t).powi(2)).sum::<f64>().sqrt();
            assert!(d >= rad - 1e-6, "{q:?} is {d} from a run");
        }
    }
}
