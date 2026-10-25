# Coder Log — Docs Reconciliation (register + progress)

- **Date:** 2026-10-25
- **Branch:** `docs/gap-register-reconciliation-2026-10-25` (worktree `wt-gap-register-recon`)
- **Base:** `main` @ `b6ef0ee`
- **Contract:** `docs/agent-team/evidence/gap-register-recon-2026-10-25/coder-contract.md`
- **Files edited:** `docs/gap-remediation-plan-2026-10-21.md`, `docs/gap-progress-2026-10-23.md`

All banner text was extracted byte-for-byte from the contract (not hand-typed), so the curly
quotes around `"Easy"` and the en-dashes in `B7–B11` / `B12–B16` / `B21–B24` (U+2013) are exact.
Both files verified LF (0 CRLF on disk); minimal edits, no reformatting.

## §A — Verification-status banner (both docs) — DONE
Inserted the exact 5-line banner after each doc's title/intro:
- **register:** after the Scope paragraph, before the `---` separator.
- **progress:** after the `> **Re-check 16:36, 23 Oct:**` blockquote, before `## 1. Answer in brief`.

## §B — Register R4 workspace-member count — DONE (with a mapping note)
- The contract's verbatim find string `` `19 members (18 crates + desktop)` `` **does not exist**
  in the register. The unique row carrying that count *with the crate/desktop breakdown* is §8.1
  (was line 313): `The workspace has 19 members (18 crates plus \`apps/desktop\`); \`README.md\` lists 7.`
  Applied the correction there with a **minimal numeric edit** (kept the doc's `plus \`apps/desktop\``
  phrasing, per the no-reformatting constraint):
  `19 members (18 crates …)` → **`20 members (19 crates plus \`apps/desktop\`)`**.
- Contract line hint `(Appears ~lines 63, 70.)` was inaccurate — the string is at line 313. Lines 63
  (R4: `5 of the 19 workspace members`) and 70 (Class 1 acceptance: `README.md lists 19 members`)
  also carry the count `19` but **without** the crate/desktop breakdown, so they do not match the
  contract's find string. **Left unchanged (don't guess).**
  → **FLAG for reviewer:** those two `19` references are now numerically inconsistent with corrected
  line 313; decide whether they should also become `20`.
- §8.2.1 (was line 369, `…across 19 workspace members`) and the §8.3 size table (18 crates +
  `apps/desktop`) are **historical measurements pinned to the `dcf8382` test run** and were
  intentionally left as-is.

## §C — Progress — PARTIAL (see section-mismatch notes)
**Section-number mismatch:** the contract's §C targets `§4.1 / §4.2 / §4.3 / §4.4`, but this doc has
**no §4 subsections** — §4 is a single table ("What this means…"). The doc's delta *tables* are
§3.1 (Blockers/shell), §3.2 (Exchange/persistence), §3.3 (MEP content). I mapped the contract's
`§4.1/4.2/4.3` → **§3.1/3.2/3.3** (the only tables that predate the re-verified closures).

Applied:
- **SUPERSEDED banners** (verbatim) at the top of §3.1, §3.2, §3.3. The banner's `see §4.4`
  cross-reference points to a section that **does not exist** in this doc — inserted verbatim per the
  contract. → **FLAG for reviewer:** fix the `§4.4` target (likely `§4` or a new note).
- **C5, C16, C18:** status `**Open**` → `**Closed**` in §3.3 (matches the verified finding). C5's
  `(not re-checked)` suffix was kept (minimal edit) and is now stale. → **FLAG.**
- **§5.1 Easy tier:** added a `> **CLOSED (2026-10-25).**` annotation at the top of §5.1 with the
  closing commits `b17a1ff` / `fccee7b` / `91eb050` / `f84104a` / `7afe07b` and the tier set
  (`B2, B7–B11, B12–B16, B21–B24`).

Skipped (target not found / instruction not applicable — per the contract STOP condition
"don't guess"):
- **§C C3 — SKIPPED.** The find string `` `7 families open` `` is **not** in the doc (C3 in §3.3
  reads `**Partial**`, evidence `= 7`), and the replacement
  `` `19 members (18 crates + apps/desktop); families closed (B11)` `` is the **§B R4 workspace count
  leaked into a MEP-family row** — nonsensical for C3. NOT applied.
  → **FLAG:** the verified finding "progress §4.3 C3 = 19 members" appears to be a transcription error;
  C3 (MEP family library) is unrelated to the workspace-member count and remains correctly `Partial`.
- **§C C28 — SKIPPED.** The find string `` `grids cannot be arcs` `` is **not** in the progress doc.
  C28 appears once, at (former) line 115, conjoined with C23
  (`grid rename/arcs, wall top constraint and wrapping still missing (C23, C28)`). No clean C28-only
  target and no matching find string. NOT applied.
  → **FLAG:** the verified finding "C28 arc grids exist (`crates/document/src/grid_arc.rs`, `01ddf1f`)"
  is real but could not be cleanly mapped; a reviewer may want a best-effort edit to that line-115
  mention.

## Constraints honored
- Docs only: the two named docs + this log. No code/tests touched. Did **not** touch
  `D:/Codex/Revit-alike-P0`.
- LF line endings preserved on both docs.
- Minimal edits, no reformatting; banners inserted verbatim from the contract.

## Not done (per contract)
- **Not pushed. No PR.** Local commit only
  (`docs: reconcile gap register + progress to main @ b6ef0ee`).

## Reviewer checklist
- [ ] Confirm §B: update lines 63 & 70 `19` → `20` too, or accept the documented inconsistency.
- [ ] Confirm §C banner target `§4.4` (does not exist) — fix or leave.
- [ ] Decide C3 (contract text is corrupt) and C28 (no matching target) handling.

## Resolved (orchestrator follow-up, 2026-10-25)
- **§B lines 63/70:** updated `README.md lists 19 members` -> `20 members` (register R4 row + Class 1 acceptance). The §8.2.1 `19 workspace members` is a historical dcf8382 test-run result, left as-is.
- **§C banner `§4.4`:** removed the cross-reference (no §4.4 exists in the doc).
- **C3:** verified against code — the family registry holds **19 families** (not 7): handle, door, window, arched door, fan coil, ahu, ceiling diffuser, VAV, pump, valve, WC, washbasin, sprinkler head, luminaire, panel board, hanger, pipe elbow/reducer/tee (`crates/family/src/library.rs:481-500`). C3 is **Closed** (register §8 evidence row + progress §3.3 row updated). The earlier "19 members" finding was a mis-read of the family count.
- **C28:** updated the line-127 mention — arc grids exist (`crates/document/src/grid_arc.rs`, `01ddf1f`).

## Independent review (bim-review, 2026-10-25) — FAIL, then fixed
The reviewer rejected the candidate; findings + resolutions:
- **P1 — C5 wrongly closed.** `family_click` routes non-door/window families to `place_family(level, at, spec)` -> `Geometry::Placed { at, level }` (no ceiling/face host); `place_family_in_wall` is wall-only. Ceiling/face hosting is **not implemented**. **Fix:** C5 restored to `**Open**` (was wrongly `Closed (not re-checked)`).
- **P3 — family citation omitted the 19th entry.** `pipe_tee()` is at `library.rs:501`; `:481-500` excluded it. **Fix:** both C3 citations -> `:481-503`.
- **P3 — R4 missing-member list omitted `commands`.** **Fix:** added `commands` (20 members - 5 listed = 15 missing).
Other reviewer notes (no doc change): the C28 statement is in §4 (not §3.4); `cargo fmt` FAIL + release-test BLOCKED are pre-existing Rust/environment, not docs-induced.
