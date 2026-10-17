//! Drawing views as SVG. One unit is one metre, y up; line weights are in metres on paper at 1:1, so a caller who wants
//! a scale applies it to the whole page. The look is a style, not data: the same view can be drawn as Revit draws it
//! (white cut walls with heavy outlines, hatched concrete), as solid poche, or shaded grey.
use crate::{Class, Item, Kind, Label, PlanView, RoomTag, Shape};
use std::fmt::Write;

type P = [f64; 2];

/// How a closed region is painted.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// not painted: only its outline lines show
    None,
    Color(String),
    /// diagonal lines in `line` colour, `spacing` metres apart, on white
    Hatch { line: String, spacing: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SvgStyle {
    pub wall: Paint,
    pub column: Paint,
    pub slab: Paint,
    pub room: Paint,
    pub cut_width: f64,
    pub projection_width: f64,
    pub beyond_width: f64,
    pub beyond_colour: String,
    pub room_line_colour: String,
    /// draw the room tags (name and area)
    pub tags: bool,
}

impl SvgStyle {
    /// As Revit draws a plan at medium detail: cut walls are white with heavy black outlines, concrete (slabs and
    /// columns) is hatched, rooms are not tinted, lines below and beyond the cut are thin, beyond ones grey.
    pub fn revit() -> SvgStyle {
        SvgStyle {
            wall: Paint::Color("white".into()),
            column: Paint::Hatch { line: "black".into(), spacing: 0.06 },
            slab: Paint::Hatch { line: "#444".into(), spacing: 0.08 },
            room: Paint::None,
            cut_width: 0.035,
            projection_width: 0.012,
            beyond_width: 0.009,
            beyond_colour: "#7f7f7f".into(),
            room_line_colour: "#7a5aa6".into(),
            tags: true,
        }
    }
    /// Everything cut is solid black, as in a coarse drawing for print.
    pub fn poche() -> SvgStyle {
        SvgStyle { wall: Paint::Color("black".into()), column: Paint::Color("black".into()), slab: Paint::Color("black".into()), ..SvgStyle::revit() }
    }
    /// Grey fills and tinted rooms, as a quick preview.
    pub fn shaded() -> SvgStyle {
        SvgStyle {
            wall: Paint::Color("#cfcfcf".into()),
            column: Paint::Color("#bdbdbd".into()),
            slab: Paint::Color("#bdbdbd".into()),
            room: Paint::Color("#e8eefc".into()),
            cut_width: 0.05,
            projection_width: 0.02,
            beyond_width: 0.015,
            beyond_colour: "#999".into(),
            ..SvgStyle::revit()
        }
    }
}

/// A plan view as SVG in the Revit look.
pub fn svg(v: &PlanView) -> String { svg_with(v, &SvgStyle::revit()) }

pub fn svg_with(v: &PlanView, st: &SvgStyle) -> String {
    page(&v.level_name, v.bounds().unwrap_or([0.0, 0.0, 1.0, 1.0]), &v.items, &v.tags, &[], st)
}

/// The page for any list of items; `bounds` is `[min_x, min_y, max_x, max_y]`.
pub(crate) fn page(title: &str, bounds: [f64; 4], items: &[Item], tags: &[RoomTag], labels: &[Label], st: &SvgStyle) -> String {
    let [x0, y0, x1, y1] = bounds;
    let m = 0.5;
    let mut s = String::new();
    let _ = write!(s, "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\">", x0 - m, -(y1 + m), x1 - x0 + 2.0 * m, y1 - y0 + 2.0 * m);
    let _ = write!(s, "<title>{}</title>", esc(title));
    // one hatch pattern per distinct hatch in the style
    let mut hatches: Vec<(String, f64)> = Vec::new();
    for p in [&st.wall, &st.column, &st.slab, &st.room] {
        if let Paint::Hatch { line, spacing } = p {
            if !hatches.iter().any(|h| h.0 == *line && h.1 == *spacing) { hatches.push((line.clone(), *spacing)); }
        }
    }
    s.push_str("<defs>");
    for (k, (line, sp)) in hatches.iter().enumerate() {
        let _ = write!(s, "<pattern id=\"hatch{k}\" patternUnits=\"userSpaceOnUse\" width=\"{sp}\" height=\"{sp}\" patternTransform=\"rotate(45)\"><rect width=\"{sp}\" height=\"{sp}\" fill=\"white\"/><line x1=\"0\" y1=\"0\" x2=\"0\" y2=\"{sp}\" stroke=\"{line}\" stroke-width=\"{}\"/></pattern>", sp / 6.0);
    }
    s.push_str("</defs>");
    let _ = write!(s, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"white\"/><g transform=\"scale(1,-1)\" fill=\"none\" stroke=\"black\" stroke-linecap=\"round\" stroke-linejoin=\"round\">", x0 - m, -(y1 + m), x1 - x0 + 2.0 * m, y1 - y0 + 2.0 * m);
    let fill_attr = |p: &Paint| -> Option<String> {
        match p {
            Paint::None => None,
            Paint::Color(c) => Some(c.clone()),
            Paint::Hatch { line, spacing } => hatches.iter().position(|h| h.0 == *line && h.1 == *spacing).map(|k| format!("url(#hatch{k})")),
        }
    };
    let paint_of = |k: Kind| match k {
        Kind::Wall => &st.wall,
        Kind::Column => &st.column,
        Kind::Slab => &st.slab,
        Kind::Room => &st.room,
        _ => &Paint::None,
    };
    let line_style = |c: Class| match c {
        Class::Room => format!("stroke=\"{}\" stroke-width=\"{}\" stroke-dasharray=\"0.2 0.1\"", st.room_line_colour, st.projection_width),
        Class::Cut => format!("stroke=\"black\" stroke-width=\"{}\"", st.cut_width),
        Class::Below | Class::Symbol => format!("stroke=\"black\" stroke-width=\"{}\"", st.projection_width),
        Class::Overhead => format!("stroke=\"black\" stroke-width=\"{}\" stroke-dasharray=\"0.15 0.1\"", st.projection_width),
        Class::Datum => format!("stroke=\"{}\" stroke-width=\"{}\" stroke-dasharray=\"0.3 0.08 0.05 0.08\"", st.beyond_colour, st.projection_width),
        Class::Beyond => format!("stroke=\"{}\" stroke-width=\"{}\"", st.beyond_colour, st.beyond_width),
    };
    for it in items {
        let ls = line_style(it.class);
        match &it.shape {
            Shape::Fill(poly) => {
                if let Some(f) = fill_attr(paint_of(it.element.kind)) {
                    let pts: Vec<String> = poly.iter().map(|p| format!("{},{}", p[0], p[1])).collect();
                    let _ = write!(s, "<polygon points=\"{}\" fill=\"{f}\" stroke=\"none\"/>", pts.join(" "));
                }
            }
            Shape::Region { outer, holes } => {
                if let Some(f) = fill_attr(paint_of(it.element.kind)) {
                    let ring = |r: &[P]| format!("M{} Z", r.iter().map(|p| format!("{},{}", p[0], p[1])).collect::<Vec<_>>().join(" L"));
                    let d: Vec<String> = std::iter::once(outer.as_slice()).chain(holes.iter().map(|h| h.as_slice())).map(ring).collect();
                    let _ = write!(s, "<path d=\"{}\" fill-rule=\"evenodd\" fill=\"{f}\" stroke=\"none\"/>", d.join(" "));
                }
            }
            Shape::Line(a, b) => { let _ = write!(s, "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" {ls}/>", a[0], a[1], b[0], b[1]); }
            Shape::Circle { c, r } => { let _ = write!(s, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" {ls}/>", c[0], c[1], r); }
            Shape::Arc { c, r, a0, a1 } => {
                let sweep = (a1 - a0).rem_euclid(360.0);
                let sweep = if sweep == 0.0 { 360.0 } else { sweep };
                let (p, q) = (point_on(*c, *r, *a0), point_on(*c, *r, a0 + sweep));
                let _ = write!(s, "<path d=\"M{},{} A{r},{r} 0 {} 1 {},{}\" {ls}/>", p[0], p[1], u8::from(sweep > 180.0), q[0], q[1]);
            }
        }
    }
    s.push_str("</g>");
    // tags are drawn upright, outside the mirrored group
    if st.tags {
        for t in tags {
            let _ = write!(s, "<g font-family=\"sans-serif\" font-size=\"0.22\" text-anchor=\"middle\" fill=\"#333\"><text x=\"{}\" y=\"{}\">{}</text><text x=\"{}\" y=\"{}\" font-size=\"0.18\">{:.1} m²</text></g>", t.at[0], -t.at[1], esc(&t.name), t.at[0], -t.at[1] + 0.28, t.area);
        }
    }
    if st.tags {
        for l in labels {
            let _ = write!(s, "<text font-family=\"sans-serif\" font-size=\"0.2\" fill=\"#333\" x=\"{}\" y=\"{}\">{}</text>", l.at[0] + 0.1, -l.at[1] - 0.05, esc(&l.text));
        }
    }
    s.push_str("</svg>");
    s
}

fn point_on(c: P, r: f64, deg: f64) -> P {
    let (s, co) = deg.to_radians().sin_cos();
    [c[0] + r * co, c[1] + r * s]
}

fn esc(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
