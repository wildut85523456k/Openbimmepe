# Block presentation and selection

Graph mode presents parameter controls inside blocks; the duplicate per-node
control list is hidden. Player retains those controls when its canvas is hidden.
Inputs stay on the left, named outputs on the right, and blue borders mark selected
blocks. Ports, parameter editors and sliders keep their own gestures.

## Selection and movement

- Click a block header to select only that block.
- Ctrl-click a header to add/remove that block from the selection.
- Drag empty canvas from left to right to select blocks completely inside the
  rectangle. Drag right to left to select blocks that intersect it.
- Hold Ctrl while starting the rectangle to add to the existing selection.
- Click empty canvas to clear selection; Escape cancels a rectangle and restores
  its previous selection.
- Drag a selected header to move all selected blocks together. Dragging an
  unselected header selects and moves that block. Movement is bounded at the
  canvas origin as a whole, preserving spacing.
- Right-click a selected header to delete the selection; unrelated blocks remain.
  Removing blocks also removes their wires and group memberships.

Click a group title to select its members; Ctrl-click adds/removes all members.
Drag a group title to move its member blocks. Ctrl+G groups selected ungrouped
blocks. Group organization does not turn the members into an executable custom node.

## Editing

Double-click a Python block header, or choose Edit Python expression from its
context menu, to open a larger editor with its quantity type. This edits the
existing restricted numeric expression using x; it is not yet a general Python
script editor and does not generate ports from code. Changes invalidate previews.
Other current block values are edited directly in their footers.

Custom-node internal workspaces, arbitrary Python blocks, nested groups and
graph undo/redo are still future work. Graph JSON retains layouts and group
memberships; transient selections and open editors are not serialized.

## Verification

Headless mouse tests cover rectangle selection, moving two selected blocks,
Ctrl-click selection, opening the Python editor, connector drag/release, wire
rerouting and group movement. GitHub CI still runs release tests, strict Clippy,
dependency audit and IFC integration checks.
