# Supplemental ribbon icons

Simple 3D symbols for commands absent from the user-supplied BIM catalog, prepared
at 128 px for display at 34 px. One image was generated per asset with the built-in
image generation tool. `prompts.json` records the final prompts and image hashes.
The committed PNGs define the shipped appearance; rebuilding the app does not
need image generation, external image paths or Pillow.

Restore the resource copies with `python3 assets/icons/prepare_ribbon.py`.

Panel and command references are documented in
`apps/desktop-qt/RIBBON-REFERENCE.md`. These are original symbols in the OpenBIM
catalog style, not copied Autodesk binary resources.
