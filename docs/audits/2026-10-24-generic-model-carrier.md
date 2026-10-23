# Generic model carrier (24 Oct 2026)

Revit brings what it has no class for into the model as Generic Models (an IfcBuildingElementProxy on the way out, and
the original class kept as a parameter). This app now does the same, as step 2 of that approach (step 1 was the import
log).

## What changed

- `Category::Generic` ("Generic Model"). A prism on a level like a slab: plan `outline`, `height_m` tall (`width_m` the
  same), top `elevation_m` above the level (`place_generic` takes the base and the extent). Its `texts` keep what the file
  said: `IfcClass`, `PredefinedType` and `<property set>.<property>` values (at most 64 entries, 64-byte names, 256-byte
  values; what does not fit is left out and said in the notes). Validity: outline is a simple ring, extent up to 200 m,
  no family, no material, no type. Saved in the native records (`cat=Generic`), meshed, moved and copied like a slab,
  drawn in plan and 3D, in the Generic Models schedule (family column = the class), in the Generic Models view
  category, and shown read-only in Properties (derived geometry and a "Source (IFC)" group).
- Export: each one is an `IfcBuildingElementProxy` (`ObjectType` = the class it was) with an `OpenBIM_Source` set holding
  the class, the predefined type and the properties, and the usual `OpenBIM_Element` set (so a later import knows it).
  All three schemas.
- Import: a product of a class that is not read and has a body that is an upright extruded profile becomes a generic model
  (`Imported::unmapped` from the reader). A proxy this app wrote comes back as what it said it was. Left alone: voids
  (`IFCOPENINGELEMENT`), ports, grids, annotations, virtual elements and spaces are not objects and are never kept.
  A product whose body is not an upright prism (lying down, a mesh, a boolean) is still left out and counted, with its
  line in the log.
- A file with the app's own records restores generic models from the records, as families are; the carrier only reads the
  products that the records do not account for.

## Not done

- Bodies that are not upright extrusions (most foreign equipment is a mesh or a mapped shape) are still left out.
- A generic model is not editable in Properties yet (no resize), only through the plan tools that move and copy.
- No class mapping table yet (IFC class -> app category); curved grids still come back as straight lines.
- A generic model is not changed by a re-import of an edited file (like families, it is kept from the records).

## Checks

New tests: four in `crates/document/tests/generic.rs` (shape, refusals, edit rules, save and read back), the generic
schedule row, and three in `apps/desktop/src/import.rs` (leaves as a proxy and comes back in every schema with and without
the app's data and twice adds nothing; a foreign class is kept with its class and property set and is not listed as left
out; a product lying down stays out and is counted). Mutation-checked: dropping the "left out" correction, the upright
test and the `OpenBIM_Source` set each fail a test.
