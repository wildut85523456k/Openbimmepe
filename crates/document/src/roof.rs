//! Roofs by footprint, as Revit makes them: the outline is at the eaves, and each edge that "defines slope" makes a
//! plane that rises from it at the roof's slope. The roof is the lowest of those planes (a hip roof when all edges
//! slope, a gable with two opposite ones, a shed with one, flat with none); an edge that does not slope is open at
//! its end, where a wall can be attached up to the roof.
use super::{Document, ElementId, Geometry, Material, Mesh};

type P = [f64; 2];

/// 30 degrees, Revit's default roof slope, as rise over run.
pub const DEFAULT_SLOPE: f64 = 0.577_350_269_189_625_7;

/// A roof to lay.
#[derive(Clone, Debug, PartialEq)]
pub struct RoofSpec {
    /// the footprint at the eaves, in plan
    pub outline: Vec<P>,
    /// thickness measured square to the slope (vertical when flat)
    pub thickness_m: f64,
    /// how far above its level the top of the roof is at the eaves
    pub base_offset_m: f64,
    /// rise over run of the sloping edges
    pub slope_ratio: f64,
    /// whether each edge defines a slope; empty means all of them
    pub edges: Vec<bool>,
    pub material: Material,
}

fn cross(o: P, a: P, b: P) -> f64 { (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]) }

/// Whether the polygon never turns the other way (a roof with sloping edges needs that: valleys are not made yet).
pub fn is_convex(r: &[P]) -> bool {
    let n = r.len();
    if n < 3 { return false; }
    let turns: Vec<f64> = (0..n).map(|i| cross(r[i], r[(i + 1) % n], r[(i + 2) % n])).collect();
    turns.iter().all(|t| *t >= -1e-9) || turns.iter().all(|t| *t <= 1e-9)
}

/// The same polygon running counter-clockwise, with the flags of its edges following it.
fn ccw(outline: &[P], edges: &[bool]) -> (Vec<P>, Vec<bool>) {
    let area: f64 = (0..outline.len()).map(|i| { let (a, b) = (outline[i], outline[(i + 1) % outline.len()]); a[0] * b[1] - b[0] * a[1] }).sum();
    if area >= 0.0 { return (outline.to_vec(), edges.to_vec()); }
    let n = outline.len();
    ((0..n).map(|k| outline[n - 1 - k]).collect(), (0..n).map(|k| edges[(2 * n - 2 - k) % n]).collect())
}

/// The edge flags after a polygon has been reversed (mirrored): edge k of the reversed polygon is edge n-2-k of the old one.
pub fn reversed_edges(edges: &[bool]) -> Vec<bool> {
    let n = edges.len();
    (0..n).map(|k| edges[(2 * n - 2 - k) % n]).collect()
}

/// Keeps the part of `poly` where a*x + b*y + c >= 0.
fn clip(poly: &[[f64; 3]], f: [f64; 3]) -> Vec<[f64; 3]> {
    let val = |p: &[f64; 3]| f[0] * p[0] + f[1] * p[1] + f[2];
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (va, vb) = (val(&a), val(&b));
        if va >= -1e-12 { out.push(a); }
        if (va > 1e-12 && vb < -1e-12) || (va < -1e-12 && vb > 1e-12) {
            let t = va / (va - vb);
            out.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]), 0.0]);
        }
    }
    out
}

/// The top surface of a roof as convex faces counter-clockwise from above, with `z0` the height at the eaves. None for an
/// outline that is not a polygon, or a roof with sloping edges on a footprint that is not convex.
pub fn top_faces(outline: &[P], edges: &[bool], slope: f64, z0: f64) -> Option<Vec<Vec<[f64; 3]>>> {
    if outline.len() < 3 || edges.len() != outline.len() { return None; }
    let (ring, edges) = ccw(outline, edges);
    let n = ring.len();
    let sloping: Vec<usize> = (0..n).filter(|&i| edges[i]).collect();
    if sloping.is_empty() || slope == 0.0 {
        return Some(vec![ring.iter().map(|p| [p[0], p[1], z0]).collect()]);
    }
    if !is_convex(&ring) { return None; }
    // the distance inward from edge i is the linear function d_i = a x + b y + c
    let dist = |i: usize| -> [f64; 3] {
        let (p, q) = (ring[i], ring[(i + 1) % n]);
        let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
        let l = dx.hypot(dy);
        let (nx, ny) = (-dy / l, dx / l);
        [nx, ny, -(nx * p[0] + ny * p[1])]
    };
    let mut faces = Vec::new();
    for &i in &sloping {
        let di = dist(i);
        let mut poly: Vec<[f64; 3]> = ring.iter().map(|p| [p[0], p[1], 0.0]).collect();
        for &j in &sloping {
            if j == i { continue; }
            let dj = dist(j);
            let g = [dj[0] - di[0], dj[1] - di[1], dj[2] - di[2]];
            if g.iter().all(|v| v.abs() < 1e-12) { if j < i { poly.clear(); } continue; }
            poly = clip(&poly, g);
            if poly.len() < 3 { break; }
        }
        if poly.len() < 3 { continue; }
        let area: f64 = (0..poly.len()).map(|k| { let (a, b) = (poly[k], poly[(k + 1) % poly.len()]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>() / 2.0;
        if area < 1e-9 { continue; }
        for p in &mut poly { p[2] = z0 + slope * (di[0] * p[0] + di[1] * p[1] + di[2]); }
        faces.push(poly);
    }
    (!faces.is_empty()).then_some(faces)
}

/// The solid of a roof: its top faces, the same faces `drop` lower, and a strip down each edge of the footprint.
/// All faces are wound outward.
pub fn mesh(outline: &[P], edges: &[bool], slope: f64, z0: f64, thickness: f64) -> Option<Mesh> {
    let faces = top_faces(outline, edges, slope, z0)?;
    let (ring, _) = ccw(outline, edges);
    let drop = thickness * (1.0 + slope * slope).sqrt();
    let on_boundary = |a: [f64; 3], b: [f64; 3]| {
        (0..ring.len()).any(|k| {
            let (p, q) = (ring[k], ring[(k + 1) % ring.len()]);
            cross(p, q, [a[0], a[1]]).abs() < 1e-9 * (q[0] - p[0]).hypot(q[1] - p[1]).max(1.0) && cross(p, q, [b[0], b[1]]).abs() < 1e-9 * (q[0] - p[0]).hypot(q[1] - p[1]).max(1.0)
        })
    };
    let (mut vertices, mut out) = (Vec::new(), Vec::new());
    for f in &faces {
        let base = vertices.len();
        vertices.extend(f.iter().copied());
        out.push((base..base + f.len()).collect::<Vec<_>>());
        let bb = vertices.len();
        vertices.extend(f.iter().map(|p| [p[0], p[1], p[2] - drop]));
        out.push((bb..bb + f.len()).rev().collect());
        for k in 0..f.len() {
            let j = (k + 1) % f.len();
            if on_boundary(f[k], f[j]) { out.push(vec![bb + k, bb + j, base + j, base + k]); }
        }
    }
    Some(Mesh { vertices, faces: out })
}

/// What a roof measures, from its shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoofFigures {
    /// the area of its top surface (square to the slope), m²
    pub surface_m2: f64,
    /// the area of its footprint in plan, m²
    pub plan_m2: f64,
    /// how far the highest point is above the eaves, m
    pub rise_m: f64,
}

impl Document {
    /// The figures of the roof `id`: None for anything else, or a roof that has no shape.
    pub fn roof_figures(&self, id: ElementId) -> Option<RoofFigures> {
        let el = self.get(id).filter(|e| e.is_roof())?;
        let Some(Geometry::Area { level }) = el.geometry else { return None };
        let z0 = self.get(level)?.elevation_m + el.elevation_m;
        let faces = top_faces(&el.outline, &el.slope_edges, el.slope_ratio.unwrap_or(0.0), z0)?;
        let mut surface = 0.0;
        for f in &faces {
            let mut s = [0.0; 3];
            for k in 0..f.len() {
                let (a, b) = (f[k], f[(k + 1) % f.len()]);
                s[0] += a[1] * b[2] - a[2] * b[1]; s[1] += a[2] * b[0] - a[0] * b[2]; s[2] += a[0] * b[1] - a[1] * b[0];
            }
            surface += (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt() / 2.0;
        }
        let rise = faces.iter().flatten().map(|p| p[2] - z0).fold(0.0, f64::max);
        Some(RoofFigures { surface_m2: surface, plan_m2: openbim_geom::profile::polygon_area(&el.outline).abs(), rise_m: rise })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 }
    fn area3(f: &[[f64; 3]]) -> f64 {
        // true (3D) area of a convex polygon
        let mut s = [0.0; 3];
        for k in 0..f.len() {
            let (a, b) = (f[k], f[(k + 1) % f.len()]);
            s[0] += a[1] * b[2] - a[2] * b[1]; s[1] += a[2] * b[0] - a[0] * b[2]; s[2] += a[0] * b[1] - a[1] * b[0];
        }
        (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt() / 2.0
    }
    fn rect(w: f64, d: f64) -> Vec<P> { vec![[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]] }
    fn top(faces: &[Vec<[f64; 3]>]) -> f64 { faces.iter().flatten().map(|p| p[2]).fold(f64::MIN, f64::max) }

    #[test]
    fn a_hip_roof_rises_to_a_ridge_half_the_short_side_in_from_the_eaves() {
        // 10 x 6 with 45 degrees: the ridge is 3 m up, 4 m long, and there are four faces
        let f = top_faces(&rect(10.0, 6.0), &[true; 4], 1.0, 3.0).unwrap();
        assert_eq!(f.len(), 4);
        assert!(near(top(&f), 6.0));
        // plan areas add to the footprint; true areas are the plan areas times sqrt(2)
        let plan: f64 = f.iter().map(|p| area3(&p.iter().map(|v| [v[0], v[1], 0.0]).collect::<Vec<_>>())).sum();
        assert!(near(plan, 60.0));
        assert!(near(f.iter().map(|p| area3(p)).sum::<f64>(), 60.0 * 2f64.sqrt()));
        // the ridge runs from (3,3) to (7,3)
        let ridge: Vec<&[f64; 3]> = f.iter().flatten().filter(|p| near(p[2], 6.0)).collect();
        assert!(ridge.iter().all(|p| near(p[1], 3.0) && (near(p[0], 3.0) || near(p[0], 7.0))));
    }

    #[test]
    fn two_opposite_sloping_edges_make_a_gable_one_a_shed_none_a_flat_roof() {
        let gable = top_faces(&rect(10.0, 6.0), &[true, false, true, false], 0.5, 0.0).unwrap();
        assert_eq!(gable.len(), 2);
        assert!(near(top(&gable), 1.5));
        // a ridge along the middle, 3 m from each sloping edge
        assert!(gable.iter().flatten().filter(|p| near(p[2], 1.5)).all(|p| near(p[1], 3.0)));
        let shed = top_faces(&rect(10.0, 6.0), &[true, false, false, false], 0.5, 0.0).unwrap();
        assert_eq!(shed.len(), 1);
        assert!(near(top(&shed), 3.0));
        let flat = top_faces(&rect(10.0, 6.0), &[false; 4], 0.5, 2.0).unwrap();
        assert_eq!((flat.len(), top(&flat)), (1, 2.0));
        // a footprint with a reflex corner cannot slope yet, but can be flat
        let l = vec![[0.0, 0.0], [6.0, 0.0], [6.0, 3.0], [3.0, 3.0], [3.0, 6.0], [0.0, 6.0]];
        assert!(top_faces(&l, &[true; 6], 0.5, 0.0).is_none());
        assert!(top_faces(&l, &[false; 6], 0.5, 0.0).is_some());
    }

    #[test]
    fn the_same_roof_comes_out_whichever_way_the_outline_runs() {
        let cw: Vec<P> = rect(10.0, 6.0).into_iter().rev().collect();
        // the edges of the clockwise outline that are the long ones: (10,6)->(10,0) is short ... flags follow their edges
        let edges_ccw = [true, false, true, false]; // the long sides (bottom and top) slope
        let edges_cw = reversed_edges(&edges_ccw);
        let a = top_faces(&rect(10.0, 6.0), &edges_ccw, 0.5, 0.0).unwrap();
        let b = top_faces(&cw, &edges_cw, 0.5, 0.0).unwrap();
        assert_eq!((a.len(), b.len()), (2, 2));
        assert!(near(top(&a), top(&b)) && near(top(&b), 1.5));
    }

    #[test]
    fn the_solid_is_closed_with_outward_faces() {
        let m = mesh(&rect(10.0, 6.0), &[true; 4], 0.5, 3.0, 0.2).unwrap();
        // every directed edge of the faces is met by the opposite edge of another face
        let key = |p: [f64; 3]| ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64, (p[2] * 1e6).round() as i64);
        let mut seen = std::collections::HashMap::new();
        for f in &m.faces { for k in 0..f.len() { let (a, b) = (key(m.vertices[f[k]]), key(m.vertices[f[(k + 1) % f.len()]])); *seen.entry((a, b)).or_insert(0) += 1; } }
        for ((a, b), n) in &seen { assert_eq!(*n, 1); assert!(seen.contains_key(&(*b, *a)), "an open edge {a:?}->{b:?}"); }
        // the outward normals: the volume by the divergence theorem is positive and equals footprint x thickness x sqrt(1+s^2) (approximately: the hip planes)
        let mut vol = 0.0;
        for f in &m.faces {
            for k in 1..f.len() - 1 {
                let (a, b, c) = (m.vertices[f[0]], m.vertices[f[k]], m.vertices[f[k + 1]]);
                vol += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0;
            }
        }
        assert!((vol - 60.0 * 0.2 * (1.25f64).sqrt()).abs() < 1e-6, "{vol}");
    }

    #[test]
    fn a_roof_is_an_element_that_is_saved_moved_mirrored_and_undone() {
        use crate::{Document, ElementId, Xf};
        let mut d = Document::spatial_sample();
        let level = ElementId(10);
        let spec = RoofSpec { outline: rect(10.0, 6.0), thickness_m: 0.2, base_offset_m: 3.0, slope_ratio: 0.5, edges: vec![true, false, true, false], material: Material::Timber };
        let id = d.place_roof(level, &spec).unwrap();
        let el = d.get(id).unwrap().clone();
        assert!(el.is_roof() && el.slope_ratio == Some(0.5) && el.slope_edges == [true, false, true, false] && el.name == "Roof 1");
        // the mesh is the gable roof: ridge 1.5 m above the eaves, which are at the level + 3 m
        let m = d.mesh(id).unwrap();
        let top = m.vertices.iter().map(|v| v[2]).fold(f64::MIN, f64::max);
        assert!((top - (d.get(level).unwrap().elevation_m + 3.0 + 1.5)).abs() < 1e-9, "{top}");
        let f = d.roof_figures(id).unwrap();
        // 10 x 6 gable at 0.5: ridge 1.5 m, surface = plan x sqrt(1 + 0.5^2)
        assert!((f.rise_m - 1.5).abs() < 1e-9 && (f.plan_m2 - 60.0).abs() < 1e-9 && (f.surface_m2 - 60.0 * 1.25f64.sqrt()).abs() < 1e-9, "{f:?}");
        // a hip roof is refused on an outline with a reflex corner, a flat one is not
        let l = vec![[0.0, 0.0], [6.0, 0.0], [6.0, 3.0], [3.0, 3.0], [3.0, 6.0], [0.0, 6.0]];
        assert!(d.place_roof(level, &RoofSpec { outline: l.clone(), edges: vec![], ..spec.clone() }).is_err());
        assert!(d.place_roof(level, &RoofSpec { outline: l, edges: vec![false; 6], ..spec.clone() }).is_ok());
        assert!(d.place_roof(level, &RoofSpec { slope_ratio: 9.0, ..spec.clone() }).is_err());
        // saved natively and restored
        let (els, site) = crate::native::parse_records(&crate::native::records(&d)).unwrap();
        let mut back = Document::new();
        back.transact(back.revision(), &crate::native::restore_commands(els, site)).unwrap();
        assert_eq!(back.get(id).unwrap().slope_edges, el.slope_edges);
        assert_eq!(back.get(id).unwrap().slope_ratio, el.slope_ratio);
        // moved: the shape is kept; mirrored: the sloping edges are still the long ones
        let ops = d.transform_commands(&[id], Xf::Move([2.0, 1.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        assert_eq!(d.get(id).unwrap().slope_edges, [true, false, true, false]);
        let ops = d.transform_commands(&[id], Xf::Mirror([0.0, 0.0], [0.0, 1.0])).unwrap();
        d.transact(d.revision(), &ops).unwrap();
        let r = d.get(id).unwrap();
        let long: Vec<bool> = (0..4).map(|k| { let (a, b) = (r.outline[k], r.outline[(k + 1) % 4]); (b[0] - a[0]).abs() > 5.0 }).collect();
        assert_eq!(r.slope_edges, long);
        assert!((d.mesh(id).unwrap().vertices.iter().map(|v| v[2]).fold(f64::MIN, f64::max) - top).abs() < 1e-9);
        // a roof turned back into a floor loses its slope
        d.transact(d.revision(), &[crate::Command::SetSlabKind { id, kind: crate::SlabKind::Floor }]).unwrap();
        assert!(d.get(id).unwrap().slope_ratio.is_none() && d.get(id).unwrap().slope_edges.is_empty());
        d.undo().unwrap();
        assert!(d.get(id).unwrap().is_roof());
    }
}
