# PRT-001 candidate verification — 2026-10-18

Status: implemented locally and verified at the sheets-crate level; not pushed or merged.
PR #45 remains documentation-only. No desktop Print dialog or branch integration is claimed.

Source baseline: m1c-sheets-printing at 18520949edece94c8db30f764378ba818d6433bf.
Git clone required credentials unavailable to the shell, so the sheets crate, XLSX fixtures and
original workspace lockfile were retrieved through the connected GitHub API. The crate was
tested standalone using the upstream lockfile; no dependency change is part of this patch.

Environment: Linux x86_64; rustc 1.99.1 (58cf8f9c3 2026-10-13), local minimal stable toolchain.

Changes:
- Custom paper preserves width and height in caller order; presets stay landscape.
- portrait() returns portrait dimensions even for an already portrait custom sheet.
- DXF EXTMIN/EXTMAX record the sheet footprint; rendered border remains inside the margins.
- A0-A4 portrait and landscape output tests, an export fixture example and an independent checker.

Results:
- Original paper/DXF source with the new export regression: FAIL as expected, A0 portrait
  actual (1189, 841), expected (841, 1189).
- Candidate: cargo test --manifest-path crates/sheets/Cargo.toml --release: PASS, 62 tests.
  Baseline and final candidate use separate target directories for final evidence.
- cargo clippy --manifest-path crates/sheets/Cargo.toml --release --all-targets -- -D warnings:
  PASS, exit 0.
- portrait_scene example: PASS, writes ten PDF pages and ten DXF files.
- Independent pypdf/ezdxf checker: PASS for all ten cases, checking PDF MediaBox, DXF header
  dimensions and actual DXF line bounds with 10 mm margins.
- git diff --check: PASS.

Limits:
- Full workspace, desktop interaction and independent human/code review have not run.
- Rustfmt check on changed existing files reports pre-existing formatting differences;
  unrelated formatting was retained. The new Rust example was formatted.
- An initial unpinned standalone dependency resolution hit calamine/zip incompatibility;
  final tests used the existing upstream lockfile (zip 2.4.2), not a dependency workaround.
- This establishes sheet/export orientation behavior, not Revit workflow parity.
