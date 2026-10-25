# Independent re-review — docs reconciliation

Verdict: PASS for the bounded documentation reconciliation contract.

Candidate: `6e98ae91c979d4fb3eb08860739a6b0582fd5eb7`
Base: `b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be`
Prior reviewed candidate: `88e8a894ad34cb8e12e0cfad6d6e02201a0da7ed`
Branch: `docs/gap-register-reconciliation-2026-10-25`
Worktree: `C:/Users/wilfr/hermes-first-task/wt-gap-register-recon`
Role: independent Rust reviewer, not implementer.

This PASS resolves the three prior documentation findings and re-confirms the prior scoped documentation claims. It is not an all-green engineering gate, full feature-closure certification, Revit-equivalence claim, or authorization to merge/push. Separate execution gates below remain FAIL/BLOCKED where indicated.

## Identity and review boundaries

HEAD and branch matched the contract. `git diff --exit-code HEAD` returned 0 before the report write: no uncommitted tracked patch. The pre-existing untracked `docs/agent-team/evidence/gap-register-recon-2026-10-25/re-review-contract.md` is outside the committed candidate; its SHA-256 is `e20c108058f6a40e7092347c5f83c84b4c43e4885e156761367163a36747a191`.

Base-to-candidate stable Git patch-id: `da807fded3be4585f891c96bb9bf461c9515db73`.
Two reconciled documents' stable Git patch-id: `25889a4f1b9105ebddd9f37e60c4bbca1c11d2fb`.

Reviewed the actual base-to-candidate and prior-candidate-to-candidate diffs, not just the coder log. Read the project and candidate team README. No tracked AGENTS.md was found. No domain-specific skill was necessary for these local Git, Rust-source and Markdown checks.

The contract simultaneously requests this report and prohibits file edits. Clarification was attempted but was undeliverable in single-query mode. Consistent with the prior review's interpretation, this newly created report is the sole intentional repository-write exception. No existing candidate document, production source or test was edited. Build outputs and raw logs were isolated in this profile's scratch directory. The office-P0 worktree was not accessed. No commit, push, PR, merge, message or recurring job was performed.

## Per-claim results

### 1. R4 workspace and missing-member enumeration — PASS

Location: `docs/gap-remediation-plan-2026-10-21.md:69`.

`Cargo.toml:3` has 20 members: 19 crates and `apps/desktop`. `README.md:8-12` lists five workspace members: geom, family, place, keymap and arch. The separate spikes row is not a workspace member.

Independent Git extraction and `comm -23` of sorted manifest/README members returned exactly the following missing set:

- apps/desktop
- crates/automation
- crates/calc
- crates/commands
- crates/document
- crates/ifc
- crates/oplog
- crates/photo
- crates/route
- crates/schematic
- crates/sheets
- crates/sketch
- crates/view
- crates/viewport
- crates/viewport-egui

The R4 enumeration matches that set; `commands` is now present. Row-local extraction/count returned 15. Manifest/README counts returned 20, 19 crate members and 5 listed members. A preliminary count over the whole register returned 20 matching tokens, but was not a missing-member count and was discarded in favor of the row-local check.

### 2. C3 registry, status and both citations — PASS

Locations: progress `:94`; register §8 evidence `:444`.

`crates/family/src/library.rs:481-503` contains 19 registry entries: handle, door, window, arched_door, fan_coil, ahu, ceiling_diffuser, vav, pump, valve, wc, washbasin, sprinkler_head, luminaire, panel_board, hanger, pipe_elbow, pipe_reducer, pipe_tee. `pipe_tee()` is at :501; both corrected citation ranges include it and the closing brace. Independent bounded function extraction/count returned 19 entries. `all()` compiles this registry at :505-507.

Progress §3.3 says Closed; the register evidence row names all 19 families. The register evidence table has no separate status column. This verifies registry content and the contracted status reconciliation, not every MEP workflow.

### 3. C5 restored to Open — PASS; prior P1 resolved

Location: progress `:96`.

The row now says `**Open**`; the stale `(not re-checked)` suffix is gone. Source trace independently rechecked:

- `apps/desktop/src/model_edit.rs:751-765`: `family_click` sends doors/windows to `place_family_in_wall`; other families use connector snap/splice handling or `place_family(level, at, spec)`. It has no ceiling/face host target.
- `crates/document/src/lib.rs:1102-1107`: placement records `Geometry::Placed { at, level }`.
- `crates/document/src/lib.rs:1110-1115`: the distinct wall-only path records `Geometry::InWall { host, along_m }`.
- `crates/document/src/spatial.rs:33-66`: geometry has level-placed and wall-hosted family representations, not a ceiling/face family-host representation.

Ceiling/face metadata in family definitions is not implemented ceiling/face host resolution, identity association, or host-derived level/offset. Open is supported; no runtime hosting capability is claimed.

### 4. C16/C18 family-existence closures — PASS within the prior scope

Locations: progress `:104` and `:106`.

Both still say Closed. `library.rs:1182-1227` defines the pendent sprinkler with solids, a FireProtection pipe connector, plan symbol, Category::Sprinkler and Host::Ceiling metadata. `library.rs:1363-1398` defines the clevis hanger with rod/clamp solids, a plan symbol and Host::Face metadata. Both remain registered in `families()`.

As in the prior review, this PASS establishes the contracted family-existence slice only. It does not certify the historical register's broader C16 requirements (upright heads and wet/dry systems), C18 automatic placement on structural hosts, or C5 host resolution. Those broader closures would require separate evidence and acceptance checks.

### 5. §5.1 Easy-tier annotation and ancestry — PASS within the prior scope

Progress `:138` says CLOSED and cites `b17a1ff`, `fccee7b`, `91eb050`, `f84104a`, `7afe07b`. Both documents retain the top verification note.

For each cited commit, `git merge-base --is-ancestor <commit> b6ef0ee1e67d527d81e851c1cdd7ded220b0b6be` returned 0. `git show --format=fuller --stat <commit>` independently confirmed relevant IFC, shell and level-tab changes. The base is itself an ancestor of the candidate (exit 0).

This is annotation/ancestry verification, not fresh functional certification of every behavior in B2/B7-B16/B21-B24. The incomplete workspace release attempt below cannot supply that certification.

### 6. C28 arc grids — PASS

Progress `:127` in §4 still says arc grids exist and cites `crates/document/src/grid_arc.rs`, `01ddf1f`. The module defines arc geometry, polyline points, bulge conversion and unit tests. `git show --stat 01ddf1f` shows document/IFC/UI curved-grid implementation and tests. Its ancestry check against the base returned 0.

No claim is made that every other C28 requirement is closed. The locator is §4, not the earlier contract's §3.4.

### 7. SUPERSEDED banners and dangling reference — PASS

Progress §3.1, §3.2 and §3.3 retain SUPERSEDED banners at :51, :67 and :88. Full document inspection and `git grep -n -F '§4.4' <candidate> -- <both documents>` found no dangling reference (search exit 1 = no match). Historical references in evidence contracts/logs were not treated as live document cross-references.

## Structure and docs-only gates — PASS

The complete base-to-candidate name-status diff contains six Markdown files: the two reconciled documents and four evidence/contract/report files. There are no production, manifest, lockfile or test changes. `git diff --exit-code <base> <candidate> -- Cargo.toml Cargo.lock README.md crates apps` returned 0.

For each reconciled document, independent comparison of extracted heading sequences returned 0, and comparison of table first-cell sequences returned 0: section order and table-row identities were preserved. Complete diff inspection confirms local row replacements and blockquote insertions without broad reformatting. `git diff --check <base> <candidate>` returned 0. `git ls-files --eol` reports LF in index and working tree for both documents. The short C5/C16/C18 rows already lacked separate evidence cells; no new table-layout defect is introduced.

## Independent execution gates

Observed environment: Windows/MSYS x86_64, MSYS_NT-10.0-26300, Msys 3.6.6-1cdd4371.x86_64. rustc 1.99.1 (58cf8f9c3 2026-10-13); cargo 1.99.1 (5f94df478 2026-08-27); cargo-audit 0.22.2. CPU/RAM were not measured; no performance claim is made.

Raw log directory, absolute path:
`C:/Users/wilfr/AppData/Local/hermes/profiles/bim-review/cache/scratch/`

1. Formatting — FAIL.
   Command: `cargo fmt --all -- --check`.
   Exit 1; existing formatting differences begin at `apps/desktop/src/app_settings.rs:14`. All Rust code is identical to the base, so this is not a documentation-induced regression. No formatter changes were applied.
   Evidence: `gap-recon-6e98ae9-fmt.log` in the raw log directory.

2. Workspace release tests — BLOCKED.
   Command: `CARGO_TARGET_DIR=C:/Users/wilfr/AppData/Local/hermes/profiles/bim-review/cache/scratch/gap-recon-6e98ae9-release cargo test --workspace --release --locked --offline`.
   Started as a tracked one-shot background process. It remained compiling after more than 15 minutes; the observed wait result reported uptime 946 seconds, with no completed test summary. The bounded attempt was explicitly terminated using process_manage kill; returned status killed. Last inspected log reached eframe compilation. No test pass/fail total or natural cargo exit code is available, and no release PASS is carried forward from the older SHA.
   Evidence: `gap-recon-6e98ae9-release.log` in the raw log directory; process `proc_c3646a9ba004`.

3. Clippy with warnings denied — PASS.
   Command: `CARGO_TARGET_DIR=C:/Users/wilfr/AppData/Local/hermes/profiles/bim-review/cache/scratch/gap-recon-6e98ae9-clippy cargo clippy --workspace --all-targets --locked --offline -- -D warnings`.
   Exit 0; log ends with openbim-desktop and Finished dev profile in 12m 28s. Separate target directory from release tests.
   Evidence: `gap-recon-6e98ae9-clippy.log` in the raw log directory; process `proc_c51700c83da0`.

4. Full dependency audit — BLOCKED for complete registry/yanked checking; advisory-only scan — PASS.
   `cargo audit` returned 0 and fetched/loaded 1352 advisories, scanning 489 dependencies. However, its log contains registry timeout errors for yanked-package checks, so exit 0 is not treated as a fully completed audit. It reports four allowed unmaintained warnings (ttf-parser, zbus_names, zvariant, zvariant_derive) and a yanked clipboard-win warning.
   An explicitly narrower alternative, `cargo audit --no-fetch --no-yanked`, returned 0 with the fetched database, 489 dependencies and four allowed unmaintained warnings. This supports only the advisory scan; it does not repair the incomplete full registry check or establish warning-free dependencies.
   Evidence: `gap-recon-6e98ae9-audit.log` and `gap-recon-6e98ae9-advisories.log` in the raw log directory.

A Python -c assertion attempt and an execute_code attempt were blocked by unattended-session approval policy and did not execute. No permission/security configuration was changed. Equivalent permitted Git extraction, bounded shell counts, set comparison and direct source reads supplied the documentation checks. No GUI, Revit comparison, benchmark or independent IFC reader was run.

## Decision

The prior P1 and both P3 findings are resolved in the exact candidate. All seven requested documentation claims and the structure/docs-only gates PASS at the candidate above, subject to the explicit inherited scope limits. There are no new findings requiring a documentation remedy within this re-review contract.

The overall engineering gates are not all green: formatting FAIL, release tests BLOCKED and complete dependency registry checks BLOCKED. These are not silently waived by the documentation verdict. Any changed candidate or uncommitted tracked patch invalidates affected results and requires rerunning those checks. This review does not authorize merging, pushing or publishing.
