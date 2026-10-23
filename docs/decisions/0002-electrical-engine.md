# 0002 - The in-repo electrical engine: deprecated, kept until edesign-alike sizes cables

Status: decided with the owner, 23 Oct 2026 (gap B26). Follows the decision of 21 Oct 2026, option B: electrical
calculations are imported from edesign-alike and not re-implemented here.

## Finding

Row B26 asked to remove `crates/calc/src/electrical.rs`, `crates/document/src/compute_elec.rs` and the Compute
electrical window, with edesign-alike's electrical crates pinned by SHA. Checked against edesign-alike at the pinned
`57ef018` and at its latest commit `3fff66e` (22 Oct 2026):

- The pinned crates are already in use for the single-line schematic (`schematic_electrical.rs`): currents, voltage
  drop and short-circuit values are read from `openbim-electrical-solver`.
- The solver **checks** a cable's ampacity given to it (`Ib <= Iz`). It does not **choose** a cable from tables with
  correction factors, check the protective device, trip and breaking, or size the protective conductor.
- Its README lists "cable sizing/voltage drop" among the release gates still open.

Removing the engine now would remove cable and protection sizing from the app with nothing to replace it.

## Decision

1. The engine and its window are **deprecated**: the files say so, the Compute electrical window shows a notice, and
   no new features go in.
2. They are **not removed** until edesign-alike sizes cables from tables.
3. When it does: pin the new SHA, move the Compute electrical window to it, delete `electrical.rs`, `compute_elec.rs` and
   `elec_ui.rs`' use of them. Acceptance of B26 is then as written: no desktop build or Compute test references the
   in-repo engine.

## Consequences

- The "no duplicate development" rule holds for new work. The existing code is a stopgap with an end date set by
  edesign-alike, not by this repository.
- Row B26 stays open in the plan, now blocked on that feature.
