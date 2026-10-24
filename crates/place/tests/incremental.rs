//! `Model::update_built` must give what a full `build` of the changed model gives, bit for bit, and leave model and
//! result alone when the change cannot be made.
use openbim_family::library;
use openbim_geom::*;
use openbim_place::penetrations::Run;
use openbim_place::*;
use proptest::prelude::*;

fn pl(family: usize, chain: usize, segment: usize, centre: f64) -> Placement {
    Placement { family, type_name: None, overrides: vec![], texts: vec![], chain, segment, centre }
}

fn zigzag(n: usize) -> Vec<[f64; 2]> {
    let mut p = vec![[0.0, 0.0]];
    for i in 0..n {
        let l = p[i];
        p.push(match i % 4 { 0 => [l[0] + 4.0, l[1]], 1 => [l[0], l[1] + 4.0], 2 => [l[0] + 4.0, l[1]], _ => [l[0], l[1] - 4.0] });
    }
    p
}

/// Three chains (a zigzag, a closed room with an opening of its own, a straight wall) and two pipes through the straight wall.
fn model(placements: Vec<Placement>) -> Model {
    use openbim_family::mep::{Domain, System};
    let mut fams = library::all().unwrap();
    fams.remove(0); // door, window, arched door, fan coil
    let mut zig = WallChain::new(zigzag(12), false, 0.2, 3.0);
    zig.base_z = 0.0;
    let mut room = WallChain::new(vec![[100.0, 0.0], [106.0, 0.0], [106.0, 5.0], [100.0, 5.0]], true, 0.25, 3.0);
    room.openings = vec![vec![Opening { offset: 0.5, width: 0.4, sill: 1.0, height: 0.4 }]];
    let straight = WallChain::new(vec![[200.0, 0.0], [210.0, 0.0]], false, 0.3, 3.0);
    let runs = vec![
        Run::round("p1", Domain::Pipe, System::ChilledWaterSupply, [203.0, -2.0, 1.5], [203.0, 2.0, 1.5], 0.1),
        Run::round("p2", Domain::Pipe, System::ChilledWaterReturn, [207.0, -2.0, 1.2], [207.0, 2.0, 1.2], 0.08),
    ];
    Model { chains: vec![zig, room, straight], families: fams, placements, runs, ..Default::default() }
}

fn start() -> Vec<Placement> {
    // one placement on each of the twelve segments of the zigzag, plus some on the room and the straight wall
    let mut v: Vec<Placement> = (0..12).map(|s| pl(s % 4, 0, s, 2.0)).collect();
    v.extend([pl(0, 1, 1, 2.5), pl(1, 1, 2, 3.0), pl(0, 2, 0, 1.5), pl(1, 2, 0, 8.5)]);
    v
}

fn describe(b: &Built) -> String {
    format!("{:?}|{:?}|{:?}|{:?}", b.chains, b.instances, b.penetrations, b.volume.to_bits())
}

/// An edit that is usually good (a spot well inside a wall segment), now and then a bad reference or a spot outside the wall.
fn edit() -> impl Strategy<Value = (usize, Placement)> {
    (0usize..16, 0usize..4, 0usize..3, 0usize..12, 0.25f64..0.75, proptest::option::of(0.0f64..1.2), 0u8..20).prop_map(|(i, family, chain, seg, frac, sill, wild)| {
        let (segment, len) = match chain { 0 => (seg, 4.0), 1 => (seg % 4, if seg % 2 == 0 { 6.0 } else { 5.0 }), _ => (0, 10.0) };
        let mut p = pl(family, chain, segment, frac * len);
        if let Some(s) = sill { p.overrides.push(("sill".into(), s)); }
        match wild {
            0 => p.chain = 7,
            1 => p.segment = 40,
            2 => p.centre = len + 3.0,
            3 => p.family = 9,
            _ => {}
        }
        (i, p)
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 60, ..ProptestConfig::default() })]
    #[test]
    fn every_edit_gives_what_a_full_build_gives(edits in proptest::collection::vec(edit(), 1..20)) {
        let mut inc = model(start());
        let mut full = model(start());
        let mut built = inc.build().unwrap();
        prop_assert_eq!(describe(&built), describe(&full.build().unwrap()));
        for (i, p) in edits {
            let before = describe(&built);
            // the quick path when it can do the edit; whatever it leaves to the full build goes through `update_built`
            let r_inc = if inc.update_quick(&mut built, i, p.clone()) { Ok(()) } else { inc.update_built(&mut built, i, p.clone()) };
            let r_full = full.try_update(i, p);
            match (r_inc, r_full) {
                (Ok(()), Ok(b)) => prop_assert_eq!(describe(&built), describe(&b)),
                (Err(a), Err(b)) => {
                    prop_assert_eq!(a, b);
                    prop_assert_eq!(describe(&built), before, "a refused edit changed the result");
                }
                (a, b) => prop_assert!(false, "one accepted and one refused: {:?} / {:?}", a, b.map(|_| ())),
            }
            prop_assert_eq!(&inc.placements, &full.placements);
        }
    }
}

#[test]
fn an_edit_moves_a_door_to_another_wall_and_the_volumes_follow() {
    let mut m = model(start());
    let mut b = m.build().unwrap();
    let before = b.volume;
    m.update_built(&mut b, 0, pl(0, 1, 3, 2.5)).unwrap();
    let fresh = m.build().unwrap();
    assert_eq!(b.volume.to_bits(), fresh.volume.to_bits());
    assert_ne!(b.volume.to_bits(), before.to_bits());
    // a bad reference is refused with the full build's error and nothing changes
    let keep = describe(&b);
    let e = m.update_built(&mut b, 0, pl(0, 9, 0, 1.0)).unwrap_err();
    assert_eq!(e, PlaceError::BadReference { placement: 0, what: "no such wall chain" });
    assert_eq!(describe(&b), keep);
    assert_eq!(m.placements[0], pl(0, 1, 3, 2.5));
    assert_eq!(m.update_built(&mut b, 99, pl(0, 0, 0, 1.0)).unwrap_err(), PlaceError::BadReference { placement: 99, what: "no such placement" });
}

#[test]
fn most_edits_take_the_quick_path_and_the_rest_the_full_build() {
    // the quick path is the one that matters for speed: check it is really used, and that a refused edit leaves everything alone
    let mut m = model(start());
    let mut b = m.build().unwrap();
    for (i, p) in [(0, pl(1, 0, 11, 1.5)), (1, pl(0, 0, 1, 3.0)), (2, pl(2, 1, 3, 2.0)), (5, pl(1, 2, 0, 5.0)), (9, pl(3, 0, 3, 1.0))] {
        assert!(m.update_quick(&mut b, i, p.clone()), "edit {i} {p:?}");
    }
    let fresh = m.build().unwrap();
    assert_eq!(describe(&b), describe(&fresh));
    let keep = describe(&b);
    // an opening that cannot fit its wall is not done on the quick path, and nothing was touched
    assert!(!m.update_quick(&mut b, 0, pl(0, 0, 0, 100.0)));
    assert_eq!(describe(&b), keep);
    assert_eq!(m.placements[0], pl(1, 0, 11, 1.5));
}

#[test]
fn a_result_that_does_not_belong_to_the_model_is_never_patched() {
    // the result of another model: wrong number of placements, of chains
    let mut m = model(start());
    let mut other = model(start()[..8].to_vec()).build().unwrap();
    assert!(!m.update_quick(&mut other, 0, pl(0, 0, 0, 1.5)));
    let mut fewer = { let mut o = model(start()); o.chains.pop(); o.runs.clear(); o.placements.retain(|p| p.chain < 2); o };
    let mut fb = fewer.build().unwrap();
    assert!(!m.update_quick(&mut fb, 0, pl(0, 0, 0, 1.5)));
    // the same model with a chain taken away after the build: the counts of chains differ
    let mut b = m.build().unwrap();
    m.chains.push(WallChain::new(vec![[300.0, 0.0], [304.0, 0.0]], false, 0.2, 3.0));
    assert!(!m.update_quick(&mut b, 0, pl(0, 0, 0, 1.5)));
    // update_built then falls back to the full build and the answer is the true one
    m.update_built(&mut b, 0, pl(0, 0, 0, 1.5)).unwrap();
    assert_eq!(b.volume.to_bits(), m.build().unwrap().volume.to_bits());
    let _ = &mut fewer;
}

#[test]
fn a_position_that_is_not_a_number_is_refused_as_the_full_build_refuses_it() {
    for bad in [f64::NAN, f64::INFINITY] {
        let mut m = model(start());
        let mut b = m.build().unwrap();
        let keep = describe(&b);
        assert!(!m.update_quick(&mut b, 0, pl(0, 0, 0, bad)));
        assert_eq!(m.update_built(&mut b, 0, pl(0, 0, 0, bad)).unwrap_err(), PlaceError::NotFinite { placement: 0 });
        assert_eq!(describe(&b), keep);
        assert_eq!(m.placements[0], pl(0, 0, 0, 2.0));
    }
    // equipment has no opening, so only the check of the position itself can stop it
    let mut m = model(start());
    let mut b = m.build().unwrap();
    assert!(!m.update_quick(&mut b, 3, pl(3, 0, 3, f64::NAN)));
    assert_eq!(m.update_built(&mut b, 3, pl(3, 0, 3, f64::NAN)).unwrap_err(), PlaceError::NotFinite { placement: 3 });
    // a segment that does not exist, while the family and chain are good
    let mut m = model(start());
    let mut b = m.build().unwrap();
    assert!(!m.update_quick(&mut b, 0, pl(0, 0, 12, 1.0)));
    // a family index past the end of the list, whatever the library holds
    let past = m.families.len();
    assert!(!m.update_quick(&mut b, 0, pl(past, 0, 0, 1.0)));
    assert!(!m.update_quick(&mut b, 0, pl(0, 9, 0, 1.0)));
    // the old position of the placement no longer exists in the chain (the chain was shortened after the build)
    let mut m = model(start());
    let mut b = m.build().unwrap();
    m.chains[0].points.truncate(6);
    assert!(!m.update_quick(&mut b, 11, pl(3, 0, 1, 1.0)));
}
