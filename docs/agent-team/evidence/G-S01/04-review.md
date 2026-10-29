# G-S01 — Review

## Review checklist
- [x] Door swing arc renders in plan view for `Category::Family` doors
- [x] Move handle: blue circle with white dot; drag adjusts `along_m` via `SetGeometry`
- [x] Width handles: gray circles; drag left/right adjusts `width_m` via `Resize`
- [x] Height handle: green circle (3D only); drag adjusts `height_m` via `Resize`
- [x] Borrow checker satisfied: all data extracted as `Copy` before `transact`
- [x] No debug `eprintln!` left in production code
- [x] Door swing direction read from `el.texts` ("swing" property)

## Pre-existing notes
- Doors/windows are `Category::Family` (not `Category::Door`/`Category::Window`).
  The eval branch used `Category::Door`; adapted to current architecture.
- The eval branch's `Geometry::Hosted` → current `Geometry::InWall`.
- `is_hosted_opening()` does NOT exist on `Category` in current core; used
  `matches!(el.geometry, Some(Geometry::InWall { .. }))` instead.

## Reviewer
Arbiter: independent review of ported handle code.
