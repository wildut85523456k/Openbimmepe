# PR #72 re-review — gh-ctl + automation.sh (shell-script review), fix candidate

| Field | Value |
|---|---|
| Reviewer profile | `bim-review` (independent reviewer) |
| Review type | READ-ONLY re-review (no Rust); re-verifies the F1/F2/F3/F6 fix |
| Repository | `wildut85523456k/revit-alike` |
| PR | #72 `Add gh-ctl and automation.sh (repo-agnostic dev tooling)` |
| **Candidate SHA** | **`23f2df4bb69d329864de70da2924288410aa1588`** (`git rev-parse HEAD` in the worktree) |
| Prior (failed) candidate | `799c1f08283d61365d9c6b37aaad425ff16fb959` — prior verdict **does not carry over** |
| Base / merge-base | `main` @ `271c774` / merge-base `f0dc0b7` (verified `git merge-base 23f2df4 271c774` → `f0dc0b7`) |
| Diff reviewed | `git diff f0dc0b7 23f2df4` → 2 files, +333/−0 (`gh-ctl` +248, `automation.sh` +85) |
| Read worktree | `C:/Users/wilfr/hermes-first-task/revit-alike-tools` (pre-existing checkout of `feat/repo-tools` @ 23f2df4, **not detached** — see note) |
| Shared checkout | `C:/Users/wilfr/hermes-first-task/revit-alike` @ `feat/auto-008-obg-ds` — scripts **not modified** |
| Host / toolchain | Windows 11, Git-for-Windows bash 5.2.37 (MSYS), GNU coreutils `stat -c` present, `/proc` present, `gh` 2.102.0 |
| Timestamp | 2026-10-23 06:17 (+07) |

## Overall verdict: **PASS**

All four HIGH defects from the prior FAIL are genuinely fixed and demonstrated with real
invocations, and the whole working surface (p2 parallel, p0/p1 lock + priority-ordered
queue drain, `status`, `health`, `cargo`, `gate` usage/exit codes, the `automation.sh`
health gate and role dispatch) still behaves. No regression was introduced. The four
out-of-scope items (F4/F5/F7/F8) remain present as known follow-ups; they are noted, not
counted against the PR.

The prior `799c1f0` FAIL is superseded by this SHA.

---

## Acceptance items

### A1 — F3 fixed: gate lock released → **PASS**

`gate <target>` acquires a `mkdir` directory lock and now releases it with
`rm -f "$LOCK_FILE.pid"` + `rm -rf "$LOCK_FILE"` (`gh-ctl:169-170`). Two gates on the
same target in a throwaway `REPO_ROOT` both exit 0 and the lock dir is gone. Evidence
`raw/e1-gate-release.txt`:

```
[gate target-x] acquired — running: echo GATE-ONE
GATE-ONE
gate1_exit=0
target-x gone (PASS)
[gate target-x] acquired — running: echo GATE-TWO
GATE-TWO
gate2_exit=0
target-x gone after gate2 (PASS)
gh-ctl gate <target> <command...>
gate_noargs_exit=2
```

### A2 — F2 fixed: unknown tier refused → **PASS**

A `case "$PRIO"` validation (`gh-ctl:183-186`) rejects anything that is not `p0|p1|p2`
**before** the rank/parallel branch. `p3`, `P0`, `p0x`, `p1x` all print
`gh-ctl: unknown priority '<x>' (use p0|p1|p2)` and exit **2**; the command is **not**
run (no `SHOULD-NOT-RUN` in output, `.gh-ctl` stays empty). Evidence `raw/e2-badprio.txt`:

```
gh-ctl p3   -> exit=2   (no command run)
gh-ctl P0   -> exit=2   (no command run)
gh-ctl p0x  -> exit=2   (no command run)
gh-ctl p1x  -> exit=2   (no command run)
gh-ctl ""   -> exit=0   (empty tier defaults to p2 — documented default, runs)
```

### A3 — F1 fixed: stale lock reclaimed → **PASS**

On a `mkdir`-failure the code now reads `lock.pid`; if the owner PID is gone it reclaims
(`rm -f lock.pid` + `rm -rf lock`) and re-acquires (`gh-ctl:197-213`). A planted dead lock
is reclaimed and the p0 job **runs**. Evidence `raw/e3-stale-lock.txt`:

```
planted: lock dir + lock.pid=999999
status before -> LOCK: held / owner PID: 999999 (stale — not running)
health before -> LOCK: STALE / NOT Sane (exit 1)
[gh-ctl p0] reclaiming stale lock (owner PID 999999 is gone)
[gh-ctl p0] holding lock -> running: echo SHOULD-RUN-RECLAIMED
SHOULD-RUN-RECLAIMED
p0_exit=0
lock reclaimed/removed (PASS)
status after -> LOCK: free / QUEUE: empty
[gh-ctl p1] holding lock -> running: echo P1-RUNS
P1-RUNS
p1_exit=0
```

### A4 — F6 fixed: automation.sh `cargo` reaches the gate → **PASS**

The parser now accepts `review|impl|cargo` (`automation.sh:44`), so the `cargo)` dispatch
branch (`automation.sh:66-69`) is reachable. `automation.sh cargo "…"` passes the parser,
runs the health gate, and reaches the cargo role. Evidence `raw/e4-automation.txt`:

```
[automation] gating: gh-ctl health
=== gh-ctl health === AUTH: ok / REPO: ok (main) / LOCK: free / ENVIRONMENT: Sane
[automation] health gate passed
[automation] role=cargo (gh-ctl cargo) — serialised build, blocking-wait, never pushes while taken
[cargo] acquiring lock (timeout 300s)...
[cargo] acquired — running: echo CARGO-REACHED
CARGO-REACHED
exit=0
```

Also verified in the same file: `review` → `gh-ctl p2` (exit 0); `impl` on `main` → warning
then `gh-ctl p0` (exit 0); `bogus` / no-role / `cargo` with no command → usage + exit 2;
non-sane repo → `ABORT — ENVIRONMENT: NOT Sane` + exit 1 (command not run).

### A5 — No regression → **PASS**

Full regression driver: `raw/pr72-regression.sh`, output `raw/e5-regression.txt`.

- **p2 parallel** — `gh-ctl p2 "echo P2-ONE"` exits 0 and `.gh-ctl` stays empty (0 entries).
- **p0/p1 lock + priority-ordered queue drain** — a live p0 holder shows
  `LOCK: held / owner PID 869 (alive)`; a p1 and a p0 both enqueue and exit 0; `status`
  reports `QUEUE: 2 pending job(s)`; the holder drains them in priority order
  (`QUEUED-B` (p0) before `QUEUED-A` (p1)); after the drain `LOCK: free / QUEUE: empty`.
- **cargo lock** — `gh-ctl cargo "echo CARGO-ONE"` acquires, runs, releases; `.gh-ctl` empty.
- **gate usage/exit codes** — `gate` with no args → usage + exit 2; two gates on `t1` both
  exit 0; `t1` is not left as a directory. Held-target path (`raw/e7-outofscope.txt`) waits
  then prints `still held — not pushing` and exits 1.
- **status / health** — `status` exit 0; `health` exit 1 on a non-git throwaway (REPO FAIL);
  `health` with a live held lock → `LOCK: held … (/proc/<pid> present)` + Sane (exit 0);
  `health` with a dead lock → `LOCK: STALE` + NOT Sane (exit 1) (`raw/e7b-health-lock.txt`).

**Pre-existing edge case (NOT a regression, not counted):** a queued command that calls
`exit N` terminates the draining holder with `N` and leaves the queue file undrained
(`raw/e5-regression.txt`, A5f: `holder2_exit=7`, queue still `1 pending`). The drain loop is
**byte-identical** between `799c1f0` and `23f2df4` (`git diff 799c1f0 23f2df4`), so this
behaviour predates the fix. It requires a queued job that calls `exit` (unusual), and the
lock is still released correctly.

### A6 — Syntax → **PASS**

```
$ bash -n gh-ctl          exit=0
$ bash -n automation.sh   exit=0
```

### A7 — Safety → **PASS**

Evidence `raw/e6-safety-repagnostic.txt`.

- **No secrets:** a scan for `token|secret|password|api[_-]?key|ghp_|github_pat_|PRIVATE KEY`
  over both files returned **no matches**.
- **No unguarded destructive command:** the only `rm -rf` sites are `gh-ctl:129`
  (`$CARGO_PID`/`$CARGO_LOCK`, fixed paths under `.gh-ctl/`), `gh-ctl:170` (gate lock),
  `gh-ctl:205` (stale-lock reclaim), `gh-ctl:223` (cleanup, guarded by `HELD`). All are
  gh-ctl-internal paths, not user-supplied.
- **`eval` surface documented:** `eval "$*"` at `gh-ctl:118,165,191,229` and
  `GHCTL_SKIP_LOCK=1 eval "$CMD"` at `gh-ctl:123,237`. Inherent to a command-runner design
  and disclosed by the usage text (`gh-ctl:14-18`, `automation.sh:15-17`). No untrusted
  string was eval'd during this review — every exercised command was an authored literal.
- Residual risk (by design, documented): a caller can pass `; rm -rf <path>`; there is no
  allow-list. Recorded, not counted against the item.

### A8 — Repo-agnostic + shell-only → **PASS**

- `git diff --name-only f0dc0b7 23f2df4` → `automation.sh`, `gh-ctl`; **0** `.rs`/`.toml`
  files in the diff. Shell-only.
- The only checkout coupling is `REPO_ROOT` (`gh-ctl:23`, `automation.sh:21`), defaulting
  to `$HOME/hermes-first-task/revit-alike`; no Windows/host-specific path, no Dynamo/Revit
  logic. The scripts call only `gh`/`git`/`cargo` generically.

---

## Out-of-scope items (known follow-ups — still present, NOT blockers)

| ID | Severity | File:line | Status in 23f2df4 |
|---|---|---|---|
| F4 | MEDIUM | `gh-ctl:45` vs `84` | Still present — `status` uses `kill -0`, `health` uses `/proc/$OWNER`; not equivalent on multi-user hosts |
| F5 | MEDIUM | `gh-ctl:150` | Still present — gate lock at `$REPO_ROOT/$TARGET` (not under `.gh-ctl/`); stray dirs in the checkout |
| F7 | MEDIUM | `gh-ctl:52,84,126,240` | Still present — GNU-only `stat -c`, `/proc`, `sort -t$'\t'`; `health` gates every `automation.sh` job, so it refuses to run without procfs |
| F8 | LOW | `gh-ctl:197,207,226` | Still present — `trap cleanup` is installed after `mkdir "$LOCK_FILE"`; a signal in that window leaves the lock held (feeds F1) |

None of these is counted against the PR per the contract.

---

## Commands actually run (summary)

```
bash -n gh-ctl                                exit 0
bash -n automation.sh                         exit 0
git rev-parse HEAD                            23f2df4bb69d329864de70da2924288410aa1588
git merge-base 23f2df4 271c774                f0dc0b7d6c927c24c7ecb99be775acf4712c72e4
git diff --name-only f0dc0b7 23f2df4          automation.sh, gh-ctl
git diff --stat f0dc0b7 23f2df4               2 files, +333/-0
git diff 799c1f0 23f2df4                      (fix commit = exactly the 4 claimed fixes)
REPO_ROOT=<throwaway> bash gh-ctl gate|p3|P0|p0x|p1x|p0|p1|p2|cargo|status|health ...
REPO_ROOT=<throwaway> bash automation.sh cargo|review|impl|bogus|(none) ...
```

Throwaway roots lived under `$TMPDIR/pr72-r2*` (removed after the run). Nothing was
pushed, merged, published or deleted. The judged scripts were not modified.

---

## Findings summary

| ID | Severity | File:line | Finding |
|---|---|---|---|
| F1 | HIGH | `gh-ctl:197-213` | **FIXED** — stale lock reclaimed; p0/p1 job runs |
| F2 | HIGH | `gh-ctl:183-186` | **FIXED** — unknown tier refused (exit 2), not run unlocked |
| F3 | HIGH | `gh-ctl:169-170` | **FIXED** — gate lock released with `rm -rf`; two gates both succeed |
| F6 | HIGH | `automation.sh:44,66-69` | **FIXED** — `cargo` role reachable; passes the parser and the health gate |
| F4 | MEDIUM | `gh-ctl:45` vs `84` | Known follow-up — `status`/`health` liveness predicates differ |
| F5 | MEDIUM | `gh-ctl:150` | Known follow-up — gate lock outside `.gh-ctl/` |
| F7 | MEDIUM | `gh-ctl:52,84,126,240` | Known follow-up — GNU/Linux-only constructs |
| F8 | LOW | `gh-ctl:197,207,226` | Known follow-up — trap installed after lock acquisition |
| — | LOW | `gh-ctl:236-240` | Pre-existing (unchanged since 799c1f0): a queued `exit N` ends the holder with `N` and leaves the queue undrained; not a regression |

## Raw evidence paths

All under `docs/agent-team/evidence/repo-tools-review/bim-review/raw/` (this SHA's captures
are the 06:12–06:17 files; the 05:5x files are the prior 799c1f0 review):

| File | Contents |
|---|---|
| `e0-setup.txt` | throwaway roots, worktree, TMPDIR |
| `e1-gate-release.txt` | A1 — two gates on one target both exit 0, lock dir gone |
| `e2-badprio.txt` | A2 — p3/P0/p0x/p1x refused (exit 2), command not run |
| `e3-stale-lock.txt` | A3 — planted dead lock reclaimed, p0 runs, p1 runs |
| `e4-automation.txt` | A4 — cargo/review/impl dispatch, health gate, usage exits |
| `e5-regression.txt` | A5 — p2, p0/p1 lock + priority drain, cargo, gate, status/health |
| `e6-safety-repagnostic.txt` | A7/A8 — secrets, rm/eval sites, diff name-only, REPO_ROOT coupling |
| `e7-outofscope.txt` | gate-held exit path; F4/F5/F7/F8 source lines |
| `e7b-health-lock.txt` | health with live vs dead held lock |
| `pr72-regression.sh` | the exact regression driver (authored literals only) |

## Decision (coordinator fills)

- Code reviewer decision: **PASS** — F1/F2/F3/F6 fixed, no regression.
- Coordinator disposition:
- User merge/push approval (separate):
