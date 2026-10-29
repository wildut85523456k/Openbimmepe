# G-S01 — Salvage inventory: eval/external-bim-repos-2026-10-26

## Branch overview

| Source | SHA range | Behind | Ahead |
|---|---|---|---|
| main → eval/external-bim-repos-2026-10-26 | `7043fa5`..`cabe6868` | 358 | 90 |

## Commit inventory

### Already in core `544ee0d` or revit-alike `update-core-deps` (no action needed)

| Commit | Subject | Core location | Verified |
|---|---|---|---|
| `cabe6868` | BIM-003/PERF-001 dependency closure | revit-alike `d8cd1c3` | ✅ Graph::dirty_closure ported |
| `172dd726` | COLLAB-002 ownership + sync state | core `crates/oplog` | ✅ |
| `deb0be7f` | COLLAB-001 central commits | core `crates/oplog` | ✅ |
| `a5bdd5bc` | DOC-001 sheet browser + DXF | core `crates/sheets`, `crates/document` | ✅ |
| `52f69c5e` | MEP-001 + STR-001 | core `crates/route`, `crates/arch` | ✅ |
| `808d77b3` | PERF-002 spatial index | core `crates/route/spatial.rs` | ✅ |
| `34562e9b` | ARCH-001 architectural workflows | core `crates/arch` | ✅ |
| `cae7fd32` | FAMILY-001 parametric family editor | core `crates/family` | ✅ |
| `8e696cf6` | M3/doc: SheetDef/Viewport/portrait | core `crates/sheets` | ✅ |
| `0273dc82` | M3/doc: PaperSize | core `crates/document` | ✅ |
| `6380cc53` | Dimension module | core `crates/document/src/dimension.rs` | ✅ |
| `177a3b29` | Purge_unused extension | core `crates/document` | ✅ |
| `b79d1ae5` | PRT-007 ScalePenTable → DXF | core `crates/document/src/drawing.rs` | ✅ |
| `b61578c8` | PRT-005 print mode | core `crates/document/src/drawing.rs` | ✅ |
| `bbf0d7c1` | PRT-004 rgb_to_aci | core `crates/document/src/drawing.rs` | ✅ |
| `59537245` | PRT-003 ScalePenTable | core `crates/document/src/drawing.rs` | ✅ |
| `c6fb379d` | PRT-002/004/006 Pen/PenTable | core `crates/document/src/drawing.rs` | ✅ |
| `ad411470` | PRT-002/004 document_to_dxf_with_pens | core `crates/document/src/drawing.rs` | ✅ |
| `ad827585` | DXF drawing-output comparison | core `crates/document/src/drawing.rs` | ✅ |
| `3f368ef8` | Purge_unused tests | core `crates/document/tests/` | ✅ |
| `e56764f5` | openbim-clash crate | core `crates/clash` | ✅ |
| `e357565f` | CADCraft fixtures | core `tests/` | ✅ |
| `390192c9` | IFC export gap analysis (docs) | core docs | ✅ |
| `f0678998` | IFC import site-masses/US survey foot | core `crates/ifc` | ✅ |

### New work not yet in core — salvage targets

| Commit | Subject | Files | Feature | In core? | Target |
|---|---|---|---|---|---|
| `95fc1ab9` | P1 export: LoadBearing/IfcBeam/IfcRoof | `crates/arch/src/...`, `crates/document/src/...` | IFC P1 export | ❌ | core `crates/arch`, `crates/document` |
| `471b1eab` | P2 export: footings/stairs/railings/masses | `crates/arch/src/...`, `crates/document/src/...` | IFC P2 export | ❌ | core |
| `9c41f8ee` | P3 export: project parameters | `crates/document/src/...` | IFC P3 export | ❌ | core `crates/document` |
| `534ef79a` | P1/P2/P3 entity types | `crates/ifc/src/...` | IFC entity expansion | ✅ core `import.rs` has IfcBeam/IfcRoof/IfcFooting/IfcStair/IfcRailing | core |
| `2db9e391` | Round-trip export→import test P1–P3 | `crates/ifc/tests/...` | IFC test | ❌ | core `crates/ifc` |
| `000d5db1` | read_pset_values method | `crates/ifc/src/...` | PSET round-trip | ✅ `import.rs:855` | core |
| `5f80292c` | TypedValue List wrapper fix | `crates/ifc/src/...` | PSET parsing fix | ❌ | core `crates/ifc` |
| `4837bb93` | Debug format for Category | `crates/ifc/src/...` | IFC import name | ❌ | core `crates/ifc` |
| `58fd0982` | IfcType dereference fix | `crates/ifc/tests/...` | Equipment count test | ❌ | core `crates/ifc` |
| `d75fdad2` | Assert 7 equipment survive round-trip | `crates/ifc/tests/...` | IFC test | ❌ | core `crates/ifc` |
| `9d5c0225` | PSET round-trip Pset_ProjectParameters | `crates/ifc/tests/...` | IFC test | ❌ | core `crates/ifc` |
| `fe25584d` | Network round-trip IfcDistributionSystem | `crates/ifc/tests/...` | IFC test | ❌ | core `crates/ifc` |
| `499d48c5` | Curved elbows MEP checks | `crates/ifc/src/...` | IFC export | ❌ | core `crates/ifc` |
| `485c13eb` | ifc-lite-core feasibility | `crates/ifc/...` | IFC import | ❌ | core `crates/ifc` |
| `e5173216` | Move ifc deps to production | `crates/ifc/...` | IFC build cfg | ❌ | core `crates/ifc` |
| `b5a4829c` | Door swing flip toggle | `apps/desktop/src/...` | Desktop UI | ❌ | revit-alike `apps/desktop` |
| `91e03ffa` | Viewport flip handle for swing | `apps/desktop/src/...` | Desktop UI | ❌ | revit-alike `apps/desktop` |
| `0b4e4fb0` | Height resize handle 3D | `apps/desktop/src/...` | Desktop UI | ❌ | revit-alike `apps/desktop` |
| `573ff648` | Width resize handles | `apps/desktop/src/...` | Desktop UI | ❌ | revit-alike `apps/desktop` |
| `8b8ced75` | Draggable move handle | `apps/desktop/src/...` | Desktop UI | ❌ | revit-alike `apps/desktop` |
| `d256c459` | Window/door placement by clicking | `apps/desktop/src/...` | Desktop UI | ❌ | revit-alike `apps/desktop` |
| `bc294309` | Wire Window/Door tools to ribbon | `apps/desktop/src/...` | Desktop UI | ❌ | revit-alike `apps/desktop` |
| `aaa88bef` | Plan swing arc | `apps/desktop/src/...` | Desktop UI | ❌ | revit-alike `apps/desktop` |
| `76bd2309` | Door swing flip (property panel) | `apps/desktop/src/...` | Desktop UI | ❌ | revit-alike `apps/desktop` |
| `1f8b32e8` | M1-09 hosted openings rotation | `crates/arch/src/...` | Arch authoring | ❌ | core `crates/arch` |
| `4fb28a77` | M1-10/11 schedule/mark data | `crates/document/src/...` | Scheduling | ❌ | core `crates/document` |
| `13eaca90` | M1-10 UI: ComboBox + mark input | `apps/desktop/src/...` | Desktop UI | ❌ | revit-alike `apps/desktop` |
| `04ac861b` | M1-10 tests: SetType/SetMark | `crates/document/tests/...` | Test | ❌ | core `crates/document` |
| `edea328a` | M1-07+08 earcut multi-openings | `crates/arch/src/...` | Arch triangulation | ❌ | core `crates/arch` |

### Cleanup-only (not needed)

| Commit | Subject | Reason |
|---|---|---|
| `23515651` | Remove unused workset stub | Cleanup |
| `4bf629a3` | Revert sibling subagent automation changes | Regression fix |
| `f25b2277` | Working tree changes for pre-push gate | Temporary |
| `97c7f7da` | Apply cargo fmt | Done in G-M11 |
| `8d474405` | Remove debug eprintln from pset test | Cleanup |
| `f8a0efa2` | Brace placement fix | Cleanup |
| `b5a4829c`* | Door swing flip | Feature (see above) |
| `e67ec6c4`, `e8039c87` | Docs alignment | Not code |
| `c1c763e3` | Update Cargo.lock | Not code |
| `1284ee93` | BIMCraft evaluation docs | Not code |

### G-A02 decision: obg-ds custom graph nodes (superseded)

| Commit | Subject | Decision |
|---|---|---|
| `3e2e6db2`, `427a29d9` | CodeBlock node backed by obg-ds | G-A02: core `openbim-automation` is authoritative → close |
| `24af5f00`–`531e11af` | Custom graph node work | G-A02: superseded → close |
