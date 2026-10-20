# 04 — Automation: Dynamo-like graph app and MCP server

Status: **native foundation implemented; full workflow remains a target**. See
[implementation status](../automation.md) and [interaction contract](../automation-wire-interactions.md).

Decision: use an independent Rust graph/runtime and egui desktop editor, with
external Python processes. Dynamo, DesignScript, Revit APIs and .NET are not
runtime dependencies. The candidate approaches below record the original study;
embedded Python and compatibility importers are not commitments.

Current scope: typed JSON graphs, snapshot previews, atomic apply/undo, inline
block editors, connector drag/release, manually adjusted curves, a restricted
numeric Python worker and a separate read-only snapshot MCP server. Docking,
search, graph debugger, general scripting, live MCP and full BIM node families
remain incomplete. The proposed Python API below is still illustrative.

Design agreed 16 Oct 2026 (full record: the Claude Doc "Automation: Dynamo-like Graph App and MCP Server"); it replaces the 15 Oct study, and the paragraph above says how much of it exists. Refined 17 Oct 2026 (proposed, see "Refinements from the Dynamo review"); UI parity and engine port plan added 20 Oct 2026 (proposed, see "UI parity with Dynamo for Revit" and "Engine port from Dynamo").

## Decisions

- **One API, three clients.** One Rust command and query API inside the app is shared by three clients:
  - the ribbon and shortcuts;
  - **OpenBIM Graph**, the Dynamo-like app;
  - an **MCP server** for AI automation runs.
- **egui only, Rust only.** Python exists only inside OpenBIM Graph, which is the external automation software, as Dynamo is to Revit. The app embeds no Python.
- **OpenBIM Graph is as close to Dynamo as possible.**
  - It is a separate egui program, launched from the app's Manage tab and linked live to the open model.
  - Its screen matches Dynamo's: node library on the left, canvas in the middle, and a Run bar with Automatic and Manual modes.
  - It has preview bubbles under nodes, Watch nodes, a Code Block node and a Python Script node.
  - Geometry previews in the main app's 3D view.
  - A Player equivalent runs saved graphs from the app without opening the canvas.
- **The MCP server is built into the app.**
  - Its tools are the same commands and queries as the ribbon. Elements, schedules and parameters can be read.
  - It connects two ways:
    - **stdio**, when the AI client starts the app headless on a project file;
    - **local HTTP with a token**, when the AI client attaches to a running app.
  - It is read-only by default. AI edits arrive as a proposal that the user previews and accepts. Every accepted change is logged under the AI client's name.
- **No compatibility promises.** Dynamo `.dyn` files and Revit API scripts are not compatible and are not promised.

## Architecture

```text
Ribbon and shortcuts --direct calls--+
OpenBIM Graph (egui, Python) --local link, scoped token--+--> openbim-commands (registry + typed API)
AI agents --> MCP server in the app ----------------------+        |          |            |
                                                          writes own model  reads links  calls calculation modules
```

## Execution contract

1. A run reads a fixed revision of the model and records what it read: each element and its revision.
2. The graph is checked before it runs: topology, dimensions and cycles.
3. Computation runs in parallel on all cores, produces results in a fixed order, and returns a **proposed change set**.
4. The proposal is shown in 2D/3D with the affected elements and any rule violations.
5. The user confirms, and the change set is committed through the per-element rules below. Undo works as for any edit.

## Rules the API enforces

| # | Topic | Rule |
|---|---|---|
| 5 | Bulk edits | Applied per element, matching the collaboration decisions of 15 Oct. <br>• Changes that don't clash apply. <br>• An unlocked element: the last writer wins, and the losing edit is kept and flagged. <br>• A locked element: the change becomes a change request, recorded with the clash results. <br>The run reports applied, flagged and requested elements. Edits that must stay whole (a route with its fittings) are marked all-or-nothing. |
| 6 | Stale edits | Each element carries the revision of its last change. The commit checks only the elements the run read; there is no whole-document revision check. |
| 7 | Units | Every value is a quantity: an SI number plus its dimension. <br>• Python gets quantity objects. <br>• MCP returns `{"value": 6.0, "unit": "m/s"}` and accepts "6 m/s". <br>A dimension mismatch is an error. |
| 8 | Parameters | Stable IDs (`duct.width`), mapped to IFC property sets, with English and French labels. A label is accepted with a warning, like Dynamo's get parameter by name. |
| 9 | Commands | Crate `openbim-commands`. Each command has: <br>• an ID, a label and an icon; <br>• typed inputs and the conditions under which it can run; <br>• a preview step and an execute step that produces log operations. <br>The ribbon, keymap, graph nodes and MCP tools are all generated from it. |
| 10 | Graph engine | Crate `openbim-graph`, shared by OpenBIM Graph and headless runs. <br>• Independent nodes run in parallel. <br>• Results come out in a fixed order and are cached per model revision. <br>• Dynamo lacing (shortest, longest, cross product) and replication guides are supported. <br>• Python nodes run in a pool of worker processes. |
| 11 | Trust | A graph file declares its permissions: read only, write own model, files, network. <br>• Python from a shared folder shows "this graph contains code"; trusted folders and signed graphs skip the warning. <br>• The app enforces permissions with a token scoped to them. <br>• MCP clients get the same treatment. |
| 12 | First use cases | 1. Firestop records from a site-inspection CSV. <br>2. Room schedule values. <br>3. Luminaire data for DIALux. <br>4. Duct sizing once M6 exists. |
| 13 | Calculations | Graph nodes and MCP tools call the app's calculation modules (M4 to M9); nothing is reimplemented. Each result records the standard and edition. |

**Graph file format (proposed; normative detail in 04b §6).**
- **Structure.** Dynamo-informed JSON with graph nodes/connections separated from canvas layout; an own format, not the same schema as `.dyn`.
- **Schema.** Own versioned schema. Node types are named by library and library version, so old graphs can be upgraded.
- **Storage.** In the project, as a file kept in the operation log with a checksum, or next to the project file.
- **Schema v1 content.**
  - Header and executable policy: stable graph ID; schema/replication profile versions; locked library dependencies; Python engine/ABI/client API and artifact lock; explicit permissions and trust requirements (never saved consent).
  - Stable Player interface IDs bind explicitly to stable node/port IDs, with type/rank/unit/default/ranges and EN/FR labels; display order is explicit.
  - Stable node/port IDs, typed parameters, lacing, ordered per-call guides, rank expectations, per-port levels/defaults, frozen state, creating-node management/identity/reorder/shrink/detach/create-new policy.
  - Stable connector IDs and typed `{node_id, port_id}` source/output and target/input endpoints.
  - A separate `view` section: positions, groups and their styles, notes and the node they are pinned to, reroute pins, pinned preview bubbles, preview flags, run mode.
  - Own canonical object/array/number/quantity policy, unknown-version handling and migrations: 04b §6 and round-trip fixture. **No** project ownership/bindings, persisted consent or base64 thumbnail (sidecar PNG).

## Refinements from the Dynamo review (17 Oct 2026, proposed)

Source: [04a research](04a-dynamo-review.md). **Proposed amendments, corrected after independent review F1–F9:** the user approved these corrections, not unverified Dynamo semantics or implementation. The normative companion [04b](04b-replication-and-serialization.md) defines the supported evidence boundary, persisted settings and implementation gates. All changed rule/table/schema requirements and refinements in this revision are proposed until independent review of the new SHA; the 16 Oct architecture decisions remain unchanged.

- Rule 10's proposed element-read-set-only cache replacement is withdrawn. The engine crate stays shared by GUI and headless runs, with ordered results. Keep conservative invalidation keyed on model identity + snapshot generations + definitions/inputs/library/API versions + captured resource identities; nonpure Python/network nodes and dependent results stay uncached, and a force-full-run exists. Finer dependencies require proof (04b §4). Dynamo-informed Auto/Shortest/Longest/Cross, guides and List@Level follow 04b §2, not invented semantics; unresolved combinations are gated, not claimed supported. Python runs in separately approved workers.
- Execution step 5 is **not replaced**. Player, Manual, Automatic, CLI and MCP all require per-run human confirmation of the exact proposal before model commit. No persistent apply-without-review consent exists. Closing/rejecting review revokes that run's consent; edits or dependency changes invalidate it. Permissions, stale validation, locks/change requests and mandatory destructive-proposal review cannot be bypassed.
- Rule 11 is explicitly amended: model token scopes enforce read-only / write-own-model access and never writes to linked models, and brokered files/network capabilities are explicit. App-scoped model tokens are not a CPython filesystem/network sandbox: arbitrary Python requires separate revision-bound trusted-local-code approval, and package installation separate consent (04b §5). Trusted folders/signatures alone cannot authorize code, installation or model commits; MCP source-read/edit/code-edit/run capabilities and path authorization stay distinct.
- Rule 6 is extended to validate snapshot-scoped collection/negative reads as well as element reads: revisions are checked per dependency group, not as a whole-document conflict veto, and collection/negative reads conservatively require their model snapshot generations still to match, invalidating affected groups until tracked predicate dependencies are proven (04b §4); rule 5 gains dependency-atomic groups and atomic ownership metadata (04b §3).
- The schema summary below is replaced by the precise **own**, Dynamo-informed schema in 04b §6; this does not introduce `.dyn` interoperability.

**Values and language**
- **Dictionaries** are a value type, with dictionary nodes. A CSV reader returns a list of dictionaries.
- **Code Block** uses a familiar subset of DesignScript: literals with optional units (`3.5 m`, `6 m/s`), lists `[..]`, dictionaries `{"k": v}`, ranges `a..b..s`, `a..#n..s`, `a..b..#n`, indexing and slices, `@Ln`, replication guides, `def` functions and `if` expressions. No imperative blocks in v1. Familiar syntax is promised, not DesignScript compatibility.
- **Core list nodes** from the start: Count, Flatten, Transpose, Chop, GroupByKey, SortByKey, FilterByBoolMask, Map/Combine.
- **Units** show in preview bubbles, Watch nodes and sliders. A dimension mismatch is a node error naming both dimensions.

**Writing to the model**
- **No transaction nodes.** Queries read the fixed revision of the run (execution contract step 1). Creating nodes return **pending handles** that downstream nodes may use; a query never sees pending elements unless fed them. Bubbles show "pending".
- **Managed outputs replace Dynamo element binding.** Project-scoped ownership uses stable row/output keys where available, not positional row identity. Run-scoped typed handles, dependency-atomic groups, atomic mapping updates/undo and explicit reorder/shrink/detach/create-new policies are mandatory (04b §3). Destructive changes always require review.
- **Automatic mode never commits.** It recomputes preview/proposal only. Every client, including Player, requires explicit per-run review/confirmation. No apply-without-review exception.
- **One undo step per committed change set**, named "Graph: <name> run <n>", linked to the run log.

**Selection and parameters**
- **Selection input nodes:** pick element(s) in the main app; category, type, level and view pickers. Each can be a Player input.
- **Parameter by label** resolves to the stable ID when the graph is edited and stores the ID, so a renamed label does not break graphs (rule 8).

**Canvas and run**
- **Node states:** Error > Warning > Info > Preview, plus "pending (proposal)" and "stale (model changed since read)". A footer counts states and jumps to them.
- **Run bar:** Automatic / Manual, with **Cancel** and per-node progress; partial results are diagnostic only and cannot be committed. Cancellation invalidates all pending handles and proposals for the run (04b §3). Periodic mode is not provided.
- **Canvas undo/redo** for graph edits, separate from model undo.
- **Library** generated from the command registry in Create / Action / Query groups, with hover cards (types, units, list structure) and search.
- P1: groups with Inputs/Actions/Outputs styles, notes pinned to nodes, align, rename node, wire editing and reroute pins, Freeze, per-node preview toggle, a profiling panel (time per node), custom nodes as subgraph files opened in tabs.

**Python Script node (A4)**
- `IN[i]` / `OUT` as in Dynamo. One pinned CPython 3 engine; the graph records the engine and API version.
- The only model access is the `openbim` client (the same API as MCP), returning proposals; no host boilerplate.
- Optional pinned/locked artifacts in an isolated environment, with integrity checks and separate informed installation consent. No installs or code execution on open/inspection. Rule 11/04b §5 require trusted-local-code approval: no OS sandbox is promised. AI code edits require separate diff acceptance and invalidate prior approvals.

**Player (A5)**
- Folder list with thumbnail, description and last-run result. Typed inputs validated before the run; a graph with unsupported inputs is refused, not run.
- "Open in OpenBIM Graph". Runs headless through the same engine and returns the same proposal review.

**Headless and AI (P1)**
- CLI: `openbim-graph run <file> --in name=value --json-out`, for regression tests of team graphs.
- MCP tools to inspect/edit/run graphs require separate source-read, graph-edit, code-edit and run capabilities, authorized canonical paths, revision-bound diff approval and code/install consent; none can confirm model commit on the user's behalf (04b §5).

**First MEP node libraries (A5), in order:** MEP-system traversal (connectors, systems, fittings); intersections of linked and host elements for firestops and dampers; space and zone tagging of MEP elements; batch views and sheets; calculation nodes (M4–M9).

**Not promised:** `.dyn` import, DesignScript imperative blocks, Periodic mode, a public package manager, Generative Design. The input/output schema keeps min/max/step so an optioneering runner can be added later.

## Prototype on `feature/rust-gui` (decision 17 Oct 2026, proposed)

The prototype (`crates/automation`, automation workspace in the desktop app) was built against the 15 Oct study. This design stays authoritative; the prototype moves to it:
- The graph editor leaves the main app and becomes the separate OpenBIM Graph program. The main app keeps only the Manage-tab launch, the Player, proposal review and 3D preview.
- Python leaves the main app and runs only from OpenBIM Graph.
- The MCP server moves into the app on the live model; the snapshot-file server is kept, if at all, for tests only.
- Commits follow the per-element rules 5 and 6 instead of whole-batch commits with a whole-document check.
- Nodes are generated from `openbim-commands`, not from direct document commands. `crates/document` converges with `crates/oplog` (BIM-001) before automation builds on either.
- The branch's edit to this file is not merged.
- Kept and moved: graph validation, the preview → plan → apply → undo pattern, the workflow tests, the canvas code (into OpenBIM Graph, with layout in the `view` section), the MCP JSON-RPC layer, the Python worker process plumbing. Rewritten: node and value types (quantities, stable IDs), the AST-only `worker.py`, the snapshot MCP backend.

## UI parity with Dynamo for Revit (20 Oct 2026, proposed)

Goal (user decision, 20 Oct 2026): every user gets the same UI experience in OpenBIM Graph as in Dynamo for Revit. The full feature-by-feature mapping, the acceptance list of interactions, the Revit-specific exclusions and the deliberate differences are in [04d](04d-ui-parity.md). This section records the parity decisions and the adopted additions (from 04d §2, adjusted by the decisions below).

**Parity decisions (20 Oct 2026):**
- Parity scope: 111 Dynamo UI features mapped. 47 are already in this design; 47 are adopted below (46 from the 04d study plus the new E9 file-trigger); 10 are Revit-specific (excluded; each has an OpenBIM equivalent, 04d §4); 8 are deliberate differences (04d §5). The 46/47 split in 04d §1 is the original study's count; the adopted set is 47.
- `.dyn` import stays out of scope; the no-compatibility promise in "Decisions" stands.
- In-canvas 3D preview and the Watch 3D node are adopted at P1 (A6, D6 below).
- One Home graph per window, as in Dynamo; no Home tabs (J5 below).
- Node Autocomplete is promoted to P1 (B9 below); Node to Code stays P2 (B10).
- A "re-run on file change" trigger replaces Periodic mode (E9 below).

**Adopted additions (04d §2, plus E9 below — 47 blocks):**

**A2 — Toolbar: New / Open / Save / Save As, Undo, Redo (P0).**
- **New** (Ctrl+N) creates an empty graph in the Home tab; **Open** (Ctrl+O) opens a `.obg` graph from the project's graph folder; **Save** (Ctrl+S) and **Save As** (Ctrl+Shift+S) write the graph to the project's graph store and record its checksum in the operation log.
- **Undo** (Ctrl+Z) and **Redo** (Ctrl+Y) act on the canvas edit history only: node moves, wire edits, group/note creation, renames. They never touch the model; the model undo stack receives exactly one entry per committed run, named "Graph: <name> run <n>".
- The toolbar shows New, Open, Save, Undo, Redo in that order, with the shortcut in each tooltip. OpenBIM Graph has no menu bar; Help opens node help (F1) or the Player.

**A3 — Export workspace as image (P2).**
- A toolbar button exports the visible canvas as a PNG; the file name defaults to the graph name and the image covers the current view, not the whole graph.

**A6 — In-canvas 3D preview viewer / Watch 3D node (P1).**
- When the main app's 3D view is not visible, a per-graph viewer toggle shows the previewed geometry inside the OpenBIM Graph window, with orbit (right-drag), pan (middle-drag) and zoom (middle-scroll).
- A Watch 3D node in the View category displays its input geometry in that viewer; a Watch 3D node with preview off behaves like any other previewed node.
- The default stays the main app's 3D view; the in-canvas viewer is opt-in per graph and its state is saved in the `view` section.
- Ctrl+B / Esc-hold focus the preview (main app or in-canvas, whichever is active); release returns to the canvas.

**A7 — Watch Image node (P2).**
- A Watch Image node in the Display/View category shows its input image on the canvas, resizes with the image and respects the per-node preview toggle.

**B4 — Dotted hierarchy search (P0).**
- Library search accepts `library.category.nodeName`, `category.nodeName` and bare `nodeName`, matching Dynamo's three forms; results update as you type and Enter places the highlighted node at the canvas centre.
- Search also matches keywords in node descriptions, so a partial word still finds the node.

**B5 — Add-ons section (P1).**
- Team libraries and their custom nodes appear in a separate Add-ons section under the built-in library, in library/version order.
- Each Add-ons entry shows its library name and locked version; a version change is visible before the graph is opened.

**B6 — Resizable, collapsible library panel (P1).**
- The library panel can be resized by dragging its right edge and collapsed to a strip; the width and collapsed state persist across sessions.

**B7 — In-canvas search (P0).**
- Right-clicking the canvas background opens an in-canvas search box; typing filters nodes by keyword and dotted hierarchy, and Enter places the highlighted node.
- The search box closes on Esc or on click outside; it never steals focus from a node being edited.

**B8 — Node help (P1).**
- F1 opens the documentation of the last selected node in a side panel: description, inputs/outputs with types and units, and a short example graph.
- A "Copy example" action opens the example graph in a new window; the current graph stays open, and closing it prompts to save unsaved changes.

**B9 — Node Autocomplete (P1; promoted from P2, 20 Oct 2026).**
- While editing a Code Block or Python node, typing a node name or port name offers registry-based completions filtered by expected type, with the type shown in the completion popup.
- Completions are suggestions only: accepting one inserts text, it never changes the graph silently.

**B10 — Node to Code (P2).**
- A node-menu action converts the current selection into a single Code Block that reproduces the selected nodes' values and connections as DesignScript-like expressions.
- Inputs of the selection become input ports; outputs remain outputs; anything the language cannot express (UI nodes, unsupported constructs) is left as a node and noted in a comment.

**C2 — Node state visual vocabulary (P0).**
- State precedence is Error > Warning > Info > Preview. Every node shows the state in four places: input-port bars (blue = satisfied, red = unconnected or wrong type), a status bar under the node (yellow = warning, red = error, blue = info), a hover tooltip on the node name/ports, and a selection border (aqua) when selected.
- A grey vertical bar over an output port marks a function output. A frozen node is translucent blue; a node with preview off shows a grey status bar and an eye icon.
- Warning and error tooltips name the offending port and the expected type/structure; the footer counters link to the affected nodes.

**C7 — Graph Node Manager (P1).**
- A side panel lists every node with its display name, type, state and (after a run) its execution time; clicking a row selects and centres the node on the canvas.
- The list can be filtered by state (error/warning/info/pending/stale/frozen) and sorted by name or execution order.

**C8 — Lacing icon (P1).**
- A node whose inputs use lacing shows the lacing icon; hovering it names the current mode (Auto / Shortest / Longest / Cross Product / Disabled) and clicking it opens the lacing menu.
- Changing lacing marks the node and its dependents dirty and triggers a re-run in Automatic mode.

**C9 — Copy-to-clipboard on bubbles (P2).**
- Info and warning bubbles carry a copy button that puts the bubble text on the clipboard.

**D3 — Bubble list shape and level labels (P0).**
- A preview bubble shows the value, the list shape (depth and lengths) and the level labels `@L1`…`@Ln` for nested lists; scalars show as a single value, quantities with their unit.
- A truncated preview shows the first and last items plus a "…" marker and the total count, and the node enters the info state with a "preview truncated" message.

**D6 — Watch 3D node (P1).**
- A Watch 3D node in the Display/View category shows its input geometry in the in-canvas viewer from A6; with preview off it behaves like any other previewed node.
- It is the node form of the A6 viewer: adopting A6 adopts this node, and closing A6 closes this row.

**D7 — Preview truncation cap (P1).**
- Only nodes with preview on and currently visible in the view are tessellated; a hard instance cap applies per node, and hitting it sets the "preview truncated" info state with the cap value.
- The cap is shown in Preferences and is the same limit used by the Player's headless preview.

**D8 — Isolation mode (P2).**
- Ctrl+I fades the preview geometry of nodes that are not in the current selection; Ctrl+I again restores all previews.
- Works on whichever preview surface is active (main app 3D view or the in-canvas viewer from A6).

**E8 — Console / log panel (P2).**
- Ctrl+Shift+Up toggles a console panel listing run messages, cache hits, gate diagnostics and Python stderr, newest last; each entry carries its run id.
- The panel is read-only and never blocks the canvas.

**E9 — Re-run on file change (P2; replaces Periodic mode).**
- A graph can declare file-watch triggers on its captured external resources (for example the firestop CSV); when a watched file changes, the graph re-runs in Automatic mode, refreshing preview and proposal only — never committing.
- The run bar shows "waiting on file change" while triggers are armed. Manual mode ignores triggers. The trigger list is stored in the graph header and requires the graph's files permission (rule 11).

**F1 — Drag from library to canvas (P0).**
- Dragging a library entry onto the canvas creates the node at the drop point, centred on the cursor; the same entry can be dragged again immediately.
- The drag shows a ghost of the node; dropping on an occupied spot still places the node (no automatic stacking), matching Dynamo's placement model.

**F2 — Double-click canvas creates a Code Block (P0).**
- Double-clicking empty canvas creates a Code Block node at that point with the editor open; double-clicking a node body opens that node's editor (Python) or does nothing for nodes without one.

**F3 — Wire creation from either end (P0).**
- A wire can be started from an output port or an input port; while dragging, the wire is dashed and snaps to compatible ports, turning solid on a valid target and being refused (no wire created) on an incompatible one.
- An input port accepts at most one incoming wire; connecting a second wire replaces the first and reports it in the footer.

**F4 — Shift multi-move and Ctrl duplicate wires (P1).**
- Shift+drag on a connected output grabs all wires leaving it: dropping on another output reattaches them, dropping on empty canvas deletes them.
- Ctrl+click on a connected input enters duplicate-connector mode: the same source can then be connected to several inputs; releasing Ctrl and clicking cancels the mode.

**F6 — Hide wires (P1).**
- A canvas setting "Show connectors" hides all wires except those of the selected node (shown in aqua); the setting is saved in the `view` section.
- Right-clicking a node's output port offers "Hide wires", which hides only that output's wires.

**F7 — Wire highlight on selection (P1).**
- Wires attached to the selected node render aqua; all other wires render grey. The highlight is per selection, not per node state.

**F11 — Auto-layout (P1).**
- Ctrl+L and a canvas-menu action run an auto-layout that produces a readable left-to-right graph without moving the selected node; the result is undoable as one canvas edit.

**F14 — Copy, paste, cut, Tab (P0).**
- Ctrl+C / Ctrl+X / Ctrl+V copy, cut and paste the selection (nodes, wires between them, notes, groups); pasted content lands near the cursor with a small offset and keeps relative positions.
- Tab adds the nodes at the end of the highlighted wires to the current selection.

**F15 — Delete selection (P0).**
- Delete removes the selected nodes and their wires; deleting a group removes only the group; deleting a node inside a group leaves the group. Deletion is undoable as one canvas edit.

**F16 — Selection box semantics (P0).**
- Dragging left→right draws a solid box and selects only items completely inside it; dragging right→left draws a dashed box and selects anything it touches.
- Shift+click adds to the selection; Alt+click removes a node from its group; Ctrl+A selects all nodes, notes and groups.

**F17 — Context menus (P0).**
- Right-click a node body: Rename, Freeze, Preview toggle, Lacing, List@Level, Group, Ungroup, Note, Hide wires, Is Input/Is Output, Node to Code.
- Right-click an input port: default value options, List@Level, hide this wire. Right-click a group: style, colour, rename. Right-click a note: edit, pin to node. Right-click the background: search, add node, auto-layout, export image.

**F18 — Input port default values (P1).**
- Right-clicking an input port lists the node's default values and a "Use default" toggle; using a default removes the wire and marks the port as defaulted in the file.

**F19 — Pan and zoom controls (P0).**
- Middle-drag pans, middle-scroll zooms around the cursor, and the icons zoom to fit / in / out. Ctrl+0 zooms to extents, then to origin, then to the selection (Dynamo's three-step behaviour); Ctrl+= and Ctrl+- zoom in and out.
- The zoom/pan controls are visible in the canvas corner and are also reachable by mouse.

**F20 — Create custom node from selection (P1).**
- Ctrl+D creates a new custom node (`.obgf`) from the current selection and replaces the selection with an instance; Ctrl+Shift+N creates an empty custom node. Both open the definition in a new tab. (See H2 for the instance-update rule.)

**G2 — Code Block editor UX (P1).**
- The Code Block editor has syntax highlighting, autocomplete for registry nodes and variables, and a live list of its input ports and outputs as the text changes.
- Free variables become input ports and each line ending in `;` becomes an output; the port list updates without leaving the editor.
- Errors are underlined inline and reported in the node's tooltip, not only in the footer.

**G3 — Dot-notation calls from Code Block (P1).**
- Code Block can call command-registry nodes in dot notation (`Point.ByCoordinates(0,10)`, `pt.Add(vec)`, `pt.X`), matching Dynamo's create/action/query conventions.
- UI nodes (Watch, Watch 3D, selection pickers) are not callable from Code Block, as in Dynamo.

**G5 — Python editor UX (P1).**
- The Python editor has syntax highlighting, completion for the `openbim` client API and the standard library, and inline error markers; it shows the pinned engine and client API version in its header.
- There is no engine dropdown: one pinned CPython 3 engine is recorded in the graph; a version mismatch blocks the run with a migration report.

**G15 — Input node family (P0).**
- The library provides Number, Number Slider, Integer Slider, String, Boolean, File Path and Directory Path input nodes in the Input category.
- Sliders carry a unit and min/max/step, show the unit in the node and in the preview bubble, and accept typed values outside the range with a warning.
- File and Directory Path inputs open a file picker and store the canonical path.

**G16 — Unit nodes (P0).**
- `Quantity.Parse("6 m/s")` builds a quantity from a string; `Convert` changes the display unit within a dimension; a display-format node sets precision and unit style.
- A dimension mismatch is a node error that names both dimensions, shown on the node and in the footer.

**H2 — Custom node instances update live (P1).**
- Editing a custom node definition updates every instance in every open graph.
- (The Ctrl+D / Ctrl+Shift+N creation gestures are in F20.)

**H5 — Preferences (P1).**
- A Preferences panel exposes numeric display precision (decimal places), preview density/instance cap, default run mode, and whether the background preview starts enabled.
- Changes apply immediately to new evaluations and are stored per user, not in the graph file.

**I2 — Player search (P1).**
- The Player folder list has a search field filtering by graph name and description; results keep their thumbnails and status.

**I7 — Player input grouping and ordering (P1).**
- Player inputs are grouped by the canvas group each bound node belongs to, in group display order, and ordered by the graph's explicit input order within a group; ungrouped inputs come last.
- Two same-typed inputs remain distinct by binding, never merged by label.

**J3 — Keyboard shortcut parity (P0 core / P2 full).**
- **P0 core:** Ctrl+N (new), Ctrl+O (open), Ctrl+S (save), Ctrl+Shift+S (save as), Ctrl+Z / Ctrl+Y (canvas undo/redo), Ctrl+C / X / V (copy/cut/paste), Delete, F5 (run in Manual), Ctrl+0 / Ctrl+= / Ctrl+- (zoom), middle-drag pan, middle-scroll zoom, Shift/Ctrl wire editing, right-click canvas search, double-click canvas → Code Block, Ctrl+G (group), Ctrl+W (note), Ctrl+D (custom node from selection).
- **P2 full parity:** Ctrl+A (select all), Ctrl+B and Esc-hold (preview), Ctrl+I (isolation), Ctrl+K (unpin all previews), Ctrl+L (auto-layout), Ctrl+P (pan mode), Ctrl+T (template), Ctrl+Shift+I (insert graph), Ctrl+Shift+N (new custom node), Tab (add connected nodes), F1 (node help), Alt+click (remove from group).
- Every shortcut is shown in its tooltip and is remappable; conflicts are reported, not silently dropped.

**J4 — Node display name `Category.Name` (P0).**
- Nodes are displayed on the canvas as `Category.Name`, matching Dynamo's naming convention; the underlying type identity is `lib@version::name` and is what the file stores.
- Renaming a node changes the display label only; it never changes the type, the ports or the bindings.

**J5 — Home workspace (P2; parity decision 20 Oct 2026).**
- One Home graph per window, as in Dynamo; custom node definitions open in their own tabs. No Home tabs.

## Engine port from Dynamo (20 Oct 2026, proposed)

Decision: **port-subset, not a wholesale port** (full study: [04c](04c-dynamo-engine-port.md)). The Code Block language front-end (grammar/AST), the lacing/replication-guide and List@Level algorithms, the node/port data model and the list/math/string function semantics are re-implemented in Rust from Dynamo's Apache-2.0 engine, pinned at `DynamoDS/Dynamo@8a211c5f306e5b54541fa57081aa1ffa32a65e90`, with attribution per 04c §8 (LICENSE.txt, THIRD_PARTY_NOTICES, per-file source-SHA headers, PORTING.md). The .NET VM, scheduler and `.dyn` serialization are reference-only; ProtoImperative, DynamoSandbox and DynamoUnits (closed-source ForgeUnits) are skipped. Our own quantity model (rule 7), execution contract and cache policy are unchanged — Dynamo's element binding, transactions and delta compute are deliberately not ported (04c §4.3).

**Phase 0 spike (precedes A0):** crate `obg-ds` — a hand-written parser and evaluator for the Code Block subset plus a pure-function port of `Replicator.BuildPartialReplicationInstructions` and the zip/Cartesian executor. Exit: `cargo test` green with outputs byte-equal to the non-gated `fixtures/replication-v1.json` cases and to transcribed Dynamo replication/AtLevel test cases. Effort ~2–4 engineer-weeks [estimate]; ~4–5 k LOC [estimate] (04c §7).

## Milestones

1. **A0:** command registry and typed Rust API, used by the ribbon. (Preceded by the Phase 0 spike, AUTO-008, and the license-compliance setup, AUTO-009.)
2. **A1:** MCP server, read-only first, then proposals the user accepts.
3. **A2:** headless engine and graph format, with 04b's source-established replication subset, unresolved behavior gates, conservative cache and serialization acceptance; engine core ported per 04c.
4. **A3:** OpenBIM Graph app: canvas, library, Watch nodes, 3D preview in the main app, with the UI-parity additions above (acceptance list in 04d §3).
5. **A4:** Python Script node with model scopes, explicit trusted-local-code boundary and separate locked-artifact install consent (04b §5), not an OS sandbox.
6. **A5:** discipline node libraries as the calculations arrive, and a Player in the app.

## Acceptance test (first)

1. Read a site-inspection CSV of firestop records, through OpenBIM Graph and through an MCP client.
2. Match each row to its wall penetration by stable ID.
3. Preview the changed records.
4. Commit.
5. See the records in the IFC export.
6. Undo.

A locked wall turns its rows into change requests instead of edits.
