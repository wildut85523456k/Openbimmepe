# P0 review evidence — the from-scratch walking skeleton (21 Oct 2026)

Evidence file for PR #67 (`feat/gap-p0-walking-skeleton`): the plan's rows **B1–B6**, **B16** and
**B17** in `docs/gap-remediation-plan-2026-10-21.md`, held to the acceptance recorded in
`docs/revit-style-ui/08-gap-closure-plan-2026-10-21.md`. As with the per-discipline audits in this
directory, it is meant to be read against the code rather than taken as a summary of it.

The point of the record is that P0 is the gate every later package is tested through, so it was the
first work in this repository to go through the full loop: implementation → an independent review in
a separate profile with no access to the author's session → corrections → a delta review of the
rebase → coordinator decision → merge.

## What is in here

| File | What it is |
|---|---|
| `prompt.md` | The review contract (round 1): the gates, the scope, the rules the reviewer works under |
| `prompt2.md` | The re-review contract after the six corrections (round 2) |
| `prompt3.md` | The delta-review contract after the rebase onto `main` (round 3) |
| `report2.md` | **Round 2 verdict: 12 gates, all PASS.** The six round-1 findings (R1–R6) and their resolutions; "fit to push and merge as P0" |
| `report3.md` | **Round 3 verdict: 7 delta gates, all PASS**, including `git range-diff` evidence that the rebase changed no content beyond the two documented conflict resolutions |
| `pr-p0.md`, `pr-fix.md`, `pr-docs.md` | The three PR bodies as written at the time |
| `logs/review.log`, `logs/review2.log`, `logs/review3.log` | The reviewers' own transcripts: every command they ran and its real output |

The round-1 report (FAIL) was not preserved as a separate file; its six findings and their
resolutions are listed in `report2.md` under `R1–R6`.

The review contracts name the worktrees they worked in (`wt-gap-p0`, `wt-gap-docs`, `wt-printfix`,
`wt-gap-p0-review`). Those were removed once the branches were merged and verified as contained in
`main`; the SHAs in the tables below are what to check out to see the reviewed trees.

## The candidate, and what each round actually saw

| Round | Tree | Base | Verdict |
|---|---|---|---|
| 1 | `feat/gap-p0-walking-skeleton` @ `09150b5`…`bccdf5b` | `c90cbc8` | **FAIL** — six findings (R1–R6) |
| 2 | the same branch @ `27bd435` | `048036b` | **PASS**, 12 gates; R1–R6 corrected |
| 3 | the same branch @ `90d1e1a` (rebased), docs @ `1a8d174` | `main` @ `3f7df8b` | **PASS**, 7 delta gates |

Round 3's `range-diff` result is the reason the earlier gates were inherited rather than re-run from
scratch: four of the seven commits were byte-identical, and the three that differed changed only
context lines plus the two conflict resolutions in `sample()`'s field list (both keeping `main`'s
fields, and dropping a stray `show_about` that only existed on the AUTO-009 line).

The base of rounds 1–2 was wrong: `c90cbc8` was the tip of `feat/auto-008-obg-ds` when the work
started (that branch has since moved on, and `c90cbc8` is not on `main`). That is recorded in
`08-gap-closure-plan-2026-10-21.md` and is why the work was rebased before it was offered for merge.

## What the reviews did not cover

Stated plainly, because the record is only useful if its limits are on it:

- **The reviewer never ran the whole workspace** (CI's scope, `cargo test --workspace --release`); it
  ran the two affected packages per instruction. CI and the pre-push gate did run the full suite.
- **The debug-build `epaint` assert was not independently reproduced** — it was the author's claim
  about the print-dialog failure, not something the reviewer saw fail.
- **No line-by-line review covers the merged tree.** See below: the merge resolution and the clippy
  fix came after the last review, and were verified by clippy, tests and the gate only.

## What changed after the last review

Round 3 cleared `90d1e1a`. `main` then moved twice more (`fa28742`, carrying #66's class-2 work and
`72290de`), and the branch was merged with it as `38c7ec8`. Two files conflicted and were resolved by
hand:

- `apps/desktop/src/model_view.rs` — `main`'s `plan_bounds`/`EMPTY_PLAN_SPAN_M` (the same fallback
  this branch called `fit_box`) was kept, so the branch's duplicate helper and its test were dropped;
  `main`'s "Mechanical" tab label was kept. The branch's **position-based** tab mapping was moved
  inside `main`'s `plan_tabs`, and this is the substantive part: `main`'s `plan_level` still returned
  `ElementId(10)`/`(11)` for `View::Plan`/`View::UpperPlan`, so on `main` a wall drawn in a new
  project's first tab still referenced a level that did not exist and the IFC export failed. With the
  first two tabs taken by position, the tabs and `plan_level` agree. `main`'s test was restated for
  that behaviour and a renumbered project was added.
- `apps/desktop/src/print_dialog.rs` — `main`'s wording of the same test fix won, and the duplicated
  `output.textures_delta.clear()` that the automatic merge left behind was removed.

One further commit, `264508f` (`fix(clippy): the rename hint is a fixed string`): the pre-push gate's
`cargo clippy --workspace --all-targets -- -D warnings` rejected a `format!` of a fixed string in
`start_level_rename`; it is a literal now, behaviour unchanged. This is the one finding the reviews
could not have caught, because they did not run clippy.

Verification on the merged tree (this is what makes the merge defensible, and it is not a review):
`cargo clippy --workspace --all-targets -- -D warnings` clean; `openbim-desktop` **185 passed, 0
failed, 1 ignored**; `openbim-document` **161 passed** + 2 integration; the local pre-push gate
(tests + clippy + `cargo audit` on that exact SHA) reported `GATE PASS: 38c7ec8`.

## Where the work landed

- **#67** merged as `29722bf`; main is `f3ab5d5` after **#65** (the decisions and backlog mapping).
- **#64** (the print-dialog fix) was closed as superseded by `main`'s `72290de`, which fixed the same
  failure independently. Note that `report2.md`'s gate **G9** therefore covers a commit that the
  rebase later dropped from the branch: the behaviour is on `main`, but through `main`'s own commit,
  not this branch's.
- The P0 exit criteria in `08-gap-closure-plan-2026-10-21.md` (new project → levels → grids → typed
  walls → floor → door/window → stair → roof → ceiling → room → section → save/reopen → IFC export,
  plus the ribbon honesty test) are what the new `Desktop::window` end-to-end tests assert.

## How to re-run what the reviewers ran

```
git worktree add /tmp/p0 <sha>            # 27bd435, 90d1e1a
cd /tmp/p0
CARGO_TARGET_DIR=<shared target> CARGO_INCREMENTAL=0 cargo test -p openbim-desktop --bin openbim-desktop --jobs=2
CARGO_TARGET_DIR=<shared target> CARGO_INCREMENTAL=0 cargo test -p openbim-document --jobs=2
cargo clippy --workspace --all-targets -- -D warnings
```

Counts move as `main` grows; the round-2 and round-3 numbers (186/150 and 183/154) are in the reports
and the merged-tree numbers (185/161) are above. The reviewer's exact commands and output are in
`logs/`.
