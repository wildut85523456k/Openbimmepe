use openbim_arch::glazing::*;
use openbim_arch::layers::*;
use openbim_arch::spaces::*;
use openbim_arch::*;
use proptest::prelude::*;

// Illustrative round numbers for testing only; not from any standard or product.
fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())) }
fn lib() -> Library { Library::new(vec![Material::new("Glass", 1.0), Material::new("Plaster", 0.5), Material::new("Insulation", 0.04)]).unwrap() }
fn double() -> Glazing {
    Glazing { name: "Double".into(), panes: vec![Pane { material: "Glass".into(), thickness: 0.006 }, Pane { material: "Glass".into(), thickness: 0.004 }], gaps: vec![Gap { thickness: 0.016, r: 0.2 }], r_se: 0.04, r_si: 0.13, declared_ug: None, g_value: Some(0.6), light_transmittance: Some(0.8) }
}
fn frame() -> Frame { Frame { name: "Frame".into(), uf: 2.0, psi: 0.06 } }

// ------------------------------------------------------------ material list and wall-type editor
#[test]
fn the_material_list_round_trips_through_text() {
    let l = Library::new(vec![
        Material { name: "Concrete".into(), conductivity: 1.5, density: Some(2300.0), specific_heat: Some(880.0), category: "Masonry".into() },
        Material::new("Air layer", 0.025),
    ]).unwrap();
    let t = l.to_text();
    assert!(t.starts_with("# name"));
    assert_eq!(Library::from_text(&t).unwrap(), l);
    // typed by hand: comments, blank lines, missing optional columns, CRLF
    let hand = "# my list\r\n\r\nBrick\t0.7\r\nFoam\t0.03\t30\t1400\tInsulation\r\n";
    let h = Library::from_text(hand).unwrap();
    assert_eq!(h.get("Brick").unwrap().conductivity, 0.7);
    assert_eq!(h.get("Foam").unwrap().density, Some(30.0));
    assert_eq!(h.get("Foam").unwrap().category, "Insulation");
    assert_eq!(h.get("Brick").unwrap().density, None);
}

#[test]
fn a_bad_list_file_names_the_line() {
    let e = Library::from_text("A\t1.0\nB\tx\n").unwrap_err();
    assert_eq!(e.0, 2);
    assert_eq!(Library::from_text("A\t1.0\nB\n").unwrap_err().0, 2);
    assert_eq!(Library::from_text("A\t1\t2\t3\tc\textra\n").unwrap_err().0, 1);
    assert_eq!(Library::from_text("A\t1.0\t-5\n").unwrap_err().1, ArchError::BadValue { what: "density", name: "A".into() });
    assert_eq!(Library::from_text("A\t1.0\t5\t0\n").unwrap_err().1, ArchError::BadValue { what: "specific heat", name: "A".into() });
    assert!(Library::from_text("A\t1.0\nA\t2.0\n").is_err());
    assert!(Library::from_text("A\t0\n").is_err());
    assert_eq!(Library::from_text("# nothing\n").unwrap().all().len(), 0);
}

#[test]
fn the_editor_breakdown_follows_each_change() {
    let l = Library::new(vec![Material::new("Plaster", 0.5), Material::new("Block", 1.0), Material::new("Insulation", 0.04)]).unwrap();
    let mut a = Assembly { name: "W".into(), kind: AssemblyKind::Wall, layers: vec![
        Layer::Solid { material: "Plaster".into(), thickness: 0.02 }, Layer::Solid { material: "Block".into(), thickness: 0.2 }, Layer::Solid { material: "Insulation".into(), thickness: 0.05 }], r_se: 0.04, r_si: 0.13 };
    let b = a.breakdown(&l).unwrap();
    // R = 0.17 + 0.04 + 0.2 + 1.25 = 1.66
    assert!(close(b.r_total, 1.66) && close(b.u, 1.0 / 1.66) && close(b.thickness, 0.27));
    assert!(close(b.layers[2].r, 1.25) && close(b.layers[2].share, 1.25 / 1.66));
    assert!(close(b.layers.iter().map(|x| x.share).sum::<f64>() + (b.r_se + b.r_si) / b.r_total, 1.0));
    assert_eq!(b.layers[1].label, "Block");
    // the designer picks a better insulation thickness: U follows
    a.layers[2] = Layer::Solid { material: "Insulation".into(), thickness: 0.1 };
    assert!(close(a.u_value(&l).unwrap(), 1.0 / (0.17 + 0.04 + 0.2 + 2.5)));
    // thickness needed for a target U: 1/0.25 = 4.0 total; others = 0.17 + 0.04 + 0.2 = 0.41; insulation R 3.59 -> 0.1436 m
    let t = a.thickness_for_u(2, 0.25, &l).unwrap();
    assert!(close(t, 3.59 * 0.04));
    a.layers[2] = Layer::Solid { material: "Insulation".into(), thickness: t };
    assert!(close(a.u_value(&l).unwrap(), 0.25));
    // the plaster alone cannot reach it, a target already met is refused, a bad layer is refused
    assert!(a.thickness_for_u(0, 0.25, &l).is_err() || a.thickness_for_u(0, 0.25, &l).unwrap() > 0.0);
    assert!(a.thickness_for_u(2, 10.0, &l).is_err());
    assert!(a.thickness_for_u(9, 0.25, &l).is_err());
    assert!(a.thickness_for_u(2, 0.0, &l).is_err());
    assert!(a.thickness_for_u(2, f64::NAN, &l).is_err());
    // too thick to be built: U 0.001 would need over 5 m
    assert!(a.thickness_for_u(2, 0.001, &l).is_err());
    // the chain gets the thickness
    let mut c = openbim_geom::WallChain::new(vec![[0.0, 0.0], [5.0, 0.0]], false, 0.1, 3.0);
    a.apply_to(&mut c, &l).unwrap();
    assert!(close(c.thickness, a.thickness()));
}

#[test]
fn a_resistance_layer_cannot_be_solved_for() {
    let l = lib();
    let a = Assembly { name: "W".into(), kind: AssemblyKind::Wall, layers: vec![Layer::Resistance { name: "Gap".into(), thickness: 0.03, r: 0.2 }, Layer::Solid { material: "Insulation".into(), thickness: 0.05 }], r_se: 0.04, r_si: 0.13 };
    assert!(a.thickness_for_u(0, 0.5, &l).is_err());
    assert!(a.thickness_for_u(1, 0.5, &l).is_ok());
}

// ------------------------------------------------------------ windows
#[test]
fn glazing_u_and_window_u_match_the_hand_calculation() {
    let l = lib();
    // Rg = 0.04 + 0.13 + 0.006/1.0 + 0.2 + 0.004/1.0 = 0.38 -> Ug = 2.631578...
    assert!(close(double().ug(&l).unwrap(), 1.0 / 0.38));
    assert!(close(double().thickness(), 0.006 + 0.016 + 0.004));
    let mut d = double(); d.declared_ug = Some(1.1);
    assert!(close(d.ug(&l).unwrap(), 1.1));
    // window 1.2 x 1.5, frame 0.07 all round, one bar 0.03 wide: pane width (1.2 - 0.14 - 0.03)/2 = 0.515, height 1.36
    let g = WindowGeometry { width: 1.2, height: 1.5, frame_width: 0.07, bars: 1, bar_width: 0.03 };
    let r = window_u(g, 1.1, &frame()).unwrap();
    let ag = 2.0 * 0.515 * 1.36; // 1.4008
    let lg = 2.0 * 2.0 * (0.515 + 1.36); // 7.5
    assert!(close(r.area, 1.8) && close(r.ag, ag) && close(r.af, 1.8 - ag) && close(r.lg, lg));
    assert!(close(r.uw, (ag * 1.1 + (1.8 - ag) * 2.0 + lg * 0.06) / 1.8));
    assert!(close(r.frame_fraction, (1.8 - ag) / 1.8));
    // numbers: Ag = 1.4008, Af = 0.3992 -> (1.54088 + 0.7984 + 0.45) / 1.8 = 1.54967...
    assert!((r.uw - 1.5496).abs() < 1e-9, "{}", r.uw);
}

#[test]
fn no_bars_and_a_bigger_window_lose_less_through_the_frame() {
    let f = frame();
    let small = window_u(WindowGeometry { width: 0.6, height: 0.6, frame_width: 0.07, bars: 0, bar_width: 0.0 }, 1.1, &f).unwrap();
    let big = window_u(WindowGeometry { width: 2.4, height: 1.8, frame_width: 0.07, bars: 0, bar_width: 0.0 }, 1.1, &f).unwrap();
    assert!(big.uw < small.uw && big.frame_fraction < small.frame_fraction);
    // single pane, no bars: Ag = (w - 2f)(h - 2f), lg = perimeter
    assert!(close(big.ag, (2.4 - 0.14) * (1.8 - 0.14)) && close(big.lg, 2.0 * (2.26 + 1.66)));
}

#[test]
fn bad_glazing_frames_and_windows_are_refused() {
    let l = lib();
    let bad = |f: &dyn Fn(&mut Glazing)| { let mut g = double(); f(&mut g); g.check(&l).is_err() };
    assert!(bad(&|g| g.panes.clear()));
    assert!(bad(&|g| g.gaps.clear()));
    assert!(bad(&|g| { g.panes.push(g.panes[0].clone()); }));
    assert!(bad(&|g| g.panes[0].thickness = 0.0005));
    assert!(bad(&|g| g.panes[0].thickness = 0.06));
    assert!(bad(&|g| g.panes[0].material = "Gold".into()));
    assert!(bad(&|g| g.gaps[0].thickness = 0.0));
    assert!(bad(&|g| g.gaps[0].thickness = 0.2));
    assert!(bad(&|g| g.gaps[0].r = 0.0));
    assert!(bad(&|g| g.gaps[0].r = 11.0));
    assert!(bad(&|g| g.r_se = -1.0));
    assert!(bad(&|g| g.r_si = 11.0));
    assert!(bad(&|g| g.declared_ug = Some(0.0)));
    assert!(bad(&|g| g.declared_ug = Some(21.0)));
    assert!(bad(&|g| g.g_value = Some(1.1)));
    assert!(bad(&|g| g.g_value = Some(-0.1)));
    assert!(bad(&|g| g.light_transmittance = Some(2.0)));
    assert!(bad(&|g| g.name = String::new()));
    assert!(!bad(&|g| g.g_value = Some(1.0)));
    assert!(!bad(&|g| g.g_value = Some(0.0)));
    assert!(!bad(&|g| g.declared_ug = Some(20.0)));
    assert!(!bad(&|g| g.gaps[0].r = 10.0));
    let six = Glazing { panes: (0..6).map(|_| Pane { material: "Glass".into(), thickness: 0.004 }).collect(), gaps: (0..5).map(|_| Gap { thickness: 0.01, r: 0.15 }).collect(), ..double() };
    assert!(six.check(&l).is_ok());
    let g = WindowGeometry { width: 1.2, height: 1.5, frame_width: 0.07, bars: 0, bar_width: 0.0 };
    for fr in [Frame { name: "".into(), uf: 2.0, psi: 0.0 }, Frame { name: "f".into(), uf: 0.0, psi: 0.0 }, Frame { name: "f".into(), uf: 21.0, psi: 0.0 }, Frame { name: "f".into(), uf: 2.0, psi: 1.5 }, Frame { name: "f".into(), uf: 2.0, psi: -0.1 }] {
        assert!(window_u(g, 1.1, &fr).is_err());
    }
    assert!(window_u(g, 0.0, &frame()).is_err());
    assert!(window_u(g, 21.0, &frame()).is_err());
    for g2 in [WindowGeometry { width: 0.0, ..g }, WindowGeometry { height: -1.0, ..g }, WindowGeometry { frame_width: 0.0, ..g }, WindowGeometry { frame_width: 0.6, ..g }, WindowGeometry { bars: 101, ..g }, WindowGeometry { bars: 30, bar_width: 0.05, ..g }, WindowGeometry { bar_width: -0.01, ..g }, WindowGeometry { width: f64::NAN, ..g }] {
        assert!(window_u(g2, 1.1, &frame()).is_err(), "{g2:?}");
    }
    // glass narrower than 1 cm is refused
    assert!(window_u(WindowGeometry { width: 0.16, height: 1.5, frame_width: 0.07, bars: 0, bar_width: 0.0 }, 1.1, &frame()).is_ok());
    assert!(window_u(WindowGeometry { width: 0.149, height: 1.5, frame_width: 0.07, bars: 0, bar_width: 0.0 }, 1.1, &frame()).is_err());
}

// ------------------------------------------------------------ ceilings and shafts
#[test]
fn the_ceiling_void_has_the_hand_calculated_depth() {
    let c = Ceiling { name: "Suspended".into(), height: 2.7, thickness: 0.05 };
    // level 3.6 m, structure (slab and beams) 0.4: void = 3.6 - 0.4 - 2.7 - 0.05 = 0.45
    assert!(close(c.void_depth(3.6, 0.4).unwrap(), 0.45));
    // a 0.25 m duct with 0.05 clearance each side needs 0.35: fits; a 0.4 duct needs 0.5: does not
    assert!(c.fits(3.6, 0.4, 0.25, 0.05).unwrap());
    assert!(!c.fits(3.6, 0.4, 0.4, 0.05).unwrap());
    // exactly full
    assert!(c.fits(3.6, 0.4, 0.35, 0.05).unwrap());
    // does not fit under the structure at all
    assert!(c.void_depth(3.0, 0.4).is_err());
    assert!(close(Ceiling { name: "x".into(), height: 2.55, thickness: 0.05 }.void_depth(3.0, 0.4).unwrap(), 0.0));
    for bad in [Ceiling { name: "".into(), height: 2.7, thickness: 0.05 }, Ceiling { name: "x".into(), height: 0.0, thickness: 0.05 }, Ceiling { name: "x".into(), height: 31.0, thickness: 0.05 }, Ceiling { name: "x".into(), height: 2.7, thickness: -0.1 }, Ceiling { name: "x".into(), height: 2.7, thickness: 2.1 }, Ceiling { name: "x".into(), height: f64::NAN, thickness: 0.0 }] {
        assert!(bad.check().is_err());
    }
    assert!(c.void_depth(0.0, 0.4).is_err());
    assert!(c.void_depth(3.6, -0.1).is_err());
    assert!(c.fits(3.6, 0.4, -0.1, 0.0).is_err());
    assert!(c.fits(3.6, 0.4, 0.1, -0.1).is_err());
}

fn levels() -> Levels { Levels::new(vec![Level { name: "G".into(), elevation: 0.0 }, Level { name: "1".into(), elevation: 3.6 }, Level { name: "2".into(), elevation: 7.2 }, Level { name: "Roof".into(), elevation: 10.8 }]).unwrap() }
fn sq(x: f64, y: f64, s: f64) -> Vec<[f64; 2]> { vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s]] }

#[test]
fn a_riser_shaft_cuts_the_floors_between_and_has_a_height() {
    let sh = Shaft { name: "Riser 1".into(), usage: "Plumbing".into(), outline: sq(10.0, 2.0, 0.8), from: 0, to: 3 };
    sh.check(&levels()).unwrap();
    assert!(close(sh.area(), 0.64));
    assert_eq!(sh.floors_cut(), vec![1, 2]);
    assert!(close(sh.height(&levels()).unwrap(), 10.8));
    let low = Shaft { from: 1, to: 2, ..sh.clone() };
    assert!(low.floors_cut().is_empty() && close(low.height(&levels()).unwrap(), 3.6));
    // clockwise outline: same area
    let mut cw = sh.clone(); cw.outline.reverse();
    assert!(close(cw.area(), 0.64));
    // a 0.5 x 0.3 bundle centred in the shaft fits, 0.9 wide does not, one off-centre pokes out
    assert!(sh.contains_rect([10.4, 2.4], 0.5, 0.3));
    assert!(!sh.contains_rect([10.4, 2.4], 0.9, 0.3));
    assert!(!sh.contains_rect([10.7, 2.4], 0.5, 0.3));
    assert!(!sh.contains_rect([20.0, 20.0], 0.1, 0.1));
}

#[test]
fn bad_shafts_are_refused() {
    let lv = levels();
    let ok = Shaft { name: "S".into(), usage: "".into(), outline: sq(0.0, 0.0, 1.0), from: 0, to: 1 };
    assert!(ok.check(&lv).is_ok());
    let bad: Vec<Shaft> = vec![
        Shaft { name: "".into(), ..ok.clone() },
        Shaft { usage: "a\nb".into(), ..ok.clone() },
        Shaft { from: 4, ..ok.clone() },
        Shaft { to: 4, ..ok.clone() },
        Shaft { from: 1, to: 1, ..ok.clone() },
        Shaft { from: 2, to: 1, ..ok.clone() },
        Shaft { outline: vec![[0.0, 0.0], [1.0, 0.0]], ..ok.clone() },
        Shaft { outline: vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]], ..ok.clone() },
        Shaft { outline: vec![[0.0, 0.0], [1.0, 1.0], [1.0, 0.0], [0.0, 1.0]], ..ok.clone() }, // bow tie
        Shaft { outline: vec![[0.0, 0.0], [1.0, f64::NAN], [1.0, 1.0]], ..ok.clone() },
    ];
    for (i, s) in bad.iter().enumerate() { assert!(s.check(&lv).is_err(), "case {i}"); }
    assert_eq!(Shaft { from: 4, ..ok.clone() }.check(&lv).unwrap_err(), ArchError::UnknownLevel(4));
    assert_eq!(Shaft { to: 7, ..ok.clone() }.check(&lv).unwrap_err(), ArchError::UnknownLevel(7));
    // overlap rules
    let a = Shaft { name: "A".into(), ..ok.clone() };
    let b = Shaft { name: "B".into(), outline: sq(0.5, 0.5, 1.0), ..ok.clone() };
    assert!(check_shafts(&[a.clone(), b.clone()], &lv).is_err());
    // same plan position but different heights: fine, they do not share any height
    let b_high = Shaft { from: 1, to: 2, ..b.clone() };
    assert!(check_shafts(&[a.clone(), b_high], &lv).is_ok());
    // one inside the other
    let inner = Shaft { name: "C".into(), outline: sq(0.2, 0.2, 0.3), ..ok.clone() };
    assert!(check_shafts(&[a.clone(), inner], &lv).is_err());
    // side by side
    let far = Shaft { name: "D".into(), outline: sq(5.0, 0.0, 1.0), ..ok.clone() };
    assert!(check_shafts(&[a.clone(), far], &lv).is_ok());
    assert!(check_shafts(&[a.clone(), a.clone()], &lv).is_err());
}

proptest! {
    #[test]
    fn window_u_lies_between_glass_and_frame_values_when_psi_is_zero(w in 0.4f64..4.0, h in 0.4f64..4.0, fw in 0.02f64..0.15, bars in 0u32..5, ug in 0.3f64..5.0, uf in 0.5f64..8.0) {
        let g = WindowGeometry { width: w, height: h, frame_width: fw, bars, bar_width: 0.03 };
        let fr = Frame { name: "f".into(), uf, psi: 0.0 };
        let r = window_u(g, ug, &fr).unwrap();
        let (lo, hi) = (ug.min(uf), ug.max(uf));
        prop_assert!(r.uw >= lo - 1e-12 && r.uw <= hi + 1e-12);
        prop_assert!((r.ag + r.af - r.area).abs() < 1e-12);
        prop_assert!(r.frame_fraction > 0.0 && r.frame_fraction < 1.0);
        // psi only ever adds loss
        let with_psi = window_u(g, ug, &Frame { psi: 0.05, ..fr.clone() }).unwrap();
        prop_assert!(with_psi.uw > r.uw);
    }

    #[test]
    fn void_depth_plus_stack_equals_the_level_height(lh in 3.0f64..8.0, st in 0.1f64..0.8, ch in 2.2f64..2.9, ct in 0.0f64..0.3) {
        prop_assume!(lh - st - ch - ct >= 0.0);
        let c = Ceiling { name: "c".into(), height: ch, thickness: ct };
        let v = c.void_depth(lh, st).unwrap();
        prop_assert!((v + st + ch + ct - lh).abs() < 1e-12);
    }
}

// ------------------------------------------------------------ edges found by the mutation test
fn what(e: ArchError) -> String { match e { ArchError::BadValue { what, .. } => what.to_string(), other => format!("{other:?}") } }

#[test]
fn material_limits_and_target_edges() {
    let mk = |d: Option<f64>, c: Option<f64>, cat: &str| Library::new(vec![Material { name: "m".into(), conductivity: 1.0, density: d, specific_heat: c, category: cat.into() }]);
    assert!(mk(Some(0.0), None, "").is_err());
    assert!(mk(Some(MAX_DENSITY), None, "").is_ok() && mk(Some(MAX_DENSITY + 1.0), None, "").is_err());
    assert!(mk(None, Some(MAX_SPECIFIC_HEAT), "").is_ok() && mk(None, Some(MAX_SPECIFIC_HEAT + 1.0), "").is_err());
    assert!(mk(None, Some(0.0), "").is_err());
    assert!(mk(None, None, &"c".repeat(128)).is_ok());
    assert!(mk(None, None, &"c".repeat(129)).is_err());
    assert!(mk(None, None, "bad\tcat").is_err());
    // targets: exact binary numbers so the equality edge is real
    let l = Library::new(vec![Material::new("Ins", 0.5)]).unwrap();
    let a = Assembly { name: "A".into(), kind: AssemblyKind::Wall, layers: vec![Layer::Resistance { name: "Gap".into(), thickness: 0.5, r: 0.5 }, Layer::Solid { material: "Ins".into(), thickness: 0.25 }], r_se: 0.25, r_si: 0.25 };
    // the other layers and surfaces already give exactly 1.0: a target U of 1.0 needs nothing from the insulation
    assert_eq!(what(a.thickness_for_u(1, 1.0, &l).unwrap_err()), "target U (already met without this layer)");
    assert_eq!(what(a.thickness_for_u(1, 0.0, &l).unwrap_err()), "target U");
    // a target of 0.5 needs R = 2: insulation R 1.0 -> thickness 0.5
    assert_eq!(a.thickness_for_u(1, 0.5, &l).unwrap(), 0.5);
    // the rest of the build-up plus the new layer may not pass 5 m even if the layer alone is under 5 m
    let big = Assembly { layers: vec![Layer::Resistance { name: "Gap".into(), thickness: 3.0, r: 0.5 }, Layer::Solid { material: "Ins".into(), thickness: 0.25 }], ..a.clone() };
    assert_eq!(big.thickness_for_u(1, 0.2, &l).unwrap(), 2.0); // 3.0 + 2.0 = 5.0 m in total is still allowed
    let tight = Assembly { layers: vec![Layer::Resistance { name: "Gap".into(), thickness: 3.5, r: 0.5 }, Layer::Solid { material: "Ins".into(), thickness: 0.25 }], ..a.clone() };
    assert!(tight.thickness_for_u(1, 0.2, &l).is_err()); // 3.5 + 2.0 > 5
    assert!(tight.thickness_for_u(1, 0.5, &l).is_ok());  // 3.5 + 0.5 = 4.0
}

#[test]
fn glazing_edges() {
    // glass with a conductivity other than 1 so a multiplication cannot pass for a division
    let l = Library::new(vec![Material::new("Glass", 0.5)]).unwrap();
    let g = Glazing { panes: vec![Pane { material: "Glass".into(), thickness: 0.004 }, Pane { material: "Glass".into(), thickness: 0.004 }], gaps: vec![Gap { thickness: 0.016, r: 0.2 }], ..double() };
    // R = 0.04 + 0.13 + 0.004/0.5 + 0.2 + 0.004/0.5 = 0.386
    assert!(close(g.ug(&l).unwrap(), 1.0 / 0.386));
    let ok = |f: &dyn Fn(&mut Glazing)| { let mut x = g.clone(); f(&mut x); x.check(&l).is_ok() };
    assert!(ok(&|x| x.panes[0].thickness = 0.05));
    assert!(ok(&|x| x.r_se = 10.0) && ok(&|x| x.r_si = 10.0) && !ok(&|x| x.r_se = 10.5));
    assert!(ok(&|x| x.panes[0].thickness = 0.001) && ok(&|x| x.gaps[0].thickness = 0.001) && ok(&|x| x.gaps[0].thickness = 0.1));
    let mut none = g.clone(); none.panes.clear(); none.gaps.clear();
    assert!(none.check(&l).is_err());
    let fr = frame();
    let w = WindowGeometry { width: 1.2, height: 1.5, frame_width: 0.07, bars: 0, bar_width: 0.0 };
    // a window with no width or height says so, with the size message
    assert_eq!(what(window_u(WindowGeometry { width: 0.0, ..w }, 1.1, &fr).unwrap_err()), "window size");
    assert_eq!(what(window_u(WindowGeometry { height: 0.0, ..w }, 1.1, &fr).unwrap_err()), "window size");
    // glass too short: 0.145 - 0.14 = 5 mm of glass height
    assert!(window_u(WindowGeometry { height: 0.145, ..w }, 1.1, &fr).is_err());
    assert!(window_u(WindowGeometry { height: 0.16, ..w }, 1.1, &fr).is_ok());
}

#[test]
fn ceiling_and_shaft_edges() {
    let c = |h: f64, t: f64| Ceiling { name: "c".into(), height: h, thickness: t };
    assert!(c(30.0, 0.0).check().is_ok() && c(30.5, 0.0).check().is_err());
    assert!(c(2.7, 0.0).check().is_ok() && c(2.7, 2.0).check().is_ok() && c(2.7, 2.5).check().is_err());
    // exact binary numbers: level 4.0, structure 0.5, ceiling 3.0 + 0.25 leaves a void of exactly 0.25
    let k = c(3.0, 0.25);
    assert_eq!(k.void_depth(4.0, 0.5).unwrap(), 0.25);
    assert!(k.fits(4.0, 0.5, 0.125, 0.0625).unwrap());      // 0.125 + 2 x 0.0625 = 0.25: exactly full
    assert!(!k.fits(4.0, 0.5, 0.1251, 0.0625).unwrap());
    // a ceiling filling the whole level height leaves no void at all, and a void of exactly zero is allowed
    assert_eq!(c(3.25, 0.25).void_depth(4.0, 0.5).unwrap(), 0.0);
    assert_eq!(what(c(2.0, 0.0).void_depth(0.0, 0.0).unwrap_err()), "level height or structure depth");
    // shafts
    let lv = levels();
    let two = Shaft { name: "S".into(), usage: String::new(), outline: vec![[0.0, 0.0], [1.0, 0.0]], from: 0, to: 1 };
    assert_eq!(what(two.check(&lv).unwrap_err()), "shaft outline points");
    let cw = Shaft { name: "S".into(), usage: String::new(), outline: vec![[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]], from: 0, to: 1 };
    assert!(cw.check(&lv).is_ok());
    // a self-crossing outline with a non-zero area (unequal lobes)
    let x = Shaft { outline: vec![[0.0, 0.0], [4.0, 0.0], [0.0, 2.0], [5.0, 2.0]], ..cw.clone() };
    assert_eq!(what(x.check(&lv).unwrap_err()), "shaft outline (crosses itself)");
}
