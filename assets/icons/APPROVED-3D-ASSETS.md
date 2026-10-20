# Catalog artwork in the Qt ribbon

The source is the user's `BIM_CAD Icon Library Catalog.png`, supplied on 15 October
2026. The exact input is versioned as `catalog-source/catalog.png`. Catalog PNGs
are extracted from the image pixels, without redrawing equipment or inventing
replacement geometries. At the ribbon's 34 px size, these source crops are
sufficient for the current prototype; enlarging them does not add source detail.

- Model Line preserves the catalog's simple blue line.
- Brace Frame uses the labeled frame. The unlabeled duplicate is excluded.
- Architecture's concrete column and Structure's steel column have separate IDs.
- The generic legacy `brace` ID is an explicit alias of `brace_frame`.
- Electrical switches and sockets were replaced with British BS 1363 style (rectangular three-pin socket, rocker switch) to match the first calculation profile (British practice, decision of 15 Oct 2026).
- Common commands and four supplemental commands absent from the catalog are
  generated 3D symbols. They are recorded separately from catalog artwork.

## Reproduce the assets and build

All runtime PNGs are committed in `approved-3d`; no separate download or ZIP
installation is needed. The egui desktop app embeds the PNGs via `apps/desktop/src/icons.rs`.

To regenerate the transparent PNGs (Pillow needed):

```sh
python3 assets/icons/make_rust_icons.py    # approved-3d/*.png -> rust/*.png (transparent)
```

> **Historical:** The catalog extraction command below requires `manifest.json`,
> which is not present on this branch. It is retained for reference only and
> will fail with FileNotFoundError. Socket and switch are marked as
> `replacement: true` in `catalog-provenance.json` (British BS 1363 style,
> not catalog crops) and are preserved by the extractor's SKIP set.

```sh
# Historical — requires absent manifest.json, will fail:
python3 assets/icons/extract_catalog.py
```

> **Historical:** The Qt/gallery instructions are no longer applicable.
> The `manifest.json` and `apps/desktop-qt` project are not present on this branch.
> The egui app uses the approved PNGs directly; `make_rust_icons.py` derives the
> transparent PNGs in `rust/`.

## Review

```sh
cargo test -p openbim-desktop --release   # embedded icon validity/uniqueness tests
```

The egui desktop app renders the icons at 24/32/48 px on light/dark backgrounds.
`docs/icon-display-update.md` records the review steps.
