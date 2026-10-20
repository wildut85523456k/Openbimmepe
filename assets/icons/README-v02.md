# OpenBIM Icons v0.2 — generator

The project icon manifest contains **87 named commands**. The v0.2 generator outputs **174 SVG resources** (light + dark), a Qt `icons.qrc`, and a browser preview `gallery.html`.

The `isometric.py` module is currently an empty stub (no isometric icons on this branch). `generate.py` retains simpler 24×24 drawings for all operations. The approved 3D PNGs in `approved-3d/` are the source of truth for the embedded icons; `make_rust_icons.py` derives the transparent PNGs in `rust/`.

Run:
```sh
python3 assets/icons/generate.py --output build/icon-preview
open build/icon-preview/gallery.html
```

Qt is built using the existing CMake project in `apps/desktop-qt`; CMake reconfigures the icon bundle when the manifest, generator, or isometric source changes:
```sh
cmake -S apps/desktop-qt -B build/desktop-qt -DCMAKE_PREFIX_PATH="$(brew --prefix qt)"
cmake --build build/desktop-qt --parallel 4
open build/desktop-qt/openbim-desktop-qt.app
```

Source artwork is newly authored and is **not** a copy of Autodesk or other proprietary product assets. Review production licensing before external distribution.
