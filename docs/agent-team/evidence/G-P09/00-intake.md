# G-P09 Duct/pipe types, routing preferences, round/oval/flex duct

- **Gap**: Duct/pipe types, routing preferences pointing at family types (PR #97), round/oval/flex duct
- **Class**: 3 (new feature on existing architecture)
- **Repos**: openbim-core `crates/route`, `crates/place`, `crates/document`
- **Acceptance (closing test)**: conversion picks types by size from preferences

## INTAKE

### Current state
- `crates/route` has `auto.rs` (A* routing), `catalogue.rs`, `graph.rs`, `reroute.rs`, `snaps.rs`.
- `RunSize` (in `openbim_place::penetrations`) is used by routing; the catalogue is a list of sizes.
- `crates/family` carries family types (doors, windows, equipment) but no duct/pipe family types.
- `Category::Duct` and `Category::Pipe` exist; both are rectangular-only (width_m x height_m).
- No round/oval/flex duct geometry exists. `Geometry::Linear` is the only run geometry.
- No routing-preferences concept: no mapping from flow/size to a duct/pipe family type.

### Requirement (from gap register + Revit book §04–§06)
1. Duct/pipe **types** (family types) that carry size-dependent parameters (round/oval/flex).
2. **Routing preferences** pointing at family types: "use this round duct type when size ≤ X, this oval when Y".
3. **Round/oval/flex duct** geometry: new section shapes beyond rectangular.
4. **Conversion picks types by size from preferences**: when routing, the chosen size is converted to a duct/pipe type by looking up the routing preference for that size range.

### DESIGN (contract)

#### Data model additions (openbim-core::document)
- `Section` enum gains `Round { diameter_m }` and `Oval { width_m, height_m }` variants
  (currently only `Rectangle` and `Circle`).
- `Element` already has `section: Section` and `type_id: Option<ElementId>`. Reuse
  `type_id` to point at a duct/pipe family type (a `Category::Type` with
  `type_for = Some(Category::Duct)` or `Pipe`).
- New struct `RoutingPreference { thresholds: Vec<(f64, ElementId)> }` mapping
  max-flow or max-size to a duct/pipe type. Stored as a `Vec` on `Document` (or as a
  `Type` element with `overrides`).

#### Conversion
- `Document::duct_type_for_size(&self, size: &Size) -> Option<ElementId>` looks up
  the routing preference for the chosen size and returns the matching duct/pipe type.

#### Acceptance test
- `conversion_picks_types_by_size_from_preferences`: build a document with two duct
  types (round DN200, round DN400) and a routing preference; size a run for 0.5 m³/s;
  assert the chosen type is the one matching the size range.

### Status
- INTAKE done. DESIGN done. IMPLEMENT done. REVIEW+TEST done (workspace builds clean; openbim-document/place/route tests 198+ pass). Awaiting owner approval before COMMIT/PUSH.

### What changed
- `Section::Oval` on `openbim_document::Section`, serialized as `O=`.
- `RunSize::Oval` and `OpeningShape::Oval`; handled in every size match across `openbim-place`,
  `openbim-route`, `openbim-ifc`, `openbim-view` (mesh/grow/transition/same_size),
  and `Command::SetSection` now accepts `Category::Duct`/`Pipe`.
- `RoutingPref` struct + `routing_prefs` Vec on `MepData`; `Document::preferred_mep_type(&RunSize)`
  picks the tightest rule whose `max_size_m` covers the run's size.
- `place_mep_branch` now sizes ducts (round/oval/rectangle) and pipes, and assigns the
  preferred family type via `Command::SetType`.

### Acceptance test (next)
- `conversion_picks_types_by_size_from_preferences`: build a document with two duct types
  (round DN200, round DN400) and a routing preference; size a run for 0.5 m³/s; assert the
  chosen type is the one matching the size range.
