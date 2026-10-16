//! Tests aimed at what mutation testing found unchecked: how the C-planes fold by symmetry, the Eulumdat
//! arithmetic, the beam angle, the limits on sizes and counts, and the archive limits.
use openbim_photo::*;
use std::f64::consts::PI;

fn close(a: f64, b: f64, tol: f64) -> bool { (a - b).abs() <= tol }

fn nums(x: &[f64]) -> String { x.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(" ") }

/// IES type C, feet or metres, candela multiplier 1, `rows` one line per horizontal angle
fn ies_c(v: &[f64], h: &[f64], rows: &[Vec<f64>]) -> String {
    let mut s = format!("IESNA:LM-63-2002\nTILT=NONE\n1 -1 1 {} {} 1 2 0.5 0.4 0.1\n1 1 25\n{}\n{}\n", v.len(), h.len(), nums(v), nums(h));
    for r in rows { s.push_str(&nums(r)); s.push('\n'); }
    s
}

/// a table whose every plane is one constant intensity, so the value read tells which plane was used
fn plane_values(h: &[f64], vals: &[f64]) -> Photometry {
    let rows: Vec<Vec<f64>> = vals.iter().map(|&x| vec![x, x]).collect();
    parse(ies_c(&[0.0, 90.0], h, &rows).as_bytes(), "a.ies").unwrap()
}

#[test]
fn c_planes_fold_by_each_symmetry() {
    let q = plane_values(&[0.0, 30.0, 60.0, 90.0], &[10.0, 20.0, 30.0, 40.0]);
    assert_eq!(q.symmetry, Symmetry::Quadrant);
    for (c, want) in [(0.0, 10.0), (30.0, 20.0), (150.0, 20.0), (210.0, 20.0), (330.0, 20.0), (90.0, 40.0), (270.0, 40.0), (180.0, 10.0), (45.0, 25.0), (135.0, 25.0), (-30.0, 20.0), (390.0, 20.0), (120.0, 30.0), (60.0, 30.0)] {
        assert!(close(q.intensity(c, 45.0), want, 1e-9), "quadrant C{c}: {}", q.intensity(c, 45.0));
    }
    let h = plane_values(&[0.0, 60.0, 120.0, 180.0], &[10.0, 20.0, 30.0, 40.0]);
    assert_eq!(h.symmetry, Symmetry::About0_180);
    for (c, want) in [(0.0, 10.0), (300.0, 20.0), (240.0, 30.0), (180.0, 40.0), (90.0, 25.0), (270.0, 25.0), (30.0, 15.0), (330.0, 15.0), (181.0, 39.833333333333336)] {
        assert!(close(h.intensity(c, 45.0), want, 1e-9), "0-180 C{c}: {}", h.intensity(c, 45.0));
    }
    let s = plane_values(&[90.0, 150.0, 210.0, 270.0], &[10.0, 20.0, 30.0, 40.0]);
    assert_eq!(s.symmetry, Symmetry::About90_270);
    for (c, want) in [(90.0, 10.0), (270.0, 40.0), (0.0, 25.0), (180.0, 25.0), (30.0, 20.0), (330.0, 30.0), (300.0, 35.0), (60.0, 15.0), (120.0, 15.0), (240.0, 35.0), (359.0, 20.0 + 10.0 * 31.0 / 60.0)] {
        // 540 - 359 = 181: between 150 and 210 at 31/60 of the way from 20 to 30
        assert!(close(s.intensity(c, 45.0), want, 1e-9), "90-270 C{c}: {}", s.intensity(c, 45.0));
    }
    let n = plane_values(&[0.0, 90.0, 180.0, 270.0, 360.0], &[10.0, 20.0, 30.0, 40.0, 10.0]);
    assert_eq!(n.symmetry, Symmetry::None);
    for (c, want) in [(0.0, 10.0), (45.0, 15.0), (-90.0, 40.0), (450.0, 20.0), (315.0, 25.0), (180.0, 30.0), (359.999, 10.0)] {
        assert!(close(n.intensity(c, 45.0), want, 1e-3), "none C{c}: {}", n.intensity(c, 45.0));
    }
    let a = plane_values(&[0.0], &[77.0]);
    assert_eq!(a.symmetry, Symmetry::Axial);
    for c in [0.0, 100.0, -50.0, 359.0] {
        assert!(close(a.intensity(c, 45.0), 77.0, 1e-12));
    }
    // angles that fit no pattern are taken as given, with a note
    let u = plane_values(&[10.0, 50.0], &[10.0, 20.0]);
    assert_eq!(u.symmetry, Symmetry::None);
    assert!(u.notes.iter().any(|n| n.contains("unusual")));
}

#[test]
fn intensity_is_linear_in_gamma_and_zero_outside_the_table() {
    let p = parse(ies_c(&[0.0, 10.0, 30.0], &[0.0], &[vec![100.0, 200.0, 0.0]]).as_bytes(), "a.ies").unwrap();
    for (g, want) in [(0.0, 100.0), (5.0, 150.0), (10.0, 200.0), (20.0, 100.0), (30.0, 0.0), (25.0, 50.0)] {
        assert!(close(p.intensity(0.0, g), want, 1e-9), "g{g}");
    }
    assert_eq!(p.intensity(0.0, -0.001), 0.0);
    assert_eq!(p.intensity(0.0, 30.001), 0.0);
    assert!(p.intensity(0.0, 30.0) == 0.0 && p.intensity(0.0, 0.0) == 100.0);
    assert!(close(p.max_intensity(), 200.0, 1e-12));
}

#[test]
fn flux_counts_every_plane_round_the_circle() {
    // gamma 0 to 180 and an intensity that depends on C only: flux = 2 x pi x integral of I over C... = 4 pi x mean(I over C)
    let p = parse(ies_c(&[0.0, 180.0], &[0.0, 180.0, 360.0], &[vec![100.0, 100.0], vec![300.0, 300.0], vec![500.0, 500.0]]).as_bytes(), "a.ies").unwrap();
    assert!(close(p.flux_lm().unwrap(), 4.0 * PI * 300.0, 1e-6 * 4.0 * PI * 300.0), "{:?}", p.flux_lm());
    // quadrant: the mean over the four quarters of a linear rise 0..90 is the mean of the planes; gamma 0..90 is a hemisphere
    let q = plane_values(&[0.0, 90.0], &[100.0, 300.0]);
    assert!(close(q.flux_lm().unwrap(), 2.0 * PI * 200.0, 1e-6 * 2.0 * PI * 200.0), "{:?}", q.flux_lm());
    // A and B photometry has no flux
    let b = ies_c(&[0.0, 90.0], &[0.0], &[vec![1.0, 1.0]]).replace(" 1 2 0.5 0.4 0.1", " 2 2 0.5 0.4 0.1");
    let pb = parse(b.as_bytes(), "a.ies").unwrap();
    assert!(pb.flux_lm().is_none() && pb.lor().is_none() && pb.symmetry == Symmetry::None);
}

#[test]
fn the_beam_angle_is_measured_in_the_c0_plane_at_half_the_peak() {
    // cosine: half the peak at 60 degrees each side
    let g: Vec<f64> = (0..=90).map(|k| k as f64).collect();
    let row: Vec<f64> = g.iter().map(|x| 100.0 * x.to_radians().cos()).collect();
    let p = parse(ies_c(&g, &[0.0], &[row]).as_bytes(), "a.ies").unwrap();
    assert!(close(p.beam_angle_deg().unwrap(), 120.0, 0.2), "{:?}", p.beam_angle_deg());
    // a ridge peaking at 30 degrees, half at 15 and 45
    let g = [0.0, 30.0, 60.0];
    let r = parse(ies_c(&g, &[0.0], &[vec![0.0, 100.0, 0.0]]).as_bytes(), "a.ies").unwrap();
    assert!(close(r.beam_angle_deg().unwrap(), 30.0, 0.2), "{:?}", r.beam_angle_deg());
    // an asymmetric ridge: rises over 20 degrees, falls over 60
    let a = parse(ies_c(&[0.0, 20.0, 80.0], &[0.0], &[vec![0.0, 100.0, 0.0]]).as_bytes(), "a.ies").unwrap();
    assert!(close(a.beam_angle_deg().unwrap(), 10.0 + 30.0, 0.2), "{:?}", a.beam_angle_deg());
    // the C0 plane decides, not the others
    let two = parse(ies_c(&[0.0, 90.0], &[0.0, 90.0], &[vec![100.0, 0.0], vec![100.0, 100.0]]).as_bytes(), "a.ies").unwrap();
    assert!(close(two.beam_angle_deg().unwrap(), 90.0, 0.2), "{:?}", two.beam_angle_deg());
    // a table that does not fall to half, or has no light, has no beam angle
    let flat = parse(ies_c(&[0.0, 45.0], &[0.0], &[vec![100.0, 80.0]]).as_bytes(), "a.ies").unwrap();
    assert!(flat.beam_angle_deg().is_none());
    let dark = parse(ies_c(&[0.0, 45.0], &[0.0], &[vec![0.0, 0.0]]).as_bytes(), "a.ies").unwrap();
    assert!(dark.beam_angle_deg().is_none());
    // peak at the far end, falling only one way
    let end = parse(ies_c(&[0.0, 40.0, 80.0], &[0.0], &[vec![0.0, 50.0, 100.0]]).as_bytes(), "a.ies").unwrap();
    assert!(end.beam_angle_deg().is_none());
}

// ---------- Eulumdat ----------

struct L {
    isym: i64,
    mc: f64,
    dc: f64,
    ng: usize,
    dg: f64,
    sizes: [f64; 3],
    lorl: f64,
    conv: f64,
    sets: Vec<(f64, String, String, f64)>, // flux, cct text, cri text, watts
    cs: Vec<f64>,
    gs: Vec<f64>,
    planes: Vec<Vec<f64>>,
}

fn base(planes: Vec<Vec<f64>>) -> L {
    L { isym: 0, mc: planes.len() as f64, dc: 90.0, ng: 2, dg: 90.0, sizes: [400.0, 300.0, 100.0], lorl: 100.0, conv: 1.0, sets: vec![(1000.0, "3000".into(), "80".into(), 30.0)], cs: (0..planes.len()).map(|k| k as f64 * 90.0).collect(), gs: vec![0.0, 90.0], planes }
}

fn ldt(l: &L) -> String {
    let mut o: Vec<String> = vec![
        "Maker".into(), "1".into(), l.isym.to_string(), l.mc.to_string(), l.dc.to_string(), l.ng.to_string(), l.dg.to_string(),
        "rep".into(), "Desc".into(), "ART".into(), "f.ldt".into(), "date".into(),
        l.sizes[0].to_string(), l.sizes[1].to_string(), l.sizes[2].to_string(), "250".into(), "250".into(), "0".into(), "0".into(), "0".into(), "0".into(),
        "100".into(), l.lorl.to_string(), l.conv.to_string(), "0".into(), l.sets.len().to_string(),
    ];
    for (f, cct, cri, w) in &l.sets {
        o.extend(["1".into(), "LED".into(), f.to_string(), cct.clone(), cri.clone(), w.to_string()]);
    }
    o.push("0.1 0.2 0.3 0.4 0.5 0.6 0.7 0.8 0.9 1.0".into());
    o.extend(l.cs.iter().map(|c| c.to_string()));
    o.extend(l.gs.iter().map(|g| g.to_string()));
    for p in &l.planes { o.extend(p.iter().map(|v| v.to_string())); }
    o.join("\n")
}

fn read(l: &L) -> Result<Photometry, PhotoError> { parse(ldt(l).as_bytes(), "a.ldt") }

#[test]
fn eulumdat_stores_the_planes_each_symmetry_asks_for() {
    let rows = |v: &[f64]| -> Vec<Vec<f64>> { v.iter().map(|&x| vec![x, x]).collect() };
    // no symmetry: all four planes
    let mut l = base(rows(&[10.0, 20.0, 30.0, 40.0]));
    let p = read(&l).unwrap();
    assert_eq!((p.symmetry, p.c.clone()), (Symmetry::None, vec![0.0, 90.0, 180.0, 270.0]));
    for (c, w) in [(0.0, 10.0), (90.0, 20.0), (180.0, 30.0), (270.0, 40.0), (45.0, 15.0)] { assert!(close(p.intensity(c, 45.0), w, 1e-9)); }
    // axial: one plane, listing one C angle
    l = base(rows(&[10.0]));
    l.isym = 1;
    l.mc = 0.0;
    let p = read(&l).unwrap();
    assert_eq!((p.symmetry, p.c.clone()), (Symmetry::Axial, vec![0.0]));
    // about C0-C180: planes 0..180 of 4 -> 3 stored
    l = base(rows(&[10.0, 20.0, 30.0]));
    l.isym = 2;
    l.mc = 4.0;
    let p = read(&l).unwrap();
    assert_eq!((p.symmetry, p.c.clone()), (Symmetry::About0_180, vec![0.0, 90.0, 180.0]));
    assert!(close(p.intensity(270.0, 45.0), 20.0, 1e-9));
    // about C90-C270: stored planes start at 90
    l = base(rows(&[10.0, 20.0, 30.0]));
    l.isym = 3;
    l.mc = 4.0;
    let p = read(&l).unwrap();
    assert_eq!((p.symmetry, p.c.clone()), (Symmetry::About90_270, vec![90.0, 180.0, 270.0]));
    assert!(close(p.intensity(0.0, 45.0), 20.0, 1e-9) && close(p.intensity(90.0, 45.0), 10.0, 1e-9));
    // quadrant: mc 8 at 45 degrees -> 3 stored
    l = base(rows(&[10.0, 20.0, 30.0]));
    l.isym = 4;
    l.mc = 8.0;
    l.dc = 45.0;
    let p = read(&l).unwrap();
    assert_eq!((p.symmetry, p.c.clone()), (Symmetry::Quadrant, vec![0.0, 45.0, 90.0]));
    assert!(close(p.intensity(315.0, 45.0), 20.0, 1e-9) && close(p.intensity(135.0, 45.0), 20.0, 1e-9));
    // the same with the list of C angles holding every plane, or a count that fits neither
    l.cs = (0..8).map(|k| k as f64 * 45.0).collect();
    assert_eq!(read(&l).unwrap().c, vec![0.0, 45.0, 90.0]);
    l.cs = vec![0.0, 45.0, 90.0, 135.0, 180.0];
    assert_eq!(read(&l).unwrap().c, vec![0.0, 45.0, 90.0]);
    // without a C step the angles of the file are used
    l.dc = 0.0;
    l.cs = vec![0.0, 20.0, 70.0];
    assert_eq!(read(&l).unwrap().c, vec![0.0, 20.0, 70.0]);
    // a table with the wrong number of values is an error that says so
    l = base(rows(&[10.0, 20.0, 30.0, 40.0]));
    l.planes.pop();
    l.cs = vec![];
    assert!(matches!(read(&l), Err(PhotoError::Bad(m)) if m.contains("numbers")));
}

#[test]
fn eulumdat_gamma_angles_come_from_the_file_or_the_step() {
    let mut l = base(vec![vec![5.0, 6.0, 7.0]]);
    l.isym = 1;
    l.mc = 0.0;
    l.ng = 3;
    l.dg = 15.0;
    l.gs = vec![0.0, 20.0, 40.0];
    assert_eq!(read(&l).unwrap().g, vec![0.0, 20.0, 40.0]);
    l.gs = vec![0.0, 0.0, 0.0];
    assert_eq!(read(&l).unwrap().g, vec![0.0, 15.0, 30.0]);
    l.gs = vec![40.0, 20.0, 0.0];
    assert_eq!(read(&l).unwrap().g, vec![0.0, 15.0, 30.0]);
    l.dg = 0.0;
    assert!(read(&l).is_err());
}

#[test]
fn eulumdat_lamp_data_sizes_and_conversion() {
    let mut l = base(vec![vec![100.0, 100.0]]);
    l.isym = 1;
    l.mc = 0.0;
    l.sets = vec![(1000.0, "3000K warm".into(), "80".into(), 30.0), (500.0, "4000".into(), "90".into(), 12.5)];
    let p = read(&l).unwrap();
    assert_eq!(p.lamp_flux_lm, Some(1500.0));
    assert_eq!(p.watts, Some(42.5));
    assert_eq!((p.cct_k, p.cri), (Some(3000.0), Some(80.0)));
    assert_eq!(p.size_m, [0.4, 0.3, 0.1]);
    // intensities are cd/klm: 100 cd/klm of 1500 lm
    assert!(close(p.intensity(0.0, 45.0), 150.0, 1e-9));
    // the conversion factor multiplies; zero or negative counts as 1
    l.conv = 2.0;
    assert!(close(read(&l).unwrap().intensity(0.0, 45.0), 300.0, 1e-9));
    for c in [0.0, -3.0] {
        l.conv = c;
        assert!(close(read(&l).unwrap().intensity(0.0, 45.0), 150.0, 1e-9));
    }
    // twenty lamp sets are allowed, none or twenty-one not
    l.conv = 1.0;
    l.sets = vec![(10.0, "3000".into(), "80".into(), 1.0); 20];
    assert_eq!(read(&l).unwrap().lamp_flux_lm, Some(200.0));
    for n in [0usize, 21] {
        l.sets = vec![(10.0, "3000".into(), "80".into(), 1.0); n];
        assert!(read(&l).is_err(), "{n} sets");
    }
    // colour temperature and rendering index outside their range are not taken
    l.sets = vec![(10.0, "999".into(), "101".into(), 1.0)];
    let p = read(&l).unwrap();
    assert_eq!((p.cct_k, p.cri), (None, None));
    for (cct, cri, ok_c, ok_r) in [("1000", "0", true, true), ("20000", "100", true, true), ("20001", "-1", false, false)] {
        l.sets = vec![(10.0, cct.into(), cri.into(), 1.0)];
        let p = read(&l).unwrap();
        assert_eq!((p.cct_k.is_some(), p.cri.is_some()), (ok_c, ok_r), "{cct} {cri}");
    }
    // no flux: a note, no error, no lamp flux; negative flux or watts: an error
    l.sets = vec![(0.0, "3000".into(), "80".into(), 0.0)];
    let p = read(&l).unwrap();
    assert!(p.lamp_flux_lm.is_none() && p.watts.is_none() && p.notes.iter().any(|n| n.contains("no lamp flux")));
    l.sets = vec![(1.0, "3000".into(), "80".into(), 0.0)];
    assert!(read(&l).unwrap().notes.iter().all(|n| !n.contains("no lamp flux")));
    for s in [(-1.0, 1.0), (1.0, -1.0)] {
        l.sets = vec![(s.0, "3000".into(), "80".into(), s.1)];
        assert!(read(&l).is_err());
    }
    // stated light output ratio: 0 and above 100 are not taken
    l.sets = vec![(1000.0, "3000".into(), "80".into(), 1.0)];
    for (lorl, want) in [(0.0, None), (100.0, Some(1.0)), (101.0, None), (50.0, Some(0.5))] {
        l.lorl = lorl;
        assert_eq!(read(&l).unwrap().lor_stated, want, "{lorl}");
    }
}

#[test]
fn the_light_output_ratio_note_appears_beyond_five_percent() {
    // a cosine law of 600 cd peak: flux pi x 600 = 1884.96 lm of a 2000 lm lamp
    let gs: Vec<f64> = (0..=9).map(|k| k as f64 * 10.0).collect();
    let row: Vec<f64> = gs.iter().map(|g| 300.0 * g.to_radians().cos()).collect();
    let mk = |lorl: f64| { let mut l = base(vec![row.clone()]); l.isym = 1; l.mc = 0.0; l.ng = 10; l.dg = 10.0; l.gs = gs.clone(); l.lorl = lorl; l.sets = vec![(2000.0, "3000".into(), "80".into(), 20.0)]; read(&l).unwrap() };
    let has = |p: &Photometry| p.notes.iter().any(|n| n.contains("light output ratio"));
    assert!(!has(&mk(94.2)));
    assert!(!has(&mk(90.0))); // 4.7 % apart
    assert!(has(&mk(89.0))); // 5.8 % apart
    assert!(has(&mk(100.0)));
}

#[test]
fn eulumdat_counts_and_symmetry_are_limited() {
    let mut l = base(vec![vec![1.0, 1.0]]);
    l.isym = 1;
    l.mc = 0.0;
    assert!(read(&l).is_ok());
    for isym in [-1, 5] {
        let mut m = base(vec![vec![1.0, 1.0]]);
        m.isym = isym;
        assert!(read(&m).is_err(), "isym {isym}");
    }
    for mc in [-1.0, 1.5, 2001.0] {
        let mut m = base(vec![vec![1.0, 1.0]]);
        m.isym = 1;
        m.mc = mc;
        assert!(read(&m).is_err(), "mc {mc}");
    }
    let mut m = base(vec![vec![1.0, 1.0]]);
    m.isym = 1;
    m.mc = 2000.0;
    m.cs = vec![0.0; 2000];
    assert!(read(&m).is_ok());
    // the number of intensities per plane: 1 is too few, 2000 allowed, 2001 not
    for (ng, ok) in [(1usize, false), (2, true), (2000, true), (2001, false)] {
        let mut m = base(vec![vec![1.0; ng]]);
        m.isym = 1;
        m.mc = 0.0;
        m.ng = ng;
        m.gs = (0..ng).map(|k| k as f64).collect();
        assert_eq!(read(&m).is_ok(), ok, "ng {ng}");
    }
    // a negative intensity, a missing line, a letter for a number
    let mut m = base(vec![vec![1.0, -1.0]]);
    m.isym = 1;
    m.mc = 0.0;
    assert!(read(&m).is_err());
    let text = ldt(&l);
    assert!(parse(text.replace("Maker\n1\n1\n", "Maker\n1\nx\n").as_bytes(), "a.ldt").is_err());
    assert!(parse(text.lines().take(25).collect::<Vec<_>>().join("\n").as_bytes(), "a.ldt").is_err());
    // decimal commas are read
    assert!(parse(text.replace("0.1 0.2", "0,1 0,2").replace("\n300\n", "\n300,5\n").as_bytes(), "a.ldt").is_ok());
}

// ---------- IES limits ----------

#[test]
fn ies_counts_multiplier_and_angles_are_checked() {
    let ok = |v: &[f64], h: &[f64], rows: &[Vec<f64>]| parse(ies_c(v, h, rows).as_bytes(), "a.ies");
    // a single vertical angle is too few
    assert!(ok(&[0.0], &[0.0], &[vec![1.0]]).is_err());
    assert!(ok(&[0.0, 90.0], &[0.0], &[vec![1.0, 1.0]]).is_ok());
    // equal or falling angles
    assert!(ok(&[0.0, 0.0], &[0.0], &[vec![1.0, 1.0]]).is_err());
    assert!(ok(&[0.0, 90.0], &[0.0, 0.0], &[vec![1.0, 1.0], vec![1.0, 1.0]]).is_err());
    // no horizontal angle at all
    assert!(parse("IESNA:LM-63-2002\nTILT=NONE\n1 -1 1 2 0 1 2 0.5 0.4 0.1\n1 1 25\n0 90\n".as_bytes(), "a.ies").is_err());
    // counts: whole numbers up to 2000
    let g2000: Vec<f64> = (0..2000).map(|k| k as f64 * 0.05).collect();
    assert!(ok(&g2000, &[0.0], &[vec![1.0; 2000]]).is_ok());
    let g2001: Vec<f64> = (0..2001).map(|k| k as f64 * 0.05).collect();
    assert!(ok(&g2001, &[0.0], &[vec![1.0; 2001]]).is_err());
    let base = ies_c(&[0.0, 90.0], &[0.0], &[vec![1.0, 1.0]]);
    assert!(parse(base.replace("1 -1 1 2 1", "1 -1 1 -2 1").as_bytes(), "a.ies").is_err());
    assert!(parse(base.replace("1 -1 1 2 1", "1 -1 1 2.5 1").as_bytes(), "a.ies").is_err());
    // the product of the counts is limited to a million values
    let g1000: Vec<f64> = (0..1000).map(|k| k as f64).collect();
    let h1000: Vec<f64> = (0..1000).map(|k| k as f64 * 0.36).collect();
    let big = ok(&g1000, &h1000, &vec![vec![1.0; 1000]; 1000]);
    assert!(big.is_ok(), "{:?}", big.err());
    let h1001: Vec<f64> = (0..1001).map(|k| k as f64 * 0.36).collect();
    assert!(ok(&g1000, &h1001, &vec![vec![1.0; 1000]; 1001]).is_err());
    // a multiplier scales the candela; zero and negative are refused
    let m2 = base.replace("1 -1 1 2 1", "1 -1 2 2 1");
    assert!(close(parse(m2.as_bytes(), "a.ies").unwrap().intensity(0.0, 10.0), 2.0, 1e-12));
    for m in ["0", "-1"] {
        assert!(parse(base.replace("1 -1 1 2 1", &format!("1 -1 {m} 2 1")).as_bytes(), "a.ies").is_err());
    }
    // units: feet are converted, metres kept; lamp data
    let feet = base.replace("1 -1 1 2 1 1 2 0.5", "1 -1 1 2 1 1 1 0.5");
    assert!(close(parse(feet.as_bytes(), "a.ies").unwrap().size_m[0], 0.4 * 0.3048, 1e-12));
    let lamps = base.replace("1 -1 1 2", "2 500 1 2");
    assert_eq!(parse(lamps.as_bytes(), "a.ies").unwrap().lamp_flux_lm, Some(1000.0));
    for l in ["0 500 1 2", "1 0 1 2", "1 -1 1 2"] {
        assert_eq!(parse(base.replace("1 -1 1 2", l).as_bytes(), "a.ies").unwrap().lamp_flux_lm, None, "{l}");
    }
    assert_eq!(parse(base.replace("1 1 25", "1 1 0").as_bytes(), "a.ies").unwrap().watts, None);
    // a candela table that gives more light than the lamps do carries a note, less does not
    let over = ies_c(&[0.0, 180.0], &[0.0], &[vec![100.0, 100.0]]).replace("1 -1 1 2", "1 1000 1 2");
    assert!(parse(over.as_bytes(), "a.ies").unwrap().notes.iter().any(|n| n.contains("lm")));
    let under = ies_c(&[0.0, 180.0], &[0.0], &[vec![50.0, 50.0]]).replace("1 -1 1 2", "1 1000 1 2");
    assert!(parse(under.as_bytes(), "a.ies").unwrap().notes.is_empty());
    let near = ies_c(&[0.0, 180.0], &[0.0], &[vec![100.0, 100.0]]).replace("1 -1 1 2", "1 1250 1 2"); // 1257 lm vs 1250 lm
    assert!(parse(near.as_bytes(), "a.ies").unwrap().notes.is_empty());
}

#[test]
fn errors_read_in_words() {
    assert_eq!(PhotoError::TooLarge.to_string(), "the file is larger than 8 MB");
    assert_eq!(PhotoError::UnknownFormat.to_string(), "not an IES or Eulumdat file");
    assert_eq!(PhotoError::Bad("because".into()).to_string(), "because");
    assert!(parse(b"IESNA:LM-63-2002\n[X] y\n", "a.ies").unwrap_err().to_string().contains("TILT"));
}

#[test]
fn the_format_is_told_from_the_content_and_the_size_limit_is_exact() {
    let max = 8 * 1024 * 1024;
    assert_eq!(parse(&vec![b'x'; max], "a.bin"), Err(PhotoError::UnknownFormat));
    assert_eq!(parse(&vec![b'x'; max + 1], "a.bin"), Err(PhotoError::TooLarge));
    assert_eq!(parse(&vec![b'x'; 2 * 1024 * 1024], "a.bin"), Err(PhotoError::UnknownFormat));
    // a file starting IESNA without TILT is IES (and bad), not unknown
    assert!(matches!(parse(b"IESNA91\nnothing", "x"), Err(PhotoError::Bad(_))));
    // by name: .LDT in capitals, .eul
    let l = ldt(&base(vec![vec![1.0, 1.0]]));
    for n in ["x.LDT", "x.eul", "X.Ldt"] {
        assert!(matches!(parse(l.as_bytes(), n), Ok(_) | Err(PhotoError::Bad(_))), "{n}");
        assert_ne!(parse(b"junk", n), Err(PhotoError::UnknownFormat), "{n}");
    }
    // by content: more than 30 lines whose third line is one digit
    let lines = |n: usize, third: &str| -> String { (0..n).map(|i| if i == 2 { third.to_string() } else { "x".to_string() }).collect::<Vec<_>>().join("\n") };
    assert_eq!(parse(lines(30, "1").as_bytes(), "x"), Err(PhotoError::UnknownFormat));
    assert!(matches!(parse(lines(31, "1").as_bytes(), "x"), Err(PhotoError::Bad(_))));
    assert!(matches!(parse(lines(31, " 1 ").as_bytes(), "x"), Err(PhotoError::Bad(_))));
    assert_eq!(parse(lines(31, "12").as_bytes(), "x"), Err(PhotoError::UnknownFormat));
    assert_eq!(parse(lines(31, "a").as_bytes(), "x"), Err(PhotoError::UnknownFormat));
    assert_eq!(parse(lines(31, "").as_bytes(), "x"), Err(PhotoError::UnknownFormat));
    // a name that says .ldt on IES content is still read as IES
    let i = ies_c(&[0.0, 90.0], &[0.0], &[vec![1.0, 1.0]]);
    assert_eq!(parse(i.as_bytes(), "wrong.ldt").unwrap().format, Format::Ies);
}

fn zip_of(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    use std::io::Write;
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (n, b) in files {
        w.start_file(n.as_str(), zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).unwrap();
        w.write_all(b).unwrap();
    }
    w.finish().unwrap().into_inner()
}

#[test]
fn archive_limits_are_exact() {
    let max = 8 * 1024 * 1024;
    // 256 entries are read, 257 are refused as a whole
    let many = |n: usize| -> Vec<(String, Vec<u8>)> { (0..n).map(|i| (format!("f{i}.ies"), b"x".to_vec())).collect() };
    assert_eq!(import(&zip_of(&many(256)), "z.zip").len(), 256);
    let r = import(&zip_of(&many(257)), "z.zip");
    assert!(r.len() == 1 && matches!(&r[0].result, Err(PhotoError::Bad(m)) if m.contains("256")));
    // one entry of exactly 8 MiB is read (and kept whole), one byte more is too large
    let r = import(&zip_of(&[("a.ies".into(), vec![b'x'; max])]), "z.zip");
    assert_eq!((r[0].result.clone(), r[0].data.len()), (Err(PhotoError::UnknownFormat), max));
    let r = import(&zip_of(&[("a.ies".into(), vec![b'x'; max + 1])]), "z.zip");
    assert_eq!(r[0].result, Err(PhotoError::TooLarge));
    // eight such entries are 64 MiB in all and still read; the ninth tips over the total
    let nine: Vec<(String, Vec<u8>)> = (0..9).map(|i| (format!("f{i}.ies"), vec![b'x'; max])).collect();
    let r = import(&zip_of(&nine), "z.zip");
    assert_eq!(r.len(), 9);
    assert!(r[..8].iter().all(|x| x.result == Err(PhotoError::UnknownFormat)));
    assert_eq!(r[8].result, Err(PhotoError::TooLarge));
    // folders and other file types are skipped; extensions in capitals count
    let r = import(&zip_of(&[("d/".into(), vec![]), ("n.txt".into(), b"a".to_vec()), ("A.LDT".into(), b"a".to_vec())]), "z.zip");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].name, "A.LDT");
}
