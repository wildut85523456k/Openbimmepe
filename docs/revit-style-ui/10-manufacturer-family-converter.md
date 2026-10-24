# Manufacturer family converter: design note (not built yet)

Aim: use what manufacturers already publish, and keep the parametric part simple. Parametric is priority 1, exact shape priority 2.

## What the app already has (checked in the code)
- `crates/family`: parametric families (typed values, expressions, solids from boxes/forms/CSG, connectors with domain, system, flow direction, shape, flow and pressure drop, nested families, openings, arrays, checks), a `family code` and the Uniclass code, 16 library families.
- `catalogue.rs`: a tab-separated type table (family, columns, one row per type), loaded and checked whole before anything is created. `Load Family` in the ribbon reads it (`model_edit.rs`). This is Revit's type catalog idea.
- `type_editor.rs`: Edit Type, but for wall layers only.
- `crates/ifc`: STEP reader/writer; families export as IFC types with the code and classification.
- Missing: a family editor window (Tier D), a fixed mesh/B-rep solid, an IFC type reader for manufacturer files, STEP geometry import.

## Approach (as Revit does: one family, many types, a table)
A manufacturer product becomes a **type of one of our parametric families**, not a new family per file.
1. **Type catalog .txt import (Revit format)**: header `,width##length##millimeters,...`, one row per type. Map columns to family inputs by name, with unit suffix converted. Already close to `catalogue.rs`; add the Revit header syntax.
2. **IFC type reader**: for each IFCTYPEPRODUCT (e.g. IFCAIRTERMINALTYPE, IFCFIRESUPPRESSIONTERMINALTYPE, IFCDOORTYPE) read name, psets (Pset_*TypeCommon, dimensions, flow, pressure drop), IFCDISTRIBUTIONPORT list (position, direction, flow direction, system type, size) and the bounding box of the geometry. Pick the family by predefined type or Uniclass code, then fill: bounding box -> width/length/height, ports -> connectors (position, direction, size), psets -> flow and pressure. Unknown items are listed in a report, never guessed.
3. **Fixed shape solid** (priority 2): a new solid kind in the family holding a triangle mesh from the IFC or STEP, scaled by the family's dimensions when they are tied to the bounding box. Connectors stay parametric, so routing works with any shape.
4. **STEP import**: mesh only, then a dialog to mark connectors (click a face, set domain/system/size) saved with the family.
5. **RFA**: no safe open reader; the user exports IFC from Revit (or we consider Autodesk Design Automation later).

## Simplifications
- No formulas are read from manufacturers' files; formulas come only from our own families.
- One family per equipment kind, many types per family.
- Ports and sizes are the only things routing needs; everything else is optional.

## Needed from the user
2-3 IFC files of the same product from one maker, a Revit type-catalog .txt, and a STEP of the same product, to test on real data.
