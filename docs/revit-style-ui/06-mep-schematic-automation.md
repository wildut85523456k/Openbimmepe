# 06 — MEP schematic automation: single-line generation from the design model

Status: **proposed 20 Oct 2026; user decisions recorded below; not implemented.** Companion to 02 (workflows), 04 (automation) and 05 (backlog rows SCH-001–006).

Implementation status of this proposal: nothing in this document is implemented or accepted. A first engine slice — the zone data model, the assignment rules and the extraction query with its JSON dump — exists on the separate local branch `feature/sch-001-schematic-zones` (commit `e37996c`); it is not part of this candidate, it is not reviewed here, and no SCH-001 acceptance is claimed for it. **[superseded 2026-10-21 — R5 register: the schematic engine is now in main — crates/schematic/ (SCH-002 layout engine, layout.rs), crates/document/src/schematic.rs (SchematicZone at :56 and the extraction query at :346) and crates/document/src/schematic_view.rs generate_schematic; "it is not part of this candidate" is stale]**

## Decisions (user, 20 Oct 2026)

1. **Zone authoring (approved).** Schematic groups are authored in the plan viewport: the designer drags a rectangle on a level and names it. The zone is a first-class document-model entity, not derived data. IFC space containment is a fallback labeler and an auto-suggestion source only.
2. **Electrical reuse (approved, "option A").** revit-alike pins `openbim_electrical_model` and `openbim_sld` from the edesign-alike repository by git SHA. Electrical calculations are produced by edesign-alike and imported; revit-alike does not reimplement them. A follow-up boundary study (SCH-005) defines the data push: panels, connectivity and loads out of the revit-alike model; panels imported and arranged in edesign-alike; calculated current and voltage drop back. Single development, no duplicate electrical engine.
3. **Spacing (both modes, designer selects).** Both spacing modes must be implemented under SCH-002: uniform per-element slot pitch and footprint-proportional spacing; the designer picks per generation and uniform is the default. Neither mode is implemented yet.
4. **Discipline scope (approved).** Electrical, ELV, access control, BMS, plumbing, HVAC chilled water, HVAC air distribution, sprinkler/fire fighting. The framework is multi-discipline from the start; annotations are per-discipline (matrix below).

## Scope

In scope: one command `MEP: Generate Schematic` that reads the design model and produces a single-line schematic view: zones, extraction, grid layout, wires from connectivity, per-discipline annotations, sheet export.

Out of scope: the calculation engines themselves (MEP-002 modules; edesign-alike electrical calculations), the OpenBIM Graph node wrapper (scheduled with AUTO-007, not in SCH-001–006), clash detection, protection-device selection.

## Pipeline

1. **Command.** Ribbon command through the typed command API (BIM-002/AUTO-001). Later an OpenBIM Graph node per AUTO-007; the node wraps the command and reimplements no calculation.
2. **Zones.** Plan rectangle tool; zones stored in the document model.
3. **Extraction query.** Panels/controllers for electrical-family disciplines; zone-summarized endpoints for hydraulic/air disciplines; connectivity from the model.
4. **Grid layout.** Pure function: elements + levels + connections + spacing mode → symbol slots and wire routes. No I/O, no rendering.
5. **Rendering + annotations.** Single-line symbols, wires, per-discipline value labels, sheet export.

## Zones: authoring, assignment and determinism

- Entity: `SchematicZone { id, level, rect, label }`, stored with the model's MEP data so that it is saved, undone and exported with the model (the operation log covers it with the rest of the model's data).
- **Durable creation order.** Zone ids are allocated from a reserved base (above element, fitting and electrical-node numbers) and increase by one with each new zone; the id is persisted, so "creation order" is the ascending zone id and survives save, reopen and undo.
- **Assignment predicate.** An end point belongs to a zone when **both** hold: the zone's level equals the level of the run the end point sits on, and the end point's plan position lies inside the rectangle. A point on the rectangle's edge counts as inside (inclusive boundary). A point with the same plan coordinates on another level never matches.
- **Overlaps.** Overlapping zones on one level are reported. An end point inside several zones is assigned to the first of them by ascending zone id (creation order) and the ambiguity is reported with every zone that contained it; the designer resolves the overlap.
- **Label precedence.** A zone's displayed label is: the designer's label, else the name of the room (`IfcSpace`) whose point lies inside the rectangle on that level, else `Zone N`, where N is the zone's number in creation order. The room candidates are every room of that level with a point inside the rectangle; the winner is the candidate with the lowest element id (the elements' durable creation order), and a candidate whose name is empty or whitespace-only never wins. Spaces are a fallback labeler and an auto-suggestion source; they never define a group.
- Electrical-family disciplines (electrical, ELV, access control, BMS) pull panels/boards/controllers as schematic nodes; hydraulic/air disciplines (plumbing, chilled water, air, sprinkler) summarize terminal endpoints per zone.

## Extraction

- Electrical, ELV, access control, BMS: panels / distribution boards / controllers pulled as nodes.
- Plumbing, chilled water, air distribution, sprinkler: terminal end points summarized per zone and system. A group's flow is the sum of its end points' flows. Demands that are not flows yet (power across a temperature difference, loading units) are counted and not summed until the calculation modules convert them (SCH-006).
- End points inside no zone are not dropped: they are grouped apart, per level and system, and reported.
- A terminal end point whose port belongs to a fitting has no plan position of its own yet: it is counted and reported as unplaced rather than assigned to a zone.
- Connectivity: from the model connection topology (`openbim_family::mep` `Connector`/`System`; IFC port connections on import) → the wire list of the single-line.
- **Ordering key, largest to smallest**, on canonical quantities only (display units never decide order): electrical family = downstream load, amperes first and kVA as the secondary key; hydraulic/air = group flow, normalized to m³/s (L/s and m³/h are display units).

## Grid layout

- **Rows = levels**, ordered by elevation ascending, then by name, then by id; each row is labeled with the level name.
- **Columns = the elements of that level, in a total order**, so that the same input always gives the same layout:
  1. the ordering key descending (largest first), on canonical quantities;
  2. then the connectivity barycenter ascending (defined below);
  3. then the stable identity ascending (the element id, or the electrical node's number);
  4. an element with no connection sorts after every connected element at the same ordering key, and among themselves by stable identity.
- **Barycenter, fully defined.** A connection is undirected and counts once per pair of elements, however many wires join them; only connections between elements of the same level row count. The initial order is the ordering key descending, then stable id ascending. One pass computes, for each element, the mean of the current ranks (zero-based positions in the order the previous pass produced) of its connected neighbors, and re-sorts by (ordering key descending, barycenter ascending, stable id ascending). Passes repeat until the order stops changing, capped at 8 passes; the order after the last pass is the result, so the rule always terminates and the same input always gives the same layout. Ranks, never pixel positions, are the source: positions depend on the spacing mode, ranks do not.
- Spacing mode, designer-selectable per generation: **uniform** slot pitch (default) or **footprint-proportional**. Both are requirements of SCH-002; neither is implemented yet.
- Pure function `(elements, levels, connections, mode) → (symbol slots, wire routes)`; golden-testable without a renderer.
- Fixture `fixtures/schematic-layout-v1.json` is specified by this document and added with SCH-002. Its expected slots and orders are derived independently from the fixture inputs (hand-computed from the rules above), never by calling the generator under test. Comparisons: positions in metres with an absolute tolerance of 1e-9 m; quantities with a relative tolerance of 1e-9; discrete results (orders, ids, level order) compared exactly. The 1e-9 rule applies to numeric values only, not to ordering.

## Annotations (per-discipline matrix)

Two mandatory annotation families, from the user's rule: **every electrical-family discipline annotates current and voltage drop; every hydraulic/air discipline annotates flow and pressure drop.** Non-electrical disciplines carry no power or voltage annotations. Additional labels are proposed additions and never replace the mandatory baseline.

| Discipline | Nodes pulled | Grouping | Mandatory annotations | Proposed additions (not required) |
|---|---|---|---|---|
| Electrical | panels / distribution boards | per panel | load current (A), voltage drop (V and %) | kW; which circuits are shown |
| ELV | ELV panels / PSUs | per panel | current (A), voltage drop (V and %) | which circuits are shown |
| Access control | controllers / PSUs | per controller | current (A), voltage drop (V and %) on lock/reader circuits | which circuits are shown |
| BMS | controllers | per controller | current (A), voltage drop (V and %) on its power feeds | I/O point counts and connectivity |
| Plumbing | — | endpoints per zone | flow (L/s), pressure drop (kPa) | — |
| HVAC chilled water | — | endpoints per zone | flow (L/s), pressure drop (kPa) | pumps drawn as nodes |
| HVAC air distribution | — | terminals per zone | airflow (m³/h or L/s), pressure drop (Pa) | duct static pressure at the fan (a point pressure, not a drop); AHU/fans drawn as nodes |
| Sprinkler / fire fighting | — | heads per zone | flow (L/s or L/min), pressure drop (kPa) | pressure at the most remote head (a point pressure, not a drop) |

- Every annotation value comes from an MEP-002 calculation module (hydraulic/airflow) or an imported edesign-alike result (electrical). Units and the adopted standard edition are recorded with each value (AUTO-007 rule: no calculation reimplemented in nodes).
- Branch lengths for voltage drop / pressure drop come from route run geometry (centerline lengths).
- **Methods and editions.** Where company rules are missing, the intended methods are: voltage drop per the IEC 60364-5-52 method; hydraulics per Darcy-Weisbach or Hazen-Williams (SI units), the choice and its parameters stated in the reference fixture; sprinkler hydraulics per EN 12845 or NFPA 13 as applicable. **The adopted editions and amendments are not yet recorded: edition selection is unresolved and gates the calculation acceptance of SCH-006.** No edition may be implied, and licensed normative texts are reviewed before any compliance claim. No compliance is claimed here.

## Electrical reuse and the edesign-alike boundary

- Option A (approved): pin `openbim_electrical_model` + `openbim_sld` from the private edesign-alike repository by git SHA, mirroring how edesign-alike pins `openbim-family` from revit-alike. The sld crate is presentation-only by design (`DiagramView` owns no topology; `validate_view` checks it against the model) — the generator fills a `DiagramView` from the canonical graph.
- The two repositories stay separate applications; the pin is a dependency, not a merge. Authorization to push to either repository does not authorize pushing the other.
- **Where the generator is consumed:** the `DiagramView` generator of SCH-004 is integrated by the electrical annotation path of SCH-006 (the imported, calculated electrical values are placed on the generated view); SCH-006 therefore depends on SCH-004. Until then, the electrical view of SCH-003 carries presentation values only.
- Boundary study (SCH-005, follow-up): exchange format and command/API mechanism for pushing model data (panels, connectivity, loads) from revit-alike to edesign-alike, importing and arranging panels there, and returning calculated data. Acceptance: a study doc with an agreed format. No second electrical development.

## Symbols and rendering

- Native symbol library drawn to IEC 60617 / ISO single-line conventions; no ABB, Autodesk or other third-party assets.
- Rendered through the existing `view`/`viewport` crates; export through DOC-001 and the sheets/printing work (`openbim-sheets`, landscape paper).
- The schematic is a view over the model: generation never mutates the design model; zone edits and regeneration are undoable like any edit.

## Phases and gates

Acceptance is split by producer: SCH-001–003 deliver the data, the layout and the presentation, with **fixture/reference labels** for values; SCH-004–006 deliver the pinned electrical reuse, the exchange study and the **live calculated or imported** values. Matrix conformance with real values is SCH-006's acceptance, not SCH-003's.

| Phase | Content | Acceptance |
|---|---|---|
| SCH-001 | Zone entities, plan assignment, extraction query, JSON dump of the draft (data only) | Zones persist with the model; assignment deterministic (same level, edges inclusive, first by zone id); overlaps and ambiguous end points reported; unzoned and unplaced end points reported; room-name fallback; draft JSON dump |
| SCH-002 | Grid layout engine, both spacing modes | Golden tests from the fixture: numeric values at 1e-9 (metres; relative for quantities), orders and ids exact, expected results independently derived; largest-to-smallest total order with the barycenter and stable-id tie-breaks; both spacing modes valid; fixtures cover multi-room label fallback, barycenter ties, disconnected and equal-key elements, and equivalent unit representations |
| SCH-003 | Single-line view, native IEC 60617-style symbols, presentation annotations, `MEP: Generate Schematic` command | Repeatable output; the mandatory annotation matrix is shown with fixture/reference labels (no live calculation claimed); undo/redo; sheet export |
| SCH-004 | Electrical reuse: pinned crates + `DiagramView` generator | Builds against the pinned SHA; the generated view passes `validate_view`; no calculation reimplemented; the generator is integrated by SCH-006 |
| SCH-005 | Boundary study revit-alike ↔ edesign-alike | Agreed exchange format and acceptance; single electrical development |
| SCH-006 | Live values: MEP-002 hydraulic/airflow wiring, imported edesign-alike electrical results, adopted standard editions | Every annotation value traceable to a calculation module or an import, with units and the adopted standard edition recorded; matrix conformance with real values; the editions are resolved before this gate |

Dependencies, all acyclic, as in the 05 table, which is authoritative for the complete dependency list: SCH-001 on MEP-001, GUI-004 and sketch; SCH-002 on MEP-001 and SCH-001; SCH-003 on SCH-002, DOC-001 and GUI-004; SCH-004 on SCH-002; SCH-005 on SCH-001; SCH-006 on MEP-002, SCH-003, SCH-004 and SCH-005.

## Acceptance (first)

- End-to-end on a fixture model: zones drawn in plan → command → schematic with levels as rows, elements largest to smallest, wires from connectivity, the mandatory annotations per the matrix.
- Regeneration after a model edit reflects the edit; undo restores the previous schematic.
- Both spacing modes produce valid layouts on the fixture.
- Live calculated or imported values are accepted at SCH-006, with the adopted editions recorded.

## Open questions

- Which circuits each electrical-family schematic shows (all of a board, or a selected subset), and whether I/O point counts belong on the BMS schematic.
- Whether pumps (chilled water) and AHU/fans (air) are drawn as nodes rather than only as group labels.
- Whether the schematic is exposed as an OpenBIM Graph node: scheduled with AUTO-007 and out of scope for SCH-001–006.
