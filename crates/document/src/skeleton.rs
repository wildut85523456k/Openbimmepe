//! The straight skeleton of a simple polygon: the faces of a hip roof whose every edge slopes alike, valleys included.
//! The outline shrinks inward with every edge moving at the same speed, its corners keeping the angle (mitred). Two
//! things change its shape: an edge shrinks to nothing (its neighbours then meet) and a reflex corner runs into an edge
//! (the outline splits in two). What each edge sweeps while it lives is a trapezoid at the slope of that edge: the
//! trapezoids of an edge are its face, and they tile the polygon.
type P = [f64; 2];

const EPS: f64 = 1e-9;

#[derive(Clone, Copy)]
struct Vtx {
    p: P,
    t0: f64,
    vel: P,
    prev: usize,
    next: usize,
    /// the edge arriving at the corner and the one leaving it
    el: usize,
    er: usize,
    alive: bool,
}

fn sub(a: P, b: P) -> P { [a[0] - b[0], a[1] - b[1]] }
fn dot(a: P, b: P) -> f64 { a[0] * b[0] + a[1] * b[1] }
fn cross(a: P, b: P) -> f64 { a[0] * b[1] - a[1] * b[0] }

struct Walk {
    n: [P; 64],
    d: [P; 64],
    c: [f64; 64],
    v: Vec<Vtx>,
    out: Vec<(usize, Vec<P>)>,
}

impl Walk {
    fn at(&self, i: usize, t: f64) -> P { let v = &self.v[i]; [v.p[0] + v.vel[0] * (t - v.t0), v.p[1] + v.vel[1] * (t - v.t0)] }

    /// A corner between edges `el` and `er` made at time `t` at `p`.
    fn corner(&mut self, p: P, t: f64, el: usize, er: usize, prev: usize, next: usize) -> usize {
        let (nl, nr) = (self.n[el], self.n[er]);
        let k = 1.0 + dot(nl, nr);
        // edges that fold back on one another leave a corner that no longer moves
        let vel = if k < 1e-7 { [0.0, 0.0] } else { [(nl[0] + nr[0]) / k, (nl[1] + nr[1]) / k] };
        self.v.push(Vtx { p, t0: t, vel, prev, next, el, er, alive: true });
        self.v.len() - 1
    }

    /// The part of the face of the edge `a -> b` swept while both its ends stayed the same, up to time `t`.
    fn emit(&mut self, a: usize, b: usize, t: f64) {
        let tb = self.v[a].t0.max(self.v[b].t0);
        let edge = self.v[a].er;
        let q = [self.at(a, tb), self.at(b, tb), self.at(b, t), self.at(a, t)];
        // drop corners that coincide
        let mut poly: Vec<P> = Vec::new();
        for p in q { if poly.last().is_none_or(|l| (l[0] - p[0]).hypot(l[1] - p[1]) > 1e-7) { poly.push(p); } }
        while poly.len() > 1 && (poly[0][0] - poly[poly.len() - 1][0]).hypot(poly[0][1] - poly[poly.len() - 1][1]) <= 1e-7 { poly.pop(); }
        if poly.len() < 3 { return; }
        let area: f64 = (0..poly.len()).map(|k| cross(poly[k], poly[(k + 1) % poly.len()])).sum::<f64>() / 2.0;
        if area > 1e-9 { self.out.push((edge, poly)); }
    }

    fn cycle(&self, from: usize) -> Vec<usize> {
        let mut c = vec![from];
        let mut i = self.v[from].next;
        while i != from && c.len() <= self.v.len() { c.push(i); i = self.v[i].next; }
        c
    }
}

/// The faces of the roof over the counter-clockwise polygon `ring`: each is the number of the edge it rises from and a
/// convex counter-clockwise polygon in plan. None when the polygon is not simple enough for this (too many corners, or
/// the walk does not end).
pub fn faces(ring: &[P]) -> Option<Vec<(usize, Vec<P>)>> {
    let n = ring.len();
    if !(3..=64).contains(&n) { return None; }
    let mut w = Walk { n: [[0.0; 2]; 64], d: [[0.0; 2]; 64], c: [0.0; 64], v: Vec::new(), out: Vec::new() };
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = dx.hypot(dy);
        if l < 1e-9 { return None; }
        w.d[i] = [dx / l, dy / l];
        w.n[i] = [-dy / l, dx / l];
        w.c[i] = dot(w.n[i], a);
    }
    for (i, &r) in ring.iter().enumerate().take(n) { w.corner(r, 0.0, (i + n - 1) % n, i, (i + n - 1) % n, (i + 1) % n); }
    let mut now = 0.0;
    for _ in 0..(8 * n + 16) {
        let alive: Vec<usize> = (0..w.v.len()).filter(|&i| w.v[i].alive).collect();
        if alive.is_empty() { break; }
        // the next event: (time, is_split, vertex, edge start vertex)
        let mut best: Option<(f64, bool, usize, usize)> = None;
        let consider = |t: f64, split: bool, a: usize, b: usize, best: &mut Option<(f64, bool, usize, usize)>| {
            let better = match best { None => true, Some((bt, bs, _, _)) => t < *bt - 1e-9 || (t <= *bt + 1e-9 && *bs && !split) };
            if better { *best = Some((t, split, a, b)); }
        };
        for &a in &alive {
            let b = w.v[a].next;
            // the edge a -> b shrinks away
            let e = w.v[a].er;
            let (pa, pb) = (w.at(a, now), w.at(b, now));
            let len = dot(w.d[e], sub(pb, pa));
            let rate = dot(w.d[e], sub(w.v[b].vel, w.v[a].vel));
            if rate < -1e-12 && len > -1e-7 { consider(now + len.max(0.0) / -rate, false, a, b, &mut best); }
        }
        for &v in &alive {
            // a reflex corner runs into an edge
            let (el, er) = (w.v[v].el, w.v[v].er);
            if cross(w.d[el], w.d[er]) >= -1e-9 { continue; }
            let cyc = w.cycle(v);
            let (pv, vel) = (w.v[v].p, w.v[v].vel);
            for &a in &cyc {
                let b = w.v[a].next;
                let e = w.v[a].er;
                if a == v || b == v { continue; }
                let nv = dot(w.n[e], vel);
                if nv >= 1.0 - 1e-12 { continue; }
                // n.(p + vel (t - t0)) = c + t
                let t = (w.c[e] - dot(w.n[e], pv) + nv * w.v[v].t0) / (nv - 1.0);
                if t < now - 1e-7 { continue; }
                let t = t.max(now);
                let hit = w.at(v, t);
                let (pa, pb) = (w.at(a, t), w.at(b, t));
                let along = dot(w.d[e], sub(hit, pa));
                let span = dot(w.d[e], sub(pb, pa));
                if along < -1e-7 || along > span + 1e-7 { continue; }
                consider(t, true, v, a, &mut best);
            }
        }
        let (t, split, x, y) = best?;
        now = t;
        if !split {
            // edge x -> y is gone: its ends become one corner
            let (a, b) = (x, y);
            let (pa, pb) = (w.v[a].prev, w.v[b].next);
            w.emit(a, b, t);
            if pa != b { w.emit(pa, a, t); }
            if pb != a && pb != pa { w.emit(b, pb, t); }
            let p = { let (u, v) = (w.at(a, t), w.at(b, t)); [(u[0] + v[0]) / 2.0, (u[1] + v[1]) / 2.0] };
            let (el, er) = (w.v[a].el, w.v[b].er);
            w.v[a].alive = false;
            w.v[b].alive = false;
            if pa == b || pb == a || pa == pb {
                // two corners or fewer remain: the cycle is closed
                for k in [pa, pb] { if k != a && k != b && w.v[k].alive { w.v[k].alive = false; } }
                continue;
            }
            let m = w.corner(p, t, el, er, pa, pb);
            w.v[pa].next = m;
            w.v[pb].prev = m;
            if w.cycle(m).len() <= 2 {
                let c = w.cycle(m);
                for k in 0..c.len() { let (u, v) = (c[k], c[(k + 1) % c.len()]); w.emit(u, v, t); }
                for k in c { w.v[k].alive = false; }
            }
        } else {
            // corner x runs into the edge y -> next(y)
            let v = x;
            let (a, b) = (y, w.v[y].next);
            let (pv, nv) = (w.v[v].prev, w.v[v].next);
            w.emit(pv, v, t);
            w.emit(v, nv, t);
            w.emit(a, b, t);
            let q = w.at(v, t);
            let (el, er, e) = (w.v[v].el, w.v[v].er, w.v[a].er);
            w.v[v].alive = false;
            let v1 = w.corner(q, t, el, e, pv, b);
            let v2 = w.corner(q, t, e, er, a, nv);
            w.v[pv].next = v1;
            w.v[b].prev = v1;
            w.v[a].next = v2;
            w.v[nv].prev = v2;
            for m in [v1, v2] {
                let c = w.cycle(m);
                if c.len() <= 2 {
                    for k in 0..c.len() { let (u, v) = (c[k], c[(k + 1) % c.len()]); w.emit(u, v, t); }
                    for k in c { w.v[k].alive = false; }
                }
            }
        }
    }
    if w.v.iter().any(|v| v.alive) { return None; }
    // the faces must tile the polygon
    let plan: f64 = (0..n).map(|i| cross(ring[i], ring[(i + 1) % n])).sum::<f64>() / 2.0;
    let got: f64 = w.out.iter().map(|(_, p)| (0..p.len()).map(|k| cross(p[k], p[(k + 1) % p.len()])).sum::<f64>() / 2.0).sum();
    if (plan - got).abs() > 1e-6 * plan.abs().max(1.0) { return None; }
    let _ = EPS;
    Some(w.out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(p: &[P]) -> f64 { (0..p.len()).map(|k| cross(p[k], p[(k + 1) % p.len()])).sum::<f64>() / 2.0 }
    /// The distance the outline has shrunk when it reaches q: the height of the roof at q, from the faces.
    fn height(f: &[(usize, Vec<P>)], ring: &[P], q: P) -> Option<f64> {
        let n = ring.len();
        for (e, poly) in f {
            let inside = (0..poly.len()).all(|k| cross(sub(poly[(k + 1) % poly.len()], poly[k]), sub(q, poly[k])) >= -1e-9);
            if inside {
                let (a, b) = (ring[*e], ring[(*e + 1) % n]);
                let l = (b[0] - a[0]).hypot(b[1] - a[1]);
                return Some(((b[1] - a[1]) * -(q[0] - a[0]) + (b[0] - a[0]) * (q[1] - a[1])) / l);
            }
        }
        None
    }

    #[test]
    fn a_rectangle_has_four_faces_and_a_ridge() {
        let r = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]];
        let f = faces(&r).unwrap();
        assert_eq!(f.len(), 4);
        assert!((f.iter().map(|(_, p)| area(p)).sum::<f64>() - 60.0).abs() < 1e-9);
        assert!((height(&f, &r, [5.0, 3.0]).unwrap() - 3.0).abs() < 1e-9);
        assert!((height(&f, &r, [1.0, 3.0]).unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn an_l_shape_has_a_valley_from_its_reflex_corner() {
        // arms 2 wide: the horizontal one runs 0..6, the vertical one 0..6
        let l = vec![[0.0, 0.0], [6.0, 0.0], [6.0, 2.0], [2.0, 2.0], [2.0, 6.0], [0.0, 6.0]];
        let f = faces(&l).unwrap();
        assert!((f.iter().map(|(_, p)| area(p)).sum::<f64>() - 20.0).abs() < 1e-9);
        for (_, p) in &f { assert!(area(p) > 0.0); for k in 0..p.len() { assert!(cross(sub(p[(k + 1) % p.len()], p[k]), sub(p[(k + 2) % p.len()], p[(k + 1) % p.len()])) >= -1e-9, "convex: {p:?}"); } }
        // the ridges of the arms are 1 m up; the valley meets them at the reflex corner's diagonal
        assert!((height(&f, &l, [4.0, 1.0]).unwrap() - 1.0).abs() < 1e-9);
        assert!((height(&f, &l, [1.0, 4.0]).unwrap() - 1.0).abs() < 1e-9);
        // the point (1.5, 1.5) is the corner of the valley: the outline reaches it after 0.5
        assert!((height(&f, &l, [1.5, 1.5]).unwrap() - 0.5).abs() < 1e-9, "{:?}", height(&f, &l, [1.5, 1.5]));
        // the union of two hip roofs, one on each arm, is the same roof
        let hip = |x: f64, y: f64, w: f64, d: f64| (x.min(w - x)).min(y.min(d - y));
        for q in [[0.5, 0.5], [3.0, 0.7], [1.2, 1.9], [0.8, 3.0], [1.0, 5.0], [5.0, 1.5], [1.9, 1.9]] {
            let want = hip(q[0], q[1], 6.0, 2.0).max(hip(q[0], q[1], 2.0, 6.0));
            assert!((height(&f, &l, q).unwrap() - want).abs() < 1e-9, "{q:?}: {:?} vs {want}", height(&f, &l, q));
        }
    }

    #[test]
    fn a_wide_l_with_different_arms_and_the_other_ways_round() {
        let l = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 4.0], [5.0, 4.0], [5.0, 9.0], [0.0, 9.0]];
        let f = faces(&l).unwrap();
        assert!((f.iter().map(|(_, p)| area(p)).sum::<f64>() - 65.0).abs() < 1e-9);
        let hip = |x: f64, y: f64, w: f64, d: f64| (x.min(w - x)).min(y.min(d - y));
        for q in [[1.0, 1.0], [2.5, 2.0], [4.0, 3.9], [7.0, 2.0], [2.5, 6.0], [4.9, 8.0], [4.0, 5.0]] {
            let want = hip(q[0], q[1], 10.0, 4.0).max(hip(q[0], q[1], 5.0, 9.0));
            assert!((height(&f, &l, q).unwrap() - want).abs() < 1e-9, "{q:?}: {:?} vs {want}", height(&f, &l, q));
        }
    }

    #[test]
    fn t_and_u_shapes_tile_their_plan_with_faces_that_are_convex() {
        let t = vec![[0.0, 0.0], [9.0, 0.0], [9.0, 3.0], [6.0, 3.0], [6.0, 8.0], [3.0, 8.0], [3.0, 3.0], [0.0, 3.0]];
        let u = vec![[0.0, 0.0], [9.0, 0.0], [9.0, 8.0], [6.0, 8.0], [6.0, 3.0], [3.0, 3.0], [3.0, 8.0], [0.0, 8.0]];
        for (ring, want) in [(t, 27.0 + 15.0), (u, 72.0 - 15.0)] {
            let f = faces(&ring).unwrap_or_else(|| panic!("no roof for {ring:?}"));
            assert!((f.iter().map(|(_, p)| area(p)).sum::<f64>() - want).abs() < 1e-6);
            for (_, p) in &f { for k in 0..p.len() { assert!(cross(sub(p[(k + 1) % p.len()], p[k]), sub(p[(k + 2) % p.len()], p[(k + 1) % p.len()])) >= -1e-9, "convex: {p:?}"); } }
        }
    }

    #[test]
    fn an_l_with_slanted_arms_is_roofed() {
        let l = vec![[0.0, 0.0], [7.0, 0.0], [7.0, 3.0], [3.5, 3.5], [3.0, 7.0], [0.0, 7.0]];
        let f = faces(&l).unwrap();
        let plan = area(&l);
        assert!((f.iter().map(|(_, p)| area(p)).sum::<f64>() - plan).abs() < 1e-6);
    }
    /// The height each face gives at `q`, from the plane of its edge.
    fn plane(ring: &[P], e: usize, q: P) -> f64 {
        let (a, b) = (ring[e], ring[(e + 1) % ring.len()]);
        let l = (b[0] - a[0]).hypot(b[1] - a[1]);
        (-(b[1] - a[1]) * (q[0] - a[0]) + (b[0] - a[0]) * (q[1] - a[1])) / l
    }

    #[test]
    fn the_faces_meet_at_the_same_height_wherever_they_touch() {
        let shapes: Vec<Vec<P>> = vec![
            vec![[0.0, 0.0], [6.0, 0.0], [6.0, 2.0], [2.0, 2.0], [2.0, 6.0], [0.0, 6.0]],
            vec![[0.0, 0.0], [9.0, 0.0], [9.0, 3.0], [6.0, 3.0], [6.0, 8.0], [3.0, 8.0], [3.0, 3.0], [0.0, 3.0]],
            vec![[0.0, 0.0], [9.0, 0.0], [9.0, 8.0], [6.0, 8.0], [6.0, 3.0], [3.0, 3.0], [3.0, 8.0], [0.0, 8.0]],
            vec![[0.0, 0.0], [7.0, 0.0], [7.0, 3.0], [3.5, 3.5], [3.0, 7.0], [0.0, 7.0]],
            // a plus, a Z and a stepped shape
            vec![[3.0, 0.0], [6.0, 0.0], [6.0, 3.0], [9.0, 3.0], [9.0, 6.0], [6.0, 6.0], [6.0, 9.0], [3.0, 9.0], [3.0, 6.0], [0.0, 6.0], [0.0, 3.0], [3.0, 3.0]],
            vec![[0.0, 0.0], [8.0, 0.0], [8.0, 3.0], [11.0, 3.0], [11.0, 6.0], [3.0, 6.0], [3.0, 3.0], [0.0, 3.0]],
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 3.0], [7.0, 3.0], [7.0, 6.0], [4.0, 6.0], [4.0, 9.0], [0.0, 9.0]],
        ];
        for (k, ring) in shapes.iter().enumerate() {
            let f = faces(ring).unwrap_or_else(|| panic!("shape {k}: no roof"));
            assert!((f.iter().map(|(_, p)| area(p)).sum::<f64>() - area(ring)).abs() < 1e-6, "shape {k}");
            for (e, poly) in &f {
                for q in poly {
                    let h = plane(ring, *e, *q);
                    assert!(h > -1e-6, "shape {k}: below the eaves {h}");
                    for (e2, poly2) in &f {
                        if e2 == e { continue; }
                        // q on the boundary of the other face
                        let on = (0..poly2.len()).any(|j| {
                            let (a, b) = (poly2[j], poly2[(j + 1) % poly2.len()]);
                            let l = (b[0] - a[0]).hypot(b[1] - a[1]);
                            let t = ((q[0] - a[0]) * (b[0] - a[0]) + (q[1] - a[1]) * (b[1] - a[1])) / (l * l);
                            cross(sub(b, a), sub(*q, a)).abs() / l < 1e-7 && (-1e-9..=1.0 + 1e-9).contains(&t)
                        });
                        if on { assert!((plane(ring, *e2, *q) - h).abs() < 1e-6, "shape {k}: faces {e} and {e2} differ at {q:?}: {h} vs {}", plane(ring, *e2, *q)); }
                    }
                }
            }
        }
    }

}
