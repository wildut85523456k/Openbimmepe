// Draws a small test building as a plan view: `plan_svg <out.svg> [cut height in m]`
use openbim_arch::{Level, Levels};
use openbim_family::library;
use openbim_geom::WallChain;
use openbim_ifc::{FamilyItem, FamilyRef, Frame, Scene, WallItem};
use openbim_place::{Model, Placement};
use openbim_view::*;

fn main() {
    let out = std::env::args().nth(1).expect("output file");
    let cut: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1.2);
    let fams = library::all().unwrap();
    let door = fams.iter().position(|c| c.name() == "Door, single flush").unwrap();
    let window = fams.iter().position(|c| c.name().starts_with("Window")).unwrap();
    let chain = |pts: Vec<[f64; 2]>, closed: bool| WallChain::new(pts, closed, 0.2, 3.0);
    let chains = vec![
        chain(vec![[0.0, 0.0], [8.0, 0.0], [8.0, 6.0], [0.0, 6.0]], true),
        chain(vec![[4.0, 0.1], [4.0, 5.9]], false),
        chain(vec![[0.1, 3.0], [3.9, 3.0]], false),
    ];
    let pl = |family: usize, chain: usize, segment: usize, centre: f64, swing: &'static str| Placement { family, type_name: None, overrides: vec![], texts: if family == door { vec![("swing".to_string(), swing.to_string())] } else { vec![] }, chain, segment, centre };
    let placements = vec![
        pl(door, 0, 0, 2.0, "left"),   // front door
        pl(window, 0, 0, 6.0, "left"),
        pl(window, 0, 2, 4.0, "left"),  // back wall, window
        pl(window, 0, 3, 3.0, "left"),
        pl(door, 1, 0, 1.0, "right"),   // interior doors
        pl(door, 1, 0, 4.8, "left"),
        pl(door, 2, 0, 1.9, "left"),
    ];
    let m = Model { chains, families: fams.clone(), placements, ..Default::default() };
    let b = m.build().unwrap();
    let items: Vec<FamilyItem> = m.placements.iter().enumerate().map(|(k, p)| FamilyItem {
        id: 1000 + k as u64, name: format!("F{k}"), family: p.family, type_name: None,
        frame: Frame::on_wall(&b.chains[p.chain], p.segment, p.centre), instance: b.instances[k].clone(), host: Some((p.chain, p.segment, p.centre)),
    }).collect();
    let scene = Scene {
        project_name: "Test".into(), project_key: 1, timestamp: "t".into(),
        levels: Levels::new(vec![Level { name: "Level 1".into(), elevation: 0.0 }]).unwrap(),
        walls: b.chains.iter().enumerate().map(|(i, c)| WallItem { id: 1 + i as u64, name: format!("W{i}"), chain: c.clone(), wall_type: None, room_bounding: true }).collect(),
        wall_penetrations: vec![], firestops: Default::default(), slabs: vec![], columns: vec![], shafts: vec![], floor_penetrations: vec![], networks: vec![],
        families: fams.into_iter().enumerate().map(|(i, f)| FamilyRef { id: i as u32 + 1, family: f }).collect(), items,
        rooms: vec![], wall_types: vec![], wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], room_lighting: Default::default(), pset_map: Default::default(),
    };
    let v = plan_view(&scene, 0, ViewRange::new(2.3, cut, 0.0, -1.0).unwrap()).unwrap();
    std::fs::write(&out, svg(&v)).unwrap();
    println!("{} shapes -> {out}", v.items.len());
}
