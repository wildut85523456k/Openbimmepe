//! Section views. A section is a vertical cut along a line `a -> b`; the view looks towards the left of the line,
//! so `u` (distance along the line from `a`) grows to the right of the drawing and `z` up. Shapes are in (u, z).
//!
//! Walls, slabs and columns the plane cuts are *cut* (filled regions with a heavy outline; openings and shaft voids
//! are gaps). Walls that lie in front of the plane within the view depth are seen from the side as *beyond*
//! outlines, doors and windows in them as rectangles. There is no hidden-line removal yet: outlines of walls that
//! hide each other all show. Levels in range are dashed datum lines with their names.
//!
//! Known limit, reported in `warnings`: a wall with a sloped top is drawn with its flat height, and an arched
//! opening in a wall that is only seen beyond the plane is left out.
use crate::draw::{page, SvgStyle};
use crate::{bounds_of, subtract, Class, Element, Item, Kind, Shape, Spans, ViewError, EPS};
use openbim_geom::WallChain;
use openbim_ifc::Scene;
use rayon::prelude::*;

type P = [f64; 2];

/// The section line and the heights of the view, in absolute metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Section {
    pub a: P,
    pub b: P,
    /// the lowest and highest heights drawn
    pub bottom: f64,
    pub top: f64,
    /// how far beyond the plane (looking left of `a -> b`) things are still drawn
    pub depth: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub at: P,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SectionView {
    pub section: Section,
    /// length of the section line
    pub length: f64,
    pub items: Vec<Item>,
    /// level names, at the right end of their datum lines
    pub labels: Vec<Label>,
    pub warnings: Vec<String>,
}

impl SectionView {
    /// `[min_u, min_z, max_u, max_z]`, always including the whole section line.
    pub fn bounds(&self) -> [f64; 4] {
        let b = bounds_of(&self.items).unwrap_or([0.0, self.section.bottom, self.length, self.section.top]);
        [b[0].min(0.0), b[1], b[2].max(self.length), b[3]]
    }
}

struct Fr {
    a: P,
    d: P,
    n: P,
    len: f64,
}

impl Fr {
    /// (u, w): along the line, and distance from the plane towards the viewing side
    fn uw(&self, p: P) -> (f64, f64) {
        let v = [p[0] - self.a[0], p[1] - self.a[1]];
        (v[0] * self.d[0] + v[1] * self.d[1], v[0] * self.n[0] + v[1] * self.n[1])
    }
}

pub fn section_view(scene: &Scene, sec: Section) -> Result<SectionView, ViewError> {
    if !sec.a.iter().chain(sec.b.iter()).chain([sec.bottom, sec.top, sec.depth].iter()).all(|v| v.is_finite()) { return Err(ViewError::BadSection("a number is not finite")); }
    let len = (sec.b[0] - sec.a[0]).hypot(sec.b[1] - sec.a[1]);
    if len < 1e-6 { return Err(ViewError::BadSection("the line has no length")); }
    if sec.bottom >= sec.top { return Err(ViewError::BadSection("bottom must be below top")); }
    if sec.depth < 0.0 { return Err(ViewError::BadSection("depth cannot be negative")); }
    let d = [(sec.b[0] - sec.a[0]) / len, (sec.b[1] - sec.a[1]) / len];
    let fr = Fr { a: sec.a, d, n: [-d[1], d[0]], len };
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    // slabs first, then walls and columns, so that a painter has the walls on top
    for (k, sl) in scene.slabs.iter().enumerate() {
        let z0 = (sl.slab.top - sl.slab.thickness).max(sec.bottom);
        let z1 = sl.slab.top.min(sec.top);
        if z1 - z0 <= EPS { continue; }
        let mut spans = ring_spans(&sl.slab.outline, &fr);
        let mut voids: Vec<Vec<P>> = scene.shafts.iter().filter(|sh| sl.slab.top > sh.z_bottom + 1e-6 && sl.slab.top < sh.z_top - 1e-6).map(|sh| sh.outline.clone()).collect();
        for p in scene.floor_penetrations.iter().filter(|p| p.id.slab == k && p.fits && p.kind == openbim_place::floors::FloorOpeningKind::Cut) {
            voids.push(vec![[p.x0, p.y0], [p.x1, p.y0], [p.x1, p.y1], [p.x0, p.y1]]);
        }
        for v in &voids { for (a, b) in ring_spans(v, &fr) { spans = subtract(spans, a, b); } }
        for (a, b) in spans { rect_item(&mut items, Element { kind: Kind::Slab, id: sl.id }, a, b, z0, z1, len); }
    }
    let walls: Vec<Result<WallDraw, ViewError>> = scene.walls.par_iter().map(|w| wall_items(w.id, &w.chain, &fr, &sec)).collect();
    for (w, r) in scene.walls.iter().zip(walls) {
        let (its, sloped, skipped) = r?;
        if sloped && !its.is_empty() { warnings.push(format!("wall {}: the sloped top is drawn flat at the wall height", w.id)); }
        if skipped { warnings.push(format!("wall {}: an arched opening seen beyond the plane is not drawn", w.id)); }
        items.extend(its);
    }
    for c in &scene.columns {
        let z0 = c.base_z.max(sec.bottom);
        let z1 = (c.base_z + c.height).min(sec.top);
        if z1 - z0 <= EPS { continue; }
        for (a, b) in ring_spans(&c.footprint, &fr) { rect_item(&mut items, Element { kind: Kind::Column, id: c.id }, a, b, z0, z1, len); }
    }
    let mut labels = Vec::new();
    for (i, lv) in scene.levels.all().iter().enumerate() {
        if lv.elevation < sec.bottom || lv.elevation > sec.top { continue; }
        items.push(Item { element: Element { kind: Kind::Level, id: i as u64 }, class: Class::Datum, shape: Shape::Line([0.0, lv.elevation], [len, lv.elevation]) });
        labels.push(Label { at: [len, lv.elevation], text: lv.name.clone() });
    }
    Ok(SectionView { section: sec, length: len, items, labels, warnings })
}

/// Pairs of `u` between which the line is inside the ring (even-odd), clipped to the line.
fn ring_spans(ring: &[P], fr: &Fr) -> Spans {
    let mut us: Vec<f64> = Vec::new();
    for k in 0..ring.len() {
        let ((ua, wa), (ub, wb)) = (fr.uw(ring[k]), fr.uw(ring[(k + 1) % ring.len()]));
        if (wa < 0.0) != (wb < 0.0) { us.push(ua + (0.0 - wa) / (wb - wa) * (ub - ua)); }
    }
    us.sort_by(|a, b| a.total_cmp(b));
    us.as_chunks::<2>().0.iter().filter_map(|c| { let (a, b) = (c[0].max(0.0), c[1].min(fr.len)); if b - a > EPS { Some((a, b)) } else { None } }).collect()
}

fn rect_ring(u0: f64, u1: f64, z0: f64, z1: f64) -> Vec<P> { vec![[u0, z0], [u1, z0], [u1, z1], [u0, z1]] }

fn ring_lines(items: &mut Vec<Item>, el: Element, class: Class, ring: &[P]) {
    for k in 0..ring.len() { items.push(Item { element: el, class, shape: Shape::Line(ring[k], ring[(k + 1) % ring.len()]) }); }
}

/// A solid cut piece with no holes.
fn rect_item(items: &mut Vec<Item>, el: Element, u0: f64, u1: f64, z0: f64, z1: f64, _len: f64) {
    let ring = rect_ring(u0, u1, z0, z1);
    items.push(Item { element: el, class: Class::Cut, shape: Shape::Region { outer: ring.clone(), holes: vec![] } });
    ring_lines(items, el, Class::Cut, &ring);
}

/// Keep the part of a polygon inside `u0..u1` and `z0..z1` (Sutherland-Hodgman, good for simple polygons).
fn clip_box(poly: &[P], u0: f64, u1: f64, z0: f64, z1: f64) -> Vec<P> {
    let mut p: Vec<P> = poly.to_vec();
    // (axis, limit, keep greater)
    for (ax, lim, ge) in [(0usize, u0, true), (0, u1, false), (1, z0, true), (1, z1, false)] {
        let inn = |q: P| if ge { q[ax] >= lim } else { q[ax] <= lim };
        let mut out: Vec<P> = Vec::new();
        for k in 0..p.len() {
            let (a, b) = (p[k], p[(k + 1) % p.len()]);
            let cross = |a: P, b: P| { let t = (lim - a[ax]) / (b[ax] - a[ax]); [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])] };
            match (inn(a), inn(b)) {
                (true, true) => out.push(b),
                (true, false) => out.push(cross(a, b)),
                (false, true) => { out.push(cross(a, b)); out.push(b); }
                (false, false) => {}
            }
        }
        p = out;
        if p.is_empty() { break; }
    }
    p
}

/// What one wall gives: its items, whether it has a sloped top, whether an arched opening was left out.
type WallDraw = (Vec<Item>, bool, bool);

/// Items of a wall, whether it has a sloped top, and whether an arched opening was left out.
fn wall_items(id: u64, c: &WallChain, fr: &Fr, sec: &Section) -> Result<WallDraw, ViewError> {
    let top = c.base_z + c.height;
    let (z0, z1) = (c.base_z.max(sec.bottom), top.min(sec.top));
    let sloped = c.tops.iter().any(|t| t.is_some());
    if z1 - z0 <= EPS { return Ok((Vec::new(), false, false)); }
    let mut skipped = false;
    let plans = c.plans().map_err(|err| ViewError::Wall { id, err })?;
    let el = Element { kind: Kind::Wall, id };
    let mut out = Vec::new();
    for (i, p) in plans.iter().enumerate() {
        let quad = [p.l0, p.l1, p.r1, p.r0];
        let uw: Vec<(f64, f64)> = quad.iter().map(|&q| fr.uw(q)).collect();
        let (wmin, wmax) = uw.iter().fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x.1), hi.max(x.1)));
        let (umin, umax) = uw.iter().fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x.0), hi.max(x.0)));
        // along the wall: s of the point of the section line at u is s0 + k u
        let t = { let v = [p.l1[0] - p.l0[0], p.l1[1] - p.l0[1]]; let l = v[0].hypot(v[1]); [v[0] / l, v[1] / l] };
        let k = fr.d[0] * t[0] + fr.d[1] * t[1];
        let s0 = p.s_l0 + (fr.a[0] - p.l0[0]) * t[0] + (fr.a[1] - p.l0[1]) * t[1];
        // the u of the section line where the wall has `s`, for the segment's openings
        let u_range = |sa: f64, sb: f64, lo: f64, hi: f64| -> Option<(f64, f64)> {
            let (a, b) = if k.abs() > 1e-9 { let (x, y) = ((sa - s0) / k, (sb - s0) / k); (x.min(y), x.max(y)) } else if s0 > sa && s0 < sb { (lo, hi) } else { return None };
            let (a, b) = (a.max(lo), b.min(hi));
            if b - a > EPS { Some((a, b)) } else { None }
        };
        if wmin < 0.0 && wmax > 0.0 {
            // cut: where the line is inside the quad
            let mut us: Vec<f64> = Vec::new();
            for e in 0..4 {
                let ((ua, wa), (ub, wb)) = (uw[e], uw[(e + 1) % 4]);
                if (wa < 0.0) != (wb < 0.0) { us.push(ua + (0.0 - wa) / (wb - wa) * (ub - ua)); }
            }
            let (lo, hi) = us.iter().fold((f64::MAX, f64::MIN), |(l, h), &x| (l.min(x), h.max(x)));
            let (ua, ub) = (lo.max(0.0), hi.min(fr.len));
            if ub - ua <= EPS { continue; }
            let mut holes: Vec<Vec<P>> = Vec::new();
            for o in c.openings.get(i).into_iter().flatten() {
                let (zl, zh) = ((c.base_z + o.sill).max(z0), (c.base_z + o.sill + o.height).min(z1));
                if zh - zl <= EPS { continue; }
                if let Some((a, b)) = u_range(o.offset, o.offset + o.width, ua, ub) { holes.push(rect_ring(a, b, zl, zh)); }
            }
            for poly in c.poly_openings.get(i).into_iter().flatten() {
                if k.abs() > 1e-9 {
                    let ring: Vec<P> = poly.iter().map(|q| [(q[0] - s0) / k, c.base_z + q[1]]).collect();
                    let r = clip_box(&ring, ua, ub, z0, z1);
                    if r.len() >= 3 { holes.push(r); }
                } else {
                    // the section crosses the wall square on: the opening is a gap over the height the polygon has at s0
                    let mut zs: Vec<f64> = Vec::new();
                    for e in 0..poly.len() {
                        let (a, b) = (poly[e], poly[(e + 1) % poly.len()]);
                        if (a[0] <= s0) != (b[0] <= s0) { zs.push(c.base_z + a[1] + (s0 - a[0]) / (b[0] - a[0]) * (b[1] - a[1])); }
                    }
                    zs.sort_by(|a, b| a.total_cmp(b));
                    for pair in zs.as_chunks::<2>().0 {
                        let (zl, zh) = (pair[0].max(z0), pair[1].min(z1));
                        if zh - zl > EPS { holes.push(rect_ring(ua, ub, zl, zh)); }
                    }
                }
            }
            let outer = rect_ring(ua, ub, z0, z1);
            out.push(Item { element: el, class: Class::Cut, shape: Shape::Region { outer: outer.clone(), holes: holes.clone() } });
            ring_lines(&mut out, el, Class::Cut, &outer);
            for h in &holes { ring_lines(&mut out, el, Class::Cut, h); }
        } else if wmin >= 0.0 && wmin <= sec.depth {
            // beyond: the wall seen from the side
            let (ua, ub) = (umin.max(0.0), umax.min(fr.len));
            if ub - ua <= EPS { continue; }
            ring_lines(&mut out, el, Class::Beyond, &rect_ring(ua, ub, z0, z1));
            if c.poly_openings.get(i).is_some_and(|v| !v.is_empty()) { skipped = true; }
            if k.abs() > 1e-9 {
                for o in c.openings.get(i).into_iter().flatten() {
                    let (zl, zh) = ((c.base_z + o.sill).max(z0), (c.base_z + o.sill + o.height).min(z1));
                    if zh - zl <= EPS { continue; }
                    if let Some((a, b)) = u_range(o.offset, o.offset + o.width, ua, ub) { ring_lines(&mut out, el, Class::Beyond, &rect_ring(a, b, zl, zh)); }
                }
            }
        }
    }
    Ok((out, sloped, skipped))
}

/// A section drawn in the Revit look.
pub fn section_svg(v: &SectionView) -> String { section_svg_with(v, &SvgStyle::revit()) }

pub fn section_svg_with(v: &SectionView, st: &SvgStyle) -> String {
    let b = v.bounds();
    page("section", [b[0], b[1], b[2] + 1.2, b[3]], &v.items, &[], &v.labels, st)
}
