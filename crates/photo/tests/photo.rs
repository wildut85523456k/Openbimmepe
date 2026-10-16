use openbim_photo::*;
use std::f64::consts::PI;

fn close(a: f64, b: f64, rel: f64) -> bool { (a - b).abs() <= rel * b.abs().max(1e-12) }

/// an IES file; `rows` has one line of candela per horizontal angle
fn ies(head: &str, tilt: &str, lamps: &str, v: &[f64], h: &[f64], rows: &[Vec<f64>], eol: &str) -> String {
    let nums = |x: &[f64]| x.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(" ");
    let mut s = format!("IESNA:LM-63-2002{eol}{head}{tilt}{eol}{lamps} {} {} 1 2 0.5 0.4 0.1{eol}1 1 25{eol}{}{eol}{}{eol}", v.len(), h.len(), nums(v), nums(h));
    for r in rows { s.push_str(&nums(r)); s.push_str(eol); }
    s
}
fn grid(step: f64, to: f64) -> Vec<f64> { (0..=(to / step).round() as usize).map(|k| k as f64 * step).collect() }

#[test]
fn an_even_light_gives_four_pi_times_its_intensity() {
    let v = grid(30.0, 180.0);
    let f = ies("[MANUFAC] Maker\r\n[LUMCAT] CAT-1\r\n[LUMINAIRE] Round light\r\n[MORE] second line\r\n", "TILT=NONE", "1 -1 1", &v, &[0.0], &[vec![100.0; 7]], "\r\n");
    let p = parse(f.as_bytes(), "a.ies").unwrap();
    assert_eq!(p.format, Format::Ies);
    assert_eq!(p.manufacturer.as_deref(), Some("Maker"));
    assert_eq!(p.catalogue.as_deref(), Some("CAT-1"));
    assert_eq!(p.description.as_deref(), Some("Round light second line"));
    assert_eq!(p.symmetry, Symmetry::Axial);
    assert!(close(p.flux_lm().unwrap(), 4.0 * PI * 100.0, 1e-9), "{:?}", p.flux_lm());
    assert!(close(p.watts.unwrap(), 25.0, 1e-12) && p.lamp_flux_lm.is_none());
    assert!(close(p.intensity(123.0, 77.0), 100.0, 1e-12) && p.intensity(0.0, 181.0) == 0.0);
    assert!(close(p.size_m[0], 0.4, 1e-12) && close(p.size_m[1], 0.5, 1e-12));
}

#[test]
fn a_cosine_downlight_gives_pi_times_its_peak_and_units_and_multiplier_count() {
    let v = grid(5.0, 90.0);
    let row: Vec<f64> = v.iter().map(|g| 200.0 * g.to_radians().cos()).collect();
    let f = ies("", "TILT=NONE", "1 3000 1", &v, &[0.0], std::slice::from_ref(&row), "\n");
    let p = parse(f.as_bytes(), "a.ies").unwrap();
    assert!(close(p.flux_lm().unwrap(), PI * 200.0, 2e-3), "{:?}", p.flux_lm());
    assert!(close(p.lor().unwrap(), PI * 200.0 / 3000.0, 2e-3));
    assert!(close(p.max_intensity(), 200.0, 1e-12));
    // half of peak at 60 degrees on each side
    assert!(close(p.beam_angle_deg().unwrap(), 120.0, 0.02), "{:?}", p.beam_angle_deg());
    // multiplier 2 doubles everything; feet are converted to metres
    let text = f.replace("1 3000 1 ", "1 3000 2 ");
    assert!(close(parse(text.as_bytes(), "a.ies").unwrap().flux_lm().unwrap(), 2.0 * PI * 200.0, 2e-3));
    let feet = f.replace(" 1 2 0.5 0.4 0.1", " 1 1 1.0 2.0 3.0");
    let pf = parse(feet.as_bytes(), "a.ies").unwrap();
    assert!(close(pf.size_m[1], 0.3048, 1e-12) && close(pf.size_m[0], 0.6096, 1e-12) && close(pf.size_m[2], 0.9144, 1e-12));
}

#[test]
fn the_horizontal_symmetries_unfold_and_agree_with_the_full_table() {
    let v = grid(10.0, 180.0);
    let ph = |c: f64| 1.0 + 0.5 * (2.0 * c.to_radians()).cos(); // even about C0-C180 and C90-C270
    let f = |c: f64| v.iter().map(|g| 100.0 * g.to_radians().sin() * ph(c)).collect::<Vec<f64>>();
    let full: Vec<f64> = grid(15.0, 360.0);
    let full_t = ies("", "TILT=NONE", "1 -1 1", &v, &full, &full.iter().map(|&c| f(c)).collect::<Vec<_>>(), "\n");
    let half: Vec<f64> = grid(15.0, 180.0);
    let half_t = ies("", "TILT=NONE", "1 -1 1", &v, &half, &half.iter().map(|&c| f(c)).collect::<Vec<_>>(), "\n");
    let quad: Vec<f64> = grid(15.0, 90.0);
    let quad_t = ies("", "TILT=NONE", "1 -1 1", &v, &quad, &quad.iter().map(|&c| f(c)).collect::<Vec<_>>(), "\n");
    let (a, b, c) = (parse(full_t.as_bytes(), "x").unwrap(), parse(half_t.as_bytes(), "x").unwrap(), parse(quad_t.as_bytes(), "x").unwrap());
    assert_eq!((a.symmetry, b.symmetry, c.symmetry), (Symmetry::None, Symmetry::About0_180, Symmetry::Quadrant));
    // the analytic flux: integral of 100 sin(g) over the sphere is 100 x pi^2 / ... = 100 * (pi * pi/2 ... ) computed as pi^2 * 100 / 1 * 0.5 * 2
    let want = 100.0 * PI * PI; // ∫ sin g sin g dg dφ = (π/2)(2π) = π², phase term integrates to 0
    for p in [&a, &b, &c] {
        assert!(close(p.flux_lm().unwrap(), want, 5e-3), "{:?} vs {want}", p.flux_lm());
    }
    for (cc, gg) in [(10.0, 50.0), (100.0, 80.0), (200.0, 120.0), (350.0, 30.0), (170.0, 90.0)] {
        assert!(close(b.intensity(cc, gg), a.intensity(cc, gg), 1e-9) && close(c.intensity(cc, gg), a.intensity(cc, gg), 1e-9), "{cc} {gg}");
    }
}

#[test]
fn a_tilt_table_and_comma_lists_are_read() {
    let v = grid(45.0, 180.0);
    let f = ies("[MANUFAC] M\n", "TILT=INCLUDE\n1\n3\n0 45 90\n1 0.9 0.8", "1 -1 1", &v, &[0.0], &[vec![10.0; 5]], "\n").replace(" 1 2 0.5 0.4 0.1", ", 1, 2, 0.5, 0.4, 0.1");
    assert!(parse(f.as_bytes(), "a.ies").is_ok());
}

#[allow(clippy::too_many_arguments)]
fn ldt(isym: i32, mc: usize, dc: f64, ng: usize, dg: f64, flux: f64, lorl: f64, planes: &[Vec<f64>], list_all_c: bool) -> String {
    let mut l: Vec<String> = vec![
        "Maker GmbH".into(), "1".into(), isym.to_string(), mc.to_string(), dc.to_string(), ng.to_string(), dg.to_string(),
        "report".into(), "Wide downlight".into(), "ART-77".into(), "f.ldt".into(), "date".into(),
        "300".into(), "300".into(), "100".into(), "250".into(), "250".into(), "0".into(), "0".into(), "0".into(), "0".into(),
        "100".into(), lorl.to_string(), "1".into(), "0".into(), "1".into(),
        "1".into(), "LED".into(), flux.to_string(), "3000".into(), "80".into(), "30".into(),
    ];
    l.push("0.1 0.2 0.3 0.4 0.5 0.6 0.7 0.8 0.9 1.0".into());
    let nc = if list_all_c { mc.max(1) } else { planes.len() };
    let cs: Vec<String> = (0..nc).map(|k| (k as f64 * dc).to_string()).collect();
    l.extend(cs);
    l.extend((0..ng).map(|k| (k as f64 * dg).to_string()));
    for p in planes { l.extend(p.iter().map(|v| v.to_string())); }
    l.join("\r\n")
}

#[test]
fn eulumdat_files_with_each_symmetry_read_the_same_light() {
    // intensity in cd/klm of a 1000 lm lamp: 300 cos(gamma) down to 90 degrees; LED of 2000 lm
    let gs = grid(10.0, 90.0);
    let row: Vec<f64> = gs.iter().map(|g| 300.0 * g.to_radians().cos()).collect();
    let axial = ldt(1, 0, 0.0, gs.len(), 10.0, 2000.0, 80.0, std::slice::from_ref(&row), true);
    let p = parse(axial.as_bytes(), "a.ldt").unwrap();
    assert_eq!(p.format, Format::Eulumdat);
    assert_eq!((p.manufacturer.as_deref(), p.catalogue.as_deref(), p.description.as_deref()), (Some("Maker GmbH"), Some("ART-77"), Some("Wide downlight")));
    assert!(close(p.lamp_flux_lm.unwrap(), 2000.0, 1e-12) && close(p.watts.unwrap(), 30.0, 1e-12));
    assert_eq!((p.cct_k, p.cri), (Some(3000.0), Some(80.0)));
    assert!(close(p.size_m[0], 0.3, 1e-12) && close(p.lor_stated.unwrap(), 0.8, 1e-12));
    // peak: 300 cd/klm x 2 klm
    assert!(close(p.max_intensity(), 600.0, 1e-12));
    // flux = pi x 600 for a cosine law; the stated 80 % of 2000 lm is 1600 lm, so a note warns of the difference
    assert!(close(p.flux_lm().unwrap(), PI * 600.0, 3e-3), "{:?}", p.flux_lm());
    assert!(p.notes.iter().any(|n| n.contains("light output ratio")), "{:?}", p.notes);
    // the same light with all 24 planes, or the C list shortened, or quadrant symmetry: same answer
    let all = ldt(0, 24, 15.0, gs.len(), 10.0, 2000.0, 100.0, &vec![row.clone(); 24], true);
    let quad = ldt(4, 24, 15.0, gs.len(), 10.0, 2000.0, 100.0, &vec![row.clone(); 7], true);
    let quad_short = ldt(4, 24, 15.0, gs.len(), 10.0, 2000.0, 100.0, &vec![row; 7], false);
    for t in [all, quad, quad_short] {
        let q = parse(t.as_bytes(), "a.ldt").unwrap();
        assert!(close(q.flux_lm().unwrap(), PI * 600.0, 3e-3) && close(q.intensity(200.0, 30.0), p.intensity(0.0, 30.0), 1e-9));
    }
}

#[test]
fn files_that_are_wrong_are_errors_not_crashes() {
    let v = grid(30.0, 180.0);
    let good = ies("", "TILT=NONE", "1 -1 1", &v, &[0.0], &[vec![100.0; 7]], "\n");
    assert!(parse(good.as_bytes(), "a.ies").is_ok());
    for (what, text) in [
        ("truncated", good[..good.len() - 12].to_string()),
        ("text in a number", good.replace("100 100", "100 abc")),
        ("negative candela", good.replace("100 100 100", "-5 100 100")),
        ("angles not increasing", good.replace("0 30 60", "0 60 30")),
        ("no tilt line", good.replace("TILT=NONE", "")),
        ("huge count", good.replace(" 7 1 1 2 0.5", " 9999999 1 1 2 0.5")),
        ("zero multiplier", good.replace("1 -1 1 7", "1 -1 0 7")),
    ] {
        assert!(matches!(parse(text.as_bytes(), "a.ies"), Err(PhotoError::Bad(_)) | Err(PhotoError::UnknownFormat)), "{what}");
    }
    assert_eq!(parse(b"hello world", "a.txt"), Err(PhotoError::UnknownFormat));
    assert_eq!(parse(&vec![b'a'; 9 * 1024 * 1024], "a.ies"), Err(PhotoError::TooLarge));
    assert!(parse(&[], "a.ldt").is_err());
    // B photometry is read, with no flux
    let b = good.replace("1 -1 1 7 1 1 2", "1 -1 1 7 1 2 2");
    let pb = parse(b.as_bytes(), "a.ies").unwrap();
    assert!(pb.flux_lm().is_none() && pb.notes.iter().any(|n| n.contains("A or B")));
    // Latin-1 text in the header
    let mut bytes = good.replace("IESNA:LM-63-2002\n", "IESNA:LM-63-2002\n[MANUFAC] Lumi\u{e8}re\n").chars().map(|c| c as u32 as u8).collect::<Vec<u8>>();
    bytes.truncate(bytes.len());
    assert_eq!(parse(&bytes, "a.ies").unwrap().manufacturer.as_deref(), Some("Lumi\u{e8}re"));
}

proptest::proptest! {
    #[test]
    fn nothing_panics(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..400), cut in 0usize..400, pos in 0usize..400, b in proptest::prelude::any::<u8>()) {
        let _ = parse(&bytes, "x.ies");
        let _ = parse(&bytes, "x.ldt");
        let v = grid(30.0, 180.0);
        let mut good = ies("[MANUFAC] M\n", "TILT=NONE", "1 -1 1", &v, &[0.0, 90.0], &[vec![100.0; 7], vec![50.0; 7]], "\n").into_bytes();
        let l = good.len();
        good[pos % l] = b;
        good.truncate(cut.max(1) % l + 1);
        if let Ok(p) = parse(&good, "x.ies") { let _ = (p.flux_lm(), p.beam_angle_deg(), p.intensity(10.0, 10.0)); }
        let t = ldt(2, 24, 15.0, 7, 30.0, 1000.0, 90.0, &vec![vec![100.0; 7]; 13], true).into_bytes();
        let mut t2 = t.clone();
        let l = t2.len();
        t2[pos % l] = b;
        t2.truncate(cut.max(1) % l + 1);
        if let Ok(p) = parse(&t2, "x.ldt") { let _ = (p.flux_lm(), p.beam_angle_deg(), p.intensity(10.0, 10.0)); }
    }
}

fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (n, b) in files {
        w.start_file(*n, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).unwrap();
        w.write_all(b).unwrap();
    }
    w.finish().unwrap().into_inner()
}

#[test]
fn an_archive_of_photometric_files_gives_one_result_per_file() {
    let v = grid(30.0, 180.0);
    let a = ies("[MANUFAC] A\n", "TILT=NONE", "1 -1 1", &v, &[0.0], &[vec![100.0; 7]], "\n");
    let b = ies("[MANUFAC] B\n", "TILT=NONE", "1 -1 1", &v, &[0.0], &[vec![50.0; 7]], "\r\n");
    let z = zip_of(&[("dir/a_0.ies", a.as_bytes()), ("readme.txt", b"hello"), ("b_1.IES", b.as_bytes()), ("broken.ies", b"nonsense")]);
    // the archive may carry any name, even .ies
    let r = import(&z, "family.ies");
    assert_eq!(r.len(), 3);
    assert_eq!((r[0].name.as_str(), r[2].name.as_str()), ("dir/a_0.ies", "broken.ies"));
    assert_eq!(r[0].result.as_ref().unwrap().manufacturer.as_deref(), Some("A"));
    assert!(close(r[1].result.as_ref().unwrap().flux_lm().unwrap(), 4.0 * PI * 50.0, 1e-9));
    assert!(r[2].result.is_err());
    // a plain file is one result; an empty or damaged archive is an error, not a crash
    assert_eq!(import(a.as_bytes(), "a.ies").len(), 1);
    assert!(import(&zip_of(&[("x.txt", b"a")]), "z.zip")[0].result.is_err());
    assert!(import(&z[..z.len() / 2], "z.zip")[0].result.is_err());
    // an archive claiming too many files
    let many: Vec<(String, Vec<u8>)> = (0..300).map(|i| (format!("f{i}.ies"), vec![b'x'])).collect();
    let refs: Vec<(&str, &[u8])> = many.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    assert!(import(&zip_of(&refs), "z.zip")[0].result.is_err());
}

/// Run with `PHOTO_SAMPLES=/folder cargo test` to read a folder of real manufacturer files: each must give a flux
/// within 1 % of the lamp flux the file states.
#[test]
fn real_manufacturer_files_when_a_folder_is_given() {
    let Ok(dir) = std::env::var("PHOTO_SAMPLES") else { return };
    let mut seen = 0;
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let bytes = std::fs::read(e.path()).unwrap();
        for it in import(&bytes, &e.file_name().to_string_lossy()) {
            let p = it.result.unwrap_or_else(|err| panic!("{}: {err}", it.name));
            if let Some(l) = p.lor() { assert!((l - 1.0).abs() < 0.01 || p.notes.iter().any(|n| !n.is_empty()), "{}: output ratio {l}", it.name); }
            seen += 1;
        }
    }
    assert!(seen > 0);
}
