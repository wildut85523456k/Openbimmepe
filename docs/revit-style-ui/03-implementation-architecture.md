# 03 — Proposed implementation architecture

## Existing Rust workspace reviewed (October 2026)
`crates/geom`, `family`, `place`, `keymap`, `arch`, `oplog`, `sketch`, `route`, `ifc`.
No integrated desktop GUI is established by these crates. Existing `keymap`, `oplog`, `family`, `route`, `ifc` should be reused, not duplicated.

## Layering
```text
Desktop GUI: ribbon | palettes | project browser | viewport | node editor
                      |
                Command registry
                      |
       Document / transaction / dependency graph
          |           |            |
    geom/sketch    family/place   arch/route
                      |
           IFC importer/exporter
                      |
        standalone persistence OR central sync
```

## Proposed crate/app boundaries (names provisional)
- `openbim-document`: stable element identifiers, typed properties, type/instance references, host links, spatial containment, dependency tracking.
- `openbim-commands`: command schemas, applicability, previews, validation, transaction execution.
- `openbim-storage`: snapshots, operation log integration, transactional local project persistence.
- `openbim-sync`: revisioned server-authoritative transaction protocol, ownership/leases, conflict handling.
- `openbim-render`: 2D/3D scene adapter, incremental tessellation cache, picking, graphic styles.
- `apps/desktop`: GUI composition only; no direct IFC mutation.
- `openbim-api`: script-facing safe client interface shared by Python and visual graphs.

## GUI toolkit evaluation (no choice made yet)
Spike each option with identical scenario: docked Properties and Project Browser, ribbon and contextual Place Duct, 2D/3D view, keyboard/mouse, selection and 100k-element picking.
- **Qt 6**: mature desktop docking/widgets; bridges and mixed-language build/deploy increase complexity.
- **Slint**: declarative Rust-friendly interface; assess advanced CAD docking and interaction needs.
- **egui + wgpu**: predominantly Rust, flexible tooling and embedded node canvas; more custom UI behavior/polish required.
Record objective latency, install size, accessibility, testability and platform compatibility.

## Multi-user rule
Never share one writable IFC file as the central collaboration protocol. Local and central modes use the same validated transactions; central server decides commit order and broadcasts revisions.
Conflict checks must include *semantic dependencies* (hosted door vs wall; duct vs fitting), not only same element/property collisions.

## IFC rule
IFC is the interchange/semantic standard; native parameter dependencies are separately persisted. Preserve GlobalIds and unsupported imported data where feasible; enforce schema and round-trip tests.

## Performance
- Incremental recalculation per dependent elements, not entire project rebuilds (see existing issue #14).
- Spatial index for route obstacle queries (see issue #21).
- Parallel geometry generation where dependency boundaries allow.
- Versioned graphics caches and background computation with cancellation.

## Security and reliability
Authentication can use OIDC; authorization remains project/discipline/action aware.
Python runs isolated or with explicit local trust: never permit arbitrary code inside central authoritative process.
Transactional recovery, periodic durable snapshots, audit trail and schema migration are mandatory.
