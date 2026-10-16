# Native OpenBIM Automation: foundation

Independent Rust engine, external preview runner and egui workspace. No Dynamo, DesignScript, Revit or
.NET dependency. This increment operates on the existing in-memory sample
document; geometry creation and full workflow parity are future work.

## Try it

Run `cargo test -p openbim-automation -p openbim-document -p openbim-desktop`, then
`cargo run -p openbim-desktop`. Open Manage > Automation > Graph Editor (also
available in Add-Ins and the Automation workspace). The example reads the sample
duct's elevation and proposes setting it to 3 metres. Preview, inspect Proposed
changes and Watch outputs, then Apply changes. Ctrl+Z restores the previous
document state. Repeating the same change creates no new transaction.

Drag headers, add nodes, connect output to input, and right-click headers to
delete. Player hides the canvas. Copy/paste JSON provides graph interchange;
native file dialogs and automatic saving are not implemented.

## Interaction design and current limits

Blocks use input ports on the left and output ports on the right, separated by
dark lines. Drag an output to a compatible input and release. Each input takes
one source; a valid replacement affects only that input. One output may feed
many inputs. Current node kinds each expose one output port: distinct multiple
output ports remain future work and must be addressed explicitly in graph schema,
validation, execution and MCP before adding such node kinds.

Edit numeric values, properties, Python expressions and element IDs inside the
block. The existing top-level controls remain available, including in Player.
Drag headers to move blocks and blue curves to reroute wires. Routing currently
uses one waypoint; automatic obstacle avoidance and multiple waypoints are not
implemented. Save routes through graph JSON; automatic file saving is absent.

See [wire interactions](automation-wire-interactions.md). Revit/Dynamo-like
interaction is the usability reference; complete feature parity is not claimed.
The shared command registry, visual model preview, graph debugger and live central
transactions from the original architecture remain targets. Current automation
uses the host Document transaction validator and does not bypass it.

## Execution contract

Schema version 1 contains node IDs, typed node kinds, layout and wires.
Validation rejects missing/duplicate inputs, type mismatches and cycles.
Evaluation uses deterministic dependency order. Reads observe the pre-run
snapshot; Set nodes propose changes and output references to the same elements.
Scalar quantities broadcast; quantity lists must match the element count.
Quantities use metres or dimensionless rise/run. Selected IDs are document-local
and deduplicated. No implicit mm/percent conversions occur.

Preview validates proposed commands against a clone using the host transaction
validator. An opaque plan retains its exact snapshot. Apply rejects a stale or
different document and executes the complete batch atomically, with one undo
checkpoint. No-op plans do not create transactions. Width and height writes
compose; competing writers fail. Redo awaits support in the document core.
For spatial duct and pipe elements, slope updates the derived mesh through the
host spatial model. Logical MEP connector/fitting propagation is not implemented. Preview runs
on a background thread with Stop/cancellation. Python expression nodes run in
a separate process when enabled by the host. See automation-runner.md.

Graph Editor and Player icons are vector primitives rasterized at the physical
display resolution with light/dark colours. Existing approved art is unchanged.

## Next increments

1. Separate graphical editor and authenticated live IPC. The standalone Rust
   runner now supports versioned immutable snapshot requests and preview reports;
   it does not apply reports to a live host. A local read-only MCP server for AI
   queries and previews is now included; see mcp-server.md.
2. Full Python scripting and packages with explicit permissions. The current
   external worker supports typed numeric expressions with cancellation and
   process deadlines. Python cannot mutate document memory directly.
3. Search, canvas pan/zoom, graph undo/redo, viewport-based selections, file dialogs,
   diagnostics, preview highlighting and persistent graphs.
4. Filters, nested lists/lacing, explicit conversions, expressions and subgraphs.
5. Persistent project IDs and creation bindings; geometry, architecture,
   structure, MEP and documentation nodes as host APIs mature.
6. Packages, integrations, batch/CLI and a capability verification register.
   Existing .dyn, DesignScript and Revit packages are outside required scope.

## Verification

Integration tests cover read-only preview, apply/undo, idempotence, stale plans,
types, cycles, invalid dimensions, unsupported properties, JSON/versioning and
composition of independent dimension writes. Run the Cargo tests above with
registry dependencies available. Visual acceptance requires opening the desktop
and checking both themes and display scales.
