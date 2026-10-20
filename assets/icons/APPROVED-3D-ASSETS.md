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
installation is needed. The Qt executable embeds the PNGs, so it does not depend
on the working directory or an external artwork folder.

To reproduce source crops (Pillow is needed only for this authoring step):

```sh
python3 assets/icons/extract_catalog.py
python3 assets/icons/prepare_common.py
```

`catalog-provenance.json` records crop coordinates and hashes. Generated common
and supplemental sources, prompts, and preparation instructions are recorded in
`generated-common/`. Build-time packaging uses only Python's standard library:

```sh
python3 assets/icons/generate.py --output build/icon-preview --require-approved
cmake -S apps/desktop-qt -B build/desktop-qt \
  -DCMAKE_PREFIX_PATH="$(brew --prefix qt)" -DOPENBIM_REQUIRE_APPROVED_ICONS=ON
cmake --build build/desktop-qt --parallel 4
ctest --test-dir build/desktop-qt --output-on-failure
```

The default build rejects missing PNGs. `coverage.json` lists every packaged
command; the embedded-resource test decodes every PNG with Qt. Ribbon commands
store explicit asset IDs, separate from their display labels. The schematic SVG
renderer is no longer used by the application.

## Review

```sh
python3 -m unittest discover -s assets/icons -p 'test_*.py'
build/desktop-qt/openbim-desktop-qt.app/Contents/MacOS/openbim-desktop-qt --verify-icons
```

For a reproducible screenshot, add `--screenshot /path/to/ribbon.png
--ribbon-tab Systems --theme Light` (or Dark). Preview runs use separate settings.
The HTML gallery includes the full library, including catalog equipment not yet
exposed as commands in this prototype.

On macOS, after building, `apps/desktop-qt/package-macos.sh build/desktop-qt`
creates `build/desktop-qt/package/openbim-desktop-qt.app`, bundles only the required
Qt frameworks and Cocoa/offscreen plugins, and signs the local bundle ad hoc. CMake
already copies the Rust bridge into the app's Frameworks directory and links it
through `@rpath`, rather than the build machine's Rust output directory.

## Additional project ribbon tabs

The eight secondary project tabs use `generated-ribbon/` for symbols absent from
the original catalog. Run `python3 assets/icons/prepare_ribbon.py` after the other
preparation steps to reproduce their resource files. Panel and command placement
is recorded in `apps/desktop-qt/RIBBON-REFERENCE.md` with Autodesk references.
