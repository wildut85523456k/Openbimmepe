# Independent review — P0 gap status reconciliation (gap-p0-2026-10-25)

You are an **independent** reviewer. You did not write the candidate and must not edit it. Read-only.

## Identity

- Candidate branch: `docs/gap-p0-production-backlog-2026-10-25`
- Candidate SHA: `003818b462138f9929784d26d5f9394c7981060b`
- Base branch / SHA: `main` @ `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`
- Worktree: `C:/Users/wilfr/hermes-first-task/wt-gap-p0-backlog` (HEAD == candidate SHA; tree clean)
- Merge-base check: `git merge-base --is-ancestor b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be HEAD` must be silent;
  `git merge-base b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be HEAD` must equal `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`.
- Diff stat: run `git diff --stat b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be...HEAD`.

## What to review

The candidate is ONE docs deliverable plus its evidence:

- Deliverable: `docs/gap-p0-production-backlog-2026-10-25.md`
- Evidence: `docs/agent-team/evidence/gap-p0-2026-10-25/` (master contract, four area contracts,
  four reports)

Read the deliverable and the four reports in full. The candidate claims (all read-only, at the base
SHA):

1. **Headline** — all 16 code rows of the register's §5.1 "Easy" tier (B2, B7–B11, B12–B16,
   B21–B24) are **CLOSED** at `main` @ `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`, closed by `b17a1ff`, `fccee7b`, `91eb050`,
   `f84104a`, `7afe07b` (all ancestors).
2. **Staleness** — the register was verified at `dcf8382`, **112 commits** behind `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`; the
   register file did not exist at `dcf8382` (added by `f7436e7`).
3. **Docs rows R4/R5/R6** are genuinely open (crate table has 6 of 21 members; three app claims
   contradicted by code; no extraction manifest).
4. **14 residuals (S-C1…S-C14)** and **11 doc splices (S-D1…S-D11)** are characterized as
   coverage/follow-on, not open register rows.

## Acceptance items (report PASS / FAIL / BLOCKED for each, with evidence)

1. **Identity confirmed** — candidate SHA, base SHA, merge-base, worktree, diff stat.
2. **Headline holds** — independently re-read at least 3 of the 16 "CLOSED" rows against the code at
   `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be` (e.g. B7 `apps/desktop/src/export.rs:272`, B2 `apps/desktop/src/model_view.rs:24-29`,
   B16 `apps/desktop/src/ribbon.rs:50`, B21 `crates/document/src/rooms.rs:128-139`,
   B12 `apps/desktop/src/compute_ui.rs:80`) and confirm each is closed. Report any row that is NOT.
3. **Staleness holds** — `git rev-list --count dcf8382..b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be` and the register-absent-at-`dcf8382`
   check.
4. **Citations resolve** — every `file:line` the deliverable cites resolves at `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be` and supports
   the claim (spot-check at least 8 across the doc; flag any that moved or is wrong).
5. **Residuals scoped** — each S-C* references an existing register row (no new gap ID) and is
   genuinely a coverage/follow-on item, not an unacknowledged open row.
6. **Doc splices accurate** — each S-D* target `file:line` resolves and the claimed current text
   matches.
7. **Cross-document consistency** — report paths exist; the doc cites existing register/progress row
   IDs; no contradiction with the register or progress report's own scope.
8. **Coordination section** correctly describes the office-P0 worktree (`D:/Codex/Revit-alike-P0`,
   branch `ci/private-dependency-auth`) and does not duplicate it.
9. **Hygiene** — relative links resolve; no first-person/session phrasing; no trailing whitespace;
   no CRLF blobs.
10. **No fabrication** — any citation you cannot confirm is BLOCKED with the reason, never guessed.

## Constraints

- READ-ONLY. Do not edit, commit, branch, fetch or push anything. Do not build (no `cargo`).
- Pass `C:/...` paths to native tools (git, rg).
- **Write your report EARLY** (before long checks) to:
  `docs/agent-team/evidence/gap-p0-2026-10-25/reports/independent-review.md`
- Report structure: (1) Identity; (2) Gate results with commands + exit codes; (3) Acceptance table
  (item / expected / evidence / verdict); (4) Findings by severity; (5) What could not be verified
  (BLOCKED with reason); (6) Overall verdict PASS / FAIL / BLOCKED.

## Handoff

Report path + the overall verdict. No push, no merge.
