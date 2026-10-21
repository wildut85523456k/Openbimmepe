## What

Records the owner's answers of 21 Oct 2026 to the questions
`docs/gap-remediation-plan-2026-10-21.md` left open (its section 6), and maps that plan's rows to
`docs/revit-style-ui/05-backlog.md`.

- `08-gap-closure-plan-2026-10-21.md` is re-scoped from "a second plan" to what it adds: the dated
  decisions, the row-by-row mapping (this record's item → the plan's **B** row → the backlog row), and
  the P0 acceptance the work was held to. The analysis, the work classes and the order stay with
  `docs/gap-remediation-plan-2026-10-21.md`, which this document now names as authoritative.
- New backlog rows: **GUI-006** (from-scratch journey), **GUI-007** (ribbon honesty), **BIM-005**
  (level authoring), **IFC-002** (IFC honesty), **MEP-003** (MEP family library), **MEP-004**
  (data-hall MEP chain, P3), **MEP-005** (electrical calculations from edesign-alike), **DOC-002**
  (docs refresh), **TEST-001** (end-to-end harness). Acceptance extended on BIM-004, GUI-004,
  GUI-005, IFC-001, ARCH-001, MEP-002 and SCH-004. Phase gating gains step 2a (the walking skeleton).
- `README.md`: the document list points at the analysis and the plan on `main`; decision log entry 7.

## Decisions recorded (answering the plan's section 6)

1. Unbuilt ribbon buttons are shown **disabled**, not as "not implemented" messages.
2. Storage = operation log + snapshots; autosave and crash recovery are the first `crates/oplog`
   consumer.
3. First MEP family library: AHU, ceiling diffuser, VAV, pump, valve, WC/basin, sprinkler head,
   luminaire, panel board, hanger/support.
4. Electrical calculations are exported to and imported from edesign-alike (the plan already records
   this as option B / row B26); the in-repo module is retired only after equivalence tests.
5. End-to-end tests use a data hall for MEP and keep the office for architecture.
6. *New* creates Level 1 and Level 2.

## Corrections made during review

An earlier commit on this branch landed the gap analysis again as `07-…` and claimed the audits were
not tracked. Both were wrong: `main` already carries `docs/gap-analysis-2026-10-21.md`,
`docs/audits/2026-10-21-*-audit.md` and the `verify-*.md` reports. `07` is deleted and the false
provenance note is gone. Four backlog rows also had acceptance that depended on another row's
deliverable (two of them in a cycle); each deliverable now has one owner.

## Review

Independently reviewed: `report2.md` gate **G10 PASS** (row dependencies, ids, CRLF, links) and
`report3.md` gate **D5 PASS** (the de-duplication, every cited `B…` row exists in the plan), both in
`C:/Users/wilfr/hermes-first-task/wt-gap-p0-review/`.
