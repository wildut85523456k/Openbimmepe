# Independent review contract — revit-alike gap-closure P0 (21 Oct 2026)

You are an independent reviewer. Nothing you are given here was written by you, and the author of the
candidate is not in this session. Judge the candidate on evidence you produce yourself.

## Identity

- Repository: `C:/Users/wilfr/hermes-first-task/revit-alike` (the main clone; do not edit it).
- **Candidate tree (read here): `C:/Users/wilfr/hermes-first-task/wt-gap-p0`** — a git worktree of the
  clone, branch `feat/gap-p0-walking-skeleton`, tip **`5d582ef`** (`bccdf5b`, `4ecfd5f`, `7f409a1`,
  `09150b5` before it), base **`c90cbc8`** = `origin/main` at the time of writing.
- Companion branch **`fix/print-dialog-texture-delta` @ `048036b`** (base `c90cbc8`, worktree
  `C:/Users/wilfr/hermes-first-task/wt-printfix`).
- Companion branch **`docs/gap-closure-2026-10-21` @ `0ead338`** (base `c90cbc8`, worktree
  `C:/Users/wilfr/hermes-first-task/wt-gap-docs`).
- The plan the candidate claims to implement is on the docs branch:
  `git -C C:/Users/wilfr/hermes-first-task/revit-alike show 0ead338:docs/revit-style-ui/08-gap-closure-plan-2026-10-21.md`
  (the evidence document is `07-gap-analysis-2026-10-21.md` in the same commit).

## What the owner decided on 21 Oct 2026 (these are the requirements)

1. Unbuilt ribbon buttons are shown **disabled**, not as "not implemented" messages.
2. Project storage = operation log + snapshots; autosave and crash recovery are the first
   `crates/oplog` consumer. **Not implemented in this candidate — it is P1/GAP-15. Say so if any
   text in the candidate claims otherwise.**
3. The first MEP family library is AHU, ceiling diffuser, VAV, pump, valve, WC/basin, sprinkler head,
   luminaire, panel board, hanger/support. (P2/GAP-18 — not in this candidate.)
4. Electrical calculations are exported to and imported from edesign-alike; the in-repo IEC 60364
   module is retired only after equivalence tests. (P5/GAP-25 — not in this candidate.)
5. End-to-end tests use a data hall for MEP and keep the office for architecture.
6. *New* creates Level 1 and Level 2.

The candidate claims to close only **P0: A1, A2, A3, A4, A5, A6, A7 and A8** of the plan's section 3,
plus one pre-existing failing test. Everything else in the plan is out of scope.

## In scope

- The five commits on `feat/gap-p0-walking-skeleton` (`git diff c90cbc8..5d582ef`).
- The one commit on `fix/print-dialog-texture-delta`.
- The four commits on `docs/gap-closure-2026-10-21`.

## Out of scope (do not report as findings)

- The Tier D features, the rest of the plan's P1–P6, and anything the plan itself defers.
- Style preferences not already established in the repository.
- The absence of GAP-15 (oplog autosave) from this candidate.

## Gates — report PASS / FAIL / BLOCKED for each, with evidence

**G1 — A1 (empty canvas).** The early return that made an empty plan ignore clicks is gone; the
default view box is real and finite; nothing else depended on the early return (check the whole
function, and that the tool branches are reached). The test that claims to prove it must actually
start from a new model and drive clicks.

**G2 — A2 (levels from the document).** In the plan path there must be no hard-coded level id
(`ElementId(10)` / `ElementId(11)`) or hard-coded level name deciding anything: the plan tabs,
`plan_level`, the plan export, the Project Browser's plan entries and the Properties "View Name".
A const fallback name is acceptable; a const deciding the *level* is not. A project with no level
must refuse a drawing tool. Look for remaining hard-coded ids anywhere in `apps/desktop/src` and
say which are in the plan path and which are not.

**G3 — A3 (two-stage Esc).** Through the real window path (`global_keys` then the viewport), the
first Esc must step back inside a tool that has work pending and the second must leave it. Check
`tool_has_pending_points` against every tool that keeps pending state in the viewport (a tool whose
pending state is missing from that list leaves the tool on the first Esc — find any).

**G4 — A4 (keys and keymap).** Ctrl+S, Ctrl+N, Ctrl+O and the typed shortcuts reach commands; typing
in a text field is never stolen; the Save As button exists, is wired and has an icon. Check the
matcher replay in `keymap_key` for correctness: a dead-end sequence must clear, a half-typed
sequence must do nothing, and a matched command must run once, not twice.

**G5 — A5 (options bar per tool).** No command inherits another's numbers. `command_defaults` must
cover every field the command arms used to set inline — diff the arms against the table and report
any value that was silently dropped (a command that no longer sets what it used to). Check the duct
and pipe paths still see their sizes before `choose_tool` runs.

**G6 — A6 (level authoring).** `place_level` refuses a duplicate name (any case) and a level closer
than `MIN_LEVEL_GAP_M`; `rename_level` and `delete_level` behave as documented (delete refuses a
level with elements on it, rename refuses a name in use); `Command::Rename` now accepts a level —
check this cannot rename something it should not, and that no test asserted the old refusal. The
options-bar UI must call them and report the outcome.

**G7 — A7 (ribbon honesty).** `IMPLEMENTED` must agree exactly with the arms of `run_command`:
extract both lists yourself and compare (report any id in one and not the other, in either
direction). The gate must prevent an unwired command from running. A disabled button must not be
clickable. The honesty test's exclusions (the file-dialog commands) must be justified and its
coverage stated. The `debug_assert` must actually be reachable for a listed command with no arm.

**G8 — A8 (whole window + end-to-end).** `Desktop::window` must be the same code the app ran before
(diff the moved body against `c90cbc8`; report any behavioural difference, including panel order and
the global-key/clipboard order). The `ribbon_hits` field must not exist in a production build and
must not affect rendering. The office test must exercise the journey it claims (new project, three
levels, a grid, four walls, a floor, a room, a section, save, reopen, IFC export) and its
assertions must be able to fail (name the assertion that would catch a regression in each step).

**G9 — the fix branch.** `048036b` must be test-only, must clear both texture deltas, and must not
change product code. Confirm the failure it fixes is a `debug_assert!` in epaint and therefore
invisible to `cargo test --release` (which is what CI runs).

**G10 — the docs branch.** On `0ead338`: every new backlog row has an acceptance criterion that does
not depend on a later phase's deliverable; one owner per deliverable across `05-backlog.md` and
`06-mep-schematic-automation.md`; no id is duplicated; no agreed decision row was rewritten (diff the
tables against `c90cbc8`); the companion `07`'s decision overlay reconciles its pre-decision text;
relative links resolve; line endings stayed CRLF.

**G11 — build and tests.** Reproduce, in the candidate tree:
`cargo test -p openbim-document` and
`cargo test -p openbim-desktop --bin openbim-desktop`
(the candidate's own claim is 150 + 2 pass for the document crate and 181 pass / 1 fail for the
desktop binary, the failure being `print_dialog::tests::settings_dialog_renders_in_both_themes_and_invalidates_stale_bytes`).
A workspace test run may be in flight in the shared target directory
(`C:/Users/wilfr/hermes-first-task/revit-alike/target`) — use
`CARGO_TARGET_DIR=C:/Users/wilfr/hermes-first-task/revit-alike/target CARGO_INCREMENTAL=0` and
`--jobs=2`, and if cargo blocks on the directory lock, wait or say so rather than rebuilding
elsewhere. Do not run the whole workspace. `cargo clippy --workspace --all-targets -- -D warnings`
is optional if time allows. Report the exact commands and their real output; do not paste a summary
in place of the result.

**G12 — claims.** For every factual claim in the five commit messages (counts, "verified", "tests
pass", what each test proves), say whether the evidence supports it. Report any overstatement.

## Constraints

- **Read-only.** Do not edit, add, remove or commit anything in any repository or worktree; do not
  `git checkout`, `git switch`, `git stash`, rebase, merge or push; do not touch `origin`. Running
  tests is allowed (they write to the target directory and to temporary files only).
- Judge the candidate, not the plan's scope. A correct implementation of a wrong requirement is a
  finding about the requirement, not a licence to redesign it.
- Distinguish "I ran it" from "I read it". Mark anything you did not run as read-only evidence.

## Output

Write your report to `C:/Users/wilfr/hermes-first-task/wt-gap-p0-review/report.md`:

1. Verdict per gate: PASS / FAIL / BLOCKED, one line of evidence each.
2. Findings: id, severity (blocker / major / minor / nit), `file:line`, what is wrong, why it
   matters, and the remedy. Findings that contradict a gate's PASS must be listed as that gate's
   failures.
3. What you ran: the exact commands and their real results.
4. What you did not verify, and why.
5. A one-paragraph overall verdict: is this candidate fit to be pushed and merged as P0?

Print the same verdict table to stdout when you finish.
