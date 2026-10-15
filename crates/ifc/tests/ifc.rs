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
    assert_eq!(string("😀"), "'\\X2\\D83DDE00\\X0\\'");
    assert_eq!(string("é1é"), "'\\X2\\00E9\\X0\\1\\X2\\00E9\\X0\\'");
    assert_eq!(string("a\nb"), "'a\\X2\\000A\\X0\\b'");
}

fn levels() -> Levels {
    Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap()
}
fn scene(walls: Vec<WallItem>) -> Scene {
    Scene { project_name: "P".into(), project_key: 1, timestamp: "t".into(), levels: levels(), walls, wall_penetrations: vec![], slabs: vec![], shafts: vec![], floor_penetrations: vec![], networks: vec![] }
}
fn wall(id: u64) -> WallItem {
    WallItem { id, name: "W".into(), chain: WallChain::new(vec![[0.0, 0.0], [4.0, 0.0]], false, 0.2, 3.0) }
}

#[test]
fn a_plain_wall_exports_and_exports_again_identically() {
    let a = export(&scene(vec![wall(5)])).unwrap();
    let b = export(&scene(vec![wall(5)])).unwrap();
    assert_eq!(a.text, b.text);
    assert_eq!(a.report.walls, 1);
    assert!(a.text.starts_with("ISO-10303-21;\nHEADER;"));
    assert!(a.text.contains("FILE_SCHEMA(('IFC4'))"));
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
