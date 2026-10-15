use openbim_arch::spaces::Shaft;
use openbim_arch::{Level, Levels};
use openbim_family::{Domain, System};
use openbim_place::floors::*;
use openbim_place::penetrations::{Run, RunProblem};

fn slab(top: f64, t: f64, fire: Option<u32>) -> Slab {
    Slab { name: "L1 floor".into(), top, thickness: t, outline: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0]], fire_rating_min: fire }
}
fn pipe(a: [f64; 3], b: [f64; 3], d: f64) -> Run {
    Run::round("p", Domain::Pipe, System::DomesticCold, a, b, d)
}
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn plumb_round_riser_gets_its_own_square_opening_with_margin() {
    let (p, i) = find_floor(&[slab(3.0, 0.2, None)], &[], &[pipe([1.0, 1.0, 0.0], [1.0, 1.0, 6.0], 0.1).with_margin(0.025)], 0.0);
    assert!(i.is_empty());
    assert_eq!(p.len(), 1);
    let o = &p[0];
    assert!(close(o.x0, 0.925) && close(o.x1, 1.075) && close(o.y0, 0.925) && close(o.y1, 1.075), "{o:?}");
    assert_eq!(o.kind, FloorOpeningKind::Cut);
    assert!(o.fits);
    assert!(close(o.centre[2], 2.9));
    assert_eq!(o.id.to_string(), "F0-P1");
    assert!(!o.in_fire_partition());
}

#[test]
fn leaning_riser_opening_is_longer_along_the_lean() {
    // 45 degrees along x: axis passes x=2.8 at the slab bottom and 3.0 at the top
    let r = pipe([0.0, 1.0, 0.0], [6.0, 1.0, 6.0], 0.1);
    let (p, _) = find_floor(&[slab(3.0, 0.2, None)], &[], &[r], 0.0);
    let o = &p[0];
    let s2 = 2f64.sqrt();
    assert!(close(o.x0, 2.8 - 0.05 * s2) && close(o.x1, 3.0 + 0.05 * s2), "{o:?}");
    assert!(close(o.y0, 0.95) && close(o.y1, 1.05));
}

#[test]
fn rect_riser_and_roll() {
    let r = Run::rect("d", Domain::Duct, System::SupplyAir, [4.0, 4.0, 0.0], [4.0, 4.0, 6.0], 0.6, 0.3);
    let (p, _) = find_floor(&[slab(3.0, 0.2, Some(60))], &[], &[r], 0.0);
    let o = &p[0];
    // a vertical duct: width along the horizontal direction d x up, with up = +Y for a vertical run
    let (sx, sy) = (o.size_x(), o.size_y());
    assert!((close(sx, 0.6) && close(sy, 0.3)) || (close(sx, 0.3) && close(sy, 0.6)), "{o:?}");
    assert!(o.in_fire_partition());
    assert_eq!(o.fire_rating_min, Some(60));
    assert!(close(o.area(), 0.18));
}

#[test]
fn close_risers_share_one_rectangular_opening_and_far_ones_do_not() {
    let runs = [
        pipe([1.0, 1.0, 0.0], [1.0, 1.0, 6.0], 0.1),
        pipe([1.25, 1.0, 0.0], [1.25, 1.0, 6.0], 0.1),
        pipe([5.0, 5.0, 0.0], [5.0, 5.0, 6.0], 0.1),
    ];
    let (p, _) = find_floor(&[slab(3.0, 0.2, None)], &[], &runs, 0.2);
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].runs, vec![0, 1]);
    assert!(close(p[0].x0, 0.95) && close(p[0].x1, 1.3) && close(p[0].y0, 0.95) && close(p[0].y1, 1.05));
    assert_eq!(p[1].runs, vec![2]);
    // gap larger than the merge gap keeps them apart
    let (p, _) = find_floor(&[slab(3.0, 0.2, None)], &[], &runs, 0.1);
    assert_eq!(p.len(), 3);
}

#[test]
fn level_runs_and_runs_beside_or_short_of_the_slab_make_nothing() {
    let s = [slab(3.0, 0.2, None)];
    // level, under the slab
    assert!(find_floor(&s, &[], &[pipe([1.0, 1.0, 2.0], [5.0, 1.0, 2.0], 0.1)], 0.0).0.is_empty());
    // vertical but outside the outline
    assert!(find_floor(&s, &[], &[pipe([12.0, 1.0, 0.0], [12.0, 1.0, 6.0], 0.1)], 0.0).0.is_empty());
    // vertical but stops below the slab
    assert!(find_floor(&s, &[], &[pipe([1.0, 1.0, 0.0], [1.0, 1.0, 2.5], 0.1)], 0.0).0.is_empty());
    // touches the underside only
    assert!(find_floor(&s, &[], &[pipe([1.0, 1.0, 0.0], [1.0, 1.0, 2.8], 0.1)], 0.0).0.is_empty());
}

#[test]
fn run_lying_in_or_ending_in_the_slab_is_a_problem() {
    let s = [slab(3.0, 0.2, None)];
    let (p, i) = find_floor(&s, &[], &[pipe([1.0, 1.0, 2.9], [5.0, 1.0, 2.9], 0.1)], 0.0);
    assert!(p.is_empty());
    assert_eq!(i[0].problem, RunProblem::InsideSlab);
    let (p, i) = find_floor(&s, &[], &[pipe([1.0, 1.0, 0.0], [1.0, 1.0, 2.9], 0.1)], 0.0);
    assert!(p.is_empty());
    assert_eq!(i[0].problem, RunProblem::EndsInSlab);
    let (_, i) = find_floor(&s, &[], &[pipe([1.0, 1.0, 2.9], [1.0, 1.0, 6.0], 0.1)], 0.0);
    assert_eq!(i[0].problem, RunProblem::EndsInSlab);
}

#[test]
fn riser_through_a_shaft_is_recorded_as_in_the_shaft() {
    let levels = Levels::new(vec![Level { name: "L0".into(), elevation: 0.0 }, Level { name: "L1".into(), elevation: 3.0 }, Level { name: "L2".into(), elevation: 6.0 }]).unwrap();
    let sh = Shaft { name: "S1".into(), usage: "plumbing".into(), outline: vec![[0.5, 0.5], [2.0, 0.5], [2.0, 2.0], [0.5, 2.0]], from: 0, to: 2 };
    let v = ShaftVoid::from_shaft(&sh, &levels).unwrap();
    let runs = [pipe([1.0, 1.0, 0.0], [1.0, 1.0, 9.0], 0.1), pipe([5.0, 5.0, 0.0], [5.0, 5.0, 9.0], 0.1)];
    let slabs = [slab(3.0, 0.2, Some(60)), Slab { name: "L2 floor".into(), top: 6.0, thickness: 0.2, outline: slab(0.0, 1.0, None).outline, fire_rating_min: None }];
    let (p, _) = find_floor(&slabs, &[v], &runs, 0.0);
    // slab at 3.0 is cut by the shaft; slab at 6.0 is at the shaft's top so it is not
    let in_shaft: Vec<_> = p.iter().filter(|o| matches!(o.kind, FloorOpeningKind::InShaft(0))).collect();
    assert_eq!(in_shaft.len(), 1);
    assert_eq!(in_shaft[0].id.slab, 0);
    assert_eq!(p.iter().filter(|o| o.kind == FloorOpeningKind::Cut).count(), 3);
}

#[test]
fn opening_that_does_not_fit_the_slab_outline_is_flagged() {
    // pipe within 0.02 of the slab edge: centre inside, rectangle sticks out
    let (p, _) = find_floor(&[slab(3.0, 0.2, None)], &[], &[pipe([0.02, 1.0, 0.0], [0.02, 1.0, 6.0], 0.1)], 0.0);
    assert_eq!(p.len(), 1);
    assert!(!p[0].fits);
    // an L-shaped slab whose notch cuts the rectangle
    let l = Slab { name: "L".into(), top: 3.0, thickness: 0.2, outline: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 4.0], [4.0, 4.0], [4.0, 8.0], [0.0, 8.0]], fire_rating_min: None };
    let (p, _) = find_floor(std::slice::from_ref(&l), &[], &[pipe([3.98, 3.98, 0.0], [3.98, 3.98, 6.0], 0.1)], 0.0);
    assert!(!p[0].fits, "{:?}", p[0]);
    let (p, _) = find_floor(&[l], &[], &[pipe([2.0, 3.0, 0.0], [2.0, 3.0, 6.0], 0.1)], 0.0);
    assert!(p[0].fits);
}

#[test]
fn bad_slabs_are_refused() {
    assert!(slab(3.0, 0.2, None).check().is_ok());
    assert_eq!(slab(3.0, 0.0, None).check(), Err(SlabError::Thickness));
    assert_eq!(slab(f64::NAN, 0.2, None).check(), Err(SlabError::Top));
    let mut s = slab(3.0, 0.2, None);
    s.outline = vec![[0.0, 0.0], [1.0, 1.0], [1.0, 0.0], [0.0, 1.0]]; // bow tie
    assert_eq!(s.check(), Err(SlabError::Outline));
    s.outline = vec![[0.0, 0.0], [1.0, 0.0]];
    assert_eq!(s.check(), Err(SlabError::Outline));
    s = slab(3.0, 0.2, None);
    s.name = String::new();
    assert_eq!(s.check(), Err(SlabError::Name));
}

/// Independent check: sample the cylinder surface and take the box of the points that lie within the slab.
fn brute(a: [f64; 3], b: [f64; 3], r: f64, top: f64, bot: f64) -> [f64; 4] {
    let l = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
    let d = [(b[0] - a[0]) / l, (b[1] - a[1]) / l, (b[2] - a[2]) / l];
    let h = if d[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let mut n1 = [d[1] * h[2] - d[2] * h[1], d[2] * h[0] - d[0] * h[2], d[0] * h[1] - d[1] * h[0]];
    let m = (n1[0] * n1[0] + n1[1] * n1[1] + n1[2] * n1[2]).sqrt();
    n1 = [n1[0] / m, n1[1] / m, n1[2] / m];
    let n2 = [d[1] * n1[2] - d[2] * n1[1], d[2] * n1[0] - d[0] * n1[2], d[0] * n1[1] - d[1] * n1[0]];
    let mut bb = [f64::MAX, f64::MIN, f64::MAX, f64::MIN];
    let steps = 4000;
    for k in 0..=steps {
        let s = l * k as f64 / steps as f64;
        for q in 0..720 {
            let th = q as f64 * std::f64::consts::PI / 360.0;
            let p: Vec<f64> = (0..3).map(|c| a[c] + s * d[c] + r * (th.cos() * n1[c] + th.sin() * n2[c])).collect();
            if p[2] <= top && p[2] >= bot {
                bb = [bb[0].min(p[0]), bb[1].max(p[0]), bb[2].min(p[1]), bb[3].max(p[1])];
            }
        }
    }
    bb
}

#[test]
fn matches_a_brute_force_sampling_of_the_pipe_surface() {
    for (a, b) in [([1.0, 2.0, 0.0], [3.0, 4.0, 6.0]), ([5.0, 3.0, -1.0], [4.0, 3.5, 7.0]), ([2.0, 2.0, 0.0], [6.0, 2.0, 5.0]), ([5.0, 5.0, 0.0], [5.2, 5.0, 8.0])] {
        let (p, _) = find_floor(&[slab(3.0, 0.25, None)], &[], &[pipe(a, b, 0.2)], 0.0);
        let bb = brute(a, b, 0.1, 3.0, 2.75);
        let o = &p[0];
        for (got, want) in [(o.x0, bb[0]), (o.x1, bb[1]), (o.y0, bb[2]), (o.y1, bb[3])] {
            assert!((got - want).abs() < 2e-3, "{got} vs {want} for {a:?}->{b:?}");
        }
    }
}

#[test]
fn register_order_is_by_slab_then_position() {
    let slabs = [slab(3.0, 0.2, None), slab(6.0, 0.2, None)];
    let runs = [pipe([6.0, 1.0, 0.0], [6.0, 1.0, 9.0], 0.1), pipe([2.0, 1.0, 0.0], [2.0, 1.0, 9.0], 0.1)];
    let (p, _) = find_floor(&slabs, &[], &runs, 0.0);
    let ids: Vec<String> = p.iter().map(|o| o.id.to_string()).collect();
    assert_eq!(ids, ["F0-P1", "F0-P2", "F1-P1", "F1-P2"]);
    assert_eq!(p[0].runs, vec![1]);
    assert_eq!(p[1].runs, vec![0]);
}

#[test]
fn merging_does_not_depend_on_the_order_or_the_axis() {
    let mk = |x: f64, y: f64| pipe([x, y, 0.0], [x, y, 6.0], 0.1);
    let s = [slab(3.0, 0.2, None)];
    // 0.15 clear between the two; merge gap 0.2 joins them, 0.1 does not, in both orders and along both axes
    for (a, b) in [((1.0, 1.0), (1.25, 1.0)), ((1.25, 1.0), (1.0, 1.0)), ((1.0, 1.0), (1.0, 1.25)), ((1.0, 1.25), (1.0, 1.0))] {
        assert_eq!(find_floor(&s, &[], &[mk(a.0, a.1), mk(b.0, b.1)], 0.2).0.len(), 1, "{a:?} {b:?}");
        assert_eq!(find_floor(&s, &[], &[mk(a.0, a.1), mk(b.0, b.1)], 0.1).0.len(), 2, "{a:?} {b:?}");
    }
}

#[test]
fn margin_applies_to_both_sides_of_a_rectangular_run() {
    let r = Run::rect("d", Domain::Duct, System::SupplyAir, [4.0, 4.0, 0.0], [4.0, 4.0, 6.0], 0.6, 0.3).with_margin(0.05);
    let (p, _) = find_floor(&[slab(3.0, 0.2, None)], &[], &[r], 0.0);
    let (sx, sy) = (p[0].size_x(), p[0].size_y());
    assert!((close(sx, 0.7) && close(sy, 0.4)) || (close(sx, 0.4) && close(sy, 0.7)), "{:?}", p[0]);
}

#[test]
fn the_floor_openings_export_as_safe_csv() {
    let evil = Run { label: "=SUM(A1),\"x\"\n+cmd".into(), ..pipe([1.0, 1.0, 0.0], [1.0, 1.0, 6.0], 0.1) };
    let runs = vec![evil, pipe([5.0, 5.0, 0.0], [5.0, 5.0, 6.0], 0.1)];
    let (p, _) = find_floor(&[slab(3.0, 0.2, Some(60))], &[], &runs, 0.0);
    let csv = floor_register_csv(&p, &runs);
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 3, "{csv}");
    assert!(lines[0].starts_with("id,slab,x0_m,x1_m"));
    assert!(lines[1].starts_with("F0-P1,0,0.9500,1.0500,0.9500,1.0500,0.1000,0.1000,1.0000,1.0000,2.9000,60,true,cut,true,"), "{}", lines[1]);
    // exactly the 17 columns, whatever the labels hold; a label never begins like a formula
    for l in &lines {
        assert_eq!(l.split(',').count(), 17, "{l}");
    }
    assert!(lines[1].contains(", =SUM(A1)  x  +cmd"), "{}", lines[1]);
    // an opening inside a shaft says so
    let shaft = ShaftVoid { name: "S".into(), outline: vec![[4.5, 4.5], [5.5, 4.5], [5.5, 5.5], [4.5, 5.5]], z_bottom: 0.0, z_top: 6.0 };
    let (p, _) = find_floor(&[slab(3.0, 0.2, None)], &[shaft], &runs[1..], 0.0);
    assert!(floor_register_csv(&p, &runs[1..]).lines().nth(1).unwrap().contains(",shaft 0,"));
    assert_eq!(floor_register_csv(&[], &[]).lines().count(), 1);
}
