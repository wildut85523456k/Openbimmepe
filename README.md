# Open BIM platform (working name)

An open, native-multicore, Revit-style BIM authoring platform. Design decisions live in the project Doc
"Open BIM Platform: Design and Decisions"; this repository holds the code spikes and their independent checks.

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
| `crates/view` | plan views: looking down on a level with a horizontal cut — cut, projected and hidden elements |
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

Run everything: `cargo test --workspace --release`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo audit`.
The IFC scripts need `pip install ifcopenshell numpy`; see `.github/workflows/ci.yml` for the exact commands.

No standard values (material properties, U-values, sizes) are built into the program; test values are illustrative.
