mod common;
use common::*;
use openbim_sheets::dxf::{aci, write_dxf};
use openbim_sheets::pdf::write_pdf;
use openbim_sheets::render::*;
use openbim_sheets::visibility::{Override, ViewLibrary};

fn coloured_views() -> ViewLibrary {
    let original = views();
    let mut lib = library();
    for name in ["GF Architectural", "GF Mechanical", "GF Coordination"] {
        let mut view = original.view(name).unwrap().clone();
        for (category, colour, ht) in [("Walls", [255, 0, 0], false), ("Doors", [0, 255, 0], false), ("Columns", [0, 0, 255], true)] {
            view.categories.insert(category.into(), Override { colour: Some(colour), halftone: Some(ht), ..Default::default() });
        }
        lib.add_view(view).unwrap();
    }
    lib
}

fn rgb(sheet: &RenderedSheet, wanted: &str) -> [u8; 3] {
    sheet
        .prims
        .iter()
        .find_map(|p| match p {
            Prim::Line { layer, rgb, .. } if layer == wanted => Some(*rgb),
            _ => None,
        })
        .unwrap()
}

#[test]
fn print_modes_preserve_colour_or_make_grey_and_black_with_halftone() {
    let set = office_set();
    let lib = coloured_views();
    let render = |mode| render_sheet_with_mode(&set, &lib, &contents(), &lw(), "C-001", &ctx(), mode).unwrap();
    let colour = render(PrintMode::Colour);
    let grey = render(PrintMode::Grayscale);
    let black = render(PrintMode::BlackLines);
    assert_eq!(rgb(&colour, "A-WALLS"), [255, 0, 0]);
    assert_eq!(rgb(&colour, "A-DOORS"), [0, 255, 0]);
    assert_eq!(rgb(&colour, "S-COLUMNS"), [127, 127, 255]);
    assert_eq!(rgb(&grey, "A-WALLS"), [76; 3]);
    assert_eq!(rgb(&grey, "A-DOORS"), [150; 3]);
    assert_eq!(rgb(&grey, "S-COLUMNS"), [142; 3], "blue grayscale 29, then halftone");
    assert_eq!(rgb(&black, "A-WALLS"), [0; 3]);
    assert_eq!(rgb(&black, "A-DOORS"), [0; 3]);
    assert_eq!(rgb(&black, "S-COLUMNS"), [127; 3]);
    assert!(black.prims.iter().any(|p| matches!(p, Prim::Line { layer, rgb, .. } if layer == "TITLEBLOCK" && *rgb == [0; 3])));
    assert!(grey.prims.iter().all(|p| !matches!(p, Prim::Line { rgb, .. } if rgb[0] != rgb[1] || rgb[1] != rgb[2])));
    assert_eq!(aci([29; 3]), 250, "dark grayscale must not become a tinted ACI colour");
}

#[test]
fn mode_selection_is_per_run_and_changes_only_stroke_colours() {
    let set = office_set();
    let lib = coloured_views();
    let contents = contents();
    let lw = lw();
    let ctx = ctx();
    let default = render_all(&set, &lib, &contents, &lw, &ctx).unwrap();
    let colour = render_all_with_mode(&set, &lib, &contents, &lw, &ctx, PrintMode::Colour).unwrap();
    assert_eq!(PrintMode::default(), PrintMode::Colour);
    assert_eq!(write_pdf(&default, "T").bytes, write_pdf(&colour, "T").bytes);
    let normalise = |s: &RenderedSheet| {
        s.prims
            .iter()
            .cloned()
            .map(|mut p| {
                if let Prim::Line { rgb, .. } = &mut p {
                    *rgb = [0; 3];
                }
                p
            })
            .collect::<Vec<_>>()
    };
    for mode in [PrintMode::Grayscale, PrintMode::BlackLines] {
        let sheets = render_all_with_mode(&set, &lib, &contents, &lw, &ctx, mode).unwrap();
        for (before, after) in default.iter().zip(&sheets) {
            assert_eq!(normalise(before), normalise(after));
            assert_eq!(before.size, after.size);
            assert_eq!(before.warnings, after.warnings);
        }
    }
    let again = render_all(&set, &lib, &contents, &lw, &ctx).unwrap();
    assert_eq!(write_pdf(&default, "T").bytes, write_pdf(&again, "T").bytes);
    assert_eq!(write_dxf(&default[0]), write_dxf(&again[0]));
    assert!(render_sheet_with_mode(&set, &lib, &contents, &lw, "missing", &ctx, PrintMode::BlackLines).is_err());
}
