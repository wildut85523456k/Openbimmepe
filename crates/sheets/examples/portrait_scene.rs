//! Export ISO A0-A4 in both orientations for independent PDF/DXF dimension checks.
//! Usage: cargo run -p openbim-sheets --release --example portrait_scene -- OUTDIR

use openbim_sheets::dxf::write_dxf;
use openbim_sheets::model::{PrintContext, Sheet, SheetSet};
use openbim_sheets::paper::{Margins, PaperSize};
use openbim_sheets::pdf::write_pdf;
use openbim_sheets::render::{render_all, LineWeightTable, ViewContents};
use openbim_sheets::titleblock::TitleBlockDef;
use openbim_sheets::visibility::ViewLibrary;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("output folder"));
    std::fs::create_dir_all(&out)?;
    let mut set = SheetSet::default();
    for n in 0..=4 {
        let landscape = PaperSize::iso_a(n).unwrap();
        for portrait in [true, false] {
            let name = format!("A{n}-{}", if portrait { "portrait" } else { "landscape" });
            let (width, height) = if portrait { landscape.portrait() } else { (landscape.width_mm, landscape.height_mm) };
            let paper = PaperSize::custom(&name, width, height)?;
            set.add_titleblock(TitleBlockDef::generic(&name, paper, Margins::all(10.0))?)?;
            set.add_sheet(Sheet::new(&name, &name, &name))?;
        }
    }
    let weights = LineWeightTable::uniform(vec![0.25; 16])?;
    let sheets = render_all(&set, &ViewLibrary::new(), &ViewContents::new(), &weights, &PrintContext::default())?;
    std::fs::write(out.join("orientations.pdf"), write_pdf(&sheets, "PRT-001 orientations").bytes)?;
    for sheet in sheets {
        std::fs::write(out.join(format!("{}.dxf", sheet.number)), write_dxf(&sheet))?;
    }
    Ok(())
}
