# Icons for the egui desktop app

The ribbon and toolbars use the **approved 3D icon package** (`approved-3d/`, 15–16 Oct 2026), the same artwork as the Qt prototype. It comes from the user's `BIM_CAD Icon Library Catalog.png`, plus generated common and supplemental symbols; see `APPROVED-3D-ASSETS.md`, `LICENSE.md` and `approved-3d/catalog-provenance.json` for provenance.

## What the Rust app embeds

`apps/desktop/src/icons.rs` embeds `rust/*.png`. The desktop downsamples this existing artwork with an alpha-aware Lanczos filter at the current physical display resolution and caches textures by theme and size. Quick-access icons use 24 logical pixels; ribbon icons use 32. See `docs/icon-display-update.md` for review steps. The derived PNGs can be rebuilt at any time:

```sh
python3 assets/icons/make_rust_icons.py   # approved-3d/*.png -> rust/*.png (transparent background)
python3 assets/icons/make_ifc_icons.py    # redraws ifc_import / ifc_export
```

- **Transparent background.** The approved PNGs sit on opaque white, which shows as white tiles in the dark theme. `make_rust_icons.py` removes the background (flood fill from the border, soft edge). The approved files are never modified.
- **Dark variants.** Seven line drawings (grid, level, tag, dimension, section, elevation, detail) have a light-stroke `*.dark.png` for the dark theme.
- **Import / Export IFC are redrawn.** The catalog's IFC icons resemble the buildingSMART logo, which is a trademark, so they are not used. `make_ifc_icons.py` draws original icons in the package style: a brushed-metal page, a glossy blue model cube and a blue arrow (into the page = import, out of it = export). The Qt branch still carries the catalog versions.
- **Command IDs.** Commands and icons are looked up by stable ID, never by label. A few Rust IDs map to differently named package icons (`pipe_support` → `support`, `valve_gate` → `valve`, `revision_cloud` → `revision`, `anchor_cast_in` → `anchor`); see `ALIASES` in `icons.rs`. Unit tests fail if any ribbon, tab or quick-access ID has no icon, if an icon ID is duplicated, or if an embedded file is not a PNG.

## Known limits

- The source crops are about 75–130 px. The ribbon shows them at 32 px, which is sharp on 2× screens; larger sizes need higher-resolution artwork.
- Electrical switch and socket keep the supplied **EU** designs, while the first calculation profile is British practice (BS 1363). A profile-specific icon may be needed.
- There is no separate cast-in anchor drawing yet.
- Choose an asset licence before external distribution (`LICENSE.md`).

## Superseded files

`ribbon/` (the 100 icons cut from the first sheet) and `common/` (the proposed 3D SVGs) are no longer embedded or used. They are kept only until the owner confirms they can be deleted.
