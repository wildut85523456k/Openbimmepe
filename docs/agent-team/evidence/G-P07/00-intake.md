# G-P07 MEP spaces/zones with design airflow and actual airflow from terminals

- **Gap**: MEP spaces/zones with design airflow and actual airflow from terminals
- **Class**: 3 (new feature on existing architecture)
- **Repos**: openbim-core `crates/document`
- **Acceptance (closing test)**: space schedule airflow sums match terminals

## INTAKE

### Current state
- `openbim-core::document` has no Space or Zone struct. There is a `Category::Room`
  (`Geometry::Room`: a point in a closed region bounded by wall/column faces) but it
  carries no airflow data.
- `schematic.rs` has `SchematicZone` (an annotated rectangle on a level used by the
  automation/schematic workflow) — *not* an MEP space. It is unrelated to MEP airflow.
- `mep.rs` has `EndDemand`, `MinFlowDemand`, flow sums in the schematic calculation, but
  nothing ties demand to a room/space terminal schedule.
- No `airflow`/`flow_rate` field exists on `Element`.

### Requirement (from gap register + Revit book §04–§06)
A Space (MEP space) must carry:
- a design airflow (supply / extract / transfer), m³/s
- a terminal inventory (diffuser / exhaust terminal elements hosted by / assigned to
  the space)

A **space schedule** must sum the actual airflow of all terminals in each space and
verify the sum matches the space's design airflow (within tolerance). This is the
closing test: `space_schedule_airflow_sums_match_terminals`.

## DESIGN (contract)

### Data model additions (openbim-core::document)
- Add `Category::Space` — an MEP space bounded by model elements; `Geometry::Room`
  (point-in-polygon at plan) reuses the existing room geometry, but a Space is an MEP
  concept with airflow fields.
- Add fields on `Element` (only meaningful for `Category::Space`):
  - `design_airflow_m3s: f64` — required supply airflow for the space, m³/s
  - `extract_airflow_m3s: f64` — extract airflow, m³/s (0 = balanced)
  - `terminals: Vec<ElementId>` — terminal diffuser/exhaust elements assigned to the space
- Add fields on `Element` (only meaningful for terminal elements, e.g. a
  `Category::DuctTerminal` or family-hosted terminal):
  - `assigned_space: Option<ElementId>` — the Space this terminal serves
  - `airflow_m3s: f64` — the terminal's actual/delivered airflow

### Command
- `Command::SetSpaceAirflow { id, design, extract }` — set a space's design/extract flow.
- `Command::AssignTerminal { space, terminal }` — attach a terminal to a space (idempotent).

### Computation
- `Document::space_schedule(&self, id) -> SpaceSchedule` — sums `airflow_m3s` of all
  terminals whose `assigned_space == id` and compares to `design_airflow_m3s`.

### Native serialization (backward-compatible)
- `da=` (design airflow), `xa=` (extract airflow), `te=` (terminal list), `sp=` (assigned space), `af=` (terminal airflow)

### Acceptance test
- `space_schedule_airflow_sums_match_terminals`: build a document with one Space and
  two terminals; assign both to the space; assert schedule sum == design airflow
  within 1e-9 and that a mismatch is detected.

## Status
- INTAKE done. DESIGN done. Awaiting owner approval before IMPLEMENT.
