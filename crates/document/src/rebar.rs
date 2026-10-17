//! Reinforcement of a rectangular beam: bottom bars, two top bars and closed stirrups, derived from the beam as
//! it is now, so that the bars follow it when it moves, is cut back by a column or changes length.
use crate::{Category, Document, Element, ElementId, Geometry, Mesh, Section};

/// The bars of a rebar element: a polyline in model coordinates, its diameter, and whether it is a stirrup.
#[derive(Clone, Debug, PartialEq)]
pub struct BarPath {
    pub path: Vec<[f64; 3]>,
    pub diameter: f64,
    pub shear: bool,
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn mul(a: [f64; 3], k: f64) -> [f64; 3] { [a[0] * k, a[1] * k, a[2] * k] }
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn unit(a: [f64; 3]) -> Option<[f64; 3]> {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    (l > 1e-9).then(|| mul(a, 1.0 / l))
}

impl Document {
    /// The bars of the rebar `el` in its host beam: `count` bars along the bottom and two along the top, each
    /// `width_m` thick, closed stirrups `height_m` thick every `spacing_m`, all `elevation_m` (the cover) inside
    /// the beam. None when the host is missing, is not a rectangular beam, or the bars do not fit in it.
    pub fn rebar_bars(&self, el: &Element) -> Option<Vec<BarPath>> {
        let (Category::Rebar, Some(Geometry::OnMember { host })) = (el.category, el.geometry) else { return None };
        let beam = self.get(host).filter(|b| b.category == Category::Beam && b.section == Section::Rectangle)?;
        let ext = self.beam_extent(beam)?;
        let (dm, ds, cover) = (el.width_m, el.height_m, el.elevation_m);
        let n = el.count as usize;
        let u = unit(sub(ext.b, ext.a))?;
        let side = unit(cross([0.0, 0.0, 1.0], u))?;
        let up = cross(u, side);
        // stirrup centre lines and bar centres, measured from the section's centre
        let (hw, hd) = (beam.width_m / 2.0 - cover - ds / 2.0, beam.height_m / 2.0 - cover - ds / 2.0);
        let (bw, bd) = (hw - ds / 2.0 - dm / 2.0, hd - ds / 2.0 - dm / 2.0);
        let length = ext.cut_length();
        if !(2..=20).contains(&n) || bw <= 0.0 || bd <= 0.0 || hw <= 0.0 || hd <= 0.0 || length <= 2.0 * cover + 1e-6 { return None; }
        if n > 1 && 2.0 * bw / (n as f64 - 1.0) < dm * 1.0 + 1e-9 { return None; }
        let at = |s: f64, y: f64, z: f64| add(add(add(ext.a, mul(u, s)), mul(side, y)), mul(up, z));
        let mut bars = Vec::new();
        let (s0, s1) = (cover, length - cover);
        for i in 0..n {
            let y = -bw + 2.0 * bw * i as f64 / (n as f64 - 1.0);
            bars.push(BarPath { path: vec![at(s0, y, -bd), at(s1, y, -bd)], diameter: dm, shear: false });
        }
        for y in [-bw, bw] {
            bars.push(BarPath { path: vec![at(s0, y, bd), at(s1, y, bd)], diameter: dm, shear: false });
        }
        let stations = ((s1 - s0 - ds) / el.spacing_m + 1e-9).floor() as usize + 1;
        if stations > 2000 { return None; }
        for k in 0..stations {
            let s = s0 + ds / 2.0 + k as f64 * el.spacing_m;
            bars.push(BarPath { path: vec![at(s, -hw, -hd), at(s, hw, -hd), at(s, hw, hd), at(s, -hw, hd), at(s, -hw, -hd)], diameter: ds, shear: true });
        }
        Some(bars)
    }

    /// Whether the rebar `el` fits in its host.
    pub(crate) fn rebar_fits(&self, el: &Element) -> bool { self.rebar_bars(el).is_some() }

    /// The rebar elements hosted by `beam`.
    pub fn rebars_of(&self, beam: ElementId) -> Vec<ElementId> {
        self.elements().filter(|e| e.geometry == Some(Geometry::OnMember { host: beam })).map(|e| e.id).collect()
    }

    /// Every bar as a square prism of its diameter, in one mesh.
    pub(crate) fn rebar_mesh(&self, el: &Element) -> Option<Mesh> {
        let mut mesh = Mesh { vertices: Vec::new(), faces: Vec::new() };
        for bar in self.rebar_bars(el)? {
            let r = bar.diameter / 2.0 * 0.886;
            for w in bar.path.windows(2) {
                let d = unit(sub(w[1], w[0]))?;
                let e1 = unit(cross(d, if d[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] }))?;
                let e2 = cross(d, e1);
                // e1, e2, d are right-handed: e2 = d x e1 gives e1 x e2 = d
                let ring = [add(mul(e1, r), mul(e2, r)), add(mul(e1, -r), mul(e2, r)), add(mul(e1, -r), mul(e2, -r)), add(mul(e1, r), mul(e2, -r))];
                let base = mesh.vertices.len();
                mesh.vertices.extend(ring.iter().map(|c| add(w[0], *c)));
                mesh.vertices.extend(ring.iter().map(|c| add(w[1], *c)));
                mesh.faces.push(vec![base + 3, base + 2, base + 1, base]);
                mesh.faces.push(vec![base + 4, base + 5, base + 6, base + 7]);
                for k in 0..4 { mesh.faces.push(vec![base + k, base + (k + 1) % 4, base + (k + 1) % 4 + 4, base + k + 4]); }
            }
        }
        Some(mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeamSpec, Command, RebarSpec};

    fn volume(m: &Mesh) -> f64 {
        m.faces.iter().map(|f| (1..f.len() - 1).map(|i| {
            let (a, b, c) = (m.vertices[f[0]], m.vertices[f[i]], m.vertices[f[i + 1]]);
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0
        }).sum::<f64>()).sum()
    }

    #[test]
    fn a_beam_is_reinforced_with_bars_and_stirrups_that_follow_it() {
        let mut d = Document::spatial_sample();
        let beam = d.place_beam(ElementId(10), &BeamSpec::level([1.0, 3.0], [7.0, 3.0], 0.3, 0.5, 2.8)).unwrap();
        let spec = RebarSpec { count: 3, bar_m: 0.016, stirrup_m: 0.008, cover_m: 0.04, stirrup_spacing_m: 0.2 };
        let r = d.place_rebar(beam, &spec).unwrap();
        let bars = d.rebar_bars(d.get(r).unwrap()).unwrap();
        // 3 bottom, 2 top and 30 stirrups
        assert_eq!((bars.len(), bars.iter().filter(|b| b.shear).count()), (35, 30));
        // every bar lies inside the beam, inside its cover
        for b in &bars {
            for p in &b.path {
                assert!(p[0] >= 1.04 - 1e-9 && p[0] <= 6.96 + 1e-9 && (p[1] - 3.0).abs() <= 0.15 - 0.04 + 1e-9 && (p[2] - 2.8).abs() <= 0.25 - 0.04 + 1e-9, "{p:?}");
            }
        }
        // the mesh is closed and outward-facing
        assert!(volume(&d.mesh(r).unwrap()) > 0.0);
        // a column at the end cuts the beam and the bars follow
        let before = bars[0].path[1][0];
        d.place_column(ElementId(10), ElementId(11), [7.0, 3.0], &crate::ColumnSpec { width_m: 0.4, depth_m: 0.4, base_offset_m: 0.0, section: crate::Section::Rectangle, material: crate::Material::Concrete }).unwrap();
        assert!(d.rebar_bars(d.get(r).unwrap()).unwrap()[0].path[1][0] < before - 0.1);
        // bars that do not fit are refused, and so is a rebar in anything but a beam
        assert!(d.place_rebar(beam, &RebarSpec { count: 20, ..spec }).is_err());
        assert!(d.place_rebar(beam, &RebarSpec { cover_m: 0.2, ..spec }).is_err());
        let wall = d.elements().find(|e| e.category == Category::Wall).unwrap().id;
        assert!(d.place_rebar(wall, &spec).is_err());
        // the bar count and the stirrup spacing are edited in place
        d.transact(d.revision(), &[Command::SetCount { id: r, count: 4 }, Command::SetBeamSystem { id: r, spacing_m: 0.4, angle_deg: 0.0 }]).unwrap();
        let bars = d.rebar_bars(d.get(r).unwrap()).unwrap();
        assert_eq!(bars.iter().filter(|b| !b.shear).count(), 6);
        assert!(bars.iter().filter(|b| b.shear).count() < 30);
        assert_eq!(d.rebars_of(beam), vec![r]);
    }
}
