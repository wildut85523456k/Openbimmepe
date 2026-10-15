//! Times the automatic router on a room-sized case: `cargo run --release -p openbim-route --example auto_bench`
use openbim_family::mep::{Connector, Domain, FlowDir, Size, System};
use openbim_place::penetrations::RunSize;
use openbim_route::auto::*;
use openbim_route::*;
use std::time::Instant;

fn conn(pos: [f64; 3], dir: [f64; 3]) -> Connector {
    Connector { name: "c".into(), domain: Domain::Pipe, system: System::ChilledWaterSupply, flow_dir: FlowDir::Out, pos, dir, up: [0.0, 0.0, 1.0], size: Size::Round { diameter: 0.05 }, flow: 0.0, pressure_drop: 0.0, power_factor: 1.0 }
}

fn main() {
    let prefs = RoutingPrefs {
        domain: Domain::Pipe, system: System::ChilledWaterSupply, label: "CHWS".into(), elbow_angles: AngleSet::List(vec![90.0]), bend_radius_factor: 1.5, min_straight: 0.05,
        junction: Junction::Tee, tee_angles: AngleSet::List(vec![90.0]), tee_half_factor: 1.0, branch_stub_factor: 1.0, pick_tolerance: 0.02, transition_angle_deg: 30.0,
        min_transition_len: 0.1, connector_stub: 0.3, margin: 0.0, catalogue: None,
    };
    // 24 m x 12 m x 4 m, a maze of boxes that make the straight way long
    let mut zones = Vec::new();
    for i in 0..8 {
        let x = 2.0 + i as f64 * 2.6;
        let (y0, y1) = if i % 2 == 0 { (-6.0, 4.0) } else { (-4.0, 6.0) };
        zones.push(Zone::hard("w", [x, y0, 0.0], [x + 0.3, y1, 4.0]).unwrap());
    }
    let (a, b) = (conn([0.0, 0.0, 1.5], [1.0, 0.0, 0.0]), conn([23.0, 0.5, 1.5], [-1.0, 0.0, 0.0]));
    for (name, pitch) in [("pitch 0.2", 0.2), ("pitch 0.1", 0.1)] {
        let o = AutoOptions { pitch, search_margin: 4.0, max_expansions: 20_000_000, ..Default::default() };
        let rq = AutoRequest { from: &a, to: &b, size: RunSize::Round { diameter: 0.05 }, prefs: &prefs, zones: &zones, from_own: &[], to_own: &[], opts: &o };
        let t = Instant::now();
        let r = auto_route(&rq);
        match r {
            Ok(r) => println!("{name}: cost {:.2}, {} waypoints, {} expansions, {:?}", r.cost, r.waypoints.len(), r.expansions, t.elapsed()),
            Err(e) => println!("{name}: {e:?} after {:?}", t.elapsed()),
        }
    }
    // twelve independent pairs on all cores
    let pairs: Vec<(Connector, Connector)> = (0..12).map(|i| (conn([0.0, i as f64 * 0.3 - 1.5, 1.5], [1.0, 0.0, 0.0]), conn([23.0, i as f64 * 0.3 - 1.0, 1.5], [-1.0, 0.0, 0.0]))).collect();
    let o = AutoOptions { pitch: 0.2, search_margin: 4.0, max_expansions: 20_000_000, ..Default::default() };
    let reqs: Vec<AutoRequest> = pairs.iter().map(|(a, b)| AutoRequest { from: a, to: b, size: RunSize::Round { diameter: 0.05 }, prefs: &prefs, zones: &zones, from_own: &[], to_own: &[], opts: &o }).collect();
    let t = Instant::now();
    let seq: Vec<_> = reqs.iter().map(auto_route).collect();
    let t_seq = t.elapsed();
    let t = Instant::now();
    let par = auto_route_many(&reqs);
    let t_par = t.elapsed();
    println!("12 paths: one after another {t_seq:?}, on all cores {t_par:?}, same results: {}", seq == par);
}
