# Area 1 — IFC export honesty (B7, B8, B9, B10, B11, B14, B15, B19)

- **Task:** gap-p0-2026-10-25 / **area-1**
- **Verified at:** `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be` ("Pipe fittings as families … (#96)"), branch
  `docs/gap-p0-production-backlog-2026-10-25`, worktree `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog`.
- **Register:** `docs/gap-remediation-plan-2026-10-21.md` §2.2 (rows at `:100-112`) and §8.5 (`:409-447`).
- **Method:** read-only. Every `file:line` below was read at the pinned SHA. Nothing was built.
- **Headline:** all eight rows are **CLOSED** at `b6ef0ee`. The closing work landed in
  `b17a1ff` ("the IFC honesty bundle — grids, named systems, run systems, penetrations, project key,
  truss assemblies, compute fixes (B7-B11, B14, B15, B19)") and `fccee7b`; every register citation
  had therefore **moved**, so the verdicts are CLOSED-on-moved-citations (new lines given).
  The honest residual is **partial coverage, not an open gap**: four sub-claims of B10/B11/B19 are
  still missing their end-to-end/acceptance half, and they are proposed as the production items below.

---

## 1. Per-row verdicts

| Row | Register claim | Citation (at `dcf8382`) | State at `b6ef0ee` | Verdict | Real `file:line` |
|---|---|---|---|---|---|
| **B7** | Every duct/pipe exported with `System::Other`; `MepData.run_systems` never read. | `export.rs:205-207`; `mep.rs:189` | The run takes its recorded system: `let system = doc.mep().run_system(el.id).unwrap_or(System::Other);` — the old `System::Other` literal is gone. `run_systems` is a real field with a doc comment ("the system each duct or pipe belongs to …"). | **CLOSED** | `apps/desktop/src/export.rs:272`; `crates/document/src/mep.rs:204` |
| **B8** | Rooms, grids and the site dropped from IFC export; crate can write `IfcSpace`. | `export.rs:198,291-293`; `ifc/lib.rs:964,1497-1556` | Rooms collected (`RoomItem { id, number, room: Room { … } }`) and fed to the scene; grids collected into `Vec<GridItem>`; georef read from the app records instead of `None`. Crate writes `IFCSPACE` + `Qto_SpaceBaseQuantities` + `IFCRELAGGREGATES` to the storey, and `IFCGRID`/`IFCGRIDAXIS`. Site/georef: `georef: GeorefInput::from_records(doc.app_records()).georef(doc.site())`. | **CLOSED** (partial test coverage, see §2 note) | `apps/desktop/src/export.rs:395-406, 316-319, 414`; `crates/ifc/src/lib.rs:1721-1750, 1686-1717` |
| **B9** | User systems not written as `IfcSystem`; one `IfcDistributionSystem` per `System` enum code. | `ifc/lib.rs:1823-1838`; `export.rs` | `system_defs` is built per `MepSystemDef` from the model and each is written as `IFCDISTRIBUTIONSYSTEM` under the user's own name with `IFCRELASSIGNSTOGROUP` to its members (runs reachable from its equipment + the members themselves). `report.named_systems += 1`. | **CLOSED** | `apps/desktop/src/export.rs:313-314`; `crates/ifc/src/lib.rs:1928-1936, 2114, 2135` |
| **B10** | Wall/floor penetrations never created; `openbim_place::penetrations` exists. | `export.rs:290-291`; `crates/place/src/penetrations.rs` | The engine is called on the export path: `penetrations::find(&chains, &runs, gap)` and `floors::find_floor(&flat, &[], &runs, gap)`; wall openings are cut into the wall chains (with a `segment_volume_exact` fit check that drops one that would break the wall); both lists are passed to the scene, and the crate writes `IFCOPENINGELEMENT` + `IFCRELVOIDSELEMENT` (+ `OpenBIM_Penetration` pset for floors). | **CLOSED** (two residuals, see §2) | `apps/desktop/src/export.rs:288-305, 412-413`; `crates/ifc/src/lib.rs:1216, 1525-1545`; `crates/place/src/penetrations.rs:279` |
| **B11** | GlobalIds not stable; project key fixed at 1; project name "Project1". | `ifc/lib.rs:645`; `document/lib.rs next_id`; `model_edit.rs:305-306` | Storeys keyed by name (`o.gid(name_key(&l.name), 0xF010)`), named systems by name (`0xF000`/`0xF001`), and every remaining placeholder id is resolved by `Step::derive_ids` from what the entity *is* (canonical text + referrers), not from write order. The key is `Document::ensure_project_key()` (clock+pid+element count, persisted as `Y|key=…` and re-read on open); the project name is the open file's stem. | **CLOSED** (residual: `next_id` still reuses a freed maximum, see §2) | `crates/ifc/src/lib.rs:1054, 1930-1932, 757-760, 1102`; `crates/ifc/src/step.rs:129-143`; `crates/document/src/lib.rs:623-632`; `apps/desktop/src/model_edit.rs:404-405`; `crates/document/src/native.rs:387-388` |
| **B14** | Marking an End Point drops the equipment join. | `compute_ui.rs:132` | `end_point_click` only drops the join for a non-Source mark: `if self.end_draft.mark != Mark::Source { data.equip_links.retain(|q| q.port != port); }`; kind 4 still re-adds it. Regression asserted in `elec_ui` ("the join survives a Source mark"). | **CLOSED** | `apps/desktop/src/compute_ui.rs:201-202`; test `apps/desktop/src/elec_ui.rs:740-743` |
| **B15** | The Air preset does not change the velocity limit. | `compute_ui.rs:69-70` | `use_fluid(true)` sets `(max_velocity, max_friction) = (8.0, 1.0)` alongside the air properties; water restores `(1.5, 0.0)`; the dialog shows both limits as editable fields and the preset hover text names them. | **CLOSED** | `apps/desktop/src/compute_ui.rs:107-114, 317, 333`; test `apps/desktop/src/compute_ui.rs:517-525` |
| **B19** | Trusses and beam systems lose their parent on export. | `export.rs:198`; `ifc/lib.rs` | `assemblies` is built from every `Truss`/`BeamSystem` element over `doc.system_beams(e.id)`; the crate writes `IFCELEMENTASSEMBLY` (`.TRUSS.` / `.BEAM_GRID.`) and removes the members from `IFCRELCONTAINEDINSPATIALSTRUCTURE`, then `IFCRELAGGREGATES` from the assembly to its members. | **CLOSED** (residual: no desktop-level acceptance test, see §2) | `apps/desktop/src/export.rs:307-311`; `crates/ifc/src/lib.rs:1296-1316`; test `crates/ifc/tests/grids.rs:170-183` |

**Moved-citation index** (register §8.5 line → line at `b6ef0ee`):

| Register §8.5 | At `dcf8382` | At `b6ef0ee` |
|---|---|---|
| B7 `export.rs:205,207` / comment `:203` | 205/207/203 | `export.rs:272` / comment `:271` |
| B7 `mep.rs:189` | 189 | `mep.rs:204` |
| B8 `export.rs:198` | 198 | `export.rs:266` (the category skip arm) |
| B8 `export.rs:291` (`rooms: vec![]`) | 291 | `export.rs:395` (now a real collection loop) |
| B8 `export.rs:292` (`georef: None`) | 292 | `export.rs:414` |
| B8 `ifc/lib.rs:964,1497-1556` | 964 / 1497-1556 | `ifc/lib.rs:1078` / `1721-1750` |
| B9 `ifc/lib.rs:1831-1838` | 1831-1838 | `ifc/lib.rs:2122-2125` (per-code group, now only the *unclaimed* remainder) |
| B11 `ifc/lib.rs:645` | 645 (`gen_gid` counter) | `ifc/lib.rs:757-760` (placeholder) + `step.rs:129` (derivation) |
| B11 `document/lib.rs next_id` | 571 | `document/lib.rs:639` |
| B11 `model_edit.rs:305` | 305 | `model_edit.rs:404-405` |
| B14 `compute_ui.rs:132` | 132 | `compute_ui.rs:202` |
| B15 `compute_ui.rs:69-70` | 69-70 | `compute_ui.rs:110, 122` |
| B19 `export.rs:198` | 198 | `export.rs:307-311` |

---

## 2. Splice candidates (the residual work, not an open gap)

Nothing below re-opens a row; each item closes the gap between "the code does it" and "the code is
proved to do it end to end" (or finishes a half-built feed). Ordered cheapest-first.

### Item 1 — B8: grid lines leave as `IFCGRID` (acceptance half)
- **What:** add the grid assertion to the existing export test so the register's own acceptance
  ("an IFC from a model with rooms and grids contains `IFCSPACE` and `IFCGRID` with the right counts")
  is actually asserted on the desktop export path.
- **Files:** `apps/desktop/src/export.rs` (test module, next to
  `a_run_carries_its_system_and_cuts_its_wall_and_the_rooms_and_systems_reach_the_file`, `:703`).
- **Acceptance:** `Document::spatial_sample()` carries 6 grid lines (`crates/document/src/spatial.rs:659-664`);
  assert `out.text.matches("IFCGRID(").count() == 1` and `matches("IFCGRIDAXIS(").count() == 6`.
  (Today the only `IFCGRID` assertion in `apps/` is on the *import* path,
  `apps/desktop/src/import.rs:1586-1587`.)
- **Owner:** `bim-rust`. **Deps:** none.

### Item 2 — B19: truss/beam-system parent on the desktop export path (acceptance half)
- **What:** assert the assembly on a real model, not only on a hand-built `Scene`.
- **Files:** `apps/desktop/src/export.rs` (test module).
- **Acceptance:** place a truss (`Document::place_truss`) and a beam system (`place_beam_system`) on a
  level, export, assert one `IFCELEMENTASSEMBLY(` with `.TRUSS.` and one with `.BEAM_GRID.`, and that
  `IFCRELAGGREGATES` names the assembly and its members. The crate-level proof already exists
  (`crates/ifc/tests/grids.rs:170-183`); the desktop half does not.
- **Owner:** `bim-rust`. **Deps:** none.

### Item 3 — B10: the penetration register and the firestop records
- **What:** the register's remediation is "call the penetrations engine … and feed the result to the
  exporter" — the engine is called, but its *register* (`register_csv`, `crates/place/src/penetrations.rs:381`)
  has no caller under `apps/`, and `firestops:` is hard-coded empty
  (`apps/desktop/src/export.rs:412`). So the opening data reaches IFC as a pset
  (`crates/ifc/src/lib.rs:1543-1548`) but no schedule/CSV of openings exists, and the firestop columns
  of the register are always blank.
- **Files:** `crates/document/src/mep.rs` (carry the register/firestops as model data, next to
  `run_systems` `:204`), `crates/document/src/native.rs` (persist/restore, as for `equip_links`),
  `apps/desktop/src/export.rs` (pass instead of `Default::default()`), plus a new
  `apps/desktop/src/penetration_ui.rs` (register view, like the existing schedule window).
- **Acceptance:** a duct crossing a fire-rated wall exports with a `OpenBIM_Penetration` pset whose
  `InFirePartition` is true and whose `FireRating` is the wall's rating; the register view lists the
  opening with the run, the system and the firestop columns; the register round-trips through
  save/open.
- **Owner:** `bim-rust`. **Deps:** Item 1 (same export test module), the MEP schedule window pattern
  already in the app.

### Item 4 — B11: `next_id` reuses a freed element number
- **What:** the one sub-claim of B11 that is still literally true:
  `pub fn next_id(&self) -> ElementId { ElementId(self.elements.keys().next_back().map_or(1, |k| k.0 + 1)) }`
  (`crates/document/src/lib.rs:639`). Deleting the newest element and creating another reuses its
  number, so the *product* GlobalId of a new element collides with a deleted one's. The register's
  acceptance ("export twice with an unrelated edit in between; product, storey and system GUIDs are
  unchanged") is met for everything else — storeys are name-keyed (`crates/ifc/src/lib.rs:1054`),
  systems are name-keyed (`:1930-1932`), and placeholders are content-derived
  (`crates/ifc/src/step.rs:129`) — but this one is not.
- **Files:** `crates/document/src/lib.rs` (keep a high-water mark of minted ids, e.g. alongside
  `revision`), `crates/document/src/native.rs` (persist it).
- **Acceptance:** create the highest-numbered element, delete it, create another, and assert the new
  element's id is greater than the deleted one's (so its `gid()` differs); export before and after and
  assert the surviving elements' GlobalIds are unchanged.
- **Owner:** `bim-rust`. **Deps:** none (independent of Items 1-3).

### Item 5 — B10: a visible hole in the 3D view for a floor crossing (acceptance half)
- **What:** the register's acceptance is "an `IFCOPENINGELEMENT` and a visible hole in the 3D view".
  The IFC half is covered (`crates/ifc/src/lib.rs:1525-1545`); the 3D half is wired but untested —
  `openbim_view::slab_holes` turns `scene.floor_penetrations` into slab holes
  (`crates/view/src/lib.rs:572-587`), and `scene_for_view` feeds the same list
  (`apps/desktop/src/export.rs:303, 413`), but no test asserts a hole from a *service* penetration
  (the plan-level test `crates/view/tests/plan.rs:447-482` covers shafts and its own
  `floor_penetrations`).
- **Files:** `crates/view/tests/mesh.rs` (or `plan.rs`), `apps/desktop/src/export.rs` (a desktop-level
  assertion if a 3D batch test is wanted).
- **Acceptance:** route a duct through a floor slab, build the scene, assert the slab's `Shape::Region`
  has the penetration as a hole.
- **Owner:** `bim-rust`. **Deps:** none.

---

## 3. Notes / blockers

- **No row is open.** All eight are closed at `b6ef0ee`; the register (dated 2026-10-21, verified at
  `dcf8382`) is stale on all eight. The staleness is uniform: `b17a1ff` and `fccee7b` moved every
  citation, which is why this report's verdicts cite the *new* lines throughout.
- **Residue is test-coverage and two un-fed data paths**, not wrong behaviour: Item 1 (grids asserted
  only on import), Item 2 (assemblies asserted only on a hand-built `Scene`), Item 3 (register +
  firestops never fed), Item 4 (`next_id` reuse — the one still-true sub-claim), Item 5 (3D hole
  untested).
- **`firestops` is a genuine dead input**, not a test gap: `Scene::firestops`
  (`crates/ifc/src/lib.rs:430`) is only ever `Default::default()` from the app
  (`apps/desktop/src/export.rs:412`) and is consumed at `crates/ifc/src/lib.rs:1229`.
- **Out of scope, not expanded:** Item 3's schedule window touches the app's schedule/sheet shell
  (existing pattern, so in scope); no file outside `apps/desktop/src`, `crates/ifc/src`,
  `crates/document/src`, `crates/place/src`, `crates/family/src`, `crates/view/src` is needed.
- **Blockers:** none. Every citation was found; nothing was guessed, so no row is `BLOCKED`.
- **Not verified:** nothing was built or run (no `cargo`), so "CLOSED" means "the code at the cited
  lines implements the claim", supported by the tests quoted above that exist in the tree at this SHA.
