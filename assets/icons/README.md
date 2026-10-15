# Ribbon and toolbar icons

Used by the egui desktop app (`apps/desktop`). They are embedded in the executable through `apps/desktop/src/icons.rs`. Commands, and the icons that go with them, are looked up by stable command ID, never by their English label.

## `ribbon/` — discipline icons (PNG, 64 × 64, transparent)

100 icons, cut from the reference sheet (except `ifc_import` and `ifc_export`, redrawn as original icons) supplied by the project owner on 15 Oct 2026 (`ribbon/source-sheet.png`). They are grouped in folders by discipline:

| Folder | Icons |
|---|---|
| `architecture/` | 17 |
| `structure/` | 17 |
| `hvac/` | 16 |
| `plumbing/` | 17 |
| `electrical/` | 17 |
| `coordination/` | 16 |

Each tile's label was removed and its background made transparent. The tile backgrounds of solid objects were filled so that white parts stay opaque.

Eight line drawings also have a `*.dark.png` variant with light lines for the dark theme: grid, level, section, elevation, detail, dimension, tag and view.

The sheet is about 60–100 px per icon. The ribbon shows the icons at 32 px, which is sharp up to 2× screens. Larger sizes need a higher-resolution source.

ID notes, against the labels on the sheet:
- The second "Railing" is `railing_alt`.
- "Bnchor" is `anchor_cast_in`.
- Structural "Column" is `structural_column`.
- "Duct (Rectangular)" is `duct`.

## `common/` — common commands (SVG, 3D)

There are 20 common commands: select, save, open, new, undo, redo, search, settings, zoom in, zoom out, pan, orbit, copy, move, rotate, mirror, align, trim/extend, delete and measure.

They are drawn as 3D SVGs in the same style as the sheet: isometric, brushed metal, with blue as the accent colour. They are a **proposal for review** (15 Oct 2026). The SVG files are the source; egui rasterises them in Rust at the size needed.

## Before shipping

- **IFC logo.** Resolved 15 Oct 2026: the Import IFC and Export IFC icons on the sheet resembled the buildingSMART logo (a trademark), so they were replaced with original drawings: a document page with a blue model cube and a green arrow out (export) or in (import).
- **Socket and switch.** These icons are European. The first calculation profile is British practice (BS 1363), so a neutral or profile-specific icon may be needed.
- **Asset licence.** Choose a licence for these assets before external distribution.
