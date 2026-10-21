> Evidence file for `docs/gap-analysis-2026-10-21.md`. Written by a review agent by reading the code at `main` @ `dcf8382`; the agent did not run the app. The consolidated report lists which findings were re-checked independently (section 12). Line numbers refer to that commit.

# Architectural mouse-only journey audit: revit-alike (apps/desktop)

Auditor's scope: read-only code review of /home/claude/revit-alike (single git commit dcf8382, branch main). Nothing was compiled or run (another process was building; the machine has 2 cores). Every statement about runtime behaviour below is therefore **from reading code and tests, NOT executed**. Where I say "WORKS" I mean "wired end to end in code AND covered by a test that I read"; I did not see any test go green. Line numbers are in the checked-out tree. Paths are relative to `apps/desktop/src/` unless they start with `crates/` or `docs/`.

Status vocabulary: WORKS (wired + test cited) / PARTIAL (what is missing is stated) / PLACEHOLDER (button exists, no behaviour) / ABSENT (nothing).

---------------------------------------------------------------------------------------------------

## 0. Headline findings (read these first)

1. **The mouse-only journey from a NEW project is blocked at the first sketch element.** `New` (quick-access button) gives a truly empty `Document` (file.rs:99-104). In a plan view, `model_view::show` computes `bounds` only from element meshes and grid ends, then does `if !bounds.is_finite() { return; }` (model_view.rs:143). Levels have no mesh and do not extend `bounds`. Every tool branch (Wall, Grid, Level click, Room, Slab, Family ...) and the Select tool are located AFTER that line (first tool branch is model_view.rs:482/489; Select is :1008). So in an empty or level-only project the plan canvas accepts no clicks at all. The only things that work with the mouse are the options-bar buttons: "Place Level" (panels.rs:365) and, as a workaround that creates a mesh, "Place Beam" with typed coordinates (panels.rs:695, model_edit.rs:343). No test starts from an empty document and draws (all 38 viewport tests start from `Desktop::sample()`). Severity: blocker for "from scratch". (Evidence is code reading; `Rect::NOTHING.is_finite()` is false per emath-0.36.2 rect.rs:55,524.)
2. **Levels/views are hard-coded to the sample.** The plan tabs "Level 1" and "Level 2" are always listed and mapped to `ElementId(10)` / `ElementId(11)` (model_view.rs:68-75, model_edit.rs:315-317, panels.rs:941-948 and :1003). A new project has no element 10/11. Levels the user creates get ids 1, 2, 3... and appear as extra tabs; the two dead tabs stay. A wall drawn in the "Level 1" tab of a new project is attached to a level that does not exist, and IFC export then fails with "a wall stands on a level that is gone" (export.rs:123). Element ids 10 and 11 also get special treatment if the user's own elements happen to receive those numbers (they are filtered out of the extra-tab list at model_view.rs:68 and panels.rs:1003, and View::Plan resolves to them).
3. **Esc behaves differently in the real app and in the tests.** `Desktop::ui` runs `if Escape pressed { choose_tool(Select) }` every frame BEFORE the viewport (main.rs:567). Tools were written for "Esc ends the chain, Esc again leaves the tool" (e.g. model_view.rs:562-563 wall, :631-632 stair, :485 cable, :846...) and the tests assert it (model_view.rs:1329), but the tests call `show()` directly and never run `Desktop::ui`. In the real app the first Esc already leaves the tool and clears the chain/points. The docs claim the two-stage behaviour (docs/revit-ux-parity-audit.md "Wall tool vs Revit": "Esc ends chain, Esc again leaves tool: done").
4. **Massing, ramps, curtain walls, areas, model lines, opening by face/dormer, "Load Family", Save As, project units/info, room naming, level/grid renaming, floor pick-walls/slope, wall top-constraint/attach: absent or placeholders** (details below). "Massing & Site" ribbon tab contains one button, "Site Component", which is the same command as the fan-coil "Component" (ribbon.rs:153, main.rs:464).
5. **IFC export drops rooms, grids and the site** from the desktop path even though the IFC crate can write IfcSpace: `rooms: vec![]` (export.rs:291), Grid/Room/Level/Opening categories skipped (export.rs:198), `georef: None` (export.rs:293), site warning (export.rs:432). The app's own records carry them in the file for re-import ("own data" checkbox, export.rs:293), but another BIM tool will not see rooms or grids. Project name is fixed "Project1" and project key fixed 1 (model_edit.rs:305-306).
6. **State leaks between tools** produce wrong values without warning: `slab_offset_mm` / `slab_thickness_mm` are shared by Floor, Roof, Ceiling, Door/Window/Component ("Offset") and footings (main.rs:450,458-459; panels.rs:482-483). After using Ceiling (2700 mm offset, 20 mm thick) the Floor button leaves 20 mm / +2700 mm, and the Door button places the door 2.7 m up (model_edit.rs:568 passes `slab_offset_mm` as elevation).
7. **Tests drive only the central canvas.** The simulated-event harness (`frames()/frame()/button()/key()`, model_view.rs:1299-1306) runs `model_view::show` inside a `CentralPanel`. Ribbon, options bar, side panels, global shortcuts (Esc, Tab, Ctrl+Z/Y) and native file dialogs are never driven by events; they are covered only by direct calls such as `app.run_command("wall", ..)`. That is why findings 1-3 are not caught.
8. Positive: a lot is genuinely wired and well tested at the model level: walls (types, layers, location line, joins, 7 draw modes), doors/windows hosted in walls, floors/roofs/ceilings, stairs (straight, L, U with railing), railings, shaft/vertical/wall openings, rooms from wall faces, plans/sections/elevations, schedules, sheets, PDF print, IFC export (levels as IfcBuildingStorey, walls with layer sets, doors/windows with openings, slabs, roofs, stairs, railings, coverings) and native save/open (.obim).

---------------------------------------------------------------------------------------------------

## 1. Step-by-step tables

### 1.1 New project, template, units, default levels

| Revit step | What the user does | Status | Evidence | Gaps |
|---|---|---|---|---|
| App start | Launches app | PARTIAL (opens a SAMPLE model, not a project) | main.rs:282 `Document::spatial_sample()`; crates/document/src/spatial.rs:648-679 (Levels "Level 1" id 10 at 0 m, "Level 2" id 11 at 3 m, grids A,B,C,1,2,3, five walls, a duct, a pipe, site lat/long Phnom Penh). Starts in `View::Mechanical`, ribbon "Systems" (main.rs:287-288). Toolbar says "Project1 (SAMPLE)" (main.rs:383) | No start page, no recent files, no command-line file argument (main.rs:663-670) |
| File > New Project (template) | Clicks "New" quick-access icon | PARTIAL | ribbon.rs:46-48 QUICK_ACCESS; main.rs:473 `"new" => new_model()`; file.rs:94-104 (asks "Unsaved changes" if dirty, then `Document::new()`); test file.rs `opening_or_starting_over_with_unsaved_changes_asks_first` (model count 0 asserted) | No template choice, no project name/number/address (Project Information), no units dialog (units fixed mm/m2/m3; docs/revit-book-review.md A14), no default levels, no default view set. View/tool/ribbon state NOT reset (file.rs:99-104 resets document, file state, selection only). Status bar still says "Workset: Mechanical (sample)" (main.rs:597) and Properties footer "In memory / not saved" (panels.rs:927) |
| Default levels in new project | Nothing | ABSENT | `Document::new()` = default (empty) | User must create all levels; see 1.2 |
| Save/Open | see 1.12 | | | |

Verdict: **with workaround.** New works. The result is an empty project with dead "Level 1/Level 2" tabs and a canvas that does not respond (finding 1).

### 1.2 Levels

| Revit step | What the user does | Status | Evidence | Gaps |
|---|---|---|---|---|
| Create levels | Architecture > Datum > Level, types elevation (mm) and optional name in the options bar, presses "Place Level" (or clicks anywhere in a plan) | PARTIAL | main.rs:446 (default elevation = highest level + 3000, 0 for none); panels.rs:357-367 (options bar, "Place Level" button); model_edit.rs:1008-1020 `level_click` -> `Document::place_level` (crates/document/src/lib.rs:689-702); view switches to the new level plan. Test: model_view.rs:1451 `grids_are_drawn_named_in_sequence_and_levels_get_their_own_plan` (click path, duplicate elevation refused, wall on the new level; sample start) | Click position is ignored (the click only confirms; message text says so, main.rs:366). **Click path is dead in an empty project (finding 1); button path works.** No level drawn in an elevation, no level lines in plans, no bubbles. No uniqueness check of typed names and no 100 mm minimum gap at placement (lib.rs:690 only refuses equal elevations within 1 mm); IFC export later fails with `DuplicateName` / `LevelsTooClose` (crates/arch/src/lib.rs:82-91, export.rs:289). Levels cannot be deleted (model_edit.rs:870), renamed (no UI; only `Rename` for Types, lib.rs:1240), or copied/monitored |
| Edit level elevation | Click the "Level: x.xxx m" button in a plan header (model_view.rs:102), Properties > Elevation, Apply | WORKS (no dedicated test of this path found) | panels.rs:98 `C::Level => Elevation`; model_edit.rs:1093 MoveElevation. Walls/slabs on that level follow because z is computed from the level at read time (export.rs:123) | Only reachable for the level of the open plan; level lines cannot be picked in elevations (docs/revit-ux-parity-audit.md Elevations row 85) |
| Plan view per level | automatic | PARTIAL | tab list model_view.rs:68-75; Project Browser panels.rs:1003-1008 | Extra levels listed as "X (plan)" below the tree. Hard-coded Level 1/2 tabs remain (finding 2). No "Create Plan View" dialog, no ceiling plan views (folder "Ceiling Plans" empty, panels.rs:945), no area plans |

Verdict: **with workaround** (use the "Place Level" button; ignore the Level 1/Level 2 tabs).

### 1.3 Grids

| Revit step | What the user does | Status | Evidence | Gaps |
|---|---|---|---|---|
| Grid tool | Architecture > Datum > Grid; click start, click end (or press-drag-release) | WORKS on a non-empty model; DEAD on an empty one | main.rs:423; model_view.rs:824 branch; model_edit.rs:998-1005 `grid_click` -> `place_grid` (lib.rs:~678); names count on (3->4, C->D, Z->AA; lib.rs:660-678). Test model_view.rs:1451 (clicks and drag, names "4","5") | Empty-project dead canvas (finding 1). Straight lines only: no arc grids, multi-segment, Pick Lines, no 2D/3D extents or bubble editing. Cannot rename a grid (no UI). Snaps: shared plan snap (endpoint, intersection, midpoint, perpendicular, nearest, 50 mm). Grids are not level-specific: drawn in every plan (model_view.rs:131-141). Not exported to IFC (export.rs:198) except in the app's own data records |
| Reference planes / work planes | | ABSENT | docs/revit-book-review.md A8 says so; no `ReferencePlane` in code | |

Verdict: **no for an empty project; yes after a beam workaround** (place any element with a mesh first).

### 1.4 Conceptual massing

| Revit step | What the user does | Status | Evidence | Gaps |
|---|---|---|---|---|
| In-Place Mass / Conceptual Mass / Building Maker (Wall/Floor/Roof/Curtain System by Face) | | **ABSENT** | `grep -ri "mass"` in apps/ and crates/*/src finds only the ribbon tab name (ribbon.rs:31,153,202) plus unrelated "mass" words in family/calc crates. Ribbon tab "Massing & Site" has one group "Model Site" with one button "Site Component" -> same handler as "Component" = fan-coil unit (ribbon.rs:153; main.rs:464). docs/revit-book-review.md A12: "Massing ... Missing, P3" | No mass category, no mass floors, no face-based creation, no toposolid/site (A11 missing) |

Verdict: **no** (and no workaround: design directly with walls/floors).

### 1.5 Walls (types, layers, location line, joins, constraints)

| Revit step | What the user does | Status | Evidence | Gaps |
|---|---|---|---|---|
| Wall tool, draw | Architecture > Build > Wall. Clicks start, then each next point (chain); or press-drag-release; Shift = ortho; type length + Enter; click first point to close; Backspace deletes a typed digit | WORKS on a non-empty model | main.rs:447 (resets to 200 x 3000, base 0, Custom type, flip off each time the button is pressed); model_view.rs:489-579; model_edit.rs:630-670 `wall_click` -> `place_wall_typed` (lib.rs:~593); chain meets at offset-line corners; closing loop moves first wall. Test model_view.rs:1309 `a_wall_is_drawn_by_two_clicks_as_a_chain_and_by_press_drag_release`; :1404 `walls_follow_the_location_line_chain_at_corners_and_close_the_loop` | Dead on empty project (1). Typed length uses the pointer direction (model_view.rs:538-543), as Revit. **Two-stage Esc is not reachable in the real app (finding 3).** No temporary dimension while drawing except length |
| Draw modes | Options bar "Draw": Line, Rectangle, Pick Lines, Inscribed/Circumscribed Polygon, Circle, Arc | WORKS (model level) | panels.rs:409-416; model_edit.rs:883-975; crates wall_shapes.rs (3 tests); test model_view.rs:1852 `walls_have_types_a_rectangle_pick_lines_and_the_spacebar_flip` | Curves are many straight walls (no true curved wall; docs A4). No Pick Faces, no ellipse/spline/fillet |
| Location Line | Options bar combo: Wall Centerline, Core Centerline, Finish Face Exterior/Interior, Core Face Exterior/Interior | WORKS | model_edit.rs:611 WALL_LOCATIONS; :617-625 `wall_axis`; crates/document/src/layers.rs:237-242 `location_shift`; test :1404 | Default Centerline (Revit default). No "Offset" field, no Radius field |
| Wall types | Options bar type combo; "Add type" (typed name + current thickness/height); "Edit Type..." | PARTIAL | panels.rs:417-440; crates/document/src/types.rs:153-164 `add_wall_type`; test type_editor.rs `a_plain_type_is_given_layers_and_applying_changes_the_type_its_walls_and_the_wall_tool_in_one_undo` | **No type exists in a new project**; the user must type a name to make one. Type selection is reset to "Custom" every time the Wall button is pressed (main.rs:447; Revit remembers the last type). Types are `Element`s with ids, so they consume ids (see magic ids, finding 2). Type Properties dialog has only Name, Height, Layers (no Function/Structural usage, Coarse Scale Fill, Wrapping at ends is per layer) |
| Layers / compound structure | Edit Type > "Edit Structure: add layers"; Insert/Delete/Up/Down; function, material, thickness, wraps; core boundaries | WORKS | type_editor.rs:100-228; crates document layers.rs (6 tests); type_editor.rs tests (3) | Material is a free-text name with a fixed list of 10 suggestions (type_editor.rs:9); no material library, colours, patterns. No layer priority / cleanup at joins (docs A3). No wall sweeps/reveals |
| Joins | Automatic miter (L) / butt (T); Wall Joins tool allows/disallows an end | PARTIAL | main.rs:428; model_edit.rs:743; test model_view.rs:2164 `wall_joins_tool_disallows_and_allows_the_join_at_a_wall_end`; crates/document/src/spatial.rs:707 `walls_are_joined_as_revit_joins_them` | No Join Geometry/Unjoin, no butt/miter/square-off choice. Corner volume overlaps are not removed from schedule quantities (docs "Not the same as Revit") |
| Base/top constraint, attach | Properties | **PARTIAL / mostly ABSENT** | A wall stores one `level` (base), `elevation_m` as Base Offset and a numeric `height_m` ("Unconnected height"). Properties shows "Reference Level" read-only (panels.rs:828-831) and Thickness/Height/Base Offset editable (panels.rs:87). `grep -i "top constraint\|attach"` finds only shaft/slab offsets | No top constraint "up to level", no top offset, no Attach Top/Base to roof/floor, no way to move a wall to another level, no Room Bounding flag in UI. Wall height does not follow level spacing: the default is 3000 mm everywhere (main.rs:447). Docs A2 say the same |
| Modify walls | Select, drag grips, Space to flip, temp dimensions, Move/Copy/Rotate/Mirror/Align/Offset/Trim | WORKS | model_view.rs tests :1699 grips, :2077 temp dimensions, :1579 move/copy/rotate/mirror, :1806 offset/align/trim | Split Element, Array, Scale, Groups absent |

Verdict: **yes with workarounds** (empty-canvas issue, type names typed, heights typed).

### 1.6 Floors / slabs

| Revit step | What the user does | Status | Evidence | Gaps |
|---|---|---|---|---|
| Floor tool | Architecture > Build > Floor; clicks corners; Enter, click first corner, or "Finish" button | WORKS (model level); mouse path tested only through the Roof twin | main.rs:450 (`"floor"` sets kind Floor only); model_edit.rs:399-405 `slab_click`, :477-547 `finish_slab` -> `place_slab` (lib.rs:803). Tests: model_edit.rs:1561 `slabs_are_drawn_by_clicks...` (direct `slab_click`), model_view.rs:2244 (mouse clicks + Enter through the same Slab tool, but as Roof) | **Boundary sketch only**: no Pick Walls, no Pick Lines, no rectangle/circle/arc sketch tools, no sketch mode with Finish/Cancel; no "Edit Boundary" after placement (Properties shows only corner count and area, panels.rs:857-861; `SetOutline` is only used at placement/import). Boundary is polygon only (must be simple; bow-tie refused). **Parameters leak from the previous tool** (finding 6) |
| Slope / shape edit | | ABSENT | `Category::supports_slope` = Beam, Duct, Pipe only (lib.rs:133). docs A6 | No slope arrow, no Add Point/Split Line, no span direction |
| Floor types / layers | | ABSENT | `Element::can_have_type` includes Slab (lib.rs:395) but a "type" is just thickness/material/kind made with "Make type" (panels.rs:247-258); no layers | |
| Openings | Architecture > Opening: Shaft, Vertical (floor/roof/ceiling), Wall | WORKS | model_view.rs:2651 `vertical_opening_picks_its_host_with_tab...`, :2708 `shaft_tool_sketches_an_outline_cuts_the_floor_above...`; model_edit.rs:2079 export of openings; crates opening.rs (7 tests) | A shaft cuts "by height", so a Level1->Level2 shaft cuts Level 2's floor, not Level 1's (docs "Opening" row; consistent with code intent). By Face and Dormer are PLACEHOLDERS (main.rs:495 message) |
| Structural / foundation slab | "Slab" button = Foundation kind | WORKS | main.rs:468 | Ribbon "Slab" lives on the Structure tab |

Verdict: **yes** (polygon sketch with clicks; no pick-walls shortcut; confirm the thickness/offset fields before drawing).

### 1.7 Doors, windows, components

| Revit step | What the user does | Status | Evidence | Gaps |
|---|---|---|---|---|
| Door / Window | Architecture > Build > Door or Window; choose family/type in options bar; click on a wall in a plan (or in 3D) | WORKS (model level, direct-call tests; 3D click path tested with an injected hit) | main.rs:462-463; model_view.rs:853-862 (plan click -> wall under the pointer); model_edit.rs:567-583 `family_click` -> `place_family_in_wall` (lib.rs:977); view3d.rs:237-246 (3D click). Tests: model_edit.rs:1858 `a_door_goes_into_the_clicked_wall_cuts_it_in_3d_and_is_exported_with_its_opening` (calls `family_click` directly), :1904 instance values, :1930 swing; view3d.rs:288 `a_click_in_the_three_d_view_places_a_door_in_the_wall_under_it` (injects `last_hit`); panels.rs `a_door_command_offers_only_door_families`. **No test drives the plan click on a door via simulated pointer events.** The door follows its wall (test :1892-ish "it follows the wall"). Cuts the wall in 3D and in IFC (IFCDOOR + IFCOPENINGELEMENT) | **Library is five families only** (crates/family/src/library.rs:310: handle, "Door, single flush", "Window, fixed with mullions", "Door, arched", "Fan coil unit, ducted"); one window family; no furniture. Types: "Default" plus user-typed catalogues, none shipped (crates/family/src/catalogue.rs:12). **No Load Family / file import UI** (only an icon id `load_family`, icons.rs:112; `catalogue` crate code is not used by the desktop). No hover preview or flip arrows (A5); swing is a Properties choice (hand left/right). Position is the click projected on the wall, clamped to 0..length, so a door can hang past a wall end without a warning (model_edit.rs:572-574). Sill = `slab_offset_mm` (finding 6). 3D click falls back to `ElementId(10)` as level (view3d.rs:241) |
| Component | Architecture > Build > Component | PLACEHOLDER-ish (only the fan-coil) | main.rs:464 `"ahu" \| "component"` -> "Fan coil unit, ducted" | No architectural furniture/fixtures |
| Column | Architecture > Build > Column | PARTIAL | main.rs:449 (structural column tool); model_edit.rs:359 requires a level above ("There is no level above...") | Not an architectural column family; test model_edit.rs:1203 uses direct `column_click` |
| Curtain Wall | | PLACEHOLDER | ribbon.rs:109-115 id `curtain_wall`; falls to default arm main.rs:495 "is not implemented in this UI prototype" | |

Verdict: **yes for doors/windows** (limited families), **no for furniture, curtain walls, family loading.**

### 1.8 Stairs, ramps, railings

| Revit step | What the user does | Status | Evidence | Gaps |
|---|---|---|---|---|
| Stair | Architecture > Circulation > Stair; options bar: Shape (Straight, L left/right, U left/right), Railing (Both/Left/Right/None), Width, Max Riser, Tread, Base Offset; click foot, click direction (or press-drag-release) | WORKS | main.rs:457; panels.rs:588-615; model_edit.rs:753-771 `stair_click` -> `place_stair_with_railing` (crates/document/src/stair.rs, stair_railing.rs). Tests: model_view.rs:2280 `stair_tool_draws_by_press_drag_release_and_shows_its_plan_symbol` (mouse); model_edit.rs:1472, :1511 (L/U, landing, export) | Risers = rise to next level / max riser; a level above is needed (else "Height" field); not editable afterward (place again). Stair by sketch, winder, landings by hand, supports: absent (docs A10). Monolithic stepped block in 3D |
| Ramp | | PLACEHOLDER | ribbon.rs:110-112 `ramp` unhandled (main.rs:495) | |
| Railing | Architecture > Circulation > Railing; clicks path points; Enter/Finish | WORKS | main.rs:452; model_edit.rs:507-514; test model_view.rs:2841 `the_railing_tool_sketches_a_path_by_clicks_and_enter_edits...` (mouse) | Free-standing on level (no host pick); one fixed pattern; no railing types |

Verdict: **yes** (stairs and railings); **no** (ramps).

### 1.9 Roofs

| Revit step | What the user does | Status | Evidence | Gaps |
|---|---|---|---|---|
| Roof by footprint | Architecture > Build > Roof; click eave corners; Enter/first corner/Finish; options: thickness, base offset, slope | WORKS | main.rs:459; model_edit.rs:532-539 -> `place_roof` (crates/document/src/roof.rs); select roof > per-edge "Defines slope" checkboxes (panels.rs:287-312). Test: model_view.rs:2244 `roof_tool_draws_by_clicks_and_a_roof_in_a_plan_lies_under_the_walls_it_covers` (mouse; final assertion is weak: `tool == Select \|\| tool == Slab`, :2273); model_edit.rs:1369; roof.rs (5 tests) | Footprint only: no Pick Walls, no extrusion roof, no overhang, no eave/ridge editing, no Attach Walls to Roof, no roof types, no dormer. Non-convex outlines only with all edges sloping |

Verdict: **yes** (simple hip/gable/shed/flat roofs).

### 1.10 Ceilings

| Revit step | What the user does | Status | Evidence | Gaps |
|---|---|---|---|---|
| Automatic ceiling | Architecture > Build > Ceiling (defaults 2700 mm / 20 mm); click in a closed room | WORKS (direct-call test) | main.rs:458; model_view.rs:910 hover outline; model_edit.rs:460-474 `ceiling_auto_click`. Test model_edit.rs:1433 `a_ceiling_is_hung_in_the_space_under_the_click...` (calls `slab_click` directly) | Outline is copied at placement, does not follow walls; columns are not cut out; no ceiling types, no ceiling plan view, no grid. Leaves `slab_offset_mm=2700` behind (finding 6) |
| Sketch ceiling | untick Automatic, click corners | WORKS | panels.rs:646 | |

### 1.11 Rooms / areas

| Revit step | What the user does | Status | Evidence | Gaps |
|---|---|---|---|---|
| Room | Architecture > Room & Area > Room; hover shows the closed space + area; click places | WORKS (mouse test) | main.rs:427; model_view.rs:634-652; crates/document/src/rooms.rs:132-144 `place_room`; test model_view.rs:2187 `room_tool_places_a_room_in_a_closed_space_shows_its_area_and_follows_the_walls` | Boundary = wall faces and columns only. Name is "Room N" and cannot be edited; no Number, no Department, no Room Tag element, no Room Separation Line, no Place Room Automatically, no Tag All (docs Room table + D4). "Not Enclosed" is a display state |
| Area | Architecture > Room & Area > Area | PLACEHOLDER | ribbon.rs:112 `area` unhandled | |
| Space/zone | | ABSENT | docs/revit-book-review.md M7 | |
| IFC | | ABSENT in desktop export | export.rs:198,291 | see finding 5 |

### 1.12 Openings (see 1.6) — WORKS for Wall, Shaft, Vertical; By Face and Dormer PLACEHOLDERS.

Wall Opening in a plan: click one end then the other on the same wall, with sill/head heights from the options bar (model_edit.rs:444-457; panels.rs:576-587; test model_view.rs:2708 and :2337 for elevation sketch).

### 1.13 Views, sections, elevations, schedules, sheets

| Revit step | What the user does | Status | Evidence | Gaps |
|---|---|---|---|---|
| Floor plan per level | Tabs / Project Browser | PARTIAL | see 1.2 | |
| Section | View > Create > Section; two picks in a plan | WORKS (mouse test) | main.rs:453; model_view.rs:653 branch; test :2302 `section_tool_draws_a_line_in_a_plan_and_its_view_and_the_four_elevations_show_the_building`; :2619 head bubble/flip | No split segments, crop, section box in sections |
| Elevations | four fixed + Elevation tool markers | WORKS (mouse test) | model_view.rs:663 branch; test :2554 `the_elevation_tool_puts_a_marker_with_views_that_switch_on_and_off_open_and_move` | The four building elevations are always listed (panels.rs:945-947) even in an empty model; computed from the building bounds, no markers in plans you can delete |
| Elevation/section editing | Select/Move/Copy/Rotate/Mirror/Align/Tag/Dimension/Wall Opening | WORKS | tests :2337, :2390, :2459, :2500, :2767 | Draw tools (Level, Grid, Wall...) say "not available in an elevation or a section yet" (docs row 102) |
| 3D view | {3D} tab, orbit, ViewCube, section box | WORKS for viewing and placing doors; no drawing tools | view3d.rs; docs/3d-view-parity-plan.md (the doc says every tool is plan-only; code in view3d.rs:237 shows Door/Window/Component placement by click in 3D, so the doc is stale) | Perspective only; no camera, no walkthrough |
| "View" ribbon button (new plan/3D/duplicate/callout) | | PLACEHOLDER | ribbon.rs `c("view","View")` unhandled; Pan/Orbit/Find also unhandled | |
| Schedules | View > Schedules/Quantities | WORKS | schedule_ui.rs tests (2), crates/document/src/schedule.rs (9) | Rooms schedule has no Number; Material takeoff absent |
| Sheets/print | View > Sheet / Place View / Print | WORKS | sheet_ui.rs tests (3), pdf.rs (6), print_dialog.rs (2), plan_export.rs (3) | 3D on a sheet is a fixed isometric |

### 1.14 Save / open / export IFC / undo-redo

| Revit step | What the user does | Status | Evidence | Gaps |
|---|---|---|---|---|
| Save | Quick-access Save: first time a native file dialog (rfd) "model.obim", later overwrites | WORKS | file.rs:41-72 (temp file + rename); test file.rs:140 `a_saved_model_opens_equal_with_no_undo_and_a_bad_file_changes_nothing` | **No Save As** reachable: id `save_as` is handled (main.rs:476) but no button has it (ribbon.rs:46-54). No Ctrl+S/Ctrl+N/Ctrl+O (grep of `Key::S` etc. finds nothing). No autosave / crash recovery / backup (grep). The crates `oplog` and `sketch` are not used by the desktop (Cargo.toml deps); persistence is a text file of records (crates/document/src/native.rs:130-136) |
| Open | Quick-access Open (native dialog) | WORKS | file.rs:75-91 | Replaces the model; view/ribbon state not reset |
| Export IFC | Insert > Link > Export IFC: window with path (typed or Browse), schema (IFC 4.3 default, IFC4, IFC2X3), "Include this app's own data" (default ON) | WORKS | model_edit.rs:233-265; export.rs:427-434. Tests model_edit.rs:1957 `the_export_window_offers_ifc_4_3_first_and_writes_the_schema_picked`; export.rs tests; crates/ifc/tests | Default path is relative "openbim-export.ifc" (CWD). Project name "Project1", key 1 constant (model_edit.rs:305). Fails (Err) if a wall/slab/etc. stands on a missing level (export.rs:123,136,145,151,163,170), if two levels share a name or are <100 mm apart. Not exported: rooms, grids, site (see finding 5), tags, dimensions, sections, views, schedules (except as app records) |
| Import IFC | Insert > Link > Import IFC | PARTIAL | import.rs: foreign files give levels, walls, beams, columns, slabs, footings, ducts, pipes; doors/windows/stairs/rooms from other programs are not read (import.rs:226 skip). Test model_edit.rs:2056 | Roundtrip of own files is exact via embedded records |
| Undo / Redo | Quick-access buttons and Ctrl+Z, Ctrl+Y / Ctrl+Shift+Z | WORKS | main.rs:578-582; lib.rs:1275-1296; crates tests `redo_puts_back_what_undo_took_and_a_new_edit_ends_it` (lib.rs:1331) | Snapshots of the whole element map per step (lib.rs:1267) with no cap; each edit validates all elements (lib.rs:1262-1263), O(model) per click. Undo does not restore view/selection: e.g. undo of a just-made level leaves `View::Level(gone)` which `plan_level` resolves to `ElementId(10)` (model_edit.rs:316) |

---------------------------------------------------------------------------------------------------

## 2. Mouse-only journey verdict per step

| # | Revit step | Can a user complete it with the mouse only? |
|---|---|---|
| 1 | New project | Yes (button) but no template/units/name; result is an empty model with dead Level 1/2 tabs |
| 2 | Levels | With workaround: "Place Level" options-bar button (type elevation first, drag-value or keyboard); cannot rename/delete |
| 3 | Grids | **No** in an empty project (canvas dead). Yes once any mesh element exists (workaround: options-bar "Place Beam" with typed coordinates, then draw, then delete the beam) |
| 4 | Massing | No (absent) |
| 5 | Walls | Yes after (3) is bypassed; types need a typed name; heights typed; no top constraint |
| 6 | Floors | Yes (corner clicks); no pick-walls; check Thickness/Top Offset first |
| 7 | Doors/windows | Yes (limited library, one window family); no family loading |
| 8 | Stairs/railings | Yes; ramps no |
| 9 | Roofs | Yes (footprint) |
| 10 | Ceilings | Yes (automatic or sketched) |
| 11 | Rooms | Yes to place; cannot name/number; areas no |
| 12 | Openings | Yes (wall, shaft, vertical); by-face/dormer no |
| 13 | Views/sections/elevations | Yes (section tool, elevation markers, schedules, sheets, PDF) |
| 14 | Save/export IFC | Yes (Save, Export window with Browse); no Save As; rooms/grids/site missing from IFC |

Overall: **not today** from a pristine New project; **probably yes** once the empty-canvas early return (model_view.rs:143) and the hard-coded Level 1/2 tabs are fixed, with the remaining gaps being Revit-parity features rather than blockers.

---------------------------------------------------------------------------------------------------

## 3. Direct answers to the questions asked

**(1) New project, template, units, default levels; levels/grids by mouse or hard-coded?**
- There IS a New command (quick-access icon `new`, main.rs:473; file.rs:94). It produces `Document::new()`: no template, no project info, no units dialog (units fixed), no levels, no grids, no views other than the hard-coded tabs.
- App launch loads `Document::spatial_sample()`, which has hard-coded Level 1 (id 10, 0 m) and Level 2 (id 11, 3 m) and grids A-C/1-3 (crates/document/src/spatial.rs:654-660).
- Levels can be created with the Level tool (options bar button works; click path dead in empty model). Grids by the Grid tool (click path dead in empty model). Hard-coded parts: tabs "Level 1"/"Level 2" and "M-101 - Mechanical" (model_view.rs:70-73), Project Browser tree (panels.rs:941-950), `plan_level()` fallbacks to ids 10/11 (model_edit.rs:316), 3D family placement fallback to id 10 (view3d.rs:241), ids 10/11 filtered out of the extra-tab lists (model_view.rs:68; panels.rs:1003). The audit doc admits "Level 1 and Level 2 are fixed tabs" (docs/revit-ux-parity-audit.md "Still missing for every element").

**(2) Massing.** None. No in-place mass, no conceptual mass, no Building Maker, no mass floors/faces, no site/toposolid. "Massing & Site" ribbon has a single button that re-uses the Component command (ribbon.rs:153).

**(3) Walls.** Location Line: yes, six choices incl. core lines (model_edit.rs:611-625). Types: yes, user-made (none shipped), plain or layered; Edit Type dialog with layers/functions/core/wraps (type_editor.rs). Joins: automatic miter/butt, per-end Allow/Disallow join; no join type choice, no Join Geometry, no layer-priority cleanup. Attach top/base: absent. Constraints: base level fixed at creation (read-only in Properties), base offset and numeric height only; no top constraint to a level; no Room Bounding toggle in UI.

**(4) Floors.** Sketch by clicking polygon corners, Enter/first-corner/Finish; thickness, top offset, material, kind in the options bar; no Pick Walls, no slope, no shape editing, no boundary editing after placement, no floor layers; openings via Shaft/Vertical Opening tools (cut by height / by host).

**(5) Doors/windows.** Hosted in a wall (`Geometry::InWall{host, along_m}`), follow the wall, cut the wall in plan/3D/IFC. Library of 5 compiled-in families; "Load Family" absent (only an icon). Type catalogue code exists in crates/family but is not wired to the UI.

**(6) Stairs/ramps/railings, roofs, ceilings, rooms/areas.** Stairs (straight/L/U + railings) yes; ramps placeholder; sketched railings yes; footprint roofs yes; automatic/sketch ceilings yes; rooms yes (unnamed); areas placeholder; spaces/zones absent.

**(7) Save/open/export IFC, undo/redo.** Save/Open native `.obim` text records (no Save As, no shortcuts, no autosave). IFC export 4.3/4/2X3 with optional embedded app data; rooms/grids/site not exported; export hard-fails on dangling level or duplicate/too-close level names. Undo/redo via whole-state snapshots; Ctrl+Z / Ctrl+Y.

**(8) Tests that simulate mouse/keys.** See section 4.

**(9) Wrong or unusual Revit-parity behaviours.** See section 6.

---------------------------------------------------------------------------------------------------

## 4. Test harness and an end-to-end test

### 4.1 What exists

Totals: 1142 `#[test]` attributes in crates+apps (grep count), 169 in apps/desktop (grep count of files; model_view.rs 38, model_edit.rs 29, import.rs 8, view_ui.rs 9, mep_tool.rs 8, automation_canvas.rs 8, view3d.rs 7 ...).

**Harness A (the only true pointer/key harness for plans and elevations): model_view.rs:1299-1306 and :1981-1986.**
- `frames()` creates `egui::Context::default()` and a 1000x700 screen; `frame(ctx, screen, app, events)` runs `ctx.run_ui(RawInput{screen_rect, events}, |ui| CentralPanel::default().show(ui, |ui| show(app, ui)))`; `button(pos, pressed)` builds `Event::PointerButton`; `key(k)` builds `Event::Key`; `frame_mod` adds modifiers; `click()` (:2450) = move + press + release.
- It exercises `model_view::show` only (viewport + view tabs + control bar). It does not run `Desktop::ui` (main.rs:565), the ribbon (`Desktop::ribbon`), the options bar (`panels::options`), the side panels, or the global Esc/Tab/undo shortcuts.
- Tools are armed by calling `app.run_command("wall", ..)` etc. (a direct function call), not by clicking the ribbon button.
- Screen-to-model mapping: plan has no exported transform; tests use arbitrary screen points (e.g. 300,300) and then inspect the document, or `wall_span` (:1681) which brute-force scans the screen for a hit. Elevations/sections have `app.section_xf` (main.rs field `section_xf`) used by `spot()` (:2444).

Scenarios driven by simulated events in Harness A (fn line in model_view.rs): wall chain + press-drag (1309); beam/duct drag (1343); pipe with slope + Properties edit (1367); wall location lines, chain corners, loop close (1404); grids named in sequence + levels get plans + wall on new level (1451); tag (1499); delete with door and tag + undo (1526); paste with ghost (1547); move/copy/rotate/mirror (1579); move by drag, click-pick (1643); wall end grips (1699); section end drag (1737); drag element + Esc cancels (1770); offset/align/trim (1806); wall types, rectangle, pick lines, Space flip (1852); multi-select by Ctrl/box (1989, 2043); temp dimensions (2077); dimension/measure (2128); wall joins (2164); room (2187); roof (2244); stair (2280); section (2302); wall opening in elevation (2337); drag in elevation (2390); move/copy in view (2459); mirror/rotate/align in view (2500); elevation tool (2554); section heads/flip (2619); vertical opening (2651); shaft + wall opening (2708); tags/dimensions in elevation (2767); railing (2841).

Other event-driven or semi-driven: view3d.rs (7; one injects a viewport `Hit`, :288); window tests via `ctx.run_ui(...)` for type editor (type_editor.rs:333), types window (model_edit.rs:2047), warnings (:1780), schedule/sheet/view windows (schedule_ui.rs:450, sheet_ui.rs:587, view_ui.rs:464), MEP settings/insulation/systems/compute/elec windows. These run the window function with default input, then call methods; no button clicks are simulated inside them except where the code inspects `app` state after a frame.

Direct-call tests (no events): doors, windows, columns, ceilings, slabs, footings, rebar, export schema window, import, file save/open (model_edit.rs, file.rs, export.rs, import.rs).

### 4.2 Could a single "build a small office from scratch" test be written with it?

Mostly yes, with two prerequisites that are product bugs/gaps rather than test gaps: (a) the empty-canvas early return (model_view.rs:143) must be fixed (or the test uses the beam workaround), (b) the harness needs a model-to-screen transform for plans (add `plan_xf: Option<section_ui::Xf>` set in `show()`, as `section_xf` is for sections; `Xf::screen([x,y])` exists, used at model_view.rs:496).

Sketch (all calls exist today unless marked NEW):

```rust
#[test]
fn small_office_from_scratch() {
    let (ctx, screen) = frames();
    let mut app = Desktop::sample();
    app.new_model();                           // file.rs:94 (not dirty after sample() -> resets at once)
    assert_eq!(app.document.elements().count(), 0);
    app.view = View::Plan;
    // levels: the click path is dead on an empty canvas; the button path is level_click()
    app.run_command("level", "Level");         // sets level_elev_mm = 0
    app.level_name = "Ground".into(); app.level_click();
    app.level_name = "First".into(); app.level_elev_mm = 3500.0; app.level_click();
    app.level_name = "Roof".into();  app.level_elev_mm = 7000.0; app.level_click();
    app.view = View::Level(app.levels()[0].0 .0);
    // NEW: make bounds finite (today: place_beam_from_controls then delete) or fix model_view.rs:143
    // grids: events through the harness using the NEW app.plan_xf
    app.run_command("grid", "Grid");
    for (a, b) in [([0.,0.],[0.,8.]), ([6.,0.],[6.,8.]), ([0.,0.],[6.,0.]), ([0.,8.],[6.,8.])] {
        let (pa, pb) = (xf.screen(a), xf.screen(b));
        frame(.., vec![PointerMoved(pa)]); frame(.., vec![button(pa,true)]); frame(.., vec![button(pa,false)]);
        frame(.., vec![PointerMoved(pb)]); frame(.., vec![button(pb,true)]); frame(.., vec![button(pb,false)]);
    }
    // wall type with layers via the real window code
    app.wall_type_name = "Ext 300".into();  // then press "Add type": app.document.add_wall_type(..)
    let t = app.document.add_wall_type("Ext 300", 0.3, 3.5).unwrap();
    app.open_type_editor(t); /* app.type_editor.as_mut().unwrap().add_layers(); ... app.apply_type_editor(&e) */
    app.run_command("wall", "Wall"); app.wall_type = Some(t); app.wall_mode = 1;   // rectangle
    // two corner clicks at grid intersections (snap) -> 4 walls in one undo step
    // interior partition: wall_mode = 0, two clicks
    // floor: app.run_command("floor", ..); four clicks + frame(key(Enter)); assert slab count 1, area
    // door + window: app.run_command("door"); click on the wall (events at xf.screen(mid)); assert Category::Family in wall
    // stair: app.run_command("stair"); press-drag-release; assert risers == ceil(3.5/0.175)
    // roof on 'Roof' level plan: run_command("roof"); 4 clicks + Enter
    // ceiling: run_command("ceiling"); click inside room; room: run_command("room"); click
    // section: run_command("section"); two clicks; open View::Section and assert elevation drawing non-empty
    // save/reopen: app.write_model(path); Desktop::sample().load_model(path) == document (as file.rs test)
    // export: app.write_ifc(path, Schema::Ifc4x3, false); parse with openbim_ifc::reader::read and
    //         assert IFCBUILDINGSTOREY==3, IFCWALL>=5, IFCSLAB>=1, IFCROOF==1, IFCSTAIR>=1, IFCDOOR==1, IFCWINDOW==1
    //         and (today) IFCSPACE==0 -> documents the gap
    // undo chain: undo N times back to 0 elements and redo
}
```

Additional pieces a faithful test would need: (i) drive the ribbon and options bar by running `Desktop::ui` (needs `eframe::Frame`; main.rs:565 takes `&mut eframe::Frame`, which is hard to construct in a test, so refactor the body into `fn ui_body(&mut self, ui)`), which would also exercise the global Esc handler (finding 3); (ii) click ribbon buttons by id (e.g. store button rects per id); (iii) an assertion that IFC export of the result has no errors and, ideally, an independent IFC parse (there are IfcOpenShell scripts in spikes/ used by CI, not by cargo tests).

---------------------------------------------------------------------------------------------------

## 5. Discrepancies: self-reported documents vs code

| # | Document claim | What the code shows |
|---|---|---|
| D1 | docs/revit-ux-parity-audit.md header: "Verified by automated event tests (simulated mouse and keys)" and "Working tools: Wall, ... Level, Grid, ..." | True for the central canvas only; ribbon/options bar/global keys never driven; no test starts from a new project; with an empty project the Level click and Grid/Wall tools are unreachable (model_view.rs:143) |
| D2 | audit "Wall tool vs Revit": "Click start, click next, chain; Esc ends chain, Esc again leaves tool: done" | In the running app the first Esc leaves the tool (main.rs:567 runs before the viewport). Test passes because it bypasses `Desktop::ui` |
| D3 | audit "Select-mode editing": Esc cancels drag (test :1770), Esc cancels move base point etc. | Same global Esc; for tools with a pending step (Move base point, Stair first point, Vertical Opening host, Cable) Esc exits the tool rather than stepping back |
| D4 | audit "Room": "Saved in OpenBIM records and the automation snapshot; not exported to IFC (no IfcSpace yet)" | The IFC crate does write IfcSpace with quantities and boundaries (crates/ifc/src/lib.rs:1499-1535) and has tests; the desktop never feeds rooms (`rooms: vec![]`, export.rs:291). The sentence is true for the app, misleading about the crate |
| D5 | docs/3d-view-parity-plan.md section 1: "Not available in 3D: every drawing and modify tool ... plan-only" | Door/Window/Component are placed by clicking in 3D (view3d.rs:237-246, test view3d.rs:288). Same doc says "there is no redo (known)" while the audit and code have redo (lib.rs:1288, main.rs:547) |
| D6 | docs/revit-style-ui/01-ui-specification.md: "All 14 ribbon tabs stay visible; tabs and tools not built yet are shown disabled" | Unbuilt tools are enabled buttons that set the status message "X is not implemented in this UI prototype" (main.rs:495). Architecture tab has six: Curtain Wall, Ramp, Model Line, Area, By Face, Dormer; View > "View", Pan, Orbit, Find; Manage > Settings; Annotate > Detail, Revision Cloud |
| D7 | 01-ui-specification keymap preset (crates/keymap; main.rs:266-281 defines WA, DD, SL ...) and gap-review GUI-005 "PARTIAL" | The keymap is constructed but never consulted for key events; it is only printed in the Help window (main.rs:656-657). No two-letter shortcuts work |
| D8 | main.rs top comment: "no model persistence, IFC import, live routing ... implied", Help window text "Model files, server, IFC editing, routing and persistence are not connected yet" (main.rs:652-655); Cargo description "(sample scene only)" | Stale: save/open and IFC import/export are implemented and tested. Labels "Project1 (SAMPLE)" (main.rs:383), "Workset: Mechanical (sample)" (main.rs:597), "Sample scene..." (main.rs:297), "In memory / not saved" (panels.rs:927) are static regardless of state |
| D9 | docs/agent-team/gap-review.md "ARCH-001 PARTIAL", "GUI-001 prototype on branch; main does not include the desktop app" | Out of date: main now contains apps/desktop (18.5k lines in src) |
| D10 | Audit "Level" row/Tool help (main.rs:366): "Revit draws a level in an elevation view; there is none here yet" | Elevation views exist (section_ui.rs), but Level/Grid/Wall are refused there (audit row 102). Consistent with the code; note the Level click position is ignored |
| D11 | audit Opening table: "Opening panel: the five buttons are there; Shaft, Wall and Vertical work, By Face and Dormer say they are not implemented" | Matches code (ribbon.rs:116-117 unhandled -> main.rs:495) |
| D12 | audit "Compound walls ... Edit Type ... in the Wall options bar, in Properties and in the Types window" | Matches (panels.rs:438, :252-255; model_edit.rs:45). Properties shows the button only when the wall already has a type |
| D13 | audit lines on Esc for Section "Esc cancels" etc. | see D2 |
| D14 | docs/revit-book-review.md A8 "two fixed level tabs" | Matches, but the consequence (dangling level 10/11 in a new project, IFC export failure) is not mentioned |
| D15 | docs/revit-style-ui/02-workflows WF-ARCH-01: "save/reopen preserves IDs, IFC export has correct classifications/host/opening relations" | Save/reopen preserves ids (file.rs test compares elements), door host/opening tested (model_edit.rs:1858). WF-STR-01 / "Currently a target" remains. No test for "change wall length, inspect host opening" via the UI except temp-dimension test without a door |

---------------------------------------------------------------------------------------------------

## 6. Wrong or unusual Revit-parity behaviours noticed in code

Items marked (B) are probable bugs; (U) are unusual choices.

1. (B) Empty/level-only plan canvas is inert (model_view.rs:143). See finding 1.
2. (B) Global Esc cancels the tool at once (main.rs:567), overriding per-tool two-stage Esc; it also fires when a text field has focus (no `egui_wants_keyboard_input` guard) and rewrites the status text to "Select a sample element. Tab cycles; Esc cancels." (main.rs:327).
3. (B) Level 1/Level 2/Mechanical tabs and ids 10/11/ElementId(1)=duct are magic numbers: `select_element` maps `ElementId(1)` to `SampleElement::Duct` (model_edit.rs:152) and Properties "Apply" routes id 1 to `apply_duct_edit` (panels.rs:710); in a new project the first element created (often the first level) is id 1. Behaviour is benign today (apply_duct_edit falls through to apply_selected_edit, model_edit.rs:1029-1035) but fragile.
4. (B) A wall/slab/stair drawn in a plan whose level id does not exist is accepted (`SetGeometry` is not validated against levels, lib.rs:1213-1217; `place_wall_typed` lib.rs:~593) and later breaks IFC export (export.rs:123).
5. (B) `place_level` accepts duplicate typed names and levels closer than 100 mm (lib.rs:689-702), which `Levels::new` rejects at export (crates/arch/src/lib.rs:82-91).
6. (B) Parameter leakage between Floor/Ceiling/Roof/Door/Window/Footing via `slab_offset_mm`, `slab_thickness_mm` (main.rs:450,458,459,462-463; panels.rs:482-483, 641-645).
7. (B) Door/window can overhang the wall end: `along` is only clamped to 0..length (model_edit.rs:572-574); the opening offset `along - w/2` can be negative in export (export.rs:260). I did not verify what the IFC writer does with it.
8. (U) Wall button resets thickness 200, height 3000, type Custom each press (main.rs:447); Revit keeps the last type. Default 3000 mm height ignores actual level spacing.
9. (U) Level has no elevation-view placement and its click position is ignored; levels cannot be deleted or renamed.
10. (U) Wall "Exterior on the left when drawn clockwise" matches Revit's rule for a clockwise loop; Spacebar during drawing toggles which face is the reference (model_view.rs:502; audit row 50 admits it differs from Revit).
11. (U) Revit's View Control Bar sits at the bottom of the view; here at the top (audit row 187).
12. (U) Floors/roofs/ceilings are one `Slab` category with a kind; Properties heading shows "Floors (1)/Generic Slab" for a roof or ceiling (panels.rs:220) and the kind can be switched in Properties, which has no Revit equivalent.
13. (U) A Type is a document `Element` (category Type) that shares the id sequence with model elements; `Delete` of a type only when unused (model_edit.rs:44).
14. (U) IFC GlobalIds are derived from (project_key=1, element id) (model_edit.rs:306): two different projects export identical GUIDs for equal ids.
15. (U) Every `transact` clones the entire element map and revalidates every element (lib.rs:1084,1262-1263), and keeps one full snapshot per undo step without a limit; large models will slow with each click (not measured; the 'PERF-001' gate in docs/revit-style-ui/05-backlog.md is still open).
16. (U) Tab key cycles all elements by id irrespective of view/level (main.rs:568-575), not "elements under the cursor".
17. (U) `Room` boundaries: walls and columns only (rooms.rs:55-60); doors/windows are not boundary elements, so rooms do not respect door thresholds (same as Revit for walls but no room separation lines).
18. (U) Quantities: wall area/volume from axis length without corner overlap removal (audit "Schedules ... Not the same as Revit").
19. (U) Ceilings placed by "Height Offset from Level" is the underside (matches Revit); outline not associative to walls (Revit's automatic ceiling is).

---------------------------------------------------------------------------------------------------

## 7. What I could NOT verify (no execution allowed)

- That the windows actually render as the tests think (tests only count shapes).
- That the empty-canvas early return really blocks tools at run time (code reading is unambiguous: `return` at model_view.rs:143 precedes every tool branch; but I did not run the app).
- That `section_ui::show` and `view3d::show` survive an empty document.
- Any IFC output validity beyond the Rust tests (CI also runs IfcOpenShell scripts under spikes/, not inspected).
- Real file-dialog behaviour (rfd) for Save/Open/Browse.

## 8. Suggested minimal fixes (ordered by value for the "from scratch" goal)

1. Replace the `return` at model_view.rs:143 by a default bounds (e.g. a 20 m x 15 m box around origin) when nothing is drawn, so that Level/Grid/Wall can be started in an empty project; add a test that starts from `new_model()`.
2. Remove the hard-coded Level 1/Level 2/Mechanical tabs and ids 10/11: derive tabs from `levels()`; make `plan_level()` return `Option` and refuse tools with "Add a level first".
3. Make Esc stepwise: delete the unconditional `choose_tool(Select)` at main.rs:567 and let each tool's branch handle it (or only call it when the tool has no pending state); add a test through `Desktop::ui`.
4. Reset `slab_offset_mm`/`slab_thickness_mm` in the Floor and Door/Window commands (or keep per-tool state).
5. Validate level name uniqueness and 100 mm gap in `place_level`; add level rename in Properties.
6. Feed rooms and grids to the IFC scene (`rooms: vec![]` at export.rs:291) and add a Save As button.
7. Ship default wall/floor/roof types and a few door/window/furniture families; add "Load Family" using crates/family catalogue.
