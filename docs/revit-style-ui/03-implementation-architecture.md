# 03 — Proposed implementation architecture

## Existing Rust workspace reviewed (October 2026)
`crates/geom`, `family`, `place`, `keymap`, `arch`, `oplog`, `sketch`, `route`, `ifc`.
Existing `keymap`, `oplog`, `family`, `route`, `ifc` should be reused, not duplicated. The egui desktop app (`apps/desktop`) and a provisional `crates/document` are on branch `feature/rust-gui`; `crates/document` is to be merged into the oplog model (#24).

## Layering
```text
Desktop GUI (egui): ribbon | palettes | project browser | viewport     OpenBIM Graph | MCP clients
                      |                                                    |
                Command registry (openbim-commands) <----------------------+
                      |
       Document / transaction / dependency graph
          |           |            |
    geom/sketch    family/place   arch/route
                      |
           IFC importer/exporter
                      |
   project files on a local disk OR a shared folder (server share / cloud-synced)
```

## Proposed crate/app boundaries (names provisional)
- `openbim-document`: stable element identifiers, typed properties, type/instance references, host links, spatial containment, dependency tracking.
- `openbim-commands`: command schemas, applicability, previews, validation, transaction execution.
- `openbim-storage`: snapshots, operation log integration, transactional local project persistence.
- `openbim-sync`: shared-folder synchronisation without a server: one writer per segment file, heartbeats, an elected snapshot writer, element borrowing, locks giving change requests, per-element commit rules (#33, #34).
- `openbim-render`: 2D/3D scene adapter, incremental tessellation cache, picking, graphic styles.
- `apps/desktop`: GUI composition only; no direct IFC mutation.
- `openbim-commands` also serves OpenBIM Graph (local link, scoped token) and the MCP server in the app; `openbim-graph`: headless graph engine shared with OpenBIM Graph (see 04).

## GUI toolkit: egui (decided 15 Oct 2026)
egui with wgpu, all in Rust: the UI and the 3D viewport draw in the same frame with no language bridge, and the same toolkit serves OpenBIM Graph. Qt 6 (C++), Slint and the web shell are dropped (#22). Still to build ourselves: the ribbon, docking polish and the visual theme.
Measure on the reference scenario (docked Properties and Project Browser, ribbon and contextual Place Duct, 2D/3D view, keyboard/mouse, 100k-element picking): latency, install size, accessibility and platform compatibility.

## Multi-user rule
Never share one writable IFC file as the central collaboration protocol. Standalone and shared-folder modes use the same validated transactions. Each user writes only their own segment files; everyone merges what the others wrote with the per-element rules (last writer wins and flags the loser; locked elements give change requests). Discipline models are linked into the central file, not merged.
Conflict checks must include *semantic dependencies* (hosted door vs wall; duct vs fitting), not only same element/property collisions.

## IFC rule
IFC (IFC4, plus IFC2X3 for DIALux) is the interchange/semantic standard, read and written in Rust inside the app; native parameter dependencies are separately persisted. Preserve GlobalIds and unsupported imported data where feasible; enforce schema and round-trip tests.

## Performance
- Incremental recalculation per dependent elements, not entire project rebuilds (see existing issue #14).
- Spatial index for route obstacle queries (see issue #21).
- Parallel geometry generation where dependency boundaries allow.
- Versioned graphics caches and background computation with cancellation.

## Security and reliability
Access to a shared project follows the shared folder's permissions plus element borrowing per discipline; authorization remains project/discipline/action aware.
No Python in the app: Python runs only in OpenBIM Graph, in worker processes, with permissions declared in each graph. OpenBIM Graph and MCP clients get tokens scoped to those permissions (#39).
Transactional recovery, periodic durable snapshots, audit trail and schema migration are mandatory.
