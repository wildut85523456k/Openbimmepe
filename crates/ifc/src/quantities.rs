//! Base quantities of the elements, as the IFC `Qto_*BaseQuantities` sets name them. They are worked out from
//! the model, never typed in, and they are the quantities of the element as built: openings take away from the
//! net values. Lengths are metres, areas square metres, volumes cubic metres.
use openbim_arch::Room;
use openbim_geom::WallChain;
use openbim_place::floors::Slab;

type P = [f64; 2];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Length,
    Area,
    Volume,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Qty {
    pub name: &'static str,
    pub kind: Kind,
    pub value: f64,
}

fn q(name: &'static str, kind: Kind, value: f64) -> Qty {
    Qty { name, kind, value }
}

pub fn ring_area(p: &[P]) -> f64 {
    let n = p.len();
    (0..n).map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1]).sum::<f64>() / 2.0
}

pub fn ring_len(p: &[P]) -> f64 {
    let n = p.len();
    (0..n).map(|i| (p[(i + 1) % n][0] - p[i][0]).hypot(p[(i + 1) % n][1] - p[i][1])).sum()
}

fn convex(p: &[P]) -> bool {
    let n = p.len();
    let s = ring_area(p).signum();
    (0..n).all(|i| {
        let (a, b, c) = (p[i], p[(i + 1) % n], p[(i + 2) % n]);
        s * ((b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0])) >= -1e-12
    })
}

/// Area shared by two simple polygons, when at least one of them is convex (Sutherland and Hodgman); None otherwise.
pub fn overlap_area(a: &[P], b: &[P]) -> Option<f64> {
    if a.len() < 3 || b.len() < 3 {
        return Some(0.0);
    }
    let (subject, clip) = if convex(b) {
        (a, b)
    } else if convex(a) {
        (b, a)
    } else {
        return None;
    };
    let mut clip: Vec<P> = clip.to_vec();
    if ring_area(&clip) < 0.0 {
        clip.reverse();
    }
    let mut out: Vec<P> = subject.to_vec();
    for i in 0..clip.len() {
        let (c0, c1) = (clip[i], clip[(i + 1) % clip.len()]);
        let side = |p: P| (c1[0] - c0[0]) * (p[1] - c0[1]) - (c1[1] - c0[1]) * (p[0] - c0[0]);
        let input = std::mem::take(&mut out);
        for j in 0..input.len() {
            let (p, r) = (input[j], input[(j + 1) % input.len()]);
            let (sp, sr) = (side(p), side(r));
            if sp >= 0.0 {
                out.push(p);
            }
            if (sp >= 0.0) != (sr >= 0.0) {
                let t = sp / (sp - sr);
                out.push([p[0] + (r[0] - p[0]) * t, p[1] + (r[1] - p[1]) * t]);
            }
        }
        if out.is_empty() {
            return Some(0.0);
        }
    }
    Some(ring_area(&out).abs())
}

/// `Qto_WallBaseQuantities` of a wall chain. Flat tops and rectangular openings only.
/// `round_credit` is the area (m2) the round openings leave in the wall compared with their bounding rectangles.
pub fn wall(c: &WallChain, round_credit: f64) -> Result<Vec<Qty>, String> {
    if c.tops.iter().any(|t| t.is_some()) {
        return Err("sloped wall tops".into());
    }
    if c.poly_openings.iter().any(|v| !v.is_empty()) {
        return Err("polygonal openings".into());
    }
    let plans = c.plans().map_err(|e| format!("{e:?}"))?;
    let n = c.points.len();
    let segs = if c.closed { n } else { n - 1 };
    let length: f64 = (0..segs).map(|i| (c.points[(i + 1) % n][0] - c.points[i][0]).hypot(c.points[(i + 1) % n][1] - c.points[i][1])).sum();
    let footprint: f64 = plans.iter().map(|p| ring_area(&[p.l0, p.l1, p.r1, p.r0]).abs()).sum();
    let open_area: f64 = c.openings.iter().flatten().map(|o| o.width * o.height).sum::<f64>() - round_credit;
    let gross_side = length * c.height;
    Ok(vec![
        q("Length", Kind::Length, length),
        q("Width", Kind::Length, c.thickness),
        q("Height", Kind::Length, c.height),
        q("GrossFootprintArea", Kind::Area, footprint),
        q("NetFootprintArea", Kind::Area, footprint),
        q("GrossSideArea", Kind::Area, gross_side),
        q("NetSideArea", Kind::Area, gross_side - open_area),
        q("GrossVolume", Kind::Volume, footprint * c.height),
        q("NetVolume", Kind::Volume, footprint * c.height - open_area * c.thickness),
    ])
}

/// `Qto_SlabBaseQuantities`. `void_area` is the plan area of everything cut through the slab.
pub fn slab(s: &Slab, void_area: f64) -> Vec<Qty> {
    let gross = ring_area(&s.outline).abs();
    let net = gross - void_area;
    vec![
        q("Depth", Kind::Length, s.thickness),
        q("Perimeter", Kind::Length, ring_len(&s.outline)),
        q("GrossArea", Kind::Area, gross),
        q("NetArea", Kind::Area, net),
        q("GrossVolume", Kind::Volume, gross * s.thickness),
        q("NetVolume", Kind::Volume, net * s.thickness),
    ]
}

/// `Qto_DoorBaseQuantities` and `Qto_WindowBaseQuantities`: the clear opening the element fills.
pub fn opening_item(width: f64, height: f64) -> Vec<Qty> {
    vec![q("Width", Kind::Length, width), q("Height", Kind::Length, height), q("Area", Kind::Area, width * height)]
}

/// `Qto_SpaceBaseQuantities` of a room: gross is inside the outer outline, net takes away what stands in the room.
pub fn space(r: &Room) -> Vec<Qty> {
    let gross = ring_area(&r.outline).abs();
    let net = r.area();
    let holes: f64 = r.holes.iter().map(|h| ring_len(h)).sum();
    vec![
        q("Height", Kind::Length, r.height),
        q("GrossPerimeter", Kind::Length, ring_len(&r.outline)),
        q("NetPerimeter", Kind::Length, ring_len(&r.outline) + holes),
        q("GrossFloorArea", Kind::Area, gross),
        q("NetFloorArea", Kind::Area, net),
        q("GrossVolume", Kind::Volume, gross * r.height),
        q("NetVolume", Kind::Volume, net * r.height),
    ]
}
