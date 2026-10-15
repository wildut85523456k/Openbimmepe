use openbim_family::library;
use openbim_family::mep::{Domain, System};
use openbim_geom::*;
use openbim_place::penetrations::*;
use openbim_place::*;

fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())) }
fn wall(len: f64, t: f64, h: f64) -> WallChain { WallChain::new(vec![[0.0, 0.0], [len, 0.0]], false, t, h) }
fn room(w: f64, d: f64, t: f64, h: f64) -> WallChain { WallChain::new(vec![[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]], true, t, h) }
fn pipe(a: [f64; 3], b: [f64; 3], d: f64) -> Run { Run::round("pipe", Domain::Pipe, System::ChilledWaterSupply, a, b, d) }
fn duct(a: [f64; 3], b: [f64; 3], w: f64, h: f64) -> Run { Run::rect("duct", Domain::Duct, System::SupplyAir, a, b, w, h) }
fn model(chains: Vec<WallChain>, runs: Vec<Run>) -> Model { Model { chains, runs, ..Default::default() } }
fn pens(chains: Vec<WallChain>, runs: Vec<Run>) -> Vec<Penetration> { model(chains, runs).build().unwrap().penetrations }

#[test]
fn a_square_pipe_through_a_wall_gets_its_own_size_plus_margin() {
    let p = pens(vec![wall(6.0, 0.2, 3.0)], vec![pipe([3.0, -1.0, 1.5], [3.0, 1.0, 1.5], 0.1)]);
    assert_eq!(p.len(), 1);
    let o = &p[0];
    assert!(close(o.offset, 2.95) && close(o.width, 0.1) && close(o.sill, 1.45) && close(o.height, 0.1));
    assert_eq!(o.id, PenetrationId { chain: 0, segment: 0, seq: 0 });
    assert_eq!(o.id.to_string(), "W0-S0-P1");
    assert!(close(o.centre[0], 3.0) && close(o.centre[1], 0.0) && close(o.centre[2], 1.5));
    assert_eq!(o.runs, vec![0]);
    let p = pens(vec![wall(6.0, 0.2, 3.0)], vec![pipe([3.0, -1.0, 1.5], [3.0, 1.0, 1.5], 0.1).with_margin(0.025)]);
    assert!(close(p[0].width, 0.15) && close(p[0].height, 0.15) && close(p[0].offset, 2.925) && close(p[0].sill, 1.425));
}

#[test]
fn a_skewed_pipe_needs_a_wider_opening_by_hand_calculation() {
    // 45 degrees in plan: the faces are 0.2 apart so the centre moves 0.2 along the wall, and the cut is an ellipse
    // with a long half-axis r / cos 45 = r sqrt 2
    let p = pens(vec![wall(6.0, 0.2, 3.0)], vec![pipe([2.0, -1.0, 1.5], [4.0, 1.0, 1.5], 0.1)]);
    assert!(close(p[0].width, 0.2 + 2.0 * 0.05 * 2f64.sqrt()), "{}", p[0].width);
    assert!(close(p[0].height, 0.1));
    assert!(close(p[0].centre[0], 3.0));
    // rising 45 degrees across the wall: height grows the same way
    let p = pens(vec![wall(6.0, 0.2, 3.0)], vec![pipe([3.0, -1.0, 0.5], [3.0, 1.0, 2.5], 0.1)]);
    assert!(close(p[0].height, 0.2 + 2.0 * 0.05 * 2f64.sqrt()), "{}", p[0].height);
    assert!(close(p[0].width, 0.1));
}

#[test]
fn rectangular_ducts_follow_width_height_and_roll() {
    let a = [3.0, -1.0, 2.0];
    let b = [3.0, 1.0, 2.0];
    let p = pens(vec![wall(6.0, 0.2, 3.0)], vec![duct(a, b, 0.4, 0.2)]);
    assert!(close(p[0].width, 0.4) && close(p[0].height, 0.2) && close(p[0].offset, 2.8) && close(p[0].sill, 1.9));
    let mut r = duct(a, b, 0.4, 0.2);
    r.size = RunSize::Rect { width: 0.4, height: 0.2, roll: 90.0 };
    let p = pens(vec![wall(6.0, 0.2, 3.0)], vec![r]);
    assert!(close(p[0].width, 0.2) && close(p[0].height, 0.4) && close(p[0].offset, 2.9) && close(p[0].sill, 1.8));
}

/// Independent: the opening of a round or rectangular run, found by walking the rim of its section on both
/// faces of the wall in world coordinates (no wall frame, no closed form).
fn brute(a: [f64; 3], b: [f64; 3], size: RunSize, margin: f64, wall: &WallChain) -> [f64; 4] {
    let d = { let v = [b[0] - a[0], b[1] - a[1], b[2] - a[2]]; let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); [v[0] / n, v[1] / n, v[2] / n] };
    let (p0, p1) = (wall.points[0], wall.points[1]);
    let l = ((p1[0] - p0[0]).powi(2) + (p1[1] - p0[1]).powi(2)).sqrt();
    let u = [(p1[0] - p0[0]) / l, (p1[1] - p0[1]) / l];
    let nrm = [-u[1], u[0]];
    let cr = |x: [f64; 3], y: [f64; 3]| [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
    let nn = |x: [f64; 3]| { let n = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt(); [x[0] / n, x[1] / n, x[2] / n] };
    // two axes square to the run: for a duct up is world Z (or Y when vertical), turned by roll
    let (e1, e2, corners): ([f64; 3], [f64; 3], Vec<(f64, f64)>) = match size {
        RunSize::Round { diameter } => {
            let r = diameter / 2.0 + margin;
            let e1 = nn(cr(d, if d[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] }));
            let e2 = cr(d, e1);
            (e1, e2, (0..20000).map(|k| { let t = k as f64 / 20000.0 * std::f64::consts::TAU; (r * t.cos(), r * t.sin()) }).collect())
        }
        RunSize::Rect { width, height, roll } => {
            let base = if d[0].abs() < 1e-9 && d[1].abs() < 1e-9 { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
            let k = base[0] * d[0] + base[1] * d[1] + base[2] * d[2];
            let up0 = nn([base[0] - k * d[0], base[1] - k * d[1], base[2] - k * d[2]]);
            let w0 = cr(d, up0);
            let (s, c) = roll.to_radians().sin_cos();
            let up = nn([c * up0[0] + s * w0[0], c * up0[1] + s * w0[1], c * up0[2] + s * w0[2]]);
            let wd = nn(cr(up, d));
            let (hw, hh) = (width / 2.0 + margin, height / 2.0 + margin);
            (wd, up, vec![(hw, hh), (hw, -hh), (-hw, hh), (-hw, -hh)])
        }
    };
    let (mut u0, mut u1, mut z0, mut z1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for face in [-wall.thickness / 2.0, wall.thickness / 2.0] {
        for &(x, y) in &corners {
            // a point of the section on the face: start at the run line, offset in the section, then slide along d to the face
            let q = [a[0] + x * e1[0] + y * e2[0], a[1] + x * e1[1] + y * e2[1], a[2] + x * e1[2] + y * e2[2]];
            let v0 = (q[0] - p0[0]) * nrm[0] + (q[1] - p0[1]) * nrm[1];
            let dv = d[0] * nrm[0] + d[1] * nrm[1];
            let s = (face - v0) / dv;
            let w = [q[0] + s * d[0], q[1] + s * d[1], q[2] + s * d[2]];
            let uu = (w[0] - p0[0]) * u[0] + (w[1] - p0[1]) * u[1];
            u0 = u0.min(uu); u1 = u1.max(uu);
            z0 = z0.min(w[2] - wall.base_z); z1 = z1.max(w[2] - wall.base_z);
        }
    }
    [u0, u1 - u0, z0, z1 - z0]
}

#[test]
fn the_opening_matches_an_independent_rim_walk_for_many_runs() {
    // a wall on a diagonal, and a spread of directions, sizes, margins and rolls (fixed generator, no randomness)
    let w = WallChain { base_z: 0.5, ..WallChain::new(vec![[1.0, 2.0], [7.0, 6.0]], false, 0.3, 3.5) };
    let (p0, p1) = (w.points[0], w.points[1]);
    let l = ((p1[0] - p0[0]).powi(2) + (p1[1] - p0[1]).powi(2)).sqrt();
    let u = [(p1[0] - p0[0]) / l, (p1[1] - p0[1]) / l];
    let nrm = [-u[1], u[0]];
    let mut seed = 12345u64;
    let mut rnd = move || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); ((seed >> 33) as f64) / ((1u64 << 31) as f64) };
    let mut checked = 0;
    for k in 0..60 {
        // a point in the middle of the wall and a direction at most 70 degrees from the wall normal
        let t = 0.3 + 0.4 * rnd();
        let mid = [p0[0] + u[0] * l * t, p0[1] + u[1] * l * t, 0.5 + 1.2 + 1.5 * rnd()];
        let ang = (rnd() - 0.5) * 2.0 * 70f64.to_radians();
        let el = (rnd() - 0.5) * 2.0 * 40f64.to_radians();
        let dir = [(ang.cos() * el.cos()) * nrm[0] + ang.sin() * el.cos() * u[0], (ang.cos() * el.cos()) * nrm[1] + ang.sin() * el.cos() * u[1], el.sin()];
        let a = [mid[0] - dir[0] * 1.5, mid[1] - dir[1] * 1.5, mid[2] - dir[2] * 1.5];
        let b = [mid[0] + dir[0] * 1.5, mid[1] + dir[1] * 1.5, mid[2] + dir[2] * 1.5];
        let margin = 0.02 * rnd();
        let size = if k % 2 == 0 { RunSize::Round { diameter: 0.05 + 0.2 * rnd() } } else { RunSize::Rect { width: 0.1 + 0.4 * rnd(), height: 0.1 + 0.3 * rnd(), roll: [0.0, 30.0, 90.0, 180.0, 225.0][k % 5] } };
        let run = Run { label: "r".into(), domain: Domain::Duct, system: System::SupplyAir, a, b, size, margin };
        let (found, issues) = find(std::slice::from_ref(&w), &[run], 0.0);
        assert!(issues.is_empty(), "{k}: {issues:?}");
        assert_eq!(found.len(), 1, "{k}");
        let want = brute(a, b, size, margin, &w);
        let got = [found[0].offset, found[0].width, found[0].sill, found[0].height];
        for i in 0..4 { assert!((want[i] - got[i]).abs() < 2e-5, "run {k} item {i}: want {want:?} got {got:?}"); }
        checked += 1;
    }
    assert_eq!(checked, 60);
}

#[test]
fn close_pipes_share_one_rectangular_opening() {
    let three = |gap: f64| vec![
        Run::round("chws", Domain::Pipe, System::ChilledWaterSupply, [1.0, -1.0, 1.5], [1.0, 1.0, 1.5], 0.1),
        Run::round("chwr", Domain::Pipe, System::ChilledWaterReturn, [1.0 + 0.1 + gap, -1.0, 1.5], [1.0 + 0.1 + gap, 1.0, 1.5], 0.1),
        Run::round("far", Domain::Pipe, System::DomesticCold, [4.0, -1.0, 1.5], [4.0, 1.0, 1.5], 0.05),
    ];
    let mut m = model(vec![wall(6.0, 0.2, 3.0)], three(0.05));
    m.merge_gap = 0.0;
    // without a merge gap the pipes stay apart
    let p = m.build().unwrap().penetrations;
    assert_eq!(p.len(), 3);
    assert!(p.iter().all(|x| x.runs.len() == 1));
    // with a gap of 0.06 the near two become one rectangle that spans both
    m.merge_gap = 0.06;
    let p = m.build().unwrap().penetrations;
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].runs, vec![0, 1]);
    assert!(close(p[0].offset, 0.95) && close(p[0].width, 0.25) && close(p[0].sill, 1.45) && close(p[0].height, 0.1));
    assert_eq!(p[0].systems, vec![System::ChilledWaterSupply, System::ChilledWaterReturn]);
    assert_eq!(p[0].domains, vec![Domain::Pipe]);
    assert_eq!(p[1].runs, vec![2]);
    assert_eq!(p[1].id.seq, 1);
    // a gap exactly as large as the clear space merges too; one hair smaller does not
    m.merge_gap = 0.05;
    assert_eq!(m.build().unwrap().penetrations.len(), 2);
    m.merge_gap = 0.0499;
    assert_eq!(m.build().unwrap().penetrations.len(), 3);
    // the same holds when the right-hand pipe is listed first, and for the vertical gap
    m.runs.swap(0, 1);
    m.merge_gap = 0.05;
    assert_eq!(m.build().unwrap().penetrations.len(), 2);
    m.merge_gap = 0.0499;
    assert_eq!(m.build().unwrap().penetrations.len(), 3);
    let stack = |first_low: bool| {
        let (lo, hi) = (pipe([1.0, -1.0, 1.0], [1.0, 1.0, 1.0], 0.1), pipe([1.0, -1.0, 1.2], [1.0, 1.0, 1.2], 0.1));
        if first_low { vec![lo, hi] } else { vec![hi, lo] }
    };
    for order in [true, false] {
        let mut v = model(vec![wall(6.0, 0.2, 3.0)], stack(order));
        v.merge_gap = 0.1;
        assert_eq!(v.build().unwrap().penetrations.len(), 1, "{order}");
        v.merge_gap = 0.0999;
        assert_eq!(v.build().unwrap().penetrations.len(), 2, "{order}");
    }
    // a chain of three, each close to the next, becomes one opening
    m.runs = vec![
        pipe([1.0, -1.0, 1.5], [1.0, 1.0, 1.5], 0.1),
        pipe([1.2, -1.0, 1.5], [1.2, 1.0, 1.5], 0.1),
        pipe([1.4, -1.0, 1.5], [1.4, 1.0, 1.5], 0.1),
    ];
    m.merge_gap = 0.11;
    let p = m.build().unwrap().penetrations;
    assert_eq!(p.len(), 1);
    assert!(close(p[0].width, 0.5) && p[0].runs == vec![0, 1, 2]);
    // a bundle that is stacked up and down merges in both directions
    m.runs = vec![pipe([1.0, -1.0, 1.0], [1.0, 1.0, 1.0], 0.1), pipe([1.0, -1.0, 1.2], [1.0, 1.0, 1.2], 0.1)];
    m.merge_gap = 0.11;
    let p = m.build().unwrap().penetrations;
    assert_eq!(p.len(), 1);
    assert!(close(p[0].height, 0.3) && close(p[0].width, 0.1));
}

#[test]
fn the_wall_is_cut_by_exactly_the_register_opening() {
    let base = model(vec![wall(6.0, 0.2, 3.0)], vec![]).build().unwrap().volume;
    let m = model(vec![wall(6.0, 0.2, 3.0)], vec![duct([2.0, -1.0, 2.0], [2.0, 1.0, 2.0], 0.5, 0.3), pipe([4.0, -1.0, 1.0], [4.0, 1.0, 1.0], 0.2)]);
    let b = m.build().unwrap();
    assert_eq!(b.penetrations.len(), 2);
    let cut: f64 = b.penetrations.iter().map(|p| p.width * p.height * 0.2).sum();
    assert!(close(base - b.volume, cut), "{} vs {}", base - b.volume, cut);
    // the lenient build gives the same
    let l = m.build_lenient().unwrap();
    assert!(l.warnings.is_empty() && close(l.volume, b.volume) && l.penetrations == b.penetrations);
}

#[test]
fn fire_partitions_are_listed() {
    let mut fire = wall(6.0, 0.2, 3.0);
    fire.fire_rating_min = Some(60);
    let plain = wall(6.0, 0.2, 3.0);
    let mut zero = wall(6.0, 0.2, 3.0);
    zero.fire_rating_min = Some(0);
    let p = pens(vec![plain, fire, zero], vec![pipe([3.0, -1.0, 1.5], [3.0, 1.0, 1.5], 0.1)]);
    assert_eq!(p.len(), 3);
    assert!(!p[0].in_fire_partition() && p[1].in_fire_partition() && !p[2].in_fire_partition());
    assert_eq!(p[1].fire_rating_min, Some(60));
    let listed: Vec<_> = p.iter().filter(|x| x.in_fire_partition()).map(|x| x.id).collect();
    assert_eq!(listed, vec![PenetrationId { chain: 1, segment: 0, seq: 0 }]);
    assert!(close(p[1].area(), std::f64::consts::FRAC_PI_4 * 0.01));
}

#[test]
fn the_register_exports_as_safe_csv() {
    let mut fire = wall(6.0, 0.2, 3.0);
    fire.fire_rating_min = Some(90);
    let runs = vec![pipe([1.0, -1.0, 1.5], [1.0, 1.0, 1.5], 0.1), Run { label: "=SUM(A1),\"x\"\n+cmd".into(), ..pipe([1.1, -1.0, 1.5], [1.1, 1.0, 1.5], 0.1) }];
    let m = Model { merge_gap: 0.5, ..model(vec![fire], runs.clone()) };
    let b = m.build().unwrap();
    let csv = register_csv(&b.penetrations, &runs, &Default::default());
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 2, "{csv}");
    assert!(lines[0].starts_with("id,wall,segment,offset_m"));
    assert!(lines[1].starts_with("W0-S0-P1,0,0,0.9500,0.2000,1.4500,0.1000"), "{}", lines[1]);
    assert!(lines[1].contains(",90,true,true,"));
    // exactly the 15 columns, whatever the labels hold; a label never begins like a formula
    assert_eq!(lines[1].split(',').count(), 19);
    assert!(lines[1].contains("pipe |  =SUM(A1)  x  +cmd"), "{}", lines[1]);
    // a label that starts like a formula gets a space in front
    let first = register_csv(&b.penetrations, &[runs[1].clone(), runs[1].clone()], &Default::default());
    assert!(first.lines().nth(1).unwrap().contains(", =SUM(A1)"), "{first}");
}

#[test]
fn runs_that_miss_touch_or_cross_several_walls() {
    let w = wall(6.0, 0.2, 3.0);
    // beside the wall, above it, below it, only touching a face, stopping short
    for (a, b) in [([7.0, -1.0, 1.5], [7.0, 1.0, 1.5]), ([3.0, -1.0, 3.5], [3.0, 1.0, 3.5]), ([3.0, -1.0, -0.5], [3.0, 1.0, -0.5]), ([3.0, -1.0, 1.5], [3.0, -0.1, 1.5]), ([3.0, -1.0, 1.5], [3.0, -0.5, 1.5])] {
        let m = model(vec![w.clone()], vec![pipe(a, b, 0.1)]);
        assert!(m.build().unwrap().penetrations.is_empty(), "{a:?}");
    }
    // a run through a room passes two walls
    let p = pens(vec![room(6.0, 4.0, 0.2, 3.0)], vec![pipe([3.0, -1.0, 1.5], [3.0, 5.0, 1.5], 0.1)]);
    assert_eq!(p.len(), 2);
    assert_eq!((p[0].id.segment, p[1].id.segment), (0, 2));
    assert!(close(p[0].centre[1], 0.0) && close(p[1].centre[1], 4.0));
    // a vertical riser beside a wall does not touch it
    let m = model(vec![wall(6.0, 0.2, 3.0)], vec![pipe([3.0, 0.5, 0.0], [3.0, 0.5, 3.0], 0.1)]);
    assert!(m.build().unwrap().penetrations.is_empty());
}

#[test]
fn bad_runs_are_errors_in_the_strict_build_and_warnings_in_the_lenient_one() {
    let w = || wall(6.0, 0.2, 3.0);
    let good = pipe([3.0, -1.0, 1.5], [3.0, 1.0, 1.5], 0.1);
    let cases: Vec<(Run, RunProblem)> = vec![
        (pipe([1.0, 0.0, 1.5], [4.0, 0.0, 1.5], 0.1), RunProblem::InsideWall),
        (pipe([2.0, 0.05, 1.5], [2.0, 0.05, 2.5], 0.1), RunProblem::InsideWall),
        (pipe([5.0, -1.0, 1.5], [5.0, 0.05, 1.5], 0.1), RunProblem::EndsInWall),
        (pipe([5.0, -0.05, 1.5], [5.0, 1.0, 1.5], 0.1), RunProblem::EndsInWall),
        (pipe([f64::NAN, -1.0, 1.5], [3.0, 1.0, 1.5], 0.1), RunProblem::BadData),
        (pipe([3.0, 0.0, 1.5], [3.0, 0.0, 1.5], 0.1), RunProblem::BadData),
        (pipe([3.0, -1.0, 1.5], [3.0, 1.0, 1.5], 0.0), RunProblem::BadData),
        (pipe([3.0, -1.0, 1.5], [3.0, 1.0, 1.5], -0.1), RunProblem::BadData),
        (pipe([3.0, -1.0, 1.5], [3.0, 1.0, 1.5], 0.1).with_margin(-0.01), RunProblem::BadData),
        (pipe([3.0, -1.0, 1.5], [3.0, 1.0, 1.5], 0.1).with_margin(f64::INFINITY), RunProblem::BadData),
        (duct([3.0, -1.0, 1.5], [3.0, 1.0, 1.5], 0.0, 0.2), RunProblem::BadData),
    ];
    for (n, (bad, why)) in cases.into_iter().enumerate() {
        let m = model(vec![w()], vec![good.clone(), bad]);
        assert_eq!(m.build().err(), Some(PlaceError::Run { run: 1, problem: why }), "case {n}");
        let l = m.build_lenient().unwrap();
        assert_eq!(l.warnings.len(), 1, "case {n}");
        assert_eq!(l.warnings[0].run, Some(1));
        assert_eq!(l.warnings[0].error, PlaceError::Run { run: 1, problem: why });
        assert_eq!(l.penetrations.len(), 1, "the good run is still recorded");
        assert!(l.penetrations[0].cut);
    }
}

fn door_model(runs: Vec<Run>) -> Model {
    let mut fams = library::all().unwrap();
    fams.remove(0);
    fams.truncate(1);
    let p = Placement { family: 0, type_name: None, overrides: vec![], texts: vec![], chain: 0, segment: 0, centre: 3.0 };
    Model { chains: vec![wall(6.0, 0.2, 3.0)], families: fams, placements: vec![p], runs, merge_gap: 0.0 }
}

#[test]
fn a_penetration_that_clashes_with_a_door_is_named_and_not_cut() {
    // through the middle of the door
    let m = door_model(vec![pipe([3.0, -1.0, 1.0], [3.0, 1.0, 1.0], 0.1), pipe([1.0, -1.0, 1.5], [1.0, 1.0, 1.5], 0.1)]);
    match m.build() {
        Err(PlaceError::Penetration { id, .. }) => assert_eq!(id, PenetrationId { chain: 0, segment: 0, seq: 1 }),
        o => panic!("{:?}", o.map(|b| b.volume)),
    }
    let l = m.build_lenient().unwrap();
    assert_eq!(l.penetrations.len(), 2);
    assert!(!l.penetrations[1].cut && l.penetrations[0].cut);
    assert!(l.warnings.iter().any(|w| matches!(w.error, PlaceError::Penetration { .. }) && w.run == Some(0)));
    // the door is still in and the wall has door plus one opening
    assert!(l.instances[0].is_some());
    assert_eq!(l.chains[0].openings[0].len(), 2);
    // no clash: same model with the first pipe moved off the door
    let ok = door_model(vec![pipe([1.0, -1.0, 1.5], [1.0, 1.0, 1.5], 0.1)]);
    assert_eq!(ok.build().unwrap().penetrations.len(), 1);
}

#[test]
fn a_penetration_that_runs_off_the_wall_end_is_not_cut() {
    // centre inside the wall but the opening reaches beyond the mitred end
    let m = model(vec![wall(6.0, 0.2, 3.0)], vec![duct([0.05, -1.0, 1.5], [0.05, 1.0, 1.5], 0.4, 0.2)]);
    assert!(matches!(m.build(), Err(PlaceError::Penetration { .. })));
    let l = m.build_lenient().unwrap();
    assert_eq!(l.penetrations.len(), 1);
    assert!(!l.penetrations[0].cut);
    assert_eq!(l.warnings.len(), 1);
    let base = model(vec![wall(6.0, 0.2, 3.0)], vec![]).build().unwrap().volume;
    assert_eq!(l.volume, base);
}

#[test]
fn registers_do_not_depend_on_the_order_of_the_runs() {
    let runs = vec![
        pipe([1.0, -1.0, 1.5], [1.0, 1.0, 1.5], 0.1),
        pipe([2.0, -1.0, 2.0], [2.0, 1.0, 2.0], 0.1),
        duct([4.0, -1.0, 1.5], [4.0, 1.0, 1.5], 0.4, 0.3),
        pipe([4.2, -1.0, 1.5], [4.2, 1.0, 1.5], 0.1),
    ];
    let mut rev = runs.clone();
    rev.reverse();
    let a = Model { merge_gap: 0.1, ..model(vec![room(6.0, 4.0, 0.2, 3.0)], runs) }.build().unwrap();
    let b = Model { merge_gap: 0.1, ..model(vec![room(6.0, 4.0, 0.2, 3.0)], rev) }.build().unwrap();
    assert_eq!(a.volume, b.volume);
    assert_eq!(a.penetrations.len(), b.penetrations.len());
    for (x, y) in a.penetrations.iter().zip(&b.penetrations) {
        assert_eq!((x.id, x.offset, x.width, x.sill, x.height), (y.id, y.offset, y.width, y.sill, y.height));
    }
}

#[test]
fn one_pipe_gets_a_round_opening_and_a_group_a_rectangle() {
    use openbim_place::penetrations::{OpeningShape, DEFAULT_GROUP_RADIUS};
    assert!(close(DEFAULT_GROUP_RADIUS, 0.40) && close(Model::default().merge_gap, 0.40));
    // a single round pipe square on: round, diameter 0.1 + 2 x margin
    let one = Run::round("p", Domain::Pipe, System::ChilledWaterSupply, [3.0, -1.0, 1.5], [3.0, 1.0, 1.5], 0.1).with_margin(0.01);
    let p = pens(vec![wall(6.0, 0.2, 3.0)], vec![one.clone()]);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].shape, OpeningShape::Round);
    assert!(close(p[0].width, 0.12) && close(p[0].height, 0.12) && close(p[0].area(), std::f64::consts::FRAC_PI_4 * 0.12 * 0.12));
    // two pipes 0.3 m apart (within the default radius) share one rectangle
    let two = Run::round("q", Domain::Pipe, System::ChilledWaterReturn, [3.3, -1.0, 1.5], [3.3, 1.0, 1.5], 0.1);
    let p = pens(vec![wall(6.0, 0.2, 3.0)], vec![one.clone(), two.clone()]);
    assert_eq!((p.len(), p[0].shape, p[0].runs.len()), (1, OpeningShape::Rect, 2));
    // the radius is a parameter: with 0.1 they stay apart and each is round
    let m = Model { merge_gap: 0.1, ..model(vec![wall(6.0, 0.2, 3.0)], vec![one.clone(), two]) };
    let p = m.build().unwrap().penetrations;
    assert!(p.len() == 2 && p.iter().all(|x| x.shape == OpeningShape::Round));
    // a skewed crossing is longer than it is high: rectangle
    let skew = Run::round("s", Domain::Pipe, System::ChilledWaterSupply, [2.0, -1.0, 1.5], [4.0, 1.0, 1.5], 0.1);
    assert_eq!(pens(vec![wall(6.0, 0.2, 3.0)], vec![skew])[0].shape, OpeningShape::Rect);
    // a rectangular run is a rectangle
    let duct = Run::rect("d", Domain::Duct, System::SupplyAir, [3.0, -1.0, 1.5], [3.0, 1.0, 1.5], 0.3, 0.2);
    assert_eq!(pens(vec![wall(6.0, 0.2, 3.0)], vec![duct])[0].shape, OpeningShape::Rect);
}

#[test]
fn firestop_fields_sit_beside_the_wall_rating() {
    use openbim_place::penetrations::{Firestop, Firestops};
    let mut fire = wall(6.0, 0.2, 3.0);
    fire.fire_rating_min = Some(120);
    let runs = vec![pipe([1.0, -1.0, 1.5], [1.0, 1.0, 1.5], 0.1), pipe([4.0, -1.0, 1.5], [4.0, 1.0, 1.5], 0.1)];
    let b = model(vec![fire], runs.clone()).build().unwrap();
    assert_eq!(b.penetrations.len(), 2);
    let mut f = Firestops::new();
    f.insert(b.penetrations[0].id, Firestop { product: Some("Collar, \"A\"".into()), installed: Some("=2026-10-01".into()), inspected: None });
    let csv = register_csv(&b.penetrations, &runs, &f);
    let rows: Vec<&str> = csv.lines().collect();
    // the opening rating is the wall's (one column, 120), the record is only what was entered; no cell breaks the row or reads as a formula
    assert!(rows[1].contains(",120,true,true,") && rows[1].ends_with(",rect,Collar   A , =2026-10-01,") || rows[1].ends_with(",round,Collar   A , =2026-10-01,"), "{}", rows[1]);
    assert!(rows.iter().all(|r| r.split(',').count() == 19));
    assert!(rows[2].ends_with(",,,"), "{}", rows[2]);
}
