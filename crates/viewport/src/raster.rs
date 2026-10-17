//! Software renderer: depth buffer, back-face culling, flat shading, crease and silhouette edges.

use crate::{cross, dot, sub, unit, Camera, V};
use openbim_view::{Batch, FaceKind, Kind};
use std::collections::HashMap;

/// The visual styles that need no textures or lights beyond one headlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Wireframe,
    HiddenLine,
    Shaded,
    ConsistentColors,
}

/// RGBA, row by row from the top.
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }
    /// Binary PPM, for tests and for looking at.
    pub fn ppm(&self) -> Vec<u8> {
        let mut o = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        for p in self.rgba.chunks_exact(4) { o.extend_from_slice(&p[..3]); }
        o
    }
}

const BACKGROUND: [u8; 3] = [255, 255, 255];
const EDGE: [u8; 3] = [20, 20, 20];
const CREASE_COS: f64 = 0.9; // edges between faces more than about 25 degrees apart

fn kind_colour(k: Kind) -> [f64; 3] {
    match k {
        Kind::Wall => [0.82, 0.80, 0.76],
        Kind::Slab => [0.62, 0.62, 0.64],
        Kind::Column => [0.70, 0.66, 0.60],
        Kind::Family => [0.45, 0.62, 0.78],
        Kind::Segment => [0.40, 0.58, 0.72],
        Kind::Fitting => [0.30, 0.45, 0.62],
        Kind::Insulation => [0.90, 0.85, 0.55],
        _ => [0.8, 0.8, 0.8],
    }
}

/// A stable muted colour for a material name.
fn material_colour(name: &str) -> [f64; 3] {
    let mut h: u32 = 2166136261;
    for b in name.bytes() { h = (h ^ b as u32).wrapping_mul(16777619); }
    let c = |s: u32| 0.55 + ((h >> s) & 0xff) as f64 / 255.0 * 0.35;
    [c(0), c(8), c(16)]
}

fn world(b: &Batch, i: u32) -> V {
    let v = b.verts[i as usize];
    [v[0] as f64 + b.origin[0], v[1] as f64 + b.origin[1], v[2] as f64 + b.origin[2]]
}

/// Draw the batch as seen by `cam` into a `width` x `height` image.
pub fn render(b: &Batch, cam: &Camera, width: usize, height: usize, style: Style) -> Image {
    let size = [width as f64, height as f64];
    let mut img = Image { width, height, rgba: BACKGROUND.iter().chain(&[255]).copied().cycle().take(width * height * 4).collect() };
    let mut depth = vec![f64::INFINITY; width * height];
    let mut owner = vec![u32::MAX; width * height];
    let eye = cam.eye();
    let (r, u, f) = (cam.right(), cam.up(), cam.forward());
    let headlight = |n: V| -> f64 {
        // light fixed to the camera: from up-left of the eye
        let l = unit([-0.35, 0.45, 0.8]);
        let l = [r[0] * l[0] + u[0] * l[1] - f[0] * l[2], r[1] * l[0] + u[1] * l[1] - f[1] * l[2], r[2] * l[0] + u[2] * l[1] - f[2] * l[2]];
        dot(n, unit(l)).max(0.0)
    };

    struct Tri { p: [[f64; 3]; 3], colour: [u8; 3], el: u32 }
    let mut tris: Vec<Tri> = Vec::new();
    let mut normals: Vec<V> = Vec::with_capacity(b.tris.len());
    for (ti, t) in b.tris.iter().enumerate() {
        let w = [world(b, t[0]), world(b, t[1]), world(b, t[2])];
        let n = unit(cross(sub(w[1], w[0]), sub(w[2], w[0])));
        normals.push(n);
        if dot(n, sub(eye, w[0])) <= 0.0 { continue; } // facing away
        let Some(p0) = cam.project(w[0], size) else { continue };
        let Some(p1) = cam.project(w[1], size) else { continue };
        let Some(p2) = cam.project(w[2], size) else { continue };
        let el = b.elements[b.tri_element[ti] as usize];
        let base = match (style, b.tri_material[ti]) {
            (Style::Shaded, m) if m != u32::MAX => material_colour(&b.materials[m as usize]),
            _ => kind_colour(el.kind),
        };
        let top_boost = if b.tri_face[ti] == FaceKind::Top { 1.0 } else { 0.0 };
        let colour = match style {
            Style::HiddenLine | Style::Wireframe => [255, 255, 255],
            _ => {
                let k = 0.45 + 0.55 * headlight(n) + 0.05 * top_boost;
                [0, 1, 2].map(|c| (base[c] * k.min(1.0) * 255.0).round() as u8)
            }
        };
        tris.push(Tri { p: [p0, p1, p2], colour, el: b.tri_element[ti] });
    }

    // fill
    for t in &tris {
        let [a, bb, c] = t.p;
        let area = (bb[0] - a[0]) * (c[1] - a[1]) - (bb[1] - a[1]) * (c[0] - a[0]);
        if area.abs() < 1e-12 { continue; }
        let (x0, x1) = (a[0].min(bb[0]).min(c[0]).floor().max(0.0) as usize, (a[0].max(bb[0]).max(c[0]).ceil().min(size[0] - 1.0)).max(0.0) as usize);
        let (y0, y1) = (a[1].min(bb[1]).min(c[1]).floor().max(0.0) as usize, (a[1].max(bb[1]).max(c[1]).ceil().min(size[1] - 1.0)).max(0.0) as usize);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let w0 = ((bb[0] - px) * (c[1] - py) - (bb[1] - py) * (c[0] - px)) / area;
                let w1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-9 || w1 < -1e-9 || w2 < -1e-9 { continue; }
                // 1/z is linear in the screen
                let z = 1.0 / (w0 / a[2] + w1 / bb[2] + w2 / c[2]);
                let i = y * width + x;
                if z < depth[i] {
                    depth[i] = z;
                    owner[i] = t.el;
                    if style != Style::Wireframe {
                        img.rgba[i * 4..i * 4 + 3].copy_from_slice(&t.colour);
                    }
                }
            }
        }
    }

    // Where two elements meet (the end of a wall in the face of another, a column on a slab) no edge of either mesh
    // lies on the line, so find it in the picture: neighbouring pixels of different elements at nearly the same depth.
    // A big difference in depth is a silhouette; the mesh edges draw those.
    if style == Style::HiddenLine {
        for y in 0..height {
            for x in 0..width {
                let i = y * width + x;
                if owner[i] == u32::MAX { continue; }
                for j in [if x + 1 < width { Some(i + 1) } else { None }, if y + 1 < height { Some(i + width) } else { None }].into_iter().flatten() {
                    if owner[j] != u32::MAX && owner[j] != owner[i] && (depth[i] - depth[j]).abs() < 0.01 * depth[i].min(depth[j]) {
                        img.rgba[i * 4..i * 4 + 3].copy_from_slice(&EDGE);
                    }
                }
            }
        }
    }

    // edges: sharp creases and the boundary of the mesh, tested against the depth buffer (all of them in Wireframe)
    if style == Style::HiddenLine || style == Style::Wireframe {
        let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (ti, t) in b.tris.iter().enumerate() {
            for k in 0..3 {
                let (p, q) = (t[k], t[(k + 1) % 3]);
                edges.entry((p.min(q), p.max(q))).or_default().push(ti);
            }
        }
        let mut keys: Vec<_> = edges.keys().copied().collect();
        keys.sort_unstable();
        for key in keys {
            let tl = &edges[&key];
            let drawn = style == Style::Wireframe || tl.len() != 2 || dot(normals[tl[0]], normals[tl[1]]) < CREASE_COS
                || b.tri_element[tl[0]] != b.tri_element[tl[1]];
            if !drawn { continue; }
            // in hidden-line mode a silhouette of a smooth join is not drawn; boundaries and creases are
            let (Some(p), Some(q)) = (cam.project(world(b, key.0), size), cam.project(world(b, key.1), size)) else { continue };
            let steps = ((q[0] - p[0]).abs().max((q[1] - p[1]).abs()).ceil() as usize).max(1);
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let (x, y) = (p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t);
                if x < 0.0 || y < 0.0 || x >= size[0] || y >= size[1] { continue; }
                let z = 1.0 / ((1.0 - t) / p[2] + t / q[2]);
                let i = y as usize * width + x as usize;
                if style == Style::Wireframe || z <= depth[i] * (1.0 + 1e-3) + 0.02 {
                    img.rgba[i * 4..i * 4 + 3].copy_from_slice(&EDGE);
                }
            }
        }
    }
    img
}
