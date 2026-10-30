# G-P12 Views on sheets placed at the middle or at the click (V7)

- **Gap**: Views on sheets placed at the middle or at the click (V7)
- **Class**: 3 (fix + new feature on existing sheet architecture)
- **Repos**: revit-alike `apps/desktop` (`src/sheet_draw.rs`, `src/sheet_ui.rs`)
- **Acceptance**: sheet test
- **Owner**: Wilfrid

## INTAKE

### Register entry (verbatim)
> **G-P12** | Views on sheets placed at the middle or at the click (V7) | 3 | sheet test

### Source reference: V7 visual review
From `docs/audits/2026-10-26-visual-review.md` line 33:

> | V7 | A view placed on a sheet goes to the top-left corner, not the middle the code comment says; the plan is small on A1 at 1:100 | Low | **Open**: place at the middle, or where the user clicks |

### Revit behaviour reference
From `docs/reference/revit-books/mep-part3-ch7-10.md` line 300:

> Add view: View > Sheet Composition > View (dialog Views list > Add View to Sheet) or drag a view from the Project Browser onto the sheet; the **viewport rectangle follows the cursor; click to place**. The view's scale is a property of the VIEW (View Scale in Properties); one view can be placed on only one sheet.

From `docs/reference/revit-books/arch-part3-ch11-16.md` line 252:

> Interaction: Place View opens a Views dialog; select a view, "Add View to Sheet"; a **rectangle attached to the cursor; click to drop**. Viewport label (view name, scale, view number) appears bottom-left.

### Current state

**`default_place()` in `sheet_draw.rs` (line 675-692):**
- Doc comment: "Where a view goes when it is put on a sheet: the first free place in the drawing area, looking along the rows from the top left (**the middle of the area when there is none**), clear of the views already there."
- Implementation: searches from top-left `(l + pad, t + pad)` in a 5 mm grid. When a free cell is found, returns it. Falls back to center `((l+r)/2, (t+b)/2 - 7)` **only when no free cell exists**.
- **Bug**: on an empty sheet (no existing viewports), the first free cell is the top-left `(l + pad, t + pad)`, NOT the middle. The function never returns the middle for the first view.

**`add_view_to_sheet()` in `sheet_ui.rs` (line 224-237):**
- Calls `default_place()` to get the position, then calls `place_viewport_commands(sheet, key, x, y)` to put the viewport at that position.
- There is no option to place at a user-clicked position.

**`show_sheet()` in `sheet_ui.rs` (line 440-557):**
- Handles click-to-select viewports (line 544-546) and drag-to-move viewports (line 518-542).
- No click-to-place mode for new views.

**Existing test** (`sheet_ui.rs` tests, line 606-662):
- `views_go_on_a_sheet_once_move_come_off_and_the_whole_is_one_undo_each` — tests add/move/remove/activate/delete cycle.
- Does NOT verify the placement position (middle vs top-left).
- Does NOT test click-to-place.

### Requirement
1. Fix `default_place()` so that when there are no existing viewports, the first view is placed at the center of the drawing area (matching the doc comment).
2. Add click-to-place: after selecting a view in the "Place View" dialog, show a ghost viewport outline attached to the cursor on the sheet; clicking drops the viewport at that position. Alternatively, keep the "Add View to Sheet" button for middle-placement and add a "Place on Sheet…" mode for click-placement.
3. Write a sheet test verifying: (a) a view on an empty sheet lands at/near the center of the drawing area, and (b) a view can be placed at a clicked position.

### DESIGN (contract)
- **File**: `apps/desktop/src/sheet_draw.rs` — fix `default_place()` to return the center when `existing` is empty.
- **File**: `apps/desktop/src/sheet_ui.rs` — add a `placing: Option<(sheet_id, view_key)>` state to `SheetUi`; in `show_sheet`, when `placing` is active, render a ghost outline at the pointer position and on click place the viewport there. Add a "Place at click" button in the Place View dialog.
- **File**: `apps/desktop/src/sheet_ui.rs` tests — add assertions for center placement and click placement.
- **No core changes**: all in `apps/desktop`.

### Status
- INTAKE done. DESIGN done (draft). Awaiting owner approval to proceed to IMPLEMENT.
