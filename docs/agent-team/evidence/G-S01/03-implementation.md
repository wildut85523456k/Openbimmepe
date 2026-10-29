# G-S01 — Implementation

## Candidate SHA
`6fea25e5` (G-S01: door swing arc + move/width/height resize handles for hosted openings)

## Changes
Single file modified: `apps/desktop/src/model_view.rs` (+145 lines, -1 line)

### Door swing arc (lines 425-471)
Added 90° quarter-circle arc in plan view for each door
(`Category::Family` with `family.starts_with("Door")` and `Geometry::InWall`):
- Arc center at door's position along wall (at `along_m`)
- Arc radius = `el.width_m`
- Arc direction based on swing property: "left" → arcs to left side, "right" (default) → right side
- Drawn as 16 line segments + radius tick-mark to door center

### Viewport handles (lines 485-578)
Added move + width/height handles for selected hosted openings:
- **Move handle**: blue circle (12px) with white dot (6px); drag along wall adjusts `along_m` via `Command::SetGeometry { geometry: Geometry::InWall { host, along_m } }`
- **Width handles**: gray circles (8px) at left/right edges; drag adjusts `width_m` via `Command::Resize`
- **Height handle**: green circle (8px) with line from move handle (3D only); drag adjusts `height_m` via `Command::Resize`
- All values extracted as `Copy` before `transact()` to satisfy borrow checker

### Borrow checker adaptation
The eval branch's code pattern (`el` borrowed → `transact` called) would fail
borrow checking in the current codebase. Fixed by extracting all needed values
into a tuple before the `transact` calls:
```rust
let handle_data = app.document.get(id).and_then(|el| {
    ...
    Some((el.id, el.width_m, el.height_m, host, along_m, start, end, level, wall_z))
});
if let Some((el_id, w_m, h_m, host, along_m, start, end, _level, wall_z)) = handle_data {
    ... // transact calls use el_id, w_m, h_m — no active borrows
}
```

## Adaptation table
| Eval branch | Current codebase |
|---|---|
| `Category::Door` | `Category::Family` + `family.starts_with("Door")` |
| `Geometry::Hosted { host, offset, angle_deg }` | `Geometry::InWall { host, along_m }` |
| `app.door_flips: HashSet<ElementId>` | `el.texts` lookup for "swing" property |
| `Command::SetGeometry { geometry: Geometry::Hosted { offset } }` | `Command::SetGeometry { geometry: Geometry::InWall { along_m } }` |
| `Id::unique` (already fixed in G-M01) | No change needed |
