use openbim_arch::{Level, Levels};
use openbim_family::{library, Request};
use openbim_geom::{Opening, WallChain};
use openbim_ifc::{ColumnItem, FamilyItem, FamilyRef, Frame, Scene, SlabItem, SlabRole, WallItem};
use openbim_place::floors::Slab;
use openbim_view::*;

fn scene(walls: Vec<WallChain>) -> Scene {
    Scene {
        project_name: "P".into(),
        project_key: 1,
        beams: vec![], materials: Default::default(),
        timestamp: "t".into(),
        levels: Levels::new(vec![
            Level {
                name: "L0".into(),
                elevation: 0.0,
            },
            Level {
                name: "L1".into(),
                elevation: 3.0,
            },
        ])
        .unwrap(),
        walls: walls
            .into_iter()
            .enumerate()
            .map(|(i, chain)| WallItem {
                id: 100 + i as u64,
                name: format!("W{i}"),
                chain,
                wall_type: None,
                room_bounding: true,
            })
            .collect(),
        wall_penetrations: vec![],
        firestops: Default::default(),
        slabs: vec![],
        columns: vec![],
        shafts: vec![],
        floor_penetrations: vec![], slab_voids: vec![],
        networks: vec![],
        families: vec![],
        items: vec![],
        rooms: vec![],
        wall_types: vec![],
        wall_joins: vec![],
        georef: None,
        room_settings: Default::default(),
        separation_lines: vec![],
        room_lighting: Default::default(),
        pset_map: Default::default(), types: vec![], member_info: Default::default(), roofs: vec![], stairs: vec![], native: vec![],
    }
}

fn wall_at(y: f64, openings: Vec<Opening>) -> WallChain {
    let mut c = WallChain::new(vec![[0.0, y], [4.0, y]], false, 0.2, 3.0);
    c.openings = vec![openings];
    c
}

/// looking north from y = -5 along a 4 m wide strip: u = x
fn south() -> Section {
    Section {
        a: [0.0, -5.0],
        b: [4.0, -5.0],
        bottom: -0.5,
        top: 4.0,
        depth: 20.0,
    }
}
type Ring = Vec<[f64; 2]>;
fn faces(v: &SectionView, kind: Kind) -> Vec<(u64, Ring, Vec<Ring>)> {
    v.items
        .iter()
        .filter(|i| i.element.kind == kind)
        .filter_map(|i| {
            if let Shape::Region { outer, holes } = &i.shape {
                Some((i.element.id, outer.clone(), holes.clone()))
            } else {
                None
            }
        })
        .collect()
}
fn extent(r: &[[f64; 2]]) -> [f64; 4] {
    r.iter()
        .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
            [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ]
        })
}
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn an_elevation_is_a_face_with_its_openings_as_holes_and_nothing_is_cut() {
    let s = scene(vec![wall_at(
        0.0,
        vec![Opening {
            offset: 1.0,
            width: 1.2,
            sill: 0.9,
            height: 1.2,
        }],
    )]);
    let v = elevation_view(&s, south()).unwrap();
    let f = faces(&v, Kind::Wall);
    assert_eq!(f.len(), 1);
    let e = extent(&f[0].1);
    assert!(
        close(e[0], 0.0) && close(e[2], 4.0) && close(e[1], 0.0) && close(e[3], 3.0),
        "{e:?}"
    );
    let h = extent(&f[0].2[0]);
    assert!(
        close(h[0], 1.0) && close(h[2], 2.2) && close(h[1], 0.9) && close(h[3], 2.1),
        "{h:?}"
    );
    assert!(v
        .items
        .iter()
        .filter(|i| i.element.kind == Kind::Wall)
        .all(|i| i.class == Class::Below));
    // the same wall in a section line standing outside is only an outline: the two views differ
    let sec = section_view(&s, south()).unwrap();
    assert!(faces(&sec, Kind::Wall).is_empty());
}

#[test]
fn nearer_walls_are_painted_after_farther_ones_so_they_hide_them() {
    let s = scene(vec![
        wall_at(8.0, vec![]),
        wall_at(0.0, vec![]),
        wall_at(3.0, vec![]),
    ]);
    let v = elevation_view(&s, south()).unwrap();
    let order: Vec<u64> = faces(&v, Kind::Wall).iter().map(|f| f.0).collect();
    assert_eq!(order, vec![100, 102, 101]); // y = 8, then 3, then 0
                                            // behind the viewer: nothing; beyond the depth: nothing
    let behind = scene(vec![wall_at(-8.0, vec![])]);
    assert!(faces(&elevation_view(&behind, south()).unwrap(), Kind::Wall).is_empty());
    let far = scene(vec![wall_at(0.0, vec![])]);
    assert!(faces(
        &elevation_view(
            &far,
            Section {
                depth: 4.0,
                ..south()
            }
        )
        .unwrap(),
        Kind::Wall
    )
    .is_empty());
}

#[test]
fn doors_and_windows_are_drawn_from_the_boxes_of_their_families_after_the_wall_they_sit_in() {
    let fams = library::all().unwrap();
    let k = fams
        .iter()
        .position(|c| c.name().starts_with("Window"))
        .unwrap();
    let mut chain = wall_at(
        0.0,
        vec![Opening {
            offset: 1.0,
            width: 1.2,
            sill: 0.9,
            height: 1.2,
        }],
    );
    let inst = fams[k].solve(&Request::default()).unwrap();
    let frame = Frame::on_wall(&chain, 0, 1.6);
    chain.openings = vec![vec![Opening {
        offset: 1.0,
        width: 1.2,
        sill: 0.9,
        height: 1.2,
    }]];
    let mut s = scene(vec![chain]);
    s.families = vec![FamilyRef {
        id: 1,
        family: fams[k].clone(),
    }];
    s.items = vec![FamilyItem {
        id: 7,
        name: "W".into(),
        family: 0,
        type_name: None,
        frame,
        instance: inst,
        host: None,
    }];
    let v = elevation_view(&s, south()).unwrap();
    let w = faces(&v, Kind::Wall);
    let fam = faces(&v, Kind::Family);
    assert!(
        fam.len() >= 5,
        "frame members, glass and mullions: {}",
        fam.len()
    );
    // the window lies inside the hole of the wall: every box within u 1.0 to 2.2 and z 0.9 to 2.1
    for (_, r, _) in &fam {
        let e = extent(r);
        assert!(
            e[0] >= 1.0 - 1e-9 && e[2] <= 2.2 + 1e-9 && e[1] >= 0.9 - 1e-9 && e[3] <= 2.1 + 1e-9,
            "{e:?}"
        );
    }
    // the front members of the window are painted after the wall; the glass, deeper in the wall, before it, and shows through the hole
    let last_family = v
        .items
        .iter()
        .rposition(|i| i.element.kind == Kind::Family)
        .unwrap();
    let wall = v
        .items
        .iter()
        .position(|i| i.element.kind == Kind::Wall)
        .unwrap();
    assert!(wall < last_family);
    assert_eq!(w.len(), 1);
}

#[test]
fn slabs_and_columns_in_front_are_faces_and_those_the_line_crosses_are_cut() {
    let mut s = scene(vec![]);
    s.slabs = vec![SlabItem {
        id: 50,
        slab: Slab {
            name: "S".into(),
            top: 3.0,
            thickness: 0.25,
            outline: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 6.0], [0.0, 6.0]],
            fire_rating_min: None,
        },
        role: SlabRole::Structure,
        room_bounding: true,
    }];
    s.columns = vec![ColumnItem {
        id: 80,
        name: "C".into(),
        footprint: vec![[1.0, 1.0], [1.3, 1.0], [1.3, 1.3], [1.0, 1.3]],
        base_z: 0.0,
        height: 3.0,
        room_bounding: false,
    }];
    let v = elevation_view(&s, south()).unwrap();
    let sl = faces(&v, Kind::Slab);
    let e = extent(&sl[0].1);
    assert!(
        sl.len() == 1
            && close(e[0], 0.0)
            && close(e[2], 4.0)
            && close(e[1], 2.75)
            && close(e[3], 3.0),
        "{e:?}"
    );
    let c = faces(&v, Kind::Column);
    let e = extent(&c[0].1);
    assert!(c.len() == 1 && close(e[0], 1.0) && close(e[2], 1.3) && close(e[3], 3.0));
    assert!(v
        .items
        .iter()
        .filter(|i| matches!(i.element.kind, Kind::Slab | Kind::Column))
        .all(|i| i.class == Class::Below));
    // a line through the slab cuts it, as in a section
    let cut = elevation_view(
        &s,
        Section {
            a: [0.0, 3.0],
            b: [4.0, 3.0],
            bottom: -0.5,
            top: 4.0,
            depth: 20.0,
        },
    )
    .unwrap();
    assert!(cut
        .items
        .iter()
        .any(|i| i.element.kind == Kind::Slab && i.class == Class::Cut));
}

#[test]
fn the_four_standard_elevations_look_at_the_building_with_u_growing_to_the_right() {
    let mut s = scene(vec![wall_at(0.0, vec![]), wall_at(6.0, vec![])]);
    s.walls.push(WallItem {
        id: 200,
        name: "E".into(),
        chain: {
            let mut c = WallChain::new(vec![[4.0, 0.0], [4.0, 6.0]], false, 0.2, 3.0);
            c.openings = vec![vec![]];
            c
        },
        wall_type: None,
        room_bounding: true,
    });
    let all = standard_elevations(&s, 2.0).unwrap();
    let names: Vec<Facing> = all.iter().map(|x| x.0).collect();
    assert_eq!(
        names,
        vec![Facing::South, Facing::East, Facing::North, Facing::West]
    );
    // each looks at the building: the nearest wall of that side is a face and the line is outside the model
    for (f, sec) in &all {
        let v = elevation_view(&s, *sec).unwrap();
        assert!(!faces(&v, Kind::Wall).is_empty(), "{f:?}");
        assert!(v.items.iter().all(|i| i.class != Class::Cut), "{f:?}");
        assert!(sec.bottom <= 0.0 && sec.top >= 3.0 && sec.depth >= 6.0);
    }
    // south: standing south of the building, west is on the left, so the east wall (x = 4) is at the right end
    let south = elevation_view(&s, all[0].1).unwrap();
    let e_wall = faces(&south, Kind::Wall)
        .into_iter()
        .find(|f| f.0 == 200)
        .unwrap();
    let e = extent(&e_wall.1);
    let line_len = south.length;
    assert!(
        e[0] > line_len / 2.0,
        "the east wall is on the right: {e:?} of {line_len}"
    );
    // north: turned round, the same wall is on the left
    let north = elevation_view(&s, all[2].1).unwrap();
    let e = extent(
        &faces(&north, Kind::Wall)
            .into_iter()
            .find(|f| f.0 == 200)
            .unwrap()
            .1,
    );
    assert!(e[2] < north.length / 2.0, "{e:?}");
    // no plan, no elevations
    assert!(standard_elevations(&scene(vec![]), 1.0).is_none());
}

#[test]
fn an_elevation_is_the_same_on_any_number_of_threads_and_draws_with_white_faces() {
    let walls: Vec<WallChain> = (0..60)
        .map(|k| {
            wall_at(
                k as f64 * 0.7,
                vec![Opening {
                    offset: 1.0,
                    width: 1.0,
                    sill: 0.0,
                    height: 2.1,
                }],
            )
        })
        .collect();
    let s = scene(walls);
    let one = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap()
        .install(|| elevation_view(&s, south()))
        .unwrap();
    let four = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap()
        .install(|| elevation_view(&s, south()))
        .unwrap();
    assert_eq!(one, four);
    let d = section_svg(&one);
    assert!(d.contains("fill=\"white\" stroke=\"none\"") && d.contains("fill-rule=\"evenodd\""));
    assert_eq!(d, section_svg(&four));
    // the shaded look paints faces light grey
    assert!(section_svg_with(&one, &SvgStyle::shaded()).contains("#f1f1f1"));
}

fn gable() -> openbim_ifc::RoofItem {
    // a closed prism: eaves at z = 3 over 4 x 6, ridge at z = 4 along x at y = 3
    let verts = vec![[0.0, 0.0, 3.0], [4.0, 0.0, 3.0], [4.0, 6.0, 3.0], [0.0, 6.0, 3.0], [0.0, 3.0, 4.0], [4.0, 3.0, 4.0]];
    let raw: Vec<Vec<usize>> = vec![vec![0, 1, 2, 3], vec![0, 1, 5, 4], vec![3, 2, 5, 4], vec![0, 3, 4], vec![1, 2, 5]];
    let c = [2.0, 3.0, 3.4];
    let faces = raw.into_iter().map(|f| {
        let mut n = [0.0; 3];
        for k in 0..f.len() {
            let (a, b) = (verts[f[k]], verts[f[(k + 1) % f.len()]]);
            n[0] += (a[1] - b[1]) * (a[2] + b[2]); n[1] += (a[2] - b[2]) * (a[0] + b[0]); n[2] += (a[0] - b[0]) * (a[1] + b[1]);
        }
        let m = f.iter().fold([0.0; 3], |s, &i| [s[0] + verts[i][0] / f.len() as f64, s[1] + verts[i][1] / f.len() as f64, s[2] + verts[i][2] / f.len() as f64]);
        if n[0] * (m[0] - c[0]) + n[1] * (m[1] - c[1]) + n[2] * (m[2] - c[2]) >= 0.0 { f } else { f.into_iter().rev().collect() }
    }).collect();
    openbim_ifc::RoofItem { id: 500, name: "R".into(), eaves_z: 3.0, verts, faces, shape: "GABLE_ROOF" }
}

fn area(r: &[[f64; 2]]) -> f64 { (0..r.len()).map(|k| r[k][0] * r[(k + 1) % r.len()][1] - r[(k + 1) % r.len()][0] * r[k][1]).sum::<f64>().abs() / 2.0 }

#[test]
fn a_roof_is_seen_in_an_elevation_from_the_side_it_slopes_to_and_cut_by_a_section() {
    let mut s = scene(vec![]);
    s.roofs = vec![gable()];
    // from the south the sloping face toward the viewer is a rectangle 4 wide and 1 high on top of the eaves
    let v = elevation_view(&s, south()).unwrap();
    let f: Vec<_> = v.items.iter().filter(|i| i.element.kind == Kind::Roof).filter_map(|i| if let Shape::Region { outer, .. } = &i.shape { Some(outer.clone()) } else { None }).collect();
    assert!(f.iter().any(|o| (area(o) - 4.0).abs() < 1e-9), "{f:?}");
    // the gable end seen from the east is a triangle 6 wide and 1 high
    let east = Section { a: [10.0, -1.0], b: [10.0, 7.0], bottom: -0.5, top: 6.0, depth: 20.0 };
    let v = elevation_view(&s, east).unwrap();
    let f: Vec<_> = v.items.iter().filter(|i| i.element.kind == Kind::Roof).filter_map(|i| if let Shape::Region { outer, .. } = &i.shape { Some(outer.clone()) } else { None }).collect();
    assert!(f.iter().any(|o| (area(o) - 3.0).abs() < 1e-9), "{f:?}");
    // a section across the ridge cuts a triangle with the roof's profile
    let cut = Section { a: [2.0, -1.0], b: [2.0, 7.0], bottom: -0.5, top: 6.0, depth: 5.0 };
    let v = section_view(&s, cut).unwrap();
    let c: Vec<_> = v.items.iter().filter(|i| i.element.kind == Kind::Roof && i.class == Class::Cut).filter_map(|i| if let Shape::Region { outer, .. } = &i.shape { Some(outer.clone()) } else { None }).collect();
    assert_eq!(c.len(), 1);
    assert!((area(&c[0]) - 3.0).abs() < 1e-9, "{:?}", c[0]);
}

#[test]
fn a_stair_is_cut_in_steps_by_a_section_along_it() {
    let mut s = scene(vec![]);
    s.stairs = vec![openbim_ifc::StairItem { id: 600, name: "S".into(), start: [0.0, 0.0, 0.0], dir: [0.0, 1.0], width: 1.0, risers: 3, riser_height: 0.2, tread_length: 0.3 }];
    // along the run, through the middle of it
    let v = section_view(&s, Section { a: [0.0, -1.0], b: [0.0, 2.0], bottom: -0.5, top: 3.0, depth: 5.0 }).unwrap();
    let c: Vec<_> = v.items.iter().filter(|i| i.element.kind == Kind::Stair && i.class == Class::Cut).filter_map(|i| if let Shape::Region { outer, .. } = &i.shape { Some(outer.clone()) } else { None }).collect();
    assert_eq!(c.len(), 2);
    let total: f64 = c.iter().map(|r| area(r)).sum();
    assert!((total - (0.3 * 0.2 + 0.3 * 0.4)).abs() < 1e-9, "{total}");
}
