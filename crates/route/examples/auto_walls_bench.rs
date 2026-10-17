//! Times the automatic router against many wall zones: `cargo run --release -p openbim-route --example auto_walls_bench`
//! Rooms of 3 m in a grid, every wall a soft zone (cost to cross), one run across the whole grid.
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
    for (nx, ny) in [(4usize, 4usize), (6, 6), (8, 8), (10, 10)] {
        // every wall of every room is a zone of its own, as walls are in a model
        let mut zones = Vec::new();
        for i in 0..nx {
            for j in 0..ny {
                let (x, y) = (i as f64 * 3.0, j as f64 * 3.0);
                for (p0, p1) in [([x, y], [x + 3.0, y]), ([x + 3.0, y], [x + 3.0, y + 3.0]), ([x + 3.0, y + 3.0], [x, y + 3.0]), ([x, y + 3.0], [x, y])] {
                    let (dx, dy) = ((p1[1] - p0[1]) / 3.0 * 0.1, -(p1[0] - p0[0]) / 3.0 * 0.1);
                    zones.push(Zone::soft("wall", &[[p0[0] + dx, p0[1] + dy], [p1[0] + dx, p1[1] + dy], [p1[0] - dx, p1[1] - dy], [p0[0] - dx, p0[1] - dy]], 0.0, 3.0, 5.0).unwrap());
                }
            }
        }
        // zones that cannot change the route (below the lowest height it uses) but are in the way of the lookups
        let decoys: usize = std::env::var("DECOYS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        for k in 0..decoys {
            let (x, y) = ((k * 37 % 997) as f64 / 997.0 * nx as f64 * 3.0, (k * 91 % 991) as f64 / 991.0 * ny as f64 * 3.0);
            zones.push(Zone::soft("decoy", &[[x, y], [x + 0.1, y], [x + 0.1, y + 0.1], [x, y + 0.1]], 0.5, 0.52, 5.0).unwrap());
        }
        if std::env::var("NOZONES").is_ok() { zones.clear(); }
        let (a, b) = (conn([1.5, 1.5, 2.5], [0.0, 1.0, 0.0]), conn([nx as f64 * 3.0 - 1.5, ny as f64 * 3.0 - 1.5, 2.5], [0.0, -1.0, 0.0]));
        let o = AutoOptions::default();
        let rq = AutoRequest { from: &a, to: &b, size: RunSize::Round { diameter: 0.05 }, prefs: &prefs, zones: &zones, from_own: &[], to_own: &[], opts: &o };
        let t = Instant::now();
        match auto_route(&rq) {
            Ok(r) => println!("{} zones: cost {:.4}, {} waypoints, {} expansions, crossed {:?}, {:?}", zones.len(), r.cost, r.waypoints.len(), r.expansions, r.crossed, t.elapsed()),
            Err(e) => println!("{} zones: {e:?} after {:?}", zones.len(), t.elapsed()),
        }
    }
}
