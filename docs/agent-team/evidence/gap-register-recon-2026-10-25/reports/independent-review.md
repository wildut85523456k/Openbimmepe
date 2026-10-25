# Independent review — docs reconciliation

Verdict: FAIL

Candidate: 88e8a894ad34cb8e12e0cfad6d6e02201a0da7ed
Base: b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be
Worktree: C:/Users/wilfr/hermes-first-task/wt-gap-register-recon
Branch supplied: docs/gap-register-reconciliation-2026-10-25
Review role: independent reviewer; no production or candidate document edits.

## Identity and scope

HEAD was verified as the candidate before and after inspection. No tracked working-tree changes were present. The pre-existing untracked review-contract.md was excluded from the candidate; SHA-256: ac414e1c83ff7de623cb1b19569ed1718780271f3ed9a02bbb5bb0c5ed08d37b. This report is the sole intentional repository write by the reviewer, interpreting the requested report as the exception to the read-only STOP condition.

Full base-to-candidate stable Git patch-id: 11518425654c9b076c1693edec7cd17b8b3bbd70. The two reconciled documents alone have patch-id 5377e296198e745ea80e02242211a91b682785ce. There is no uncommitted tracked patch. Any changed candidate requires affected checks to be rerun.

Read docs/agent-team/README.md in the project and at the candidate. AGENTS.md does not exist at the candidate root; tracked-tree lookup found no AGENTS.md. No domain-specific installed skill was needed for local Git/Rust/document inspection. The office-P0 worktree was not accessed. No commit, push, PR, source fix, or security-setting change was performed.

## Findings

### P1 — Unsupported closure of ceiling/face hosting

Location: docs/gap-progress-2026-10-23.md:96.

C5 is changed to Closed, but the exact base does not implement the hosting behavior required by register C5 (docs/gap-remediation-plan-2026-10-21.md:155): host resolution with level/offset derived from a ceiling or wall face.

Reproduction by source trace at b6ef0ee:
- crates/family/src/mep.rs:121-128 defines Host, including Ceiling and Face. Library definitions declare these preferences (sprinkler at library.rs:1224; hanger at :1395).
- apps/desktop/src/model_edit.rs:751-765 routes doors/windows to place_family_in_wall; all other families go to document.place_family(level, at, spec). The signature accepts only a possible wall, not a ceiling or face target. Optional connector snap/splice handling is not ceiling/face host resolution.
- crates/document/src/lib.rs:1102-1107 constructs Geometry::Placed { at, level }, with no ceiling/face host identity. The distinct wall-hosted path at :1110-1115 does retain a host.
- crates/document/src/spatial.rs:33-50 distinguishes level-placed family geometry from wall-hosted geometry; it has no ceiling/face family-host representation.

Thus a ceiling host declaration is not evidence of actual host placement, association, or host-derived level/offset. The retained '(not re-checked)' suffix further contradicts the new closure assertion. This is a documentation correctness failure, not a request to implement hosting in this docs task.

Required remedy: restore C5 to Open, or explicitly mark Partial with the narrow statement that family host metadata exists but ceiling/face host resolution remains unimplemented. Remove or accurately qualify the stale verification suffix. Do not claim Closed without independently reproducible host placement/association evidence at the pinned base.

### P3 — Family citation excludes the nineteenth entry

Locations: docs/gap-progress-2026-10-23.md:94; docs/gap-remediation-plan-2026-10-21.md:444 (C3 evidence row).

The count and names are correct, but library.rs:481-500 stops at pipe_reducer(). pipe_tee() is at :501 and the function closes at :503.

Required remedy: cite :481-503 (or :483-501) so the evidence range includes all nineteen entries.

### P3 — R4's missing-member enumeration remains incomplete

Location: docs/gap-remediation-plan-2026-10-21.md:69.

The corrected total of 20 is right, but the accompanying list of members absent from README still omits crates/commands. Cargo.toml includes it; README's five crate rows do not.

Required remedy: include commands in the missing-member enumeration. This does not invalidate the requested numeric correction.

## Per-claim gates

1. R4 count — PASS for the requested count. `git show b6ef0ee:Cargo.toml` lists nineteen crates and apps/desktop. Independent `git grep -o` counts returned 19 crate entries and 1 app entry. README has exactly the five claimed workspace crate rows (geom, family, place, keymap, arch); spikes/* is not a workspace member. See the separate minor enumeration finding above.

2. C3 family registry — PASS for count, names and document status. families() at library.rs:481-503 contains exactly: handle, door, window, arched_door, fan_coil, ahu, ceiling_diffuser, vav, pump, valve, wc, washbasin, sprinkler_head, luminaire, panel_board, hanger, pipe_elbow, pipe_reducer, pipe_tee. all() compiles this list at :505-507. Progress says Closed and the register evidence says nineteen. The register evidence table has no separate status column. Citation-range defect noted above. This gate establishes registry content, not Revit equivalence or every associated MEP workflow.

3. C5/C16/C18 — FAIL overall because C5 closure contradicts the placement implementation. C16 PASS for the contracted family-existence check: sprinkler_head() at library.rs:1182 has solids, FireProtection pipe connector, plan symbol and Category::Sprinkler. C18 PASS for the contracted family-existence check: hanger() at :1363 has rod/clamp solids and Host::Face metadata. Both occur in families(). Their existence does not independently establish every larger remediation requirement in the historical register: upright heads, wet/dry systems, or auto-placement of supports on structural hosts were not certified by this review. If Closed is meant to cover those full rows, narrow the status to the demonstrated slice or provide additional checks.

4. Easy-tier annotation and ancestry — PASS for the explicitly requested annotation/ancestry checks. Progress :138 says CLOSED and names b17a1ff, fccee7b, 91eb050, f84104a, 7afe07b. Each `git merge-base --is-ancestor <commit> b6ef0ee` returned 0. `git show --stat --oneline` confirms relevant IFC, shell and level-tab changes. Ancestry is not a fresh functional certification of every B2/B7-B16/B21-B24 behavior; workspace release execution remains BLOCKED below.

5. C28 — PASS for the actual statement and code; contract section locator is incorrect. The changed row is in progress section 4 at :127, not section 3.4. It says arc grids exist and cites grid_arc.rs / 01ddf1f. grid_arc.rs defines bulge-to-arc geometry, points and tests; commit 01ddf1f adds curved-grid document/IFC/UI support and is an ancestor of b6ef0ee (exit 0). No relocation is required to establish this fact.

6. Banners — PASS. SUPERSEDED banners are present in sections 3.1, 3.2 and 3.3 at :51, :67, :88. None references section 4.4. Full document read confirms no such dangling reference.

## Structure and docs-only gates

PASS — `git diff --name-status b6ef0ee 88e8a894` reports exactly four files: two modified reconciliation documents plus added coder-contract.md and coder-log.md. No production or test file is changed. `git diff b6ef0ee 88e8a894 -- Cargo.toml Cargo.lock crates apps` is empty. Working-tree source/manifest/README comparison to b6ef0ee also returned exit 0. Cargo.toml blob is 543cc1f88c61d9f8939b2a603c97b480a8d38473 at both commits; family library blob is 4db69825995b2d15c3420f2241a170a40ce135ce at both.

PASS — reviewed the complete two-document diff: headings/section ordering remain unchanged, table row delimiters are retained, and edits are local row replacements plus blockquote insertions; no broad reformatting. `git diff --check b6ef0ee 88e8a894` returned 0. C5's shortened historical row still lacks a separate evidence cell; this was pre-existing, not introduced by this patch.

## Independent execution gates

Environment observed: Windows/MSYS x86_64, uname MSYS_NT-10.0-26300 / Msys 3.6.6-1cdd4371.x86_64. rustc 1.99.1 (58cf8f9c3 2026-10-13); cargo 1.99.1 (5f94df478 2026-08-27). CPU model/RAM were not measured; no performance result is claimed.

- Formatting: FAIL. `cargo fmt --all -- --check` returned 1 with widespread existing Rust formatting differences, beginning at apps/desktop/src/app_settings.rs:14. All Rust code is unchanged from the base; this is not a docs-induced regression. No formatting was applied. Raw tool capture: C:/Users/wilfr/AppData/Local/hermes/profiles/bim-review/cache/terminal-output/out-1792910615-39988-efd0.log.
- Workspace release tests: BLOCKED by execution time limits, not reported as failed tests or a pass. `cargo test --workspace --release --locked --offline` was attempted with an isolated CARGO_TARGET_DIR. Initial attempt timed out at 180 seconds during compilation. A resumed attempt exceeded the tool service's 420-second limit despite a requested 600-second command allowance. A final explicitly located target attempt also timed out at 180 seconds during compilation. No completed test summary or passing count was produced. Partial logs: C:/Users/wilfr/AppData/Local/Temp/gap-recon-88e8a894-release.log and C:/Users/wilfr/AppData/Local/hermes/profiles/bim-review/cache/scratch/gap-recon-88e8a894-release-final.log. The shell exposed TMPDIR=/tmp rather than the advertised profile scratch; early target/log paths consequently resolved through Git Bash's temp mapping. These temporary artifacts were not deleted or treated as candidate changes.
- Clippy: PASS. `CARGO_TARGET_DIR="$TMPDIR/gap-recon-review-88e8a894-clippy" cargo clippy --workspace --all-targets --locked --offline -- -D warnings` returned 0. Log ends with openbim-desktop and `Finished dev profile ... in 2m 30s`. Log: C:/Users/wilfr/AppData/Local/Temp/gap-recon-88e8a894-clippy.log.
- Dependency audit: PASS for the available cached advisory database only. cargo-audit 0.22.2; `cargo audit --no-fetch` returned 0, loaded 1351 advisories and scanned 489 dependencies. It reported allowed unmaintained warnings for ttf-parser, zbus_names, zvariant, zvariant_derive and a yanked clipboard-win warning. Current-online-database freshness: BLOCKED/not established; no fresh database fetch was performed. This is not a claim that dependencies are warning-free.

Local Python/heredoc and execute_code attempts were denied by unattended-session approvals. They did not execute; no policy configuration was changed. Equivalent Git reads/counts and direct file reads were used where possible. No GUI test, benchmark, IFC round-trip or runtime hosting test was performed; none is fabricated here.

## Decision

Reject this candidate for documentation acceptance because C5 is incorrectly closed. Correct the status and citation/enumeration issues in a new docs candidate, then rerun the affected reconciliation checks. Neither the implementing agent's log nor commit ancestry supplies independent functional approval. This report does not approve merging or publishing.
