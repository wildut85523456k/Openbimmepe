// Draws a small test building as a plan view: `plan_svg <out.svg> [cut height in m] [slab top in m] [look: revit|poche|shaded]`
use openbim_arch::Room;
use openbim_arch::{Level, Levels};
use openbim_family::library;
use openbim_geom::WallChain;
use openbim_ifc::{
    ColumnItem, FamilyItem, FamilyRef, Frame, RoomItem, Scene, SlabItem, SlabRole, WallItem,
};
use openbim_place::floors::{ShaftVoid, Slab};
use openbim_place::{Model, Placement};
use openbim_view::*;

fn room(id: u64, name: &str, usage: &str, o: &[[f64; 2]]) -> RoomItem {
    RoomItem {
        number: String::new(),
        id,
        room: Room {
            name: name.into(),
            usage: usage.into(),
            occupancy: 0.0,
            level: 0,
            outline: o.to_vec(),
            holes: vec![],
            height: 2.7,
        },
    }
}

fn main() {
    let out = std::env::args().nth(1).expect("output file");
    let cut: f64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.2);
    let slab_top: f64 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let look = match std::env::args().nth(4).as_deref() {
        Some("poche") => SvgStyle::poche(),
        Some("shaded") => SvgStyle::shaded(),
        _ => SvgStyle::revit(),
    };
    let fams = library::all().unwrap();
    let door = fams
        .iter()
        .position(|c| c.name() == "Door, single flush")
        .unwrap();
    let window = fams
        .iter()
        .position(|c| c.name().starts_with("Window"))
        .unwrap();
    let chain = |pts: Vec<[f64; 2]>, closed: bool| WallChain::new(pts, closed, 0.2, 3.0);
    let chains = vec![
        chain(vec![[0.0, 0.0], [8.0, 0.0], [8.0, 6.0], [0.0, 6.0]], true),
        chain(vec![[4.0, 0.1], [4.0, 5.9]], false),
        chain(vec![[0.1, 3.0], [3.9, 3.0]], false),
    ];
    let pl =
        |family: usize, chain: usize, segment: usize, centre: f64, swing: &'static str| Placement {
            family,
            type_name: None,
            overrides: vec![],
            texts: if family == door {
                vec![("swing".to_string(), swing.to_string())]
            } else {
                vec![]
            },
            chain,
            segment,
            centre,
        };
    let placements = vec![
        pl(door, 0, 0, 2.0, "left"), // front door
        pl(window, 0, 0, 6.0, "left"),
        pl(window, 0, 2, 4.0, "left"), // back wall, window
        pl(window, 0, 3, 3.0, "left"),
        pl(door, 1, 0, 1.0, "right"), // interior doors
        pl(door, 1, 0, 4.8, "left"),
        pl(door, 2, 0, 1.9, "left"),
    ];
    let m = Model {
        chains,
        families: fams.clone(),
        placements,
        ..Default::default()
    };
    let b = m.build().unwrap();
    let items: Vec<FamilyItem> = m
        .placements
        .iter()
        .enumerate()
        .map(|(k, p)| FamilyItem {
            id: 1000 + k as u64,
            name: format!("F{k}"),
            family: p.family,
            type_name: None,
            frame: Frame::on_wall(&b.chains[p.chain], p.segment, p.centre),
            instance: b.instances[k].clone(),
            host: Some((p.chain, p.segment, p.centre)),
        })
        .collect();
    let scene = Scene {
        project_name: "Test".into(),
        project_key: 1,
        beams: vec![], materials: Default::default(),
        timestamp: "t".into(),
        levels: Levels::new(vec![Level {
            name: "Level 1".into(),
            elevation: 0.0,
        }])
        .unwrap(),
        walls: b
            .chains
            .iter()
            .enumerate()
            .map(|(i, c)| WallItem {
                id: 1 + i as u64,
                name: format!("W{i}"),
                chain: c.clone(),
                wall_type: None,
                room_bounding: true,
            })
            .collect(),
        wall_penetrations: vec![],
        firestops: Default::default(),
        slabs: vec![SlabItem {
            id: 60,
            slab: Slab {
                name: "Floor".into(),
                top: slab_top,
                thickness: 0.25,
                outline: vec![[-0.1, -0.1], [8.1, -0.1], [8.1, 6.1], [-0.1, 6.1]],
                fire_rating_min: None,
            },
            role: SlabRole::Structure,
            room_bounding: true,
        }],
        columns: vec![ColumnItem {
            id: 80,
            name: "C1".into(),
            footprint: vec![[7.4, 0.4], [7.7, 0.4], [7.7, 0.7], [7.4, 0.7]],
            base_z: 0.0,
            height: 3.0,
            room_bounding: false,
        }],
        shafts: vec![ShaftVoid {
            name: "Shaft".into(),
            outline: vec![[6.5, 4.5], [7.5, 4.5], [7.5, 5.5], [6.5, 5.5]],
            z_bottom: -3.0,
            z_top: 3.0,
        }],
        floor_penetrations: vec![], slab_voids: vec![],
        networks: vec![],
        families: fams
            .into_iter()
            .enumerate()
            .map(|(i, f)| FamilyRef {
                id: i as u32 + 1,
                family: f,
            })
            .collect(),
        items,
        rooms: vec![
            room(
                1,
                "Office",
                "office",
                &[[0.1, 0.1], [3.9, 0.1], [3.9, 2.9], [0.1, 2.9]],
            ),
            room(
                2,
                "Store",
                "storage",
                &[[0.1, 3.1], [3.9, 3.1], [3.9, 5.9], [0.1, 5.9]],
            ),
            room(
                3,
                "Hall",
                "circulation",
                &[[4.1, 0.1], [7.9, 0.1], [7.9, 5.9], [4.1, 5.9]],
            ),
        ],
        wall_types: vec![],
        wall_joins: vec![],
        georef: None,
        room_settings: Default::default(),
        separation_lines: vec![],
        room_lighting: Default::default(),
        proxies: vec![], pset_map: Default::default(), grids: vec![], system_defs: vec![], assemblies: vec![], types: vec![], member_info: Default::default(), roofs: vec![], railings: vec![], stairs: vec![], native: vec![],
    };
    let v = plan_view(&scene, 0, ViewRange::new(2.3, cut, 0.0, -1.0).unwrap()).unwrap();
    std::fs::write(&out, svg_with(&v, &look)).unwrap();
    // a section along x = 3 from the front to the back of the building, looking towards -x... the left of the line
    let sv = section_view(
        &scene,
        Section {
            a: [3.0, -1.0],
            b: [3.0, 7.0],
            bottom: -1.0,
            top: 3.6,
            depth: 10.0,
        },
    )
    .unwrap();
    let sec_out = out.replace(".svg", ".section.svg");
    std::fs::write(&sec_out, section_svg_with(&sv, &look)).unwrap();
    for (facing, sec) in standard_elevations(&scene, 1.0).unwrap() {
        let ev = elevation_view(&scene, sec).unwrap();
        let name = out.replace(
            ".svg",
            &format!(".elevation-{}.svg", format!("{facing:?}").to_lowercase()),
        );
        std::fs::write(&name, section_svg_with(&ev, &look)).unwrap();
    }
    println!(
        "section: {} shapes, {:?} -> {sec_out}",
        sv.items.len(),
        sv.warnings
    );
    println!("{} shapes -> {out}", v.items.len());
}
