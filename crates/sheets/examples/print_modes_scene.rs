//! Six model strokes: red, green, blue, followed by the same colours halftoned.
//! Usage: cargo run -p openbim-sheets --release --example print_modes_scene -- OUTDIR
use openbim_sheets::dxf::write_dxf;
use openbim_sheets::model::{Params, PrintContext, Sheet, SheetSet, Viewport};
use openbim_sheets::paper::{Margins, PaperSize};
use openbim_sheets::pdf::write_pdf;
use openbim_sheets::render::{render_all_with_mode, LineWeightTable, ModelPrim, PrintMode, ViewContents};
use openbim_sheets::titleblock::TitleBlockDef;
use openbim_sheets::visibility::{Category, CategoryKind, Discipline, Override, ViewLibrary, ViewSettings};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("output folder"));
    std::fs::create_dir_all(&out)?;
    let mut set = SheetSet::default();
    let tb = TitleBlockDef::generic("A0 portrait", PaperSize::custom("A0", 841.0, 1189.0)?, Margins::all(10.0))?;
    let free = tb.free_rect;
    set.add_titleblock(tb)?;
    let mut sheet = Sheet::new("MODES", "Print modes", "A0 portrait");
    sheet.viewports.push(Viewport { view: "Colours".into(), centre: free.centre(), size: [free.width() - 10.0, free.height() - 20.0], scale: 100, model_centre: [0.0, 0.0], title: None });
    set.add_sheet(sheet)?;
    let mut lib = ViewLibrary::new();
    let mut view = ViewSettings::new("Colours", Discipline::Architectural);
    let mut lines = Vec::new();
    for i in 0..6 {
        let name = format!("C{i}");
        lib.add_category(Category { name: name.clone(), discipline: Discipline::Architectural, kind: CategoryKind::Model })?;
        view.categories.insert(name.clone(), Override { colour: Some([[255, 0, 0], [0, 255, 0], [0, 0, 255]][i % 3]), halftone: Some(i >= 3), ..Default::default() });
        lines.push(ModelPrim { a: [0.0, i as f64], b: [1.0, i as f64], category: name, params: Params::new(), link: None, weight: 5 });
    }
    lib.add_view(view)?;
    let contents = ViewContents::from([("Colours".into(), lines)]);
    let weights = LineWeightTable::uniform(vec![0.25; 16])?;
    for (name, mode) in [("colour", PrintMode::Colour), ("grayscale", PrintMode::Grayscale), ("black", PrintMode::BlackLines)] {
        let sheets = render_all_with_mode(&set, &lib, &contents, &weights, &PrintContext::default(), mode)?;
        std::fs::write(out.join(format!("{name}.pdf")), write_pdf(&sheets, "Print modes").bytes)?;
        std::fs::write(out.join(format!("{name}.dxf")), write_dxf(&sheets[0]))?;
    }
    Ok(())
}
