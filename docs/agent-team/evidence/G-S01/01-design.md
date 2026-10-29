# G-S01 — Design: Door/window viewport handles

## Scope
Port eval-branch `M1-01` through `M1-04` handle features into the Wave 0-cleaned
`model_view.rs` (3954 lines, uses `Geometry::InWall`).

## Design decisions
1. **Data extraction**: All handle data extracted into local `Copy` values before
   `transact()` call, to satisfy borrow checker (element + wall borrowed from
   `app.document`, `transact` needs `&mut`).
2. **Door identification**: `Category::Family` + `family.starts_with("Door")`.
3. **Swing direction**: Read from `el.texts` ("swing" key), values "right"/"left".
4. **Handle rendering**: Painter-based overlay in plan view and 3D view.
5. **Handle interaction**: Each frame, check pointer position vs handle positions;
   if dragging and within 24px of a handle, dispatch `Command::SetGeometry` (move)
   or `Command::Resize` (width/height).

## Files modified
- `apps/desktop/src/model_view.rs`: door swing arc + move/width/height handles

## Test plan
- `cargo check -p openbim-desktop`: clean ✅
- `cargo test -p openbim-desktop --lib`: 272/272 pass ✅
- `the_two_storey_office` test: pass ✅
