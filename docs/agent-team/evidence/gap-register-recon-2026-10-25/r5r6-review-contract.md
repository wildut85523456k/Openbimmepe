# Review contract — R5 (stale claims) + R6 (extraction manifest)

Reviewer: `bim-review` profile. READ-ONLY. Verdict PASS/FAIL with findings (id, severity, file:line, evidence, required fix). Do NOT edit, commit, or push.

## Candidate
- Repo: `wildut85523456k/revit-alike`. Worktree: `C:/Users/wilfr/hermes-first-task/wt-gap-register-recon`.
- Candidate SHA `a8158aa` (branch `docs/gap-register-reconciliation-2026-10-25`); base `b6ef0ee` (= `b6ef0ee`, live main).
- Diff: `git -C "C:/Users/wilfr/hermes-first-task/wt-gap-register-recon" diff b6ef0ee..a8158aa`. This is the R5 + R6 slice (the previous reconciliation is in the base).

## What the slice claims
R5 (register `docs/gap-remediation-plan-2026-10-21.md` row R5): mark six stale-document claims at their source, keeping the original wording, appending `superseded 2026-10-21` + a code citation. R6 (row R6): add `docs/reference/extraction-manifest.md`.
Researcher evidence: `docs/agent-team/evidence/gap-register-recon-2026-10-25/research-r5r6.md`. Coder log: `.../coder-r5r6-log.md`.

## Verify each R5 mark (the mark must keep the original statement AND add an accurate superseded note)
1. `docs/revit-ux-parity-audit.md` event-test claim — marked STALE: event tests now cover ribbon (`model_view.rs:1406` ribbon_click), options bar (`mep_settings_ui.rs:188`, `model_view.rs:1645`), global keys (`model_view.rs:1416` key_frame). Verify the citation + that the original is kept.
2. `docs/3d-view-parity-plan.md` — PARTIAL: `:21` partly superseded (3D Family `view3d.rs:251` + Beam `model_edit.rs:495` exist; modify tools still plan-only), `:36` redo half STALE (`main.rs:647/714`, `oplog redo()`). Verify the mark is genuinely partial (does NOT over-claim that all 3D tools exist) + the redo citation.
3. `docs/agent-team/gap-review.md:26` GUI-001 — STALE (`apps/desktop` is a workspace member). Verify.
4. `docs/revit-style-ui/06-mep-schematic-automation.md:5` — STALE (`crates/schematic`, `document/src/schematic.rs`, `schematic_view.rs` in main). Verify.
5. `docs/revit-book-review.md:63` A8 — STALE (`plan_tabs` derives from levels, `model_view.rs:24`; `7afe07b`). Verify.
6. `docs/agent-team/gap-review.md:38` MEP-002 — **STILL-TRUE, deliberately left UNMARKED.** Confirm the coder did NOT mark it (it is not stale) and that this is the correct call (the mismatch still exists; gap-review.md:17 discloses the labels are its own assessment). Flag if it was wrongly marked OR if leaving it unmarked leaves the register R5 row's list inaccurate.

For each: confirm (a) the original statement is preserved (nothing deleted), (b) the `superseded 2026-10-21` note is present, (c) the code citation in the note is accurate at the candidate SHA.

## Verify the R6 manifest (`docs/reference/extraction-manifest.md`)
- Records: source file names (`arch.txt`/`mep.txt`), the four required fields (source name, SHA-256, tool, page/line convention).
- HONESTY (load-bearing): SHA-256 and tool must be marked **not recorded / owner action needed** — NOT fabricated. The manifest must state the extraction files are not committed (copyright) and that only paraphrase is stored. Verify no SHA-256 hash or tool name is invented.
- The page/line convention (line numbers of the extracted text, not PDF pages) is recorded.

## Gates
1. **Docs-only gate:** diff is only markdown under `docs/` + `docs/reference/` + `README`-free; no `.rs`/`.toml`/`Cargo.lock`.
2. **No office-P0 files:** no path under the office worktree; no office-worktree paths committed.
3. **Register/progress untouched this slice:** the register + progress deltas are NOT re-touched here (they were the prior slice on this branch) — this slice is only the R5 source docs + the R6 manifest + evidence.
