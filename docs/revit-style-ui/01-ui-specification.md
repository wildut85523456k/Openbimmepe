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
- Contextual tabs and ribbon controls are derived from **active command** and **selection**.
- 2D plans, sections, elevations and 3D share one selection model; selection can highlight the same element in other views.
- Navigation: middle mouse pan, wheel zoom, Shift+middle orbit (confirm details against adopted Revit-style preset); ESC cancels active command; TAB cycles candidates; SPACE flip/rotate during supported placements.
- Unit-aware widgets display user units, while model calculations use canonical typed units.
- Accessibility: keyboard focus visibility, configurable UI scale, descriptive tooltips, high contrast and explicit error reporting.

## Command model
Each tool defines: stable command ID, label, icon identifier, active-document/selection applicability, parameter schema, transaction policy, preview handler, validation, execute handler and undo semantics.
- Ribbon, shortcut, menu, command search, Python, and graph nodes dispatch to the **same** command registry.
- UI previews are ephemeral; commit occurs only after validation.
- Each tool has Escape/cancel and undo behavior documented.
- Avoid using GUI event handlers to directly mutate geometry or IFC records.

## Properties palette
- Explicit Type vs Instance parameters, Edit Type, ordered parameter groups, unit fields and data validation.
- Multiple selection: show common fields, mixed-value state, atomic batch edit and conflicts.
- Search/filter, favorites and source of each value (explicit, inherited, formula/calculated).
- Host, system, constraints and document owner/revision exposed when relevant.

## Collaboration state
- Connected/offline, local revision, central revision, pending edits, current owner/borrowed elements and conflicts.
- On locked elements, show reason and safe actions (request ownership, inspect, cancel).
- Do not imply a change is synchronized until acknowledged by the central server.

## Workspace tabs
**Design:** authoring ribbon, properties, project browser, split 2D/3D views.
**Coordination:** federated IFC models, compare revisions, visibility filters, clash/issue panels.
**Automation:** node graph, library/search, Python editor, run/debug/output panels, model preview.

## Non-goals for first UI milestone
Pixel-perfect proprietary Revit branding; native RVT authoring; full Structure and MEP calculation suites; Dynamo .dyn compatibility.
