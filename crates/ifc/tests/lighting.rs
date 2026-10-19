use openbim_ifc::lighting::*;

fn full() -> RoomLighting {
    RoomLighting { work_plane_height_m: Some(0.8), boundary_zone_m: Some(0.5), maintenance_factor: Some(0.8), required_lux: Some(300.0), min_uniformity: Some(0.6), max_ugr: Some(19.0), min_cri: Some(80.0), reflectance_ceiling: Some(0.7), reflectance_walls: Some(0.5), reflectance_floor: Some(0.2) }
}

#[test]
fn room_inputs_are_checked_and_missing_ones_listed() {
    assert!(full().check().is_ok() && full().missing_for_calculation().is_empty());
    assert_eq!(RoomLighting::default().missing_for_calculation(), vec!["work plane height", "maintenance factor", "required illuminance", "surface reflectances"]);
    // one reflectance missing still counts as missing reflectances
    assert_eq!(RoomLighting { reflectance_walls: None, ..full() }.missing_for_calculation(), vec!["surface reflectances"]);
    for (bad, why) in [
        (RoomLighting { maintenance_factor: Some(0.0), ..full() }, "maintenance"),
        (RoomLighting { maintenance_factor: Some(1.2), ..full() }, "maintenance"),
        (RoomLighting { required_lux: Some(-1.0), ..full() }, "illuminance"),
        (RoomLighting { work_plane_height_m: Some(f64::NAN), ..full() }, "work plane"),
        (RoomLighting { reflectance_floor: Some(1.5), ..full() }, "floor"),
        (RoomLighting { min_cri: Some(120.0), ..full() }, "rendering"),
        (RoomLighting { min_uniformity: Some(-0.1), ..full() }, "uniformity"),
    ] {
        assert!(bad.check().unwrap_err().contains(why), "{why}");
    }
}

#[test]
fn luminaires_of_different_types_carry_their_own_lighting_in_the_schedule_and_the_file() {
    use openbim_arch::{Level, Levels};
    use openbim_family::{light::*, Category, Family, Request};
    use openbim_ifc::*;
    let ies = |cd: f64, watts: f64| -> Vec<u8> {
        let v: Vec<String> = (0..=6).map(|k| (k * 30).to_string()).collect();
        format!("IESNA:LM-63-2002\n[MANUFAC] M\nTILT=NONE\n1 -1 1 7 1 1 2 0.6 0.6 0\n1 1 {watts}\n{}\n0\n{}\n", v.join(" "), vec![cd.to_string(); 7].join(" ")).into_bytes()
    };
    let imports = vec![
        openbim_photo::Imported { name: "Lo.ies".into(), data: ies(100.0, 20.0), result: openbim_photo::parse(&ies(100.0, 20.0), "Lo.ies") },
        openbim_photo::Imported { name: "Hi.ies".into(), data: ies(200.0, 40.0), result: openbim_photo::parse(&ies(200.0, 40.0), "Hi.ies") },
    ];
    let mut f = Family { name: "Panel".into(), category: Category::LightFixture, ..Default::default() };
    let r = types_from_imports(&mut f, &imports).unwrap();
    assert_eq!(r.added.len(), 2);
    for t in &mut f.types {
        let l = t.lighting.as_mut().unwrap();
        l.picture = Some(format!("{}.png", t.name));
        l.datasheet = Some(format!("{}.pdf", t.name));
    }
    let c = f.compile().unwrap();
    let inst = |t: &str| c.solve(&Request { type_name: Some(t), ..Default::default() }).unwrap();
    let item = |id: u64, t: &str, x: f64| FamilyItem { id, name: format!("L{id}"), family: 0, type_name: Some(t.into()), frame: Frame { origin: [x, 0.0, 2.5], x: [1.0, 0.0] }, instance: inst(t), host: None };
    let scene = Scene {
        project_name: "P".into(), project_key: 1, beams: vec![], materials: Default::default(), timestamp: "t".into(), levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(), walls: vec![], wall_penetrations: vec![], firestops: Default::default(), slabs: vec![], columns: vec![], shafts: vec![], floor_penetrations: vec![], slab_voids: vec![], networks: vec![],
        families: vec![FamilyRef { id: 1, family: c.clone() }], items: vec![item(10, "Lo", 0.0), item(11, "Hi", 1.0), item(12, "Hi", 2.0)], rooms: vec![], wall_types: vec![], wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], room_lighting: Default::default(), pset_map: Default::default(), types: vec![], member_info: Default::default(), roofs: vec![], railings: vec![], stairs: vec![], native: vec![],
    };
    let (csv, warns) = luminaire_schedule(&scene);
    let rows: Vec<Vec<&str>> = csv.lines().skip(1).map(|l| l.split(',').collect()).collect();
    assert_eq!(rows.len(), 3);
    let col = |name: &str| HEADER.trim_end().split(',').position(|h| h == name).unwrap();
    assert_eq!(rows.iter().map(|r| r[col("photometry")]).collect::<Vec<_>>(), vec!["Lo.ies", "Hi.ies", "Hi.ies"]);
    assert_eq!(rows.iter().map(|r| r[col("power_w")]).collect::<Vec<_>>(), vec!["20", "40", "40"]);
    assert_eq!(rows.iter().map(|r| r[col("picture")]).collect::<Vec<_>>(), vec!["Lo.png", "Hi.png", "Hi.png"]);
    assert_eq!(rows.iter().map(|r| r[col("datasheet")]).collect::<Vec<_>>(), vec!["Lo.pdf", "Hi.pdf", "Hi.pdf"]);
    assert!(warns.iter().all(|w| !w.contains("power")), "{warns:?}");
    // the file: two light fixture types, each with its own photometric file
    let out = export(&scene).unwrap();
    assert_eq!(out.report.types, 2);
    assert!(out.text.contains("'PhotometryFile',$,IFCLABEL('Lo.ies')") && out.text.contains("'PhotometryFile',$,IFCLABEL('Hi.ies')"));
    assert!(out.text.contains("'PictureFile',$,IFCLABEL('Lo.png')") && out.text.contains("'DatasheetFile',$,IFCLABEL('Hi.pdf')"));
    assert!(out.text.contains("'TotalWattage',$,IFCREAL(20.0)") && out.text.contains("'TotalWattage',$,IFCREAL(40.0)"));
}

#[test]
fn the_luminaire_catalogue_page_embeds_checked_pictures_and_escapes_text() {
    use openbim_arch::{Level, Levels};
    use openbim_family::{light::*, Category, Family, Request};
    use openbim_ifc::*;
    let png: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4];
    let l = |pic: &str| LightingDef { manufacturer: Some("A<b>&Co".into()), picture: Some(pic.into()), datasheet: Some("d.pdf".into()), flux_lm: Some(1000.0), power_w: Some(10.0), photometry: Some("a.ies".into()), ..Default::default() };
    let mk = |pic: &str| {
        let f = Family { name: "Panel".into(), category: Category::LightFixture, lighting: Some(l(pic)), ..Default::default() };
        let c = f.compile().unwrap();
        let inst = c.solve(&Request::default()).unwrap();
        let item = |id: u64| FamilyItem { id, name: format!("L{id}"), family: 0, type_name: None, frame: Frame { origin: [0.0, 0.0, 2.5], x: [1.0, 0.0] }, instance: inst.clone(), host: None };
        Scene {
            project_name: "P&Q".into(), project_key: 1, beams: vec![], materials: Default::default(), timestamp: "t".into(), levels: Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }]).unwrap(), walls: vec![], wall_penetrations: vec![], firestops: Default::default(), slabs: vec![], columns: vec![], shafts: vec![], floor_penetrations: vec![], slab_voids: vec![], networks: vec![],
            families: vec![FamilyRef { id: 1, family: c }], items: vec![item(1), item(2)], rooms: vec![], wall_types: vec![], wall_joins: vec![], georef: None, room_settings: Default::default(), separation_lines: vec![], room_lighting: Default::default(), pset_map: Default::default(), types: vec![], member_info: Default::default(), roofs: vec![], railings: vec![], stairs: vec![], native: vec![],
        }
    };
    let files = |role: &str, name: &str| match (role, name) { ("picture", "p.png") => Some(png.clone()), ("datasheet", "d.pdf") => Some(b"%PDF-1.4".to_vec()), ("picture", "bad.png") => Some(b"%PDF-1.4".to_vec()), _ => None };
    let (h, w) = luminaire_catalogue_html(&mk("p.png"), &files);
    assert!(w.is_empty(), "{w:?}");
    assert!(h.contains("src=\"data:image/png;base64,iVBORw0KGgoBAgME\""), "{h}");
    assert!(h.contains("A&lt;b&gt;&amp;Co") && !h.contains("A<b>") && h.contains("P&amp;Q"));
    assert!(h.contains("<td>Placed</td><td>2</td>") && h.contains("<td>Data sheet</td><td><a download=\"d.pdf\" href=\"data:application/pdf;base64,JVBERi0xLjQ=\">d.pdf</a></td>") && h.contains("1000 lm"));
    // a picture that is not what it says, and one that is missing: no image, a warning each
    let (h, w) = luminaire_catalogue_html(&mk("bad.png"), &files);
    assert!(!h.contains("<img") && h.contains("no picture") && w.len() == 1 && w[0].contains("PNG or JPEG"), "{w:?}");
    let (h, w) = luminaire_catalogue_html(&mk("gone.png"), &files);
    assert!(!h.contains("<img") && w.len() == 1 && w[0].contains("not kept"), "{w:?}");
    // padding of the encoder
    let (h, _) = luminaire_catalogue_html(&mk("p.png"), &|r, n| if r == "picture" { Some([&png[..], &[9]].concat()) } else { files(r, n) });
    assert!(h.contains("iVBORw0KGgoBAgMECQ=="), "{h}");
    // a data sheet over the limit is named, not embedded, with a warning
    let big = |r: &str, n: &str| if r == "datasheet" { let mut v = b"%PDF-1.4".to_vec(); v.resize((5 << 20) + 1, 0); Some(v) } else { files(r, n) };
    let (h, w) = luminaire_catalogue_html(&mk("p.png"), &big);
    assert!(h.contains("<td>Data sheet</td><td>d.pdf</td>") && !h.contains("application/pdf") && w.len() == 1 && w[0].contains("5 MB"), "{w:?}");
    let exact = |r: &str, n: &str| if r == "datasheet" { let mut v = b"%PDF-1.4".to_vec(); v.resize(5 << 20, 0); Some(v) } else { files(r, n) };
    assert!(luminaire_catalogue_html(&mk("p.png"), &exact).0.contains("application/pdf"));
}
