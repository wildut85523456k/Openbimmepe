# Properties, Project Browser and contextual editing — 16 October 2026

## Behaviour

The Rust desktop now has compact Revit-style palette title bars, type previews,
parameter search, expandable groups and aligned name/value rows. Only the current
sample type is offered; Edit Type reports that type editing is not connected.
Duct dimension fields are editable. AHU and room selections show their own
read-only sample properties instead of reusing duct dimensions. With no element
selected, Properties displays the current sample view.

The Project Browser has an indented expandable hierarchy for plans, 3D views,
ceiling plans, elevations, sections, legends, schedules, sheets, families,
groups and model links. Empty branches explicitly report no items in the sample.
Search retains and opens the ancestors of matching leaves. A single click
selects a tree row; double-click opens an available view. The current view is
bold and accented, independently of tree-row selection.

The sidebar width and the horizontal divider between palettes are resizable.
Each palette scrolls independently and can be closed from its title bar.
The status-bar Properties and Browser checkboxes restore closed palettes.
Single-palette mode uses all sidebar height. Full docking/floating is not added.

Selecting the sample duct exposes width, height, middle elevation and slope in
the options bar below the ribbon, as well as Properties. Both share the same
draft values. Apply commits one atomic document transaction; Reset discards
pending changes; document Undo restores committed values. Other selections
cannot edit the duct through Apply. Tab and Ctrl+Z no longer dispatch document
commands when a text/value control has keyboard focus.

## Category-specific slope

The document now supports a finite signed slope ratio on Duct and Pipe elements;
Wall and Equipment reject SetSlope. New ducts and pipes default to zero slope.
Percent in the UI is ratio multiplied by 100: 2% = 20 mm rise per horizontal metre.
The sign is defined along the future start-to-end element direction. Slope changes
participate in validation, atomic rollback, document revisions and Undo.
The desktop currently has only an editable sample duct, not a placed pipe.

## Scope

The plan remains illustrative. Applying width, elevation or slope changes stored
parameters but does not redraw actual BIM geometry. Geometry-driven views,
length/endpoint editing, on-canvas temporary dimensions, grips and routing remain
for the rendering/model integration milestone. No unimplemented wall/grid
controls are presented as working. Changes are in memory, without file saving.
No new dependencies or Qt code are introduced. The local Windows Cargo.lock
correction is unchanged by this update.

## Review in the app

1. Resize the left sidebar and drag its horizontal divider.
2. Close either palette, restore it using the status checkbox, and test Light/Dark.
3. Select the duct, AHU, a room and a browser view; inspect the parameter groups.
4. Search for Width or Slope in Properties; search for M-101 in the Browser.
5. Single-click a browser row, then double-click it to open the view.
6. Select the duct. Set width to 600 mm and slope to 2% in the options bar.
   Check that Properties agrees, Apply, then leave the field and use Undo.
7. Confirm selecting the AHU or room does not expose editable duct dimensions.

Windows desktop and document tests include headless palette rendering for all
sample selections, both themes and each palette-visibility layout, plus editing,
Undo, search ancestry, category slope validation and atomic rollback checks.
Interactive visual review remains required before accepting the appearance.

Verification: all workspace targets passed cargo check on Windows. The desktop
suite passed 6 tests and the document suite passed 7. Clippy was attempted but
blocked by the host's CARGO_MANIFEST_DIR directory access restriction.
