# 04d — OpenBIM Graph UI-parity checklist against Dynamo for Revit

Date: 20 Oct 2026. Status: **companion to 04 (proposed).** Derived from `04-visual-automation.md` ("04"), `04a-dynamo-review.md` ("04a"), `04b-replication-and-serialization.md` ("04b"), `05-backlog.md` ("05"), and the Dynamo Primer / Autodesk help pages fetched on 20 Oct 2026 (listed in §7). The 04a §2 gap table is the starting point; this study converts it into a decision-ready parity list with paste-ready additions to 04.

The parity decisions and the adopted additions are recorded in 04 "UI parity with Dynamo for Revit"; this document keeps the full mapping, the acceptance list, the exclusions and the sources. Every `[Add to 04]` row in §1 has a matching spec block in §2. Items marked **[unverified]** are not established by the cited evidence. Where 04a itself marks something unverified, that mark is carried forward.

**Decision overlay (20 Oct 2026).** The user decided six open items; 04 is authoritative. This document's §1 table, §2 specs and §6 were written before those decisions, so where they disagree with 04, 04 wins:

- `.dyn` import stays out of scope (§6 item 1 → resolved).
- In-canvas 3D preview / Watch 3D node adopted at P1 (rows A5, A6, D6; §6 item 2 → resolved).
- One Home graph per window, no Home tabs (row J5, spec J5; §6 item 3 → resolved; row A4's "J9" reference is a typo for J5).
- Node Autocomplete promoted to P1, Node to Code stays P2 (row B9, spec B9; §6 item 4 → resolved).
- "Re-run on file change" trigger replaces Periodic mode (new 04 spec E9; §6 item 5 → resolved).
- Row E9 (footer run messages) keeps its ID here; the 04 E9 file-trigger is a different feature that happens to share the letter.

## 0. How to read this document

**Status values** (one per row, exactly as written):

| Status | Meaning |
|---|---|
| **[In 04 already]** | The behavior is already decided in 04 / 04b; no change needed. Notes say what to keep in mind when implementing. |
| **[Add to 04 — P0 / P1 / P2]** | Not yet decided; must be added to 04. Priority: P0 = needed for A2–A3 and the first acceptance test; P1 = first-release polish; P2 = later. Spec block in §2. |
| **[Revit-specific — excluded: reason]** | A Dynamo-for-Revit behavior with no equivalent; do **not** copy it. Reason and the chosen equivalent in §4. |
| **[Deliberately different: reason]** | The project intentionally differs from Dynamo; the user still gets an equivalent experience. One-line reasons in §5. |

**Evidence tags:** `[04]`, `[04a §n]`, `[04b §n]`, `[05]` refer to repo docs; `[P1]`–`[P5]` are the fetched Dynamo sources in §7. Shortcut and mouse rows cite the Dynamo Primer keyboard-shortcuts page `[P5]` unless noted. Dynamo UI conventions that 04a already verified keep their 04a citation.

---

## 1. Parity table

### 1a. Shell, layout and workspace tabs

| # | Feature | Where it lives in Dynamo | Status | Notes |
|---|---|---|---|---|
| A1 | Workspace layout: library left, canvas middle, run bar | Five regions: Menus, Toolbar, Library, Workspace, Execution bar `[P1]` | **[In 04 already]** | 04 Decisions: "node library on the left, canvas in the middle, and a Run bar with Automatic and Manual modes". Menus are the one region not carried over — see A2. |
| A2 | Toolbar: New / Open / Save / Save As, Undo (Ctrl+Z), Redo (Ctrl+Y) | Toolbar `[P1]`, `[P5]` | **[Add to 04 — P0]** | Spec A2. Dynamo shows New, Open, Save/Save As, Undo, Redo, Export Image. OpenBIM Graph keeps the file actions in a toolbar; no full menu bar. |
| A3 | Export workspace as image (PNG) | Toolbar far right `[P1]` | **[Add to 04 — P2]** | Spec A3. 04a gap table: Adopt P2. |
| A4 | Home workspace tabs + custom-node tabs | "All Active Tabs"; one Home `.dyn` per window, many Custom Node `.dyf` tabs `[P2]` | **[In 04 already]** | 04 P1: "custom nodes as subgraph files opened in tabs" (`.obgf`, the project's own extension). Dynamo allows exactly one Home workspace per window; multiple Home tabs was the optional P2 row J5, decided 20 Oct 2026 as parity (no Home tabs). |
| A5 | Graph Preview ↔ 3D Preview toggle (top-right icons, right-click menu, Ctrl+B) | Preview Mode `[P2]` | **[Add to 04 — P1]** (decision overlay, 20 Oct 2026: in-canvas viewer adopted; see spec A6) | 04: preview lives in the main app's 3D view, with an opt-in in-canvas viewer at P1 (specs A6/D6). Ctrl+B / Esc-hold focus whichever preview surface is active. |
| A6 | In-canvas 3D preview / Watch 3D equivalent | Workspace 3D Preview, orbit/pan/zoom `[P2]` | **[Add to 04 — P1]** | Spec A6. 04a gap table: Adapt P1 ("an in-canvas Watch 3D helps when no model view is visible"). Default stays main-app preview; this is the optional fallback viewer. |
| A7 | Watch Image node | Display/View category `[04a §1.3]` | **[Add to 04 — P2]** | Spec A7. 04a: P2. |

### 1b. Library, search and node discovery

| # | Feature | Where it lives in Dynamo | Status | Notes |
|---|---|---|---|---|
| B1 | Library hierarchy: Library > Category > Subcategory > node, each split Create / Action / Query | Library `[P3]` | **[In 04 already]** | 04: "Library generated from the command registry in Create / Action / Query groups" (rule 9). Create/Action/Query definitions match `[P3]`. |
| B2 | Library hover card: description, icon, inputs and outputs with types and list structure | Library hover `[P3]` | **[In 04 already]** | 04: "hover cards (types, units, list structure)". 04a gap table: Adopt P0. |
| B3 | Keyword search + Enter places highlighted node | Library Search field `[P3]` | **[In 04 already]** | 04: "search". Dynamo's Enter adds to the **centre of the workspace** `[P3]` — see acceptance item 2 and spec B4. |
| B4 | Dotted hierarchy search `library.category.nodeName` (also `category.nodeName`, `nodeName`) | Library "Search by hierarchy" `[P3]` | **[Add to 04 — P0]** | Spec B4. 04a gap table: No → Adopt P0. The main way users find nodes; Code Block dot-notation names map onto it. |
| B5 | Add-ons section (custom nodes, packages, DLLs) | Library "Add-ons" section since 2.0 `[04a §1.2]` | **[Add to 04 — P1]** | Spec B5. 04a §4 library bullet: "an Add-ons section for team libraries". |
| B6 | Library panel resizable and collapsible | Library panel (web component) `[04a §1.2]` | **[Add to 04 — P1]** | Spec B6. Cheap; stops the library eating canvas space. |
| B7 | In-canvas search: right-click background and type | Workspace right-click `[P2]`, `[P5]` | **[Add to 04 — P0]** | Spec B7. 04a §1.1. This is the second main node-placement gesture after drag from library. |
| B8 | Node help: F1 shows the last selected node's documentation; node help with an example graph | Help menu / Dynamo Dictionary `[P1]`; 04a §4 | **[Add to 04 — P1]** | Spec B8. `[P5]` F1. 04a §4: "node help with an example graph". |
| B9 | Node Autocomplete (port-based suggestions) | Canvas, release notes `[04a §1.1]` | **[Add to 04 — P1]** (promoted from P2, decision overlay 20 Oct 2026) | Spec B9. Type-based suggestions from the command registry. |
| B10 | Node to Code (selection → one Code Block) | Node menu `[04a §1.3]` | **[Add to 04 — P2]** | Spec B10. 04a gap table: Adopt P2 (needs a Code Block AST printer; good for teaching). |

### 1c. Node states and feedback

| # | Feature | Where it lives in Dynamo | Status | Notes |
|---|---|---|---|---|
| C1 | State precedence Error > Warning > Info > Preview, with hover tooltips | Node status bars `[P4]` | **[In 04 already]** | 04 refinements: "Node states: Error > Warning > Info > Preview". |
| C2 | State visual vocabulary: blue satisfied input bars, red unconnected input bars, grey function output bar, yellow/red/blue status bars, aqua selection border, warning/error tooltip icon | Node anatomy and Node States `[P4]` | **[Add to 04 — P0]** | Spec C2. 04a gap table: "Node states … with tooltips | No (only 'graph is checked') | Adopt | P0". 04 names the states; the colours, bars and tooltip icons are the user-visible contract. |
| C3 | Freeze node (right-click menu): stops the node and everything downstream; translucent blue | Node right-click menu `[P4]` | **[In 04 already]** | 04 P1: "Freeze"; 04b §6.1 node field `frozen`; 04a: freeze freezes downstream. Saved in the file. |
| C4 | Per-node preview on/off: grey status bar + eye icon | Node status bar `[P4]`; 04a `ShowGeometry` | **[In 04 already]** | 04 P1: "per-node preview toggle"; 04b §6.1 `view` preview flags; 04b §6.2 preview flags do not alter executable digest. |
| C5 | Pending (proposal) and stale (model changed since read) states | Dynamo has no equivalent | **[In 04 already]** | 04 refinements: "plus 'pending (proposal)' and 'stale (model changed since read)'". Added by OpenBIM; bubbles show "pending" (04). |
| C6 | Graph footer: run messages ("Run completed" / "Run blocked") and state counters that jump to nodes | Graph Footer `[04a §1.1]` | **[In 04 already]** | 04 refinements: "A footer counts states and jumps to them." Run-message text is not yet specified — see spec C7 note. |
| C7 | Graph Node Manager: lists every node with type and state | 2.15 release `[04a §1.1]` | **[Add to 04 — P1]** | Spec C7. 04a gap table: Adopt P1. Also the place to show per-node run time once the profiling panel lands. |
| C8 | Lacing icon on the node (shows the lacing setting) | Node anatomy `[P4]`; 04a §1.3 | **[Add to 04 — P1]** | Spec C8. 04b §6.1 carries `lacing` in the file; the icon is the UI contract. |
| C9 | Copy-to-clipboard button on info/warning bubbles | Node data preview `[04a §1.3]` | **[Add to 04 — P2]** | Spec C9. Release-notes behaviour cited in 04a. |

### 1d. Preview bubbles, Watch and preview limits

| # | Feature | Where it lives in Dynamo | Status | Notes |
|---|---|---|---|---|
| D1 | Preview bubble on hover under the node, showing the output value | Node data preview `[P3]` | **[In 04 already]** | 04: "preview bubbles under nodes". |
| D2 | Bubble pin, and pin state saved in the file | Node data preview; 2.0 did not save it `[04a §1.3]` | **[In 04 already]** | 04a gap table: Adopt P0 "Save pin state; Dynamo 2.0 lost it"; 04b §6.1 `view` carries pinned preview bubbles. |
| D3 | Bubble shows list shape and level labels (`@L1`…) | Node data preview `[04a gap table]` | **[Add to 04 — P0]** | Spec D3. 04a gap table note: "Show list shape and level labels (`@L1`…) in the bubble." 04 already requires units in bubbles. |
| D4 | Units shown in preview bubbles | Dynamo is unitless; OpenBIM shows quantities | **[In 04 already]** | 04 refinements: "Units show in preview bubbles, Watch nodes and sliders." Deliberate difference #6. |
| D5 | Watch node: text and list tree with indices and level labels, values coloured by type, node resizes | View category `[P3]`; 2.15 `[04a §1.3]` | **[In 04 already]** | 04: Watch nodes. 04a gap table: Adopt P0 — list tree with indices and level labels, coloured by type, quantities with units. 2.15 resize behaviour is the target. |
| D6 | Watch 3D node | View category `[P3]` | **[Add to 04 — P1]** | Spec A6 above covers the in-canvas viewer; a Watch 3D node is the node form of it. Same priority and same decision. |
| D7 | Preview truncation cap: hard instance cap + "preview truncated" info state | Dynamo tessellates everything (cost) `[04a §3.11]` | **[Add to 04 — P1]** | Spec D7. 04a §3.11: lazy preview only for nodes with preview on and visible, hard cap, "preview truncated" info state. |
| D8 | Isolation mode (Ctrl+I): fades geometry of nodes not selected | Geometry Preview shortcuts `[P5]` | **[Add to 04 — P2]** | Spec D8. Operates on the main-app 3D view; folded into A6 when the in-canvas viewer lands. |

### 1e. Run bar and execution

| # | Feature | Where it lives in Dynamo | Status | Notes |
|---|---|---|---|---|
| E1 | Automatic mode: re-runs the graph on every change | Execution bar `[P1]` | **[In 04 already]** | 04. **Never commits** — see E5. |
| E2 | Manual mode: runs only when Run is clicked (F5) | Execution bar `[P1]`, `[P5]` | **[In 04 already]** | 04. F5 is part of shortcut row J4. |
| E3 | Periodic mode (timer; greyed unless `DateTime.Now` is used) | Execution bar `[P1]` | **[Deliberately different: Periodic is not provided (04); Dynamo gates it on `DateTime.Now` and 04a rates it low MEP value]** | 04 refinements: "Periodic mode is not provided." 04a: Skip P2. 04b §6.1 `view` run mode is `Manual|Automatic` only. File-watch trigger = open question 5. |
| E4 | Cancel and per-node progress; partial results diagnostic only | Dynamo hangs the host UI `[04a §1.8]` | **[In 04 already]** | 04 refinements: "Cancel and per-node progress; partial results are diagnostic only and cannot be committed"; cancellation invalidates handles/proposals (04b §3.1). |
| E5 | Automatic mode never commits; per-run human confirmation in every client incl. Player | Own policy | **[In 04 already]** | 04 refinements step 5 and the Player rule; 04b §5 "MCP cannot self-confirm". |
| E6 | One undo step per committed change set, named "Graph: <name> run <n>" | Own policy | **[In 04 already]** | 04 refinements: "One undo step per committed change set … linked to the run log"; undo restores model + ownership mappings together (04b §3.2). |
| E7 | Profiling: execution order, time per node, force full re-run | TuneUp extension `[04a §1.1]` | **[In 04 already]** | 04 P1: "a profiling panel (time per node)"; rule 10 "Force-full-run exists". TuneUp-equivalent. |
| E8 | Console / log panel toggle (Ctrl+Shift+Up) | Workspace shortcut `[P5]` | **[Add to 04 — P2]** | Spec E8. Dynamo's console shows evaluation messages; the OpenBIM console shows run messages, cache hits and gate diagnostics. |
| E9 | Run messages in the footer ("Run completed" / "Run blocked") | Graph Footer `[04a §1.1]` | **[In 04 already]** | Covered by C6; message strings are part of spec C7's panel. This row keeps the E9 ID; the 04 E9 file-change trigger is a different feature. |

### 1f. Canvas editing

| # | Feature | Where it lives in Dynamo | Status | Notes |
|---|---|---|---|---|
| F1 | Drag a node from the library onto the canvas: node appears at the drop point | Library → Workspace `[P3]` | **[Add to 04 — P0]** | Spec F1. The first Dynamo gesture; also the acceptance example in the task. |
| F2 | Double-click empty canvas creates a Code Block node | Workspace double-click `[P3]`, `[P5]` | **[Add to 04 — P0]** | Spec F2. |
| F3 | Create a wire from either end; wire is dashed while dragging and solid when connected | Wires `[P4]` | **[Add to 04 — P0]** | Spec F3. Dynamo allows clicking the output first or the input first; both must work. |
| F4 | Wire editing: Shift+drag on a connected output moves all its wires (reattach or delete); Ctrl+click on a connected input duplicates the connector | Editing Wires `[P4]`, `[P5]` | **[Add to 04 — P1]** | Spec F4. 04 P1 says "wire editing"; 04a gap table: Adopt P1 with these exact gestures. |
| F5 | Reroute points on wires, saved in the view | `ConnectorPins` in `View` `[04a §1.3]` | **[In 04 already]** | 04 P1: "reroute pins"; 04b §6.1 `view` reroute pins. |
| F6 | Hide wires: globally (View > Connectors > untick Show Connectors) and per output (right-click node output > Hide Wires) | Wires `[P4]` | **[Add to 04 — P1]** | Spec F6. 04b §6.1: connector visibility lives in `view`, not endpoint identity. |
| F7 | Wire highlight: wires of a selected node render aqua, others grey | Default vs Highlighted Wires `[P4]` | **[Add to 04 — P1]** | Spec F7. |
| F8 | Groups with styles Inputs / Actions / Outputs / Review; Ctrl+G groups the selection | Annotations, Graph Strategies `[04a §1.3]` | **[In 04 already]** | 04 P1: "groups with Inputs/Actions/Outputs styles"; 04b §6.1 groups with styles in `view`. |
| F9 | Notes and notes pinned to a node; Ctrl+W creates a note | Annotations `[P5]`, `[04a §1.3]` | **[In 04 already]** | 04 P1: "notes pinned to nodes"; 04b §6.1 notes and the node they are pinned to. |
| F10 | Align nodes | Graph Strategies `[04a §1.3]` | **[In 04 already]** | 04 P1: "align". |
| F11 | Auto-layout / clean up node layout (Ctrl+L) | Shortcuts `[P5]`; DynamoMCP `auto_layout_workspace` `[04a §1.1]` | **[Add to 04 — P1]** | Spec F11. 04 P1 lists align only; 04a §4 canvas bullet adds auto-layout. |
| F12 | Rename node ("Rename Node…") | Node menu `[04a §1.3]` | **[In 04 already]** | 04 P1: "rename node". |
| F13 | Canvas undo/redo (Ctrl+Z / Ctrl+Y) for graph edits, separate from model undo | Workspace shortcuts `[P5]` | **[In 04 already]** | 04 refinements: "Canvas undo/redo for graph edits, separate from model undo" (P0). Model undo is E6. |
| F14 | Copy / paste / cut nodes, notes, groups (Ctrl+C / V / X); Tab adds nodes connected to the current selection | Shortcuts `[P5]` | **[Add to 04 — P0]** | Spec F14. Paste places near the cursor with a small offset; cut/paste across graphs uses the same clipboard shape as Dynamo's (nodes, notes, groups). |
| F15 | Delete selected nodes (Delete key) | Shortcuts `[P5]` | **[Add to 04 — P0]** | Spec F15. Deleting a node removes its wires; deleting a group does not delete its nodes. |
| F16 | Selection box semantics: left→right solid box selects only fully contained items; right→left dashed box selects anything touched; Shift+click adds, Alt+click removes from group | Workspace mouse interaction `[P5]`, `[P2]` | **[Add to 04 — P0]** | Spec F16. |
| F17 | Context menus: right-click node body, input port, group, note, background | Workspace mouse interaction `[P5]`, `[P4]` | **[Add to 04 — P0]** | Spec F17. Carries rename, freeze, preview toggle, lacing, List@Level, group, note, hide wires, default value. |
| F18 | Right-click an input port to use/clear a default value | Node anatomy #4 `[P4]` | **[Add to 04 — P1]** | Spec F18. 04b §6.1 port field `using_default`; Dynamo: "some Nodes have default values that can be used or not used". |
| F19 | Pan and zoom: middle-drag pan, middle-scroll zoom, zoom-to-fit / in / out icons, Ctrl+0 / Ctrl+= / Ctrl+- | Workspace zoom/pan controls `[P2]`, `[P5]` | **[Add to 04 — P0]** | Spec F19. Ctrl+0 = extents, then origin, then selection (Dynamo's three-step behaviour). |
| F20 | Ctrl+D creates a new custom node from the selection; Ctrl+Shift+N creates a new custom node | Shortcuts `[P5]` | **[Add to 04 — P1]** | Spec F20. See H1 for the custom-node model itself. |

### 1g. Coding: Code Block, Python, lacing, lists, units

| # | Feature | Where it lives in Dynamo | Status | Notes |
|---|---|---|---|---|
| G1 | Code Block node with a familiar DesignScript subset | Code Block `[P3]`, `[04a §1.5]` | **[In 04 already]** | 04 language bullets: units in literals, lists, dictionaries, ranges, indexing/slices, `@Ln`, guides, `def`, `if`. No imperative blocks in v1. |
| G2 | Code Block editor UX: syntax highlighting, autocomplete, free variables become input ports, each `;` line an output | Code Block demo `[P3]`, `[04a §1.5]` | **[Add to 04 — P1]** | Spec G2. 04 fixes the language, not the editor. Editor widgets follow the same decision as the Python editor (04a: Adopt P1, "use an egui code editor widget"). |
| G3 | Dot-notation calls to library nodes from Code Block (`Point.ByCoordinates(0,10)`, `pt.Add(vec)`, property queries) | DesignScript Syntax `[04a §1.5]` | **[Add to 04 — P1]** | Spec G3. "Most library nodes can be called this way" — implement for command-registry nodes; UI nodes (Watch 3D) are excluded as in Dynamo. |
| G4 | Python Script node with `IN[i]` / `OUT` | Script > Editor > Python Script `[04a §1.7]` | **[In 04 already]** | 04 A4. Double-click opens the editor (04a). |
| G5 | Python editor UX: syntax highlighting, completion, single pinned engine shown in the node | Python node editor `[04a §1.7]` | **[Add to 04 — P1]** | Spec G5. 04a gap table: Adopt P1. No engine dropdown: one pinned CPython 3, version recorded in the graph (04). |
| G6 | One pinned CPython 3 engine; graph records engine/ABI/client API; locked isolated environment | Engine history 2.7 → 2.13 → 4.0 `[04a §1.7]` | **[In 04 already]** | 04 A4 and 04b §5/§6.1 (`python` block: engine/build/ABI/client_api/environment_lock). Deliberate difference #8. |
| G7 | Python packages (numpy, pandas, openpyxl) via locked wheel artifacts | PythonNet3 note `[04a §1.7]` | **[In 04 already]** | 04 A4: optional pinned/locked artifacts, integrity checks, separate informed installation consent; no installs on open/inspection (04b §5). |
| G8 | Trusted-local-code approval dialog + "no filesystem/network confinement" warning; restricted policy refuses arbitrary Python | Own security model (04b §5) | **[In 04 already]** | 04 rule 11 and 04b §5: UI MUST explicitly warn; approvals are revision-bound; graph never stores consent. Deliberate difference #10. |
| G9 | AI code edits to Python appear as a diff the user accepts or rejects | Dynamo 4.3 review window `[04a §1.1]` | **[In 04 already]** | 04: "AI edits to Python appear as a diff the user accepts or rejects (Dynamo 4.3 has the same pattern)"; 04b §5 invalidates prior approvals. |
| G10 | Lacing UI: node menu Auto / Shortest / Longest / Cross Product + lacing icon | Node menu / lacing icon `[P4]`, `[04a §1.3]` | **[In 04 already]** | 04 rule 10 + 04b §2/§6.1 (`lacing` enum, `Disabled` included). Auto is **not** Shortest (04b §2.1). Unresolved rank combinations refuse the run (04b §2.2). |
| G11 | List@Level per input port (level spinner + "Keep list structure") | Per-port spinner `[04a §1.6]` | **[In 04 already]** | 04b §6.1: port `level`, `use_levels`, `keep_structure`; selecting a kept port clears the prior kept port in UI. 04b §2.1 pins AtLevelHandler semantics and the single-dominant-structure limit. |
| G12 | Replication guides in Code Block (`xs<1>`, `ys<2>`, `xs<1L>`) and per-call explicit guides | Replication wiki `[04a §1.6]` | **[In 04 already]** | 04 refinements (guides), 04b §2.2 (`guide_source`, ordered arguments, mixed lacing+explicit gated). |
| G13 | Dictionaries as a value type, with dictionary nodes; CSV reader returns a list of dictionaries | Dictionaries `[04a §1.5]` | **[In 04 already]** | 04 refinements: "Dictionaries are a value type, with dictionary nodes." |
| G14 | Core list node set: Count, Flatten, Transpose, Chop, GroupByKey, SortByKey, FilterByBoolMask, Map/Combine | Working with Lists `[04a §1.6]` | **[In 04 already]** | 04 refinements: "Core list nodes from the start". |
| G15 | Input nodes: Number, Number Slider, Integer Slider, String, Boolean, File Path, Directory Path | Core Input category `[P3]` | **[Add to 04 — P0]** | Spec G15. 04 mentions "sliders" with units; the input-node family itself needs an explicit library row. Number/Integer Sliders carry a unit and min/max/step. |
| G16 | Unit nodes: `Quantity.Parse("6 m/s")`, `Convert`, display-format | Forge unit nodes since 2.13 `[04a §1.8]` | **[Add to 04 — P0]** | Spec G16. 04a gap table: "Units / ForgeTypeId conversion nodes | Yes (better) | Adapt | P0". Policy is 04 rule 7; the node set is the addition. |

### 1h. Custom nodes, packages, trust and preferences

| # | Feature | Where it lives in Dynamo | Status | Notes |
|---|---|---|---|---|
| H1 | Custom nodes as subgraph files (`.obgf`), opened in tabs, editing the definition updates every instance, own lacing and defaults, versioned in the team library | Custom Nodes `.dyf` `[04a §1.3]` | **[In 04 already]** | 04 P1: "custom nodes as subgraph files opened in tabs"; 04a §4: `.obgf` analogous to `.dyf`, versioned in the team library; 04b §6.1 `dependencies[]` locks library versions. |
| H2 | Ctrl+D create custom node from selection | Shortcuts `[P5]` | **[Add to 04 — P1]** | Spec F20 (same row as Ctrl+Shift+N). |
| H3 | Packages: public Package Manager, dynamopackages.com, dependency/host compatibility | Packages `[04a §1.3]` | **[Deliberately different: no public package manager (04); a shared team library folder plus a version lock in the graph (`NodeLibraryDependencies` / `dependencies[]`)]** | 04a gap table: Adapt P2. 04b §6.1: exact `dependencies[]` with SHA-256. |
| H4 | Security: untrusted graph blocked until trusted (once/session or trusted folder); Trusted Locations under Preferences > Security; warning can be disabled | 2.15 release, Graph Security `[04a §1.3]` | **[Deliberately different: revision-bound informed code approval + separate install consent replace trusted folders; the graph stores required trust policy, never saved consent (04 rule 11, 04b §5)]** | The OpenBIM dialogs must still block execution of unapproved code — see §5 #10. |
| H5 | Preferences: decimal precision, geometry render quality, show background preview | Dynamo menu > Preferences `[P1]`, `[P3]` | **[Add to 04 — P1]** | Spec H5. At minimum: numeric display precision (decimal places) and preview density/cap; background-preview default belongs to the main app. |
| H6 | Start page / templates (Ctrl+T) | Dynamo start page `[P1]` | **[Deliberately different: no start page; graphs are opened from the Player folder list, the project's graph folder, or the library; templates remain optional P2]** | 04 A5 Player replaces graph browsing. |

### 1i. Player

| # | Feature | Where it lives in Dynamo | Status | Notes |
|---|---|---|---|---|
| I1 | Player folder list with thumbnail, description, documentation link and status | Revit Manage tab > Visual Programming `[04a §1.4]` | **[In 04 already]** | 04 A5: "Folder list with thumbnail, description and last-run result"; 04b §6.1 `name`/`description`/`doc_url` + sidecar PNG (no base64 in JSON). |
| I2 | Player search across the folder list | Player 2027 help `[04a §1.4]` | **[Add to 04 — P1]** | Spec I2. |
| I3 | Player inputs: author ticks Is Input; typed widgets; unsupported input types refused before the run | Player inputs `[04a §1.4]` | **[In 04 already]** | 04 A5: "Typed inputs validated before the run; a graph with unsupported inputs is refused, not run"; 04b §6.1 `inputs[]` bindings with type/rank/unit/default/min/max/step and EN/FR labels. |
| I4 | Player outputs: Is Output flags and named Watch nodes appear as outputs | Player outputs `[04a §1.4]` | **[In 04 already]** | 04b §6.1 `outputs[]` with explicit bindings and display order; 04a §4 Player bullet. |
| I5 | "Edit in Dynamo" / "Open in OpenBIM Graph" | Player 2027 `[04a §1.4]` | **[In 04 already]** | 04 A5: "'Open in OpenBIM Graph'". |
| I6 | Player runs headless through the same engine and returns the same proposal review | Own policy | **[In 04 already]** | 04 A5: "Runs headless through the same engine and returns the same proposal review"; 04b §5 headless produces a proposal only. |
| I7 | Player input grouping and ordering from canvas groups | Player gap `[04a §3.9]` | **[Add to 04 — P1]** | Spec I7. 04a §3.9: "grouping or ordering of inputs from canvas groups". |
| I8 | Player custom forms (Data-Shapes-style UI forms) | Data-Shapes package `[04a §1.11]` | **[Deliberately different: typed input schema (quantity+unit+range, enum, element filter, file) covers most needs; custom forms are not built]** | 04a gap table: Adapt P2. Revisit only if the typed schema proves insufficient. |
| I9 | Generative Design / optioneering runner | GD for Revit `[04a §1.4]` | **[Deliberately different: not promised; the input/output schema keeps min/max/step and numeric outputs so an optioneering runner can be added later (04)]** | 04a gap table: Skip, keep schema-ready. |

### 1j. Import/export, shortcuts, naming and misc

| # | Feature | Where it lives in Dynamo | Status | Notes |
|---|---|---|---|---|
| J1 | Own graph format: graph logic separated from canvas view, JSON, canonical and diffable | `.dyn` structure `[04a §1.10]` | **[In 04 already]** | 04 graph file format + 04b §6: own versioned schema, `view` section, canonical bytes, sidecar thumbnail only. Deliberate difference #5. |
| J2 | Import `.dyn` files | `.dyn` JSON since 2.0 `[04a §1.10]` | **[Deliberately different: no `.dyn` import and no compatibility promise (04)]** | Open question 1 (P2 converter). |
| J3 | Keyboard shortcut parity (full list) | Keyboard Shortcuts `[P5]` | **[Add to 04 — P0]** (core set) / **[Add to 04 — P2]** (full parity) | Spec J3. Core P0 set: Ctrl+N/O/S, Ctrl+Z/Y, Ctrl+C/V/X, Delete, F5, Ctrl+0/=/-, middle-drag pan, middle-scroll zoom, Shift/Ctrl wire editing, right-click canvas search, double-click Code Block, Ctrl+G group, Ctrl+W note, Ctrl+D custom node. Full parity P2: Ctrl+A, Ctrl+B, Ctrl+I, Ctrl+K, Ctrl+L, Ctrl+P, Ctrl+T, Ctrl+Shift+I/N/S, Tab, F1, Esc-hold, Alt+click. Shortcut tiers follow this P0/P2 split; individual feature rows keep their own priorities. |
| J4 | Node display name on canvas is `Category.Name` (type identity is `lib@version::name`) | Library `[P3]` | **[Add to 04 — P0]** | Spec J4. 04b §6.1: node `type` is `lib@version::name`; Dynamo renders `category.nodeName`. Friendly display name keeps muscle memory; the type ID is what versioning binds to. |
| J5 | Home workspace per window | One Home workspace per window `[P2]` | **[In 04 already]** (decision overlay 20 Oct 2026: Dynamo parity — one Home per window, no Home tabs) | Spec J5 as decided. |
| J6 | Graph thumbnail, description, documentation URL | `Thumbnail` (base64), `Description`, `GraphDocumentationURL` in `.dyn` `[04a §1.10]` | **[In 04 already]** | 04 graph file format: "no … base64 thumbnail (sidecar PNG)"; 04b §6.1 `name`/`description`/`doc_url` + sidecar thumbnails. Player row I1. |
| J7 | MCP tools for graphs (DynamoMCP-style: create/connect nodes, lacing, guides, groups, notes, auto-layout, run) | DynamoMCP 4.2+ `[04a §1.1]` | **[In 04 already]** | 04: "MCP tools to inspect/edit/run graphs require separate source-read, graph-edit, code-edit and run capabilities … none can confirm model commit"; 04b §5. |
| J8 | Revit element binding (trace): `Bindings[]` in the file, call-site trace, modify/reuse/replace, per-document scope | Integration guide `[04a §1.8]` | **[Revit-specific — excluded: element binding UI is Revit host integration; 04 replaces it with managed outputs]** | OpenBIM equivalent: 04 managed outputs + pending handles + per-element commit rules (04b §3). §4. |
| J9 | Transaction nodes (`Transaction.Start` / `End`) and Revit UI-thread execution envelope | Integration guide `[04a §1.8]` | **[Revit-specific — excluded: no transactions exist in OpenBIM Graph; reads see the fixed run revision and writes are proposals]** | OpenBIM equivalent: 04 execution contract steps 1–5, pending handles, separate process + cancel (04a §3.2/3.4). §4. |
| J10 | Revit selection modes: `Select Model Element` interactive button, `Categories` + `All Elements of Category`, Revit category/type/level/view pickers | Dynamo for Revit `[04a §1.8]` | **[Revit-specific — excluded: pickers run in the Revit canvas with Revit enums]** | OpenBIM equivalent: selection input nodes pick in the **main app** (04). §4. |
| J11 | `Element.SetParameterByName` exact case-sensitive free-text match | Editing `[04a §1.8]` | **[Revit-specific — excluded: exact case-sensitive names are a Revit API pattern; the project uses stable parameter IDs]** | OpenBIM equivalent: 04 rule 8 — stable IDs (`duct.width`), EN/FR labels accepted with a warning. §4. |
| J12 | Unitless values taking the Revit project's units; `DisplayUnitType` / `ForgeTypeId` enums | Working with Revit `[04a §1.8]` | **[Revit-specific — excluded: Revit project units and Revit enum types have no IFC equivalent]** | OpenBIM equivalent: 04 rule 7 — every value is a quantity (SI + dimension), dimension mismatch is an error. §4. |
| J13 | Python Revit API boilerplate: `clr.AddReference('RevitNodes')`, `RevitServices`, `DocumentManager`, `UnwrapElement`/`ToDSType`, `TransactionManager` | Python and Revit `[04a §1.7]` | **[Revit-specific — excluded: these wrap the Revit .NET API and transactions]** | OpenBIM equivalent: the `openbim` client returns proposals; no host boilerplate (04 A4, 04a gap table P0/A4). §4. |
| J14 | Generative Design for Revit runner | GD for Revit `[04a §1.4]` | **[Revit-specific — excluded: GD is a Revit-hosted study runner; the schema stays ready but no runner is promised]** | 04: "Not promised: … Generative Design." Row I9. |
| J15 | `.dyn` `.NET` `ConcreteType` names tying the file to the host; SOAP trace data; base64 bindings | `.dyn` structure `[04a §1.10]` | **[Revit-specific — excluded: host .NET type names and legacy SOAP are not portable]** | OpenBIM equivalent: `lib@version::name` node types and no bindings (04b §6). |
| J16 | Graph reuse across projects requiring bindings cleanup | Element binding forum note `[04a §1.8]` | **[Revit-specific — excluded: bindings are per-document Revit data]** | OpenBIM equivalent: project-scoped ownership mappings live in the project op log, not in the graph (04b §3.2). |
| J17 | Host UI hang while a graph runs (in-process hosts run the scheduler on the UI thread) | UI Performance wiki `[04a §1.8]` | **[Revit-specific — excluded: a Revit-host threading artifact, not a UI feature]** | OpenBIM equivalent: separate process, cancel and progress (04, 04a §3.4). |

---

## 2. Additions spec (paste-ready into 04)

Each block is written as user-facing behavior. Row numbers match §1. Priorities are as assigned in §1.

### A2 — Toolbar: New / Open / Save / Save As, Undo, Redo (P0)
- **New** (Ctrl+N) creates an empty graph in the Home tab; **Open** (Ctrl+O) opens a `.obg` graph from the project's graph folder; **Save** (Ctrl+S) and **Save As** (Ctrl+Shift+S) write the graph to the project's graph store and record its checksum in the operation log.
- **Undo** (Ctrl+Z) and **Redo** (Ctrl+Y) act on the canvas edit history only: node moves, wire edits, group/note creation, renames. They never touch the model; the model undo stack receives exactly one entry per committed run, named "Graph: <name> run <n>".
- The toolbar shows New, Open, Save, Undo, Redo in that order, with the shortcut in each tooltip. OpenBIM Graph has no menu bar; Help opens node help (F1) or the Player.
- *Source:* Dynamo toolbar `[P1]`, shortcuts `[P5]`; 04 graph file format; 04 refinements (canvas undo/redo, one undo step per commit).

### A3 — Export workspace as image (P2)
- A toolbar button exports the visible canvas as a PNG; the file name defaults to the graph name and the image covers the current view, not the whole graph.
- *Source:* Dynamo toolbar `[P1]`; 04a gap table P2.

### A6 — In-canvas 3D preview viewer / Watch 3D node (P1)
- When the main app's 3D view is not visible, a per-graph viewer toggle shows the previewed geometry inside the OpenBIM Graph window, with orbit (right-drag), pan (middle-drag) and zoom (middle-scroll).
- A Watch 3D node in the View category displays its input geometry in that viewer; a Watch 3D node with preview off behaves like any other previewed node.
- The default stays the main app's 3D view; the in-canvas viewer is opt-in per graph and its state is saved in the `view` section.
- Ctrl+B / Esc-hold focus the preview (main app or in-canvas, whichever is active) and release returns to the canvas.
- *Source:* Dynamo Preview Mode and 3D Preview navigation `[P2]`; 04a gap table Adapt P1; 04 (preview in main app).

### A7 — Watch Image node (P2)
- A Watch Image node in the Display/View category shows its input image on the canvas, resizes with the image and respects the per-node preview toggle.
- *Source:* 04a §1.3 (AU handout, Library); 04a gap table P2.

### D6 — Watch 3D node (P1)
- A Watch 3D node in the Display/View category shows its input geometry in the in-canvas viewer from A6; with preview off it behaves like any other previewed node.
- It is the node form of the A6 viewer: adopting A6 adopts this node, and closing A6 closes this row.
- *Source:* Dynamo Watch3D `[P3]`; 04a gap table Adapt P1.

### B4 — Dotted hierarchy search (P0)
- Library search accepts `library.category.nodeName`, `category.nodeName` and bare `nodeName`, matching Dynamo's three forms; results update as you type and Enter places the highlighted node at the canvas centre.
- Search also matches keywords in node descriptions, so a partial word still finds the node.
- *Source:* Dynamo "Search by hierarchy" `[P3]`; 04a gap table Adopt P0.

### B5 — Add-ons section (P1)
- Team libraries and their custom nodes appear in a separate Add-ons section under the built-in library, in library/version order.
- Each Add-ons entry shows its library name and locked version; a version change is visible before the graph is opened.
- *Source:* Dynamo Add-ons section `[04a §1.2]`; 04a §4 library bullet; 04b §6.1 `dependencies[]`.

### B6 — Resizable, collapsible library panel (P1)
- The library panel can be resized by dragging its right edge and collapsed to a strip; the width and collapsed state persist across sessions.
- *Source:* Dynamo 2.0 library panel `[04a §1.2]`.

### B7 — In-canvas search (P0)
- Right-clicking the canvas background opens an in-canvas search box; typing filters nodes by keyword and dotted hierarchy, and Enter places the highlighted node.
- The search box closes on Esc or on click outside; it never steals focus from a node being edited.
- *Source:* Dynamo Workspace right-click `[P2]`, `[P5]`; 04a §1.1.

### B8 — Node help (P1)
- F1 opens the documentation of the last selected node in a side panel: description, inputs/outputs with types and units, and a short example graph.
- A "Copy example" action loads the example into a new tab.
- *Source:* Dynamo Help/Dynamo Dictionary `[P1]`, F1 `[P5]`; 04a §4 library bullet.

### B9 — Node Autocomplete (P2)
- While editing a Code Block or Python node, typing a node name or port name offers registry-based completions filtered by expected type, with the type shown in the completion popup.
- Completions are suggestions only: accepting one inserts text, it never changes the graph silently.
- *Source:* 04a §1.1 (release notes), gap table Adapt P2.

### B10 — Node to Code (P2)
- A node-menu action converts the current selection into a single Code Block that reproduces the selected nodes' values and connections as DesignScript-like expressions.
- Inputs of the selection become input ports; outputs remain outputs; anything the language cannot express (UI nodes, unsupported constructs) is left as a node and noted in a comment.
- *Source:* Dynamo "Node to Code" `[04a §1.3]`; 04a gap table Adopt P2.

### C2 — Node state visual vocabulary (P0)
- State precedence is Error > Warning > Info > Preview. Every node shows the state in four places: input-port bars (blue = satisfied, red = unconnected or wrong type), a status bar under the node (yellow = warning, red = error, blue = info), a hover tooltip on the node name/ports, and a selection border (aqua) when selected.
- A grey vertical bar over an output port marks a function output. A frozen node is translucent blue; a node with preview off shows a grey status bar and an eye icon.
- Warning and error tooltips name the offending port and the expected type/structure; the footer counters link to the affected nodes.
- *Source:* Dynamo Node States `[P4]`; 04a gap table Adopt P0; 04 refinements.

### C7 — Graph Node Manager (P1)
- A side panel lists every node with its display name, type, state and (after a run) its execution time; clicking a row selects and centres the node on the canvas.
- The list can be filtered by state (error/warning/info/pending/stale/frozen) and sorted by name or execution order.
- *Source:* Dynamo 2.15 Node Manager `[04a §1.1]`; 04a gap table Adopt P1; 04 P1 profiling panel.

### C8 — Lacing icon (P1)
- A node whose inputs use lacing shows the lacing icon; hovering it names the current mode (Auto / Shortest / Longest / Cross Product / Disabled) and clicking it opens the lacing menu.
- Changing lacing marks the node and its dependents dirty and triggers a re-run in Automatic mode.
- *Source:* Dynamo lacing icon `[P4]`; 04 rule 10; 04b §2/§6.1.

### C9 — Copy-to-clipboard on bubbles (P2)
- Info and warning bubbles carry a copy button that puts the bubble text on the clipboard.
- *Source:* 04a §1.3 (release notes).

### D3 — Bubble list shape and level labels (P0)
- A preview bubble shows the value, the list shape (depth and lengths) and the level labels `@L1`…`@Ln` for nested lists; scalars show as a single value, quantities with their unit.
- A truncated preview shows the first and last items plus a "…" marker and the total count, and the node enters the info state with a "preview truncated" message.
- *Source:* 04a gap table note; 04 (units in bubbles); 04a §3.11 truncation cap.

### D7 — Preview truncation cap (P1)
- Only nodes with preview on and currently visible in the view are tessellated; a hard instance cap applies per node, and hitting it sets the "preview truncated" info state with the cap value.
- The cap is shown in Preferences and is the same limit used by the Player's headless preview.
- *Source:* 04a §3.11; 04 rule 10 (force-full-run), 04b §4 (captured resources).

### D8 — Isolation mode (P2)
- Ctrl+I fades the preview geometry of nodes that are not in the current selection; Ctrl+I again restores all previews.
- Works on whichever preview surface is active (main app 3D view or the in-canvas viewer from A6).
- *Source:* Dynamo Geometry Preview shortcuts `[P5]`.

### E8 — Console / log panel (P2)
- Ctrl+Shift+Up toggles a console panel listing run messages, cache hits, gate diagnostics and Python stderr, newest last; each entry carries its run id.
- The panel is read-only and never blocks the canvas.
- *Source:* Dynamo console shortcut `[P5]`; 04b §4 (cache), §2.2 (gates).

### F1 — Drag from library to canvas (P0)
- Dragging a library entry onto the canvas creates the node at the drop point, centred on the cursor; the same entry can be dragged again immediately.
- The drag shows a ghost of the node; dropping on an occupied spot still places the node (no automatic stacking), matching Dynamo's placement model.
- *Source:* Dynamo Library → Workspace `[P3]`; task acceptance example.

### F2 — Double-click canvas creates a Code Block (P0)
- Double-clicking empty canvas creates a Code Block node at that point with the editor open; double-clicking a node body opens that node's editor (Python) or does nothing for nodes without one.
- *Source:* Dynamo double-click `[P3]`, `[P5]`; 04 Code Block.

### F3 — Wire creation from either end (P0)
- A wire can be started from an output port or an input port; while dragging, the wire is dashed and snaps to compatible ports, turning solid on a valid target and being refused (no wire created) on an incompatible one.
- An input port accepts at most one incoming wire; connecting a second wire replaces the first and reports it in the footer.
- *Source:* Dynamo Wires `[P4]`; 04b §6.1 (at most one incoming wire per input).

### F4 — Shift multi-move and Ctrl duplicate wires (P1)
- Shift+drag on a connected output grabs all wires leaving it: dropping on another output reattaches them, dropping on empty canvas deletes them.
- Ctrl+click on a connected input enters duplicate-connector mode: the same source can then be connected to several inputs; releasing Ctrl and clicking cancels the mode.
- *Source:* Dynamo Editing Wires `[P4]`, mouse interaction `[P5]`; 04 P1 "wire editing".

### F6 — Hide wires (P1)
- A canvas setting "Show connectors" hides all wires except those of the selected node (shown in aqua); the setting is saved in the `view` section.
- Right-clicking a node's output port offers "Hide wires", which hides only that output's wires.
- *Source:* Dynamo Hide Wires `[P4]`; 04b §6.1 (connector visibility in `view`).

### F7 — Wire highlight on selection (P1)
- Wires attached to the selected node render aqua; all other wires render grey. The highlight is per selection, not per node state.
- *Source:* Dynamo Default vs Highlighted Wires `[P4]`.

### F11 — Auto-layout (P1)
- Ctrl+L and a canvas-menu action run an auto-layout that produces a readable left-to-right graph without moving the selected node; the result is undoable as one canvas edit.
- *Source:* Dynamo Ctrl+L `[P5]`; 04a §4 canvas bullet; DynamoMCP `auto_layout_workspace` `[04a §1.1]`.

### F14 — Copy, paste, cut, Tab (P0)
- Ctrl+C / Ctrl+X / Ctrl+V copy, cut and paste the selection (nodes, wires between them, notes, groups); pasted content lands near the cursor with a small offset and keeps relative positions.
- Tab adds the nodes at the end of the highlighted wires to the current selection.
- *Source:* Dynamo shortcuts `[P5]`.

### F15 — Delete selection (P0)
- Delete removes the selected nodes and their wires; deleting a group removes only the group; deleting a node inside a group leaves the group. Deletion is undoable as one canvas edit.
- *Source:* Dynamo shortcuts `[P5]`.

### F16 — Selection box semantics (P0)
- Dragging left→right draws a solid box and selects only items completely inside it; dragging right→left draws a dashed box and selects anything it touches.
- Shift+click adds to the selection; Alt+click removes a node from its group; Ctrl+A selects all nodes, notes and groups.
- *Source:* Dynamo Workspace mouse interaction `[P5]`, `[P2]`.

### F17 — Context menus (P0)
- Right-click a node body: Rename, Freeze, Preview toggle, Lacing, List@Level, Group, Ungroup, Note, Hide wires, Is Input/Is Output, Node to Code.
- Right-click an input port: default value options, List@Level, hide this wire. Right-click a group: style, colour, rename. Right-click a note: edit, pin to node. Right-click the background: search, add node, auto-layout, export image.
- *Source:* Dynamo mouse interaction `[P5]`; node anatomy and default values `[P4]`; 04b §6.1 (levels/defaults).

### F18 — Input port default values (P1)
- Right-clicking an input port lists the node's default values and a "Use default" toggle; using a default removes the wire and marks the port as defaulted in the file.
- *Source:* Dynamo Node anatomy #4 `[P4]`; 04b §6.1 port field `using_default`.

### F19 — Pan and zoom controls (P0)
- Middle-drag pans, middle-scroll zooms around the cursor, and the icons zoom to fit / in / out. Ctrl+0 zooms to extents, then to origin, then to the selection (Dynamo's three-step behaviour); Ctrl+= and Ctrl+- zoom in and out.
- The zoom/pan controls are visible in the canvas corner and are also reachable by mouse.
- *Source:* Dynamo Zoom/Pan Controls `[P2]`, shortcuts `[P5]`.

### F20 — Create custom node from selection (P1)
- Ctrl+D creates a new custom node (`.obgf`) from the current selection and replaces the selection with an instance; Ctrl+Shift+N creates an empty custom node. Both open the definition in a new tab.
- *Source:* Dynamo shortcuts `[P5]`; 04 P1 custom nodes; 04a §4 (`.obgf`).

### H2 — Ctrl+D create custom node from selection (P1)
- Ctrl+D creates a new custom node (`.obgf`) from the current selection and replaces the selection with an instance; Ctrl+Shift+N creates an empty one (see F20 for the full gesture).
- Both open the definition in a new tab; editing the definition updates every instance in every open graph.
- *Source:* Dynamo shortcuts `[P5]`; 04 P1 custom nodes; 04a §4 (`.obgf`).

### G2 — Code Block editor UX (P1)
- The Code Block editor has syntax highlighting, autocomplete for registry nodes and variables, and a live list of its input ports and outputs as the text changes.
- Free variables become input ports and each line ending in `;` becomes an output; the port list updates without leaving the editor.
- Errors are underlined inline and reported in the node's tooltip, not only in the footer.
- *Source:* Dynamo Code Block demo `[P3]`, `[04a §1.5]`; 04 language bullets; 04a gap table (editor widget, P1).

### G3 — Dot-notation calls from Code Block (P1)
- Code Block can call command-registry nodes in dot notation (`Point.ByCoordinates(0,10)`, `pt.Add(vec)`, `pt.X`), matching Dynamo's create/action/query conventions.
- UI nodes (Watch, Watch 3D, selection pickers) are not callable from Code Block, as in Dynamo.
- *Source:* Dynamo DesignScript Syntax `[04a §1.5]`; 04 rule 9 (commands generated into nodes).

### G5 — Python editor UX (P1)
- The Python editor has syntax highlighting, completion for the `openbim` client API and the standard library, and inline error markers; it shows the pinned engine and client API version in its header.
- There is no engine dropdown: one pinned CPython 3 engine is recorded in the graph; a version mismatch blocks the run with a migration report.
- *Source:* 04a §1.7 (engine dropdown history), gap table Adopt P1; 04 A4; 04b §5/§6.1.

### G15 — Input node family (P0)
- The library provides Number, Number Slider, Integer Slider, String, Boolean, File Path and Directory Path input nodes in the Input category.
- Sliders carry a unit and min/max/step, show the unit in the node and in the preview bubble, and accept typed values outside the range with a warning.
- File and Directory Path inputs open a file picker and store the canonical path.
- *Source:* Dynamo Input nodes `[P3]`; 04 (sliders with units); 04 rule 7 (quantities).

### G16 — Unit nodes (P0)
- `Quantity.Parse("6 m/s")` builds a quantity from a string; `Convert` changes the display unit within a dimension; a display-format node sets precision and unit style.
- A dimension mismatch is a node error that names both dimensions, shown on the node and in the footer.
- *Source:* 04a gap table "Units / ForgeTypeId conversion nodes" Adapt P0; 04 rule 7.

### H5 — Preferences (P1)
- A Preferences panel exposes numeric display precision (decimal places), preview density/instance cap, default run mode, and whether the background preview starts enabled.
- Changes apply immediately to new evaluations and are stored per user, not in the graph file.
- *Source:* Dynamo Preferences `[P1]`, `[P3]`; 04a §3.11 (preview cap).

### I2 — Player search (P1)
- The Player folder list has a search field filtering by graph name and description; results keep their thumbnails and status.
- *Source:* Dynamo Player 2027 help `[04a §1.4]`.

### I7 — Player input grouping and ordering (P1)
- Player inputs are grouped by the canvas group each bound node belongs to, in group display order, and ordered by the graph's explicit input order within a group; ungrouped inputs come last.
- Two same-typed inputs remain distinct by binding, never merged by label.
- *Source:* 04a §3.9; 04b §6.1 `inputs[]` `order` and explicit bindings.

### J3 — Keyboard shortcut parity (P0 core / P2 full)
- **P0 core:** Ctrl+N (new), Ctrl+O (open), Ctrl+S (save), Ctrl+Shift+S (save as), Ctrl+Z / Ctrl+Y (canvas undo/redo), Ctrl+C / X / V (copy/cut/paste), Delete, F5 (run in Manual), Ctrl+0 / Ctrl+= / Ctrl+- (zoom), middle-drag pan, middle-scroll zoom, Shift/Ctrl wire editing, right-click canvas search, double-click canvas → Code Block, Ctrl+G (group), Ctrl+W (note), Ctrl+D (custom node from selection).
- **P2 full parity:** Ctrl+A (select all), Ctrl+B and Esc-hold (preview), Ctrl+I (isolation), Ctrl+K (unpin all previews), Ctrl+L (auto-layout), Ctrl+P (pan mode), Ctrl+T (template), Ctrl+Shift+I (insert graph), Ctrl+Shift+N (new custom node), Tab (add connected nodes), F1 (node help), Alt+click (remove from group).
- Every shortcut is shown in its tooltip and is remappable; conflicts are reported, not silently dropped.
- *Source:* Dynamo Keyboard Shortcuts `[P5]`; 04 (canvas undo/redo, run bar).

### J4 — Node display name `Category.Name` (P0)
- Nodes are displayed on the canvas as `Category.Name`, matching Dynamo's naming convention; the underlying type identity is `lib@version::name` and is what the file stores.
- Renaming a node (F12/F17) changes the display label only; it never changes the type, the ports or the bindings.
- *Source:* Dynamo Library naming `[P3]`; 04b §6.1 node `type`.

### J5 — Home workspace tabs (P2)
- One Home graph per window, as in Dynamo; additional graphs open in tabs only if this is adopted. Custom node tabs are unaffected.
- *Source:* Dynamo "Only one Home Workspace is allowed in each Dynamo window" `[P2]`; 04a gap table Adapt P2.

---

## 3. Parity acceptance list

The twenty interactions that must feel identical to Dynamo for Revit in OpenBIM Graph. "Dynamo behavior" is the contract; "OpenBIM" is what 04/04b already fixes.

| # | Interaction (must feel identical) | Dynamo behavior to match | OpenBIM |
|---|---|---|---|
| 1 | **Drag a node from the library to the canvas** — it appears at the drop point, centred on the cursor, and can be dragged again immediately. | Library → Workspace drag `[P3]` | Spec F1; 04 layout |
| 2 | **Search and place** — library keyword and dotted `library.category.node` search; Enter adds the highlighted node at the canvas centre; in-canvas search (right-click background + type) adds at the click point. | Library Search + "Search by hierarchy" `[P3]`; In-Canvas Search `[P2]` | Specs B4, B7; 04 library |
| 3 | **Hover a library entry** — a card with plain-language description, icon, and inputs/outputs with types and list structure. | Library hover card `[P3]` | 04 hover cards |
| 4 | **Hover a node** — a bubble under it shows the output value, list shape, `@Ln` level labels and units on quantities. | Node data preview `[P3]` | 04 bubbles; spec D3 |
| 5 | **Pin a bubble** — it stays after the mouse leaves and survives save/reopen. | Pin `[P3]`; 2.0 did not persist it `[04a]` | 04a Adopt; 04b `view` |
| 6 | **Watch node** — list tree with indices and level labels, values coloured by type, quantities with units; the node resizes to its content. | Watch `[P3]`; 2.15 colour-by-type + resize `[04a]` | 04; spec D3 |
| 7 | **Draw a wire** — from either end; dashed while dragging, solid when connected; an incompatible target is refused and an occupied input replaces its wire with a footer note. | Wires `[P4]` | Spec F3; 04b §6.1 |
| 8 | **Shift+drag a connected output** — grabs all its wires; drop on another output reattaches, drop on empty canvas deletes. **Ctrl+click a connected input** duplicates the connector for multi-target wiring. | Editing Wires `[P4]`, mouse interaction `[P5]` | Spec F4; 04 P1 |
| 9 | **Right-click a node** — Rename, Freeze, Preview toggle, Lacing, List@Level, Group, Note, Hide wires, Is Input/Is Output. | Node right-click `[P5]`; freeze `[P4]` | Spec F17; 04b §6.1 |
| 10 | **Freeze a node** — it turns translucent blue, stops executing, freezes everything downstream, and the state is saved in the file. | Freezing `[P4]` | 04 P1; 04b `frozen` |
| 11 | **Read node states** — Error > Warning > Info > Preview; blue/red input bars, yellow/red/blue status bars, hover tooltips, aqua selection border; footer counters click through to the node. | Node States `[P4]`; Footer `[04a §1.1]` | 04 refinements; spec C2 |
| 12 | **Run bar** — Automatic re-runs on every change but only refreshes the proposal; Manual runs on the Run button or F5; Cancel stops immediately and partial results are diagnostic only. | Execution bar `[P1]` | 04 refinements; 04b §3.1 |
| 13 | **Ctrl+Z / Ctrl+Y** — undo/redo the last canvas edit (move, wire, group, note, rename), never a model edit. | Workspace shortcuts `[P5]` | 04 refinements; 04b §3.2 |
| 14 | **Group / note / align / delete** — Ctrl+G groups with a style, Ctrl+W adds a note (pinnable), align arranges the selection, Delete removes it; all persisted in the `view` section. | Shortcuts `[P5]`; Graph Strategies `[04a §1.3]` | 04 P1; 04b `view` |
| 15 | **Selection box** — left→right solid box selects only fully contained items; right→left dashed box selects anything touched; Shift adds, Alt removes from group. | Workspace mouse interaction `[P5]` | Spec F16 |
| 16 | **Pan and zoom** — middle-drag pan, middle-scroll zoom, corner icons, Ctrl+0 extents/origin/selection, Ctrl+= / Ctrl+-. | Zoom/Pan `[P2]`; shortcuts `[P5]` | Spec F19; spec J3 |
| 17 | **Code Block** — double-click empty canvas creates it; units in literals, lists, dictionaries, ranges, `@Ln`, guides, `def`; free variables become ports, each `;` line an output; syntax highlighting and autocomplete in the editor. | Code Block `[P3]`; `[04a §1.5]` | 04 language; specs F2, G2, G3 |
| 18 | **Python node** — double-click opens the editor with highlighting and completion; `IN[i]` / `OUT`; a run returns a proposal the user reviews and never auto-commits; unapproved Python is refused before any import. | Python node `[04a §1.7]`; 4.3 review window `[04a §1.1]` | 04 A4; specs G5, G8; 04b §5 |
| 19 | **Selection input node** — click it, pick element(s) in the main app; the value appears on the node with quantity/unit; it is usable as a Player input; category, type, level and view pickers available. | `Select Model Element` in Revit `[04a §1.8]` | 04 selection inputs (picked in the main app) |
| 20 | **Player** — folder list with thumbnail, description, doc link and last-run status; typed inputs validated before the run; unsupported input → refused, not run; "Open in OpenBIM Graph" opens the canvas; the run returns the same proposal review. | Player `[04a §1.4]` | 04 A5; specs I2, I7 |

**Next tier (21–25, P1–P2, same standard once scheduled):**
21. Trust and install dialogs — opening a Python graph shows the trusted-local-code approval with the no-filesystem/network warning; locked packages need a separate install consent listing artifacts/origins; approvals invalidate on code/dependency/revision change (04 rule 11, 04b §5).
22. Per-node preview toggle — the eye on/off; on → geometry appears in the main app's 3D view; off → grey bar and eye icon (04 P1; `[P4]`).
23. Save/reopen fidelity — nodes, wires, lacing, levels, guides, frozen state, groups, notes, reroute pins, pinned bubbles, preview flags and run mode all survive; layout edits do not change the executable digest (04b §6).
24. State feedback on run — "pending (proposal)" and "stale (model changed since read)" states, plus the footer's run message and counters (04 refinements; spec C7).
25. Graph MCP tools — create/connect nodes, set lacing and guides, groups, notes, auto-layout, run; graph/source/code/run capabilities distinct, none can confirm a model commit (04; 04b §5).

---

## 4. Revit-specific exclusions

Dynamo-for-Revit behaviors with no equivalent. **Do not copy them**; the OpenBIM equivalent chosen in 04 is given in the last column.

| Dynamo-for-Revit behavior | Why it is excluded | OpenBIM equivalent already chosen in 04 |
|---|---|---|
| **Element binding UI (trace)**: `Bindings[]` in the file, call-site trace, modify/reuse/replace semantics, per-document scope, SOAP trace data `[04a §1.8]` | Host integration tied to the Revit document and .NET type names; opaque and project-specific. | **Managed outputs**: stable row/output keys in project-scoped ownership mappings, per-node mode update / create-new / detach, proposal shows modify/create/delete counts (04; 04b §3.2). |
| **Transaction nodes** (`Transaction.Start` / `End`) and the Revit UI-thread execution envelope `[04a §1.8]` | Revit's API requires a transaction; ordering bugs come from that model. | **No transaction nodes.** Reads see the fixed run revision; creating nodes return pending handles; writes are proposals (04 execution contract; 04b §3.1). |
| **Revit selection modes**: `Select Model Element` interactive button, `Categories` + `All Elements of Category`, Revit category/type/level/view pickers `[04a §1.8]` | Pickers drive the Revit canvas and Revit enums (`BuiltInCategory`). | **Selection input nodes** pick element(s) in the main app, with category/type/level/view pickers; each can be a Player input (04). |
| **`Element.SetParameterByName`** exact case-sensitive free-text match `[04a §1.8]` | Revit API string matching; fragile across language and renames. | **Stable parameter IDs** (`duct.width`) mapped to IFC property sets, EN/FR labels accepted with a warning (04 rule 8). |
| **Revit project units / unitless values**: "5" means 5 mm or 5 ft by project; `DisplayUnitType` → `ForgeTypeId` `[04a §1.8]` | Revit project settings and Revit enums have no IFC equivalent. | **Quantities everywhere**: SI number + dimension; MCP `{"value": 6.0, "unit": "m/s"}`; dimension mismatch is an error (04 rule 7). |
| **Python Revit API boilerplate**: `clr.AddReference('RevitNodes')`, `RevitServices`, `DocumentManager`, `UnwrapElement`/`ToDSType`, `TransactionManager` `[04a §1.7]` | Wraps the Revit .NET API and transactions. | **The `openbim` client** (same API as MCP) returning proposals; no host boilerplate (04 A4). |
| **Watch 3D inside Revit's canvas** (background preview in the host) `[04a §1.8]` | The preview surface is Revit's own 3D view. | **3D preview in the main app's 3D view**; optional in-canvas viewer as P1 (04; spec A6). |
| **Generative Design for Revit** runner `[04a §1.4]` | A Revit-hosted study runner, not part of Dynamo's canvas UX. | **Not promised**; input/output schema keeps min/max/step so a runner can be added later (04; 04a gap table). |
| **`.dyn` `.NET` `ConcreteType` names, SOAP bindings, base64 thumbnail** `[04a §1.10]` | Host .NET type names and binary blobs make files non-portable and non-diffable. | **Own schema**: node types `lib@version::name`, no bindings, sidecar PNG thumbnail, canonical bytes (04b §6). |
| **Bindings cleanup when reusing a graph across projects** `[04a §1.8]` | Bindings are per-document Revit data. | **Project-scoped ownership mappings live in the project op log**, never in the graph (04b §3.2). |
| **Host UI hang while a graph runs** (in-process hosts run the scheduler on the UI thread) `[04a §1.8]` | A Revit-host threading artifact, not a UI feature. | **Separate process**, cancel and per-node progress; partial results diagnostic only (04; 04a §3.4). |

---

## 5. Deliberate differences

Where the project intentionally differs from Dynamo and why the user still gets an equivalent experience. One line each.

1. **Units everywhere** — every value is a quantity (SI + dimension), so "5" can never mean 5 mm in one model and 5 ft in another; dimension mismatches are node errors naming both dimensions (04 rule 7).
2. **Proposals instead of auto-commit** — Automatic mode recomputes previews and proposals only; every client, including Player, requires explicit per-run review before a model commit (04 refinements; 04b §5).
3. **Per-element commit rules** — bulk edits apply per element with last-writer-wins on unlocked elements, change requests on locked ones, and all-or-nothing groups for whole routes (04 rule 5; 04b §3).
4. **No Periodic mode** — the timer mode is not provided; a file-change trigger is the candidate replacement (04; 04a gap table).
5. **Own graph file format** — `.obg` JSON with graph logic separated from a `view` section, own versioned schema and canonical bytes; no `.dyn` compatibility promise (04; 04b §6).
6. **Preview in the main app** — geometry previews live in the main app's 3D view, so the graph app stays a pure editor; per-node preview toggles and an optional in-canvas viewer cover the rest (04; spec A6).
7. **No transaction nodes** — reads see the fixed run revision and creating nodes return pending handles, so ordering is explicit and queries never see pending elements (04; 04b §3.1).
8. **One pinned Python engine** — a single pinned CPython 3 with engine/ABI/client API recorded in the graph, avoiding Dynamo's IronPython2 → CPython3 → PythonNet3 churn (04 A4; 04a §1.7).
9. **No public package manager** — a shared team library folder plus a version lock in the graph (`dependencies[]` with SHA-256) replaces the online registry (04; 04a gap table).
10. **Revision-bound trust approvals instead of trusted folders** — informed code approval and separate install consent, bound to graph/code/dependency/environment revisions; the graph stores policy, never saved consent (04 rule 11; 04b §5).
11. **Sidecar thumbnail instead of base64** — thumbnails are a separate PNG next to the graph, so graph files stay diffable and Git-friendly (04; 04b §6).
12. **Stable parameter IDs instead of case-sensitive names** — labels resolve to IDs at edit time and the ID is stored, so renaming a label does not break graphs (04 rule 8).
13. **Canvas undo separate from model undo** — Ctrl+Z/Y edit the graph only; the model gets one named undo entry per committed run (04 refinements; 04b §3.2).
14. **No imperative DesignScript blocks in v1** — the Code Block is a familiar, expression-oriented subset; `def` and `if` expressions are included (04; 04a gap table).
15. **Typed Player input schema instead of Data-Shapes forms** — quantity+unit+range, enum, element filter and file inputs, validated before the run and grouped by canvas group (04 A5; 04a §3.9).

---

## 6. Open questions — resolved 20 Oct 2026

All five open questions are decided; 04 "UI parity with Dynamo for Revit" is authoritative:

1. `.dyn` import: **out of scope**; the no-compatibility promise stands.
2. In-canvas 3D preview / Watch 3D node: **adopted at P1** (specs A6, D6).
3. Home workspace: **one Home graph per window**, as in Dynamo; no Home tabs (spec J5).
4. Node Autocomplete: **promoted to P1**; Node to Code stays P2 (specs B9, B10).
5. Periodic replacement: **"re-run on file change" trigger** added as spec E9 in 04.

---

## 7. Sources

**Repo docs (read-only):** `revit-alike/docs/revit-style-ui/04-visual-automation.md`, `04a-dynamo-review.md`, `04b-replication-and-serialization.md`, `05-backlog.md`.

**Fetched Dynamo sources (20 Oct 2026):**
- `[P1]` Dynamo Primer — User Interface: https://primer2.dynamobim.org/3_user_interface
- `[P2]` Dynamo Primer — Workspace: https://primer2.dynamobim.org/3_user_interface/1-workspace.md
- `[P3]` Dynamo Primer — Library: https://primer2.dynamobim.org/3_user_interface/2-library.md
- `[P4]` Dynamo Primer — Nodes and Wires: https://primer2.dynamobim.org/4_nodes_and_wires.md
- `[P5]` Dynamo Primer — Keyboard Shortcuts: https://primer2.dynamobim.org/a_appendix/a-6_dynamo-keyboard-shortcuts.md

**Cited inside 04a (not re-fetched here):** Autodesk help pages (Graph Security, Player, Requiring Input, Unit Nodes), dynamobim.org release posts (2.0, 2.7, 2.13, 2.15, 3.3, 4.0), the Dynamo wiki (Release Notes, Integration Guide, UI Performance, Replication Part 3), the Dynamo Primer pages for custom nodes, packages, DesignScript, Python and Revit integration, the AU handout, and the pinned Dynamo source snapshot `8a211c5f306e5b54541fa57081aa1ffa32a65e90`.

**Not verified in this study (carried as [unverified]):** exact placement point of in-canvas search results; current 4.x behaviour of the historical "broken shortest lacing" wiki claim (04b §2.1/§3 keeps it unasserted); current element-binding encoding (04a §1.8); how Revit groups a Dynamo run in its undo stack (04a §3.12).
