# 3D view and elevations: plan and Revit-like behaviour

Status: plan. Elevations are implemented in `crates/view` (`elevation_view`, `standard_elevations`); the 3D view is not started.

## 1. Elevations (done, extend)

Implemented: South/East/North/West from the building bounds, hidden-line faces drawn far to near, openings as holes, family solids as boxes, slabs/columns in front, level datum lines with labels.

Revit-like behaviour still to build in the UI:
- Elevation markers in plan (a pointer with a view tag); double-click opens the view. Default four per project, named "South", "East", "North", "West".
- Per-view crop region, annotation crop, far clip offset (maps to `Section.depth`).
- Rename, duplicate, delete, view templates (range, style, visibility).
- Level lines draggable; elevation tags follow the project's naming rule.
- Known limits (reported as warnings): sloped tops drawn flat; arched openings omitted from beyond walls in section mode.

## 2. 3D view

### Data path
- Per-element meshes from `geom::Mesh {verts, tris}`: walls (with openings), slabs, columns, family solids/forms. Built in parallel with rayon, cached per element id and invalidated by the op log, so an edit re-meshes only the touched elements (same incremental rule as `Model::update`).
- Mesh output carries element id, category and material index for picking and visibility/graphics.
- No JSON on the data path: buffers go straight to the GPU.

### Viewport
- `wgpu` inside egui (`egui_wgpu` paint callback), all Rust.
- Depth buffer, per-element id buffer for picking, edge pass for the Hidden Line style.

### Revit-like interaction
- Default `{3D}` view (isometric, from the south-east).
- Orbit: Shift + middle-drag. Pan: middle-drag. Zoom: wheel, toward the cursor. Zoom to fit, zoom to selection.
- ViewCube (top-right): faces, edges, corners, drag-to-orbit, home button, compass ring.
- Visual styles: Wireframe, Hidden Line, Shaded, Consistent Colors, Realistic (later).
- Section box: six grips, follows the selection ("Section Box" in Properties), clips the meshes and the id buffer.
- Walk/fly (later), camera views saved with names.
- Selection: click, Tab to cycle, window/crossing select, filters by category; Properties palette shows the selected element and the view's parameters.
- Visibility/Graphics: per-category visibility, overrides, transparency; view templates shared with plan/section/elevation views.
- Same project browser tree as the 2D views (Views > 3D Views).

### Milestones
1. Mesh export of walls/slabs/columns/families with ids (crate `openbim-mesh` or module in `view`), tests for determinism across thread counts.
2. wgpu viewport in egui: orbit/pan/zoom, Shaded style.
3. ViewCube, Hidden Line, picking.
4. Section box, Visibility/Graphics, view templates.
5. Realistic style, walk/fly.

Relates to #13 and backlog GUI-004.

## Status of the 3D geometry (17 Oct 2026)

Revit's family editor offers extrusion, blend, revolve, sweep and swept blend (solid and void), and its pipe and duct fittings are families built from them, so the 3D view draws all of them.

Family model (crate `family`): forms are extrusions, revolves, sweeps, blends and swept blends; any form can be a void (`FormDef::void`), which cuts the solids of the family (boxes and other forms) and adds no volume. `Instance::body()` gives the meshes with the voids cut out (`family::meshing`, `family::csg`). Blends pair point k of the bottom profile with point k of the top one (both put counter-clockwise), or both are circles; no holes.

Meshed in the 3D view: walls (sloped tops, openings, arched openings), slabs (shaft and floor-opening holes), columns, family boxes, extrusions, revolves, sweeps, blends, swept blends and voids, service runs (round and rectangular), elbows as bends, transitions as lofts (round, rectangular, or one to the other), tees and taps as the straight legs that meet (as in Revit, a tee is straight pieces joined), and insulation: `Network::insulate(system, thickness, material)` puts a layer of that thickness round every run and fitting of a system, drawn as its own element (`Kind::Insulation`).

IFC: blends are written as an `IfcFacetedBrep` (IFC has no solid with a changing section), voids as `IfcBooleanResult` differences. Insulation is not written to IFC yet.

Not done: a void cutting the host wall (a door or window already cuts its opening), swept blends round a bend are meshed with the mitre but their volume is the straight estimate, insulation in IFC and in the clash and penetration checks, rooms, levels and grids, connectors and ports. A cut leaves T-junctions in the surface of a solid (volume and look are right). `ElementMesh::warnings` reports a form that could not be meshed or a void that was too complex to cut.
