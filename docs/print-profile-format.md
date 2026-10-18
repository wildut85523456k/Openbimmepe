# PRT-002: version 1 print-profile data

Status: generic loader/API implemented; company acceptance remains BLOCKED.
No team PC3 files, pen tables, layer mappings or company title block were found in the available
project file inventory. The fixture is explicitly synthetic and is not an approved company standard.

Use PrintProfile::from_json to load and validate the complete profile. Unknown fields, missing
required values, unsupported versions, duplicate paper names/scales, bad dimensions/margins,
invalid pen tables and malformed JSON are refused. to_json validates before serializing.
The input is bounded to 1 MiB, 256 papers and 128 scale tables.

Required JSON fields:

| Field | Meaning |
|---|---|
| schema_version | Integer 1 |
| name | Profile display name |
| papers | Named papers with width_mm, height_mm and margins_mm (left/top/right/bottom) |
| paper_pens_mm | Exactly 16 pen widths in mm |
| model_pen_tables | Scale denominator and exactly 16 widths_mm per table |
| mode | colour, grayscale or black_lines |

Widths must be 0.01–10 mm. Model scale denominators must be nonzero and unique.
The existing engine uses the first scale at least as large as the requested denominator,
or the last table beyond the supplied range. Input table order is preserved on serialization;
the renderer sorts a copy. Paper dimensions retain their supplied orientation.
Printable width/height must each be at least 20 mm.

generic_titleblock creates an explicitly requested placeholder from the named paper and margins;
it does not import a company title-block design or replace any existing template.
render_all applies the loaded pen tables and mode while leaving existing title-block geometry
unchanged. Smaller papers may be valid profile data yet too small for the existing generic
title-block layout; that construction fails explicitly.

compare_media compares supplied, named source media with a configurable nonnegative tolerance.
Names match exactly; no fuzzy substitutions are made. It reports source/profile dimensions,
signed differences (profile minus source), and Match/Different/Missing in source order.
Swapped portrait/landscape dimensions and a 1188.8 vs 1189 mm difference are reported.
This is not a PC3 parser or a claim to have converted the actual ten team media.

## Validation (2026-10-18)

- 69 sheets-crate release tests pass; Clippy all targets with warnings denied passes.
- With the pinned dependency repair from PR #48: 567 full-workspace release tests pass
  from a fresh target directory; full-workspace all-target Clippy with warnings denied passes.
  Local cargo audit exits 0 after scanning 93 dependencies against 1338 advisories.
  Its registry-index refresh issued a local warning; remote CI remains a separate gate.
- Four new integration tests cover synthetic ten-paper load/serialization, comparison reports,
  real loaded-profile portrait PDF output, scale-dependent pens, mode/halftone application,
  and rejection of malformed/ambiguous inputs.
- Fixture: crates/sheets/tests/data/synthetic-print-profile.json.
- Company profile and team ten-media comparison remain unverified until the original source
  files and approved pen/margin values are supplied.
- Desktop profile selection remains PRT-006; company layer mapping remains PRT-005.
