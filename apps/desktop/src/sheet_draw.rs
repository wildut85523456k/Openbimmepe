//! What a sheet shows, as vector shapes in millimetres on paper: the paper's frame and title block, and the views and
//! schedules placed on it, each drawn at its scale. The same shapes are painted on the screen (the sheet view) and
//! written to the PDF, so what is printed is what was seen.
//!
//! A plan comes from the view engine (cut walls with their openings, concrete hatched, what is above the cut plane
//! dashed), with the ducts, pipes, beams, stairs and railings the engine does not draw added from their meshes, and the
//! grids, dimensions, tags, section marks and room tags on top. An elevation or a section is the engine's view. The
//! 3D view is an isometric view of the model, shaded. A schedule is its table.
use crate::pdf::{text_width_mm, Align, Page, Rgb};
use crate::View;
use openbim_document::{Category, Document, ElementId, Geometry, SheetDef};
use std::collections::BTreeMap;

/// the height of annotation text on paper, as Revit's default text (2.5 mm)
pub const NOTE_MM: f64 = 2.5;

/// One shape on paper.
#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    /// closed rings, filled even-odd (an inner ring is a hole)
    Poly { rings: Vec<Vec<[f64; 2]>>, fill: Option<Rgb>, stroke: Option<(Rgb, f64)> },
    Line { a: [f64; 2], b: [f64; 2], rgb: Rgb, w: f64, dash: Option<(f64, f64)> },
    Circle { c: [f64; 2], r: f64, fill: Option<Rgb>, stroke: Option<(Rgb, f64)> },
    Text { at: [f64; 2], size: f64, text: String, align: Align, rgb: Rgb, bold: bool, degrees: f64 },
}

fn grow(b: &mut Option<[f64; 4]>, p: [f64; 2]) {
    if !(p[0].is_finite() && p[1].is_finite()) { return; }
    *b = Some(match *b { None => [p[0], p[1], p[0], p[1]], Some(o) => [o[0].min(p[0]), o[1].min(p[1]), o[2].max(p[0]), o[3].max(p[1])] });
}

impl Prim {
    /// `[min x, min y, max x, max y]`
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut b = None;
        match self {
            Prim::Poly { rings, .. } => rings.iter().flatten().for_each(|p| grow(&mut b, *p)),
            Prim::Line { a, b: e, .. } => { grow(&mut b, *a); grow(&mut b, *e); }
            Prim::Circle { c, r, .. } => { grow(&mut b, [c[0] - r, c[1] - r]); grow(&mut b, [c[0] + r, c[1] + r]); }
            Prim::Text { at, size, text, align, bold, degrees, .. } => {
                let w = text_width_mm(text, *size, *bold);
                let (s, c) = degrees.to_radians().sin_cos();
                let back = match align { Align::Left => 0.0, Align::Center => w / 2.0, Align::Right => w };
                // the baseline runs from -back to w - back along the direction of the text, the text stands above it
                for (along, up) in [(-back, 0.0), (w - back, 0.0), (-back, *size), (w - back, *size)] {
                    grow(&mut b, [at[0] + along * c - up * s, at[1] - along * s - up * c]);
                }
            }
        }
        b
    }

    pub fn shifted(&self, dx: f64, dy: f64) -> Prim {
        let mv = |p: &[f64; 2]| [p[0] + dx, p[1] + dy];
        match self {
            Prim::Poly { rings, fill, stroke } => Prim::Poly { rings: rings.iter().map(|r| r.iter().map(mv).collect()).collect(), fill: *fill, stroke: *stroke },
            Prim::Line { a, b, rgb, w, dash } => Prim::Line { a: mv(a), b: mv(b), rgb: *rgb, w: *w, dash: *dash },
            Prim::Circle { c, r, fill, stroke } => Prim::Circle { c: mv(c), r: *r, fill: *fill, stroke: *stroke },
            Prim::Text { at, size, text, align, rgb, bold, degrees } => Prim::Text { at: mv(at), size: *size, text: text.clone(), align: *align, rgb: *rgb, bold: *bold, degrees: *degrees },
        }
    }

    /// Writes the shape to a page, offset by (`dx`, `dy`); `bw` prints in black and white (grays).
    pub fn paint(&self, page: &mut Page, dx: f64, dy: f64, bw: bool) {
        let ink = |c: Rgb| -> Rgb { if bw { let g = (0.3 * f64::from(c[0]) + 0.59 * f64::from(c[1]) + 0.11 * f64::from(c[2])) as u8; [g, g, g] } else { c } };
        let line = |c: Rgb| -> Rgb { if bw { let g = ink(c)[0]; if g < 140 { [0, 0, 0] } else { [g / 2, g / 2, g / 2] } } else { c } };
        match self.shifted(dx, dy) {
            Prim::Poly { rings, fill, stroke } => page.polygon(&rings, fill.map(ink), stroke.map(|(c, w)| (line(c), w))),
            Prim::Line { a, b, rgb, w, dash } => page.line(a, b, line(rgb), w, dash),
            Prim::Circle { c, r, fill, stroke } => page.circle(c, r, fill.map(ink), stroke.map(|(col, w)| (line(col), w))),
            Prim::Text { at, size, text, align, rgb, bold, degrees } => page.text_rotated(at, size, &text, align, line(rgb), bold, degrees),
        }
    }
}

pub const BLACK: Rgb = [0, 0, 0];
pub const GRAY: Rgb = [90, 90, 90];
pub const WHITE: Rgb = [255, 255, 255];

/// What one view or schedule looks like on paper, with its top left corner at the origin.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawing {
    pub prims: Vec<Prim>,
    pub w: f64,
    pub h: f64,
}

impl Drawing {
    /// Moves the shapes so that their box starts at the origin.
    pub fn from_prims(prims: Vec<Prim>) -> Drawing {
        let mut b = None;
        for p in &prims { if let Some(pb) = p.bounds() { grow(&mut b, [pb[0], pb[1]]); grow(&mut b, [pb[2], pb[3]]); } }
        let Some(b) = b else { return Drawing { prims: Vec::new(), w: 0.0, h: 0.0 } };
        Drawing { prims: prims.iter().map(|p| p.shifted(-b[0], -b[1])).collect(), w: b[2] - b[0], h: b[3] - b[1] }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// the views

/// The level a plan key stands for. `mech`, `p10` and `p11` are view keys, not level ids: they are the first two
/// plan views and the mechanical plan, so their level is the document's own (A2/R1). A `pN` key names a level.
pub(crate) fn plan_level_id(doc: &Document, key: &str) -> Option<ElementId> {
    let levels = |n: usize| -> Option<ElementId> {
        let mut l: Vec<(ElementId, f64)> = doc.elements().filter(|e| e.geometry == Some(Geometry::Level)).map(|e| (e.id, e.elevation_m)).collect();
        l.sort_by(|a, b| a.1.total_cmp(&b.1));
        l.get(n).map(|(id, _)| *id)
    };
    match key {
        "mech" | "p10" => return levels(0),
        "p11" => return levels(1).or_else(|| levels(0)),
        _ => {}
    }
    key.strip_prefix('p')?.parse().ok().map(ElementId)
}

/// The name Revit gives the view on the sheet and in the browser.
pub fn view_title(doc: &Document, key: &str) -> String {
    if let Some(id) = openbim_document::schedule_ref(key) { return doc.schedule(id).map_or("Schedule".into(), |s| s.name.clone()); }
    match crate::view_ui::view_from_key(key) {
        Some(View::Plan | View::UpperPlan | View::Level(_)) => plan_level_id(doc, key).and_then(|l| doc.get(l)).map_or("Plan".into(), |l| l.name.clone()),
        Some(View::Mechanical) => "M-101 - Mechanical".into(),
        Some(View::ThreeD) => "{3D}".into(),
        Some(View::Elevation(k)) => crate::section_ui::FACINGS.get(usize::from(k)).map_or("Elevation".into(), |f| format!("{} elevation", f.1)),
        Some(View::Section(id)) => doc.get(ElementId(id)).map_or("Section".into(), |e| e.name.clone()),
        Some(View::Marked(id, k)) => crate::section_ui::name_of_marker(&doc.get(ElementId(id)).map_or("Elevation".to_string(), |e| e.name.clone()), k),
        _ => key.to_string(),
    }
}

/// The view or schedule `key` drawn on paper at the scale of its settings (a schedule has no scale).
pub fn view_drawing(doc: &Document, key: &str) -> Result<Drawing, String> {
    if let Some(id) = openbim_document::schedule_ref(key) { return schedule_drawing(doc, id); }
    let view = crate::view_ui::view_from_key(key).ok_or("this view is not one a sheet can show")?;
    let vs = doc.view_settings(key);
    // what the view hides is left out of the model it is made from
    let hides = !vs.hidden_categories.is_empty() || !vs.hidden_elements.is_empty();
    let filtered;
    let seen = if hides { filtered = doc.filtered(&|e| !vs.hides(e)); &filtered } else { doc };
    let prims = match view {
        View::Plan | View::UpperPlan | View::Level(_) | View::Mechanical => plan_prims(seen, key, &vs)?,
        View::ThreeD => iso_prims(seen, &vs)?,
        View::Elevation(_) | View::Section(_) | View::Marked(..) => section_prims(seen, view, &vs)?,
        View::Schedule(_) => return Err("a schedule is placed by its number".into()),
        View::Sheet(_) => return Err("a sheet cannot be placed on a sheet".into()),
    };
    if prims.is_empty() { return Err("the view has nothing to show".into()); }
    Ok(Drawing::from_prims(prims))
}

/// Millimetres on paper for a metre of the model at the scale of the view.
fn per_metre(vs: &openbim_document::ViewSettings) -> f64 { 1000.0 / f64::from(vs.scale.max(1)) }

fn blend(c: Rgb, t: f64) -> Rgb { let f = |v: u8| (f64::from(v) + (255.0 - f64::from(v)) * (1.0 - t)).round().clamp(0.0, 255.0) as u8; [f(c[0]), f(c[1]), f(c[2])] }

/// Lines at 45 degrees, `spacing` apart, inside the rings (an inner ring is a hole).
fn hatch(rings: &[Vec<[f64; 2]>], spacing: f64, rgb: Rgb, w: f64) -> Vec<Prim> {
    let mut out = Vec::new();
    let mut b = None;
    rings.iter().flatten().for_each(|p| grow(&mut b, *p));
    let Some(b) = b else { return out };
    if spacing <= 0.0 || !spacing.is_finite() { return out; }
    // lines x + y = c; where one crosses an edge x + y = c meets it
    let (c0, c1) = (b[0] + b[1], b[2] + b[3]);
    let n = ((c1 - c0) / spacing).ceil().min(2000.0) as usize;
    for k in 1..n {
        let c = c0 + k as f64 * spacing;
        let mut hits: Vec<f64> = Vec::new();
        for ring in rings {
            for i in 0..ring.len() {
                let (p, q) = (ring[i], ring[(i + 1) % ring.len()]);
                let (sp, sq) = (p[0] + p[1] - c, q[0] + q[1] - c);
                if (sp < 0.0) != (sq < 0.0) {
                    let t = sp / (sp - sq);
                    hits.push(p[0] + (q[0] - p[0]) * t);
                }
            }
        }
        hits.sort_by(f64::total_cmp);
        for pair in hits.as_chunks::<2>().0 {
            out.push(Prim::Line { a: [pair[0], c - pair[0]], b: [pair[1], c - pair[1]], rgb, w, dash: None });
        }
    }
    out
}

fn arc_points(c: [f64; 2], r: f64, a0: f64, a1: f64) -> Vec<[f64; 2]> {
    let sweep = (a1 - a0).rem_euclid(360.0);
    let sweep = if sweep == 0.0 { 360.0 } else { sweep };
    let n = ((sweep / 7.5).ceil() as usize).max(4);
    (0..=n).map(|i| { let a = (a0 + sweep * i as f64 / n as f64).to_radians(); [c[0] + r * a.cos(), c[1] + r * a.sin()] }).collect()
}

fn push_polyline(out: &mut Vec<Prim>, pts: &[[f64; 2]], rgb: Rgb, w: f64) {
    for p in pts.windows(2) { out.push(Prim::Line { a: p[0], b: p[1], rgb, w, dash: None }); }
}

fn text(out: &mut Vec<Prim>, at: [f64; 2], size: f64, s: &str, align: Align, rgb: Rgb, bold: bool) {
    out.push(Prim::Text { at, size, text: s.to_string(), align, rgb, bold, degrees: 0.0 });
}

/// The items of the view engine as shapes: `p` maps a model point (metres) to paper.
fn engine_items(out: &mut Vec<Prim>, items: &[openbim_view::Item], p: &dyn Fn([f64; 2]) -> [f64; 2]) {
    use openbim_view::{Class, Kind, Shape};
    for it in items {
        let (fill, line_rgb, w, dash): (Option<Rgb>, Rgb, f64, Option<(f64, f64)>) = match (it.class, it.element.kind) {
            (Class::Cut, Kind::Wall) => (Some(WHITE), BLACK, 0.35, None),
            (Class::Cut, _) => (Some(WHITE), BLACK, 0.35, None),
            (Class::Below, _) => (Some(WHITE), [40, 40, 40], 0.13, None),
            (Class::Overhead, _) => (None, [90, 90, 90], 0.13, Some((2.0, 1.5))),
            (Class::Beyond, _) => (None, [150, 150, 150], 0.09, None),
            (Class::Symbol, _) => (None, [30, 30, 30], 0.13, None),
            (Class::Room, Kind::SeparationLine) => (None, [122, 90, 166], 0.13, None),
            (Class::Room, _) => (None, WHITE, 0.0, None),
            (Class::Datum, _) => (None, [70, 130, 200], 0.13, Some((4.0, 2.0))),
        };
        let concrete = matches!(it.element.kind, Kind::Column | Kind::Slab | Kind::Roof);
        match &it.shape {
            Shape::Region { outer, holes } => {
                if it.class == Class::Room { continue; }
                let rings: Vec<Vec<[f64; 2]>> = std::iter::once(outer).chain(holes.iter()).map(|r| r.iter().map(|q| p(*q)).collect()).collect();
                out.push(Prim::Poly { rings: rings.clone(), fill, stroke: Some((line_rgb, w)) });
                if concrete && it.class == Class::Cut { out.extend(hatch(&rings, 2.0, [60, 60, 60], 0.08)); }
            }
            Shape::Fill(ring) => {
                let ring: Vec<[f64; 2]> = ring.iter().map(|q| p(*q)).collect();
                out.push(Prim::Poly { rings: vec![ring.clone()], fill, stroke: Some((line_rgb, w)) });
                if concrete && it.class == Class::Cut { out.extend(hatch(&[ring], 2.0, [60, 60, 60], 0.08)); }
            }
            Shape::Line(a, b) => out.push(Prim::Line { a: p(*a), b: p(*b), rgb: line_rgb, w, dash }),
            Shape::Arc { c, r, a0, a1 } => {
                let pts: Vec<[f64; 2]> = arc_points(*c, *r, *a0, *a1).into_iter().map(p).collect();
                push_polyline(out, &pts, line_rgb, w);
            }
            Shape::Circle { c, r } => {
                let centre = p(*c);
                let edge = p([c[0] + r, c[1]]);
                out.push(Prim::Circle { c: centre, r: (edge[0] - centre[0]).abs().max(0.05), fill: None, stroke: Some((line_rgb, w)) });
            }
        }
    }
}

/// A grid bubble: the circle and the name.
fn bubble(out: &mut Vec<Prim>, c: [f64; 2], r: f64, label: &str, rgb: Rgb) {
    out.push(Prim::Circle { c, r, fill: Some(WHITE), stroke: Some((rgb, 0.25)) });
    text(out, [c[0], c[1] + NOTE_MM * 0.35], NOTE_MM, label, Align::Center, rgb, false);
}

fn plan_prims(doc: &Document, key: &str, vs: &openbim_document::ViewSettings) -> Result<Vec<Prim>, String> {
    let level_id = plan_level_id(doc, key).ok_or("not a plan")?;
    let level = doc.get(level_id).filter(|l| matches!(l.geometry, Some(Geometry::Level))).ok_or("the level of the plan is gone")?;
    let level_z = level.elevation_m;
    let s = per_metre(vs);
    let p = move |q: [f64; 2]| [q[0] * s, -q[1] * s];
    let range = vs.effective_range().ok_or("not a plan")?;
    let (scene, _) = crate::export::scene_for_view(doc)?;
    let index = scene.levels.index_of(&level.name).ok_or("the level is not in the scene")?;
    let view = openbim_view::plan_view(&scene, index, openbim_view::ViewRange::new(range.top_m, range.cut_m, range.bottom_m, range.depth_m).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    engine_items(&mut out, &view.items, &p);
    // what the engine does not draw: the ducts, pipes, beams, stairs, railings and footings, from their meshes
    let extras = [Category::Duct, Category::Pipe, Category::Beam, Category::BeamSystem, Category::Truss, Category::Stair, Category::Railing, Category::Footing, Category::WallFooting];
    type Face = (f32, Vec<[f64; 2]>, Rgb, Rgb, f64);
    let mut faces: Vec<Face> = Vec::new();
    for pm in crate::view_ui::all_meshes(doc) {
        let Some(el) = doc.get(pm.id) else { continue };
        if !extras.contains(&el.category) { continue; }
        let class = crate::view_ui::plan_class(Some(range), level_id, level_z, &pm);
        if class == openbim_document::RangeClass::Out { continue; }
        let fade = f64::from(crate::view_ui::range_fade(class));
        let base: Rgb = match el.category {
            Category::Duct => [70, 157, 194], Category::Pipe => [136, 153, 166], Category::Beam | Category::BeamSystem | Category::Truss => [176, 140, 104],
            Category::Railing => [120, 120, 130], Category::Stair => [210, 210, 210], _ => [160, 150, 135],
        };
        let (fill, edge) = (blend(blend(base, 0.45), fade), blend([60, 60, 60], fade));
        for face in &pm.mesh.faces {
            let ring: Vec<[f64; 2]> = face.iter().map(|&v| p([pm.mesh.vertices[v][0], pm.mesh.vertices[v][1]])).collect();
            let area: f64 = ring.iter().zip(ring.iter().cycle().skip(1)).take(ring.len()).map(|(a, b)| a[0] * b[1] - b[0] * a[1]).sum::<f64>().abs() / 2.0;
            if area < 0.05 { continue; }
            let depth = face.iter().map(|&v| pm.mesh.vertices[v][2]).sum::<f64>() / face.len() as f64;
            faces.push((depth as f32, ring, fill, edge, if class == openbim_document::RangeClass::Cut { 0.25 } else { 0.13 }));
        }
    }
    faces.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, ring, fill, edge, w) in faces { out.push(Prim::Poly { rings: vec![ring], fill: Some(fill), stroke: Some((edge, w)) }); }
    // room tags
    for t in &view.tags {
        let at = p(t.at);
        let name = if t.name.is_empty() { "Room".to_string() } else { t.name.clone() };
        text(&mut out, [at[0], at[1] - 0.5], NOTE_MM, &name, Align::Center, BLACK, false);
        text(&mut out, [at[0], at[1] + NOTE_MM + 0.5], NOTE_MM, &format!("{:.1} m\u{b2}", t.area).replace('\u{b2}', "2"), Align::Center, BLACK, false);
    }
    annotations(&mut out, doc, level_id, &p, s, vs.detail);
    Ok(out)
}

/// Grids, dimensions, tags, stair arrows, openings that are not in a wall, section lines and elevation markers.
fn annotations(out: &mut Vec<Prim>, doc: &Document, level_id: ElementId, p: &dyn Fn([f64; 2]) -> [f64; 2], _s: f64, _detail: openbim_document::Detail) {
    let r = 4.0;
    for el in doc.elements() {
        match el.geometry {
            Some(Geometry::Grid { start, end }) => {
                let (a, b) = (p(start), p(end));
                out.push(Prim::Line { a, b, rgb: GRAY, w: 0.13, dash: Some((12.0, 2.5)) });
                for c in [a, b] { bubble(out, c, r, &el.name, GRAY); }
            }
            Some(Geometry::Linear { start, end, level }) if el.category == Category::Section && level == level_id => {
                let red: Rgb = [170, 40, 40];
                let (a, b) = (p(start), p(end));
                out.push(Prim::Line { a, b, rgb: red, w: 0.25, dash: Some((8.0, 2.0)) });
                let number = el.name.rsplit(' ').next().unwrap_or("");
                bubble(out, a, r * 0.9, number, red);
                // the tick toward the side the section looks to: the left of the line in the model
                let d = [b[0] - a[0], b[1] - a[1]];
                let len = d[0].hypot(d[1]);
                if len > 1.0 {
                    let left = [d[1] / len, -d[0] / len];
                    let m = [a[0] + d[0] / 2.0, a[1] + d[1] / 2.0];
                    out.push(Prim::Line { a: m, b: [m[0] + left[0] * 6.0, m[1] + left[1] * 6.0], rgb: red, w: 0.25, dash: None });
                    out.push(Prim::Line { a: b, b: [b[0] + left[0] * 4.0, b[1] + left[1] * 4.0], rgb: red, w: 0.25, dash: None });
                }
            }
            Some(Geometry::Point { at, level, .. }) if el.category == Category::Elevation && level == level_id => {
                let c = p(at);
                bubble(out, c, r, el.name.rsplit(' ').next().unwrap_or(""), GRAY);
                for k in 0..4u8 {
                    if el.count >> k & 1 == 0 { continue; }
                    let f = crate::section_ui::facing_vector(usize::from(k));
                    let toward = [f[0], -f[1]];
                    let side = [-toward[1] * 1.8, toward[0] * 1.8];
                    let base = [c[0] + toward[0] * (r + 0.3), c[1] + toward[1] * (r + 0.3)];
                    let tip = [c[0] + toward[0] * (r + 4.5), c[1] + toward[1] * (r + 4.5)];
                    out.push(Prim::Poly { rings: vec![vec![[base[0] + side[0], base[1] + side[1]], [base[0] - side[0], base[1] - side[1]], tip]], fill: Some(GRAY), stroke: None });
                }
            }
            Some(Geometry::Dimension { a, b, at, level }) if el.category == Category::Dimension && level == level_id => {
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let l = dx.hypot(dy);
                if l < 1e-9 { continue; }
                let off = ((at[0] - a[0]) * -dy + (at[1] - a[1]) * dx) / l;
                let (nx, ny) = (-dy / l * off, dx / l * off);
                let (q1, q2) = (p([a[0] + nx, a[1] + ny]), p([b[0] + nx, b[1] + ny]));
                for (from, to) in [(p(a), q1), (p(b), q2)] {
                    let d = [to[0] - from[0], to[1] - from[1]];
                    let n = d[0].hypot(d[1]);
                    if n > 0.5 { let u = [d[0] / n, d[1] / n]; out.push(Prim::Line { a: [from[0] + u[0] * 1.0, from[1] + u[1] * 1.0], b: [to[0] + u[0] * 2.0, to[1] + u[1] * 2.0], rgb: BLACK, w: 0.13, dash: None }); }
                }
                out.push(Prim::Line { a: q1, b: q2, rgb: BLACK, w: 0.13, dash: None });
                let v = [q2[0] - q1[0], q2[1] - q1[1]];
                let n = v[0].hypot(v[1]).max(1e-9);
                let u = [v[0] / n, v[1] / n];
                for q in [q1, q2] {
                    // the slash tick at each end, at 45 degrees to the line
                    let t = [(u[0] - u[1]) * 1.4, (u[1] + u[0]) * 1.4];
                    out.push(Prim::Line { a: [q[0] - t[0], q[1] - t[1]], b: [q[0] + t[0], q[1] + t[1]], rgb: BLACK, w: 0.35, dash: None });
                }
                // the text stands on the line, turned to read from below or from the right
                let mut deg = (-u[1]).atan2(u[0]).to_degrees();
                if deg > 90.0 + 1e-9 { deg -= 180.0; } else if deg <= -90.0 { deg += 180.0; }
                let rad = deg.to_radians();
                let mid = [(q1[0] + q2[0]) / 2.0, (q1[1] + q2[1]) / 2.0];
                let up = [-rad.sin(), -rad.cos()];
                out.push(Prim::Text { at: [mid[0] + up[0] * 0.8, mid[1] + up[1] * 0.8], size: NOTE_MM, text: format!("{:.0}", l * 1000.0), align: Align::Center, rgb: BLACK, bold: false, degrees: deg });
            }
            Some(Geometry::Linear { start, end, level }) if el.category == Category::Stair && level == level_id => {
                let (a, b) = (p(start), p(end));
                out.push(Prim::Line { a, b, rgb: BLACK, w: 0.18, dash: None });
                out.push(Prim::Circle { c: a, r: 0.8, fill: Some(BLACK), stroke: None });
                let d = [b[0] - a[0], b[1] - a[1]];
                let n = d[0].hypot(d[1]);
                if n > 1.0 {
                    let u = [d[0] / n, d[1] / n];
                    let side = [-u[1] * 1.5, u[0] * 1.5];
                    for sgn in [1.0, -1.0] { out.push(Prim::Line { a: b, b: [b[0] - u[0] * 3.0 + side[0] * sgn, b[1] - u[1] * 3.0 + side[1] * sgn], rgb: BLACK, w: 0.18, dash: None }); }
                }
                text(out, [a[0] + 2.0, a[1] - 1.0], NOTE_MM, "UP", Align::Left, BLACK, false);
            }
            Some(Geometry::Shaft { .. }) | Some(Geometry::Area { level: _ }) if el.category == Category::Opening => {
                let on_level = match el.geometry { Some(Geometry::Area { level }) => level == level_id, _ => doc.shaft_range(el.id).zip(doc.get(level_id)).is_some_and(|((lo, hi), l)| l.elevation_m >= lo - 1e-6 && l.elevation_m <= hi + 1e-6) };
                if !on_level || el.outline.len() < 3 { continue; }
                let ring: Vec<[f64; 2]> = el.outline.iter().map(|q| p(*q)).collect();
                for i in 0..ring.len() { out.push(Prim::Line { a: ring[i], b: ring[(i + 1) % ring.len()], rgb: BLACK, w: 0.13, dash: Some((3.0, 1.5)) }); }
                let mut b = None;
                ring.iter().for_each(|q| grow(&mut b, *q));
                if let Some(b) = b {
                    out.push(Prim::Line { a: [b[0], b[1]], b: [b[2], b[3]], rgb: GRAY, w: 0.09, dash: None });
                    out.push(Prim::Line { a: [b[2], b[1]], b: [b[0], b[3]], rgb: GRAY, w: 0.09, dash: None });
                }
            }
            _ => {}
        }
    }
    // tags: the label of an element, boxed, on a leader to the place that was clicked
    for el in doc.elements().filter(|e| e.category == Category::Tag) {
        let Some(Geometry::Tag { host, at, anchor }) = el.geometry else { continue };
        if doc.level_of(host) != Some(level_id) { continue; }
        let (head, tip) = (p(at), p(anchor));
        let label = doc.tag_text(el.id);
        let (w, h) = (text_width_mm(&label, NOTE_MM, false) + 3.0, NOTE_MM + 2.0);
        if (head[0] - tip[0]).hypot(head[1] - tip[1]) > 1.0 {
            out.push(Prim::Line { a: head, b: tip, rgb: BLACK, w: 0.13, dash: None });
            out.push(Prim::Circle { c: tip, r: 0.7, fill: Some(BLACK), stroke: None });
        }
        out.push(Prim::Poly { rings: vec![vec![[head[0] - w / 2.0, head[1] - h / 2.0], [head[0] + w / 2.0, head[1] - h / 2.0], [head[0] + w / 2.0, head[1] + h / 2.0], [head[0] - w / 2.0, head[1] + h / 2.0]]], fill: Some(WHITE), stroke: Some((BLACK, 0.13)) });
        text(out, [head[0], head[1] + NOTE_MM * 0.35], NOTE_MM, &label, Align::Center, BLACK, false);
    }
}

fn section_prims(doc: &Document, view: View, vs: &openbim_document::ViewSettings) -> Result<Vec<Prim>, String> {
    use openbim_view::{Class, Kind, Shape};
    let built = crate::section_ui::build(doc, view)?;
    let s = per_metre(vs);
    let p = move |q: [f64; 2]| [q[0] * s, -q[1] * s];
    let mut out = Vec::new();
    for it in &built.items {
        let (fill, line_rgb, w, dash): (Rgb, Rgb, f64, Option<(f64, f64)>) = match (it.class, it.element.kind) {
            (Class::Cut, _) => ([105, 105, 105], BLACK, 0.35, None),
            (Class::Datum, _) => (WHITE, [70, 130, 200], 0.13, Some((4.0, 2.0))),
            (_, Kind::Roof) => ([200, 150, 140], [60, 60, 60], 0.13, None),
            (_, Kind::Family) => ([190, 215, 190], [60, 60, 60], 0.13, None),
            (_, Kind::Stair) => ([210, 210, 210], [60, 60, 60], 0.13, None),
            _ => ([232, 232, 232], [60, 60, 60], 0.13, None),
        };
        match &it.shape {
            Shape::Region { outer, holes } => {
                let rings: Vec<Vec<[f64; 2]>> = std::iter::once(outer).chain(holes.iter()).map(|r| r.iter().map(|q| p(*q)).collect()).collect();
                out.push(Prim::Poly { rings, fill: Some(fill), stroke: Some((line_rgb, w)) });
            }
            Shape::Fill(ring) => out.push(Prim::Poly { rings: vec![ring.iter().map(|q| p(*q)).collect()], fill: Some(fill), stroke: Some((line_rgb, w)) }),
            Shape::Line(a, b) => out.push(Prim::Line { a: p(*a), b: p(*b), rgb: line_rgb, w, dash }),
            Shape::Arc { c, r, a0, a1 } => { let pts: Vec<[f64; 2]> = arc_points(*c, *r, *a0, *a1).into_iter().map(p).collect(); push_polyline(&mut out, &pts, line_rgb, w); }
            Shape::Circle { c, r } => out.push(Prim::Circle { c: p(*c), r: (r * s).max(0.05), fill: None, stroke: Some((line_rgb, w)) }),
        }
    }
    for l in &built.labels {
        let at = p(l.at);
        text(&mut out, [at[0] + 2.0, at[1] + NOTE_MM * 0.35], NOTE_MM, &format!("{}  {:+.3}", l.text, l.at[1]), Align::Left, [70, 130, 200], false);
    }
    Ok(out)
}

/// The 3D view as an isometric view of the model, shaded by the light from the upper left.
fn iso_prims(doc: &Document, vs: &openbim_document::ViewSettings) -> Result<Vec<Prim>, String> {
    let s = per_metre(vs);
    let batch = crate::view3d::batch(doc);
    if batch.tris.is_empty() { return Err("the model has nothing to show".into()); }
    let (yaw, pitch) = (std::f64::consts::FRAC_PI_4, 0.6155_f64);
    let proj = |v: [f64; 3]| -> ([f64; 2], f64) {
        let (x, y, z) = (v[0], v[1], v[2]);
        let horizontal = x * yaw.sin() + y * yaw.cos();
        ([(x * yaw.cos() - y * yaw.sin()) * s, (horizontal * pitch.sin() - z * pitch.cos()) * s], horizontal * pitch.cos() + z * pitch.sin())
    };
    let world = |i: u32| { let v = batch.verts[i as usize]; [f64::from(v[0]) + batch.origin[0], f64::from(v[1]) + batch.origin[1], f64::from(v[2]) + batch.origin[2]] };
    let light = { let l = [-0.45_f64, -0.55, 0.70]; let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt(); [l[0] / n, l[1] / n, l[2] / n] };
    let mut tris: Vec<(f64, [[f64; 2]; 3], Rgb)> = Vec::new();
    for (k, t) in batch.tris.iter().enumerate() {
        let (a, b, c) = (world(t[0]), world(t[1]), world(t[2]));
        let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let nrm = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let len = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
        if len < 1e-12 { continue; }
        let n = [nrm[0] / len, nrm[1] / len, nrm[2] / len];
        let lit = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
        let kind = batch.elements.get(batch.tri_element[k] as usize).map(|e| e.kind);
        let base: [f64; 3] = match kind {
            Some(openbim_view::Kind::Wall) => [200.0, 200.0, 200.0], Some(openbim_view::Kind::Slab) => [205.0, 205.0, 212.0], Some(openbim_view::Kind::Roof) => [178.0, 96.0, 78.0],
            Some(openbim_view::Kind::Family) => [120.0, 150.0, 110.0], Some(openbim_view::Kind::Column) => [150.0, 150.0, 160.0], Some(openbim_view::Kind::Beam) => [176.0, 140.0, 104.0],
            Some(openbim_view::Kind::Segment) => [70.0, 157.0, 194.0], _ => [170.0, 170.0, 170.0],
        };
        let shade = 0.5 + 0.5 * lit;
        let rgb = [(base[0] * shade) as u8, (base[1] * shade) as u8, (base[2] * shade) as u8];
        let (pa, da) = proj(a);
        let (pb, db) = proj(b);
        let (pc, dc) = proj(c);
        // a triangle that faces away from the viewer is hidden by the ones in front of it; the order does the rest
        let area = (pb[0] - pa[0]) * (pc[1] - pa[1]) - (pc[0] - pa[0]) * (pb[1] - pa[1]);
        if area.abs() < 1e-4 { continue; }
        tris.push((da.min(db).min(dc), [pa, pb, pc], rgb));
    }
    tris.sort_by(|x, y| x.0.total_cmp(&y.0));
    Ok(tris.into_iter().map(|(_, ring, rgb)| Prim::Poly { rings: vec![ring.to_vec()], fill: Some(rgb), stroke: Some((rgb, 0.05)) }).collect())
}

// ---------------------------------------------------------------------------------------------------------------------
// schedules

fn schedule_drawing(doc: &Document, id: u64) -> Result<Drawing, String> {
    use openbim_document::{RowKind, ScheduleValue};
    let def = doc.schedule(id).ok_or("the schedule is gone")?;
    let table = doc.schedule_table(def);
    let cols = table.headings.len();
    if cols == 0 { return Err("the schedule has no fields".into()); }
    let pad = 1.5;
    let size = NOTE_MM;
    let row_h = size + 2.4;
    let heading = |c: usize| { let u = table.types[c].unit(); if u.is_empty() { table.headings[c].clone() } else { format!("{} ({u})", table.headings[c]) } };
    let title_col = |r: &openbim_document::TableRow| if matches!(r.kind, RowKind::Data | RowKind::Blank) { None } else { r.cells.iter().position(|c| matches!(c, ScheduleValue::Blank)) };
    let cell = |r: &openbim_document::TableRow, c: usize| if title_col(r) == Some(c) { r.label.clone() } else { table.text(r, c) };
    let bold_row = |r: &openbim_document::TableRow| !matches!(r.kind, RowKind::Data | RowKind::Blank);
    let mut widths: Vec<f64> = (0..cols).map(|c| text_width_mm(&heading(c), size, true) + 2.0 * pad).collect();
    for r in &table.rows { for (c, wd) in widths.iter_mut().enumerate() { *wd = wd.max(text_width_mm(&cell(r, c), size, bold_row(r)) + 2.0 * pad); } }
    let total_w: f64 = widths.iter().sum();
    let title_h = row_h + 1.5;
    let mut out = Vec::new();
    // the title centred over the table
    text(&mut out, [total_w / 2.0, row_h * 0.75], size + 1.5, &def.name, Align::Center, BLACK, true);
    let top = title_h;
    let n_rows = table.rows.len() + 1;
    let bottom = top + row_h * n_rows as f64;
    let mut x = 0.0;
    let xs: Vec<f64> = widths.iter().map(|w| { let at = x; x += w; at }).collect();
    // the heading row, then the lines
    out.push(Prim::Poly { rings: vec![vec![[0.0, top], [total_w, top], [total_w, top + row_h], [0.0, top + row_h]]], fill: Some([235, 235, 235]), stroke: None });
    for (c, x0) in xs.iter().enumerate().take(cols) { text(&mut out, [x0 + pad, top + row_h * 0.72], size, &heading(c), Align::Left, BLACK, true); }
    for (i, r) in table.rows.iter().enumerate() {
        let y = top + row_h * (i + 1) as f64;
        for c in 0..cols {
            let s = cell(r, c);
            if s.is_empty() { continue; }
            let numeric = table.types[c].numeric() && table.types[c] != openbim_document::FieldType::Id && title_col(r) != Some(c);
            if numeric { text(&mut out, [xs[c] + widths[c] - pad, y + row_h * 0.72], size, &s, Align::Right, BLACK, bold_row(r)); }
            else { text(&mut out, [xs[c] + pad, y + row_h * 0.72], size, &s, Align::Left, BLACK, bold_row(r)); }
        }
    }
    for i in 0..=n_rows {
        let y = top + row_h * i as f64;
        out.push(Prim::Line { a: [0.0, y], b: [total_w, y], rgb: BLACK, w: if i <= 1 || i == n_rows { 0.35 } else { 0.1 }, dash: None });
    }
    for (c, &x) in xs.iter().enumerate() { out.push(Prim::Line { a: [x, top], b: [x, bottom], rgb: BLACK, w: if c == 0 { 0.35 } else { 0.1 }, dash: None }); }
    out.push(Prim::Line { a: [total_w, top], b: [total_w, bottom], rgb: BLACK, w: 0.35, dash: None });
    Ok(Drawing::from_prims(out))
}

// ---------------------------------------------------------------------------------------------------------------------
// the sheet

/// The width and height of the margin around the drawing area and the title block of a sheet, in millimetres.
pub const MARGIN_MM: f64 = 10.0;

/// The title block's box at the bottom right of the border: (left, top, width, height).
pub fn title_block_box(sheet: &SheetDef) -> [f64; 4] {
    let (w, h) = sheet.dims_mm();
    let tb_w = 180.0_f64.min((w - 2.0 * MARGIN_MM) * 0.5);
    let tb_h = 45.0_f64.min((h - 2.0 * MARGIN_MM) * 0.3);
    [w - MARGIN_MM - tb_w, h - MARGIN_MM - tb_h, tb_w, tb_h]
}

/// The area inside the border and above the title block where views go: (left, top, right, bottom).
pub fn drawing_area(sheet: &SheetDef) -> [f64; 4] {
    let (w, h) = sheet.dims_mm();
    let tb = title_block_box(sheet);
    [MARGIN_MM, MARGIN_MM, w - MARGIN_MM, (tb[1] - 2.0).max(MARGIN_MM + 1.0).min(h - MARGIN_MM)]
}

fn rect(l: f64, t: f64, w: f64, h: f64) -> Vec<[f64; 2]> { vec![[l, t], [l + w, t], [l + w, t + h], [l, t + h]] }

/// The frame of the paper and the title block, with the texts of the sheet.
pub fn frame_prims(sheet: &SheetDef, scale_note: &str) -> Vec<Prim> {
    let (w, h) = sheet.dims_mm();
    let mut out = vec![
        Prim::Poly { rings: vec![rect(0.0, 0.0, w, h)], fill: Some(WHITE), stroke: Some(([120, 120, 120], 0.1)) },
        Prim::Poly { rings: vec![rect(MARGIN_MM, MARGIN_MM, w - 2.0 * MARGIN_MM, h - 2.0 * MARGIN_MM)], fill: None, stroke: Some((BLACK, 0.5)) },
    ];
    let [l, t, bw, bh] = title_block_box(sheet);
    out.push(Prim::Poly { rings: vec![rect(l, t, bw, bh)], fill: Some(WHITE), stroke: Some((BLACK, 0.35)) });
    // three rows: client and project / sheet name and number / date, drawn by, checked by, scale
    let u = bh / 45.0;
    let (r2, r3) = (t + 15.0 * u, t + 32.0 * u);
    for y in [r2, r3] { out.push(Prim::Line { a: [l, y], b: [l + bw, y], rgb: BLACK, w: 0.25, dash: None }); }
    let number_w = bw * 0.28;
    out.push(Prim::Line { a: [l + bw - number_w, r2], b: [l + bw - number_w, r3], rgb: BLACK, w: 0.25, dash: None });
    let (label_size, value_size) = (1.8 * u.max(0.6), 2.8 * u.max(0.6));
    let cells = [("Date", sheet.block.date.as_str()), ("Drawn by", sheet.block.drawn_by.as_str()), ("Checked by", sheet.block.checked_by.as_str()), ("Scale", scale_note)];
    let cw = bw / 4.0;
    for (i, (label, value)) in cells.iter().enumerate() {
        let x = l + cw * i as f64;
        if i > 0 { out.push(Prim::Line { a: [x, r3], b: [x, t + bh], rgb: BLACK, w: 0.1, dash: None }); }
        text(&mut out, [x + 1.5, r3 + 3.2 * u.max(0.6)], label_size, label, Align::Left, GRAY, false);
        text(&mut out, [x + 1.5, t + bh - 2.5 * u.max(0.6)], value_size, value, Align::Left, BLACK, false);
    }
    let fit = |s: &str, max: f64, size: f64| -> String {
        let mut s = s.to_string();
        while text_width_mm(&s, size, true) > max && s.chars().count() > 1 { s.pop(); }
        s
    };
    text(&mut out, [l + 2.0, t + 4.0 * u.max(0.6)], label_size, &sheet.block.client, Align::Left, GRAY, false);
    let project_size = 4.5 * u.max(0.6);
    text(&mut out, [l + 2.0, r2 - 3.5 * u.max(0.6)], project_size, &fit(&sheet.block.project, bw - 4.0, project_size), Align::Left, BLACK, true);
    let name_size = 5.5 * u.max(0.6);
    text(&mut out, [l + 2.0, r2 + 3.2 * u.max(0.6)], label_size, "Sheet name", Align::Left, GRAY, false);
    text(&mut out, [l + 2.0, r3 - 3.5 * u.max(0.6)], name_size, &fit(&sheet.name, bw - number_w - 4.0, name_size), Align::Left, BLACK, true);
    text(&mut out, [l + bw - number_w + 1.5, r2 + 3.2 * u.max(0.6)], label_size, "Sheet number", Align::Left, GRAY, false);
    let num_size = 9.0 * u.max(0.6);
    text(&mut out, [l + bw - number_w / 2.0, r3 - 3.5 * u.max(0.6)], num_size, &fit(&sheet.number, number_w - 3.0, num_size), Align::Center, BLACK, true);
    out
}

/// The drawing of every viewport of a sheet, by the viewport's number. Making them takes time (it works out the views),
/// so the sheet view keeps them while the model does not change.
pub fn viewport_drawings(doc: &Document, sheet: &SheetDef) -> BTreeMap<u64, Result<Drawing, String>> {
    use rayon::prelude::*;
    sheet.viewports.par_iter().map(|v| (v.id, view_drawing(doc, &v.view))).collect()
}

/// A viewport as laid out on the sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub id: u64,
    /// the box of the drawing, with its title: left, top, right, bottom
    pub rect: [f64; 4],
}

/// The sheet's shapes and where each viewport is. `moved` shifts one viewport while it is being dragged.
pub fn compose(doc: &Document, sheet: &SheetDef, drawings: &BTreeMap<u64, Result<Drawing, String>>, moved: Option<(u64, f64, f64)>) -> (Vec<Prim>, Vec<Placed>) {
    let scales: Vec<u32> = sheet.viewports.iter().filter(|v| openbim_document::schedule_ref(&v.view).is_none()).map(|v| doc.view_settings(&v.view).scale).collect();
    let note = match scales.as_slice() { [] => String::new(), [one] => format!("1 : {one}"), many => if many.iter().all(|s| *s == many[0]) { format!("1 : {}", many[0]) } else { "As indicated".to_string() } };
    let mut out = frame_prims(sheet, &note);
    let mut placed = Vec::new();
    for (k, v) in sheet.viewports.iter().enumerate() {
        let (dx, dy) = moved.filter(|m| m.0 == v.id).map_or((0.0, 0.0), |m| (m.1, m.2));
        let centre = [v.x_mm + dx, v.y_mm + dy];
        match drawings.get(&v.id) {
            Some(Ok(d)) => {
                let (left, top) = (centre[0] - d.w / 2.0, centre[1] - d.h / 2.0);
                out.extend(d.prims.iter().map(|p| p.shifted(left, top)));
                let is_schedule = openbim_document::schedule_ref(&v.view).is_some();
                let mut rect = [left, top, left + d.w, top + d.h];
                if v.show_title && !is_schedule {
                    let title = if v.title.is_empty() { view_title(doc, &v.view) } else { v.title.clone() };
                    let scale = format!("Scale  1 : {}", doc.view_settings(&v.view).scale);
                    let y = top + d.h + 8.0;
                    let r = 3.5;
                    out.push(Prim::Circle { c: [left + r, y], r, fill: Some(WHITE), stroke: Some((BLACK, 0.35)) });
                    text(&mut out, [left + r, y + 1.2], 3.0, &(k + 1).to_string(), Align::Center, BLACK, true);
                    text(&mut out, [left + 2.0 * r + 2.0, y + 1.2], 4.0, &title, Align::Left, BLACK, true);
                    text(&mut out, [left + 2.0 * r + 2.0, y + 7.6], NOTE_MM, &scale, Align::Left, BLACK, false);
                    let line_w = (text_width_mm(&title, 4.0, true) + 2.0 * r + 4.0).max(d.w.min(60.0));
                    out.push(Prim::Line { a: [left, y + 3.9], b: [left + line_w, y + 3.9], rgb: BLACK, w: 0.5, dash: None });
                    rect[3] = y + 9.0;
                    rect[2] = rect[2].max(left + line_w);
                }
                placed.push(Placed { id: v.id, rect });
            }
            other => {
                // a view that cannot be drawn is a labelled box, so that it can still be found and taken off
                let msg = match other { Some(Err(e)) => e.clone(), _ => "not drawn".into() };
                let (w, h) = (70.0, 22.0);
                let (left, top) = (centre[0] - w / 2.0, centre[1] - h / 2.0);
                out.push(Prim::Poly { rings: vec![rect_ring(left, top, w, h)], fill: Some([250, 240, 240]), stroke: Some(([170, 40, 40], 0.25)) });
                text(&mut out, [centre[0], centre[1] - 1.0], 3.0, &view_title(doc, &v.view), Align::Center, BLACK, true);
                text(&mut out, [centre[0], centre[1] + 4.0], 2.2, &msg.chars().take(40).collect::<String>(), Align::Center, [170, 40, 40], false);
                placed.push(Placed { id: v.id, rect: [left, top, left + w, top + h] });
            }
        }
    }
    (out, placed)
}

fn rect_ring(l: f64, t: f64, w: f64, h: f64) -> Vec<[f64; 2]> { rect(l, t, w, h) }

/// The page for a sheet.
pub fn sheet_page(doc: &Document, sheet: &SheetDef, bw: bool) -> Page {
    let (w, h) = sheet.dims_mm();
    let mut page = Page::new(w, h);
    let (prims, _) = compose(doc, sheet, &viewport_drawings(doc, sheet), None);
    for p in &prims { p.paint(&mut page, 0.0, 0.0, bw); }
    page
}

/// Where a view goes when it is put on a sheet: the first free place in the drawing area, looking along the rows from
/// the top left (the middle of the area when there is none), clear of the views already there. `existing` are their boxes
/// with titles; the new view's box is its drawing and its title below.
pub fn default_place(sheet: &SheetDef, existing: &[[f64; 4]], drawing_size: (f64, f64)) -> (f64, f64) {
    let [l, t, r, b] = drawing_area(sheet);
    let (w, h) = (drawing_size.0, drawing_size.1 + 17.0);
    let gap = 8.0;
    let step = 5.0;
    let pad = 8.0;
    let mut y = t + pad;
    while y + h <= b + 1e-9 {
        let mut x = l + pad;
        while x + w <= r + 1e-9 {
            let free = existing.iter().all(|e| x + w + gap <= e[0] || e[2] + gap <= x || y + h + gap <= e[1] || e[3] + gap <= y);
            if free { return (x + w / 2.0, y + drawing_size.1 / 2.0); }
            x += step;
        }
        y += step;
    }
    ((l + r) / 2.0, (t + b) / 2.0 - 7.0)
}
