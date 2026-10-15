# OpenBIM Icons v0.2 — generator

The project icon manifest contains **87 named commands**. The v0.2 generator outputs **174 SVG resources** (light + dark), a Qt `icons.qrc`, and a browser preview `gallery.html`.

The new `isometric.py` contains editable original 48×48 isometric artwork for Architecture, Structure, HVAC, Plumbing and Electrical, with an explicit **2D blue Model Line** exception. `generate.py` retains simpler 24×24 drawings for general editing and coordination operations where 3D pictures are less informative.

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
