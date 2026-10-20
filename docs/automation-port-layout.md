# Automation port layout update

Each node now contains a two-column port table beneath its header: inputs on the
left, output on the right. Dark horizontal separators divide port rows and a
vertical separator divides inputs from outputs. Set nodes have separate elements
and property-value input rows. Each attachment tab sits inside its row at the
node edge; empty ports are outlined and connected ports are filled blue.

Connections use cubic Bezier curves, with horizontal tangents leaving an output
and entering an input. Node height, socket positions, clickable rows and wire
endpoints all come from the same layout functions. Dragging moves wire endpoints
in the same frame. Wires are painted behind the node bodies.

Drag an output row to a compatible input row and release to connect. Click-click
also works. While connecting,
a blue curve follows the pointer; compatible inputs highlight blue and invalid
inputs red. Pressing Escape cancels.
Right-click an input row to disconnect only that input. Replacing a connection
preserves the existing one-input/one-source rule. The whole port row is a hit
target and its tooltip reports the port's quantity/element type.

An output supports multiple outgoing wires; each input has one source. Read dimensions exposes three separate outputs: width (0), height (1), elevation
(2). Other current node kinds have one output. Connections retain these fixed IDs
independently of display names. See [multiple ports](automation-multiple-ports.md). Block footers provide inline numeric, property,
expression and element-ID editors. Socket hit targets include the outside edge;
drop detection uses pointer geometry rather than widget hover ownership.

Existing graph positions and model transaction semantics are preserved. Manual
wire routes use an optional layout field; see automation-wire-interactions.md.
Light and dark themes have
separate separator, body and wire colours.

Verification: desktop Cargo check and the automation/desktop test suite passed
(historical full-suite result: 49 successful tests, one optional Python test
skipped; latest connector/inline-edit change: 13 desktop tests passed), along with
whitespace and reverse patch application checks. The egui 0.36 CubicBezierShape
signature was checked against its published API documentation. Visual acceptance
still requires opening the desktop to review both themes, multi-input Set nodes,
dragged nodes, connections in both directions and per-input disconnect.
