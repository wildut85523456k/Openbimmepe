# OpenBIM / Revit-alike

Rust desktop BIM authoring with egui/wgpu, a native project format and IFC exchange.
The application is under development. Implemented subsets are not full Revit parity.

Start with the [current capability register](docs/capability-register.md), which records the inspected commit,
implementation evidence, limitations and P0 validation gates. The [UI specification](docs/revit-style-ui/README.md)
defines the intended behaviour; historical audits describe their own pinned baselines.

## Run and validate

```sh
cargo run --locked -p openbim-desktop
cargo test --locked -p openbim-desktop
cargo test --locked --workspace --release
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo audit
```

Windows requires MSVC Build Tools and the Windows SDK; Linux needs X11/Wayland/graphics development libraries.
The workspace depends on pinned private `edesign-alike` crates: contributors and clean runners need read access.
A lockfile pins versions; it does not provide credentials or cached source.
See [P0 validation](docs/p0-validation.md) for native CI and evidence limits.

## Implementation map

| Area | Location |
|---|---|
| Desktop, ribbons, properties, browser and tools | `apps/desktop` |
| Document transactions, native records, editing, schedules, sheets | `crates/document` |
| Geometry, sketching, parametric families and placement | `crates/geom`, `sketch`, `family`, `place` |
| Architecture and envelope primitives | `crates/arch` |
| MEP routing, calculations and schematics | `crates/route`, `calc`, `schematic` |
| IFC reading/writing | `crates/ifc` |
| Drawing generation and 3D viewport | `crates/view`, `viewport`, `viewport-egui` |
| Sheet rendering, print profiles, PDF/DXF | `crates/sheets` |
| Recovery operation log and shortcuts | `crates/oplog`, `keymap` |
| Command API and automation integration | `crates/commands`, `automation` |
| Photo/report utilities and independent checks | `crates/photo`, `spikes` |

Save/Open, autosave recovery, architectural placement, MEP routing, schedules, sheets and IFC exchange have
implementations. Associative dimensions, complete documentation workflows, a graphical family editor and
shared-folder collaboration remain incomplete. See the register before planning new work.

Example/test sizes and engineering values are illustrative, not project design recommendations.
