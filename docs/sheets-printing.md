# M1c: views, sheets, printing (spike s13, crate `openbim-sheets`)

Decisions (user, 2026-10-15)
1. Default title block follows the MEP-E layout (to be supplied by the user); standard ISO templates are added too. Until then `TitleBlockDef::generic` is a placeholder.
2. ISO A sizes by default, ANSI optional.
3. Milestone M1c "views, sheets, printing" with this spike.
4. Sheet sets can be generated from an Excel file; columns are matched to field names.

What exists
- Title blocks: template per paper size, `{Field}` placeholders, revision table. Sheet size comes from the title block (as in Revit).
- Sheets, viewports (view at 1:n), revisions (issued flag) and clouds, sheet set, project and sheet parameters.
- Discipline display: category -> discipline, per view a discipline and a Show/Halftone/Hide mode per discipline, per-category overrides, rule filters, view templates, linked-model display.
- Printing: vector PDF 1.4 (true-size MediaBox), DXF R12 (weights in layer names `{D}-{CATEGORY}_W{n}`).
  Custom paper dimensions preserve the supplied width and height, including portrait.
  ISO/ANSI presets remain landscape by default. DXF header extents describe the full sheet.
- Excel import (calamine): header names matched to intrinsic fields or title block fields; atomic; create-only or create-or-update; row numbers in errors.
- No standard values are built in: line weights, templates and margins are data.

Verification
- `cargo test -p openbim-sheets` (unit, integration, proptest on clipping).
- Portrait regression: `cargo run -p openbim-sheets --release --example portrait_scene -- OUTDIR`,
  then `python spikes/s13_sheets_check/check_portrait.py OUTDIR` (pypdf and ezdxf).
- `python -I spikes/s13_sheets_check/check_sheets.py`: qpdf, pdftotext, pypdf, pdfium raster, ezdxf, reportlab width tables, strokes re-derived without the Rust code.
- Mutation checks done by hand (halftone maths, clip bounds, weight index, layer naming, mode table, revision lookup, number validation, header matching): each caught by a test.

Known limits
- PDF text uses standard Helvetica (WinAnsi); other scripts (for example Khmer) need embedded fonts later.
- Pending input: MEP-E title block, ISO templates (ISO 5457 / 7200 / 13567 references not found in the mailbox/Drive).
