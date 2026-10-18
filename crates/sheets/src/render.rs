//! A sheet as drawing primitives: title block, viewports with the view's content after discipline
//! visibility and clipping, revision clouds and tags. Units are millimetres on the sheet.

use std::collections::BTreeMap;

use crate::model::{Params, PrintContext, Sheet, SheetSet};
use crate::paper::{PaperSize, Rect};
use crate::titleblock::{Align, Item, RevField};
use crate::visibility::{Display, ElementRef, ViewLibrary};
use crate::{finite, SheetError};

pub const MAX_PRIMS: usize = 5_000_000;

/// Per-export stroke colour policy. Does not modify view or model settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PrintMode {
    #[default]
    Colour,
    Grayscale,
    BlackLines,
}

impl PrintMode {
    fn base_colour(self, rgb: [u8; 3]) -> [u8; 3] {
        match self {
            Self::Colour => rgb,
            // Rounded BT.601 luma on the encoded RGB channels, deliberately deterministic.
            Self::Grayscale => {
                let grey = (299 * u32::from(rgb[0]) + 587 * u32::from(rgb[1]) + 114 * u32::from(rgb[2]) + 500) / 1000;
                [grey as u8; 3]
            }
            Self::BlackLines => [0; 3],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Prim {
    Line { a: [f64; 2], b: [f64; 2], weight_mm: f64, rgb: [u8; 3], layer: String },
    Text { at: [f64; 2], height: f64, align: Align, text: String, bold: bool, layer: String },
}

#[derive(Debug, Clone)]
pub struct RenderedSheet {
    pub number: String,
    pub name: String,
    pub size: PaperSize,
    pub prims: Vec<Prim>,
    pub warnings: Vec<String>,
}

/// A line of a view's content, in metres in the view's coordinates.
#[derive(Debug, Clone)]
pub struct ModelPrim {
    pub a: [f64; 2],
    pub b: [f64; 2],
    pub category: String,
    pub params: Params,
    pub link: Option<String>,
    /// line weight number, 1 to 16
    pub weight: u8,
}

pub type ViewContents = BTreeMap<String, Vec<ModelPrim>>;

/// Pen widths in millimetres by line weight number (1 to 16): one list for sheet graphics, and lists
/// per view scale for model lines. Like Revit, a view uses the first listed scale that is at least its
/// own scale number, or the last one. The numbers are the company's to set.
#[derive(Debug, Clone)]
pub struct LineWeightTable {
    paper: Vec<f64>,
    model: Vec<(u32, Vec<f64>)>,
}

impl LineWeightTable {
    pub fn new(paper: Vec<f64>, mut model: Vec<(u32, Vec<f64>)>) -> Result<Self, SheetError> {
        let ok = |v: &Vec<f64>| v.len() == 16 && v.iter().all(|w| w.is_finite() && (0.01..=10.0).contains(w));
        if !ok(&paper) || model.is_empty() || model.iter().any(|(s, v)| *s == 0 || !ok(v)) {
            return Err(SheetError::BadValue { what: "line weight table", value: "needs 16 widths between 0.01 and 10 mm per list".into() });
        }
        model.sort_by_key(|(s, _)| *s);
        Ok(LineWeightTable { paper, model })
    }

    /// The same 16 widths everywhere.
    pub fn uniform(widths: Vec<f64>) -> Result<Self, SheetError> {
        Self::new(widths.clone(), vec![(1, widths)])
    }

    pub fn paper_mm(&self, w: u8) -> f64 {
        self.paper[usize::from(w.clamp(1, 16)) - 1]
    }

    pub fn model_mm(&self, w: u8, scale: u32) -> f64 {
        let row = self.model.iter().find(|(s, _)| *s >= scale).unwrap_or_else(|| self.model.last().expect("not empty"));
        row.1[usize::from(w.clamp(1, 16)) - 1]
    }
}

/// Layer name from the discipline letter and the category: `M-DUCTS`.
pub fn layer_name(code: char, category: &str) -> String {
    let c: String = category.chars().map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_uppercase() } else { '_' }).collect();
    format!("{code}-{c}")
}

fn halftone(c: [u8; 3]) -> [u8; 3] {
    [((u16::from(c[0]) + 255) / 2) as u8, ((u16::from(c[1]) + 255) / 2) as u8, ((u16::from(c[2]) + 255) / 2) as u8]
}

/// Liang–Barsky: the part of a segment inside a rectangle.
pub fn clip_segment(a: [f64; 2], b: [f64; 2], r: &Rect) -> Option<([f64; 2], [f64; 2])> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a[0] - r.x0), (dx, r.x1 - a[0]), (-dy, a[1] - r.y0), (dy, r.y1 - a[1])] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                if t > t1 {
                    return None;
                }
                t0 = t0.max(t);
            } else {
                if t < t0 {
                    return None;
                }
                t1 = t1.min(t);
            }
        }
    }
    Some(([a[0] + t0 * dx, a[1] + t0 * dy], [a[0] + t1 * dx, a[1] + t1 * dy]))
}

fn signed_area(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    (0..n).map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1]).sum::<f64>() / 2.0
}

/// The bumpy outline of a revision cloud: every edge is cut into chords of about `bump` mm and each chord
/// gets a semicircle bulging outward.
pub fn cloud_polyline(outline: &[[f64; 2]], bump: f64) -> Vec<[f64; 2]> {
    let ccw = signed_area(outline) >= 0.0;
    let n = outline.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (p, q) = (outline[i], outline[(i + 1) % n]);
        let len = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
        if len == 0.0 {
            continue;
        }
        let parts = ((len / bump).round() as usize).max(1);
        let u = [(q[0] - p[0]) / len, (q[1] - p[1]) / len];
        let nrm = if ccw { [u[1], -u[0]] } else { [-u[1], u[0]] };
        for k in 0..parts {
            let s0 = len * k as f64 / parts as f64;
            let s1 = len * (k + 1) as f64 / parts as f64;
            let (c0, c1) = ([p[0] + u[0] * s0, p[1] + u[1] * s0], [p[0] + u[0] * s1, p[1] + u[1] * s1]);
            let chord = s1 - s0;
            let m = [(c0[0] + c1[0]) / 2.0, (c0[1] + c1[1]) / 2.0];
            for j in 0..=8 {
                if j == 0 && !out.is_empty() {
                    continue; // the end of the previous bump is this point
                }
                let th = std::f64::consts::PI * j as f64 / 8.0;
                let (cs, sn) = (th.cos(), th.sin());
                out.push([m[0] - u[0] * chord / 2.0 * cs + nrm[0] * chord / 2.0 * sn, m[1] - u[1] * chord / 2.0 * cs + nrm[1] * chord / 2.0 * sn]);
            }
        }
    }
    out
}

struct Out<'a> {
    prims: Vec<Prim>,
    warnings: Vec<String>,
    lw: &'a LineWeightTable,
}

impl Out<'_> {
    fn line(&mut self, a: [f64; 2], b: [f64; 2], weight: u8, layer: &str) {
        let w = self.lw.paper_mm(weight);
        self.prims.push(Prim::Line { a, b, weight_mm: w, rgb: [0, 0, 0], layer: layer.to_string() });
    }
    fn rect(&mut self, r: &Rect, weight: u8, layer: &str) {
        let c = [[r.x0, r.y0], [r.x1, r.y0], [r.x1, r.y1], [r.x0, r.y1]];
        for i in 0..4 {
            self.line(c[i], c[(i + 1) % 4], weight, layer);
        }
    }
    fn text(&mut self, at: [f64; 2], height: f64, align: Align, text: String, bold: bool, layer: &str) {
        if !text.is_empty() {
            self.prims.push(Prim::Text { at, height, align, text, bold, layer: layer.to_string() });
        }
    }
}

/// Draw one sheet in colour (backwards-compatible default).
pub fn render_sheet(set: &SheetSet, lib: &ViewLibrary, contents: &ViewContents, lw: &LineWeightTable, number: &str, ctx: &PrintContext) -> Result<RenderedSheet, SheetError> {
    render_sheet_with_mode(set, lib, contents, lw, number, ctx, PrintMode::Colour)
}

/// Draw one sheet with an explicit output mode, before PDF/DXF encoding.
/// Apply the mode before halftoning so black lines retain distinguishable gray halftones.
/// Title-block, revision and annotation graphics remain black in every mode.
pub fn render_sheet_with_mode(set: &SheetSet, lib: &ViewLibrary, contents: &ViewContents, lw: &LineWeightTable, number: &str, ctx: &PrintContext, mode: PrintMode) -> Result<RenderedSheet, SheetError> {
    let s: &Sheet = set.sheet(number).ok_or_else(|| SheetError::Unknown { what: "sheet", value: number.to_string() })?;
    let tb = set.titleblock(&s.titleblock).ok_or_else(|| SheetError::Unknown { what: "title block", value: s.titleblock.clone() })?;
    let mut o = Out { prims: Vec::new(), warnings: Vec::new(), lw };
    let lookup = |n: &str| set.field(s, ctx, n);
    let shown = set.revisions_on(s);

    for it in &tb.items {
        match it {
            Item::Line { a, b, weight } => o.line(*a, *b, *weight, "TITLEBLOCK"),
            Item::Rect { rect, weight } => o.rect(rect, *weight, "TITLEBLOCK"),
            Item::Text { at, height, align, text, bold } => {
                let (t, missing) = text.render(&lookup);
                for m in missing {
                    o.warnings.push(format!("sheet {}: field '{m}' has no value", s.number));
                }
                o.text(*at, *height, *align, t.trim().to_string(), *bold, "TITLEBLOCK");
            }
            Item::Placeholder { rect, label } => {
                o.rect(rect, 2, "TITLEBLOCK");
                let c = rect.centre();
                o.text([c[0], c[1] - 0.8], 1.6, Align::Centre, label.clone(), false, "TITLEBLOCK");
            }
            Item::RevisionTable(t) => {
                let (x0, y0) = (t.origin[0], t.origin[1]);
                let (w, h) = (t.width(), t.height());
                o.rect(&Rect::new(x0, y0, w, h), 3, "TITLEBLOCK");
                for r in 1..=t.rows {
                    let y = y0 + t.row_height * r as f64;
                    o.line([x0, y], [x0 + w, y], 2, "TITLEBLOCK");
                }
                let mut x = x0;
                for c in &t.columns {
                    if x > x0 {
                        o.line([x, y0], [x, y0 + h], 2, "TITLEBLOCK");
                    }
                    o.text([x + 1.0, y0 + 1.4], 2.0, Align::Left, c.header.clone(), true, "TITLEBLOCK");
                    x += c.width;
                }
                let skip = shown.len().saturating_sub(t.rows);
                if skip > 0 {
                    o.warnings.push(format!("sheet {}: {} older revisions do not fit the revision table", s.number, skip));
                }
                for (i, r) in shown.iter().skip(skip).enumerate() {
                    let y = y0 + t.row_height * (i + 1) as f64 + 1.4;
                    let mut x = x0;
                    for c in &t.columns {
                        let v = match c.field {
                            RevField::Number => r.seq.to_string(),
                            RevField::Date => r.date.clone(),
                            RevField::Description => r.description.clone(),
                        };
                        o.text([x + 1.0, y], 2.2, Align::Left, v, false, "TITLEBLOCK");
                        x += c.width;
                    }
                }
            }
        }
    }

    for vp in &s.viewports {
        let view = lib.view(&vp.view).ok_or_else(|| SheetError::Unknown { what: "view", value: vp.view.clone() })?;
        let fr = vp.frame();
        o.rect(&fr, 4, "VIEWPORT");
        let label = vp.title.clone().unwrap_or_else(|| vp.view.clone());
        o.text([fr.x0, fr.y0 - 5.0], 3.5, Align::Left, format!("{label}   1:{}", vp.scale), true, "VIEWPORT");
        let Some(prims) = contents.get(&vp.view) else {
            o.warnings.push(format!("sheet {}: view '{}' has no content", s.number, vp.view));
            continue;
        };
        let k = 1000.0 / f64::from(vp.scale);
        for mp in prims {
            finite(mp.a[0], "model line")?;
            finite(mp.a[1], "model line")?;
            finite(mp.b[0], "model line")?;
            finite(mp.b[1], "model line")?;
            let el = ElementRef { category: &mp.category, params: &mp.params, link: mp.link.as_deref() };
            let Display::Shown { halftone: ht, weight, colour } = lib.display(view, &el)? else { continue };
            let cat = lib.category(&mp.category).expect("display checked the category");
            let to = |p: [f64; 2]| [vp.centre[0] + (p[0] - vp.model_centre[0]) * k, vp.centre[1] + (p[1] - vp.model_centre[1]) * k];
            let Some((a, b)) = clip_segment(to(mp.a), to(mp.b), &fr) else { continue };
            let base = mode.base_colour(colour.unwrap_or([0, 0, 0]));
            o.prims.push(Prim::Line {
                a,
                b,
                weight_mm: lw.model_mm(weight.unwrap_or(mp.weight), vp.scale),
                rgb: if ht { halftone(base) } else { base },
                layer: layer_name(cat.discipline.code(), &cat.name),
            });
            if o.prims.len() > MAX_PRIMS {
                return Err(SheetError::TooMany("drawing primitives"));
            }
        }
    }

    for c in &s.clouds {
        let pts = cloud_polyline(&c.outline, 8.0);
        for w in pts.windows(2) {
            o.line(w[0], w[1], 4, "REVISION");
        }
        let n = c.outline.len() as f64;
        let cx = c.outline.iter().map(|p| p[0]).sum::<f64>() / n;
        let cy = c.outline.iter().map(|p| p[1]).sum::<f64>() / n;
        let tri = [[cx, cy + 3.5], [cx - 3.0, cy - 2.5], [cx + 3.0, cy - 2.5]];
        for i in 0..3 {
            o.line(tri[i], tri[(i + 1) % 3], 3, "REVISION");
        }
        o.text([cx, cy - 1.7], 2.4, Align::Centre, c.revision.to_string(), true, "REVISION");
    }

    Ok(RenderedSheet { number: s.number.clone(), name: s.name.clone(), size: tb.size.clone(), prims: o.prims, warnings: o.warnings })
}

/// Every sheet in set order, in colour.
pub fn render_all(set: &SheetSet, lib: &ViewLibrary, contents: &ViewContents, lw: &LineWeightTable, ctx: &PrintContext) -> Result<Vec<RenderedSheet>, SheetError> {
    render_all_with_mode(set, lib, contents, lw, ctx, PrintMode::Colour)
}

/// Every sheet in set order, using the same output mode.
pub fn render_all_with_mode(set: &SheetSet, lib: &ViewLibrary, contents: &ViewContents, lw: &LineWeightTable, ctx: &PrintContext, mode: PrintMode) -> Result<Vec<RenderedSheet>, SheetError> {
    set.sheets().iter().map(|s| render_sheet_with_mode(set, lib, contents, lw, &s.number, ctx, mode)).collect()
}
