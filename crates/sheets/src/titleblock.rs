//! Title blocks: one template per paper size, with fixed graphics, text bound to named fields and a
//! revision table. Coordinates are millimetres from the bottom-left corner of the sheet, y up.

use crate::paper::{Margins, PaperSize, Rect};
use crate::{clean_name, finite, SheetError, MAX_TEXT};

pub const MAX_ITEMS: usize = 2_000;
pub const MAX_FIELD_NAME: usize = 64;
pub const MIN_TEXT_MM: f64 = 0.5;
pub const MAX_TEXT_MM: f64 = 50.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Centre,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
enum Part {
    Lit(String),
    Field(String),
}

/// Text with `{Field Name}` references. `{{` and `}}` are literal braces.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    source: String,
    parts: Vec<Part>,
}

impl Template {
    pub fn new(src: &str) -> Result<Self, SheetError> {
        if src.chars().count() > MAX_TEXT || src.chars().any(|c| c.is_control()) {
            return Err(SheetError::BadTemplate(format!("'{src}' is too long or has control characters")));
        }
        let cs: Vec<char> = src.chars().collect();
        let mut parts = Vec::new();
        let mut lit = String::new();
        let mut i = 0;
        while i < cs.len() {
            match cs[i] {
                '{' if cs.get(i + 1) == Some(&'{') => {
                    lit.push('{');
                    i += 2;
                }
                '}' if cs.get(i + 1) == Some(&'}') => {
                    lit.push('}');
                    i += 2;
                }
                '{' => {
                    let end = cs[i + 1..].iter().position(|&c| c == '}').ok_or_else(|| SheetError::BadTemplate(format!("'{src}' has an unclosed {{")))?;
                    let name: String = cs[i + 1..i + 1 + end].iter().collect();
                    let name = name.trim().to_string();
                    if name.is_empty() || name.chars().count() > MAX_FIELD_NAME || name.contains('{') {
                        return Err(SheetError::BadTemplate(format!("'{src}' has a bad field name")));
                    }
                    if !lit.is_empty() {
                        parts.push(Part::Lit(std::mem::take(&mut lit)));
                    }
                    parts.push(Part::Field(name));
                    i += end + 2;
                }
                '}' => return Err(SheetError::BadTemplate(format!("'{src}' has a stray }}"))),
                c => {
                    lit.push(c);
                    i += 1;
                }
            }
        }
        if !lit.is_empty() {
            parts.push(Part::Lit(lit));
        }
        Ok(Template { source: src.to_string(), parts })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn fields(&self) -> Vec<&str> {
        self.parts
            .iter()
            .filter_map(|p| if let Part::Field(n) = p { Some(n.as_str()) } else { None })
            .collect()
    }

    /// Text with fields filled in. A field the lookup does not know prints as nothing (as Revit does)
    /// and is returned in the second value so the caller can warn.
    pub fn render(&self, lookup: &dyn Fn(&str) -> Option<String>) -> (String, Vec<String>) {
        let mut out = String::new();
        let mut missing = Vec::new();
        for p in &self.parts {
            match p {
                Part::Lit(s) => out.push_str(s),
                Part::Field(n) => match lookup(n) {
                    Some(v) => out.push_str(&v),
                    None => missing.push(n.clone()),
                },
            }
        }
        (out, missing)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RevField {
    Number,
    Date,
    Description,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RevColumn {
    pub header: String,
    pub width: f64,
    pub field: RevField,
}

/// Revision history. The header row is at the bottom and the history grows upward, oldest first.
#[derive(Debug, Clone, PartialEq)]
pub struct RevisionTable {
    pub origin: [f64; 2],
    pub row_height: f64,
    pub rows: usize,
    pub columns: Vec<RevColumn>,
}

impl RevisionTable {
    pub fn width(&self) -> f64 {
        self.columns.iter().map(|c| c.width).sum()
    }
    pub fn height(&self) -> f64 {
        self.row_height * (self.rows + 1) as f64
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Line { a: [f64; 2], b: [f64; 2], weight: u8 },
    Rect { rect: Rect, weight: u8 },
    Text { at: [f64; 2], height: f64, align: Align, text: Template, bold: bool },
    /// A frame with a small caption: owner logo, designer logo, stamp.
    Placeholder { rect: Rect, label: String },
    RevisionTable(RevisionTable),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TitleBlockDef {
    pub name: String,
    pub size: PaperSize,
    pub margins: Margins,
    pub items: Vec<Item>,
    /// where views may be placed (inside the border, clear of the title block)
    pub free_rect: Rect,
}

fn pt_ok(p: [f64; 2], page: &Rect, what: &str) -> Result<(), SheetError> {
    finite(p[0], "x")?;
    finite(p[1], "y")?;
    if page.contains(p) {
        Ok(())
    } else {
        Err(SheetError::OutOfBounds(what.to_string()))
    }
}

impl TitleBlockDef {
    pub fn page(&self) -> Rect {
        Rect { x0: 0.0, y0: 0.0, x1: self.size.width_mm, y1: self.size.height_mm }
    }

    /// The bold frame inside the margins.
    pub fn border(&self) -> Rect {
        Rect {
            x0: self.margins.left,
            y0: self.margins.bottom,
            x1: self.size.width_mm - self.margins.right,
            y1: self.size.height_mm - self.margins.top,
        }
    }

    pub fn validate(&self) -> Result<(), SheetError> {
        clean_name(&self.name, "title block name")?;
        let m = &self.margins;
        for (v, w) in [(m.left, "left margin"), (m.top, "top margin"), (m.right, "right margin"), (m.bottom, "bottom margin")] {
            finite(v, w)?;
            if v < 0.0 {
                return Err(SheetError::BadValue { what: "margin", value: v.to_string() });
            }
        }
        let b = self.border();
        if b.width() < 20.0 || b.height() < 20.0 {
            return Err(SheetError::OutOfBounds("the border".into()));
        }
        let page = self.page();
        if self.items.len() > MAX_ITEMS {
            return Err(SheetError::TooMany("title block items"));
        }
        for it in &self.items {
            match it {
                Item::Line { a, b, weight } => {
                    pt_ok(*a, &page, "a line")?;
                    pt_ok(*b, &page, "a line")?;
                    weight_ok(*weight)?;
                }
                Item::Rect { rect, weight } => {
                    rect_ok(rect, &page, "a rectangle")?;
                    weight_ok(*weight)?;
                }
                Item::Text { at, height, .. } => {
                    pt_ok(*at, &page, "a text")?;
                    finite(*height, "text height")?;
                    if !(MIN_TEXT_MM..=MAX_TEXT_MM).contains(height) {
                        return Err(SheetError::BadValue { what: "text height", value: height.to_string() });
                    }
                }
                Item::Placeholder { rect, label } => {
                    rect_ok(rect, &page, "a placeholder")?;
                    if label.chars().count() > MAX_TEXT || label.chars().any(|c| c.is_control()) {
                        return Err(SheetError::BadValue { what: "placeholder label", value: label.clone() });
                    }
                }
                Item::RevisionTable(t) => {
                    pt_ok(t.origin, &page, "the revision table")?;
                    finite(t.row_height, "row height")?;
                    if t.row_height <= 0.0 || t.rows == 0 || t.rows > 200 || t.columns.is_empty() || t.columns.len() > 20 {
                        return Err(SheetError::BadValue { what: "revision table", value: format!("{} rows", t.rows) });
                    }
                    if t.columns.iter().any(|c| !c.width.is_finite() || c.width <= 0.0) {
                        return Err(SheetError::BadValue { what: "revision column width", value: String::new() });
                    }
                    let r = Rect::new(t.origin[0], t.origin[1], t.width(), t.height());
                    rect_ok(&r, &page, "the revision table")?;
                }
            }
        }
        let f = &self.free_rect;
        for v in [f.x0, f.y0, f.x1, f.y1] {
            finite(v, "free area")?;
        }
        if f.x1 <= f.x0 || f.y1 <= f.y0 || !f.inside(&b) {
            return Err(SheetError::OutOfBounds("the free area".into()));
        }
        Ok(())
    }

    /// Every field the text items ask for, once each, in order of first use.
    pub fn fields(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for it in &self.items {
            if let Item::Text { text, .. } = it {
                for f in text.fields() {
                    if !out.iter().any(|o| o == f) {
                        out.push(f.to_string());
                    }
                }
            }
        }
        out
    }

    /// A title block holding the usual fields of a design drawing: owner and designer logo boxes, stamp
    /// box, project and drawing titles, who designed, drew, checked and approved with dates, scale, size,
    /// drawing number, revision, sheet X of Y, a revision table, and plot data. It is a placeholder to be
    /// replaced by the company template; margins are the caller's choice.
    pub fn generic(name: &str, size: PaperSize, margins: Margins) -> Result<Self, SheetError> {
        let mut tb = TitleBlockDef {
            name: name.to_string(),
            size,
            margins,
            items: Vec::new(),
            free_rect: Rect::new(0.0, 0.0, 1.0, 1.0),
        };
        let b = tb.border();
        if b.width() < 150.0 || b.height() < 100.0 {
            return Err(SheetError::BadPaper("too small for the generic title block".into()));
        }
        let pw = (0.42 * b.width()).min(180.0);
        let px = b.x1 - pw;
        let py = b.y0;
        let ph = 70.0;
        let t = |s: &str| Template::new(s);
        let mut it: Vec<Item> = Vec::new();
        it.push(Item::Rect { rect: Rect::new(b.x0, b.y0, b.width(), b.height()), weight: 6 });
        it.push(Item::Rect { rect: Rect::new(px, py, pw, ph), weight: 5 });
        for y in [10.0, 30.0, 46.0, 58.0] {
            it.push(Item::Line { a: [px, py + y], b: [px + pw, py + y], weight: 3 });
        }
        // bottom row: scale | size | number | revision
        let cuts = [0.2, 0.4, 0.8];
        for c in cuts {
            it.push(Item::Line { a: [px + c * pw, py], b: [px + c * pw, py + 10.0], weight: 3 });
        }
        let cells = [(0.0, "Scale", "{Scale}"), (0.2, "Size", "{Sheet Size}"), (0.4, "Drawing number", "{Sheet Number}"), (0.8, "Rev.", "{Current Revision}")];
        for (c, label, field) in cells {
            let x = px + c * pw + 1.0;
            it.push(Item::Text { at: [x, py + 7.5], height: 1.6, align: Align::Left, text: t(label)?, bold: false });
            it.push(Item::Text { at: [x, py + 2.0], height: 3.5, align: Align::Left, text: t(field)?, bold: true });
        }
        // sign-off rows with the stamp box on the right
        let sx = px + 0.7 * pw;
        it.push(Item::Line { a: [sx, py + 10.0], b: [sx, py + 30.0], weight: 3 });
        it.push(Item::Placeholder { rect: Rect::new(sx, py + 10.0, 0.3 * pw, 20.0), label: "Stamp".into() });
        let rows = [("Design by", "{Designed By}", "{Design Date}"), ("Drawn by", "{Drawn By}", "{Drawn Date}"), ("Check by", "{Checked By}", "{Check Date}"), ("Appr. by", "{Approved By}", "{Approval Date}")];
        for (i, (label, who, when)) in rows.iter().enumerate() {
            let y = py + 10.0 + 5.0 * i as f64;
            if i > 0 {
                it.push(Item::Line { a: [px, y], b: [sx, y], weight: 2 });
            }
            it.push(Item::Text { at: [px + 1.0, y + 1.4], height: 2.2, align: Align::Left, text: t(label)?, bold: false });
            it.push(Item::Text { at: [px + 0.2 * pw, y + 1.4], height: 2.4, align: Align::Left, text: t(who)?, bold: false });
            it.push(Item::Text { at: [px + 0.45 * pw, y + 1.4], height: 2.4, align: Align::Left, text: t(when)?, bold: false });
        }
        // titles
        it.push(Item::Text { at: [px + 1.0, py + 42.0], height: 2.0, align: Align::Left, text: t("{Drawing Type}")?, bold: false });
        it.push(Item::Text { at: [px + 1.0, py + 33.0], height: 5.0, align: Align::Left, text: t("{Sheet Name}")?, bold: true });
        it.push(Item::Text { at: [px + 1.0, py + 52.0], height: 4.0, align: Align::Left, text: t("{Project Name}")?, bold: true });
        it.push(Item::Text { at: [px + 1.0, py + 48.0], height: 2.2, align: Align::Left, text: t("{Facility} {Area}")?, bold: false });
        it.push(Item::Text { at: [px + pw - 1.0, py + 48.0], height: 2.2, align: Align::Right, text: t("Sheet {Sheet Index} of {Sheet Count}")?, bold: false });
        // logos
        it.push(Item::Placeholder { rect: Rect::new(px, py + 58.0, 0.5 * pw, 12.0), label: "Owner logo".into() });
        it.push(Item::Placeholder { rect: Rect::new(px + 0.5 * pw, py + 58.0, 0.5 * pw, 12.0), label: "Designer logo".into() });
        // revision history above the block, header at the bottom
        it.push(Item::RevisionTable(RevisionTable {
            origin: [px, py + ph],
            row_height: 5.0,
            rows: 6,
            columns: vec![
                RevColumn { header: "Rev.".into(), width: 0.1 * pw, field: RevField::Number },
                RevColumn { header: "Date".into(), width: 0.2 * pw, field: RevField::Date },
                RevColumn { header: "Description".into(), width: 0.7 * pw, field: RevField::Description },
            ],
        }));
        // plot data inside the border, bottom left
        it.push(Item::Text { at: [b.x0 + 2.0, b.y0 + 2.0], height: 1.8, align: Align::Left, text: t("{File Path}  {Plot Date}")?, bold: false });
        tb.items = it;
        tb.free_rect = Rect { x0: b.x0 + 2.0, y0: b.y0 + 6.0, x1: px - 2.0, y1: b.y1 - 2.0 };
        tb.validate()?;
        Ok(tb)
    }
}

fn weight_ok(w: u8) -> Result<(), SheetError> {
    if (1..=16).contains(&w) {
        Ok(())
    } else {
        Err(SheetError::BadValue { what: "line weight number", value: w.to_string() })
    }
}

fn rect_ok(r: &Rect, page: &Rect, what: &str) -> Result<(), SheetError> {
    for v in [r.x0, r.y0, r.x1, r.y1] {
        finite(v, "rectangle")?;
    }
    if r.x1 <= r.x0 || r.y1 <= r.y0 || !r.inside(page) {
        return Err(SheetError::OutOfBounds(what.to_string()));
    }
    Ok(())
}
