use openbim_family::*;
use openbim_geom::*;
use openbim_place::*;

fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())) }
fn arch_family() -> Compiled {
    let f = Family {
        name: "Arch".into(),
        values: vec![
            ValueDef::input("width", 1.0, 0.1, 10.0, Scope::Instance),
            ValueDef::input("height", 2.4, 0.1, 10.0, Scope::Instance),
            ValueDef::input("rise", 0.3, 0.0, 10.0, Scope::Instance),
            ValueDef::input("sill", 0.0, 0.0, 10.0, Scope::Instance),
        ],
        opening: Some(OpeningDef::new("width", "height", "sill").arched("rise")),
        ..Default::default()
    };
    compile_library(&[f]).unwrap().remove(0)
}
fn pl(centre: f64, ov: &[(&str, f64)]) -> Placement {
    Placement { family: 0, type_name: None, overrides: ov.iter().map(|(n, v)| (n.to_string(), *v)).collect(), texts: vec![], chain: 0, segment: 0, centre }
}
fn wall() -> WallChain { WallChain::new(vec![[0.0, 0.0], [6.0, 0.0]], false, 0.25, 3.2) }
/// segment area of an arch polygon of N pieces
fn arch_area(w: f64, h: f64, r: f64) -> f64 {
    let half = w / 2.0;
    let rr = (half * half + r * r) / (2.0 * r);
    let a = (half / rr).asin();
    let n = ARCH_SEGMENTS as f64;
    w * (h - r) + 0.5 * rr * rr * (n * (2.0 * a / n).sin() - (2.0 * a).sin())
}
fn model(ps: Vec<Placement>) -> Model { Model { chains: vec![wall()], families: vec![arch_family()], placements: ps } }

#[test]
fn an_arched_door_cuts_the_hand_calculated_volume_and_the_mesh_agrees() {
    let b = model(vec![pl(2.0, &[])]).build().unwrap();
    let want = (6.0 * 3.2 - arch_area(1.0, 2.4, 0.3)) * 0.25;
    assert!(close(b.volume, want), "{} vs {}", b.volume, want);
    assert!(b.chains[0].openings[0].is_empty() && b.chains[0].poly_openings[0].len() == 1);
    let m = b.chains[0].chain_mesh().unwrap();
    assert!(m.is_closed_manifold() && close(m.volume(), want));
}

#[test]
fn an_arched_window_with_a_sill_is_a_hole_and_mixes_with_a_flat_one() {
    let b = model(vec![pl(1.5, &[("sill", 0.9), ("height", 1.2), ("rise", 0.25), ("width", 1.2)]), pl(4.0, &[("rise", 0.0), ("width", 0.9), ("height", 2.1)])]).build().unwrap();
    let want = (6.0 * 3.2 - arch_area(1.2, 1.2, 0.25) - 0.9 * 2.1) * 0.25;
    assert!(close(b.volume, want), "{} vs {}", b.volume, want);
    assert!(b.chains[0].openings[0].len() == 1 && b.chains[0].poly_openings[0].len() == 1);
    let m = b.chains[0].chain_mesh().unwrap();
    assert!(m.is_closed_manifold() && close(m.volume(), want));
    assert_eq!(m.euler_characteristic(), 0); // one door notch (sphere) + one hole = torus-like genus 1
}

#[test]
fn trouble_is_reported_against_the_placement() {
    // crown too close to the top of the wall
    let e = model(vec![pl(2.0, &[("height", 3.2)])]).build().err().unwrap();
    assert!(matches!(e, PlaceError::Wall { placement: 0, .. }), "{e:?}");
    // arch too wide for its rise
    let e = model(vec![pl(2.0, &[("rise", 0.6)])]).build().err().unwrap();
    assert!(matches!(e, PlaceError::Family { placement: 0, err: FamilyError::BadArch(_) }), "{e:?}");
    // two arches that touch
    let e = model(vec![pl(2.0, &[]), pl(2.5, &[])]).build().err().unwrap();
    assert!(matches!(e, PlaceError::Wall { .. }), "{e:?}");
    // arch spilling over the end of the wall
    let e = model(vec![pl(0.3, &[])]).build().err().unwrap();
    assert!(matches!(e, PlaceError::Wall { placement: 0, .. }), "{e:?}");
}

#[test]
fn the_build_is_the_same_on_one_and_many_threads() {
    let m = model((0..5).map(|k| pl(0.8 + 1.1 * k as f64, &[("rise", 0.1 + 0.05 * k as f64)])).collect());
    let one = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap().install(|| m.build().unwrap().volume);
    let four = rayon::ThreadPoolBuilder::new().num_threads(4).build().unwrap().install(|| m.build().unwrap().volume);
    assert_eq!(one.to_bits(), four.to_bits());
}

#[test]
fn arches_go_into_the_wall_segment_they_are_placed_on() {
    let room = WallChain::new(vec![[0.0, 0.0], [6.0, 0.0], [6.0, 5.0], [0.0, 5.0]], true, 0.25, 3.2);
    let p = |seg: usize, c: f64, rise: f64| Placement { segment: seg, ..pl(c, &[("rise", rise)]) };
    let m = Model { chains: vec![room.clone()], families: vec![arch_family()], placements: vec![p(2, 3.0, 0.3), p(1, 2.5, 0.2)] };
    let b = m.build().unwrap();
    let counts: Vec<usize> = (0..4).map(|s| b.chains[0].poly_openings.get(s).map_or(0, |v| v.len())).collect();
    assert_eq!(counts, vec![0, 1, 1, 0]);
    let plain = openbim_geom::total_volume_serial(std::slice::from_ref(&room)).unwrap();
    let want = plain - (arch_area(1.0, 2.4, 0.3) + arch_area(1.0, 2.4, 0.2)) * 0.25;
    assert!(close(b.volume, want), "{} vs {}", b.volume, want);
}
