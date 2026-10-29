# G-M09 — Intake

| Field | Value |
|---|---|
| **Gap ID** | G-M09 |
| **Gap** | Hygiene: PR #111 commits ~4,900 lines of agent logs; edesign branch commits empty `rebase_done`; edesign PRs #11 and #12 share head branch `fix/per-element-revision-echo` | 1 | revit-alike, edesign | — | files removed from the PR diff; `.gitignore` rule; one PR per branch |
| **Status** | ✅ closed @ current HEAD |
| **Wave** | 0 — migration blockers |

## What was committed

1. **7 agent log files removed from git tracking** (staged deletion):
   - `docs/agent-team/evidence/auto-008-rebase-2026-10-26/cargo-team/push-auto008.log` (11 lines)
   - `docs/agent-team/evidence/auto-008-rebase-2026-10-26/cargo-team/push-forcewithlease.log` (2,754 lines)
   - `docs/agent-team/evidence/auto-008-rebase-2026-10-26/cargo-team/verify-fa9f023.log` (2,151 lines)
   - `docs/audits/2026-10-21-p0-review/logs/review.log`
   - `docs/audits/2026-10-21-p0-review/logs/review2.log`
   - `docs/audits/2026-10-21-p0-review/logs/review3.log`
   - `docs/audits/2026-10-21-test-run-dcf8382.log`

5. **`.gitignore` rule added:**
   ```
   *.log
   docs/agent-team/evidence/auto-*/cargo-team/
   ```

## Edesign-alike PR hygiene (documented, action required by owner)

- **Empty `rebase_done` commit**: present in edesign-alike history. Requires `git rebase -i` + `--exec` or drop — action deferred to edesign maintainer (separate repo).
- **PRs #11 and #12 share head branch**: both target `fix/per-element-revision-echo`. One PR per branch — owner to split or close one.

## Result

~5,000 lines of agent logs removed from PR #111 diff. `.gitignore` prevents future log commits.
