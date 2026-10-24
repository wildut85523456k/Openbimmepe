//! The arc of a curved grid. A grid keeps its straight chord (`Geometry::Grid { start, end }`), so that selecting,
//! moving, copying and snapping work on it as on any grid; its `bulge` bends the chord into an arc, as the bulge of a
//! DXF polyline does: `tan(included angle / 4)`, positive when the arc runs counter-clockwise from `start` to `end`
//! (the centre then lies to the left of the chord), negative for clockwise, 0 for a straight grid. A bulge of 1 is a
//! half circle, the largest arc a grid can be.

/// An arc in the plan: its centre and radius, the angle it starts at and the signed angle it sweeps (radians).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    pub centre: [f64; 2],
    pub radius: f64,
    pub start_angle: f64,
    pub sweep: f64,
}

impl Arc {
    pub fn point(&self, t: f64) -> [f64; 2] {
        let a = self.start_angle + self.sweep * t;
        [self.centre[0] + self.radius * a.cos(), self.centre[1] + self.radius * a.sin()]
    }
    /// The length along the arc.
    pub fn length(&self) -> f64 { self.radius * self.sweep.abs() }
}

/// The arc a chord and a bulge make; `None` for a straight grid or a chord with no length.
pub fn arc(start: [f64; 2], end: [f64; 2], bulge: f64) -> Option<Arc> {
    let (dx, dy) = (end[0] - start[0], end[1] - start[1]);
    let chord = dx.hypot(dy);
    if bulge == 0.0 || !bulge.is_finite() || chord < 1e-9 { return None; }
    let sweep = 4.0 * bulge.atan();
    let radius = chord / (2.0 * (sweep.abs() / 2.0).sin());
    // the centre is along the left normal of the chord, as far from its middle as the arc's apothem
    let off = (chord / 2.0) / (sweep / 2.0).tan();
    let (nx, ny) = (-dy / chord, dx / chord);
    let centre = [(start[0] + end[0]) / 2.0 + nx * off, (start[1] + end[1]) / 2.0 + ny * off];
    Some(Arc { centre, radius, start_angle: (start[1] - centre[1]).atan2(start[0] - centre[0]), sweep })
}

/// The points of the grid as a polyline: the two ends for a straight grid, the arc cut into steps of at most 5 degrees
/// (ends exact) for a curved one.
pub fn points(start: [f64; 2], end: [f64; 2], bulge: f64) -> Vec<[f64; 2]> {
    let Some(a) = arc(start, end, bulge) else { return vec![start, end] };
    let n = ((a.sweep.abs() / 5f64.to_radians()).ceil() as usize).max(2);
    let mut out: Vec<[f64; 2]> = (0..=n).map(|i| a.point(i as f64 / n as f64)).collect();
    out[0] = start;
    out[n] = end;
    out
}

/// The bulge of the arc about `centre` that runs from `start` to `end` counter-clockwise (`ccw`) or clockwise. `None`
/// when it is more than a half circle (a grid cannot be) or has no size.
pub fn bulge_of(start: [f64; 2], end: [f64; 2], centre: [f64; 2], ccw: bool) -> Option<f64> {
    let ang = |p: [f64; 2]| (p[1] - centre[1]).atan2(p[0] - centre[0]);
    let tau = std::f64::consts::TAU;
    let d = (ang(end) - ang(start)).rem_euclid(tau);
    if d < 1e-9 || tau - d < 1e-9 { return None; }
    let sweep = if ccw { d } else { d - tau };
    if sweep.abs() > std::f64::consts::PI + 1e-6 { return None; }
    Some((sweep / 4.0).tan().clamp(-1.0, 1.0))
}

/// The point of an arc's chord-side that is farthest from the chord: where its label or a handle goes.
pub fn middle(start: [f64; 2], end: [f64; 2], bulge: f64) -> [f64; 2] {
    arc(start, end, bulge).map_or([(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0], |a| a.point(0.5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quarter_circle_has_the_bulge_tan_of_a_sixteenth_turn_and_its_centre_to_the_left() {
        // from (1,0) to (0,1) counter-clockwise about the origin: a quarter turn
        let b = (std::f64::consts::FRAC_PI_2 / 4.0).tan();
        let a = arc([1.0, 0.0], [0.0, 1.0], b).unwrap();
        assert!(a.centre[0].abs() < 1e-12 && a.centre[1].abs() < 1e-12, "{a:?}");
        assert!((a.radius - 1.0).abs() < 1e-12 && (a.sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        let m = a.point(0.5);
        assert!((m[0] - 0.5f64.sqrt()).abs() < 1e-12 && (m[1] - 0.5f64.sqrt()).abs() < 1e-12);
        // clockwise from (0,1) to (1,0): the same arc the other way
        let c = arc([0.0, 1.0], [1.0, 0.0], -b).unwrap();
        assert!(c.centre[0].abs() < 1e-12 && c.centre[1].abs() < 1e-12 && c.sweep < 0.0, "{c:?}");
        assert_eq!(arc([0.0, 0.0], [1.0, 0.0], 0.0), None);
        assert_eq!(arc([0.0, 0.0], [0.0, 0.0], 0.5), None);
    }

    #[test]
    fn a_half_circle_has_its_centre_on_the_chord_and_the_ends_of_the_polyline_are_exact() {
        let a = arc([-2.0, 0.0], [2.0, 0.0], 1.0).unwrap();
        // counter-clockwise from the left end to the right end passes under the chord
        assert!(a.centre[0].abs() < 1e-12 && a.centre[1].abs() < 1e-9 && (a.radius - 2.0).abs() < 1e-12, "{a:?}");
        assert!(a.point(0.5)[1] < 0.0);
        let p = points([-2.0, 0.0], [2.0, 0.0], 1.0);
        assert_eq!((p[0], *p.last().unwrap()), ([-2.0, 0.0], [2.0, 0.0]));
        assert!(p.len() >= 37, "5 degrees at most: {}", p.len());
        assert!(p.iter().all(|q| (q[0].hypot(q[1]) - 2.0).abs() < 1e-9));
        assert_eq!(points([0.0, 0.0], [3.0, 4.0], 0.0), vec![[0.0, 0.0], [3.0, 4.0]]);
    }

    #[test]
    fn the_bulge_comes_back_from_a_centre_and_a_sense_and_a_major_arc_is_refused() {
        for b in [0.3, -0.3, 1.0, -1.0, 0.01] {
            let (s, e) = ([3.0, 1.0], [7.0, 4.0]);
            let a = arc(s, e, b).unwrap();
            let back = bulge_of(s, e, a.centre, b > 0.0).unwrap();
            assert!((back - b).abs() < 1e-9, "{b} {back}");
        }
        // three quarters of a circle about the origin is more than a half circle
        assert_eq!(bulge_of([1.0, 0.0], [0.0, -1.0], [0.0, 0.0], true), None);
        assert!(bulge_of([1.0, 0.0], [0.0, -1.0], [0.0, 0.0], false).is_some());
        assert_eq!(bulge_of([1.0, 0.0], [1.0, 0.0], [0.0, 0.0], true), None);
    }
}
