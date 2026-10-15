//! Writes the test building as IFC4 to the path given, and prints what it expects to be in the file, as plain
//! text, for the independent IfcOpenShell check (this text is not a product data path).
use openbim_arch::spaces::Shaft;
use openbim_arch::{Level, Levels, Room};
use openbim_family::forms::{FormDef, ProfileDef};
use openbim_family::{compile_library, library, Axis, Category, Family, NestedDef, SolidDef, ValueDef};
use openbim_family::mep::{Domain, System};
use openbim_family::Request;
use openbim_geom::WallChain;
use openbim_ifc::pset_map::PsetMap;
use openbim_ifc::*;
use openbim_place::floors::{find_floor, ShaftVoid, Slab};
use openbim_place::penetrations::RunSize;
use openbim_place::{Model, Placement};
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
    // a ducted fan coil unit, a placed family with its own connectors
    let mut fams = library::all().unwrap();
    // a made-up furniture family with forms: boxes, a holed extrusion, a revolve with a hole, and a nested part turned 90 degrees
    let r = ProfileDef::rect;
    let c = ProfileDef::circle;
    let part = Family {
        name: "Test part".into(),
        forms: vec![
            FormDef::extrusion("plate", Axis::Z, "0", "0.05", r("0", "0", "0.4", "0.2")).hole(c("0.1", "0.1", "0.03")).hole(r("0.25", "0.05", "0.35", "0.15")),
            FormDef::extrusion("tube", Axis::X, "0.1", "0.5", c("0.1", "0.05", "0.04")).hole(c("0.1", "0.05", "0.03")),
        ],
        category: Category::Generic,
        ..Default::default()
    };
    let stand = Family {
        name: "Test stand".into(),
        values: vec![
            ValueDef::plane("x0", Axis::X, "0"), ValueDef::plane("x1", Axis::X, "0.6"),
            ValueDef::plane("y0", Axis::Y, "0"), ValueDef::plane("y1", Axis::Y, "0.4"),
            ValueDef::plane("z0", Axis::Z, "0"), ValueDef::plane("z1", Axis::Z, "0.1"),
        ],
        solids: vec![SolidDef::new("base", ("x0", "x1"), ("y0", "y1"), ("z0", "z1"))],
        forms: vec![
            FormDef::revolve("knob", Axis::Z, "0.1", true, "0.3", "250", r("0.35", "0.1", "0.45", "0.25")).hole(r("0.38", "0.15", "0.42", "0.2")),
            FormDef::extrusion("rail", Axis::Y, "0.35", "0.4", r("0.05", "0.1", "0.55", "0.3")),
        ],
        nested: vec![NestedDef::new("part", "Test part", ("0.1", "0.05", "0.1")).rotate_about("90", "0.1", "0.05")],
        category: Category::Furniture,
        ..Default::default()
    };
    let extra = compile_library(&[part, stand]).unwrap();
    let stand_family = fams.len() + 1;
    fams.extend(extra);
    let fc_family = 4usize;
    let fc_frame = Frame { origin: [4.5, 3.0, 0.5], x: [1.0, 0.0] };
    let fc_inst = fams[fc_family].solve(&Request::default()).unwrap();
    let find = |n: &str| fc_inst.connectors.iter().find(|c| c.name == n).unwrap().clone();
    let c_in = fc_frame.connector(&find("chw_in"));
    let c_out = fc_frame.connector(&find("chw_out"));
    let p1 = prefs(Domain::Pipe, System::ChilledWaterSupply, "CHWS");
    let z = c_in.pos[2];
    let r1 = route_from_connector(&c_in, &[[12.0, 3.15, z], [12.0, 6.0, z]], round(0.022), &p1, &off).unwrap();
    net.add_route(&r1).unwrap();
    net.attach_equipment(A | 900, &c_in, 0, End::Start).unwrap();
    let p1r = prefs(Domain::Pipe, System::ChilledWaterReturn, "CHWR");
    let first_r = net.segments.len();
    let r1r = route_from_connector(&c_out, &[[12.0, 2.85, z], [12.0, 1.0, z]], round(0.022), &p1r, &off).unwrap();
    net.add_route(&r1r).unwrap();
    net.attach_equipment(A | 900, &c_out, first_r, End::Start).unwrap();
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
    let runs: Vec<_> = net.segments.iter().map(|s| s.run.clone()).collect();
    let mut upper = WallChain::new(vec![[2.0, 3.0], [6.0, 3.0]], false, 0.15, 3.0);
    upper.base_z = 3.0;
    let pl = |family: usize, ty: Option<&str>, texts: &[(&str, &str)], segment: usize, centre: f64| Placement { family, type_name: ty.map(|t| t.to_string()), overrides: vec![], texts: texts.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), chain: 0, segment, centre };
    let d1_centre = if edit { 3.0 } else { 2.0 };
    let placements = vec![
        pl(1, Some("Type A"), &[("mark", "D-01")], 0, d1_centre),
        pl(1, Some("Type A"), &[("mark", "D-02"), ("swing", "left")], 0, 4.5),
        pl(1, Some("Type A"), &[("mark", "D-03")], 0, 7.0),
        pl(2, None, &[("mark", "W-01")], 3, 4.0),
    ];
    let model = Model { chains: vec![wall.clone(), upper.clone()], families: fams.clone(), placements: placements.clone(), runs: runs.clone(), merge_gap: 0.1 };
    let built = model.build().unwrap();
    let mut items: Vec<FamilyItem> = Vec::new();
    for (k, p) in placements.iter().enumerate() {
        let mark = p.texts.iter().find(|(n, _)| n == "mark").map_or(String::new(), |t| t.1.clone());
        items.push(FamilyItem { id: A | (10 + k as u64), name: mark, family: p.family, type_name: p.type_name.clone(), frame: Frame::on_wall(&model.chains[0], p.segment, p.centre), instance: built.instances[k].clone(), host: Some((0, p.segment, p.centre)) });
    }
    let stand_frame = Frame { origin: [1.0, 5.0, 0.0], x: [0.0, 1.0] };
    items.push(FamilyItem { id: A | 950, name: "STAND-01".into(), family: stand_family, type_name: None, frame: stand_frame, instance: fams[stand_family].solve(&Request::default()).unwrap(), host: None });
    items.push(FamilyItem { id: A | 900, name: "FCU-01".into(), family: fc_family, type_name: None, frame: fc_frame, instance: fc_inst.clone(), host: None });

    let levels = Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }, Level { name: "L1".into(), elevation: 3.0 }, Level { name: "L2".into(), elevation: 6.0 }]).unwrap();
    let shaft = Shaft { name: "S1".into(), usage: "plumbing".into(), outline: vec![[0.5, 0.5], [2.0, 0.5], [2.0, 2.0], [0.5, 2.0]], from: 0, to: 2 };
    let void = ShaftVoid::from_shaft(&shaft, &levels).unwrap();
    let slab = Slab { name: "L1 floor".into(), top: 3.0, thickness: 0.25, outline: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0]], fire_rating_min: Some(60) };
    let (floor_pens, issues) = find_floor(std::slice::from_ref(&slab), std::slice::from_ref(&void), &runs, 0.1);
    assert!(issues.is_empty());
    let partition = WallChain::new(vec![[3.5, 7.9], [3.5, 0.1]], false, 0.15, 3.0);
    let chains: Vec<&WallChain> = vec![&built.chains[0], &built.chains[1], &partition];
    // a test position: a UTM-like map, the model turned 12.5 degrees on it, true north 3.25 degrees from the model's Y axis
    let georef = openbim_ifc::georef::Georef {
        crs: openbim_ifc::georef::Crs { name: "TEST:48N".into(), geodetic_datum: Some("TESTDATUM".into()), vertical_datum: Some("TESTVERT".into()), map_projection: Some("TestTM".into()), map_zone: Some("48N".into()) },
        easting: 492_345.678,
        northing: 1_278_901.234,
        height: 12.5,
        grid_angle_deg: 12.5,
        scale: 0.9996,
        true_north_deg: 3.25,
        latitude: 11.556_388_9,
        longitude: 104.928_333_3,
        elevation: 12.5,
    };
    let wall_joins = openbim_ifc::walls::find_joins(&chains, 0.01);
    let layer = |m: &str, t: f64| openbim_ifc::walls::Layer { material: m.into(), thickness: t };
    let wall_types = vec![
        openbim_ifc::walls::WallType { name: "Cavity 200".into(), layers: vec![layer("Brick", 0.1), layer("Insulation", 0.05), layer("Plaster", 0.05)] },
        openbim_ifc::walls::WallType { name: "Partition 150".into(), layers: vec![layer("Gypsum", 0.05), layer("Stud void", 0.05), layer("Gypsum", 0.05)] },
    ];
    Scene {
        project_name: "Test office".into(),
        wall_types,
        wall_joins,
        georef: Some(georef.clone()),
        project_key: 0x0123_4567_89ab,
        timestamp: "2026-10-15T00:00:00".into(),
        levels,
        walls: vec![WallItem { id: A | 1, name: "Outer wall".into(), chain: built.chains[0].clone(), wall_type: Some(0) }, WallItem { id: A | 3, name: "Upper wall".into(), chain: built.chains[1].clone(), wall_type: Some(1) }, WallItem { id: A | 4, name: "Partition".into(), chain: partition.clone(), wall_type: Some(1) }],
        wall_penetrations: built.penetrations.clone(),
        slabs: vec![SlabItem { id: A | 2, slab }],
        shafts: vec![void],
        floor_penetrations: floor_pens,
        networks: vec![net],
        families: fams.iter().enumerate().map(|(i, f)| FamilyRef { id: i as u32 + 1, family: f.clone() }).collect(),
        items,
        rooms: vec![
            RoomItem { id: A | 960, room: Room { name: "Office".into(), usage: "office".into(), occupancy: 6.0, level: 0, outline: vec![[3.575, 0.1], [9.9, 0.1], [9.9, 7.9], [3.575, 7.9]], holes: vec![vec![[4.0, 4.0], [4.0, 5.0], [5.0, 5.0], [5.0, 4.0]]], height: 2.8 } },
            RoomItem { id: A | 961, room: Room { name: "Store".into(), usage: "store".into(), occupancy: 0.0, level: 1, outline: vec![[2.0, 3.075], [6.0, 3.075], [6.0, 7.0], [2.0, 7.0]], holes: vec![], height: 2.8 } },
        ],
        pset_map: PsetMap::from_text("# class\tparameter\tproperty set\tproperty\tkind\nIfcDoor\tmark\tPset_DoorCommon\tReference\tidentifier\nIfcWindow\tmark\tPset_WindowCommon\tReference\tidentifier\n").unwrap(),
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
    for it in &scene.items {
        let fam = &scene.families[it.family].family;
        let vol: f64 = it.instance.solids.iter().map(|(_, b)| b.volume()).sum::<f64>() + it.instance.forms.iter().map(|f| f.volume()).sum::<f64>();
        println!("ITEM {} {} {} {} {} {} {} {}", it.id, fam.category().ifc_class(), it.type_name.clone().unwrap_or_else(|| "-".into()).replace(' ', "_"), v(it.frame.origin), it.frame.x[0], it.frame.x[1], vol, it.name);
        // bounds in the family frame, from the solved boxes (turned by their own xf) and forms
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        let mut take = |p: [f64; 3]| {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        };
        for (n, (_, b)) in it.instance.solids.iter().enumerate() {
            for x in [b.min[0], b.max[0]] {
                for y in [b.min[1], b.max[1]] {
                    for z in [b.min[2], b.max[2]] {
                        let q = it.instance.xf[n].map_or([x, y], |t| t.apply([x, y]));
                        take([q[0], q[1], z]);
                    }
                }
            }
        }
        for f in &it.instance.forms {
            let (l, h, exact) = f.bounds();
            assert!(exact, "the test scene turns forms by quarter turns only");
            take(l);
            take(h);
        }
        println!("IBOX {} {} {}", it.id, v(lo), v(hi));
        if let Some([w, h, sill]) = it.instance.opening {
            println!("IOPEN {} {} {} {}", it.id, w, h, sill);
        }
        for (c, conn) in it.instance.connectors.iter().enumerate() {
            println!("ICON {} {} {} {} {}", it.id, 0x2000 + c, conn.name, v(conn.pos), v(conn.dir));
        }
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
    for (ti, t) in scene.wall_types.iter().enumerate() {
        let layers: Vec<String> = t.layers.iter().map(|l| format!("{}={}", l.material.replace(' ', "_"), l.thickness)).collect();
        println!("WTYPE {} {} {}", ti, t.name.replace(' ', "_"), layers.join(","));
    }
    for w in &scene.walls {
        if let Some(t) = w.wall_type {
            println!("WTYPED {} {}", w.id, t);
        }
    }
    for j in &scene.wall_joins {
        println!("WJOIN {} {:?} {} {:?}", scene.walls[j.a].id, j.a_end, scene.walls[j.b].id, j.b_end);
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
    if let Some(g) = &scene.georef {
        println!("GEOREF {} {} {} {} {} {} {} {} {} {}", g.easting, g.northing, g.height, g.grid_angle_deg, g.scale, g.true_north_deg, g.latitude, g.longitude, g.elevation, g.crs.name);
        for p in [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 8.0, 3.0], [3.5, 7.9, 1.25]] {
            let m = g.to_map(p);
            let back = g.to_local(m);
            println!("GEOPT {} {} {} {} {} {} {} {} {}", p[0], p[1], p[2], m[0], m[1], m[2], back[0], back[1], back[2]);
        }
    }
    for r in &scene.rooms {
        let rm = &r.room;
        println!("ROOM {} {} {} {} {} {}", r.id, rm.level, scene.levels.all()[rm.level].elevation, rm.height, rm.outline.len(), rm.outline.iter().map(|p| format!("{} {}", p[0], p[1])).collect::<Vec<_>>().join(" "));
        let (bs, _) = openbim_ifc::boundaries::room_boundaries(&scene, r);
        let elev = scene.levels.all()[r.room.level].elevation;
        for (bi, b) in bs.iter().enumerate() {
            let (kind, id) = match b.target {
                openbim_ifc::boundaries::Target::Wall(i) => ("WALL", scene.walls[i].id),
                openbim_ifc::boundaries::Target::Slab(i) => ("SLAB", scene.slabs[i].id),
                openbim_ifc::boundaries::Target::Item(i) => ("ITEM", scene.items[i].id),
            };
            let world = |ring: &[[f64; 2]]| ring.iter().map(|p| { let q = b.surface.point(*p); format!("{} {} {}", q[0], q[1], q[2] + elev) }).collect::<Vec<_>>().join(" ");
            println!("SB {} {} {} {} {} {} {} {}", r.id, bi, kind, id, b.parent.map_or(-1, |p| p as i64), b.surface.area(), b.surface.outer.len(), world(&b.surface.outer));
            for h in &b.surface.inner {
                println!("SBHOLE {} {} {} {}", r.id, bi, h.len(), world(h));
            }
            let n = b.surface.axis;
            println!("SBNORMAL {} {} {} {} {}", r.id, bi, n[0], n[1], n[2]);
        }
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
                    let e = &net.equipment[i];
                    let it = scene.items.iter().find(|it| it.id == e.element).expect("equipment is a placed family");
                    let c = it.instance.connectors.iter().position(|c| c.name == e.connector).expect("connector");
                    (e.element, 0x2000 + c as u16)
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
