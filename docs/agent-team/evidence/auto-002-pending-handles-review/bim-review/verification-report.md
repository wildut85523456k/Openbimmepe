# AUTO-002 pending handles — independent verification report

- **Profile:** `bim-review` (independent Rust reviewer, READ-ONLY)
- **Timestamp:** 2026-10-22 14:44:11 (machine local, UTC+07 Asia/Phnom_Penh)
- **Host / OS:** Windows 11, host `CDC_COOlaptop`
- **Toolchain:** rustc 1.99.1 (58cf8f9c3 2026-10-13); cargo 1.99.1 (5f94df478 2026-08-27); clippy 0.1.99 (58cf8f9c35 2026-10-13); cargo-audit 0.22.2
- **Candidate SHA:** `ff2e8ed36b467e69e02fce910a7b1090cee4c18e` (branch `feat/auto-002-pending-handles`)
- **Base branch / tip:** `main` @ `853a91a020bae2aae32a40b7ed31a258d3691ec6`
- **Branch point (merge-base):** `29722bf13c92c7c0ba689a1e5527afc47639eb9c`
- **Scope inspected:** detached worktree `C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr69` at `ff2e8ed`

## Scope note (important)

The task contract states `git diff main ff2e8ed` is "7 files, +576/-0". On this
clone that literal command does **not** yield 7 files: local `main` is at `853a91a`
but `ff2e8ed` branched off the older merge-base `29722bf`, so `git diff main ff2e8ed`
reports 22 files / +585 / -2294 — the extra 15 are main-side additions that post-date
the branch point (docs/audits and docs/revit-style-ui), shown as deletions relative to the
branch. The candidate's **own** changes are exactly the 7 files, +576/-0:

```
$ git diff --stat 29722bf13c92c7c0ba689a1e5527afc47639eb9c ff2e8ed
 Cargo.lock                                |   2 +
 crates/automation/Cargo.toml              |   1 +
 crates/automation/src/engine/group.rs     |  65 +++++++++
 crates/automation/src/engine/handle.rs    |  57 ++++++++
 crates/automation/src/engine/mod.rs       | 232 +++++++++++++++++++++++++
 crates/automation/src/lib.rs              |   1 +
 crates/automation/tests/04b_acceptance.rs | 218 ++++++++++++++++++++++++++
 7 files changed, 576 insertions(+)
```

This matches the contract's stated scope. (Also note: the contract's command uses the
package name `automation`, but the real package is `openbim-automation`; the workspace
equivalent `-p openbim-automation` was used.)

## Commands actually run (and exit codes)

All builds used an isolated target dir `C:/Users/wilfr/AppData/Local/hermes/cache/scratch/pr69-target`
so the shared checkout's `feat/auto-008-obg-ds` build was not disturbed.

| # | Command | Result |
|---|---|---|
| A4 | `cargo test -p openbim-automation --test 04b_acceptance --release` | **exit 0** — `16 passed; 0 failed; 0 ignored` |
| A5a | `cargo test --workspace --release --no-fail-fast` | **exit 0** — 108 test binaries, `1182 passed; 0 failed; 2 ignored` |
| A5b | `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** — 0 `warning:` lines, 0 `error`/`error[` lines, 0 `clippy::` lints |
| A5c | `cargo audit` | **exit 0** — 4 pre-existing `unmaintained` **warnings** (see below) |
| A6 | `grep -rn unsafe crates/automation/src/engine/` | no matches; `#![forbid(unsafe_code)]` at `lib.rs:3` |
| A5c aux | `cargo tree -p openbim-automation` | `uuid v1.28.0 -> getrandom v0.4.3`; none of the 4 advisory crates reachable |

### Audit detail

`cargo audit` (488 deps scanned) reports exactly the 4 pre-existing unmaintained-crate
**warnings** (warning level, not errors, hence exit 0):

- `ttf-parser` 0.25.1 — RUSTSEC-2026-0192
- `zbus_names` 4.3.4 — RUSTSEC-2026-0371
- `zvariant` 5.15.0 — RUSTSEC-2026-0370
- `zvariant_derive` 5.15.0 — RUSTSEC-2026-0372

`cargo tree -p openbim-automation` confirms **none** of these four are in the candidate
crate's dependency tree. The only dependency the candidate added is `uuid` 1.28.0, which is
actively maintained and is **not** in the advisory list. Confirmed: none of the 4 advisories
are in the candidate's crates.

## Acceptance items

| Item | Result | Evidence |
|---|---|---|
| A1 Handle identity | **PASS** | `handle.rs:12-46` — `RunId(pub Uuid)` built via `Uuid::now_v7()` (UUIDv7); `ProposalId(u64)`, `LocalId(u64)`; `PendingHandle { run_id, proposal_id, local_id, category }` (element type = `Category`). A handle is accepted only for its own run/proposal/type: enforced at construction `mod.rs:106-117` (WrongRun/WrongProposal/WrongType, duplicate local id → UnknownHandle) and re-checked in `apply` `mod.rs:193-204`. Tests `wrong_run_handle_is_refused`, `wrong_type_handle_is_refused`, `wrong_proposal_handle_is_refused`. |
| A2 op_group connected components | **PASS** | `group.rs:17-65` builds undirected adjacency from `depends_on` (ignoring edges whose endpoints are absent), walks each component, merges transitively, sorts members by `local_id`; group order by first-seen seed. Tests: `op_group_merges_overlapping_dependencies_transitively` (chain 1←2←3 + isolated 4 → groups {1,2,3},{4}), `op_group_keeps_independent_creations_separate` (5 isolated), `op_group_ignores_dependencies_on_absent_ids`. |
| A3 Engine::apply atomic commit + refusal | **PASS** | `mod.rs:177-222` validates the whole group (UnknownHandle, WrongRun, WrongProposal, WrongType, Stale, Locked) before calling `commit` exactly once (`mod.rs:212`); on any validation error or `CommitError` it returns before touching `committed`/`state`. On success records `resolved` and advances `Open→PartiallyApplied→Applied` (`mod.rs:216-220`); `NotOpen` guard for Applied/Cancelled (`mod.rs:185`); `cancel` only from Open (`mod.rs:225-231`). Tests: `apply_valid_group_commits_and_advances`, `apply_again_is_refused`, `cancel_makes_the_proposal_uncommittable`, `cancel_twice_is_refused`, `unknown_handle_is_refused`, `stale_prerequisite_refuses_whole_group` (state stays Open), `locked_member_refuses_whole_group` (state stays Open), `commit_failure_leaves_proposal_open` (state stays Open), `partial_application_then_completion`, `independent_500_case_applies_499_and_refuses_the_locked_one`. |
| A4 Acceptance tests | **PASS** | `cargo test -p openbim-automation --test 04b_acceptance --release` → exit 0, **16 passed / 0 failed** (all 16 test names listed in raw log `pr69-a4.log`). |
| A5 Workspace gates | **PASS** | workspace test exit 0 (1182 passed/0 failed/2 ignored); clippy exit 0 with `-D warnings` and 0 diagnostics; cargo audit exit 0 with only the 4 pre-existing unmaintained warnings, none in the candidate's crates. |
| A6 Safety / style | **PASS** | no `unsafe` in `engine/*`; `#![forbid(unsafe_code)]` at `lib.rs:3`; clippy `--all-targets -D warnings` clean; `uuid = { version = "1", features = ["v7"] }` added at `Cargo.toml:11`, lock updated (`Cargo.lock`: uuid listed under openbim-automation; uuid block gains `getrandom 0.4.3`). |

## Findings (severity-ranked; none block the gate)

1. **[nit / cosmetic] `group.rs:15-16`** — the doc comment says "each component is walked
   breadth-first and then sorted", but the code uses `queue.pop()` (`group.rs:47`), which is
   LIFO (DFS), not breadth-first. Output is unaffected: connected-component membership is
   identical under BFS or DFS and members are sorted afterwards, so results stay deterministic
   and correct. Comment inaccuracy only.

2. **[info / minor] `mod.rs:193-204`** — the run/proposal/type re-checks inside `apply` are
   unreachable in practice, because `Engine::new` (`mod.rs:106-117`) already guarantees every
   stored handle matches the engine's own `run_id`/`proposal_id`/`category`. Redundant but
   harmless defence-in-depth; not a defect.

3. **[info / minor] `mod.rs:212-215`** — `apply` records whatever the commit callback returns
   into `self.committed` and decides `Applied` via `committed.len() == handles.len()`, without
   checking that `resolved` is a subset of the submitted group. A misbehaving callback that
   returned extra ids would push `committed.len()` past `handles.len()` and pin the proposal at
   `PartiallyApplied`. This relies on the trusted-callback contract (the real atomic write is
   explicitly deferred to §3.2), so it is not a gate failure — noted for the future integration.

No HIGH/MEDIUM defects found. The three items above are informational.

## Overall verdict

**PASS** — all six acceptance items (A1–A6) PASS on candidate SHA `ff2e8ed`.
No unmet requirements.

## Raw evidence paths

- `C:/Users/wilfr/AppData/Local/hermes/cache/scratch/pr69-a4.log` (16/16 acceptance run)
- `C:/Users/wilfr/AppData/Local/hermes/cache/scratch/pr69-wstest.log` (workspace test, exit 0)
- `C:/Users/wilfr/AppData/Local/hermes/cache/scratch/pr69-clippy.log` (clippy, exit 0, 0 diagnostics)
- `C:/Users/wilfr/AppData/Local/hermes/cache/scratch/pr69-audit.log` (audit, exit 0)
- `C:/Users/wilfr/AppData/Local/hermes/cache/scratch/pr69-auto-deps.txt` (automation dep tree)

## Cleanup

The detached worktree `C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr69` is removed after
this review (`git worktree remove --force`). The only tracked-tree change is this report under
`docs/agent-team/evidence/`; the shared checkout's `feat/auto-008-obg-ds` branch and working
files were not modified.
