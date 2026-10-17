# 01 — Interface specification

## Reference screen: Systems / Place Duct
The reference PDF shows a light-theme, Revit-style project window with:
1. Top quick-access actions, application title, help/search and window controls.
2. Ribbon tabs: Architecture, Structure, Steel, Precast, Systems, Insert, Annotate, Analyze, Massing & Site, Collaborate, View, Manage, Add-Ins, Modify.
3. Systems tools grouped as HVAC, Plumbing & Piping, Electrical and Model.
4. Contextual command bar: **Modify | Place Duct**; Width, Height, Middle Elevation in mm, Inherit Elevation, Inherit Size, Apply.
5. Left Properties palette (family/type, Edit Type, Constraints, Mechanical, Dimensions, Identity Data), above Project Browser.
6. Drawing tabs: Level 1, {3D}, M-101 - Mechanical Plan.
7. Main plan with structural grids 1–3 and A–C, AHU/ducts/air terminals, office/meeting/store spaces, dimensions, scale and navigation.
8. Status line with selection guidance and Workset: Mechanical / Main Model.

These are the **baseline**. The following additions are product proposals.

## Layout rules
- Panels (Properties, Project Browser, System Browser, Issues, Output) can dock, float, hide and resize. Persist layout per user.
- Multiple projects, views and view types. Tab switch must not reset active selections unnecessarily.
- Default layout mirrors PDF. Compact ribbon and narrow-screen layout available; all tools remain keyboard accessible.
- All 14 ribbon tabs stay visible; tabs and tools not built yet are shown disabled.
- UI language: English and French (Khmer later). Labels come from translations; commands, parameters and saved UI settings use stable IDs, never English labels.
- The Analyze tab opens one window to choose the calculation; it becomes available once the design is complete enough.
- Contextual tabs and ribbon controls are derived from **active command** and **selection**.
- 2D plans, sections, elevations and 3D share one selection model; selection can highlight the same element in other views.
- Navigation: middle mouse pan, wheel zoom, Shift+middle orbit (confirm details against adopted Revit-style preset); ESC cancels active command; TAB cycles candidates; SPACE flip/rotate during supported placements.
- Unit-aware widgets display user units, while model calculations use canonical typed units.
- Accessibility: keyboard focus visibility, configurable UI scale, descriptive tooltips, high contrast and explicit error reporting.

## Command model
Each tool defines: stable command ID, label, icon identifier, active-document/selection applicability, parameter schema, transaction policy, preview handler, validation, execute handler and undo semantics.
- Ribbon, shortcut, menu, command search, OpenBIM Graph nodes and MCP tools dispatch to the **same** command registry (`openbim-commands`, see 04).
- UI previews are ephemeral; commit occurs only after validation.
- Each tool has Escape/cancel and undo behavior documented.
- Avoid using GUI event handlers to directly mutate geometry or IFC records.

## Properties palette
- Explicit Type vs Instance parameters, Edit Type, ordered parameter groups, unit fields and data validation.
- Multiple selection: show common fields, mixed-value state, atomic batch edit and conflicts.
- Search/filter, favorites and source of each value (explicit, inherited, formula/calculated).
- Host, system, constraints and document owner/revision exposed when relevant.

## Collaboration state
There is no server: discipline models sit on a shared folder (server share or cloud-synced folder) and are linked to the central file.
- Shown in the status bar: shared-folder reachable or offline, local revision, last revision seen from the others, pending edits, borrowed elements and conflicts.
- Linked models (architecture, other disciplines) are read only.
- On a locked element, show the reason and the safe actions: raise a change request (recorded with the clash results), inspect, cancel. Elsewhere the last writer wins and the losing edit is kept and flagged.
- Do not show a change as synchronized until it is written to the shared folder and read back.

## Workspace tabs
**Design:** authoring ribbon, properties, project browser, split 2D/3D views.
**Coordination:** federated IFC models, compare revisions, visibility filters, clash/issue panels.
**Automation:** graphs are built in OpenBIM Graph, a separate egui program launched from the Manage tab (Dynamo-like: library, canvas, Run bar, Watch nodes, Python Script node). Inside the app: a Player to run saved graphs, the 3D preview of proposed changes, and a panel for proposals coming from MCP clients.

## Non-goals for first UI milestone
Pixel-perfect proprietary Revit branding; native RVT authoring; full Structure and MEP calculation suites; Dynamo .dyn compatibility.
