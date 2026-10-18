//! Metre-based definitions shared by plan and 3D views. Meshes are derived,
//! never a second mutable copy of the document's parameters.
use super::{Category, Document, EditError, Element, ElementId};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Geometry {
    Level,
    Grid { start: [f64; 2], end: [f64; 2] },
    Linear { start: [f64; 2], end: [f64; 2], level: ElementId },
    /// a point on `level` with something standing from there up to the level `top` (a column)
    Point { at: [f64; 2], level: ElementId, top: ElementId },
    /// a family instance standing at a point of a level (turned by the element's angle)
    Placed { at: [f64; 2], level: ElementId },
    /// a family instance in a wall, `along_m` from its start (a door or a window)
    InWall { host: ElementId, along_m: f64 },
    /// reinforcement placed in a beam
    OnMember { host: ElementId },
    /// a slab lies on `level`; its outline is the element's
    Area { level: ElementId },
    /// a pad footing centred on `at`, `thickness_m` thick
    Pad { at: [f64; 2], level: ElementId, thickness_m: f64 },
    /// a strip footing hosted by the wall `host`, along its axis
    OnWall { host: ElementId },
    /// a tag: its label at `at` in the plan, with a leader to `anchor`, on the element `host`
    Tag { host: ElementId, at: [f64; 2], anchor: [f64; 2] },
    /// an aligned dimension on `level`: the distance between the points `a` and `b`, its line through `at`
    Dimension { a: [f64; 2], b: [f64; 2], at: [f64; 2], level: ElementId },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<[f64; 3]>,
    pub faces: Vec<Vec<usize>>,
}

fn valid_axis(start: [f64; 2], end: [f64; 2]) -> bool {
    start.into_iter().chain(end).all(|n| n.is_finite() && n.abs() <= 1e6)
        && (end[0] - start[0]).hypot(end[1] - start[1]) > 1e-6
}

/// The axes of the beams of a system: lines `spacing` apart at `angle_deg` from the x axis, centred on the outline
/// and cut to it, so that a concave outline gives several pieces on a line. Pieces shorter than 50 mm are dropped.
pub fn system_lines(outline: &[[f64; 2]], angle_deg: f64, spacing: f64) -> Vec<([f64; 2], [f64; 2])> {
    if outline.len() < 3 || spacing.is_nan() || spacing <= 0.0 || spacing.is_infinite() { return Vec::new(); }
    let a = angle_deg.to_radians();
    let (d, n) = ([a.cos(), a.sin()], [-a.sin(), a.cos()]);
    let across = |p: [f64; 2]| p[0] * n[0] + p[1] * n[1];
    let along = |p: [f64; 2]| p[0] * d[0] + p[1] * d[1];
    let (lo, hi) = outline.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), p| (l.min(across(*p)), h.max(across(*p))));
    let count = (((hi - lo) / spacing) + 1e-9).floor().max(1.0) as usize;
    if count > 2000 { return vec![([0.0; 2], [0.0; 2]); 2001]; }
    let mid = (lo + hi) / 2.0;
    let mut out = Vec::new();
    for k in 0..count {
        let o = mid + (k as f64 - (count as f64 - 1.0) / 2.0) * spacing;
        let mut ts = Vec::new();
        for i in 0..outline.len() {
            let (p, q) = (outline[i], outline[(i + 1) % outline.len()]);
            let (sp, sq) = (across(p) - o, across(q) - o);
            if (sp > 0.0) != (sq > 0.0) {
                let t = sp / (sp - sq);
                ts.push(along([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]));
            }
        }
        ts.sort_by(f64::total_cmp);
        for pair in ts.chunks_exact(2) {
            if pair[1] - pair[0] < 0.05 { continue; }
            let at = |s: f64| [o * n[0] + s * d[0], o * n[1] + s * d[1]];
            out.push((at(pair[0]), at(pair[1])));
        }
    }
    out
}

/// A beam's axis after its joins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamExtent {
    pub a: [f64; 3],
    pub b: [f64; 3],
    /// how much of the axis is cut off at the start and at the end
    pub cut_start: f64,
    pub cut_end: f64,
    /// the length of the whole axis, before the cuts
    pub full_length: f64,
}

impl BeamExtent {
    /// the length that is left
    pub fn cut_length(&self) -> f64 { self.full_length - self.cut_start - self.cut_end }
}

/// Whether `p` is inside the polygon or on its edge.
fn inside(ring: &[[f64; 2]], p: [f64; 2]) -> bool {
    let n = ring.len();
    let mut c = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (ring[i], ring[j]);
        let cr = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        if cr.abs() < 1e-12 && p[0] >= a[0].min(b[0]) - 1e-12 && p[0] <= a[0].max(b[0]) + 1e-12 && p[1] >= a[1].min(b[1]) - 1e-12 && p[1] <= a[1].max(b[1]) + 1e-12 { return true; }
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] { c = !c; }
        j = i;
    }
    c
}

/// Where the segment `q -> p` meets the segment `c -> d`: the fraction along `q -> p`, if they meet.
fn segment_param(q: [f64; 2], p: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Option<f64> {
    let (r, s) = ([p[0] - q[0], p[1] - q[1]], [d[0] - c[0], d[1] - c[1]]);
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-14 { return None; }
    let qc = [c[0] - q[0], c[1] - q[1]];
    let t = (qc[0] * s[1] - qc[1] * s[0]) / den;
    let u = (qc[0] * r[1] - qc[1] * r[0]) / den;
    ((-1e-9..=1.0 + 1e-9).contains(&t) && (-1e-9..=1.0 + 1e-9).contains(&u)).then_some(t.clamp(0.0, 1.0))
}

/// A simple polygon: 3 to 1000 finite corners, some area, and no edge crossing another.
fn valid_ring(r: &[[f64; 2]]) -> bool {
    let n = r.len();
    if !(3..=1000).contains(&n) || r.iter().any(|p| !p[0].is_finite() || !p[1].is_finite() || p[0].abs() > 1e6 || p[1].abs() > 1e6) {
        return false;
    }
    if openbim_geom::profile::polygon_area(r).abs() < 1e-6 {
        return false;
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let on = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| {
        cross(a, b, p).abs() < 1e-12 && p[0] >= a[0].min(b[0]) - 1e-12 && p[0] <= a[0].max(b[0]) + 1e-12 && p[1] >= a[1].min(b[1]) - 1e-12 && p[1] <= a[1].max(b[1]) + 1e-12
    };
    // non-adjacent edges may neither cross nor touch
    let hit = |a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]| {
        let (d1, d2, d3, d4) = (cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d));
        (d1 * d2 < 0.0 && d3 * d4 < 0.0) || on(c, d, a) || on(c, d, b) || on(a, b, c) || on(a, b, d)
    };
    for i in 0..n {
        for j in (i + 2)..n {
            if i == 0 && j == n - 1 { continue; }
            if hit(r[i], r[(i + 1) % n], r[j], r[(j + 1) % n]) { return false; }
        }
    }
    true
}

pub(super) fn validate(elements: &BTreeMap<ElementId, Element>) -> Result<(), EditError> {
    let mut names = std::collections::BTreeSet::new();
    for el in elements.values() {
        // a type is named once among the types of its kind
        if el.category == Category::Type && !names.insert((el.type_for.map(|c| c as u8), el.name.trim().to_lowercase())) { return Err(EditError::InvalidElement); }
        // an element with a type has the values of that type, and the type is of its kind
        if let Some(t) = el.type_id {
            let ok = elements.get(&t).is_some_and(|ty| ty.category == Category::Type && ty.type_for == Some(el.category)
                && crate::TypeValues::of(el) == crate::TypeValues::of(ty));
            if !ok { return Err(EditError::InvalidGeometry(el.id)); }
        }
        if let Some(p) = el.parent {
            // a beam made by a beam system points at it
            if !(el.category == Category::Beam && elements.get(&p).is_some_and(|s| matches!(s.category, Category::BeamSystem | Category::Truss))) { return Err(EditError::InvalidElement); }
        }
        let valid = match &el.geometry {
            None => !matches!(el.category, Category::Level | Category::Grid | Category::Column | Category::Slab | Category::Footing | Category::WallFooting | Category::BeamSystem | Category::Truss | Category::Rebar | Category::Family),
            Some(Geometry::Level) => el.category == Category::Level && el.elevation_m.abs() <= 1e6,
            Some(Geometry::Grid { start, end }) => el.category == Category::Grid && valid_axis(*start, *end),
            Some(Geometry::OnWall { host }) => {
                el.category == Category::WallFooting
                    && elements.get(host).is_some_and(|w| w.category == Category::Wall && matches!(w.geometry, Some(Geometry::Linear { .. })))
            }
            Some(Geometry::Tag { host, at, anchor }) => {
                el.category == Category::Tag && at.iter().chain(anchor.iter()).all(|v| v.is_finite() && v.abs() <= 1e6)
                    && elements.get(host).is_some_and(|h| !matches!(h.category, Category::Tag | Category::Level | Category::Grid | Category::Type))
            }
            Some(Geometry::Dimension { a, b, at, level }) => {
                el.category == Category::Dimension && valid_axis(*a, *b) && at.iter().all(|v| v.is_finite() && v.abs() <= 1e6)
                    && elements.get(level).is_some_and(|l| l.category == Category::Level && l.geometry == Some(Geometry::Level))
            }
            Some(Geometry::Placed { at, level }) => {
                el.category == Category::Family && !el.family.is_empty() && at.iter().all(|v| v.is_finite() && v.abs() <= 1e6) && el.elevation_m.abs() <= 1e6
                    && elements.get(level).is_some_and(|l| l.category == Category::Level && l.geometry == Some(Geometry::Level))
            }
            Some(Geometry::InWall { host, along_m }) => {
                el.category == Category::Family && !el.family.is_empty() && el.elevation_m.abs() <= 1e6
                    && elements.get(host).is_some_and(|w| w.category == Category::Wall && match w.geometry {
                        Some(Geometry::Linear { start, end, .. }) => (0.0..=(end[0] - start[0]).hypot(end[1] - start[1])).contains(along_m),
                        _ => false,
                    })
            }
            Some(Geometry::OnMember { host }) => {
                let (dm, ds) = (el.width_m, el.height_m);
                el.category == Category::Rebar
                    && elements.get(host).is_some_and(|b| b.category == Category::Beam && b.section == crate::Section::Rectangle)
                    && (0.004..=0.05).contains(&dm) && (0.004..=0.05).contains(&ds) && (0.01..=0.1).contains(&el.elevation_m)
                    && (0.05..=1.0).contains(&el.spacing_m) && (2..=20).contains(&el.count)
            }
            Some(Geometry::Pad { at, level, thickness_m }) => {
                el.category == Category::Footing && at.iter().all(|v| v.is_finite() && v.abs() <= 1e6)
                    && thickness_m.is_finite() && *thickness_m > 0.0 && *thickness_m <= 5.0
                    && elements.get(level).is_some_and(|l| l.category == Category::Level && l.geometry == Some(Geometry::Level))
            }
            Some(Geometry::Area { level }) => {
                matches!(el.category, Category::Slab | Category::BeamSystem) && valid_ring(&el.outline)
                    && (el.category != Category::BeamSystem || (0.05..=1000.0).contains(&el.spacing_m))
                    && elements.get(level).is_some_and(|l| l.category == Category::Level && l.geometry == Some(Geometry::Level))
                    && el.elevation_m.abs() <= 1e6
            }
            Some(Geometry::Point { at, level, top }) => {
                let lv = |id: &ElementId| elements.get(id).filter(|l| l.category == Category::Level && l.geometry == Some(Geometry::Level));
                el.category == Category::Column
                    && at.iter().all(|n| n.is_finite() && n.abs() <= 1e6)
                    && el.width_m <= 1e6 && el.height_m <= 1e6
                    && match (lv(level), lv(top)) {
                        (Some(a), Some(b)) => b.elevation_m - (a.elevation_m + el.elevation_m) > 1e-6 && el.elevation_m.abs() <= 1e6,
                        _ => false,
                    }
            }
            Some(Geometry::Linear { start, end, level }) => {
                let datum = elements.get(level).filter(|l| l.category == Category::Level
                    && l.geometry == Some(Geometry::Level));
                matches!(el.category, Category::Wall | Category::Beam | Category::Duct | Category::Pipe | Category::Truss)
                    && (el.category != Category::Truss || (0.05..=1000.0).contains(&el.spacing_m))
                    && valid_axis(*start, *end) && datum.is_some()
                    && el.width_m <= 1e6 && el.height_m <= 1e6
                    && (el.category != Category::Pipe || (el.width_m - el.height_m).abs() <= 1e-9)
                    && {
                        let z = datum.map_or(0.0, |l| l.elevation_m) + el.elevation_m;
                        let end_z = z + (end[0] - start[0]).hypot(end[1] - start[1]) * el.slope_ratio.unwrap_or(0.0);
                        z.abs() <= 1e6 && end_z.is_finite() && end_z.abs() <= 1e6
                    }
            }
        };
        if !valid { return Err(EditError::InvalidGeometry(el.id)); }
    }
    Ok(())
}

type P2 = [f64; 2];
fn p2_sub(a: P2, b: P2) -> P2 { [a[0] - b[0], a[1] - b[1]] }
fn p2_add(a: P2, b: P2) -> P2 { [a[0] + b[0], a[1] + b[1]] }
fn p2_mul(a: P2, k: f64) -> P2 { [a[0] * k, a[1] * k] }
fn p2_cross(a: P2, b: P2) -> f64 { a[0] * b[1] - a[1] * b[0] }
fn p2_dot(a: P2, b: P2) -> f64 { a[0] * b[0] + a[1] * b[1] }
fn p2_len(a: P2) -> f64 { a[0].hypot(a[1]) }
fn p2_unit(a: P2) -> P2 { let l = p2_len(a); if l < 1e-12 { [0.0, 0.0] } else { [a[0] / l, a[1] / l] } }
fn p2_left(d: P2) -> P2 { [-d[1], d[0]] }
/// The point where the line through `p` along `d` meets the line through `q` along `e`.
pub fn line_meet(p: P2, d: P2, q: P2, e: P2) -> Option<P2> {
    let den = p2_cross(d, e);
    if den.abs() < 1e-9 { return None; }
    let t = p2_cross(p2_sub(q, p), e) / den;
    Some(p2_add(p, p2_mul(d, t)))
}

impl Document {
    /// The walls of a level other than `id`, as (start, end, thickness).
    fn walls_around(&self, id: ElementId, level: ElementId) -> Vec<(P2, P2, f64, [bool; 2])> {
        self.elements().filter(|e| e.id != id && e.category == Category::Wall).filter_map(|e| match e.geometry {
            Some(Geometry::Linear { start, end, level: l }) if l == level && p2_len(p2_sub(end, start)) > 1e-6 => Some((start, end, e.width_m, e.join_ends)),
            _ => None,
        }).collect()
    }

    /// How one end of a wall meets the walls around it, as Revit joins them: two walls that meet at their ends
    /// are mitred, three or more meet with the straight pair running through and the others stopping at its
    /// face, and a wall whose end touches the body of another stops at the face of that wall. The line is where
    /// the end of the wall stops.
    fn wall_end_stop(&self, w: &Element, at_start: bool) -> Option<openbim_geom::StopLine> {
        let Some(Geometry::Linear { start, end, level }) = w.geometry else { return None };
        // a join that is not allowed at this end leaves it square
        if !w.join_ends[usize::from(!at_start)] { return None; }
        let (e, far) = if at_start { (start, end) } else { (end, start) };
        let dw = p2_unit(p2_sub(far, e));
        let tw = w.width_m;
        let others = self.walls_around(w.id, level);
        // the walls that end at the same point, as the direction they leave it in
        let meeting: Vec<(P2, f64)> = others.iter().filter_map(|&(s, t, th, joins)| {
            if p2_len(p2_sub(s, e)) < 1e-3 { joins[0].then(|| (p2_unit(p2_sub(t, s)), th)) }
            else if p2_len(p2_sub(t, e)) < 1e-3 { joins[1].then(|| (p2_unit(p2_sub(s, t)), th)) } else { None }
        }).collect();
        let face = |d: P2, t: f64, toward: P2| -> openbim_geom::StopLine {
            let side = if p2_cross(d, toward) >= 0.0 { 1.0 } else { -1.0 };
            openbim_geom::StopLine { point: p2_add(e, p2_mul(p2_left(d), side * t / 2.0)), dir: d }
        };
        match meeting.len() {
            0 => {}
            1 => {
                let (d, t) = meeting[0];
                if p2_cross(dw, d).abs() < 0.17 { return None; }
                let (nw, no) = (p2_left(dw), p2_left(d));
                let pa = line_meet(p2_add(e, p2_mul(nw, tw / 2.0)), dw, p2_sub(e, p2_mul(no, t / 2.0)), d)?;
                let pb = line_meet(p2_sub(e, p2_mul(nw, tw / 2.0)), dw, p2_add(e, p2_mul(no, t / 2.0)), d)?;
                let dir = p2_sub(pb, pa);
                return if p2_len(dir) < 1e-9 { None } else { Some(openbim_geom::StopLine { point: pa, dir }) };
            }
            _ => {
                let mut all = vec![(dw, tw)];
                all.extend(meeting.iter().copied());
                let mut best = (0, 1, f64::MAX);
                for i in 0..all.len() { for j in i + 1..all.len() {
                    let dot = p2_dot(all[i].0, all[j].0);
                    if dot < best.2 { best = (i, j, dot); }
                } }
                if best.0 == 0 || best.1 == 0 { return None; }
                let (d, t) = all[best.0];
                return Some(face(d, t, dw));
            }
        }
        // an end on the body of another wall
        for (s, t, th, _) in others {
            let d = p2_unit(p2_sub(t, s));
            let len = p2_len(p2_sub(t, s));
            let along = p2_dot(p2_sub(e, s), d);
            let off = p2_cross(d, p2_sub(e, s));
            if along > 1e-3 && along < len - 1e-3 && off.abs() <= th / 2.0 + 1e-3 && p2_cross(d, dw).abs() > 0.17 {
                let side = if p2_cross(d, dw) >= 0.0 { 1.0 } else { -1.0 };
                return Some(openbim_geom::StopLine { point: p2_add(s, p2_mul(p2_left(d), side * th / 2.0)), dir: d });
            }
        }
        None
    }

    /// The wall as a chain of one segment, its ends joined to the walls around it. Ends that cannot be joined
    /// (a join that does not make sense) stay square.
    pub fn wall_chain(&self, id: ElementId) -> Option<openbim_geom::WallChain> {
        let w = self.get(id)?;
        if w.category != Category::Wall { return None; }
        let Some(Geometry::Linear { start, end, .. }) = w.geometry else { return None };
        let mut chain = openbim_geom::WallChain::new(vec![start, end], false, w.width_m, w.height_m);
        chain.start_stop = self.wall_end_stop(w, true);
        chain.end_stop = self.wall_end_stop(w, false);
        if chain.plans().is_ok() { return Some(chain); }
        let (a, b) = (chain.start_stop.take(), chain.end_stop.take());
        for keep in [(a, None), (None, b)] {
            chain.start_stop = keep.0; chain.end_stop = keep.1;
            if chain.plans().is_ok() { return Some(chain); }
        }
        chain.start_stop = None; chain.end_stop = None;
        Some(chain)
    }

    /// The plan of a wall with its joins: the corners of its right edge (start, end) and its left edge (end, start).
    pub fn wall_plan(&self, id: ElementId) -> Option<[P2; 4]> {
        let plan = self.wall_chain(id)?.plans().ok()?.into_iter().next()?;
        Some([plan.r0, plan.r1, plan.l1, plan.l0])
    }
}

impl Document {
    /// The axis of a beam after its joins: each end that lies in a column (and has its join allowed) is cut back to
    /// the face of that column, where the beam first meets it. None for anything but a beam on a level.
    pub fn beam_extent(&self, el: &Element) -> Option<BeamExtent> {
        let (Category::Beam, Some(Geometry::Linear { start, end, level })) = (el.category, el.geometry) else { return None };
        let z = self.get(level)?.elevation_m + el.elevation_m;
        let slope = el.slope_ratio.unwrap_or(0.0);
        let run = (end[0] - start[0]).hypot(end[1] - start[1]);
        let z_at = |p: [f64; 2]| z + (p[0] - start[0]).hypot(p[1] - start[1]) * slope;
        let columns: Vec<(Vec<[f64; 2]>, f64, f64)> = self.elements().filter_map(|c| {
            let (Category::Column, Some(Geometry::Point { at, level, top })) = (c.category, c.geometry) else { return None };
            let ring = c.profile().outline().into_iter().map(|p| [at[0] + p[0], at[1] + p[1]]).collect();
            Some((ring, self.get(level)?.elevation_m + c.elevation_m, self.get(top)?.elevation_m))
        }).collect();
        // `p` is the end to trim and `q` the other end; the new end is where q -> p first meets a column it ends in
        let trim = |p: [f64; 2], q: [f64; 2]| -> [f64; 2] {
            let pz = z_at(p);
            for (ring, z0, z1) in &columns {
                if pz < z0 - 1e-6 || pz > z1 + 1e-6 || !inside(ring, p) || inside(ring, q) { continue; }
                let mut best: Option<f64> = None;
                for i in 0..ring.len() {
                    if let Some(s) = segment_param(q, p, ring[i], ring[(i + 1) % ring.len()]) {
                        best = Some(best.map_or(s, |b: f64| b.min(s)));
                    }
                }
                if let Some(s) = best { return [q[0] + (p[0] - q[0]) * s, q[1] + (p[1] - q[1]) * s]; }
            }
            p
        };
        let a2 = if el.join_ends[0] { trim(start, end) } else { start };
        let b2 = if el.join_ends[1] { trim(end, start) } else { end };
        // both trims are made against the original other end; they must leave a beam with a length
        let (a2, b2) = if (b2[0] - a2[0]).hypot(b2[1] - a2[1]) > 1e-6 && (b2[0] - a2[0]) * (end[0] - start[0]) + (b2[1] - a2[1]) * (end[1] - start[1]) > 0.0 { (a2, b2) } else { (start, end) };
        let norm = (1.0 + slope * slope).sqrt();
        Some(BeamExtent {
            a: [a2[0], a2[1], z_at(a2)],
            b: [b2[0], b2[1], z_at(b2)],
            cut_start: (a2[0] - start[0]).hypot(a2[1] - start[1]) * norm,
            cut_end: (end[0] - b2[0]).hypot(end[1] - b2[1]) * norm,
            full_length: run * norm,
        })
    }

    pub fn mesh(&self, id: ElementId) -> Option<Mesh> {
        let el = self.get(id)?;
        if let Some(Geometry::Point { at, level, top }) = el.geometry {
            // the section stood on its point from the foot to the top level; the end faces of an I-section are
            // concave polygons, which a plan does not mind (the 3D view meshes columns itself)
            let (z0, z1) = (self.get(level)?.elevation_m + el.elevation_m, self.get(top)?.elevation_m);
            let section: Vec<[f64; 2]> = el.profile().outline().into_iter().map(|p| [at[0] + p[0], at[1] + p[1]]).collect();
            let n = section.len();
            let mut vertices: Vec<[f64; 3]> = section.iter().map(|p| [p[0], p[1], z0]).collect();
            vertices.extend(section.iter().map(|p| [p[0], p[1], z1]));
            let mut faces: Vec<Vec<usize>> = vec![(0..n).rev().collect(), (n..2 * n).collect()];
            for i in 0..n { faces.push(vec![i, (i + 1) % n, (i + 1) % n + n, i + n]); }
            return Some(Mesh { vertices, faces });
        }
        if let Some(Geometry::OnWall { host }) = el.geometry {
            let wall = self.get(host)?;
            let Some(Geometry::Linear { start, end, level }) = wall.geometry else { return None };
            let (dx, dy) = (end[0] - start[0], end[1] - start[1]);
            let len = dx.hypot(dy);
            let (nx, ny) = (-dy / len * el.width_m / 2.0, dx / len * el.width_m / 2.0);
            let top = self.get(level)?.elevation_m + wall.elevation_m + el.elevation_m;
            let ring = [[start[0] - nx, start[1] - ny], [end[0] - nx, end[1] - ny], [end[0] + nx, end[1] + ny], [start[0] + nx, start[1] + ny]];
            let mut vertices: Vec<[f64; 3]> = ring.iter().map(|p| [p[0], p[1], top - el.height_m]).collect();
            vertices.extend(ring.iter().map(|p| [p[0], p[1], top]));
            let mut faces = vec![vec![3, 2, 1, 0], vec![4, 5, 6, 7]];
            for i in 0..4 { faces.push(vec![i, (i + 1) % 4, (i + 1) % 4 + 4, i + 4]); }
            return Some(Mesh { vertices, faces });
        }
        if let Some(Geometry::Pad { at, level, thickness_m }) = el.geometry {
            let top = self.get(level)?.elevation_m + el.elevation_m;
            let (hx, hy) = (el.width_m / 2.0, el.height_m / 2.0);
            let ring = [[at[0] - hx, at[1] - hy], [at[0] + hx, at[1] - hy], [at[0] + hx, at[1] + hy], [at[0] - hx, at[1] + hy]];
            let mut vertices: Vec<[f64; 3]> = ring.iter().map(|p| [p[0], p[1], top - thickness_m]).collect();
            vertices.extend(ring.iter().map(|p| [p[0], p[1], top]));
            let mut faces = vec![vec![3, 2, 1, 0], vec![4, 5, 6, 7]];
            for i in 0..4 { faces.push(vec![i, (i + 1) % 4, (i + 1) % 4 + 4, i + 4]); }
            return Some(Mesh { vertices, faces });
        }
        if matches!(el.category, Category::BeamSystem | Category::Truss) { return None; }
        if el.category == Category::Rebar { return self.rebar_mesh(el); }
        if el.category == Category::Family { return None; }
        if let Some(Geometry::Area { level }) = el.geometry {
            // the slab hangs from its top, which is `elevation_m` above the level
            let ring = &el.outline;
            let n = ring.len();
            let top = self.get(level)?.elevation_m + el.elevation_m;
            let bottom = top - el.height_m;
            let mut vertices: Vec<[f64; 3]> = ring.iter().map(|p| [p[0], p[1], bottom]).collect();
            vertices.extend(ring.iter().map(|p| [p[0], p[1], top]));
            let ccw = openbim_geom::profile::polygon_area(ring) > 0.0;
            let mut faces: Vec<Vec<usize>> = Vec::new();
            for t in openbim_geom::profile::triangulate(ring)? {
                // each triangle is wound on its own: counter-clockwise on top, clockwise underneath
                let [a, b, c] = if openbim_geom::profile::polygon_area(&[ring[t[0]], ring[t[1]], ring[t[2]]]) > 0.0 { t } else { [t[0], t[2], t[1]] };
                faces.push(vec![a + n, b + n, c + n]);
                faces.push(vec![c, b, a]);
            }
            for i in 0..n {
                let j = (i + 1) % n;
                faces.push(if ccw { vec![i, j, j + n, i + n] } else { vec![j, i, i + n, j + n] });
            }
            return Some(Mesh { vertices, faces });
        }
        if el.category == Category::Wall {
            if let (Some(Geometry::Linear { level, .. }), Some(ring)) = (el.geometry, self.wall_plan(id)) {
                let z0 = self.get(level)?.elevation_m + el.elevation_m;
                let mut vertices: Vec<[f64; 3]> = ring.iter().map(|p| [p[0], p[1], z0]).collect();
                vertices.extend(ring.iter().map(|p| [p[0], p[1], z0 + el.height_m]));
                let mut faces = vec![vec![3, 2, 1, 0], vec![4, 5, 6, 7]];
                for i in 0..4 { faces.push(vec![i, (i + 1) % 4, (i + 1) % 4 + 4, i + 4]); }
                return Some(Mesh { vertices, faces });
            }
        }
        let Geometry::Linear { start, end, level } = el.geometry.as_ref()? else { return None; };
        let z = self.get(*level)?.elevation_m + el.elevation_m;
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let length = dx.hypot(dy);
        let slope = el.slope_ratio.unwrap_or(0.0);
        let norm = (1.0 + slope * slope).sqrt();
        let across = [-dy / length, dx / length, 0.0];
        let up = [-dx / length * slope / norm, -dy / length * slope / norm, 1.0 / norm];
        let center_z = z + if el.category == Category::Wall { el.height_m / 2.0 } else { 0.0 };
        let mut centers = [[start[0], start[1], center_z], [end[0], end[1], center_z + length * slope]];
        if let Some(ext) = self.beam_extent(el) { centers = [ext.a, ext.b]; }
        let section: Vec<[f64; 2]> = if el.category == Category::Beam {
            // the outline of an I-section is not convex: its end faces are polygons that are fine for a plan, but
            // the 3D view meshes beams itself (`openbim_view::beam_mesh`)
            el.profile().outline()
        } else if el.category == Category::Pipe {
            (0..16).map(|i| {
                let angle = i as f64 * std::f64::consts::TAU / 16.0;
                [el.width_m / 2.0 * angle.cos(), el.width_m / 2.0 * angle.sin()]
            }).collect()
        } else {
            vec![[-el.width_m / 2.0, -el.height_m / 2.0],
                 [el.width_m / 2.0, -el.height_m / 2.0],
                 [el.width_m / 2.0, el.height_m / 2.0],
                 [-el.width_m / 2.0, el.height_m / 2.0]]
        };
        let n = section.len();
        let mut vertices = Vec::with_capacity(n * 2);
        for center in centers {
            for [a, b] in &section {
                vertices.push(std::array::from_fn(|i| center[i] + across[i] * a + up[i] * b));
            }
        }
        let mut faces = vec![(0..n).rev().collect(), (n..2 * n).collect()];
        for i in 0..n { faces.push(vec![i, (i + 1) % n, (i + 1) % n + n, i + n]); }
        Some(Mesh { vertices, faces })
    }

    /// All spatial seed data is created through the same transaction interface
    /// used by edits. Renderers contain no duplicate wall/grid coordinates.
    pub fn spatial_sample() -> Self {
        let mut ops = Vec::new();
        let mut add = |id, category, name: &str, width_m, height_m, elevation_m, geometry| {
            ops.push(super::Command::Create { id: ElementId(id), category, name: name.into(), width_m, height_m, elevation_m });
            ops.push(super::Command::SetGeometry { id: ElementId(id), geometry });
        };
        add(10, Category::Level, "Level 1", 1.0, 1.0, 0.0, Geometry::Level);
        add(11, Category::Level, "Level 2", 1.0, 1.0, 3.0, Geometry::Level);
        for (id, name, start, end) in [
            (100, "A", [0.0, -1.0], [0.0, 7.0]), (101, "B", [4.0, -1.0], [4.0, 7.0]),
            (102, "C", [8.0, -1.0], [8.0, 7.0]), (103, "1", [-1.0, 0.0], [9.0, 0.0]),
            (104, "2", [-1.0, 3.0], [9.0, 3.0]), (105, "3", [-1.0, 6.0], [9.0, 6.0]),
        ] { add(id, Category::Grid, name, 1.0, 1.0, 0.0, Geometry::Grid { start, end }); }
        for (i, start, end) in [
            (0, [0.0, 0.0], [8.0, 0.0]), (1, [8.0, 0.0], [8.0, 6.0]),
            (2, [8.0, 6.0], [0.0, 6.0]), (3, [0.0, 6.0], [0.0, 0.0]),
            (4, [4.0, 0.0], [4.0, 6.0]),
        ] { add(20 + i, Category::Wall, &format!("Wall {}", i + 1), 0.2, 3.0, 0.0,
            Geometry::Linear { start, end, level: ElementId(10) }); }
        add(30, Category::Wall, "Upper wall", 0.2, 2.8, 0.0,
            Geometry::Linear { start: [0.0, 6.0], end: [8.0, 6.0], level: ElementId(11) });
        add(1, Category::Duct, "Supply Air Duct", 0.2, 0.3, 2.7,
            Geometry::Linear { start: [1.0, 2.0], end: [7.0, 2.0], level: ElementId(10) });
        add(2, Category::Pipe, "Drainage Pipe", 0.1, 0.1, 0.6,
            Geometry::Linear { start: [1.0, 4.0], end: [7.0, 4.0], level: ElementId(10) });
        // the sample's site: an illustration, like the rest of the sample, and editable
        ops.push(super::Command::SetSite(crate::Site { latitude: 11.5564, longitude: 104.9282, angle_to_true_north_deg: 0.0, utc_offset_hours: 7.0 }));
        let mut document = Self::new();
        document.transact(0, &ops).expect("valid spatial sample");
        document.clear_undo_history();
        document
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_tag_says_the_type_follows_its_element_goes_with_it_and_is_saved() {
        let mut d = Document::spatial_sample();
        d.create_type(ElementId(20), "Wall 200 x 3000").unwrap();
        let tag = d.place_tag(ElementId(20), [1.0, 1.0], [0.5, 0.0]).unwrap();
        assert_eq!(d.tag_text(tag), "Wall 200 x 3000");
        // an element without a type is tagged with its name; levels, grids and tags are not tagged
        let tag2 = d.place_tag(ElementId(21), [8.5, 1.0], [8.0, 1.0]).unwrap();
        assert_eq!(d.tag_text(tag2), "Wall 2");
        assert!(d.place_tag(ElementId(10), [0.0, 0.0], [0.0, 0.0]).is_err() && d.place_tag(tag, [0.0, 0.0], [0.0, 0.0]).is_err());
        assert_eq!(d.level_of(tag), Some(ElementId(10)));
        // it is saved with the model and comes back as it was
        let lines = crate::native::records(&d);
        let (elements, _) = crate::native::parse_records(&lines).unwrap();
        assert_eq!(elements.iter().find(|e| e.id == tag).unwrap().geometry, d.get(tag).unwrap().geometry);
        // deleting the element takes its tags, and undo brings them back
        d.transact(d.revision(), &[Command::Delete { id: ElementId(21) }]).unwrap();
        assert!(d.get(tag2).is_none() && d.get(tag).is_some());
        d.undo().unwrap();
        assert!(d.get(tag2).is_some());
    }

    #[test]
    fn walls_are_joined_as_revit_joins_them() {
        let d = Document::spatial_sample();
        let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6;
        let has = |ring: &[[f64; 2]; 4], p: [f64; 2]| ring.iter().any(|&q| near(q, p));
        // Wall 1 runs (0,0)-(8,0) and wall 4 (0,6)-(0,0): they meet at (0,0) and are mitred, so the outer corner is
        // filled and the inner corner is cut
        let (w1, w4) = (d.wall_plan(ElementId(20)).unwrap(), d.wall_plan(ElementId(23)).unwrap());
        for ring in [&w1, &w4] { assert!(has(ring, [-0.1, -0.1]) && has(ring, [0.1, 0.1]), "{ring:?}"); }
        // Wall 5 runs (4,0)-(4,6) from the body of wall 1 to the body of wall 3: it stops at their faces
        let w5 = d.wall_plan(ElementId(24)).unwrap();
        assert!(has(&w5, [3.9, 0.1]) && has(&w5, [4.1, 0.1]) && has(&w5, [3.9, 5.9]) && has(&w5, [4.1, 5.9]), "{w5:?}");
        // the mesh of a wall is that plan, standing 3 m high, and it is closed
        let m = d.mesh(ElementId(24)).unwrap();
        assert!((volume(&m).abs() - 0.2 * 5.8 * 3.0).abs() < 1e-6, "{}", volume(&m));
        // a wall on its own keeps its square ends
        let mut alone = Document::spatial_sample();
        let level = ElementId(10);
        let id = alone.place_wall(level, [20.0, 20.0], [25.0, 20.0], 0.3, 3.0, 0.0).unwrap();
        assert!(has(&alone.wall_plan(id).unwrap(), [20.0, 20.15]));
        // two collinear walls meet flat
        let b = alone.place_wall(level, [25.0, 20.0], [30.0, 20.0], 0.3, 3.0, 0.0).unwrap();
        assert!(has(&alone.wall_plan(b).unwrap(), [25.0, 20.15]) && has(&alone.wall_plan(b).unwrap(), [25.0, 19.85]));
        // walls on another level do not join
        let c = alone.place_wall(ElementId(11), [25.0, 20.0], [25.0, 25.0], 0.3, 3.0, 0.0).unwrap();
        assert!(has(&alone.wall_plan(c).unwrap(), [24.85, 20.0]));
    }
    use super::*;
    use crate::{Material, SlabKind, SlabSpec};
    #[test]
    fn a_beam_takes_a_round_or_an_i_section_changes_with_its_size_and_the_sections_are_validated() {
        use crate::Section;
        let mut d = Document::spatial_sample();
        let place = |d: &mut Document, w, h, s| d.place_beam(ElementId(10), &crate::BeamSpec { section: s, ..crate::BeamSpec::level([0.0, 1.0], [4.0, 1.0], w, h, 2.8) });
        // a round beam is as deep as it is wide, and its mesh is a 32-gon prism
        let round = place(&mut d, 0.3, 0.3, Section::Circle).unwrap();
        assert_eq!(d.mesh(round).unwrap().vertices.len(), 64);
        let other = place(&mut d, 0.3, 0.4, Section::Circle).unwrap();
        assert_eq!(d.get(other).unwrap().height_m, 0.3, "a circle takes its width as its diameter");
        // an I-section needs a web thinner than its flanges are wide and room between its flanges
        let i = place(&mut d, 0.2, 0.4, Section::IShape { web_m: 0.01, flange_m: 0.02 }).unwrap();
        assert_eq!(d.mesh(i).unwrap().vertices.len(), 24);
        assert!(place(&mut d, 0.2, 0.4, Section::IShape { web_m: 0.3, flange_m: 0.02 }).is_err());
        assert!(place(&mut d, 0.2, 0.04, Section::IShape { web_m: 0.01, flange_m: 0.02 }).is_err());
        // a section and its size change in one transaction: I to circle
        d.transact(d.revision(), &[
            Command::Resize { id: i, width_m: 0.25, height_m: 0.5 },
            Command::SetSection { id: i, section: Section::Circle },
        ]).unwrap();
        assert_eq!((d.get(i).unwrap().width_m, d.get(i).unwrap().height_m), (0.25, 0.25), "the circle takes the width");
        d.undo().unwrap();
        assert_eq!(d.get(i).unwrap().section, Section::IShape { web_m: 0.01, flange_m: 0.02 });
        // only beams have a section
        assert!(d.transact(d.revision(), &[Command::SetSection { id: ElementId(20), section: Section::Circle }]).is_err());
    }

    #[test]
    fn a_column_stands_between_two_levels_is_validated_resized_and_undone() {
        use crate::{ColumnSpec, Material, Section};
        let mut d = Document::spatial_sample();
        let spec = ColumnSpec { width_m: 0.4, depth_m: 0.4, base_offset_m: 0.0, section: Section::Rectangle, material: Material::Concrete };
        let id = d.place_column(ElementId(10), ElementId(11), [2.0, 2.0], &spec).unwrap();
        let el = d.get(id).unwrap();
        assert_eq!((el.category, el.material, el.name.as_str()), (Category::Column, Material::Concrete, "Column 1"));
        let m = d.mesh(id).unwrap();
        let zs: Vec<f64> = m.vertices.iter().map(|v| v[2]).collect();
        assert!(zs.iter().all(|z| *z == 0.0 || *z == 3.0), "from Level 1 (0) to Level 2 (3): {zs:?}");
        assert_eq!(m.vertices.len(), 8);
        // the top has to be above the foot: Level 2 to Level 1, or a foot raised to the top, are refused
        assert!(d.place_column(ElementId(11), ElementId(10), [2.0, 2.0], &spec).is_err());
        assert!(d.place_column(ElementId(10), ElementId(11), [2.0, 2.0], &ColumnSpec { base_offset_m: 3.0, ..spec }).is_err());
        assert!(d.place_column(ElementId(10), ElementId(20), [2.0, 2.0], &spec).is_err(), "a wall is not a level");
        assert!(d.place_column(ElementId(10), ElementId(11), [f64::NAN, 2.0], &spec).is_err());
        // a round column, and the foot raised 0.5 m: one transaction
        d.transact(d.revision(), &[
            Command::SetSection { id, section: Section::Circle },
            Command::MoveElevation { id, elevation_m: 0.5 },
        ]).unwrap();
        let el = d.get(id).unwrap();
        assert_eq!((el.section, el.width_m, el.height_m, el.elevation_m), (Section::Circle, 0.4, 0.4, 0.5));
        assert_eq!(d.mesh(id).unwrap().vertices.len(), 64);
        // moving Level 2 down so the column would have no height is refused and changes nothing
        let rev = d.revision();
        assert!(d.transact(rev, &[Command::MoveElevation { id: ElementId(11), elevation_m: 0.4 }]).is_err());
        assert_eq!(d.revision(), rev);
        // a column cannot be created without its place
        assert!(d.transact(d.revision(), &[Command::Create { id: ElementId(500), category: Category::Column, name: "C".into(), width_m: 0.3, height_m: 0.3, elevation_m: 0.0 }]).is_err());
        d.undo().unwrap();
        assert_eq!(d.get(id).unwrap().section, Section::Rectangle);
        d.undo().unwrap();
        assert!(d.get(id).is_none());
    }

    #[test]
    fn a_brace_is_a_sloped_beam_with_a_usage_and_a_material_and_only_beams_have_them() {
        use crate::{BeamSpec, Material, Usage};
        let mut d = Document::spatial_sample();
        // 3 m of run rising 3 m: a diagonal from 0.5 m at the start to 3.5 m at the end
        let spec = BeamSpec { usage: Usage::Brace, material: Material::Steel, slope_ratio: 1.0, ..BeamSpec::level([0.0, 1.0], [3.0, 1.0], 0.1, 0.1, 0.5) };
        let id = d.place_beam(ElementId(10), &spec).unwrap();
        let el = d.get(id).unwrap();
        assert_eq!((el.usage, el.material, el.slope_ratio, el.name.as_str()), (Usage::Brace, Material::Steel, Some(1.0), "Brace 1"));
        let zs: Vec<f64> = d.mesh(id).unwrap().vertices.iter().map(|v| v[2]).collect();
        let (lo, hi) = (zs.iter().cloned().fold(f64::MAX, f64::min), zs.iter().cloned().fold(f64::MIN, f64::max));
        // the centre line runs 0.5 to 3.5; the 0.1 square section is turned with the member
        assert!(lo > 0.4 - 0.1 && lo < 0.5 && hi > 3.5 && hi < 3.6, "{lo} {hi}");
        d.transact(d.revision(), &[Command::SetUsage { id, usage: Usage::Joist }, Command::SetMaterial { id, material: Material::Timber }]).unwrap();
        assert_eq!((d.get(id).unwrap().usage, d.get(id).unwrap().material), (Usage::Joist, Material::Timber));
        d.undo().unwrap();
        assert_eq!((d.get(id).unwrap().usage, d.get(id).unwrap().material), (Usage::Brace, Material::Steel));
        assert!(d.transact(d.revision(), &[Command::SetUsage { id: ElementId(20), usage: Usage::Brace }]).is_err());
        assert!(d.transact(d.revision(), &[Command::SetMaterial { id: ElementId(20), material: Material::Steel }]).is_err());
    }

    #[test]
    fn a_beam_is_placed_on_a_level_validated_and_undone_in_one_step() {
        let mut d = Document::spatial_sample();
        let id = d.place_beam(ElementId(10), &crate::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.2, 0.4, 2.8)).unwrap();
        assert_eq!(id, ElementId(d.elements().map(|e| e.id.0).max().unwrap()));
        assert_eq!(d.get(id).unwrap().category, Category::Beam);
        let m = d.mesh(id).unwrap();
        assert_eq!(m.vertices.len(), 8);
        let zs: Vec<f64> = m.vertices.iter().map(|v| v[2]).collect();
        assert!(zs.iter().all(|z| (z - 2.6).abs() < 1e-9 || (z - 3.0).abs() < 1e-9));
        // zero length, no size, and a level that is not one are all refused and change nothing
        let rev = d.revision();
        assert!(d.place_beam(ElementId(10), &crate::BeamSpec::level([1.0, 1.0], [1.0, 1.0], 0.2, 0.4, 2.8)).is_err());
        assert!(d.place_beam(ElementId(10), &crate::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.0, 0.4, 2.8)).is_err());
        assert!(d.place_beam(ElementId(100), &crate::BeamSpec::level([1.0, 1.0], [5.0, 1.0], 0.2, 0.4, 2.8)).is_err());
        assert_eq!(d.revision(), rev);
        d.undo().unwrap();
        assert!(d.get(id).is_none(), "one undo removes it whole");
    }

    use crate::Command;

    #[test]
    fn wall_dimensions_and_level_elevation_drive_vertices_and_undo() {
        let mut d = Document::spatial_sample();
        let original = d.mesh(ElementId(20)).unwrap();
        assert!(!d.can_undo());
        d.transact(d.revision(), &[
            Command::Resize { id: ElementId(20), width_m: 0.4, height_m: 4.0 },
            Command::MoveElevation { id: ElementId(10), elevation_m: 1.0 },
        ]).unwrap();
        let changed = d.mesh(ElementId(20)).unwrap();
        assert_eq!(changed.vertices[0][1], -0.2);
        assert_eq!(changed.vertices[0][2], 1.0);
        assert_eq!(changed.vertices[4][2], 5.0);
        d.undo().unwrap();
        assert_eq!(d.mesh(ElementId(20)).unwrap(), original);
    }

    #[test]
    fn duct_and_pipe_slope_raise_the_end_by_rise_over_run() {
        let mut d = Document::spatial_sample();
        for id in [ElementId(1), ElementId(2)] {
            d.transact(d.revision(), &[Command::SetSlope { id, slope_ratio: 0.02 }]).unwrap();
            let mesh = d.mesh(id).unwrap();
            let n = mesh.vertices.len() / 2;
            assert!((mesh.vertices[n][2] - mesh.vertices[0][2] - 0.12).abs() < 1e-9);
        }
    }

    #[test]
    fn invalid_axes_and_referenced_level_deletion_are_atomic() {
        let mut d = Document::spatial_sample();
        let before = d.clone();
        assert!(d.transact(d.revision(), &[Command::Delete { id: ElementId(10) }]).is_err());
        assert_eq!(d, before);
        assert!(d.transact(d.revision(), &[Command::SetGeometry {
            id: ElementId(100), geometry: Geometry::Grid { start: [0.0, 0.0], end: [0.0, 0.0] },
        }]).is_err());
        assert_eq!(d, before);
    }

    #[test]
    fn datum_dimensions_and_inconsistent_pipe_sections_are_rejected() {
        let mut d = Document::spatial_sample();
        let before = d.clone();
        for id in [ElementId(10), ElementId(100)] {
            assert_eq!(d.transact(d.revision(), &[Command::Resize { id, width_m: 1.0, height_m: 2.0 }]),
                Err(EditError::UnsupportedProperty(id)));
        }
        assert!(d.transact(d.revision(), &[Command::Resize { id: ElementId(2), width_m: 0.2, height_m: 0.1 }]).is_err());
        assert_eq!(d, before);
    }

    fn slab(outline: Vec<[f64; 2]>) -> SlabSpec {
        SlabSpec { outline, thickness_m: 0.2, top_offset_m: 0.0, kind: SlabKind::Floor, material: Material::Concrete }
    }

    fn volume(mesh: &Mesh) -> f64 {
        // divergence theorem over fan-triangulated faces: positive when the faces face outwards
        let mut v = 0.0;
        for f in &mesh.faces {
            for k in 1..f.len() - 1 {
                let (a, b, c) = (mesh.vertices[f[0]], mesh.vertices[f[k]], mesh.vertices[f[k + 1]]);
                v += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0;
            }
        }
        v
    }

    #[test]
    fn slabs_are_placed_validated_undone_and_meshed_closed() {
        let mut d = Document::spatial_sample();
        let level = ElementId(10);
        let before = d.clone();
        // an L-shaped slab, 5 x 4 minus a 2 x 2 corner = 16 m2, in both windings
        let l = vec![[0.0, 0.0], [5.0, 0.0], [5.0, 2.0], [3.0, 2.0], [3.0, 4.0], [0.0, 4.0]];
        let mut rev = l.clone();
        rev.reverse();
        for ring in [l.clone(), rev] {
            let id = d.place_slab(level, &slab(ring)).unwrap();
            let el = d.get(id).unwrap();
            assert_eq!((el.category, el.material, el.slab_kind), (Category::Slab, Material::Concrete, SlabKind::Floor));
            let mesh = d.mesh(id).unwrap();
            assert!((volume(&mesh) - 16.0 * 0.2).abs() < 1e-9, "volume {}", volume(&mesh));
        }
        // refused: too few corners, no area, a bow-tie, a vertex touching an edge, a foundation outline on a missing level
        for bad in [
            vec![[0.0, 0.0], [1.0, 0.0]],
            vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]],
            vec![[0.0, 0.0], [2.0, 2.0], [2.0, 0.0], [0.0, 2.0]],
            vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [2.0, 0.0], [0.0, 4.0]],
            vec![[0.0, 0.0], [f64::NAN, 0.0], [1.0, 1.0]],
        ] {
            let n = d.elements().count();
            assert!(d.place_slab(level, &slab(bad)).is_err());
            assert_eq!(d.elements().count(), n);
        }
        assert!(d.place_slab(ElementId(9999), &slab(l)).is_err());
        // undo takes the slabs away again
        while d.undo().unwrap() {}
        assert_eq!(d.elements().count(), before.elements().count());
        assert!(d.elements().all(|e| e.category != Category::Slab));
    }

    #[test]
    fn a_foundation_slab_hangs_from_its_top_and_keeps_its_kind() {
        let mut d = Document::spatial_sample();
        let spec = SlabSpec { outline: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]], thickness_m: 0.5, top_offset_m: -0.1, kind: SlabKind::Foundation, material: Material::Concrete };
        let id = d.place_slab(ElementId(10), &spec).unwrap();
        let mesh = d.mesh(id).unwrap();
        let zs: Vec<f64> = mesh.vertices.iter().map(|v| v[2]).collect();
        let base = d.get(ElementId(10)).unwrap().elevation_m;
        assert!((zs.iter().cloned().fold(f64::MIN, f64::max) - (base - 0.1)).abs() < 1e-9);
        assert!((zs.iter().cloned().fold(f64::MAX, f64::min) - (base - 0.6)).abs() < 1e-9);
        assert_eq!(d.get(id).unwrap().slab_kind, SlabKind::Foundation);
        // only slabs take an outline or a kind
        assert!(d.transact(d.revision(), &[Command::SetSlabKind { id: ElementId(2), kind: SlabKind::Foundation }]).is_err());
        assert!(d.transact(d.revision(), &[Command::SetOutline { id: ElementId(2), outline: vec![] }]).is_err());
    }

    #[test]
    fn a_beam_is_cut_back_to_the_columns_it_ends_in_unless_the_join_is_disallowed() {
        use crate::{BeamSpec, ColumnSpec, Command};
        let mut d = Document::spatial_sample();
        let col = |d: &mut Document, x| d.place_column(ElementId(10), ElementId(11), [x, 3.0], &ColumnSpec { width_m: 0.4, depth_m: 0.4, base_offset_m: 0.0, section: crate::Section::Rectangle, material: Material::Concrete }).unwrap();
        let beam = d.place_beam(ElementId(10), &BeamSpec::level([1.0, 3.0], [5.0, 3.0], 0.2, 0.4, 2.8)).unwrap();
        let ext = |d: &Document| d.beam_extent(d.get(beam).unwrap()).unwrap();
        // no columns: the whole axis
        assert_eq!((ext(&d).a, ext(&d).b), ([1.0, 3.0, 2.8], [5.0, 3.0, 2.8]));
        let c1 = col(&mut d, 1.0);
        col(&mut d, 5.0);
        // both ends end in a 0.4 m column: 0.2 m comes off each
        let e = ext(&d);
        assert!((e.a[0] - 1.2).abs() < 1e-9 && (e.b[0] - 4.8).abs() < 1e-9, "{e:?}");
        assert!((e.cut_start - 0.2).abs() < 1e-9 && (e.cut_end - 0.2).abs() < 1e-9 && (e.cut_length() - 3.6).abs() < 1e-9);
        // the mesh and its volume follow the cut
        let m = d.mesh(beam).unwrap();
        let (x0, x1) = (m.vertices.iter().map(|v| v[0]).fold(f64::MAX, f64::min), m.vertices.iter().map(|v| v[0]).fold(f64::MIN, f64::max));
        assert!((x0 - 1.2).abs() < 1e-9 && (x1 - 4.8).abs() < 1e-9);
        // the join of the start disallowed: that end stays where it was
        d.transact(d.revision(), &[Command::SetJoins { id: beam, start: false, end: true }]).unwrap();
        assert!((ext(&d).a[0] - 1.0).abs() < 1e-9 && (ext(&d).b[0] - 4.8).abs() < 1e-9);
        // a column moves away: the beam follows by itself
        d.transact(d.revision(), &[Command::SetGeometry { id: c1, geometry: Geometry::Point { at: [0.0, 3.0], level: ElementId(10), top: ElementId(11) } }]).unwrap();
        d.transact(d.revision(), &[Command::SetJoins { id: beam, start: true, end: true }]).unwrap();
        assert!((ext(&d).a[0] - 1.0).abs() < 1e-9, "the column at 0 does not reach the beam any more");
        // above the column tops nothing is joined
        d.transact(d.revision(), &[Command::MoveElevation { id: beam, elevation_m: 3.5 }]).unwrap();
        assert!((ext(&d).b[0] - 5.0).abs() < 1e-9);
        // only beams have joins
        assert!(d.transact(d.revision(), &[Command::SetJoins { id: c1, start: false, end: true }]).is_err());
    }

    #[test]
    fn a_beam_system_makes_parallel_beams_that_follow_its_edits_and_go_with_it() {
        use crate::{BeamSystemSpec, Command, Section, Usage};
        let mut d = Document::spatial_sample();
        let spec = BeamSystemSpec { outline: vec![[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [0.0, 4.0]], spacing_m: 1.0, angle_deg: 0.0, width_m: 0.1, depth_m: 0.2, elevation_m: 2.8, section: Section::Rectangle, usage: Usage::Joist, material: Material::Timber };
        let sys = d.place_beam_system(ElementId(10), &spec).unwrap();
        let kids = d.system_beams(sys);
        assert_eq!(kids.len(), 4);
        let k = d.get(kids[0]).unwrap();
        assert_eq!((k.parent, k.category), (Some(sys), Category::Beam));
        let Some(Geometry::Linear { start, end, .. }) = k.geometry else { panic!() };
        assert!((end[0] - start[0] - 6.0).abs() < 1e-9 && (start[1] - end[1]).abs() < 1e-9);
        // a concave outline cuts a line into pieces
        let l = vec![[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [4.0, 4.0], [4.0, 2.0], [0.0, 2.0]];
        assert!(system_lines(&l, 0.0, 1.0).len() >= 4);
        assert!(system_lines(&l, 90.0, 1.5).len() >= 4);
        // editing replaces the beams in one transaction; undo brings the old ones back
        d.edit_beam_system(sys, &BeamSystemSpec { spacing_m: 2.0, angle_deg: 90.0, ..spec.clone() }).unwrap();
        assert_eq!(d.system_beams(sys).len(), 3);
        d.undo().unwrap();
        assert_eq!(d.system_beams(sys).len(), 4);
        // a bad spacing is refused, and nothing changes
        let n = d.elements().count();
        assert!(d.edit_beam_system(sys, &BeamSystemSpec { spacing_m: 0.0, ..spec.clone() }).is_err());
        assert_eq!(d.elements().count(), n);
        // a parent must be a beam system, and only beams have one
        assert!(d.transact(d.revision(), &[Command::SetParent { id: kids[0], parent: Some(kids[1]) }]).is_err());
        assert!(d.transact(d.revision(), &[Command::SetParent { id: sys, parent: None }]).is_err());
        // deleting the system takes its beams along
        d.delete_beam_system(sys).unwrap();
        assert!(d.get(sys).is_none() && d.get(kids[0]).is_none());
    }

    #[test]
    fn a_warren_truss_is_made_of_chords_and_diagonals_that_follow_its_edits() {
        use crate::{Command, TrussSpec, Usage};
        let mut d = Document::spatial_sample();
        let spec = TrussSpec { start: [0.0, 1.0], end: [8.0, 1.0], spacing_m: 2.0, depth_m: 1.0, member_m: 0.1, elevation_m: 2.0, material: Material::Steel };
        let t = d.place_truss(ElementId(10), &spec).unwrap();
        let kids = d.system_beams(t);
        // 4 panels: bottom chord, top chord and 8 diagonals
        assert_eq!(kids.len(), 10);
        assert_eq!(kids.iter().filter(|k| d.get(**k).unwrap().usage == Usage::Brace).count(), 8);
        let top = d.get(kids[1]).unwrap();
        assert_eq!(top.elevation_m, 3.0);
        let Some(Geometry::Linear { start, end, .. }) = top.geometry else { panic!() };
        assert!((start[0] - 1.0).abs() < 1e-9 && (end[0] - 7.0).abs() < 1e-9, "the top chord runs between the first and last apex");
        // the diagonals climb and fall by the depth over half a panel
        assert!((d.get(kids[2]).unwrap().slope_ratio.unwrap() - 1.0).abs() < 1e-9 && (d.get(kids[3]).unwrap().slope_ratio.unwrap() + 1.0).abs() < 1e-9);
        // a longer panel gives fewer members, undo brings them back
        d.edit_truss(t, &TrussSpec { spacing_m: 4.0, ..spec }).unwrap();
        assert_eq!(d.system_beams(t).len(), 6);
        d.undo().unwrap();
        assert_eq!(d.system_beams(t).len(), 10);
        assert!(d.edit_truss(t, &TrussSpec { spacing_m: 0.0, ..spec }).is_err());
        assert!(d.transact(d.revision(), &[Command::SetParent { id: kids[0], parent: Some(kids[1]) }]).is_err());
        d.delete_beam_system(t).unwrap();
        assert!(d.get(t).is_none() && d.get(kids[0]).is_none());
    }

    #[test]
    fn family_instances_stand_on_a_level_or_in_a_wall_and_follow_it() {
        use crate::{Command, FamilySpec};
        let mut d = Document::spatial_sample();
        let spec = FamilySpec { family: "Door, single flush".into(), type_name: String::new(), overrides: vec![("width".into(), 1.0)], angle_deg: 0.0, elevation_m: 0.0 };
        let wall = d.elements().find(|e| e.category == Category::Wall).unwrap().id;
        let Some(Geometry::Linear { start, end, .. }) = d.get(wall).unwrap().geometry else { panic!() };
        let len = (end[0] - start[0]).hypot(end[1] - start[1]);
        let door = d.place_family_in_wall(wall, len / 2.0, &spec).unwrap();
        // beyond the wall's end, or in something that is not a wall: refused
        assert!(d.place_family_in_wall(wall, len + 1.0, &spec).is_err());
        assert!(d.place_family_in_wall(ElementId(10), 1.0, &spec).is_err());
        // the wall cannot go from under it
        assert!(d.transact(d.revision(), &[Command::Delete { id: wall }]).is_err());
        // free standing on a level, turned
        let fcu = d.place_family(ElementId(10), [3.0, 3.0], &FamilySpec { family: "Fan coil unit, ducted".into(), angle_deg: 90.0, elevation_m: 2.4, ..spec.clone() }).unwrap();
        assert_eq!(d.get(fcu).unwrap().angle_deg, 90.0);
        assert!(d.place_family(ElementId(10), [f64::NAN, 0.0], &spec).is_err());
        assert!(d.place_family(ElementId(10), [0.0, 0.0], &FamilySpec { family: String::new(), ..spec.clone() }).is_err());
        assert!(d.mesh(door).is_none());
        // only families have a family
        assert!(d.transact(d.revision(), &[Command::SetFamily { id: wall, family: "x".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0 }]).is_err());
        d.transact(d.revision(), &[Command::SetFamily { id: door, family: "Door, single flush".into(), type_name: String::new(), overrides: vec![("width".into(), 1.2)], angle_deg: 0.0 }]).unwrap();
        assert_eq!(d.get(door).unwrap().overrides, vec![("width".to_string(), 1.2)]);
    }
}
