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
The workspace builds with no private access. The electrical solver (single-line feeder currents and voltage
drops) is the private `edesign-alike` repository and is plugged in only by the separate [full build](full/README.md);
without it, schematics keep their placeholders and say the currents were not calculated.
See [P0 validation](docs/p0-validation.md) for native CI and evidence limits.

## Crates

| Crate | What it is |
|---|---|
| `crates/geom` | wall chains, mitres, openings, exact volumes, meshes (S1) |
| `crates/family` | parametric family engine: formulas, typed values, nested families, turns (S2) |
| `crates/place` | placing families in walls and cutting openings |
| `crates/keymap` | keyboard shortcut presets and user overrides |
| `crates/arch` | levels, grids, rooms, layered assemblies, envelope, windows, ceilings, shafts |
| `crates/oplog` | operation log spike (S8): a small binary format with a CRC-32 per record, so a torn tail is detected and dropped |
| `crates/sketch` | sketch-and-lock: a small 2D geometric constraint solver (S2h) |
| `crates/route` | MEP routing: a path drawn point by point becomes straight runs joined by fittings |
| `crates/ifc` | IFC4 writer: takes the model as the app holds it (wall chains, slabs, openings, MEP networks) and writes a STEP file |
| `crates/view` | plan views: looking down on a level with a horizontal cut — cut, below-cut, overhead and beyond geometry |
| `crates/viewport` | the logic of a 3D view with no window or GPU: camera, mouse-driven orbit, ViewCube, software renderer |
| `crates/viewport-egui` | the 3D view as a widget for egui, drawn with wgpu |
| `crates/photo` | photometric files as manufacturers publish them: IES (IESNA LM-63) and Eulumdat (.ldt) |
| `crates/sheets` | sheets, title blocks and printing, the Revit way |
| `crates/automation` | independent typed automation: evaluation is read-only; only a validated plan can modify the document |
| `crates/calc` | sizing and losses of a branched network of pipes or ducts |
| `crates/document` | sprint 1 BIM document foundation |
| `crates/schematic` | openbim-schematic: the pure grid layout engine behind `MEP: Generate Schematic` |
| `crates/commands` | typed command and query API (AUTO-001) |
| `apps/desktop` | the desktop application |
| `spikes/*` | Python scripts that write the Rust output to IFC4 and check it against hand-calculated figures |

Save/Open, autosave recovery, architectural placement, MEP routing, schedules, sheets and IFC exchange have
implementations. Associative dimensions, complete documentation workflows, a graphical family editor and
shared-folder collaboration remain incomplete. See the register before planning new work.

Example/test sizes and engineering values are illustrative, not project design recommendations.
