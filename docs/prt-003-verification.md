# PRT-003: colour, grayscale and black print modes — 2026-10-18

Status: renderer/export implementation verified; desktop controls and shared application command
integration are still PRT-006. PR #45 remains a documentation-only snapshot.
Implementation builds on sheets baseline 18520949edece94c8db30f764378ba818d6433bf
and the PRT-001 / PRT-004 fixes.

## Contract

- Existing render_sheet/render_all calls remain colour by default.
- Explicit PrintMode is passed per call through render_sheet_with_mode/render_all_with_mode.
- Colour preserves the existing view RGB and halftone behavior.
- Grayscale uses rounded BT.601 encoded-channel luma: (299R + 587G + 114B + 500)/1000.
- BlackLines converts normal strokes to black.
- Conversion precedes halftone. BlackLines halftones are [127,127,127], not solid black.
- Title blocks, revision graphics and text remain black.
- Rendered strokes feed both PDF and DXF; modes do not mutate model/view settings,
  geometry, units, pen widths, page sizes, visibility, text or warnings.
- Neutral RGB must map only to neutral ACI entries. This corrects dark gray tending
  toward a tinted entry under unrestricted nearest-RGB matching.

## Verification

Linux x86_64; rustc 1.99.1; original upstream lockfile; no dependency changes.

- The pre-implementation mode tests failed to compile because the API did not exist.
- cargo test --manifest-path crates/sheets/Cargo.toml --release: PASS, 65 tests.
- cargo clippy --manifest-path crates/sheets/Cargo.toml --release --all-targets -- -D warnings:
  PASS, exit 0.
- Independent pypdf/ezdxf checks: PASS for all three modes. Six coloured/halftoned
  model strokes are checked against explicit expected RGB values; DXF entity/layer colours,
  neutral grayscale, black sheet graphics, identical geometry and portrait page sizes are checked.
- Expanded ACI/save-reopen/PDF-preservation checker: PASS for 1249 RGB cases, including every
  grayscale byte value. PRT-001 independent checks pass for ten paper/orientation combinations.
- A dedicated printing-regressions GitHub workflow reruns the crate tests, Clippy and all
  three independent checks against the full repository checkout. Its remote result must
  be read from GitHub; local passes do not establish remote CI success.

## Remaining gates

Full-workspace local tests, independent code review, Revit reference comparison and desktop
interaction have not been performed. DXF remains lossy indexed R12, not true colour.
No print dialog, company profile/layer mapping, printer driver, RVT importer or purge is included.
Merge remains separate from the authorized push/PR publication.
