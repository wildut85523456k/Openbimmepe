# Multiple named ports — first library foundation

Implemented in the native Rust runtime, egui canvas, external preview reports and
MCP catalog/schema. This increment does not add a general Python editor or package
installer.

## Try it

Open Manage > Automation > Graph Editor. Add Selection, Dimensions and Set blocks.
Connect Selection's elements output to Dimensions' elements input and to Set's
elements input. Connect Dimensions' height output to Set's width input. Preview
shows the height proposed as width. Apply commits the batch; Ctrl+Z undoes it.
Use a rectangular duct for this example: round pipe width/height must remain equal.
The original example can be reused by replacing its Read block with Dimensions
in graph JSON and connecting output 1 instead of the Length block.

Read dimensions has one elements input and three named Length outputs:

| Output ID | Name | Unit |
|---|---|---|
| 0 | width | metre |
| 1 | height | metre |
| 2 | elevation | metre |

An output may feed many inputs; each input accepts one source. Socket routing,
highlighting and dragging identify both node and output ID. Watch outputs shows
individual node/output pairs.

## Schema and compatibility

Wire `output` is an optional u16 ID, default 0. It is omitted when zero to preserve
older single-output graph JSON. Existing input IDs remain their fixed indices.
IDs are stable within a node kind and must not be renumbered when labels change.
Removed IDs must be rejected, never silently redirected. The current built-in
output rows use the same fixed ID order; dynamic/reorderable ports remain future
work. Unknown output IDs fail graph validation before execution.

This reader loads existing version-1 graphs. Older applications cannot load the
new Dimensions node or nonzero output field; upgrade before sharing these graphs.
The Dimensions kind serializes as the string `"Dimensions"`.

The preview runner retains legacy `values` (output 0 per node) and adds `outputs`,
a list of `{node, port, value}` records containing every output. MCP accepts
Dimensions and explicit wire output IDs; it still previews without model writes.

## Next increments

1. General Python block editor opened by double-click; familiar IN/OUT convention.
2. Parse typed function signatures without executing code; generate named ports.
3. Persist dynamic port IDs and offer explicit port updates with connection review.
4. Reusable graph blocks, local library and versioned GitHub packages.

Python currently remains the restricted one-input/one-output expression worker.
Automatic Python port discovery and arbitrary custom block definitions are not
implemented by this increment.

## Verification

Regression tests cover distinct output values, output-specific drag/release,
fan-out, invalid IDs, legacy JSON, graph roundtrip, atomic apply/undo and external
preview output serialization. Visual acceptance in the running desktop remains
required.
