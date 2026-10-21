# Independent delta review — rebase onto main and docs de-duplication (21 Oct 2026)

Candidate: wt-gap-p0, branch feat/gap-p0-walking-skeleton, tip 90d1e1a. Fix branch: wt-printfix, fix/print-dialog-texture-delta, tip 02ee217. Docs branch: wt-gap-docs, docs/gap-closure-2026-10-21, tip 1a8d174. Base main = 3f7df8b.

## Gate verdicts

| Gate | Verdict | Evidence |
|---|---|---|
| D1 rebase content-neutral | PASS | range-diff shows 4 commits identical (=), 3 differing only in context lines and the two documented conflict resolutions (show_about removal, GenerateSchematic context, level_rename additions). No other differences. |
| D2 conflict resolutions | PASS | sample() field list is main's list plus key_typed/level_rename_id/level_rename_name; show_about absent everywhere (grep returned no matches); struct definition matches. |
| D3 main's work intact | PASS | Command::GenerateSchematic present; crates/schematic in members; place_level/rename_level/delete_level present; diff vs origin/main is exactly the author's 14 files. |
| D4 the fix branch | PASS | range-diff commit 1 shows '=' (byte-identical content); test-only. |
| D5 the docs de-duplication | PASS | 07 deleted; 08 cites only valid B-rows (B1-B9,B10-B14,B16,B17,B20,B26 all exist in plan); CRLF preserved; links resolve. |
| D6 tests on the rebased tip | PASS | openbim-desktop 183 passed, 0 failed, 1 ignored; openbim-document 154 + 2 doc tests passed. |
| D7 inheritance | PASS | G1-G9, G11-G12 inherited (subject commits byte-identical); G10 re-examined via D5. |

## Findings

None blocking. Minor: none.

## Commands run and real results

- `git range-diff c90cbc8..27bd435 origin/main..90d1e1a` — 4 '=' commits, 3 '!' with only context-line and documented conflict differences.
- `grep -rn show_about` in wt-gap-p0 — no matches.
- `grep GenerateSchematic` and Cargo.toml members — intact.
- `git diff --stat origin/main..90d1e1a` — 14 files, author's intended changes.
- docs: grep B-rows all valid against plan; CRLF line terminators confirmed.
- `cargo test -p openbim-desktop --bin openbim-desktop --jobs=2` (CARGO_TARGET_DIR=.../target, CARGO_INCREMENTAL=0): 183 passed, 0 failed, 1 ignored (154.62s).
- `cargo test -p openbim-document --jobs=2`: 154 passed + 2 doc tests.

## Not verified

- Whole workspace not run (per constraints).
- Debug-build epaint assert not independently reproduced.

## Verdict

On the strength of the earlier PASS on all 12 gates plus this delta (content-neutral rebase, correct conflict resolutions, main's work intact, fix branch byte-identical, docs de-duplicated, tests green), the rebased 90d1e1a and 1a8d174 may be pushed as P0.
