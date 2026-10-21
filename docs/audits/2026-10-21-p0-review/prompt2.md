# Independent re-review contract — revit-alike gap-closure P0, corrected candidate (21 Oct 2026)

You are an independent reviewer. A previous review of this work returned FAIL with six findings (R1–R6);
the author has since corrected them. Your job is to judge the corrected candidate on evidence you
produce yourself, and to say whether the corrections are real and complete. Nothing here was written
by you, and the author is not in this session.

## Identity

- Repository: `C:/Users/wilfr/hermes-first-task/revit-alike` (the main clone; do not edit it).
- **Candidate tree (read here): `C:/Users/wilfr/hermes-first-task/wt-gap-p0`** — a worktree of the
  clone, branch `feat/gap-p0-walking-skeleton`, tip **`27bd435`**, based on **`048036b`** (the
  companion test fix, which is now part of this branch's history; `c90cbc8` is the original base and
  the tip of `origin/main` at the time of writing).
  - `27bd435` P0 corrections: the review's six findings (the corrections under review)
  - `0e7c2f2`, `b125b3f`, `03a42e1`, `323cc46`, `26d3b3c` — the five original P0 commits, rebased
  - `048036b` fix(test): the print dialog's texture deltas (the companion fix, reviewed PASS before)
- Companion docs branch: **`docs/gap-closure-2026-10-21` @ `6243f28`** (worktree
  `C:/Users/wilfr/hermes-first-task/wt-gap-docs`), two commits on `c90cbc8`: `0ead338` (the gap
  analysis 07, the plan 08, the backlog rows) and `6243f28` (the R6 corrections).
- The plan the candidate implements: `git -C C:/Users/wilfr/hermes-first-task/revit-alike show 6243f28:docs/revit-style-ui/08-gap-closure-plan-2026-10-21.md`
  (evidence document: `07-gap-analysis-2026-10-21.md` in the same commit).

## What the previous review found (the author's summary of it — verify, do not trust)

| Id | Finding | Claimed correction |
|---|---|---|
| R1 | G2 FAIL: the Project Browser still filtered levels by `ElementId(10)/(11)`, `level_view_name` did not resolve `View::Level(id)`, and `sheet_draw`/`view3d` selected level 10 | Browser adds the levels after the tree's own two, by position; search matches the shown name; `level_view_name` resolves `View::Level`; `sheet_draw::plan_level_id` resolves plan keys from the document; the 3D drop falls back to the document's lowest level; `view_ui` documents `p10`/`p11` as view names, with the key scheme deliberately unchanged (it would invalidate stored view settings and sheet references) |
| R2 | G3 FAIL: `tool_has_pending_points` omitted `edit_ref`, `opening_host`, `view_corner`, `view_edit`; Beam, Truss and a one-point Duct/Pipe run left the tool on the first Esc | All four states added; Beam, Truss and the one-point run step back first; tested for each |
| R3 | G5 FAIL: Beam, Column, Truss and the footing commands still inherited shared numbers | The framing tools and both footing commands start from their own numbers; the Foundation button initialises everything an isolated pad consumes |
| R4 | G6 FAIL: the rename field rebuilt its draft every frame, so typing was lost | The draft lives in the app (`start_level_rename`/`commit_level_rename`/`cancel_level_rename`); the field stays open on a refused name; tested over frames |
| R5 | G8 FAIL: `clipboard_events` moved in the input order, and the office test did not exercise the whole window | Clipboard keys keep their original place; the journey chooses every tool by clicking its ribbon button, aims its pointer clicks with the plan's own transform, and compares every element's identity, geometry and name plus the room's area on reopen |
| R6 | G10 FAIL: four backlog rows had acceptance that depended on another row's deliverable, two in a cycle | IFC-001 no longer requires IFC-002; MEP-002 is hydraulic/airflow only and MEP-005 owns the electrical import without depending on SCH-004 (P2); SCH-004 depends on MEP-005; TEST-001 is the harness and the office (P0) and the data-hall chain is the new MEP-004 (P3) |

The previous review also recorded G11 FAIL (the desktop suite had one failing test) — the companion
fix `048036b` is now part of this branch, so the suite should be green. G12 FAIL was about
overstated coverage in the commit messages; judge the new commit messages too.

## Gates — report PASS / FAIL / BLOCKED for each, with evidence

**G1 — A1 (empty canvas).** As before: the early return is gone, the default box is real, nothing
else depended on it, and the test starts from a new model and drives clicks.

**G2 — A2 (levels from the document).** Verify R1 yourself: search the plan path for any remaining
decision made by a hard-coded level id or name (`ElementId(10)`, `ElementId(11)`, `"Level 1"`,
`"Level 2"`, `"p10"`, `"p11"`) and classify each occurrence as (a) a view key/name, (b) test
fixture, (c) a real level selection. Say explicitly whether any occurrence still decides a level.
Check the browser's added-plans rule against a project whose levels have other ids, and the search
by a renamed level's name.

**G3 — A3 (two-stage Esc).** Verify R2: for every tool that keeps pending state in the viewport,
the first Esc must step back and the second must leave. Enumerate the tools and the states yourself
from `model_view.rs`, `mep_tool.rs`, `section_ui.rs`, `view_tools.rs` and `view_notes.rs`, and name
any tool that still leaves on the first Esc with work pending.

**G4 — A4 (keys and keymap).** As before, plus: the ordering of `clipboard_events` relative to
undo/redo and Ctrl+P must match `c90cbc8`'s order exactly.

**G5 — A5 (options bar per tool).** Verify R3 by enumerating, for every command arm, the shared
fields its placement path reads, and checking each is either initialised by `command_defaults` or
not shared. Name any remaining leak.

**G6 — A6 (level authoring).** Verify R4: the draft is held by the app and survives frames; a
refused name keeps the field open; cancelling changes nothing. Also re-check `place_level`,
`rename_level`, `delete_level` and `Command::Rename` as before.

**G7 — A7 (ribbon honesty).** As before: extract `IMPLEMENTED` and the dispatch arms yourself and
compare in both directions; the gate must prevent an unwired command from running; the disabled
button must not be clickable.

**G8 — A8 (whole window + end-to-end).** Verify R5: `Desktop::window` must match `c90cbc8`'s body
modulo the documented changes; the test-only `ribbon_hits` and `plan_transform` must not exist in a
production build and must not change rendering; the office test must actually click ribbon buttons
(and fail if the click does not start the tool), and its reopen assertions must be able to fail.
Name the assertion that catches a regression in each step.

**G9 — the fix.** `048036b` is test-only, clears both deltas, and the failure it fixes is an epaint
`debug_assert!` invisible to `cargo test --release`.

**G10 — the docs branch.** Verify R6: no row's acceptance depends on another row's deliverable; no
phase inversion (a P0/P1 row must not depend on a P2/P3 row); one owner per deliverable; ids unique;
agreed decision rows unchanged from `c90cbc8`; the plan's mapping table and phase lists agree with
the backlog; links resolve; CRLF preserved in the worktree.

**G11 — build and tests.** Reproduce, in the candidate tree, with
`CARGO_TARGET_DIR=C:/Users/wilfr/hermes-first-task/revit-alike/target CARGO_INCREMENTAL=0` and
`--jobs=2`:
`cargo test -p openbim-desktop --bin openbim-desktop` (the author's claim: 186 passed, 0 failed,
1 ignored, 187 total — the print dialog test now passes because `048036b` is in the branch) and
`cargo test -p openbim-document` (150 + 2 passed). Note: the shared target directory may hold
artifacts from another worktree; if a build looks stale, `cargo clean -p <crate>` before rebuilding.
Do not run the whole workspace (CI does that in release).

**G12 — claims.** For each of the six P0 commit messages and the docs commit messages, say whether
the evidence supports every factual claim (counts, "verified", what each test proves). Report any
overstatement.

## Constraints

- **Read-only.** Do not edit, add, remove or commit anything in any repository or worktree; do not
  `git checkout`, `git switch`, `git stash`, rebase, merge or push; do not touch `origin`. Running
  tests is allowed (they write to the target directory and to temporary files only).
- Judge the candidate, not the plan's scope. Deliberate exclusions the author states (the view-key
  scheme, the Tier D work, GAP-15) are out of scope unless they make an accepted gate untrue.
- Distinguish "I ran it" from "I read it".

## Output

Write your report to `C:/Users/wilfr/hermes-first-task/wt-gap-p0-review/report2.md`:

1. Verdict per gate: PASS / FAIL / BLOCKED, one line of evidence each.
2. For each of R1–R6: corrected / partly corrected / not corrected, with `file:line` evidence.
3. New findings: id, severity, `file:line`, what is wrong, why it matters, the remedy.
4. What you ran: exact commands and their real results.
5. What you did not verify, and why.
6. A one-paragraph verdict: is this candidate fit to be pushed and merged as P0?

Print the verdict table and the R1–R6 table to stdout when you finish, and write the report file
early (before you run out of tool iterations) so it exists even if you are cut short.
