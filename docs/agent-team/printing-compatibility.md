# Team plot settings vs. OpenBIM printing behaviour

Checked 2026-10-17 against `origin/m1c-sheets-printing` @ `18520949` (crate `openbim-sheets`). Printing does not exist on `main` (`9ea67f61`) or in the `feature/rust-gui` desktop app: the GUI only has an empty "Sheets (all)" browser folder, with no print command.

Evidence: source reading plus a local probe example (not committed) that rendered the team's 10 plot media, ISO pens and team colours through `render_all`, `write_pdf` and `write_dxf`. Output was checked independently with pypdf. `cargo test -p openbim-sheets --release` passes 61 tests.

## How the app prints

| Aspect | AutoCAD PC3 (team files) | OpenBIM app |
|---|---|---|
| Output | Windows printer driver (Adobe PDF), `plot_to_file=FALSE` | Writes files only: vector PDF 1.4 and DXF R12. No printer devices, no driver, no spooling |
| Paper size | Chosen per plot from device forms | Set by the **title block** (as in Revit). ISO A0-A4, ANSI A-E, or custom 50-5000 mm |
| Orientation | `dm_orientation` 1/2 | **Landscape only.** `PaperSize::custom` stores the longer side as the width, so portrait silently becomes landscape |
| Printable area | Device-dependent (e.g. A4 209.9 x 297 mm) | Full page at true size; borders are title-block margins (data) |
| Resolution | 1200 dpi | Not applicable: pure vector output |
| Line weights | Per AutoCAD layer, or CTB/STB | 16 pen numbers mapped to mm, with an optional table per view scale (Revit model). Title block uses paper pens |
| Colour | True colour per layer | PDF keeps RGB. DXF converts to the nearest of only 6 base AutoCAD colours plus grey |
| Black/grey/colour print mode | Plot style | None; always prints in colour |
| Layers on export | Team names (`CAM-ACC`, `SWW-PIP-KTV-PVC` ...) | Generated as `{discipline}-{CATEGORY}_W{pen}` (e.g. `P-PIPES_W035`). Team names are not used |

## Probe results

**Paper.** All 10 team profiles were read. A0 renders at 1189 x 841 mm and A1 at 841 x 594 mm, both exact in the PDF MediaBox. **All 5 portrait profiles (A0-A4) came out landscape, with no warning.** The team's A0 form is 1188.8 mm long; the app uses ISO 1189 mm.

**Line weights.** Pens set to the ISO series printed exactly: 0.18 mm = 0.51 pt, 0.25 = 0.709, 0.35 = 0.992, 0.5 = 1.417, 0.7 = 1.984. Scale-dependent tables work, e.g. pen 4 = 0.35 mm up to 1:100 and 0.245 mm at 1:200.

**Colour in DXF.** Team colour olive `122,122,0` (all `*-ACC` layers) became ACI 1, **red**. Purple `112,0,223` became blue (5) and `183,0,183` became magenta (6). PDF colours are correct.

## What this means for the team files

1. **The PC3 files cannot be used by the app.** They remain valid for AutoCAD only. The app equivalents are title-block paper sizes, margins and a pen table. So the corrected `Portrait-A0` and `Landscape-A1` matter in AutoCAD, but in the app the issue becomes "portrait not supported".
2. **Team lineweights move to the pen table.** The ISO 0.18 / 0.25 / 0.35 mm values work once they are assigned per category (supports, cables/accessories, pipes/ducts/equipment), not per AutoCAD layer name.
3. **Team colours are kept in PDF and lost in DXF.**
4. **Team layer names are lost on DXF export.**

## Changes needed in the app (proposed backlog items, not implemented)

| ID | Priority | Item | Acceptance |
|---|---|---|---|
| PRT-1 | P1 | Portrait sheets: the title block keeps its orientation; PDF MediaBox and DXF extents follow it | 841 x 1189 title block gives a tall PDF page; no silent swap |
| PRT-2 | P1 | Company print profile (data, imported once): paper list with names, pen table per scale, margins, colour mode | Team profile loads; the 10 AutoCAD media map to profile paper entries with a report of any differences |
| PRT-3 | P1 | Print modes: colour / grayscale / black lines (Revit Print Setup) | PDF strokes are black or grey when asked; halftone stays distinguishable |
| PRT-4 | P1 | DXF colour fix: nearest of the full 255-colour ACI palette, or export DXF 2000+ with true colour (group 420) and lineweight (group 370) | `122,122,0` no longer exports as red; round-trip check with ezdxf |
| PRT-5 | P2 | Export layer mapping: category to team layer name, colour and pen (from `layers.csv`), falling back to the current naming | Exported DXF uses `SWW-PIP-...` names when the mapping is present |
| PRT-6 | P2 | Print UI in the desktop app: Print/Export dialog (sheet set selection, profile, mode, combined or separate files) | Driven through the shared command API, with a UX test |
| PRT-7 | P3 | Optional OS printer output (Windows print spooler) | Only if the team needs direct plotting; PDF covers the PC3s' current use (Adobe PDF) |

The team's PC3s all target the Adobe PDF printer, so the app's direct PDF output already covers what they are used for, apart from portrait sheets.
