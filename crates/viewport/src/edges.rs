//! The lines of a drawing in Hidden Line and Wireframe: creases and boundaries of the surfaces, and the lines where
//! two elements cut into each other.

use openbim_view::Batch;
use rayon::prelude::*;
use std::collections::HashMap;

type V = [f64; 3];
/// A line between two model points, relative to the batch origin like its vertices.
pub type Edge = [V; 2];

fn sub(a: V, b: V) -> V { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn dot(a: V, b: V) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: V, b: V) -> V { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn unit(a: V) -> V { let l = dot(a, a).sqrt(); if l == 0.0 { a } else { [a[0] / l, a[1] / l, a[2] / l] } }

fn vert(b: &Batch, i: u32) -> V { let v = b.verts[i as usize]; [v[0] as f64, v[1] as f64, v[2] as f64] }
fn tri(b: &Batch, t: usize) -> [V; 3] { let k = b.tris[t]; [vert(b, k[0]), vert(b, k[1]), vert(b, k[2])] }
fn normal(t: [V; 3]) -> V { unit(cross(sub(t[1], t[0]), sub(t[2], t[0]))) }

/// Edges of the surfaces: those with a single triangle, those between two triangles more than `crease_deg` apart,
/// and those between triangles of different elements. Edges inside a flat or smoothly curved face are left out.
pub fn crease_edges(b: &Batch, crease_deg: f64) -> Vec<Edge> {
    let cos = crease_deg.to_radians().cos();
    let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (ti, t) in b.tris.iter().enumerate() {
        for k in 0..3 {
            let (p, q) = (t[k], t[(k + 1) % 3]);
            edges.entry((p.min(q), p.max(q))).or_default().push(ti);
        }
    }
    let mut keys: Vec<_> = edges.keys().copied().collect();
    keys.sort_unstable();
    keys.into_iter().filter_map(|key| {
        let tl = &edges[&key];
        let drawn = tl.len() != 2 || dot(normal(tri(b, tl[0])), normal(tri(b, tl[1]))) < cos || b.tri_element[tl[0]] != b.tri_element[tl[1]];
        drawn.then(|| [vert(b, key.0), vert(b, key.1)])
    }).collect()
}

/// The segment where two triangles cut through each other, if they do (coplanar triangles give none).
fn tri_tri(a: [V; 3], b: [V; 3]) -> Option<Edge> {
    const EPS: f64 = 1e-9;
    let (na, nb) = (normal(a), normal(b));
    let dir = cross(na, nb);
    if dot(dir, dir) < 1e-12 { return None; }
    let dir = unit(dir);
    // where the edges of `t` pass through the plane (n, point p0); None if `t` does not reach it
    let hit = |t: [V; 3], n: V, p0: V| -> Option<[V; 2]> {
        let d = [dot(n, sub(t[0], p0)), dot(n, sub(t[1], p0)), dot(n, sub(t[2], p0))];
        if d.iter().all(|x| *x > EPS) || d.iter().all(|x| *x < -EPS) { return None; }
        let mut pts: Vec<V> = Vec::new();
        for k in 0..3 {
            let (j, dk, dj) = ((k + 1) % 3, d[k], d[(k + 1) % 3]);
            if dk.abs() <= EPS { pts.push(t[k]); }
            if (dk > EPS && dj < -EPS) || (dk < -EPS && dj > EPS) {
                let s = dk / (dk - dj);
                pts.push([t[k][0] + (t[j][0] - t[k][0]) * s, t[k][1] + (t[j][1] - t[k][1]) * s, t[k][2] + (t[j][2] - t[k][2]) * s]);
            }
        }
        if pts.len() < 2 { return None; }
        // the two farthest apart along the line
        pts.sort_by(|p, q| dot(dir, *p).total_cmp(&dot(dir, *q)));
        Some([pts[0], pts[pts.len() - 1]])
    };
    let sa = hit(a, nb, b[0])?;
    let sb = hit(b, na, a[0])?;
    let (lo_a, hi_a) = (dot(dir, sa[0]), dot(dir, sa[1]));
    let (lo_b, hi_b) = (dot(dir, sb[0]), dot(dir, sb[1]));
    let lo = if lo_a >= lo_b { sa[0] } else { sb[0] };
    let hi = if hi_a <= hi_b { sa[1] } else { sb[1] };
    (dot(dir, hi) - dot(dir, lo) > 1e-6).then_some([lo, hi])
}

/// The lines where triangles of different elements cut through each other (the end of a wall in the face of another, a
/// column standing in a slab). Triangles that merely touch, or lie in one plane, give no line.
pub fn intersection_edges(b: &Batch) -> Vec<Edge> {
    let tris: Vec<[V; 3]> = (0..b.tris.len()).map(|t| tri(b, t)).collect();
    let boxes: Vec<[V; 2]> = tris.iter().map(|t| {
        let lo = [0, 1, 2].map(|k| t[0][k].min(t[1][k]).min(t[2][k]));
        let hi = [0, 1, 2].map(|k| t[0][k].max(t[1][k]).max(t[2][k]));
        [lo, hi]
    }).collect();
    let mut order: Vec<usize> = (0..tris.len()).collect();
    order.sort_by(|&i, &j| boxes[i][0][0].total_cmp(&boxes[j][0][0]));
    let found: Vec<Vec<Edge>> = (0..order.len()).into_par_iter().map(|oi| {
        let i = order[oi];
        let mut out = Vec::new();
        for &j in &order[oi + 1..] {
            if boxes[j][0][0] > boxes[i][1][0] + 1e-9 { break; }
            if b.tri_element[i] == b.tri_element[j] { continue; }
            if (1..3).any(|k| boxes[j][0][k] > boxes[i][1][k] + 1e-9 || boxes[i][0][k] > boxes[j][1][k] + 1e-9) { continue; }
            if let Some(e) = tri_tri(tris[i], tris[j]) { out.push(e); }
        }
        out
    }).collect();
    found.into_iter().flatten().collect()
}
