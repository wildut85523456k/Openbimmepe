use openbim_ifc::guid::*;
use openbim_ifc::step::{real, string};
use openbim_ifc::*;
use openbim_arch::{Level, Levels};
use openbim_geom::WallChain;

#[test]
fn global_id_text_matches_the_reference_encoder() {
    // values produced by IfcOpenShell's own encoder
    assert_eq!(compress(0), "0000000000000000000000");
    assert_eq!(compress(u128::MAX), "3$$$$$$$$$$$$$$$$$$$$$");
    assert_eq!(compress(0x0123456789abcdef0123456789abcdef), "018qLdYQlDxm4ZHMU9gytl");
    assert_eq!(expand("018qLdYQlDxm4ZHMU9gytl"), Some(0x0123456789abcdef0123456789abcdef));
}

#[test]
fn global_ids_roundtrip_and_never_collide_across_parts_elements_and_projects() {
    for v in [0u128, 1, 1 << 64, (1 << 127) + 5, u128::MAX - 1] {
        assert_eq!(expand(&compress(v)), Some(v));
    }
    assert_eq!(expand("short"), None);
    assert_eq!(expand("4000000000000000000000"), None, "first character is at most 3");
    assert_eq!(expand("0000000000000000000!00"), None);
    let mut seen = std::collections::HashSet::new();
    for key in [0u64, 1, MAX_PROJECT_KEY] {
        for part in [0u16, 1, 0x1000, u16::MAX] {
            for el in [0u64, 1, 1 << 32, u64::MAX] {
                assert!(seen.insert(global_id(key, part, el)));
            }
        }
    }
    // the key is masked to 48 bits, so a key past that cannot spill into the part field
    assert_eq!(bits(MAX_PROJECT_KEY + 1, 0, 0), 0);
    assert_eq!(bits(5, 7, 9), (5u128 << 80) | (7u128 << 64) | 9);
}

#[test]
fn reals_and_strings_follow_step_rules() {
    assert_eq!(real(0.0), "0.");
    assert_eq!(real(-0.0), "0.");
    assert_eq!(real(1.0), "1.0");
    assert_eq!(real(1e-7), "1.E-7");
    assert_eq!(real(1.5e-7), "1.5E-7");
    assert_eq!(real(2.5e20), "2.5E20");
    assert_eq!(real(-3.25), "-3.25");
    assert_eq!(real(0.1 + 0.2).parse::<f64>().unwrap(), 0.1 + 0.2);
    assert_eq!(string("a'b"), "'a''b'");
    assert_eq!(string("a\\b"), "'a\\\\b'");
    assert_eq!(string("Café"), "'Caf\\X2\\00E9\\X0\\'");
    assert_eq!(string("😀"), "'\\X4\\0001F600\\X0\\'");
    assert_eq!(string("é😀é"), "'\\X2\\00E9\\X0\\\\X4\\0001F600\\X0\\\\X2\\00E9\\X0\\'");
    assert_eq!(string("a😀b"), "'a\\X4\\0001F600\\X0\\b'");
    assert_eq!(string("é1é"), "'\\X2\\00E9\\X0\\1\\X2\\00E9\\X0\\'");
    assert_eq!(string("a\nb"), "'a\\X2\\000A\\X0\\b'");
}

fn levels() -> Levels {
    Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap()
}
fn scene(walls: Vec<WallItem>) -> Scene {
    Scene { types: vec![], member_info: Default::default(), roofs: vec![], railings: vec![], stairs: vec![], native: vec![], project_name: "P".into(), project_key: 1, beams: vec![], materials: Default::default(), timestamp: "t".into(), levels: levels(), walls, wall_penetrations: vec![], firestops: Default::default(), slabs: vec![], columns: vec![], shafts: vec![], floor_penetrations: vec![], slab_voids: vec![], networks: vec![], families: vec![], items: vec![], rooms: vec![], wall_types: vec![], wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], room_lighting: Default::default(), pset_map: Default::default(), grids: vec![], system_defs: vec![], assemblies: vec![] }
}
fn wall(id: u64) -> WallItem {
    WallItem { id, name: "W".into(), chain: WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.2, 3.0), wall_type: None, room_bounding: true }
}

#[test]
fn a_plain_wall_exports_and_exports_again_identically() {
    let a = export(&scene(vec![wall(5)])).unwrap();
    let b = export(&scene(vec![wall(5)])).unwrap();
    assert_eq!(a.text, b.text);
    assert_eq!(a.report.walls, 1);
    assert!(a.text.starts_with("ISO-10303-21;\nHEADER;"));
    assert!(a.text.contains("FILE_SCHEMA(('IFC4'))"));
    // IFC 4.3 is the same entities under the newer schema name, and as repeatable
    let n = openbim_ifc::export_ifc4x3(&scene(vec![wall(5)])).unwrap();
    assert!(n.text.contains("FILE_SCHEMA(('IFC4X3_ADD2'))") && !n.text.contains("FILE_SCHEMA(('IFC4'))"));
    assert_eq!(n.text.replace("IFC4X3_ADD2", "IFC4").lines().skip(3).collect::<Vec<_>>(), a.text.lines().skip(3).collect::<Vec<_>>());
    assert!(a.text.contains(&format!("IFCWALL('{}'", global_id(1, 0, 5))));
    assert!(a.text.trim_end().ends_with("END-ISO-10303-21;"));
}

#[test]
fn bad_scenes_are_refused() {
    let mut s = scene(vec![wall(5)]);
    s.levels = Levels::default();
    assert_eq!(export(&s).err(), Some(ExportError::NoLevels));
    let mut s = scene(vec![wall(5)]);
    s.project_key = MAX_PROJECT_KEY + 1;
    assert_eq!(export(&s).err(), Some(ExportError::BadKey));
    assert_eq!(export(&scene(vec![wall(5), wall(5)])).err(), Some(ExportError::DuplicateId(5)));
    let mut w = wall(6);
    w.chain.points[1][0] = f64::NAN;
    assert!(export(&scene(vec![w])).is_err());
    let mut w = wall(7);
    w.chain.tops = vec![Some(vec![[0.0, 3.0], [4.0, 3.0]])];
    assert!(matches!(export(&scene(vec![w])).err(), Some(ExportError::Unsupported(_))));
}

#[test]
fn an_edited_wall_keeps_its_id_and_a_new_one_gets_its_own() {
    let a = export(&scene(vec![wall(5)])).unwrap();
    let mut w = wall(5);
    w.chain.height = 3.5;
    let b = export(&scene(vec![w, wall(6)])).unwrap();
    let id5 = format!("IFCWALL('{}'", global_id(1, 0, 5));
    assert!(a.text.contains(&id5) && b.text.contains(&id5));
    assert!(b.text.contains(&format!("IFCWALL('{}'", global_id(1, 0, 6))));
    assert_ne!(a.text, b.text);
}

fn close3(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < 1e-12)
}

#[test]
fn a_frame_on_a_wall_turns_and_shifts_points_and_directions() {
    let c = WallChain::new(vec![[1.0, 1.0], [1.0, 5.0]], false, 0.2, 3.0);
    let f = Frame::on_wall(&c, 0, 1.5);
    assert!(close3(f.origin, [1.0, 2.5, 0.0]));
    assert!((f.x[0]).abs() < 1e-12 && (f.x[1] - 1.0).abs() < 1e-12);
    // local x runs up the wall, local y is to its left (towards -x)
    assert!(close3(f.point([2.0, 0.0, 0.5]), [1.0, 4.5, 0.5]));
    assert!(close3(f.point([0.0, 1.0, 0.0]), [0.0, 2.5, 0.0]));
    assert!(close3(f.vector([0.0, 1.0, 1.0]), [-1.0, 0.0, 1.0]));
    // a raised wall base lifts the frame
    let mut c2 = c.clone();
    c2.base_z = 3.0;
    assert!(close3(Frame::on_wall(&c2, 0, 0.0).origin, [1.0, 1.0, 3.0]));
}

fn library_scene(types: &[(Option<&str>, &str)]) -> Scene {
    use openbim_family::{library, Request};
    let fams = library::all().unwrap();
    let mut items = Vec::new();
    for (k, (t, name)) in types.iter().enumerate() {
        let inst = fams[1].solve(&Request { type_name: *t, ..Default::default() }).unwrap();
        items.push(FamilyItem { id: 10 + k as u64, name: (*name).into(), family: 1, type_name: t.map(|s| s.to_string()), frame: Frame { origin: [k as f64 * 2.0, 0.0, 0.0], x: [1.0, 0.0] }, instance: inst, host: None });
    }
    let mut s = scene(vec![wall(5)]);
    s.families = fams.iter().enumerate().map(|(i, f)| FamilyRef { id: i as u32 + 1, family: f.clone() }).collect();
    s.items = items;
    s
}

#[test]
fn occurrences_of_one_type_share_one_type_entity_and_one_map() {
    let out = export(&library_scene(&[(Some("Type A"), "D1"), (Some("Type A"), "D2"), (Some("Type B"), "D3")])).unwrap();
    assert_eq!((out.report.items, out.report.types, out.report.mapped), (3, 2, 3));
    assert_eq!(out.text.matches("IFCDOORTYPE(").count(), 2);
    assert_eq!(out.text.matches("IFCREPRESENTATIONMAP(").count(), 2);
    assert_eq!(out.text.matches("IFCRELDEFINESBYTYPE(").count(), 2);
    assert_eq!(out.text.matches("IFCMAPPEDITEM(").count(), 3);
    assert_eq!(out.text.matches("IFCDOOR(").count(), 3);
    // the type id comes from the family number and the type's place in the family, not from the occurrences
    let a = global_id(1, 0, (0xFFFF_FFFEu64 << 32) | (2u64 << 16) | 1);
    let b = global_id(1, 0, (0xFFFF_FFFEu64 << 32) | (2u64 << 16) | 2);
    assert!(out.text.contains(&format!("IFCDOORTYPE('{a}'")) && out.text.contains(&format!("IFCDOORTYPE('{b}'")));
    // the same input writes the same file; removing the first occurrence keeps the type ids
    let again = export(&library_scene(&[(Some("Type A"), "D1"), (Some("Type A"), "D2"), (Some("Type B"), "D3")])).unwrap();
    assert_eq!(out.text, again.text);
    let fewer = export(&library_scene(&[(Some("Type B"), "D3")])).unwrap();
    assert!(fewer.text.contains(&format!("IFCDOORTYPE('{b}'")));
}

#[test]
fn an_occurrence_with_another_shape_keeps_its_own_bodies() {
    use openbim_family::Request;
    let mut s = library_scene(&[(Some("Type A"), "D1"), (Some("Type A"), "D2")]);
    let left = s.families[1].family.solve(&Request { type_name: Some("Type A"), texts: vec![("swing", "left")], ..Default::default() }).unwrap();
    s.items[1].instance = left;
    let out = export(&s).unwrap();
    assert_eq!((out.report.types, out.report.mapped), (1, 1));
    assert_eq!(out.text.matches("IFCMAPPEDITEM(").count(), 1);
    assert!(out.text.contains("SINGLE_SWING_LEFT") && out.text.contains("SINGLE_SWING_RIGHT"));
}

#[test]
fn a_missing_family_is_refused() {
    let mut s = library_scene(&[(None, "D1")]);
    s.items[0].family = 99;
    assert!(matches!(export(&s).err(), Some(ExportError::Unsupported(_))));
}

#[test]
fn occurrences_differing_only_in_a_form_or_a_turn_keep_their_own_bodies() {
    use openbim_family::forms::{FormDef, ProfileDef};
    use openbim_family::{compile_library, Axis, Category, Family, Request, Scope, SolidDef, ValueDef};
    let f = Family {
        name: "Bench".into(),
        values: vec![
            ValueDef::input("w", 0.4, 0.1, 1.0, Scope::Instance),
            ValueDef::angle("a", 0.0, 0.0, 90.0, Scope::Instance),
            ValueDef::plane("x0", Axis::X, "0"), ValueDef::plane("x1", Axis::X, "0.5"),
            ValueDef::plane("y0", Axis::Y, "0"), ValueDef::plane("y1", Axis::Y, "0.2"),
            ValueDef::plane("z0", Axis::Z, "0"), ValueDef::plane("z1", Axis::Z, "0.1"),
        ],
        solids: vec![SolidDef::new("seat", ("x0", "x1"), ("y0", "y1"), ("z0", "z1")).rotate_about("a", "0.1", "0.1")],
        forms: vec![FormDef::extrusion("leg", Axis::Z, "0", "0.3", ProfileDef::rect("0", "0", "w", "0.1"))],
        category: Category::Furniture,
        ..Default::default()
    };
    let c = compile_library(std::slice::from_ref(&f)).unwrap();
    let mk = |ov: Vec<(&str, f64)>| c[0].solve(&Request { overrides: ov, ..Default::default() }).unwrap();
    let mut s = scene(vec![wall(5)]);
    s.families = vec![FamilyRef { id: 1, family: c[0].clone() }];
    let item = |id: u64, inst| FamilyItem { id, name: format!("B{id}"), family: 0, type_name: None, frame: Frame { origin: [id as f64, 0.0, 0.0], x: [1.0, 0.0] }, instance: inst, host: None };
    // same everything: shared; another leg width: own bodies; another turn: own bodies
    s.items = vec![item(10, mk(vec![])), item(11, mk(vec![])), item(12, mk(vec![("w", 0.3)])), item(13, mk(vec![("a", 30.0)]))];
    let out = export(&s).unwrap();
    assert_eq!(out.report.items, 4);
    assert_eq!(out.report.mapped, 2, "only the two identical benches share the map");
}

#[test]
fn mapped_parameters_go_to_their_standard_set_and_the_rest_stay_custom() {
    use openbim_ifc::pset_map::PsetMap;
    let mut s = library_scene(&[(Some("Type A"), "D1")]);
    s.pset_map = PsetMap::from_text("IfcDoor\thandle_height\tPset_DoorCommon\tHandleHeight\tpositive_length\nIfcDoor\topen_angle\tPset_DoorCommon\tAngle\tangle\n").unwrap();
    let out = export(&s).unwrap();
    assert_eq!(out.report.mapped_properties, 2);
    assert!(out.report.warnings.is_empty(), "{:?}", out.report.warnings);
    assert!(out.text.contains("IFCPROPERTYSET(") && out.text.contains("'Pset_DoorCommon'"));
    assert!(out.text.contains("'HandleHeight',$,IFCPOSITIVELENGTHMEASURE(1.0)"));
    assert!(out.text.contains("'Angle',$,IFCPLANEANGLEMEASURE(0.)"));
    // the mapped one is gone from the custom type set
    assert!(!out.text.contains("'handle_height'"));
    assert!(out.text.contains("'OpenBIM_Family'"));
    // without a table, the same value stays in the custom set
    let plain = export(&library_scene(&[(Some("Type A"), "D1")])).unwrap();
    assert!(plain.text.contains("'handle_height'") && !plain.text.contains("Pset_DoorCommon"));
    assert_eq!(plain.report.mapped_properties, 0);
}

#[test]
fn a_value_that_does_not_fit_its_row_stays_custom_with_a_warning() {
    use openbim_ifc::pset_map::PsetMap;
    let mut s = library_scene(&[(Some("Type A"), "D1")]);
    // 'frame' is a length, not a count; 'open_angle' is an angle, not a length
    s.pset_map = PsetMap::from_text("IfcDoor\tframe\tPset_DoorCommon\tFrameCount\tinteger\n*\topen_angle\tPset_X\tLen\tpositive_length\n").unwrap();
    let out = export(&s).unwrap();
    assert_eq!(out.report.mapped_properties, 0);
    assert_eq!(out.report.warnings.len(), 2, "{:?}", out.report.warnings);
    assert!(out.report.warnings.iter().any(|w| w.contains("frame") && w.contains("is a length")));
    assert!(out.report.warnings.iter().any(|w| w.contains("open_angle") && w.contains("is an angle")));
    assert!(out.text.contains("'frame'") && !out.text.contains("'FrameCount'"));
}

#[test]
fn door_and_window_get_base_quantities_from_their_opening() {
    let out = export(&library_scene(&[(Some("Type A"), "D1")])).unwrap();
    assert_eq!(out.text.matches("'Qto_DoorBaseQuantities'").count(), 1);
    assert!(out.text.contains("IFCQUANTITYAREA('Area',$,$,"));
    assert_eq!(out.report.quantity_sets, 2, "the wall and the door");
}

#[test]
fn walls_slabs_and_rooms_get_base_quantities() {
    use openbim_arch::Room;
    use openbim_place::floors::Slab;
    let mut s = scene(vec![wall(5)]);
    s.slabs = vec![SlabItem { id: 6, slab: Slab { name: "F".into(), top: 0.0, thickness: 0.2, outline: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]], fire_rating_min: None }, room_bounding: true, role: Default::default() }];
    s.rooms = vec![RoomItem { id: 7, number: String::new(), room: Room { name: "R".into(), usage: "office".into(), occupancy: 1.0, level: 0, outline: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]], holes: vec![vec![[1.0, 1.0], [1.0, 2.0], [2.0, 2.0], [2.0, 1.0]]], height: 2.5 } }];
    let out = export(&s).unwrap();
    assert_eq!((out.report.walls, out.report.slabs, out.report.spaces, out.report.quantity_sets), (1, 1, 1, 3));
    for n in ["Qto_WallBaseQuantities", "Qto_SlabBaseQuantities", "Qto_SpaceBaseQuantities"] {
        assert_eq!(out.text.matches(&format!("'{n}'")).count(), 1, "{n}");
    }
    assert!(out.text.contains("IFCARBITRARYPROFILEDEFWITHVOIDS("));
    assert!(out.text.contains("IFCSPACE("));
    // net space volume: (16 - 1) * 2.5
    assert!(out.text.contains("IFCQUANTITYVOLUME('NetVolume',$,$,37.5,$)"), "{}", out.text.lines().filter(|l| l.contains("NetVolume")).collect::<Vec<_>>().join("\n"));
    // the room is written twice the same
    assert_eq!(out.text, export(&s).unwrap().text);
    // a room on a level that does not exist is refused
    s.rooms[0].room.level = 3;
    assert!(matches!(export(&s), Err(ExportError::Unsupported(_))));
}

#[test]
fn slab_net_quantities_take_off_shafts_and_cut_openings() {
    use openbim_arch::spaces::Shaft;
    use openbim_place::floors::{ShaftVoid, Slab};
    let mut s = scene(vec![wall(5)]);
    let levels = Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }, Level { name: "L1".into(), elevation: 3.0 }, Level { name: "L2".into(), elevation: 6.0 }]).unwrap();
    s.levels = levels.clone();
    let shaft = Shaft { name: "S".into(), usage: "x".into(), outline: vec![[1.0, 1.0], [3.0, 1.0], [3.0, 2.0], [1.0, 2.0]], from: 0, to: 2 };
    s.shafts = vec![ShaftVoid::from_shaft(&shaft, &levels).unwrap()];
    s.slabs = vec![SlabItem { id: 6, slab: Slab { name: "F".into(), top: 3.0, thickness: 0.2, outline: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0]], fire_rating_min: None }, room_bounding: true, role: Default::default() }];
    let out = export(&s).unwrap();
    // 80 - 2 (shaft) = 78 net; the volume is 78 x 0.2 = 15.6
    assert!(out.text.contains("IFCQUANTITYAREA('NetArea',$,$,78.0,$)"));
    assert!(out.text.contains("IFCQUANTITYVOLUME('NetVolume',$,$,15.6"));
    assert!(out.text.contains("IFCQUANTITYVOLUME('GrossVolume',$,$,16.0,$)"));
}

#[test]
fn quantities_with_a_unit_are_written_as_that_measure() {
    use openbim_family::{library, Request};
    let fams = library::all().unwrap();
    let mut s = library_scene(&[(Some("Type A"), "D1")]);
    s.items[0].instance = fams[1].solve(&Request { type_name: Some("Type A"), overrides: vec![("open_angle", 90.0)], ..Default::default() }).unwrap();
    let out = export(&s).unwrap();
    // the angle is held in degrees and written in radians; lengths are lengths
    assert!(out.text.contains(&format!("'open_angle',$,IFCPLANEANGLEMEASURE({})", real(std::f64::consts::FRAC_PI_2))), "{}", out.text.lines().filter(|l| l.contains("open_angle")).collect::<Vec<_>>().join("\n"));
    assert!(out.text.contains("'handle_height',$,IFCLENGTHMEASURE(1.0)"));
    assert!(!out.text.contains("'handle_height',$,IFCREAL"));
}

#[test]
fn the_ifc2x3_export_has_the_2x3_header_an_owner_history_and_the_same_ids() {
    let a = export_ifc2x3(&scene(vec![wall(5)])).unwrap();
    let b = export_ifc2x3(&scene(vec![wall(5)])).unwrap();
    assert_eq!(a.text, b.text);
    assert!(a.text.contains("FILE_SCHEMA(('IFC2X3'))") && !a.text.contains("IFC4"));
    // the wall keeps the id it has in the IFC4 file, and points at the owner history; it has no predefined type
    let oh = a.text.lines().find(|l| l.contains("=IFCOWNERHISTORY(")).and_then(|l| l.split('=').next()).unwrap().to_string();
    let w = format!("IFCWALL('{}',{oh},'W',$,$,", global_id(1, 0, 5));
    assert!(a.text.lines().any(|l| l.contains(&w) && l.ends_with(",$);")), "wall line");
    // every rooted entity has the owner history, none has $ there
    let mut rooted = 0;
    for l in a.text.lines() {
        let Some((_, body)) = l.split_once('=') else { continue };
        let Some((_, after)) = body.split_once("('") else { continue };
        if after.len() > 25 && after.as_bytes()[22] == b'\'' && !body.starts_with("IFCPROPERTYSINGLEVALUE") && !body.starts_with("IFCQUANTITY") {
            rooted += 1;
            assert!(after[23..].starts_with(",#"), "rooted entity without owner history: {l}");
        }
    }
    assert!(rooted >= 5, "{rooted}");
    // without a map conversion
    assert!(!a.text.contains("IFCMAPCONVERSION"));
}

#[test]
fn the_ifc2x3_export_writes_doors_and_windows_as_plain_boxes_and_counts_what_it_leaves_out() {
    use openbim_family::{library, Category, Request};
    let fams = library::all().unwrap();
    let door = fams.iter().position(|f| f.category() == Category::Door).unwrap();
    let other = fams.iter().position(|f| !matches!(f.category(), Category::Door | Category::Window)).unwrap();
    let mut s = library_scene(&[]);
    for (k, (fam, name)) in [(door, "D1"), (door, "D2"), (other, "E1"), (other, "E2")].into_iter().enumerate() {
        let inst = fams[fam].solve(&Request::default()).unwrap();
        s.items.push(FamilyItem { id: 10 + k as u64, name: name.into(), family: fam, type_name: None, frame: Frame { origin: [k as f64 * 2.0, 0.0, 0.0], x: [1.0, 0.0] }, instance: inst, host: None });
    }
    let out = export_ifc2x3(&s).unwrap();
    assert_eq!(out.report.items, 2);
    assert_eq!(out.text.matches("IFCDOOR(").count(), 2);
    // no IFC4 type entities, mapped items or ports; the doors keep the ids they have in the IFC4 file
    for t in ["IFCDOORTYPE", "IFCMAPPEDITEM", "IFCDISTRIBUTIONPORT", "IFCREPRESENTATIONMAP"] {
        assert!(!out.text.contains(t), "{t}");
    }
    let gid = global_id(1, 0, 10);
    let line = out.text.lines().find(|l| l.contains(&format!("IFCDOOR('{gid}',#"))).expect("door line");
    assert!(line.contains(",'D1',$,$,#") && line.ends_with(");"), "{line}");
    // overall height and width are the last two attributes, after the tag
    let attrs: Vec<&str> = line.trim_end_matches(");").rsplitn(3, ',').collect();
    assert!(attrs[0].parse::<f64>().is_ok() && attrs[1].parse::<f64>().is_ok(), "{line}");
    // the two other families are counted, once, in one warning
    let w: Vec<&String> = out.report.warnings.iter().filter(|w| w.contains("placed families")).collect();
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("2 placed families"), "{}", w[0]);
}

#[test]
fn a_swept_profile_is_written_turned_a_quarter_for_the_fixed_reference_sweep() {
    use openbim_family::forms::{FormDef, ProfileDef};
    use openbim_family::{compile_library, Category, Family, Request};
    // a bar 0.04 wide (u) and 0.01 high (v) swept along x: IFC wants the profile's x along "up" and y to the right,
    // so the corner (u 0.04, v 0.01) is written at (0.01, -0.04)
    let f = Family {
        name: "Bar".into(),
        forms: vec![FormDef::sweep("bar", "0", &[("0", "0"), ("1", "0")], ProfileDef::rect("0", "0", "0.04", "0.01"))],
        category: Category::Furniture,
        ..Default::default()
    };
    let c = compile_library(std::slice::from_ref(&f)).unwrap();
    let mut s = scene(vec![wall(5)]);
    s.families = vec![FamilyRef { id: 1, family: c[0].clone() }];
    let inst = c[0].solve(&Request::default()).unwrap();
    s.items = vec![FamilyItem { id: 10, name: "B".into(), family: 0, type_name: None, frame: Frame { origin: [0.0, 0.0, 0.0], x: [1.0, 0.0] }, instance: inst, host: None }];
    let out = export(&s).unwrap();
    assert_eq!(out.text.matches("IFCFIXEDREFERENCESWEPTAREASOLID(").count(), 1);
    assert!(out.text.contains("IFCCARTESIANPOINT((0.01,-0.04))"), "turned corner missing");
    assert!(!out.text.contains("IFCCARTESIANPOINT((0.04,0.01))"), "profile was written unturned");
}

#[test]
fn a_blend_is_a_faceted_brep_and_a_void_is_cut_with_a_boolean_result() {
    use openbim_family::forms::{FormDef, ProfileDef};
    use openbim_family::{compile_library, Axis, Category, Family, Request};
    let r = |a, b, c, d| ProfileDef::rect(a, b, c, d);
    let plain = |forms| Family { name: "F".into(), forms, category: Category::Furniture, ..Default::default() };
    let run = |forms: Vec<FormDef>| {
        let f = plain(forms);
        let c = compile_library(std::slice::from_ref(&f)).unwrap();
        let mut s = scene(vec![wall(5)]);
        s.families = vec![FamilyRef { id: 1, family: c[0].clone() }];
        let inst = c[0].solve(&Request::default()).unwrap();
        s.items = vec![FamilyItem { id: 10, name: "B".into(), family: 0, type_name: None, frame: Frame { origin: [0.0, 0.0, 0.0], x: [1.0, 0.0] }, instance: inst, host: None }];
        export(&s).unwrap()
    };
    // a blend: a closed brep, 2 caps and 4 sides in triangles; the body is a Brep representation
    let out = run(vec![FormDef::blend("b", Axis::Z, "0", "1", r("0", "0", "1", "1"), r("0.25", "0.25", "0.75", "0.75"))]);
    assert_eq!(out.text.matches("IFCFACETEDBREP(").count(), 1); // written once, in the type's shared map
    assert!(out.text.contains("'Body','Brep'"));
    assert_eq!(out.text.matches("IFCCLOSEDSHELL(").count(), 1);
    assert_eq!(out.text.matches("IFCFACE((").count(), 12); // 2 caps and 4 sides, each in two triangles
    assert!(!out.report.warnings.iter().any(|w| w.contains("form was not written")), "{:?}", out.report.warnings);
    let walls_only = out.text.matches("IFCEXTRUDEDAREASOLID(").count(); // the wall of the scene has bodies of its own
    // a void through an extrusion: one boolean difference per solid and void, and the body is a Clipping representation
    let out = run(vec![
        FormDef::extrusion("solid", Axis::Z, "0", "1", r("0", "0", "1", "1")),
        FormDef::extrusion("hole", Axis::Z, "-1", "2", r("0.25", "0.25", "0.75", "0.75")).void(),
    ]);
    assert!(out.text.contains("IFCBOOLEANRESULT(.DIFFERENCE.,"));
    assert!(out.text.contains("'Body','Clipping'"));
    // the solid and the void are written once each, the void only as the operand of the difference
    assert_eq!(out.text.matches("IFCEXTRUDEDAREASOLID(").count(), walls_only + 2);
    assert_eq!(out.text.matches("IFCBOOLEANRESULT(").count(), 1);
}

#[test]
fn a_beam_exports_as_an_ifcbeam_along_its_own_axis_and_a_bad_one_is_refused() {
    use openbim_ifc::BeamItem;
    let mut s = scene(vec![wall(5)]);
    s.beams = vec![BeamItem::rect(9, "B1", [0.0, 1.0, 2.6], [4.0, 1.0, 2.6], 0.2, 0.4)];
    let a = export(&s).unwrap();
    assert!(a.text.contains(&format!("IFCBEAM('{}'", global_id(1, 0, 9))), "{}", a.text);
    assert!(a.text.contains("'B1'") && a.text.contains(".BEAM."));
    assert!(a.text.contains("IFCRECTANGLEPROFILEDEF(.AREA.,$,#"), "a rectangular section");
    let solid = a.text.lines().rfind(|l| l.contains("IFCEXTRUDEDAREASOLID")).unwrap();
    assert!(solid.ends_with(",4.);") || solid.ends_with(",4.0);"), "extruded over its 4 m: {solid}");
    assert_eq!(a.text, export(&s).unwrap().text, "stable");
    s.beams[0].b = [0.0, 1.0, 4.0];
    assert!(export(&s).is_err(), "a vertical member is a column");
    s.beams[0].b = [4.0, 1.0, 2.6];
    s.beams[0].profile = openbim_geom::profile::Profile::Rect { width: 0.2, depth: 0.0 };
    assert!(export(&s).is_err());
}

#[test]
fn beams_and_columns_take_round_and_i_sections() {
    use openbim_geom::profile::Profile;
    use openbim_ifc::{BeamItem, ColumnItem};
    let mut s = scene(vec![wall(5)]);
    let i = Profile::I { width: 0.2, depth: 0.4, web: 0.01, flange: 0.02 };
    s.beams = vec![
        BeamItem { id: 9, name: "I".into(), a: [0.0, 1.0, 2.6], b: [4.0, 1.0, 2.6], profile: i, usage: openbim_ifc::FramingUsage::Girder, bars: vec![] },
        BeamItem { id: 10, name: "R".into(), a: [0.0, 2.0, 2.6], b: [4.0, 2.0, 2.6], profile: Profile::Circle { diameter: 0.3 }, usage: openbim_ifc::FramingUsage::Brace, bars: vec![] },
    ];
    let t = export(&s).unwrap().text;
    assert_eq!((t.matches("IFCBEAM(").count(), t.matches("IFCMEMBER(").count()), (1, 1), "the round one is a brace, so a member");
    assert!(t.contains("IFCISHAPEPROFILEDEF(.AREA.,$,#") && t.contains(",0.2,0.4,0.01,0.02,$,$,$)"), "{t}");
    assert!(t.contains("IFCCIRCLEPROFILEDEF(.AREA.,$,#") && t.contains(",0.15)"));
    // a column on a round or an I section
    let col = ColumnItem::from_profile(11, "C", &Profile::Circle { diameter: 0.4 }, [2.0, 2.0], 0.0, 3.0, true).unwrap();
    assert_eq!(col.footprint.len(), 32);
    let h = ColumnItem::from_profile(12, "H", &i, [5.0, 2.0], 0.0, 3.0, false).unwrap();
    assert_eq!(h.footprint.len(), 12);
    assert!(ColumnItem::from_profile(13, "H", &i, [5.0, 2.0], 0.0, 3.0, true).is_err(), "not convex, so no room boundary");
    assert!(ColumnItem::from_profile(14, "X", &Profile::Circle { diameter: 0.0 }, [0.0, 0.0], 0.0, 3.0, false).is_err());
    s.columns = vec![col, h];
    assert_eq!(export(&s).unwrap().text.matches("IFCCOLUMN(").count(), 2);
}
