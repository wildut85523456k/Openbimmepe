#![forbid(unsafe_code)]
//! The open build of the desktop: no electrical solver is installed, so single-line schematics show
//! their placeholders and say the currents were not calculated. The full build is `full/desktop-full`.

fn main() -> eframe::Result {
    openbim_desktop::run()
}
