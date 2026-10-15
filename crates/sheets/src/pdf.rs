//! Vector PDF (1.4), one page per sheet at true size, written by hand: lines with pen widths and colours,
//! text in the standard Helvetica and Helvetica-Bold fonts (WinAnsi, so Western European text works).
//! Characters outside WinAnsi print as '?' and are reported; other scripts need an embedded font.

use crate::paper::PaperSize;
use crate::render::{Prim, RenderedSheet};
use crate::titleblock::Align;

const PT_PER_MM: f64 = 72.0 / 25.4;

/// Advance widths in 1/1000 em for characters 32 to 126.
const HELV: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722,
    722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556,
    556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];
const HELV_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722,
    722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611,
    611, 278, 278, 556, 278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

/// The WinAnsi byte for a character, if it has one.
fn winansi(c: char) -> Option<u8> {
    let u = c as u32;
    match u {
        32..=126 | 0xA0..=0xFF => Some(u as u8),
        0x20AC => Some(0x80),
        0x2018 => Some(0x91),
        0x2019 => Some(0x92),
        0x201C => Some(0x93),
        0x201D => Some(0x94),
        0x2022 => Some(0x95),
        0x2013 => Some(0x96),
        0x2014 => Some(0x97),
        _ => None,
    }
}

fn base_letter(c: char) -> char {
    match c {
        'À'..='Å' => 'A',
        'à'..='å' => 'a',
        'Ç' => 'C',
        'ç' => 'c',
        'È'..='Ë' => 'E',
        'è'..='ë' => 'e',
        'Ì'..='Ï' => 'I',
        'ì'..='ï' => 'i',
        'Ñ' => 'N',
        'ñ' => 'n',
        'Ò'..='Ö' | 'Ø' => 'O',
        'ò'..='ö' | 'ø' => 'o',
        'Ù'..='Ü' => 'U',
        'ù'..='ü' => 'u',
        'Ý' => 'Y',
        'ý' | 'ÿ' => 'y',
        other => other,
    }
}

/// Width of a string in millimetres at a font size given in millimetres. Accented letters take the width
/// of their base letter, which matches Helvetica; other non-ASCII characters count as 556.
pub fn text_width_mm(text: &str, size_mm: f64, bold: bool) -> f64 {
    let t = if bold { &HELV_BOLD } else { &HELV };
    let units: u32 = text
        .chars()
        .map(|c| {
            let b = base_letter(c);
            if (' '..='~').contains(&b) {
                u32::from(t[(b as usize) - 32])
            } else {
                556
            }
        })
        .sum();
    f64::from(units) * size_mm / 1000.0
}

fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn escape_into(out: &mut Vec<u8>, bytes: &[u8]) {
    for &b in bytes {
        match b {
            b'\\' | b'(' | b')' => {
                out.push(b'\\');
                out.push(b);
            }
            _ => out.push(b),
        }
    }
}

pub struct PdfOutput {
    pub bytes: Vec<u8>,
    pub warnings: Vec<String>,
}

fn encode(text: &str, bad: &mut Vec<char>) -> Vec<u8> {
    text.chars()
        .map(|c| match winansi(c) {
            Some(b) => b,
            None => {
                if !bad.contains(&c) {
                    bad.push(c);
                }
                b'?'
            }
        })
        .collect()
}

fn page_stream(s: &RenderedSheet, bad: &mut Vec<char>) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::new();
    c.extend_from_slice(b"1 J 1 j\n");
    for p in &s.prims {
        match p {
            Prim::Line { a, b, weight_mm, rgb, .. } => {
                let col = |v: u8| num(f64::from(v) / 255.0);
                c.extend_from_slice(
                    format!(
                        "{} w {} {} {} RG {} {} m {} {} l S\n",
                        num(weight_mm * PT_PER_MM),
                        col(rgb[0]),
                        col(rgb[1]),
                        col(rgb[2]),
                        num(a[0] * PT_PER_MM),
                        num(a[1] * PT_PER_MM),
                        num(b[0] * PT_PER_MM),
                        num(b[1] * PT_PER_MM)
                    )
                    .as_bytes(),
                );
            }
            Prim::Text { at, height, align, text, bold, .. } => {
                let w = text_width_mm(text, *height, *bold);
                let x = match align {
                    Align::Left => at[0],
                    Align::Centre => at[0] - w / 2.0,
                    Align::Right => at[0] - w,
                };
                c.extend_from_slice(format!("BT 0 g /{} {} Tf {} {} Td (", if *bold { "F2" } else { "F1" }, num(height * PT_PER_MM), num(x * PT_PER_MM), num(at[1] * PT_PER_MM)).as_bytes());
                escape_into(&mut c, &encode(text, bad));
                c.extend_from_slice(b") Tj ET\n");
            }
        }
    }
    c
}

/// Write the sheets as a PDF. `title` goes into the document information.
pub fn write_pdf(sheets: &[RenderedSheet], title: &str) -> PdfOutput {
    let mut bad: Vec<char> = Vec::new();
    // object numbers: 1 catalog, 2 pages, 3 and 4 fonts, 5 info, then page and content per sheet
    let mut objs: Vec<Vec<u8>> = Vec::new();
    let first_page = 6;
    let kids: Vec<String> = (0..sheets.len()).map(|i| format!("{} 0 R", first_page + 2 * i)).collect();
    objs.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), sheets.len()).into_bytes());
    objs.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec());
    objs.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".to_vec());
    let mut info = b"<< /Producer (openbim-sheets) /Title (".to_vec();
    escape_into(&mut info, &encode(title, &mut bad));
    info.extend_from_slice(b") >>");
    objs.push(info);
    for (i, s) in sheets.iter().enumerate() {
        let PaperSize { width_mm, height_mm, .. } = s.size;
        objs.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
                num(width_mm * PT_PER_MM),
                num(height_mm * PT_PER_MM),
                first_page + 2 * i + 1
            )
            .into_bytes(),
        );
        let body = page_stream(s, &mut bad);
        let mut o = format!("<< /Length {} >>\nstream\n", body.len()).into_bytes();
        o.extend_from_slice(&body);
        o.extend_from_slice(b"endstream");
        objs.push(o);
    }
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    let mut warnings: Vec<String> = sheets.iter().flat_map(|s| s.warnings.iter().cloned()).collect();
    if !bad.is_empty() {
        let list: String = bad.iter().collect();
        warnings.push(format!("characters not in the PDF font were printed as '?': {list}"));
    }
    PdfOutput { bytes: out, warnings }
}
