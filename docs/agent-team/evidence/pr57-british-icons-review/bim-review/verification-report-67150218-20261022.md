# PR #57 — British BS 1363 socket/switch icons: independent verification report

- **Profile / role:** `bim-review` (independent reviewer), READ-ONLY
- **Host / toolchain:** Windows 11; cargo/rustc 1.99.1; Python with Pillow 12.3.0, numpy 2.5.4
- **Timestamp:** 2026-10-22T08:49:44+07:00
- **Repository:** `wildut85523456k/revit-alike` — local clone `C:/Users/wilfr/hermes-first-task/revit-alike`
- **Candidate SHA:** `67150218336b76ee48cf5c2ee77210c3c0e60337` (branch `fix/british-electrical-icons-v2`)
- **Base SHA:** `840f709e1e754b79bfb6340cf8dd7a173b5d8d1d` (main)
- **Exact diff:** `git diff 840f709 67150218` — **12 files, +78/−81**, all under `assets/icons/`; **0 `.rs` files**
- **Isolation:** inspected in a dedicated detached worktree `C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr57` (created with `git worktree add --detach … 67150218`); the shared checkout stayed on `feat/auto-008-obg-ds` and was never modified. Reproducibility scripts were run in a scratch copy under `$LOCALAPPDATA/Temp` / the Hermes scratch dir, so the reviewed `assets/`, scripts, JSON and docs were never written to. This report was written to the evidence directory only.

## Diff (text changes, verbatim intent)

- `generate.py` — `SHAPES["socket"]` and `SHAPES["switch"]` SVGs rewritten. New socket: faceplate + `<rect x="11" y="6" width="2" height="4"/>` (earth, top-centre, **vertical**) + `<rect x="8" y="14" width="3" height="2"/>` and `<rect x="13" y="14" width="3" height="2"/>` (live/neutral, bottom, **horizontal**). Old socket used two `circle` (round) pins — the European style.
- `extract_catalog.py` — adds `SKIP = {'socket','switch'}`; for those IDs it retains the existing PNG, records `replacement: true` + sha256, and does **not** overwrite (preserves the British replacements).
- `isometric.py` — new stub module (`SUPPORTED = set()`, `svg()` raises). No isometric icons on this branch.
- `catalog-provenance.json` — `socket`/`switch` entries changed from `{crop, sha256}` (EU crops) to `{replacement: true, note, sha256}` with new hashes; **IDs retained**.
- `rust/socket.png`, `rust/switch.png`, `approved-3d/socket.png`, `approved-3d/switch.png` — binary replacements (European → British).
- Docs `README.md`, `README-v02.md`, `V0.2-REVIEW.md`, `APPROVED-3D-ASSETS.md` — updated to British style; Qt/gallery marked historical.

Note: `socket`/`switch` are **approved PNG overrides** in `generate.py` (they have committed PNGs in `approved-3d/`), so the changed `SHAPES` SVGs are only the unused fallback path; even that fallback is now BS 1363-consistent, so there is no contradictory European geometry left anywhere.

---

## Acceptance items

### A1 — Socket artwork is BS 1363 — **PASS**
Method: `vision_analyze` on `approved-3d/socket.png` and the embedded `rust/socket.png` (the reviewer cannot view images inline; a vision model transcribed pixel geometry).
- `approved-3d/socket.png`: "three rectangular slots … Earth pin top centre, oriented vertically; Live and Neutral bottom, horizontal … NOT European Schuko/Type-F (two round pins)."
- `rust/socket.png` (embedded): "one vertical rectangle centered at the top … two horizontal rectangles at the bottom … British BS 1363 … ruling out the Schuko option."
- `generate.py` fallback SVG for `socket` is also now three rectangular slots (earth vertical top-centre; live/neutral horizontal bottom).
Conclusion: three rectangular slots, earth top-centre **vertical**, live+neutral bottom **horizontal**, **no** Schuko/Type-F round pins. Meets A1 at 24/32/48 px (PNGs are 48×48, shipped at 32 px on 2×).

### A2 — Switch artwork — **PASS**
Method: `vision_analyze` on `approved-3d/switch.png` and `rust/switch.png`.
- `approved-3d/switch.png`: "square outer border (faceplate) … smaller inset square (rocker) … small blue symbol in the center functions as an indicator light."
- `rust/switch.png` (embedded): "rectangular … faceplate with an inset rocker and an LED indicator … outer rounded square border … central gray square represents the rocker … small blue cross … LED indicator."
Conclusion: rectangular faceplate with inset rocker and an LED indicator, style consistent with A1 (minimalist flat). Meets A2. (Cosmetic note R1 below: the LED renders as a small blue plus/cross rather than a round dot; it occupies the LED position and satisfies the requirement.)

### A3 — Embedded assets valid — **PASS**
Commands (run in the detached worktree):
```
cargo test -p openbim-desktop --release --no-run      # exit 0, "Finished in 6m 15s"
cargo test -p openbim-desktop --release embedded       # exit 0
cargo test -p openbim-desktop --release ribbon::tests  # exit 0
```
Result (`ribbon::tests`, 3 passed / 0 failed):
```
test ribbon::tests::embedded_icons_are_valid_png_and_unique ... ok
test ribbon::tests::every_ribbon_id_has_an_icon ... ok
test ribbon::tests::transparent_pixels_do_not_tint_filtered_edges ... ok
```
The test `embedded_icons_are_valid_png_and_unique` asserts every `ICONS[].light` is a valid PNG ≥32×32 with unique IDs; `socket`/`switch` (48×48) are in that table and pass.
PNG validity (independent Python check, `png_size` equivalent):

| file | bytes | signature | dims | colour type |
|---|---|---|---|---|
| `rust/socket.png` | 271 | OK | 48×48 | 6 (RGBA) |
| `rust/switch.png` | 294 | OK | 48×48 | 6 (RGBA) |
| `approved-3d/socket.png` | 251 | OK | 48×48 | 6 (RGBA) |
| `approved-3d/switch.png` | 284 | OK | 48×48 | 6 (RGBA) |

All valid, non-empty, RGBA, ≥32. Note: `cargo test -p openbim-desktop --release` (unfiltered) was allowed to run but timed out at 180 s on **unrelated slow `model_view` tests** (several drag/grip tests each "running for over 60 seconds"); the timeout is not in the icon path and does not affect A3.

### A4 — Provenance integrity — **PASS**
Independent recompute of `sha256` (Python `hashlib`) vs `approved-3d/catalog-provenance.json`:
- `approved-3d/socket.png` → `25948d6b20853ed2cd10f2fd5e92d887dcd869e176b51d60c75bfb005d694cf3` — **matches** recorded `socket.sha256`.
- `approved-3d/switch.png` → `6bf849076fb51e44c98d8a793b9b4c1cb7bb17074996a891211d1a17e02078a1` — **matches** recorded `switch.sha256`.
- Both entries carry `"replacement": true` with a British-BS-1363 note; IDs `socket` and `switch` are **retained** (keys unchanged).
Note (by design, not a defect): provenance records the `approved-3d/` **source** hashes; the derived `rust/` copies have different bytes/hashes (`rust/socket.png` = `d40eea1f…`, `rust/switch.png` = `1d43b1e3…`) because `make_rust_icons.py` strips the near-white background. This is expected and documented (see A5/A6).

### A5 — Reproducibility — **PASS** (with documented, justified scope note)
Method: scripts run in a scratch copy of `assets/icons/` (reviewed assets untouched).

1. **`make_rust_icons.py` (the real embedded-icon pipeline) — byte-identical.**
   `python3 assets/icons/make_rust_icons.py` regenerated the full `rust/` set; the two PR icons matched the committed bytes exactly:
   ```
   socket: regenerated=d40eea1f37fb4eae committed=d40eea1f37fb4eae identical=True (271/271)
   switch: regenerated=1d43b1e328a7f946 committed=1d43b1e328a7f946 identical=True (294/294)
   ```
   The embedded PNGs are thus deterministically reproduced from the British `approved-3d/` source.
2. **`extract_catalog.py` — replacement preservation verified.** A partial run (see scope note) read the existing `approved-3d/socket.png`/`switch.png`, left their bytes unchanged, and wrote them into the regenerated `catalog-provenance.json` as `{"replacement": true, "note": "British BS 1363 style replacement (not a catalog crop).", "sha256": <matching>}` — i.e. replacement bytes, IDs and hashes are preserved by the `SKIP` set.
3. **Scope note (documented & justified, not a PR defect):** `generate.py` and `extract_catalog.py` both read `ROOT/manifest.json`, which is **absent from the committed tree on both the candidate `67150218` and the base `840f709`** (`git ls-files` / `git ls-tree -r 840f709` → no manifest.json). Consequently those two scripts fail with `FileNotFoundError` before completing. This is a **pre-existing, explicitly documented** condition: `README-v02.md` and `APPROVED-3D-ASSETS.md` state the Qt/gallery path is historical and that "`manifest.json` … [is] not present on this branch"; `APPROVED-3D-ASSETS.md` literally warns `python3 assets/icons/extract_catalog.py` "will fail with FileNotFoundError" and is "retained for reference only." These are the retired Qt-path scripts; the live pipeline is `make_rust_icons.py`, which is fully reproducible (step 1). The acceptance clause "byte-identical, **or a documented and justified difference**" is satisfied.

### A6 — Documentation accuracy — **PASS**
Read all four docs against the artwork/code:
- `README.md` — "Electrical switch and socket use **British BS 1363** style (rectangular three-pin socket, rocker switch), matching the first calculation profile (British practice, decision of 15 Oct 2026)." Replaces the prior EU-design line. Correct.
- `README-v02.md` — marks Qt/gallery instructions historical; states `manifest.json`/`apps/desktop-qt` not present; `isometric.py` is an empty stub; source is "newly authored … not a copy of … proprietary product assets." Consistent.
- `V0.2-REVIEW.md` — status "British electrical icon replacement committed"; Electrical = "British-style rocker switch and BS 1363 socket (rectangular three-pin)"; Qt/gallery marked historical; gives the correct review commands (`cargo test -p openbim-desktop --release`, `make_rust_icons.py`). Consistent.
- `APPROVED-3D-ASSETS.md` — documents the British replacement, the `make_rust_icons.py` pipeline, the `SKIP` set preserving socket/switch, and the historical/failing `extract_catalog.py`. Consistent.
- `grep -niE "schuko|type[- ]?f|european|\bEU\b"` over all four docs and over `catalog-provenance.json` → **no matches** (exit 1). No stale European-icon or contradictory provenance claims remain.

### A7 — Mechanical gates — **PASS**
- **No production Rust in the diff:** `git diff --name-only 840f709 67150218 | grep -c '\.rs$'` → **0**. All 12 changed files are under `assets/icons/` (PNG/JSON/Py/md). The icons reach the app only via the unchanged `include_bytes!` in `apps/desktop/src/icons.rs` (`icon!("socket")`, `icon!("switch")`, no `.dark.png` — correct, they are not line art).
- **clippy:** `cargo clippy --workspace --all-targets -- -D warnings` → **exit 0**, 0 warnings (`Finished dev profile in 3m 59s`). The only 4 lines containing the substring "error" are crate names (`quick-error`, `thiserror`, `thiserror-impl`, `error-code`).
- **fmt on touched files:** no Rust files are touched, so there is nothing for `rustfmt` to check on this PR (trivially clean). For context, `cargo fmt --all -- --check` does report pre-existing formatting drift in `apps/desktop/src/automation.rs`, but that file is **not** in this PR's diff (it belongs to other in-flight work) and is out of scope here.

---

## Findings (severity-ranked)

- **LOW / informational (pre-existing, documented):** `generate.py` and `extract_catalog.py` cannot complete end-to-end because `manifest.json` is absent from the tree (on base and candidate). Not introduced by PR #57 and explicitly documented as historical in `README-v02.md` / `APPROVED-3D-ASSETS.md`. The embedded-icon pipeline (`make_rust_icons.py`) is unaffected and byte-reproducible. No action required for this PR.
- **LOW / informational (by design):** `catalog-provenance.json` records `approved-3d/` source hashes, not the derived `rust/` hashes; the two sets legitimately differ (background removal). Correct and documented; not a defect.
- **INFO / cosmetic:** the switch LED renders as a small blue plus/cross rather than a round dot. It sits in the LED-indicator position and satisfies A2's "LED indicator"; flag only for the user's aesthetic preference, not a gate failure.

## Raw evidence paths
- Worktree (detached, will be removed after review): `C:/Users/wilfr/hermes-first-task/revit-alike-wt-pr57`
- A5 scratch copies: `$LOCALAPPDATA/Local/hermes/cache/scratch/pr57-a5/icons/` (`make_rust_icons.py` and partial `extract_catalog.py` outputs)
- clippy log: `$LOCALAPPDATA/Temp/pr57-clippy.log`
- Candidate SHA pinned: `67150218336b76ee48cf5c2ee77210c3c0e60337`

## Overall verdict: **PASS**
All acceptance items A1–A7 pass on candidate `67150218`. No unmet requirements. The three LOW/INFO findings are pre-existing or cosmetic and do not block this PR. This is an independent verification; the prior `bim-review` report at `docs/agent-team/evidence/pr57-british-icons-review/bim-review/verification-report.md` (2026-10-21) was not used as a substitute.
