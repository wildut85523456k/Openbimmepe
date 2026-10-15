//! Paper sizes. Sizes are stored landscape (width is the longer side); `portrait` turns one.

use crate::{finite, SheetError};

pub const MIN_PAPER_MM: f64 = 50.0;
pub const MAX_PAPER_MM: f64 = 5_000.0;

#[derive(Debug, Clone, PartialEq)]
pub struct PaperSize {
    pub name: String,
    pub width_mm: f64,
    pub height_mm: f64,
}

impl PaperSize {
    /// ISO A series, A0 to A4 (landscape, millimetres).
    pub fn iso_a(n: u8) -> Option<Self> {
        let (w, h) = match n {
            0 => (1189.0, 841.0),
            1 => (841.0, 594.0),
            2 => (594.0, 420.0),
            3 => (420.0, 297.0),
            4 => (297.0, 210.0),
            _ => return None,
        };
        Some(PaperSize { name: format!("A{n}"), width_mm: w, height_mm: h })
    }

    /// ANSI sheets A (letter) to E, from inches, landscape.
    pub fn ansi(letter: char) -> Option<Self> {
        let (w, h) = match letter.to_ascii_uppercase() {
            'A' => (11.0, 8.5),
            'B' => (17.0, 11.0),
            'C' => (22.0, 17.0),
            'D' => (34.0, 22.0),
            'E' => (44.0, 34.0),
            _ => return None,
        };
        Some(PaperSize { name: format!("ANSI {}", letter.to_ascii_uppercase()), width_mm: w * 25.4, height_mm: h * 25.4 })
    }

    pub fn custom(name: &str, w: f64, h: f64) -> Result<Self, SheetError> {
        crate::clean_name(name, "paper name")?;
        finite(w, "paper width")?;
        finite(h, "paper height")?;
        if !(MIN_PAPER_MM..=MAX_PAPER_MM).contains(&w) || !(MIN_PAPER_MM..=MAX_PAPER_MM).contains(&h) {
            return Err(SheetError::BadPaper(format!("{w} x {h} mm is outside {MIN_PAPER_MM} to {MAX_PAPER_MM} mm")));
        }
        Ok(PaperSize { name: name.to_string(), width_mm: w.max(h), height_mm: w.min(h) })
    }

    /// "A1", "a3", "ISO A2", "ANSI D".
    pub fn parse(text: &str) -> Option<Self> {
        let t: String = text.trim().to_ascii_uppercase().split_whitespace().collect::<Vec<_>>().join(" ");
        let t = t.strip_prefix("ISO ").unwrap_or(&t).to_string();
        if let Some(rest) = t.strip_prefix("ANSI ") {
            let mut c = rest.chars();
            let l = c.next()?;
            return if c.next().is_none() { Self::ansi(l) } else { None };
        }
        let mut c = t.chars();
        if c.next()? == 'A' {
            let d: String = c.collect();
            if d.len() == 1 {
                return Self::iso_a(d.parse().ok()?);
            }
        }
        None
    }

    /// Width and height swapped (a tall sheet).
    pub fn portrait(&self) -> (f64, f64) {
        (self.height_mm, self.width_mm)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Margins {
    pub fn all(v: f64) -> Self {
        Margins { left: v, top: v, right: v, bottom: v }
    }
}

/// An axis-aligned rectangle, x0 < x1 and y0 < y1, origin bottom-left of the sheet, y up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Rect { x0: x, y0: y, x1: x + w, y1: y + h }
    }
    pub fn contains(&self, p: [f64; 2]) -> bool {
        p[0] >= self.x0 && p[0] <= self.x1 && p[1] >= self.y0 && p[1] <= self.y1
    }
    pub fn inside(&self, o: &Rect) -> bool {
        self.x0 >= o.x0 && self.y0 >= o.y0 && self.x1 <= o.x1 && self.y1 <= o.y1
    }
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }
    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }
    pub fn centre(&self) -> [f64; 2] {
        [(self.x0 + self.x1) / 2.0, (self.y0 + self.y1) / 2.0]
    }
}
