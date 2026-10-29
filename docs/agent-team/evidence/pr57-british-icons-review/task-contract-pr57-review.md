# Task acceptance contract — independent review of PR #57 (British BS 1363 icons)

**Workflow:** independent review -> coordinator decision -> user-approved merge.
**Role:** `bim-review` (independent reviewer). This file is the task; the profile supplies the role constraints.

Read `AGENTS.md` (if present) and `docs/agent-team/README.md` before starting. You are **READ-ONLY on production assets**: you may write your report under your evidence directory, but you must not modify the assets, scripts, JSON or docs you judge, and you must not push, merge, publish, schedule or delete files.

## 1. Identity — pin every result to these exact values

| Field | Value |
|---|---|
| Repository | `wildut85523456k/revit-alike` |
| Local clone | `C:/Users/wilfr/hermes-first-task/revit-alike` |
| PR | **#57** `fix(icons): replace European socket/switch with British BS 1363 style` |
| Base branch / SHA | `main` @ `840f709e1e754b79bfb6340cf8dd7a173b5d8d1d` |
| Candidate branch / SHA | `fix/british-electrical-icons-v2` @ **`67150218336b76ee48cf5c2ee77210c3c0e60337`** |
| Uncommitted patch | none — review the committed SHA only |
| Exact diff | `git diff 840f709 67150218` — 12 files, +70 / -79 |

A rebased or amended candidate is a new SHA; prior results do not carry over. Inspect the diff itself, not any summary.

## 2. What the change is

The first calculation profile is British practice (decision of 15 Oct 2026). PR #57 replaces the European (Schuko-style) electrical socket/switch command icons with **British BS 1363** artwork, and updates the icon pipeline and provenance docs to match.

Changed files (all under `assets/icons/`):

- `approved-3d/socket.png`, `approved-3d/switch.png` — approved 3D icon source (BS 1363 redraw)
- `rust/socket.png`, `rust/switch.png` — the transparent PNGs **embedded into the desktop app** via `apps/desktop/src/icons.rs` (`include_bytes!("../../../assets/icons/rust/<id>.png")`)
- `approved-3d/catalog-provenance.json` — provenance: hashes, replacement flags, retained IDs
- `generate.py`, `extract_catalog.py`, `isometric.py` — generation / extraction pipeline
- `README.md`, `README-v02.md`, `V0.2-REVIEW.md`, `APPROVED-3D-ASSETS.md` — documentation

No production Rust source is changed. The icons reach the app only through the `include_bytes!` embedding in `apps/desktop/src/icons.rs`.

## 3. In scope / out of scope

**In scope**
- The exact diff `840f709..67150218`.
- Correctness of the socket/switch artwork against BS 1363.
- Integrity of `catalog-provenance.json` (hashes, replacement flags, retained IDs).
- Reproducibility of the generation/extraction scripts against the committed PNGs.
- Accuracy of the four documentation files.
- The embedded-icon Rust test (`cargo test -p openbim-desktop --release`).

**Out of scope**
- Any other branch or PR (printing stack #48/#49, AUTO-008 #58, PRT-006 #50/#51).
- Re-deriving or challenging the British-practice decision itself.
- Redrawing or "improving" the icons.
- Any change to production source.

## 4. Acceptance items

| # | Acceptance item | Expected result | Evidence |
|---|---|---|---|
| A1 | Socket artwork is BS 1363 | Three rectangular slots: earth top-centre **vertical**; live + neutral bottom **horizontal**; no Schuko/Type-F round-pin or Euro geometry remains | Image inspection at 24/32/48 px on light and dark backgrounds |
| A2 | Switch artwork | Rectangular faceplate with inset rocker and LED indicator; consistent with A1 style | Same as A1 |
| A3 | Embedded assets valid | `cargo test -p openbim-desktop --release` passes (embedded icon validity/uniqueness); `rust/socket.png` and `rust/switch.png` are valid RGBA PNGs, correct dimensions, non-empty | Command + exit code |
| A4 | Provenance integrity | `catalog-provenance.json` recorded hashes match the actual committed PNG bytes; socket/switch marked as replacements; IDs retained | Recompute hashes independently |
| A5 | Reproducibility | `generate.py` / `make_rust_icons.py` / `extract_catalog.py` reproduce the committed PNGs (byte-identical, or a documented and justified difference); extraction preserves replacement bytes, IDs and hashes | Re-run scripts, compare bytes |
| A6 | Documentation accuracy | The four docs describe British icons, mark Qt/gallery instructions as historical, and contain no stale European-icon or contradictory provenance claims | Read against the code/artwork |
| A7 | Mechanical gates | `cargo clippy --workspace --all-targets -- -D warnings` clean; touched files fmt-clean; no production Rust source in the diff | Commands + exit codes |

## 5. Gates

- Use **PASS / FAIL / BLOCKED** per item. A missing tool or unavailable check is **BLOCKED**, never a pass.
- Every gate is tied to candidate SHA `67150218`.
- Do not fabricate commands, screenshots, measurements or "Revit equivalence". Report what ran, what did not, and why.
- A prior `bim-review` (mentioned in the PR body: Gates 1,3,4,5 PASS; R1/R2/R3 addressed) is **not** a substitute — verify independently on this SHA.

## 6. Evidence and output

Write your report to:

`docs/agent-team/evidence/pr57-british-icons-review/bim-review/verification-report.md`

Include: profile, timestamp, exact candidate SHA, actual commands and exit codes, findings with severity + file/line, raw evidence paths, and an overall PASS/FAIL/BLOCKED verdict with the unmet requirements listed.

Keep large binary captures and machine-specific logs out of commits.

## 7. Constraints

- READ-ONLY on the judged artifacts. Reviewers may not fix the code they judge.
- No push, merge, publish, schedule, message or delete without explicit user approval.
- No unattended loops; one bounded run.
- Keep other profiles, tokens and security settings unchanged.
- Scope all inspection to the repository and explicitly supplied references.

## 8. Decision (coordinator fills after the review)

- Code reviewer decision: **PASS** — independent `bim-review` run on candidate SHA `67150218336b76ee48cf5c2ee77210c3c0e60337`; all seven acceptance items A1–A7 PASS. Report: `docs/agent-team/evidence/pr57-british-icons-review/bim-review/verification-report.md` (2026-10-21T08:27:36+0700). Coordinator spot-checked independently: the diff is 12 files under `assets/icons/` (+78/−81) with 0 `.rs` files, and the reviewer's working tree was left clean.

- Coordinator disposition: **ACCEPT the PASS. No merge action taken or required — PR #57 was already merged.** `gh pr view 57` reports `state: MERGED`, `mergedAt: 2026-10-21T00:20:07Z`, `mergeCommit: dcf838297246d1adb0093143ad86a393af8e0303`, which is the current `origin/main` tip. That merge commit's parents are exactly the reviewed base `840f709e1e754b79bfb6340cf8dd7a173b5d8d1d` and candidate `67150218336b76ee48cf5c2ee77210c3c0e60337`, and `git diff 67150218 dcf8382 -- assets/icons` is empty: `main` carries byte-identical icon bytes and no other commit touched `assets/icons`. The verdict therefore binds the merged tree. **Caveat recorded:** the review ran ~40 minutes *after* the merge, so it is retrospective verification rather than the intended pre-merge gate; this does not weaken the verdict because the reviewed SHA is the merged candidate and the merged tree is unchanged in scope, but the intended sequence (review → decision → merge) was not followed in that order.
  Follow-ups (non-blocking, not part of this PR): **F3** — workspace-wide `cargo fmt --check` fails on pre-existing `.rs` files outside the PR's diff; recommend a separate cleanup task. **I1** — the reviewer installed `numpy` into the `bim-review` profile venv to run `make_rust_icons.py`; reversible profile tooling, no production or secret file touched.

- User merge/push approval (separate): the merge was completed by the user on 2026-10-21T00:20:07Z before this review; no further push, merge or remote write was authorized or performed by the coordinator. The evidence directory remains untracked (`?? docs/agent-team/evidence/`); committing it is a separate user-approved action.
