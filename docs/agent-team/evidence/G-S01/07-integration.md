# G-S01 — Integration

## Status: COMPLETE (M1-01 through M1-04)

### Implemented and verified
- Door swing arc rendering in plan view
- Move handle (blue) for adjusting `along_m` along wall
- Width handles (gray) for resizing via `Command::Resize`
- Height handle (green, 3D only) for vertical resizing

### Deferred to next cycle
- M1-05: Swing flip toggle — swing direction is stored as a text property ("swing")
  on the Family element; implementing a viewport click-to-flip handle requires
  adding a new `Command::SetParam` dispatch. This is a small addition but not
  critical for the visual review (V4 High).

### Next steps
- G-S02: IFC P1/P2/P3 round-trip export→import test (investigation of `2db9e391`)
- G-S03: IFC PSET round-trip test (investigation of `5f80292c`)
- G-S04: purge_unused port (not found in core)

## Commit
`6fea25e5` on `origin/update-core-deps` (not yet pushed — pending user approval)
