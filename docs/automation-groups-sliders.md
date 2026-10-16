# Visual groups and slider inputs

## Groups

Click a block header, then Ctrl-click other headers to select multiple blocks.
Click Group selected or press Ctrl+G. Groups are colored organizational containers,
not executable custom nodes. Drag the group title to move all members together;
wire endpoints follow. Right-click the title to rename, change color, add/remove
selected blocks, or ungroup while retaining blocks and connections.

Groups fit their members automatically. This first increment supports flat groups;
nested groups, collapse/expand and marquee selection are not implemented.
Deleting a block removes its group membership and deletes an empty group.

## Sliders

Add Length slider (metres) or Number slider (dimensionless/ratio). Drag its slider
or edit its displayed value. Expand Min / Max / Step to configure the range and
step and switch the output quantity. Each slider has one output and no inputs.
Connect it to a compatible value input. Sliders remain editable in Player.

Changing slider settings invalidates the preview. Preview evaluates the graph;
Apply commits the proposed changes and Ctrl+Z undoes the model transaction.
There is no automatic model modification while dragging a slider.

Min must be below Max, Step must be positive, and the value must lie in the range.
All numbers must be finite; invalid settings are rejected before execution.
The slider UI steps values; exact graph JSON values are preserved on load.

## Persistence and compatibility

Graph JSON stores an optional groups array (ID, title, member IDs, RGB color) and
Slider node parameters. Old graphs default to no groups. Visual groups do not
affect graph evaluation. A block belongs to one group; IDs and members are checked.
New graphs using these fields require an updated application. Save/load remains
the Graph JSON copy/paste workflow rather than automatic project persistence.

## Verification

Tests check grouping roundtrip, unchanged computation, group dragging, slider
output, invalid ranges/steps and invalid membership. Desktop visual acceptance
remains necessary. CI uses strict Clippy; passing local tests alone is not a claim
that GitHub Actions passed.
