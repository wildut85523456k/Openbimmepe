# Full build: with the edesign-alike electrical solver

The main workspace builds without any private repository. This separate workspace adds the one piece that
needs one: the `edesign-alike` electrical solver, which calculates the feeder currents and voltage drops
stamped on single-line schematics (SCH-006, MEP-002).

| Crate | What it is |
|---|---|
| `electrical-adapter` (`openbim-electrical-adapter`) | Implements `openbim_document::electrical_calc::ElectricalCalc` with the solver; holds the solver's end-to-end tests |
| `desktop-full` (`openbim-desktop-full`) | The desktop app with the solver installed at start-up |

How it fits: `openbim-document` describes the model's network as plain data and asks the installed engine
for results. The open build installs none, so boards keep "A —", "V —", "% —" and the schematic says the
currents were not calculated. Nothing is ever invented in place of a solver result.

## Build

Needs read access to `github.com/wildut85523456k/edesign-alike` (a git credential, or the
`P0_DEPENDENCY_READ_TOKEN` secret in CI).

```sh
cd full
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -p openbim-desktop-full --release
```

`full/Cargo.lock` was seeded from the main lockfile. On first use, run `cargo build` once **without**
`--locked` so Cargo adds the two local crates, then commit the updated lockfile. The `full-build`
workflow (manual dispatch) runs the same commands.
