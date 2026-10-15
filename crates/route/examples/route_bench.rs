//! Time of one automatic route with many walls in the model: `route_bench [walls]`.
use openbim_family::mep::{Connector, Domain, FlowDir, Size, System};
use openbim_place::penetrations::RunSize;
use openbim_route::auto::*;
use openbim_route::*;
use std::time::Instant;

fn main() {
    let walls: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(200);
    let prefs = RoutingPrefs {
        domain: Domain::Pipe, system: System::ChilledWaterSupply, label: "CHWS".into(), elbow_angles: AngleSet::List(vec![90.0]), bend_radius_factor: 1.5, min_straight: 0.05,
        junction: Junction::Tee, tee_angles: AngleSet::List(vec![90.0]), tee_half_factor: 1.0, branch_stub_factor: 1.0, pick_tolerance: 0.02, transition_angle_deg: 30.0,
        min_transition_len: 0.1, connector_stub: 0.3, margin: 0.0, catalogue: None,
    };
    let conn = |pos: [f64; 3], dir: [f64; 3]| Connector { name: "c".into(), domain: Domain::Pipe, system: System::ChilledWaterSupply, flow_dir: FlowDir::Out, pos, dir, up: [0.0, 0.0, 1.0], size: Size::Round { diameter: 0.05 }, flow: 0.0, pressure_drop: 0.0, power_factor: 1.0 };
    let (a, b) = (conn([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]), conn([12.0, 6.0, 1.0], [-1.0, 0.0, 0.0]));
    // walls all over the model, far from and across the route
    let mut zones = Vec::new();
    for i in 0..walls {
        let (x, y) = (-2.0 + (i % 8) as f64 * 1.9, -2.0 + (i / 8) as f64 * 0.7);
        zones.push(Zone::soft("wall", &[[x, y], [x + 1.8, y], [x + 1.8, y + 0.2], [x, y + 0.2]], 0.0, 3.0, 3.0).unwrap());
    }
    let o = AutoOptions::default();
    let t = Instant::now();
    let r = auto_route(&AutoRequest { from: &a, to: &b, size: RunSize::Round { diameter: 0.05 }, prefs: &prefs, zones: &zones, from_own: &[], to_own: &[], opts: &o }).unwrap();
    println!("{walls} walls: {:.3} s, {} expansions, cost {:.4}", t.elapsed().as_secs_f64(), r.expansions, r.cost);
}
