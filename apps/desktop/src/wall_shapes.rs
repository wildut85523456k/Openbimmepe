//! The shapes the Wall tool draws besides a line and a rectangle: polygons, circles and arcs. Walls here are straight, so
//! a circle or an arc is built from straight walls that follow it within `MAX_SAGITTA_M`; the points come out clockwise
//! for a closed shape (so that the exterior, on the left of the direction, faces out) and from the start to the end for
//! an arc.
use std::f64::consts::{PI, TAU};

type P = [f64; 2];

/// How far a straight wall may stand inside the circle or arc it stands for.
pub const MAX_SAGITTA_M: f64 = 0.025;
/// The smallest radius, and the fewest and most sides a polygon has.
pub const MIN_RADIUS_M: f64 = 0.1;
pub const MIN_SIDES: usize = 3;
pub const MAX_SIDES: usize = 64;
/// The most walls one circle or arc is made of.
const MAX_STEPS: usize = 360;

/// How many straight walls make a sweep of `sweep` radians of a circle of `radius` stay within the tolerance.
pub fn steps(radius: f64, sweep: f64) -> usize {
    let half = (1.0 - MAX_SAGITTA_M / radius).clamp(-1.0, 1.0).acos();
    let n = if half <= 1e-9 { MAX_STEPS } else { (sweep.abs() / (2.0 * half)).ceil() as usize };
    n.clamp(1, MAX_STEPS)
}

fn at(c: P, r: f64, angle: f64) -> P { [c[0] + r * angle.cos(), c[1] + r * angle.sin()] }

fn angle_of(c: P, p: P) -> f64 { (p[1] - c[1]).atan2(p[0] - c[0]) }

/// A regular polygon around `center` with `sides` sides, clockwise. Inscribed, `to` is a corner; circumscribed, `to` is
/// the middle of a side.
pub fn polygon(center: P, to: P, sides: usize, circumscribed: bool) -> Option<Vec<P>> {
    let r = (to[0] - center[0]).hypot(to[1] - center[1]);
    if !(MIN_SIDES..=MAX_SIDES).contains(&sides) || !r.is_finite() || r < MIN_RADIUS_M { return None; }
    let (n, phi) = (sides as f64, angle_of(center, to));
    let (radius, first) = if circumscribed { (r / (PI / n).cos(), phi + PI / n) } else { (r, phi) };
    Some((0..sides).map(|k| at(center, radius, first - TAU * k as f64 / n)).collect())
}

/// A circle through `to` around `center`, clockwise, as the corners of the walls that make it.
pub fn circle(center: P, to: P) -> Option<Vec<P>> {
    let r = (to[0] - center[0]).hypot(to[1] - center[1]);
    if !r.is_finite() || r < MIN_RADIUS_M { return None; }
    let n = steps(r, TAU).max(12);
    let phi = angle_of(center, to);
    Some((0..n).map(|k| at(center, r, phi - TAU * k as f64 / n as f64)).collect())
}

/// The arc from `a` to `b` through `t`, as the points of the walls that make it (the first is `a`, the last `b`). None
/// when the three points are on a line or two of them are the same.
pub fn arc(a: P, b: P, t: P) -> Option<Vec<P>> {
    let d = 2.0 * (a[0] * (b[1] - t[1]) + b[0] * (t[1] - a[1]) + t[0] * (a[1] - b[1]));
    let len = |p: P, q: P| (p[0] - q[0]).hypot(p[1] - q[1]);
    if len(a, b).min(len(b, t)).min(len(a, t)) < 1e-6 { return None; }
    let (sa, sb, st) = (a[0] * a[0] + a[1] * a[1], b[0] * b[0] + b[1] * b[1], t[0] * t[0] + t[1] * t[1]);
    // a line has no circle through three of its points; 1e-9 of the squared size is a line to the eye
    if d.abs() < 1e-9 * (sa + sb + st).max(1.0) { return None; }
    let c = [(sa * (b[1] - t[1]) + sb * (t[1] - a[1]) + st * (a[1] - b[1])) / d, (sa * (t[0] - b[0]) + sb * (a[0] - t[0]) + st * (b[0] - a[0])) / d];
    let r = len(c, a);
    if !r.is_finite() || r < MIN_RADIUS_M || r > 1e5 { return None; }
    let wrap = |x: f64| x.rem_euclid(TAU);
    let (tb, tt) = (wrap(angle_of(c, b) - angle_of(c, a)), wrap(angle_of(c, t) - angle_of(c, a)));
    // counter-clockwise from a to b when that way meets t, else clockwise
    let sweep = if tt < tb { tb } else { tb - TAU };
    let n = steps(r, sweep);
    let phi = angle_of(c, a);
    let mut out: Vec<P> = (0..n).map(|k| at(c, r, phi + sweep * k as f64 / n as f64)).collect();
    out.push(b);
    out[0] = a;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist(a: P, b: P) -> f64 { (a[0] - b[0]).hypot(a[1] - b[1]) }
    /// twice the signed area: negative for clockwise
    fn area2(p: &[P]) -> f64 { p.iter().zip(p.iter().cycle().skip(1)).map(|(a, b)| a[0] * b[1] - b[0] * a[1]).sum() }

    #[test]
    fn a_polygon_is_clockwise_regular_and_inscribed_or_circumscribed() {
        let ins = polygon([1.0, 2.0], [4.0, 2.0], 6, false).unwrap();
        assert_eq!(ins.len(), 6);
        assert!(area2(&ins) < 0.0, "clockwise");
        assert!(ins.iter().all(|p| (dist(*p, [1.0, 2.0]) - 3.0).abs() < 1e-9), "corners on the circle");
        assert!(dist(ins[0], [4.0, 2.0]) < 1e-9, "the picked point is a corner");
        // six sides of an inscribed hexagon are as long as its radius
        assert!((0..6).all(|i| (dist(ins[i], ins[(i + 1) % 6]) - 3.0).abs() < 1e-9));
        let cir = polygon([1.0, 2.0], [4.0, 2.0], 4, true).unwrap();
        // a square whose side is 6 m: the picked point is the middle of its east side
        assert!((0..4).all(|i| (dist(cir[i], cir[(i + 1) % 4]) - 6.0).abs() < 1e-9), "{cir:?}");
        assert!(cir.iter().any(|p| (p[0] - 4.0).abs() < 1e-9 && (p[1] - 5.0).abs() < 1e-9) && cir.iter().any(|p| (p[0] - 4.0).abs() < 1e-9 && (p[1] + 1.0).abs() < 1e-9));
        assert!(area2(&cir) < 0.0);
        assert!(polygon([0.0; 2], [0.05, 0.0], 5, false).is_none() && polygon([0.0; 2], [1.0, 0.0], 2, false).is_none() && polygon([0.0; 2], [1.0, 0.0], 65, true).is_none());
    }

    #[test]
    fn a_circle_stays_within_the_tolerance_and_a_bigger_one_has_more_walls() {
        let small = circle([0.0, 0.0], [0.5, 0.0]).unwrap();
        let big = circle([0.0, 0.0], [6.0, 0.0]).unwrap();
        assert!(small.len() >= 12 && big.len() > small.len());
        assert!(area2(&big) < 0.0);
        for ring in [&small, &big] {
            let r = dist(ring[0], [0.0, 0.0]);
            for i in 0..ring.len() {
                let mid = [(ring[i][0] + ring[(i + 1) % ring.len()][0]) / 2.0, (ring[i][1] + ring[(i + 1) % ring.len()][1]) / 2.0];
                assert!(r - dist(mid, [0.0, 0.0]) <= MAX_SAGITTA_M + 1e-9, "a wall stands {} m inside", r - dist(mid, [0.0; 2]));
            }
        }
        assert!(circle([0.0; 2], [0.01, 0.0]).is_none());
    }

    #[test]
    fn an_arc_runs_from_start_to_end_through_the_third_point_either_way_round() {
        // a half circle of radius 2 about the origin, over the top and under the bottom
        for through in [[0.0, 2.0], [0.0, -2.0]] {
            let a = arc([2.0, 0.0], [-2.0, 0.0], through).unwrap();
            assert!(dist(a[0], [2.0, 0.0]) < 1e-12 && dist(*a.last().unwrap(), [-2.0, 0.0]) < 1e-12);
            assert!(a.iter().all(|p| (dist(*p, [0.0, 0.0]) - 2.0).abs() < 1e-9));
            assert!(a.iter().any(|p| dist(*p, through) < 0.2), "it passes by {through:?}");
            assert_eq!(a.iter().all(|p| p[1] >= -1e-9), through[1] > 0.0);
        }
        // the longer way round when the third point is on it
        let long = arc([2.0, 0.0], [0.0, 2.0], [-2.0, 0.0]).unwrap();
        assert!(long.iter().any(|p| dist(*p, [-2.0, 0.0]) < 0.2) && long.len() > arc([2.0, 0.0], [0.0, 2.0], [2f64.sqrt(), 2f64.sqrt()]).unwrap().len());
        // on a line, repeated points or a tiny radius give nothing
        assert!(arc([0.0, 0.0], [2.0, 0.0], [1.0, 0.0]).is_none() && arc([0.0, 0.0], [0.0, 0.0], [1.0, 1.0]).is_none() && arc([0.0, 0.0], [0.05, 0.0], [0.025, 0.01]).is_none());
        // consecutive points are never further apart than the tolerance allows
        let a = arc([5.0, 0.0], [-5.0, 0.0], [0.0, 5.0]).unwrap();
        for w in a.windows(2) {
            let r = 5.0;
            let half = dist(w[0], w[1]) / 2.0;
            assert!(r - (r * r - half * half).sqrt() <= MAX_SAGITTA_M + 1e-9);
        }
    }
}
