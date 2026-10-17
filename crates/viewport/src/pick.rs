//! Picking by ray, the section box, and the blend between two cameras (for the animated ViewCube).

use crate::{add, cross, dot, mul, sub, unit, Camera, FOV, V};
use openbim_view::{Batch, Element, FaceKind};
use rayon::prelude::*;

/// A box in model coordinates that cuts away everything outside it (Revit's Section Box).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionBox { pub min: V, pub max: V }

impl SectionBox {
    pub fn contains(&self, p: V) -> bool { (0..3).all(|k| p[k] >= self.min[k] - 1e-9 && p[k] <= self.max[k] + 1e-9) }
}

/// What a click on the view found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub triangle: usize,
    pub element: Element,
    pub face: FaceKind,
    /// metres from the eye along the ray
    pub distance: f64,
    pub point: V,
}

impl Camera {
    /// The ray through a pixel: the eye and the unit direction.
    pub fn ray(&self, px: [f64; 2], size: [f64; 2]) -> (V, V) {
        let f = size[1] / 2.0 / (FOV / 2.0).tan();
        let d = add(self.forward(), add(mul(self.right(), (px[0] - size[0] / 2.0) / f), mul(self.up(), -(px[1] - size[1] / 2.0) / f)));
        (self.eye(), unit(d))
    }

    /// A camera part of the way from `a` to `b`: turning the short way round, and moving the distance by a ratio so
    /// that zooming feels even.
    pub fn blend(a: &Camera, b: &Camera, t: f64) -> Camera {
        let t = t.clamp(0.0, 1.0);
        let mut dy = (b.yaw - a.yaw) % std::f64::consts::TAU;
        if dy > std::f64::consts::PI { dy -= std::f64::consts::TAU; }
        if dy < -std::f64::consts::PI { dy += std::f64::consts::TAU; }
        Camera {
            target: add(mul(a.target, 1.0 - t), mul(b.target, t)),
            distance: a.distance * (b.distance / a.distance).powf(t),
            yaw: a.yaw + dy * t,
            pitch: a.pitch + (b.pitch - a.pitch) * t,
        }
    }
}

/// The first front face the ray through `px` hits, ignoring what the section box has cut away.
pub fn pick(b: &Batch, cam: &Camera, px: [f64; 2], size: [f64; 2], section: Option<SectionBox>) -> Option<Hit> {
    let (o, d) = cam.ray(px, size);
    let v = |i: u32| { let p = b.verts[i as usize]; [p[0] as f64 + b.origin[0], p[1] as f64 + b.origin[1], p[2] as f64 + b.origin[2]] };
    let best = (0..b.tris.len()).into_par_iter().filter_map(|ti| {
        let t = b.tris[ti];
        let (p0, p1, p2) = (v(t[0]), v(t[1]), v(t[2]));
        let (e1, e2) = (sub(p1, p0), sub(p2, p0));
        // facing the eye
        if dot(cross(e1, e2), d) >= 0.0 { return None; }
        // Möller–Trumbore
        let h = cross(d, e2);
        let a = dot(e1, h);
        if a.abs() < 1e-14 { return None; }
        let s = sub(o, p0);
        let u = dot(s, h) / a;
        if !(-1e-9..=1.0 + 1e-9).contains(&u) { return None; }
        let q = cross(s, e1);
        let w = dot(d, q) / a;
        if w < -1e-9 || u + w > 1.0 + 1e-9 { return None; }
        let dist = dot(e2, q) / a;
        if dist <= 0.0 { return None; }
        let point = add(o, mul(d, dist));
        if section.is_some_and(|s| !s.contains(point)) { return None; }
        Some((dist, ti, point))
    }).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))?;
    let (distance, triangle, point) = best;
    Some(Hit { triangle, element: b.elements[b.tri_element[triangle] as usize], face: b.tri_face[triangle], distance, point })
}
