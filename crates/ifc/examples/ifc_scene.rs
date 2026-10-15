//! Writes the test building as IFC4 to the path given, and prints what it expects to be in the file, as plain
//! text, for the independent IfcOpenShell check (this text is not a product data path).
use openbim_arch::spaces::Shaft;
use openbim_arch::{Level, Levels};
use openbim_family::mep::{Connector, Domain, FlowDir, Size, System};
use openbim_geom::{Opening, WallChain};
use openbim_ifc::*;
use openbim_place::floors::{find_floor, ShaftVoid, Slab};
use openbim_place::penetrations::RunSize;
use openbim_place::Model;
use openbim_route::graph::{End, Network, NodeRef, PortRef};
use openbim_route::snaps::{Snaps, SnapSettings};
use openbim_route::*;

const A: u64 = 1 << 32;

fn round(d: f64) -> RunSize {
    RunSize::Round { diameter: d }
}

fn prefs(domain: Domain, system: System, label: &str) -> RoutingPrefs {
    RoutingPrefs {
        domain,
        system,
        label: label.into(),
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
        margin: 0.025,
        catalogue: None,
    }
}

/// The scene, built the way the app builds it. `edit` moves the door of the wall (an edit that must not change any id).
pub fn build_scene(edit: bool) -> Scene {
    let off = Snaps::new(SnapSettings::off());
    let mut net = Network::new(A | 100);
    let pump = Connector { name: "Out".into(), domain: Domain::Pipe, system: System::ChilledWaterSupply, flow_dir: FlowDir::Out, pos: [6.0, 3.0, 1.0], dir: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], size: Size::Round { diameter: 0.05 }, flow: 0.0, pressure_drop: 0.0, power_factor: 1.0 };
    let p1 = prefs(Domain::Pipe, System::ChilledWaterSupply, "CHWS");
    let r1 = route_from_connector(&pump, &[[12.0, 3.0, 1.0], [12.0, 6.0, 1.0]], round(0.05), &p1, &off).unwrap();
    net.add_route(&r1).unwrap();
    net.attach_equipment(A | 900, &pump, 0, End::Start).unwrap();
    // cold water with a tee branch
    let p2 = prefs(Domain::Pipe, System::DomesticCold, "DCW");
    let r2 = route_path(&[[2.0, 6.0, 2.0], [8.0, 6.0, 2.0]], &[round(0.1)], &p2, &off).unwrap();
    let first2 = net.segments.len();
    net.add_route(&r2).unwrap();
    let br = branch_from_run(&net.segments[first2].run, &[[5.0, 6.0, 2.0], [5.0, 7.5, 2.0]], &[round(0.05)], &p2, &off).unwrap();
    net.add_branch(first2, &br).unwrap();
    // two risers: one in the shaft, one through the slab on its own
    let p3 = prefs(Domain::Pipe, System::Sanitary, "SAN");
    net.add_route(&route_path(&[[1.25, 1.25, 0.5], [1.25, 1.25, 7.0]], &[round(0.11)], &p3, &off).unwrap()).unwrap();
    let p4 = prefs(Domain::Pipe, System::DomesticHot, "DHW");
    net.add_route(&route_path(&[[7.0, 5.0, 0.5], [7.0, 5.0, 7.0]], &[round(0.05)], &p4, &off).unwrap()).unwrap();
    // a rectangular duct with an elbow
    let p5 = prefs(Domain::Duct, System::SupplyAir, "SA");
    let sz = RunSize::Rect { width: 0.4, height: 0.3, roll: 0.0 };
    net.add_route(&route_path(&[[3.0, 2.0, 2.4], [9.0, 2.0, 2.4], [9.0, 4.0, 2.4]], &[sz, sz], &p5, &off).unwrap()).unwrap();
    // something on the upper level: a gas pipe at 4.2 m
    let p6 = prefs(Domain::Pipe, System::Gas, "GAS");
    net.add_route(&route_path(&[[1.0, 7.0, 4.2], [4.0, 7.0, 4.2], [4.0, 6.0, 4.2]], &[round(0.08), round(0.08)], &p6, &off).unwrap()).unwrap();
    net.validate().unwrap();

    let mut wall = WallChain::new(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0]], true, 0.2, 3.0);
    wall.fire_rating_min = Some(60);
    wall.openings = vec![vec![]; 4];
    wall.openings[0].push(Opening { offset: if edit { 3.0 } else { 2.0 }, width: 0.9, sill: 0.0, height: 2.1 });
    let runs: Vec<_> = net.segments.iter().map(|s| s.run.clone()).collect();
    let mut upper = WallChain::new(vec![[2.0, 3.0], [6.0, 3.0]], false, 0.15, 3.0);
    upper.base_z = 3.0;
    let model = Model { chains: vec![wall, upper], runs: runs.clone(), merge_gap: 0.1, ..Default::default() };
    let built = model.build().unwrap();

    let levels = Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }, Level { name: "L1".into(), elevation: 3.0 }, Level { name: "L2".into(), elevation: 6.0 }]).unwrap();
    let shaft = Shaft { name: "S1".into(), usage: "plumbing".into(), outline: vec![[0.5, 0.5], [2.0, 0.5], [2.0, 2.0], [0.5, 2.0]], from: 0, to: 2 };
    let void = ShaftVoid::from_shaft(&shaft, &levels).unwrap();
    let slab = Slab { name: "L1 floor".into(), top: 3.0, thickness: 0.25, outline: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0]], fire_rating_min: Some(60) };
    let (floor_pens, issues) = find_floor(std::slice::from_ref(&slab), std::slice::from_ref(&void), &runs, 0.1);
    assert!(issues.is_empty());
    Scene {
        project_name: "Test office".into(),
        project_key: 0x0123_4567_89ab,
        timestamp: "2026-10-15T00:00:00".into(),
        levels,
        walls: vec![WallItem { id: A | 1, name: "Outer wall".into(), chain: built.chains[0].clone() }, WallItem { id: A | 3, name: "Upper wall".into(), chain: built.chains[1].clone() }],
        wall_penetrations: built.penetrations.clone(),
        slabs: vec![SlabItem { id: A | 2, slab }],
        shafts: vec![void],
        floor_penetrations: floor_pens,
        networks: vec![net],
    }
}

fn v(p: [f64; 3]) -> String {
    format!("{:.9} {:.9} {:.9}", p[0], p[1], p[2])
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("usage: ifc_scene OUT.ifc [edit]");
    let edit = args.get(2).is_some_and(|a| a == "edit");
    let scene = build_scene(edit);
    let out = export(&scene).expect("export");
    std::fs::write(path, out.text.as_bytes()).expect("write");
    println!("PROJECT {} {}", scene.project_key, scene.project_name);
    println!("COUNTS walls={} openings={} slabs={} segments={} fittings={} ports={} connections={}", out.report.walls, out.report.openings, out.report.slabs, out.report.segments, out.report.fittings, out.report.ports, out.report.connections);
    for w in &out.report.warnings {
        println!("WARN {w}");
    }
    for l in scene.levels.all() {
        println!("LEVEL {} {}", l.name, l.elevation);
    }
    for w in &scene.walls {
        println!("WALL {} {} {} {} {}", w.id, w.name, w.chain.fire_rating_min.unwrap_or(0), w.chain.thickness, w.chain.height);
        println!("WBASE {} {} {}", w.id, w.chain.base_z, w.chain.closed as u8);
        for p in &w.chain.points {
            println!("WPT {} {} {}", w.id, p[0], p[1]);
        }
    }
    for w in &scene.walls {
        for (si, ops) in w.chain.openings.iter().enumerate() {
            for o in ops {
                println!("OPEN {} {} {} {} {} {}", w.id, si, o.offset, o.width, o.sill, o.height);
            }
        }
    }
    for sh in &scene.shafts {
        let xs = sh.outline.iter().map(|p| p[0]);
        let ys = sh.outline.iter().map(|p| p[1]);
        println!("SHAFT {} {} {} {} {} {} {}", sh.name, xs.clone().fold(f64::MAX, f64::min), xs.fold(f64::MIN, f64::max), ys.clone().fold(f64::MAX, f64::min), ys.fold(f64::MIN, f64::max), sh.z_bottom, sh.z_top);
    }
    for s in &scene.slabs {
        println!("SLAB {} {} {} {} {}", s.id, s.slab.name, s.slab.top, s.slab.thickness, s.slab.fire_rating_min.unwrap_or(0));
    }
    for p in &scene.wall_penetrations {
        println!("WPEN {} {} {} {} {} {}", p.id, p.cut as u8, v(p.centre), p.width, p.height, p.fire_rating_min.unwrap_or(0));
    }
    for p in &scene.floor_penetrations {
        println!("FPEN {} {:?} {} {} {} {} {}", p.id, p.kind, p.x0, p.x1, p.y0, p.y1, p.fits as u8);
    }
    for net in &scene.networks {
        for s in &net.segments {
            println!("SEG {} {} {} {} {}", s.id, s.run.system.ifc_system(), s.run.domain.ifc_port_type(), v(s.run.a), v(s.run.b));
        }
        for f in &net.fittings {
            println!("FIT {} {} {}", f.id, f.fitting.ports.len(), v(f.fitting.at));
        }
        for (i, e) in net.equipment.iter().enumerate() {
            println!("EQ {} {} {}", e.element, i, v(e.pos));
        }
        let port = |p: PortRef| -> (u64, u16) {
            match p.node {
                NodeRef::Segment(i) => (net.segments[i].id, 0x1000 + p.port as u16),
                NodeRef::Fitting(i) => (net.fittings[i].id, 0x1000 + p.port as u16),
                NodeRef::Equipment(i) => {
                    let el = net.equipment[i].element;
                    let k = net.equipment[..i].iter().filter(|x| x.element == el).count();
                    (el, 0x1000 + k as u16)
                }
            }
        };
        let all = (0..net.segments.len()).map(NodeRef::Segment).chain((0..net.fittings.len()).map(NodeRef::Fitting)).chain((0..net.equipment.len()).map(NodeRef::Equipment));
        for n in all {
            for k in 0..match n {
                NodeRef::Segment(_) => 2,
                NodeRef::Fitting(i) => net.fittings[i].fitting.ports.len(),
                NodeRef::Equipment(_) => 1,
            } {
                let pr = PortRef { node: n, port: k };
                let (id, part) = port(pr);
                println!("PORT {} {} {} {}", id, part, v(net.port_pos(pr).unwrap()), v(net.port_dir(pr).unwrap()));
            }
        }
        for (a, b) in net.links() {
            let (ia, pa) = port(*a);
            let (ib, pb) = port(*b);
            println!("LINK {ia} {pa} {ib} {pb}");
        }
    }
}
