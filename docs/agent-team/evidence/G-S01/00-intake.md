# G-S01 — Intake: Stranded eval-branch salvage (door/window handles)

## Source
- Eval branch: `eval/external-bim-repos-2026-10-26` (90 commits ahead of `b6ef0ee`)
- Worktree: `wt-g-m02` off `origin/update-core-deps` @ `04a4e8fb`
- Gap register entry: G-S01 — door/window handle UI parity with Revit/Dynamo

## Objective
Port stranded eval-branch door/window handle features into the Wave 0-cleaned
`update-core-deps` branch. Items per salvage inventory:

### Port items (implemented in this cycle)
| Item | Eval branch feature | Status |
|------|-------------------|--------|
| M1-01 | Door swing arc in plan view | ✅ Ported + implemented |
| M1-02 | Move handle for hosted openings | ✅ Ported + implemented |
| M1-03 | Width resize handles | ✅ Ported + implemented |
| M1-04 | Height resize handle (3D) | ✅ Ported + implemented |
| M1-05 | Swing flip toggle | ⏳ Deferred (swing direction via property text) |

### Already in core (no port needed)
| Item | Evidence |
|------|----------|
| IFC P1/P2/P3 | `IfcBeam`/`IfcRoof`/`IfcFooting`/`IfcStair`/`IfcRailing` in `crates/ifc/src/lib.rs` |
| pset round-trip | `read_pset_values` at `import.rs:855` in core |

### Not found in core (port candidates)
| Feature | Location (eval branch) | Status |
|---------|----------------------|--------|
| purge_unused | `crates/document` | ⏳ Needs investigation |
| drawing comparison | `crates/document` | ⏳ Needs investigation |
| door swing flip | `model_view.rs:522` | ⏳ Deferred |
| resize handles | `model_view.rs:340-420` | ✅ Ported |
| M1-09 rotation | core arch crate | ⏳ Needs investigation |
| M1-10 schedule | core document crate | ⏳ Needs investigation |

## Architectural adaptation notes
The eval branch used `Category::Door`/`Category::Window` with `Geometry::Hosted`.
The current (Wave 0-cleaned) codebase uses `Category::Family` (family name starts
with "Door"/"Window") with `Geometry::InWall { host, along_m }`.

Adaptations made:
- `Category::Door` → `Category::Family` + `el.family.starts_with("Door")`
- `Geometry::Hosted { host, offset, .. }` → `Geometry::InWall { host, along_m }`
- `app.door_flips` (HashSet) → `el.texts` lookup for "swing" property
- `Command::SetGeometry { Geometry::Hosted }` → `Command::SetGeometry { Geometry::InWall }`
