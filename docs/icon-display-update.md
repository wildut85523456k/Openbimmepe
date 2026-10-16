# Icon display update — 16 October 2026

The existing reviewed PNG artwork is preserved. No new icons were generated,
no legacy SVG proposals were restored, and no discipline designs were replaced.

The desktop now prepares each icon at its physical display size with a Lanczos
filter rather than shrinking its full texture during GPU sampling. Transparent
pixels are premultiplied before filtering to avoid coloured fringes. Textures
are cached by icon, theme and physical size, including Windows DPI changes.
Quick-access icons increase from 20 to 24 logical pixels; ribbon icons stay 32.

No dependencies were added. This improves downsampling; it cannot restore detail
already absent from the original PNG. Review the existing artwork in the app
before considering higher-resolution replacements.

## App review

Run `cargo test --locked -p openbim-desktop`, then
`cargo run --locked -p openbim-desktop`. Compare New, Open, Save, Undo, Redo,
Measure and Settings, plus Modify and View tabs. Check Light and Dark at Windows
scales of 100%, 150% and 200%, including moving between monitors. Icon shapes,
colours, labels and command behaviour should remain familiar. Check that the
24 px quick-access icons do not crowd the header.

For Windows builds, retain the previously tested local Cargo.lock correction
to windows 0.62.0. This icon update does not change the dependency lockfile.

## Verification

Windows cargo check passed. All three desktop tests passed, including an
alpha-edge regression test. Clippy was attempted but blocked by the host's
CARGO_MANIFEST_DIR directory access restriction. Interactive visual comparison
at 100%, 150% and 200% remains for the app review.
