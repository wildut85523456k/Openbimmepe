#![forbid(unsafe_code)]
//! The desktop with the edesign-alike electrical solver: single-line schematics carry calculated
//! feeder currents and voltage drops (preliminary, `not_approved`).

fn main() -> eframe::Result {
    // installed before the window opens, so the first schematic already uses it
    let _ = openbim_document::electrical_calc::install(Box::new(openbim_electrical_adapter::EdesignSolver));
    openbim_desktop::run()
}
