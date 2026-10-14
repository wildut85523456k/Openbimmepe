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
| `spikes/*` | Python scripts that write the Rust output to IFC4 and check it against hand-calculated figures |

Run everything: `cargo test --workspace --release`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo audit`.
The IFC scripts need `pip install ifcopenshell numpy`; see `.github/workflows/ci.yml` for the exact commands.

No standard values (material properties, U-values, sizes) are built into the program; test values are illustrative.
