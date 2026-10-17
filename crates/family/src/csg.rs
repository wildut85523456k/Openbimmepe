//! Boolean difference of two closed triangle meshes with a binary space partition (the method of csg.js).
//! It is used to cut void forms out of the solids of a family; meshes here have a few hundred triangles.

use crate::meshing::{cross, dot, sub, TriMesh};
use std::collections::HashMap;

type V = [f64; 3];
const EPS: f64 = 1e-7;
/// More than this many triangles in both meshes together and the cut is not attempted.
const MAX_TRIS: usize = 20_000;

#[derive(Clone)]
struct Plane { n: V, w: f64 }

#[derive(Clone)]
struct Poly { v: Vec<V>, plane: Plane }

impl Plane {
    fn of(a: V, b: V, c: V) -> Option<Plane> {
        let n = cross(sub(b, a), sub(c, a));
        let l = dot(n, n).sqrt();
        if l < 1e-12 { return None; }
        let n = [n[0] / l, n[1] / l, n[2] / l];
        Some(Plane { n, w: dot(n, a) })
    }
    fn flip(&mut self) { self.n = [-self.n[0], -self.n[1], -self.n[2]]; self.w = -self.w; }

    /// Cut `poly` by this plane into the four heaps.
    fn split(&self, poly: &Poly, coplanar_front: &mut Vec<Poly>, coplanar_back: &mut Vec<Poly>, front: &mut Vec<Poly>, back: &mut Vec<Poly>) {
        const FRONT: u8 = 1;
        const BACK: u8 = 2;
        let mut all = 0u8;
        let kinds: Vec<u8> = poly.v.iter().map(|v| {
            let t = dot(self.n, *v) - self.w;
            let k = if t < -EPS { BACK } else if t > EPS { FRONT } else { 0 };
            all |= k;
            k
        }).collect();
        match all {
            0 => if dot(self.n, poly.plane.n) > 0.0 { coplanar_front.push(poly.clone()) } else { coplanar_back.push(poly.clone()) },
            FRONT => front.push(poly.clone()),
            BACK => back.push(poly.clone()),
            _ => {
                let (mut f, mut b): (Vec<V>, Vec<V>) = (Vec::new(), Vec::new());
                let n = poly.v.len();
                for i in 0..n {
                    let j = (i + 1) % n;
                    let (ti, tj, vi, vj) = (kinds[i], kinds[j], poly.v[i], poly.v[j]);
                    if ti != BACK { f.push(vi); }
                    if ti != FRONT { b.push(vi); }
                    if (ti | tj) == (FRONT | BACK) {
                        let t = (self.w - dot(self.n, vi)) / dot(self.n, sub(vj, vi));
                        let p = [vi[0] + (vj[0] - vi[0]) * t, vi[1] + (vj[1] - vi[1]) * t, vi[2] + (vj[2] - vi[2]) * t];
                        f.push(p);
                        b.push(p);
                    }
                }
                if f.len() >= 3 { front.push(Poly { v: f, plane: poly.plane.clone() }); }
                if b.len() >= 3 { back.push(Poly { v: b, plane: poly.plane.clone() }); }
            }
        }
    }
}

#[derive(Default)]
struct Node {
    plane: Option<Plane>,
    front: Option<Box<Node>>,
    back: Option<Box<Node>>,
    polys: Vec<Poly>,
}

impl Node {
    fn from(polys: Vec<Poly>) -> Node { let mut n = Node::default(); n.build(polys); n }

    fn invert(&mut self) {
        for p in &mut self.polys { p.v.reverse(); p.plane.flip(); }
        if let Some(pl) = &mut self.plane { pl.flip(); }
        if let Some(f) = &mut self.front { f.invert(); }
        if let Some(b) = &mut self.back { b.invert(); }
        std::mem::swap(&mut self.front, &mut self.back);
    }

    fn clip_polys(&self, polys: &[Poly]) -> Vec<Poly> {
        let Some(plane) = &self.plane else { return polys.to_vec() };
        let (mut front, mut back) = (Vec::new(), Vec::new());
        for p in polys {
            let (mut cf, mut cb) = (Vec::new(), Vec::new());
            plane.split(p, &mut cf, &mut cb, &mut front, &mut back);
            front.append(&mut cf);
            back.append(&mut cb);
        }
        let mut out = match &self.front { Some(f) => f.clip_polys(&front), None => front };
        if let Some(b) = &self.back { out.extend(b.clip_polys(&back)); }
        out
    }

    fn clip_to(&mut self, other: &Node) {
        self.polys = other.clip_polys(&self.polys);
        if let Some(f) = &mut self.front { f.clip_to(other); }
        if let Some(b) = &mut self.back { b.clip_to(other); }
    }

    fn all(&self, out: &mut Vec<Poly>) {
        out.extend(self.polys.iter().cloned());
        if let Some(f) = &self.front { f.all(out); }
        if let Some(b) = &self.back { b.all(out); }
    }

    fn build(&mut self, polys: Vec<Poly>) {
        if polys.is_empty() { return; }
        let plane = self.plane.get_or_insert_with(|| polys[0].plane.clone()).clone();
        let (mut front, mut back) = (Vec::new(), Vec::new());
        for p in &polys {
            let (mut cf, mut cb) = (Vec::new(), Vec::new());
            plane.split(p, &mut cf, &mut cb, &mut front, &mut back);
            self.polys.append(&mut cf);
            self.polys.append(&mut cb);
        }
        if !front.is_empty() { self.front.get_or_insert_with(Default::default).build(front); }
        if !back.is_empty() { self.back.get_or_insert_with(Default::default).build(back); }
    }
}

fn polys_of(m: &TriMesh) -> Vec<Poly> {
    m.tris.iter().filter_map(|t| {
        let v = [m.verts[t[0] as usize], m.verts[t[1] as usize], m.verts[t[2] as usize]];
        Plane::of(v[0], v[1], v[2]).map(|plane| Poly { v: v.to_vec(), plane })
    }).collect()
}

fn mesh_of(polys: &[Poly]) -> TriMesh {
    let mut out = TriMesh::default();
    let mut index: HashMap<(i64, i64, i64), u32> = HashMap::new();
    let mut id = |p: V, out: &mut TriMesh| -> u32 {
        let key = ((p[0] * 1e8).round() as i64, (p[1] * 1e8).round() as i64, (p[2] * 1e8).round() as i64);
        *index.entry(key).or_insert_with(|| { out.verts.push(p); out.verts.len() as u32 - 1 })
    };
    for p in polys {
        let ids: Vec<u32> = p.v.iter().map(|&q| id(q, &mut out)).collect();
        for k in 1..ids.len() - 1 {
            let t = [ids[0], ids[k], ids[k + 1]];
            if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] { out.tris.push(t); }
        }
    }
    out
}

fn boxes_overlap(a: &TriMesh, b: &TriMesh) -> bool {
    match (a.bounds(), b.bounds()) {
        (Some((al, ah)), Some((bl, bh))) => (0..3).all(|k| al[k] < bh[k] + EPS && bl[k] < ah[k] + EPS),
        _ => false,
    }
}

/// `a` with `b` cut out of it. Both are closed meshes with outward triangles. None if the meshes are too big to try.
pub(crate) fn difference(a: &TriMesh, b: &TriMesh) -> Option<TriMesh> {
    if !boxes_overlap(a, b) { return Some(a.clone()); }
    if a.tris.len() + b.tris.len() > MAX_TRIS { return None; }
    // the recursion follows the depth of the tree, so give it a big stack
    std::thread::scope(|s| {
        std::thread::Builder::new().stack_size(64 << 20).spawn_scoped(s, || {
            let mut na = Node::from(polys_of(a));
            let mut nb = Node::from(polys_of(b));
            na.invert();
            na.clip_to(&nb);
            nb.clip_to(&na);
            nb.invert();
            nb.clip_to(&na);
            nb.invert();
            let mut rest = Vec::new();
            nb.all(&mut rest);
            na.build(rest);
            na.invert();
            let mut all = Vec::new();
            na.all(&mut all);
            mesh_of(&all)
        }).ok()?.join().ok()
    })
}
