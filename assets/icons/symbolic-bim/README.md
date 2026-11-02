# Symbolic BIM icons - navy / neon

48 newly authored geometric symbols, provided as an asset proposal for OpenBIM. This adds a collection; it does not switch the running application's ribbon.

## Files

- [gallery.html](gallery.html): self-contained review at actual 16, 24 and 32 CSS pixels on white and charcoal. Download/open locally to view HTML; GitHub displays its source.
- [symbols.svg](symbols.svg): 96 reusable SVG symbols, IDs such as `door-light` and `door-dark`.
- [manifest.json](manifest.json): stable asset IDs and categories. These are asset IDs, not verified app command mappings.
- [export-icons.mjs](export-icons.mjs): generates 96 individual transparent SVG files without dependencies.

```sh
node assets/icons/symbolic-bim/export-icons.mjs
# Outputs assets/icons/symbolic-bim/svg/light/door.svg, svg/dark/door.svg, etc.
```

For browser use: `<svg viewBox="0 0 24 24"><use href="symbols.svg#door-light"/></svg>`. Desktop SVG loaders should use the exported individual files; sprite support is renderer-dependent. Rasterize at physical pixel dimensions for high-DPI PNG use.

## Visual rules

24-unit canvas; 1.75-unit strokes with round caps/joins. Maximum two flat glyph colours: navy #000080 and neon #1F51FF. No embedded backgrounds, shadows, gradients or opacity. Light mode uses navy body plus neon details. Dark mode uses neon for all paths, preserving geometry and negative spaces. No navbar or background colour is part of the icon.

Calculated WCAG relative-luminance ratios: navy/white 16.01:1; neon/white 5.71:1; navy/#171B24 1.08:1; neon/#171B24 3.02:1. Dark use is specific to #171B24 or darker surfaces with sufficient contrast, not a guarantee for every dark theme. Neon is marginal at this threshold; inspect thin antialiased edges at actual size. Do not dim these icons with opacity to represent disabled states; use a separate accessible state treatment. Selection should use an outline/shape as well as colour.

The 16px display is a review target, not a separately pixel-hinted master. Review real app DPI, disabled, hover and selection states before enabling. The round socket/switch are generic European-inspired tool pictograms, not standards-compliant circuit-diagram symbols. Distinct rounded pipe elbows and rectangular duct elbows are intentional.

## Provenance

Authored as SVG geometry from the user's two-blue symbolic collection brief, 2 November 2026. Lapin's docs/mock/icons.svg informed the 24-unit rounded technical-symbol visual approach; no SVG paths or artwork files were copied. The generated chat image was a concept only; these editable vectors are the implementation proposal and may differ in detail. Reference: https://github.com/Ciuntuc-BIM-Studio/lapin/blob/main/docs/mock/icons.svg (inspected at 8b023411b604cb173f0986b06d938c26fcb7b485).

Follow the repository's asset licensing policy in ../LICENSE.md; no new licence is assigned here.

