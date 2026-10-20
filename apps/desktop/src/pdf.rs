//! A small PDF writer for the printed sheets: pages of vector paths (lines, filled and stroked polygons with holes,
//! circles, dashes) and text in the standard Helvetica fonts, which every PDF reader has. Coordinates are millimetres
//! from the top left of the page, y down, as the sheets are laid out.
use std::fmt::Write as _;

const PT_PER_MM: f64 = 72.0 / 25.4;

/// Widths of the characters 32 to 126 of Helvetica, in thousandths of the text size.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556,
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556,
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];
/// Helvetica Bold is wider; this is its width for a typical mix, as a factor of the regular width.
const BOLD_FACTOR: f64 = 1.07;

/// The width in millimetres of `text` set at `size_mm`.
pub fn text_width_mm(text: &str, size_mm: f64, bold: bool) -> f64 {
    let units: u32 = text.chars().map(|c| match c as u32 { 32..=126 => u32::from(HELVETICA[c as usize - 32]), 160..=255 => 556, _ => 556 }).sum();
    f64::from(units) / 1000.0 * size_mm * if bold { BOLD_FACTOR } else { 1.0 }
}

/// How text sits at its point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align { Left, Center, Right }

/// A colour, red green blue.
pub type Rgb = [u8; 3];

fn colour(c: Rgb) -> String { format!("{:.3} {:.3} {:.3}", f64::from(c[0]) / 255.0, f64::from(c[1]) / 255.0, f64::from(c[2]) / 255.0) }

fn n(x: f64) -> String {
    let s = format!("{x:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" || s == "-0" { "0".into() } else { s.to_string() }
}

/// The bytes of `text` in the WinAnsi encoding of the standard fonts, escaped for a PDF string; what the encoding does
/// not have becomes `?`.
fn encode(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        let b = match c as u32 { 32..=126 => c as u8, 160..=255 => c as u8, 0x20AC => 128, 0x2013 => 150, 0x2014 => 151, 0x2018 | 0x2019 => 39, 0x201C | 0x201D => 34, 0x2022 => 149, _ => b'?' };
        match b {
            b'(' | b')' | b'\\' => { out.push('\\'); out.push(b as char); }
            32..=126 => out.push(b as char),
            _ => { let _ = write!(out, "\\{b:03o}"); }
        }
    }
    out
}

/// One page.
pub struct Page {
    pub w_mm: f64,
    pub h_mm: f64,
    content: String,
}

impl Page {
    pub fn new(w_mm: f64, h_mm: f64) -> Page { Page { w_mm, h_mm, content: String::new() } }

    fn x(&self, x: f64) -> String { n(x * PT_PER_MM) }
    fn y(&self, y: f64) -> String { n((self.h_mm - y) * PT_PER_MM) }

    fn path(&mut self, rings: &[Vec<[f64; 2]>]) {
        for ring in rings {
            for (i, p) in ring.iter().enumerate() {
                let _ = writeln!(self.content, "{} {} {}", self.x(p[0]), self.y(p[1]), if i == 0 { "m" } else { "l" });
            }
            self.content.push_str("h\n");
        }
    }

    fn stroke_style(&mut self, rgb: Rgb, w_mm: f64, dash: Option<(f64, f64)>) {
        let _ = writeln!(self.content, "{} RG {} w", colour(rgb), n(w_mm * PT_PER_MM));
        match dash { Some((d, g)) => { let _ = writeln!(self.content, "[{} {}] 0 d", n(d * PT_PER_MM), n(g * PT_PER_MM)); } None => self.content.push_str("[] 0 d\n") }
    }

    /// Closed rings, filled (an inner ring is a hole) and/or outlined.
    pub fn polygon(&mut self, rings: &[Vec<[f64; 2]>], fill: Option<Rgb>, stroke: Option<(Rgb, f64)>) {
        let rings: Vec<Vec<[f64; 2]>> = rings.iter().filter(|r| r.len() >= 3 && r.iter().all(|p| p[0].is_finite() && p[1].is_finite())).cloned().collect();
        if rings.is_empty() || (fill.is_none() && stroke.is_none()) { return; }
        self.content.push_str("q\n");
        if let Some(f) = fill { let _ = writeln!(self.content, "{} rg", colour(f)); }
        if let Some((c, w)) = stroke { self.stroke_style(c, w, None); self.content.push_str("1 j\n"); }
        self.path(&rings);
        self.content.push_str(match (fill.is_some(), stroke.is_some()) { (true, true) => "B*\n", (true, false) => "f*\n", _ => "S\n" });
        self.content.push_str("Q\n");
    }

    pub fn line(&mut self, a: [f64; 2], b: [f64; 2], rgb: Rgb, w_mm: f64, dash: Option<(f64, f64)>) {
        if ![a[0], a[1], b[0], b[1]].iter().all(|x| x.is_finite()) { return; }
        self.content.push_str("q\n");
        self.stroke_style(rgb, w_mm, dash);
        let _ = writeln!(self.content, "{} {} m {} {} l S\nQ", self.x(a[0]), self.y(a[1]), self.x(b[0]), self.y(b[1]));
    }

    pub fn circle(&mut self, c: [f64; 2], r: f64, fill: Option<Rgb>, stroke: Option<(Rgb, f64)>) {
        if !(c[0].is_finite() && c[1].is_finite() && r.is_finite() && r > 0.0) || (fill.is_none() && stroke.is_none()) { return; }
        const K: f64 = 0.552_284_75;
        self.content.push_str("q\n");
        if let Some(f) = fill { let _ = writeln!(self.content, "{} rg", colour(f)); }
        if let Some((col, w)) = stroke { self.stroke_style(col, w, None); }
        let (x, y) = (c[0], c[1]);
        let mut path = String::new();
        {
            let p = |a: f64, b: f64| format!("{} {}", n(a * PT_PER_MM), n((self.h_mm - b) * PT_PER_MM));
            let _ = writeln!(path, "{} m", p(x + r, y));
            for (c1, c2, e) in [
                ((x + r, y + K * r), (x + K * r, y + r), (x, y + r)),
                ((x - K * r, y + r), (x - r, y + K * r), (x - r, y)),
                ((x - r, y - K * r), (x - K * r, y - r), (x, y - r)),
                ((x + K * r, y - r), (x + r, y - K * r), (x + r, y)),
            ] {
                let _ = writeln!(path, "{} {} {} c", p(c1.0, c1.1), p(c2.0, c2.1), p(e.0, e.1));
            }
        }
        self.content.push_str(&path);
        self.content.push_str(match (fill.is_some(), stroke.is_some()) { (true, true) => "h B\n", (true, false) => "h f\n", _ => "h S\n" });
        self.content.push_str("Q\n");
    }

    /// Text with its baseline at `at`, `size_mm` high.
    #[cfg(test)]
    pub fn text(&mut self, at: [f64; 2], size_mm: f64, text: &str, align: Align, rgb: Rgb, bold: bool) { self.text_rotated(at, size_mm, text, align, rgb, bold, 0.0); }

    /// The same turned `degrees` counter-clockwise about `at`.
    #[allow(clippy::too_many_arguments)]
    pub fn text_rotated(&mut self, at: [f64; 2], size_mm: f64, text: &str, align: Align, rgb: Rgb, bold: bool, degrees: f64) {
        if text.is_empty() || !(at[0].is_finite() && at[1].is_finite() && size_mm.is_finite() && size_mm > 0.0 && degrees.is_finite()) { return; }
        let w = text_width_mm(text, size_mm, bold);
        let back = match align { Align::Left => 0.0, Align::Center => w / 2.0, Align::Right => w };
        let (s, c) = degrees.to_radians().sin_cos();
        let _ = writeln!(self.content, "q\nBT\n{} rg\n/{} {} Tf\n{} {} {} {} {} {} Tm\n({}) Tj\nET\nQ", colour(rgb), if bold { "F2" } else { "F1" }, n(size_mm * PT_PER_MM),
            n(c), n(s), n(-s), n(c), n((at[0] - back * c) * PT_PER_MM), n((self.h_mm - (at[1] + back * s)) * PT_PER_MM), encode(text));
    }
}

/// A document of pages.
#[derive(Default)]
pub struct Pdf {
    pub title: String,
    pages: Vec<Page>,
}

impl Pdf {
    pub fn new(title: &str) -> Pdf { Pdf { title: title.to_string(), pages: Vec::new() } }

    pub fn add(&mut self, page: Page) { self.pages.push(page); }

    pub fn pages(&self) -> usize { self.pages.len() }

    /// The file. A document of no pages has one empty A4 page, because a PDF needs one.
    pub fn finish(mut self) -> Vec<u8> {
        if self.pages.is_empty() { self.pages.push(Page::new(297.0, 210.0)); }
        // 1 catalog, 2 pages, 3 and 4 fonts, 5 info, then a page and its content for each
        let count = self.pages.len();
        let mut objs: Vec<String> = Vec::new();
        objs.push("<< /Type /Catalog /Pages 2 0 R >>".into());
        let kids: Vec<String> = (0..count).map(|i| format!("{} 0 R", 6 + 2 * i)).collect();
        objs.push(format!("<< /Type /Pages /Kids [{}] /Count {count} >>", kids.join(" ")));
        objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into());
        objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".into());
        objs.push(format!("<< /Title ({}) /Producer (OpenBIM) >>", encode(&self.title)));
        for (i, p) in self.pages.iter().enumerate() {
            objs.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>", n(p.w_mm * PT_PER_MM), n(p.h_mm * PT_PER_MM), 7 + 2 * i));
            objs.push(format!("<< /Length {} >>\nstream\n{}endstream", p.content.len(), p.content));
        }
        let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = Vec::new();
        for (i, o) in objs.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
        for o in offsets { out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes()); }
        out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_measured_with_the_widths_of_helvetica() {
        // "Hello": H 722 + e 556 + l 222 + l 222 + o 556 = 2278
        assert!((text_width_mm("Hello", 10.0, false) - 22.78).abs() < 1e-9);
        assert!(text_width_mm("Hello", 10.0, true) > 22.78);
        assert_eq!(text_width_mm("", 10.0, false), 0.0);
        assert!(text_width_mm("é日", 10.0, false) > 0.0, "what the table has not is given a width");
    }

    #[test]
    fn text_is_escaped_and_encoded() {
        assert_eq!(encode("a(b)c\\d"), "a\\(b\\)c\\\\d");
        assert_eq!(encode("é"), "\\351");
        assert_eq!(encode("日本"), "??");
        assert_eq!(encode("€"), "\\200");
    }

    #[test]
    fn numbers_are_short_and_never_negative_zero() {
        assert_eq!(n(1.0), "1");
        assert_eq!(n(1.5), "1.5");
        assert_eq!(n(-0.0001), "0");
        assert_eq!(n(0.0), "0");
        assert_eq!(n(12.34567), "12.346");
    }

    #[test]
    fn a_pdf_has_its_objects_where_the_table_says_and_a_page_for_each() {
        let mut pdf = Pdf::new("Sheets (A101)");
        for k in 0..3 {
            let mut p = Page::new(420.0, 297.0);
            p.polygon(&[vec![[10.0, 10.0], [100.0, 10.0], [100.0, 50.0], [10.0, 50.0]], vec![[20.0, 20.0], [30.0, 20.0], [30.0, 30.0]]], Some([200, 200, 200]), Some(([0, 0, 0], 0.25)));
            p.line([0.0, 0.0], [420.0, 297.0], [255, 0, 0], 0.5, Some((4.0, 2.0)));
            p.circle([50.0, 50.0], 4.0, None, Some(([0, 0, 0], 0.2)));
            p.text([60.0, 60.0], 3.5, &format!("Page {k} (é)"), Align::Center, [0, 0, 0], true);
            pdf.add(p);
        }
        assert_eq!(pdf.pages(), 3);
        let bytes = pdf.finish();
        // one character a byte, so that offsets of the file are offsets of the text
        let text: String = bytes.iter().map(|&b| if b < 128 { b as char } else { '?' }).collect();
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.trim_end().ends_with("%%EOF"));
        assert!(text.contains("/Count 3"));
        assert!(text.contains("/MediaBox [0 0 1190.551 841.89]"), "A3 in points");
        assert!(text.contains("f*") || text.contains("B*"), "a polygon with a hole is filled even-odd");
        assert!(text.contains("Sheets \\(A101\\)"));
        // every entry of the cross-reference table points at "n 0 obj"
        let start: usize = text.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        assert!(text[start..].starts_with("xref"));
        let table: Vec<&str> = text[start..].lines().skip(2).take_while(|l| !l.starts_with("trailer")).collect();
        assert_eq!(table.len(), 1 + 5 + 2 * 3, "the free entry, the catalog, pages, two fonts, the info, and a page and its content for each of 3 pages");
        for (i, line) in table.iter().enumerate().skip(1) {
            let at: usize = line[..10].parse().unwrap();
            assert!(text[at..].starts_with(&format!("{i} 0 obj")), "object {i} at {at}");
        }
        // the stream lengths are right
        let mut rest = text.as_str();
        let mut streams = 0;
        while let Some(i) = rest.find("/Length ") {
            rest = &rest[i + 8..];
            let len: usize = rest[..rest.find(' ').unwrap()].parse().unwrap();
            let s = rest.find("stream\n").unwrap() + 7;
            assert!(rest[s + len..].starts_with("endstream"), "a stream of {len} bytes");
            streams += 1;
        }
        assert_eq!(streams, 3);
    }

    #[test]
    fn an_empty_document_still_has_a_page() {
        let bytes = Pdf::new("x").finish();
        let text = String::from_utf8_lossy(&bytes).to_string();
        assert!(text.contains("/Count 1"));
    }

    #[test]
    fn bad_numbers_draw_nothing_and_do_not_break_the_file() {
        let mut p = Page::new(100.0, 100.0);
        p.line([f64::NAN, 0.0], [1.0, 1.0], [0, 0, 0], 0.1, None);
        p.circle([1.0, 1.0], f64::INFINITY, None, Some(([0, 0, 0], 0.1)));
        p.polygon(&[vec![[0.0, 0.0], [f64::NAN, 1.0], [1.0, 1.0]]], Some([1, 2, 3]), None);
        p.text([f64::NAN, 0.0], 3.0, "x", Align::Left, [0, 0, 0], false);
        p.polygon(&[vec![[0.0, 0.0], [1.0, 1.0]]], Some([1, 2, 3]), None);
        assert!(p.content.is_empty(), "{}", p.content);
    }
}
