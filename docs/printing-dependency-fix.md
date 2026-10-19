# Printing dependency audit repair — 2026-10-18

The original PR #48 head 6d0ca634 passed the printing workflow, full workspace tests and Clippy
on GitHub. The separate cargo audit gate failed on five findings: quick-xml
RUSTSEC-2026-0194/0195 and zip RUSTSEC-2026-0367/0368/0369.

The sheets branch's old calamine 0.26 dependency introduced quick-xml 0.31 and zip 2.4.2.
The published calamine 0.36.1 fixes quick-xml but still depends on zip 8.6.
The maintained upstream source supports both patched dependency ranges; pin
39ebf5d851543e4f3dc5964baeadd383eb17a3ae rather than following a moving branch.
The lockfile now resolves quick-xml 0.42.0 and zip 9.0.5.

This is an upstream source pin, not a released calamine update. Replace it with a tested
published release carrying these fixes when available. The sheets crate now declares the
dependency's Rust 1.88 minimum. No audit ignore, disabled check or tolerance is added.

The existing main ZIP upgrade 0a890e7 concerns photo/family code absent from this sheets
branch; its patch cannot be directly applied here. No unrelated photo/family work is imported.

Local validation on Linux x86_64 / Rust 1.99.1:
- Full workspace release tests: PASS, 563 tests, including the Excel importer fixtures.
- Full workspace all-target Clippy with warnings denied: PASS.
- Real PDF/DXF colour/grayscale/black-mode checks: PASS.
- The current cargo audit result and the full remote workflow result remain separate gates;
  read their actual reports rather than inferring an audit PASS from version changes.

Reference: https://github.com/tafia/calamine/tree/39ebf5d851543e4f3dc5964baeadd383eb17a3ae
