//! Prints sketches (drawing, constraints) with the solver's answer for the independent check in spikes/s11_sketch_check.
use openbim_sketch::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); ((self.0 >> 11) as f64) / ((1u64 << 53) as f64) }
    fn range(&mut self, a: f64, b: f64) -> f64 { a + (b - a) * self.next() }
    fn idx(&mut self, n: usize) -> usize { ((self.next() * n as f64) as usize).min(n - 1) }
}

fn show(id: &str, s: &Sketch) {
    println!("SKETCH {id}");
    for p in &s.points { println!("P {:.17e} {:.17e}", p[0], p[1]); }
    for l in &s.lines { println!("L {} {}", l.0, l.1); }
    for c in &s.circles { println!("C {} {:.17e}", c.centre, c.radius); }
    use Constraint::*;
    for c in &s.constraints {
        match *c {
            Fixed { p, x, y } => println!("K fixed {p} {x:.17e} {y:.17e}"),
            Coincident { a, b } => println!("K coincident {a} {b}"),
            Horizontal { a, b } => println!("K horizontal {a} {b}"),
            Vertical { a, b } => println!("K vertical {a} {b}"),
            Parallel { l1, l2 } => println!("K parallel {l1} {l2}"),
            Perpendicular { l1, l2 } => println!("K perpendicular {l1} {l2}"),
            EqualLength { l1, l2 } => println!("K equallength {l1} {l2}"),
            Distance { a, b, d } => println!("K distance {a} {b} {d:.17e}"),
            DistanceX { a, b, d } => println!("K distancex {a} {b} {d:.17e}"),
            DistanceY { a, b, d } => println!("K distancey {a} {b} {d:.17e}"),
            PointOnLine { p, l } => println!("K pointonline {p} {l}"),
            Midpoint { p, l } => println!("K midpoint {p} {l}"),
            Angle { l1, l2, degrees } => println!("K angle {l1} {l2} {degrees:.17e}"),
            Radius { c, r } => println!("K radius {c} {r:.17e}"),
            EqualRadius { c1, c2 } => println!("K equalradius {c1} {c2}"),
            PointOnCircle { p, c } => println!("K pointoncircle {p} {c}"),
            Tangent { l, c } => println!("K tangent {l} {c}"),
        }
    }
    let sol = s.solve().unwrap();
    let st = match sol.status { Status::Solved => "solved".to_string(), Status::Unsolved { .. } => "unsolved".to_string() };
    println!("RESULT {st} {} {}", sol.dof, sol.redundant);
    for p in &sol.points { println!("S {:.17e} {:.17e}", p[0], p[1]); }
    for r in &sol.radii { println!("R {r:.17e}"); }
    println!("FIXED {}", sol.fixed.iter().map(|b| if *b { "1" } else { "0" }).collect::<Vec<_>>().join(" "));
    println!("END");
}

fn rectangle(w: f64, h: f64) -> Sketch {
    let mut s = Sketch::default();
    for q in [(0.1, -0.1), (3.8, 0.2), (4.1, 3.1), (-0.2, 2.9)] { s.point(q.0, q.1); }
    for k in 0..4 { s.line(k, (k + 1) % 4); }
    s.constrain(Constraint::Fixed { p: 0, x: 0.0, y: 0.0 }).constrain(Constraint::Horizontal { a: 0, b: 1 }).constrain(Constraint::Vertical { a: 1, b: 2 })
        .constrain(Constraint::Horizontal { a: 2, b: 3 }).constrain(Constraint::Vertical { a: 3, b: 0 })
        .constrain(Constraint::Distance { a: 0, b: 1, d: w }).constrain(Constraint::Distance { a: 1, b: 2, d: h });
    s
}

fn main() {
    show("rect", &rectangle(4.0, 3.0));
    let mut s = rectangle(4.0, 3.0);
    s.constraints.pop();
    show("rect-open", &s);
    let mut s = rectangle(4.0, 3.0);
    s.constrain(Constraint::Parallel { l1: 0, l2: 2 }).constrain(Constraint::Horizontal { a: 0, b: 1 });
    show("rect-redundant", &s);
    let mut s = rectangle(4.0, 3.0);
    s.constrain(Constraint::Distance { a: 0, b: 1, d: 5.0 });
    show("rect-conflict", &s);
    let mut s = Sketch::default();
    s.point(0.0, 0.0); s.point(3.0, 0.0);
    s.constrain(Constraint::Distance { a: 0, b: 1, d: 5.0 });
    show("stretch", &s);
    // random sketches built from a true drawing, solved from a nearby one
    let mut rng = Rng(12345);
    for k in 0..40 {
        let n = 5 + rng.idx(5);
        let truth: Vec<[f64; 2]> = (0..n).map(|_| [rng.range(-10.0, 10.0), rng.range(-10.0, 10.0)]).collect();
        let mut s = Sketch::default();
        for p in &truth { s.point(p[0] + rng.range(-0.05, 0.05), p[1] + rng.range(-0.05, 0.05)); }
        for i in 0..n { s.line(i, (i + 1) % n); }
        let rad = rng.range(0.5, 3.0);
        s.circle(0, rad * 1.01);
        s.circle(1, rad * 1.3 * 0.99);
        let tr = [rad, rad * 1.3];
        let dist = |a: usize, b: usize| ((truth[a][0] - truth[b][0]).powi(2) + (truth[a][1] - truth[b][1]).powi(2)).sqrt();
        let heading = |l: usize| { let (a, b) = (l, (l + 1) % n); (truth[b][1] - truth[a][1]).atan2(truth[b][0] - truth[a][0]) };
        for _ in 0..(3 + rng.idx(10)) {
            let (a, b) = (rng.idx(n), rng.idx(n));
            let (l1, l2) = (rng.idx(n), rng.idx(n));
            use Constraint::*;
            match rng.idx(14) {
                0 => { s.constrain(Fixed { p: a, x: truth[a][0], y: truth[a][1] }); }
                1 if a != b => { s.constrain(Distance { a, b, d: dist(a, b) }); }
                2 => { s.constrain(DistanceX { a, b, d: truth[b][0] - truth[a][0] }); }
                3 => { s.constrain(DistanceY { a, b, d: truth[b][1] - truth[a][1] }); }
                4 if l1 != l2 => { s.constrain(Angle { l1, l2, degrees: (heading(l2) - heading(l1)).to_degrees() }); }
                5 if l1 != l2 => {
                    let len = |l: usize| dist(l, (l + 1) % n);
                    let _ = len;
                    s.constrain(Distance { a: l1, b: (l1 + 1) % n, d: dist(l1, (l1 + 1) % n) });
                }
                6 => { s.constrain(Radius { c: a % 2, r: tr[a % 2] }); }
                7 => { s.constrain(Fixed { p: b, x: truth[b][0], y: truth[b][1] }); }
                8 if a != b => { s.constrain(Distance { a, b, d: dist(a, b) }); }
                9 => { s.constrain(DistanceX { a, b, d: truth[b][0] - truth[a][0] }); }
                10 if l1 != l2 => { s.constrain(Angle { l1, l2, degrees: (heading(l2) - heading(l1)).to_degrees() }); }
                11 => { s.constrain(Fixed { p: a, x: truth[a][0], y: truth[a][1] }); }
                _ => {}
            }
        }
        show(&format!("random{k}"), &s);
    }
}
