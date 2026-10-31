# OpenBIMMepe — candidate documentation

This directory holds the S2 deliverables for **OpenBIMMepe**, an independently-buildable
desktop candidate forked from `revit-alike` commit `76963c0` (PR #111), pinning
`openbim-core` to `544ee0d7c035cb0dffa57f5981094f28fe7ce591` and `egui`/`eframe` to `0.37`.

- **`OpenBIM-GAP-REGISTER-2026-10-31.csv`** — the 60-row gap register refreshed against
  the chosen candidate. Statuses are the state after the S0/S1 work below; see
  `S1-S3-contracts.md` for what each letter is.
- **`S1-S3-contracts.md`** — bounded contracts for every stream; which are done/pushed,
  which are blocked, which are S3/future.
- **`S0-PRESERVATION-LEDGER.md`** — what the fork preserves verbatim from `revit-alike@76963c0`
  and what it intentionally changes.
- **`dependency-diagram.svg`** — ownership/dependency diagram (app → pinned core → crates,
  host, downstreams).

## Evidence for the candidate

| Stream | Commit (local + remote `main`) | Build | Tests |
|---|---|---|---|
| S0-defect (`ifc_export` arm) | `a4ebb05` | `cargo build -p openbim-desktop` → exit 0 | 273 → **275 passed**, 0 failed, 1 ignored |
| S1a (theme) | `fa29309` | green, 2 warnings (`model_view.rs` unused vars, pre-existing) | **276 passed**, 0 failed, 1 ignored |
| S1b (Revit shortcuts + mouse) | `10e0347` | green | **277 passed**, 0 failed, 1 ignored |

Toolchain: `cargo 1.99.1 / rustc 1.88.0`. The pre-existing `.build-watchdog.log`
(egui 0.36) is stale and does **not** describe this candidate build; see
`S0-PRESERVATION-LEDGER.md`.

## How the register maps to this fork

The register's `evidence` column names the upstream reference contracts
(BIMCraft, Lapin, etc.). For OpenBIMMepe, the equivalent evidence is the candidate's
own build/test log at the commit shown above, plus the source at `544ee0d7`. Rows
promoted by S1 are marked `VERIFIED` below with the commit that closed them.

### Rows closed by S1 (post-S0)

| Row | Was | Now | Closed by |
|---|---|---|---|
| REA-007 | PARTIAL | **VERIFIED** | S1a theme (commit `fa29309`) |
| REA-046 | PARTIAL | **VERIFIED** | S1b Revit shortcuts + mouse (commit `10e0347`) |
