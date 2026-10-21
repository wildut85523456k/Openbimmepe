# Independent delta review — the rebase and the docs de-duplication (21 Oct 2026)

You are an independent reviewer. A full review of this work already ran and returned **PASS on all 12
gates** (its report is `C:/Users/wilfr/hermes-first-task/wt-gap-p0-review/report2.md`), on candidate
`27bd435` and docs branch `6243f28`. Since then the work was **rebased onto `main`** and the docs
branch was **re-scoped**. This review covers only that delta, and then states explicitly which of the
earlier gates it inherits unchanged.

Nothing here was written by you; the author is not in this session.

## What changed, in the author's words (verify, do not trust)

1. **The base was wrong.** The work started from `c90cbc8`, which is the tip of
   `feat/auto-008-obg-ds` (AUTO-008 + AUTO-009), **not** `main`. `main` is `3f7df8b` and already
   carries the gap analysis, the three audits, the `verify-*.md` reports and
   `docs/gap-remediation-plan-2026-10-21.md` with rows B1–B26.
2. **All three branches were rebased onto `main`** with
   `git rebase --onto origin/main c90cbc8 <branch>`:
   - `fix/print-dialog-texture-delta` `048036b` → **`02ee217`**
   - `docs/gap-closure-2026-10-21` `6243f28` → **`1a8d174`**
   - `feat/gap-p0-walking-skeleton` `27bd435` → **`90d1e1a`**
3. **Two commits needed conflict resolution**, both in the `Desktop::sample()` field list of
   `apps/desktop/src/main.rs`: the author kept **main's** field list and added the new fields
   (`key_typed`, `level_rename_id`, `level_rename_name`). A first attempt wrongly kept
   `show_about: false`, a field that exists only on the AUTO-009 line; it was caught by the compiler
   and removed inside the same commit.
4. **The docs branch was re-scoped**: `07-gap-analysis-2026-10-21.md` deleted (it duplicated
   `docs/gap-analysis-2026-10-21.md` on main, and its provenance note was false); `08` rewritten as
   *the owner's decisions and the backlog mapping*, with the plan's B-rows as a middle column; the
   README's document list repointed at the two documents on main.

The author's claim is that the rebase changed **no content**: `git range-diff c90cbc8..27bd435
origin/main..90d1e1a` shows 4 of 7 commits identical and 3 differing only in context lines and the
two conflict resolutions above. The author also claims the rebased tip is green: desktop bin
**183 passed, 0 failed, 1 ignored**; `openbim-document` **154 + 2 passed**.

## Trees

- Repository `C:/Users/wilfr/hermes-first-task/revit-alike` (do not edit it). `main` is `3f7df8b`.
- Candidate worktree (read here): `C:/Users/wilfr/hermes-first-task/wt-gap-p0`, branch
  `feat/gap-p0-walking-skeleton` at `90d1e1a`.
- Fix worktree: `C:/Users/wilfr/hermes-first-task/wt-printfix`, branch
  `fix/print-dialog-texture-delta` at `02ee217`.
- Docs worktree: `C:/Users/wilfr/hermes-first-task/wt-gap-docs`, branch
  `docs/gap-closure-2026-10-21` at `1a8d174`.
- The earlier review's report: `C:/Users/wilfr/hermes-first-task/wt-gap-p0-review/report2.md`
  (read it — the gates it passed are the ones you may inherit).

## Gates for this delta review

**D1 — the rebase is content-neutral.** Reproduce
`git -C <clone> range-diff c90cbc8..27bd435 origin/main..90d1e1a` and account for **every** difference
it reports: each must be either (a) a context line that differs because `main` moved, or (b) one of the
two documented conflict resolutions. Report any difference that is neither. For each of the three
commits that differ, say what the difference is in one sentence.

**D2 — the conflict resolutions are correct.** In `40d89b6` and `90d1e1a`, the `sample()` field list
must be `main`'s list plus the new fields, with no field that `main`'s `Desktop` does not have
(check the struct definition in the same commit) and no field silently dropped from `main`'s list.
Confirm `show_about` (and any other AUTO-009-only symbol) appears nowhere in the rebased tree, and
that `apps/desktop/src/main.rs` is otherwise `main`'s file plus the author's changes.

**D3 — main's own work survives.** In the rebased tree: `Command::GenerateSchematic` and the
schematic crates (`crates/schematic`, `apps/desktop/src/…` uses, `Cargo.toml`'s member list) are
intact; `crates/document/src/lib.rs` has both main's additions and the author's
`place_level`/`rename_level`/`delete_level`; nothing of main's was reverted to `c90cbc8`'s version.
Diff the rebased tree against `main` (`git diff origin/main..90d1e1a`) and say whether the result is
exactly the author's intended changes.

**D4 — the fix branch.** `02ee217` must be byte-identical in content to the reviewed `048036b`
(the range-diff says `=`) and still test-only.

**D5 — the docs de-duplication.** On `1a8d174`:
- no file duplicating `docs/gap-analysis-2026-10-21.md` or `docs/gap-remediation-plan-2026-10-21.md`
  remains, and nothing claims those are untracked;
- `08`'s mapping table cites only row ids that exist in main's plan (verify each `B…` against
  `docs/gap-remediation-plan-2026-10-21.md`), and its "no backlog row of its own" note is consistent
  with `05-backlog.md`;
- the four acceptance cycles fixed earlier are still fixed, and no agreed decision row in
  `05-backlog.md` or `README.md` was rewritten relative to `main`;
- relative links in the changed documents resolve; CRLF is preserved in the worktree files.

**D6 — the gates on the rebased tip.** Reproduce, in `wt-gap-p0` with
`CARGO_TARGET_DIR=C:/Users/wilfr/hermes-first-task/revit-alike/target CARGO_INCREMENTAL=0` and
`--jobs=2`: `cargo test -p openbim-desktop --bin openbim-desktop` and
`cargo test -p openbim-document`. Report the real numbers. (Do not run the whole workspace.)

**D7 — inheritance.** State which gates of `report2.md` (G1–G12) you inherit unchanged and why
(the commit content they judged is byte-identical), and which you re-examined. Do not inherit a gate
whose subject the rebase touched (`apps/desktop/src/main.rs`'s `sample()` and the docs branch).

## Constraints

- **Read-only.** No edits, no `git checkout`/`switch`/`stash`, no rebase, merge or push, no `origin`
  writes. Running tests is allowed.
- Distinguish "I ran it" from "I read it".
- This is a delta review: do not re-litigate the P0 design or the plan's scope.

## Output

Write your report to `C:/Users/wilfr/hermes-first-task/wt-gap-p0-review/report3.md`, and print the
same verdict table to stdout when you finish:

| Gate | Verdict | Evidence |
|---|---|---|
| D1 rebase content-neutral | PASS/FAIL/BLOCKED | … |
| D2 conflict resolutions | … | … |
| D3 main's work intact | … | … |
| D4 the fix branch | … | … |
| D5 the docs de-duplication | … | … |
| D6 tests on the rebased tip | … | … |
| D7 inheritance | … | … |

Then: findings (id, severity, `file:line`, remedy), the exact commands you ran with their real
results, what you did not verify, and one paragraph saying whether the rebased `90d1e1a` and
`1a8d174` may be pushed as P0 on the strength of the earlier PASS plus this delta.
