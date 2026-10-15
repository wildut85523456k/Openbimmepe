//! DXF export of one sheet in millimetres, in the old R12 (AC1009) form every CAD program opens.
//! R12 has no line weights, so each line goes on a layer named after its content and pen width
//! (`M-DUCTS_W025` is 0.25 mm) for the receiving program to map to pens; colours are the nearest AutoCAD
//! colour index. Text is left, centre or right justified.

use std::collections::BTreeSet;
use std::fmt::Write;

use crate::render::{Prim, RenderedSheet};
use crate::titleblock::Align;

/// Nearest AutoCAD colour index: black is 7 (shown black on white paper), greys are 8 and 9.
pub fn aci(rgb: [u8; 3]) -> u8 {
    if rgb == [0, 0, 0] {
        return 7;
    }
    if rgb[0] == rgb[1] && rgb[1] == rgb[2] {
        return if rgb[0] <= 128 { 8 } else { 9 };
    }
    const BASE: [(u8, [i32; 3]); 6] = [(1, [255, 0, 0]), (2, [255, 255, 0]), (3, [0, 255, 0]), (4, [0, 255, 255]), (5, [0, 0, 255]), (6, [255, 0, 255])];
    BASE.iter()
        .min_by_key(|(_, c)| (0..3).map(|i| (c[i] - i32::from(rgb[i])).pow(2)).sum::<i32>())
        .map(|(n, _)| *n)
        .expect("not empty")
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
    let _ = write!(w, "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n");
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
