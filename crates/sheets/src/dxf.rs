//! DXF export of one sheet in millimetres, in the old R12 (AC1009) form every CAD program opens.
//! R12 has no line weights, so each line goes on a layer named after its content and pen width
//! (`M-DUCTS_W025` is 0.25 mm) for the receiving program to map to pens; colours are the nearest AutoCAD
//! colour index. Text is left, centre or right justified.

use std::collections::BTreeSet;
use std::fmt::Write;

use crate::render::{Prim, RenderedSheet};
use crate::titleblock::Align;

// AutoCAD 2020 default modelspace reference palette (background RGB 33,40,48),
// pinned to ezdxf 1.4.4 DXF_DEFAULT_COLORS. RGB appearance varies by CAD palette.
// Reference: https://ezdxf.readthedocs.io/en/stable/colors.html#default-palettes
// Index 0 is a placeholder, not an exportable colour.
const ACI_RGB: [u32; 256] = [
    0x000000, 0xFF0000, 0xFFFF00, 0x00FF00, 0x00FFFF, 0x0000FF, 0xFF00FF, 0xFFFFFF, 0x808080, 0xC0C0C0, 0xFF0000, 0xFF7F7F, 0xA50000, 0xA55252, 0x7F0000, 0x7F3F3F, 0x4C0000, 0x4C2626, 0x260000, 0x261313, 0xFF3F00, 0xFF9F7F, 0xA52900,
    0xA56752, 0x7F1F00, 0x7F4F3F, 0x4C1300, 0x4C2F26, 0x260900, 0x261713, 0xFF7F00, 0xFFBF7F, 0xA55200, 0xA57C52, 0x7F3F00, 0x7F5F3F, 0x4C2600, 0x4C3926, 0x261300, 0x261C13, 0xFFBF00, 0xFFDF7F, 0xA57C00, 0xA59152, 0x7F5F00, 0x7F6F3F,
    0x4C3900, 0x4C4226, 0x261C00, 0x262113, 0xFFFF00, 0xFFFF7F, 0xA5A500, 0xA5A552, 0x7F7F00, 0x7F7F3F, 0x4C4C00, 0x4C4C26, 0x262600, 0x262613, 0xBFFF00, 0xDFFF7F, 0x7CA500, 0x91A552, 0x5F7F00, 0x6F7F3F, 0x394C00, 0x424C26, 0x1C2600,
    0x212613, 0x7FFF00, 0xBFFF7F, 0x52A500, 0x7CA552, 0x3F7F00, 0x5F7F3F, 0x264C00, 0x394C26, 0x132600, 0x1C2613, 0x3FFF00, 0x9FFF7F, 0x29A500, 0x67A552, 0x1F7F00, 0x4F7F3F, 0x134C00, 0x2F4C26, 0x092600, 0x172613, 0x00FF00, 0x7FFF7F,
    0x00A500, 0x52A552, 0x007F00, 0x3F7F3F, 0x004C00, 0x264C26, 0x002600, 0x132613, 0x00FF3F, 0x7FFF9F, 0x00A529, 0x52A567, 0x007F1F, 0x3F7F4F, 0x004C13, 0x264C2F, 0x002609, 0x135817, 0x00FF7F, 0x7FFFBF, 0x00A552, 0x52A57C, 0x007F3F,
    0x3F7F5F, 0x004C26, 0x264C39, 0x002613, 0x13581C, 0x00FFBF, 0x7FFFDF, 0x00A57C, 0x52A591, 0x007F5F, 0x3F7F6F, 0x004C39, 0x264C42, 0x00261C, 0x135858, 0x00FFFF, 0x7FFFFF, 0x00A5A5, 0x52A5A5, 0x007F7F, 0x3F7F7F, 0x004C4C, 0x264C4C,
    0x002626, 0x135858, 0x00BFFF, 0x7FDFFF, 0x007CA5, 0x5291A5, 0x005F7F, 0x3F6F7F, 0x00394C, 0x26427E, 0x001C26, 0x135858, 0x007FFF, 0x7FBFFF, 0x0052A5, 0x527CA5, 0x003F7F, 0x3F5F7F, 0x00264C, 0x26397E, 0x001326, 0x131C58, 0x003FFF,
    0x7F9FFF, 0x0029A5, 0x5267A5, 0x001F7F, 0x3F4F7F, 0x00134C, 0x262F7E, 0x000926, 0x131758, 0x0000FF, 0x7F7FFF, 0x0000A5, 0x5252A5, 0x00007F, 0x3F3F7F, 0x00004C, 0x26267E, 0x000026, 0x131358, 0x3F00FF, 0x9F7FFF, 0x2900A5, 0x6752A5,
    0x1F007F, 0x4F3F7F, 0x13004C, 0x2F267E, 0x090026, 0x171358, 0x7F00FF, 0xBF7FFF, 0x5200A5, 0x7C52A5, 0x3F007F, 0x5F3F7F, 0x26004C, 0x39267E, 0x130026, 0x1C1358, 0xBF00FF, 0xDF7FFF, 0x7C00A5, 0x9152A5, 0x5F007F, 0x6F3F7F, 0x39004C,
    0x42264C, 0x1C0026, 0x581358, 0xFF00FF, 0xFF7FFF, 0xA500A5, 0xA552A5, 0x7F007F, 0x7F3F7F, 0x4C004C, 0x4C264C, 0x260026, 0x581358, 0xFF00BF, 0xFF7FDF, 0xA5007C, 0xA55291, 0x7F005F, 0x7F3F6F, 0x4C0039, 0x4C2642, 0x26001C, 0x581358,
    0xFF007F, 0xFF7FBF, 0xA50052, 0xA5527C, 0x7F003F, 0x7F3F5F, 0x4C0026, 0x4C2639, 0x260013, 0x58131C, 0xFF003F, 0xFF7F9F, 0xA50029, 0xA55267, 0x7F001F, 0x7F3F4F, 0x4C0013, 0x4C262F, 0x260009, 0x581317, 0x000000, 0x656565, 0x666666,
    0x999999, 0xCCCCCC, 0xFFFFFF,
];

/// Nearest fixed ACI colour by squared RGB distance; ties prefer the lowest index.
/// Black uses adaptive foreground 7. Other colours exclude 7 so white stays white.
/// R12 indexed colour is approximate; the receiving CAD palette controls appearance.
pub fn aci(rgb: [u8; 3]) -> u8 {
    if rgb == [0, 0, 0] {
        return 7;
    }
    let mut best_index = 1;
    let mut best_distance = u32::MAX;
    for (index, &colour) in ACI_RGB.iter().enumerate().skip(1) {
        if index == 7 {
            continue;
        }
        let channels = [(colour >> 16) & 255, (colour >> 8) & 255, colour & 255];
        let distance = rgb.iter().zip(channels).map(|(&a, b)| u32::from(a).abs_diff(b).pow(2)).sum();
        if distance < best_distance {
            best_distance = distance;
            best_index = index as u8;
        }
    }
    best_index
}

fn layer_of(p: &Prim) -> (String, u8) {
    match p {
        Prim::Line { layer, weight_mm, rgb, .. } => (format!("{layer}_W{:03}", (weight_mm * 100.0).round() as u32), aci(*rgb)),
        Prim::Text { layer, .. } => (layer.clone(), 7),
    }
}

fn clean(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

fn n(v: f64) -> String {
    format!("{v:.4}")
}

pub fn write_dxf(s: &RenderedSheet) -> String {
    let mut layers: BTreeSet<(String, u8)> = BTreeSet::new();
    for p in &s.prims {
        layers.insert(layer_of(p));
    }
    let mut o = String::new();
    let w = &mut o;
    let _ = write!(w, "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n9\n$INSUNITS\n70\n4\n");
    // The sheet footprint, independent of title-block margins and content bounds.
    let _ = write!(w, "9\n$EXTMIN\n10\n0.0\n20\n0.0\n30\n0.0\n9\n$EXTMAX\n10\n{}\n20\n{}\n30\n0.0\n0\nENDSEC\n", n(s.size.width_mm), n(s.size.height_mm));
    let _ = write!(w, "0\nSECTION\n2\nTABLES\n");
    let _ = write!(w, "0\nTABLE\n2\nLTYPE\n70\n1\n0\nLTYPE\n2\nCONTINUOUS\n70\n0\n3\nSolid line\n72\n65\n73\n0\n40\n0.0\n0\nENDTAB\n");
    let _ = write!(w, "0\nTABLE\n2\nLAYER\n70\n{}\n", layers.len());
    for (name, col) in &layers {
        let _ = write!(w, "0\nLAYER\n2\n{name}\n70\n0\n62\n{col}\n6\nCONTINUOUS\n");
    }
    let _ = write!(w, "0\nENDTAB\n");
    let _ = write!(w, "0\nTABLE\n2\nSTYLE\n70\n1\n0\nSTYLE\n2\nSTANDARD\n70\n0\n40\n0.0\n41\n1.0\n50\n0.0\n71\n0\n42\n2.5\n3\ntxt\n4\n\n0\nENDTAB\n");
    let _ = write!(w, "0\nENDSEC\n0\nSECTION\n2\nENTITIES\n");
    for p in &s.prims {
        let (layer, col) = layer_of(p);
        match p {
            Prim::Line { a, b, .. } => {
                let _ = write!(w, "0\nLINE\n8\n{layer}\n62\n{col}\n10\n{}\n20\n{}\n30\n0.0\n11\n{}\n21\n{}\n31\n0.0\n", n(a[0]), n(a[1]), n(b[0]), n(b[1]));
            }
            Prim::Text { at, height, align, text, .. } => {
                let _ = write!(w, "0\nTEXT\n8\n{layer}\n62\n{col}\n10\n{}\n20\n{}\n30\n0.0\n40\n{}\n1\n{}\n", n(at[0]), n(at[1]), n(*height), clean(text));
                match align {
                    Align::Left => {}
                    Align::Centre => {
                        let _ = write!(w, "72\n1\n11\n{}\n21\n{}\n31\n0.0\n", n(at[0]), n(at[1]));
                    }
                    Align::Right => {
                        let _ = write!(w, "72\n2\n11\n{}\n21\n{}\n31\n0.0\n", n(at[0]), n(at[1]));
                    }
                }
            }
        }
    }
    let _ = write!(w, "0\nENDSEC\n0\nEOF\n");
    o
}
