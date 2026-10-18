//! `render <out-prefix>`: draw a small building in the four styles from the default {3D} view, as PPM files.
use openbim_arch::{Level, Levels};
use openbim_geom::{Opening, WallChain};
use openbim_ifc::{ColumnItem, Scene, SlabItem, SlabRole, WallItem};
use openbim_place::floors::Slab;
use openbim_family::{library, Request};
use openbim_ifc::{FamilyItem, FamilyRef, Frame};
use openbim_place::penetrations::Run;
use openbim_route::graph::{NetSegment, Network};
use openbim_view::{Batch, MeshCache};
use openbim_viewport::*;

fn main() {
    let out = std::env::args().nth(1).expect("render <out-prefix>");
    let mut ring = WallChain::new(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 7.0], [0.0, 7.0]], true, 0.3, 3.0);
    ring.openings = vec![vec![Opening { offset: 2.0, width: 1.2, sill: 0.9, height: 1.2 }, Opening { offset: 6.0, width: 1.2, sill: 0.9, height: 1.2 }], vec![], vec![], vec![Opening { offset: 2.5, width: 1.0, sill: 0.0, height: 2.1 }]];
    let mut inner = WallChain::new(vec![[5.0, 0.0], [5.0, 7.0]], false, 0.15, 3.0);
    inner.tops = vec![Some(vec![[0.0, 3.0], [3.5, 3.6], [7.0, 3.0]])];
    let mut s = Scene {
        project_name: "P".into(), project_key: 1, beams: vec![], materials: Default::default(), timestamp: "t".into(),
        levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(),
        walls: vec![WallItem { id: 1, name: "outer".into(), chain: ring, wall_type: None, room_bounding: true }, WallItem { id: 2, name: "inner".into(), chain: inner, wall_type: None, room_bounding: true }],
        wall_penetrations: vec![], firestops: Default::default(),
        slabs: vec![SlabItem { id: 3, slab: Slab { name: "S".into(), top: 0.0, thickness: 0.25, outline: vec![[-0.3, -0.3], [10.3, -0.3], [10.3, 7.3], [-0.3, 7.3]], fire_rating_min: None }, role: SlabRole::Structure, room_bounding: true }],
        columns: vec![ColumnItem { id: 4, name: "C".into(), footprint: vec![[8.0, 3.0], [8.4, 3.0], [8.4, 3.4], [8.0, 3.4]], base_z: 0.0, height: 3.0, room_bounding: false }],
        shafts: vec![], floor_penetrations: vec![], networks: vec![], families: vec![], items: vec![], rooms: vec![], wall_types: vec![], wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], room_lighting: Default::default(), pset_map: Default::default(), types: vec![], member_info: Default::default(), native: vec![],
    };
    // a ducted fan coil under the ceiling of the left room, a door and a window in the walls, and the pipes and duct
    // that leave its connectors
    let fams = library::all().unwrap();
    let pick = |name: &str| fams.iter().position(|c| c.name().starts_with(name)).unwrap();
    let fc = pick("Fan");
    let inst = fams[fc].solve(&Request::default()).unwrap();
    let frame = Frame { origin: [2.5, 3.5, 2.4], x: [1.0, 0.0] };
    let mut net = Network::new(1);
    for (i, c) in inst.connectors.iter().enumerate() {
        let w = frame.point(c.pos);
        let d = [frame.x[0] * c.dir[0] - frame.x[1] * c.dir[1], frame.x[1] * c.dir[0] + frame.x[0] * c.dir[1], c.dir[2]];
        let end = [w[0] + d[0] * 1.2, w[1] + d[1] * 1.2, w[2] + d[2] * 1.2];
        let run = match c.size {
            openbim_family::mep::Size::Rect { width, height } => Run::rect(&c.name, c.domain, c.system, w, end, width, height),
            openbim_family::mep::Size::Round { diameter } => Run::round(&c.name, c.domain, c.system, w, end, diameter),
            _ => continue,
        };
        net.segments.push(NetSegment { id: 10 + i as u64, run });
    }
    s.networks = vec![net];
    s.families = fams.iter().map(|c| FamilyRef { id: 1, family: c.clone() }).collect();
    s.items = vec![FamilyItem { id: 20, name: "FCU".into(), family: fc, type_name: None, frame, instance: inst, host: None }];
    let meshes = MeshCache::new().update(&s).unwrap().meshes;
    let b = Batch::new(&meshes);
    let mut cam = Camera::default_3d([0.0; 3], 1.0);
    cam.fit([0.0, 0.0, 0.0], [10.0, 7.0, 3.6], 4.0 / 3.0);
    for (name, style) in [("wireframe", Style::Wireframe), ("hidden", Style::HiddenLine), ("shaded", Style::Shaded), ("consistent", Style::ConsistentColors)] {
        let img = render(&b, &cam, 800, 600, style);
        std::fs::write(format!("{out}-{name}.ppm"), img.ppm()).unwrap();
    }
}
