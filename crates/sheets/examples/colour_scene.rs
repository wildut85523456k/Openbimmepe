//! Export RGB rows from an input CSV for independent ACI palette checks.
//! Usage: cargo run -p openbim-sheets --release --example colour_scene -- INPUT.csv OUTDIR

use openbim_sheets::dxf::write_dxf;
use openbim_sheets::paper::PaperSize;
use openbim_sheets::pdf::write_pdf;
use openbim_sheets::render::{Prim, RenderedSheet};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("input CSV");
    let out = std::path::PathBuf::from(args.next().expect("output folder"));
    std::fs::create_dir_all(&out)?;
    let mut sheet = RenderedSheet { number: "COLOURS".into(), name: "ACI regression".into(), size: PaperSize::custom("Custom", 1000.0, 1000.0)?, prims: Vec::new(), warnings: Vec::new() };
    for (i, row) in std::fs::read_to_string(input)?.lines().enumerate() {
        let values: Vec<u8> = row.split(',').map(str::parse).collect::<Result<_, _>>()?;
        let rgb: [u8; 3] = values.try_into().map_err(|_| "expected three RGB channels")?;
        let x = (i / 100) as f64 * 20.0;
        let y = (i % 100) as f64 * 5.0;
        sheet.prims.push(Prim::Line { a: [x, y], b: [x + 10.0, y], weight_mm: 0.25, rgb, layer: format!("RGB_{}_{}_{}", rgb[0], rgb[1], rgb[2]) });
    }
    std::fs::write(out.join("colours.dxf"), write_dxf(&sheet))?;
    std::fs::write(out.join("colours.pdf"), write_pdf(std::slice::from_ref(&sheet), "ACI regression").bytes)?;
    Ok(())
}
