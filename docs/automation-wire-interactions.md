# Wire interaction update

## Connect blocks

Hold the left mouse button on an output connector/row, drag onto a compatible
input connector/row, then release. The blue preview wire follows the pointer.
Inputs highlight blue when compatible and red when incompatible. Releasing on
empty space, the source itself or an incompatible input cancels without replacing
an existing connection. Escape cancels. The earlier click-output/click-input
gesture remains supported.

## Reshape an existing connection

Drag the blue curve, or its circular route handle, then release. Its endpoints
remain attached to the same ports. Double-click the curve to restore its automatic
route. An adjusted wire uses two smooth Bezier segments through the chosen route
point; output and input tangents remain horizontal.

The optional wire `route` field stores an integer offset from the port midpoint.
It is saved in graph JSON and moves with the midpoint when nodes are repositioned.
Older graphs without the field retain automatic routing. Routing has no effect
on graph evaluation, units, dependencies or the model. Its coordinate limits are
validated; the MCP graph schema includes this optional visual field.

## Execute the link

Connections are functional data links, not only lines. Execution is manual:
Preview evaluates node outputs and computes proposed changes. Watch outputs
shows those values. Apply changes commits a valid preview as a host transaction.
Continuous evaluation and automatic application are not enabled.

## Verification

Headless egui pointer-event tests exercise drag/release connection creation,
drag/release routing, and rejection of incompatible destinations. A graph
roundtrip test checks route persistence, old-file compatibility and unchanged
preview results. Full automation/desktop tests are run before packaging.
