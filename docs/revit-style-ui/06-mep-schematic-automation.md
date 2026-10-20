# 06 — MEP schematic automation: single-line generation from the design model

Status: **proposed 20 Oct 2026; user decisions recorded below; not implemented.** Companion to 02 (workflows), 04 (automation) and 05 (backlog rows SCH-001–006).

## Decisions (user, 20 Oct 2026)

1. **Zone authoring (approved).** Schematic groups are authored in the plan viewport: the designer drags a rectangle on a level and names it. The zone is a first-class document-model entity, not derived data. IFC space containment is a fallback labeler and an auto-suggestion source only.
2. **Electrical reuse (approved, "option A").** revit-alike pins `openbim_electrical_model` and `openbim_sld` from the edesign-alike repository by git SHA. Electrical calculations are produced by edesign-alike and imported; revit-alike does not reimplement them. A follow-up boundary study (SCH-005) defines the data push: panels, connectivity and loads out of the revit-alike model; panels imported and arranged in edesign-alike; calculated current and voltage drop back. Single development, no duplicate electrical engine.
3. **Spacing (both modes, designer selects).** Uniform per-element slot pitch and footprint-proportional spacing are both implemented; the designer picks per generation. Default: uniform.
4. **Discipline scope (approved).** Electrical, ELV, access control, BMS, plumbing, HVAC chilled water, HVAC air distribution, sprinkler/fire fighting. The framework is multi-discipline from the start; annotations are per-discipline (matrix below).

## Scope

In scope: one command `MEP: Generate Schematic` that reads the design model and produces a single-line schematic view: zones, extraction, grid layout, wires from connectivity, per-discipline annotations, sheet export.

Out of scope: the calculation engines themselves (MEP-002 modules; edesign-alike electrical calculations), the OpenBIM Graph node wrapper (AUTO-007, later), clash detection, protection-device selection.

## Pipeline

1. **Command.** Ribbon command through the typed command API (BIM-002/AUTO-001). Later an OpenBIM Graph node per AUTO-007; the node wraps the command and reimplements no calculation.
2. **Zones.** Plan rectangle tool; zones stored in the document model.
3. **Extraction query.** Panels/controllers for electrical-family disciplines; zone-summarized endpoints for hydraulic/air disciplines; connectivity from the model.
4. **Grid layout.** Pure function: elements + levels + connections + spacing mode → symbol slots and wire routes. No I/O, no rendering.
5. **Rendering + annotations.** Single-line symbols, wires, per-discipline value labels, sheet export.

## Zones: authoring and assignment

- Entity: `SchematicZone { id, level, rectangle, label }` stored in the document model (operation log), so assignment is deterministic, persisted and undoable.
- Assignment rule: an endpoint whose plan position lies inside the rectangle belongs to the zone. Overlaps are reported in the UI; resolution is first-match by creation order (deterministic), and the designer resolves the overlap.
- Fallback: the zone label inherits the containing `IfcSpace` name; zones can be auto-suggested from spaces, but spaces never define groups on their own.
- Electrical-family disciplines (electrical, ELV, access control, BMS) pull panels/boards/controllers as schematic nodes; hydraulic/air disciplines (plumbing, chilled water, air, sprinkler) summarize terminal endpoints per zone.

## Extraction

- Electrical, ELV, access control, BMS: panels / distribution boards / controllers pulled as nodes.
- Plumbing, chilled water, air distribution, sprinkler: terminal endpoints summarized per zone (group flow = sum of fixture/terminal flows).
- Connectivity: from the model connection topology (`openbim_family::mep` `Connector`/`System`; IFC port connections on import) → the wire list of the single-line.
- Ordering key, largest to smallest: electrical family = downstream load (A, then kVA); hydraulic/air = group flow (L/s, m³/h).

## Grid layout

- Rows = levels, sorted by elevation; each row is labeled with the level name.
- Columns = elements of that level, sorted by the ordering key, largest first.
- Spacing mode, designer-selectable per generation: **uniform** slot pitch (default) or **footprint-proportional**.
- Crossings are reduced with a barycenter tie-break over connectivity order.
- Pure function `(elements, levels, connections, mode) → (symbol slots, wire routes)`; golden-testable without a renderer.
- Fixture `fixtures/schematic-layout-v1.json` is specified by this document and added with SCH-002; golden tests recompute expected slots and orders from the fixture inputs at 1e-9 (clean-room pattern).

## Annotations (per-discipline matrix)

Two annotation families, per the user's rule: electrical-family disciplines annotate current and voltage drop; hydraulic/air disciplines annotate flow and pressure drop. Non-electrical disciplines carry no power or voltage annotations.

| Discipline | Nodes pulled | Grouping | Annotations |
|---|---|---|---|
| Electrical | panels / distribution boards | per panel | load current (A), voltage drop (V and %); kW proposed. Values imported from edesign-alike |
| ELV | ELV panels / PSUs | per panel | current (A), voltage drop (V and %) |
| Access control | controllers / PSUs | per controller | current (A), voltage drop (V and %) on lock/reader circuits (proposed) |
| BMS | controllers | per controller | I/O point counts and connectivity; current/voltage drop on power feeds (proposed) |
| Plumbing | — | endpoints per zone | flow (L/s), pressure drop (kPa) |
| HVAC chilled water | — (pumps as nodes, proposed) | endpoints per zone | flow (L/s or m³/h), pressure drop (kPa) |
| HVAC air distribution | — (AHU/fans as nodes, proposed) | terminals per zone | airflow (m³/h or L/s), static pressure (Pa) |
| Sprinkler / fire fighting | — | heads per zone | flow (L/s or L/min), pressure (kPa) at the most remote head |

- Every annotation value comes from an MEP-002 calculation module (hydraulic/airflow) or an imported edesign-alike result (electrical). Units and the standard/edition are recorded with each value (AUTO-007 rule: no calculation reimplemented in nodes).
- Branch lengths for voltage drop / pressure drop come from route run geometry (centerline lengths).
- Where company rules are missing, apply: voltage drop per the IEC 60364-5-52 method; hydraulics per Darcy-Weisbach or Hazen-Williams (SI units); sprinkler hydraulics per EN 12845 / NFPA 13 as applicable. The edition is recorded; licensed normative texts are reviewed before any compliance claim.

## Electrical reuse and the edesign-alike boundary

- Option A (approved): pin `openbim_electrical_model` + `openbim_sld` from the private edesign-alike repository by git SHA, mirroring how edesign-alike pins `openbim-family` from revit-alike. The sld crate is presentation-only by design (`DiagramView` owns no topology; `validate_view` checks it against the model) — the generator fills a `DiagramView` from the canonical graph.
- The two repositories stay separate applications; the pin is a dependency, not a merge. Authorization to push to either repository does not authorize pushing the other.
- Boundary study (SCH-005, follow-up): exchange format and command/API mechanism for pushing model data (panels, connectivity, loads) from revit-alike to edesign-alike, importing and arranging panels there, and returning calculated data. Acceptance: a study doc with an agreed format. No second electrical development.

## Symbols and rendering

- Native symbol library drawn to IEC 60617 / ISO single-line conventions; no ABB, Autodesk or other third-party assets.
- Rendered through the existing `view`/`viewport` crates; export through DOC-001 and the sheets/printing work (`openbim-sheets`, landscape paper).
- The schematic is a view over the model: generation never mutates the design model; zone edits and regeneration are undoable like any edit.

## Phases and gates

| Phase | Content | Gate |
|---|---|---|
| SCH-001 | Zone tool + extraction query (data only, JSON dump) | zones persist; assignment deterministic |
| SCH-002 | Grid layout engine, both spacing modes | golden tests at 1e-9 from fixtures |
| SCH-003 | Single-line view + annotations + command | repeatable output; matrix conformance |
| SCH-004 | Electrical reuse: pinned crates + DiagramView generator | `validate_view` passes; no calculation reimplemented |
| SCH-005 | Boundary study revit-alike ↔ edesign-alike | agreed exchange format |
| SCH-006 | Calculation wiring for all disciplines | every annotation traceable, units + standard recorded |

SCH-001–003 are unblocked by MEP-001/GUI-004; SCH-006 is gated on MEP-002 and SCH-005.

## Acceptance (first)

- End-to-end on a fixture model: zones drawn in plan → command → schematic with levels as rows, elements largest to smallest, wires from connectivity, annotations per the matrix.
- Regeneration after a model edit reflects the edit; undo restores the previous schematic.
- Both spacing modes produce valid layouts on the fixture.

## Open questions

- BMS and access-control annotation sets are proposed, not yet agreed with the team.
- Whether the schematic is exposed as an OpenBIM Graph node immediately or only after AUTO-007.
- Default spacing mode when the designer does not choose (uniform is assumed).
