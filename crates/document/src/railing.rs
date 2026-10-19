//! Railings, as Revit has them: a path sketched in plan with a top rail at the railing height, posts where the path
//! bends and at its ends, and balusters between the posts at an even spacing close to the one asked for.
use crate::spatial::Mesh;
use crate::{Category, Command, Document, EditError, ElementId, Geometry};

/// What a new railing is made of.
#[derive(Clone, Debug, PartialEq)]
pub struct RailingSpec {
    /// the path in plan, two or more points
    pub path: Vec<[f64; 2]>,
    /// the height of the top rail above the base, in metres
    pub height_m: f64,
    /// the size of the rails and posts, in metres
    pub rail_m: f64,
    /// the distance between balusters, in metres; 0 for none
    pub baluster_spacing_m: f64,
    /// how far above the level the railing stands, in metres
    pub base_offset_m: f64,
}

impl RailingSpec {
    /// Revit's usual railing: 1100 mm high, a 50 mm top rail and posts, balusters about every 150 mm.
    pub fn new(path: Vec<[f64; 2]>) -> RailingSpec { RailingSpec { path, height_m: 1.1, rail_m: 0.05, baluster_spacing_m: 0.15, base_offset_m: 0.0 } }
}

/// The thickness of a baluster, in metres.
pub const BALUSTER_M: f64 = 0.02;

/// A box along the line from `p` to `q` (each with its own height range `za` at `p`, `zb` at `q`), `w` across, its
/// ends pushed out by `ext` along the line. A level bar has the same range at both ends; one on a stair is tilted.
fn bar(m: &mut Mesh, p: [f64; 2], q: [f64; 2], w: f64, za: [f64; 2], zb: [f64; 2], ext: f64) {
    let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
    let l = dx.hypot(dy).max(1e-12);
    let (d, n) = ([dx / l, dy / l], [-dy / l * w / 2.0, dx / l * w / 2.0]);
    let (a, b) = ([p[0] - d[0] * ext, p[1] - d[1] * ext], [q[0] + d[0] * ext, q[1] + d[1] * ext]);
    // the ends pushed out keep the slope of the bar
    let slope = [(zb[0] - za[0]) / l, (zb[1] - za[1]) / l];
    let (za, zb) = ([za[0] - slope[0] * ext, za[1] - slope[1] * ext], [zb[0] + slope[0] * ext, zb[1] + slope[1] * ext]);
    let ring = [[a[0] - n[0], a[1] - n[1]], [b[0] - n[0], b[1] - n[1]], [b[0] + n[0], b[1] + n[1]], [a[0] + n[0], a[1] + n[1]]];
    let (lo, hi) = ([za[0], zb[0], zb[0], za[0]], [za[1], zb[1], zb[1], za[1]]);
    let base = m.vertices.len();
    m.vertices.extend(ring.iter().zip(lo).map(|(r, z)| [r[0], r[1], z]));
    m.vertices.extend(ring.iter().zip(hi).map(|(r, z)| [r[0], r[1], z]));
    m.faces.push(vec![base + 3, base + 2, base + 1, base]);
    m.faces.push(vec![base + 4, base + 5, base + 6, base + 7]);
    for i in 0..4 { m.faces.push(vec![base + i, base + (i + 1) % 4, base + (i + 1) % 4 + 4, base + i + 4]); }
}

/// The solid parts of a railing on `path`, its base at `z0`: the top rail along each stretch, a post at each point and
/// the balusters in between. Each part is a closed box of its own.
#[cfg(test)]
pub fn mesh(path: &[[f64; 2]], z0: f64, height: f64, rail: f64, spacing: f64) -> Option<Mesh> {
    let path: Vec<[f64; 3]> = path.iter().map(|p| [p[0], p[1], z0]).collect();
    mesh3(&path, height, rail, spacing)
}

/// The same for a path whose points have a height of their own (the base of the railing there): the rail follows the
/// slope between them, the posts and balusters stand upright on it.
pub fn mesh3(path: &[[f64; 3]], height: f64, rail: f64, spacing: f64) -> Option<Mesh> {
    if path.len() < 2 || !(height > rail) || !(rail > 0.0) { return None; }
    let mut m = Mesh { vertices: Vec::new(), faces: Vec::new() };
    for w in path.windows(2) {
        let (p, q) = ([w[0][0], w[0][1]], [w[1][0], w[1][1]]);
        // the rail overlaps the posts at its ends, so that a bend has no gap on its outside
        bar(&mut m, p, q, rail, [w[0][2] + height - rail, w[0][2] + height], [w[1][2] + height - rail, w[1][2] + height], rail / 2.0);
        let len = (q[0] - p[0]).hypot(q[1] - p[1]);
        if spacing > 0.0 && len > 1e-9 {
            let n = (len / spacing).ceil().max(1.0);
            let d = [(q[0] - p[0]) / len, (q[1] - p[1]) / len];
            for k in 1..n as usize {
                let t = k as f64 / n;
                let c = [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
                let z = w[0][2] + (w[1][2] - w[0][2]) * t;
                let (a, b) = ([c[0] - d[0] * BALUSTER_M / 2.0, c[1] - d[1] * BALUSTER_M / 2.0], [c[0] + d[0] * BALUSTER_M / 2.0, c[1] + d[1] * BALUSTER_M / 2.0]);
                bar(&mut m, a, b, BALUSTER_M, [z, z + height - rail], [z, z + height - rail], 0.0);
            }
        }
    }
    for p in path {
        let post_w = rail.max(BALUSTER_M);
        bar(&mut m, [p[0] - post_w / 2.0, p[1]], [p[0] + post_w / 2.0, p[1]], post_w, [p[2], p[2] + height - rail], [p[2], p[2] + height - rail], 0.0);
    }
    Some(m)
}

impl Document {
    /// Puts a railing on `level`, in one undoable step.
    pub fn place_railing(&mut self, level: ElementId, spec: &RailingSpec) -> Result<ElementId, EditError> {
        let id = self.next_id();
        let n = self.elements().filter(|e| e.category == Category::Railing).count() + 1;
        self.transact(self.revision(), &[
            Command::Create { id, category: Category::Railing, name: format!("Railing {n}"), width_m: spec.rail_m, height_m: spec.height_m, elevation_m: spec.base_offset_m },
            Command::SetBeamSystem { id, spacing_m: spec.baluster_spacing_m, angle_deg: 0.0 },
            Command::SetOutline { id, outline: spec.path.clone() },
            Command::SetGeometry { id, geometry: Geometry::Area { level } },
        ])?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_railing_has_a_top_rail_a_post_at_every_point_and_even_balusters_and_each_part_is_a_closed_box() {
        let m = mesh(&[[0.0, 0.0], [3.0, 0.0], [3.0, 2.0]], 0.0, 1.1, 0.05, 0.15).unwrap();
        // 2 rails, 3 posts, balusters: 3 m in 20 steps leaves 19, 2 m in 14 leaves 13
        assert_eq!(m.faces.len() % 6, 0);
        assert_eq!(m.faces.len() / 6, 2 + 3 + 19 + 13);
        let top = m.vertices.iter().map(|v| v[2]).fold(f64::MIN, f64::max);
        let bottom = m.vertices.iter().map(|v| v[2]).fold(f64::MAX, f64::min);
        assert!((top - 1.1).abs() < 1e-9 && bottom.abs() < 1e-9);
        // no balusters when the spacing is 0
        assert_eq!(mesh(&[[0.0, 0.0], [3.0, 0.0]], 0.0, 1.1, 0.05, 0.0).unwrap().faces.len() / 6, 1 + 2);
        assert!(mesh(&[[0.0, 0.0]], 0.0, 1.1, 0.05, 0.15).is_none());
    }

    #[test]
    fn a_railing_is_placed_on_a_level_checked_moved_and_comes_back_from_the_records() {
        let mut d = Document::spatial_sample();
        let level = ElementId(10);
        let id = d.place_railing(level, &RailingSpec::new(vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]])).unwrap();
        assert!(d.mesh(id).is_some());
        // a path of one point, a path that stops and a baluster spacing that would fill the model are refused
        assert!(d.place_railing(level, &RailingSpec::new(vec![[0.0, 0.0]])).is_err());
        assert!(d.place_railing(level, &RailingSpec::new(vec![[0.0, 0.0], [0.0, 0.0]])).is_err());
        let mut far = RailingSpec::new(vec![[0.0, 0.0], [900.0, 0.0]]);
        far.baluster_spacing_m = 0.05;
        assert!(d.place_railing(level, &far).is_err());
        // it moves with the other tools and copies
        let (ops, new) = d.copy_commands(&[id], crate::Xf::Move([1.0, 1.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert!(matches!(d.get(new[0]).unwrap().outline.first(), Some(p) if (p[0] - 1.0).abs() < 1e-9 && (p[1] - 1.0).abs() < 1e-9));
        // moved, turned and mirrored in place; a mirror keeps it valid
        for xf in [crate::Xf::Move([2.0, -1.0]), crate::Xf::Rotate([0.0, 0.0], std::f64::consts::FRAC_PI_2), crate::Xf::Mirror([0.0, 0.0], [1.0, 0.0])] {
            let ops = d.transform_commands(&[id], xf).unwrap();
            d.transact(d.revision(), &ops).unwrap();
            assert!(d.mesh(id).is_some());
        }
        assert!(matches!(d.get(id).unwrap().outline.first(), Some(p) if p[0].is_finite()));
        let (back, _) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
        assert_eq!(back.iter().find(|e| e.id == id).unwrap().outline, d.get(id).unwrap().outline);
        assert_eq!(back.iter().find(|e| e.id == id).unwrap().category, Category::Railing);
    }
}
