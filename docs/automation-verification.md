# Automation verification - 2026-10-16

The full Rust dependencies are now available locally. These commands passed:

```powershell
cargo check -p openbim-desktop --offline
cargo test -p openbim-automation -p openbim-desktop -p openbim-document --offline
```

Results: 49 tests passed, zero failed, one explicitly optional real-Python
interpreter test ignored. This uses the real Serde/JSON dependencies and compiled
desktop, not the earlier dependency-free verification harness.

Coverage includes typed graph validation, JSON roundtrips, preview/apply/undo,
idempotence, stale plans, conflicting writes, native quantity constraints,
snapshot validation, reverse-order scheduling, MCP lifecycle and schemas,
pagination, read-only previews, message framing, an actual MCP subprocess
initialization/context exchange, icon integrity and existing palette behavior.
Headless pointer-event tests also verify connector drag/release, curve
drag/release, incompatible-drop rejection and persisted routing semantics.

Remaining acceptance:

- Open the desktop to review port tables, curved connections, dragging,
  connection preview, hover feedback and per-input disconnect in both themes.
- Run the Python worker unit tests and optional real-interpreter subprocess test.
- Exercise real Python timeout, process termination and malformed responses.
- Connect an external MCP client/Inspector and run the AI evaluation fixture.

The test suite launches the MCP binary, but does not establish a live AI client
connection or provide visual confirmation of the graph canvas.

Integration with the latest spatial desktop preserves levels, grids, walls and
MEP geometry. Snapshot roundtrip tests verify derived meshes and reject invalid
round-pipe dimensions and unsupported grid edits before applying a plan.
