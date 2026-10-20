# Dynamo for Revit: features and UX, compared with the OpenBIM Graph design

Date: 17 Oct 2026. Compared against `revit-alike/docs/revit-style-ui/04-visual-automation.md` ("04").
Sources are linked inline. **[unverified]** identifies claims not established by the cited evidence. The release posts/wiki examined on 17 Oct 2026 describe 4.x, including 4.3; older documentation below retains its historical scope. This inventory is research, not an assertion of uniform current-version behavior. Proposed OpenBIM changes are subordinate to corrected 04 and the normative supported subset/gates in [04b](04b-replication-and-serialization.md).

---

## 1. Dynamo features

### 1.1 Shell and UI layout
- **Five regions:** Menus, Toolbar, Library, Workspace, Execution bar ([Primer: UI](https://primer2.dynamobim.org/3_user_interface)). The toolbar has New/Open/Save, Undo (Ctrl+Z), Redo (Ctrl+Y) and "Export Workspace as Image" (PNG).
- **Workspace tabs:** one Home workspace (`.dyn`) per window, plus any number of Custom Node workspaces (`.dyf`) in tabs ([Primer: Workspace](https://primer2.dynamobim.org/3_user_interface/1-workspace.md)).
- **Graph Preview and 3D background preview:** you switch between them with icons, the right-click menu or Ctrl+B. The 3D mode has orbit, pan and zoom ([Workspace](https://primer2.dynamobim.org/3_user_interface/1-workspace.md)). Right-clicking the canvas opens in-canvas search.
- **Execution bar** ([UI](https://primer2.dynamobim.org/3_user_interface)):
  - **Automatic:** the graph re-runs on every change.
  - **Manual:** the graph runs only when you click Run.
  - **Periodic:** runs on a timer. It is greyed out unless the graph contains `DateTime.Now`.
  - The run mode and period are saved in the file (`"RunType": "Manual", "RunPeriod": "1000"`) ([sample .dyn](https://github.com/DynamoDS/Dynamo/blob/8a211c5f306e5b54541fa57081aa1ffa32a65e90/doc/distrib/Samples/en-US/Basics/Basics_Basic01.dyn)).
- **Graph Footer and Graph Node Manager (2.15):**
  - The footer shows run messages ("Run completed", "Run blocked") and counters that let you click through to nodes with warnings or errors.
  - The Node Manager lists every node with its type and state ([2.15 release](https://dynamobim.org/dynamo-core-2-15-release/)).
- **TuneUp (packaged extension, improved in 3.3):**
  - Shows the execution order and the time each node took.
  - Can force a full re-run. The Run button only executes nodes that changed ([3.3 release](https://dynamobim.org/dynamo-core-3-3-release/)).
- **Node Autocomplete:** release notes describe port-based suggestions and updates in 3.x/4.x ([Release Notes](https://github.com/DynamoDS/Dynamo/wiki/Release-Notes)). The Unit Nodes page is not evidence for autocomplete; no navigation-only citation is used.
- **DynamoMCP (4.2+), a built-in MCP server used by Autodesk Assistant** ([Primer: Dynamo MCP](https://primer2.dynamobim.org/12_ai_tools/1-what-is-dynamo-mcp)):
  - **Read tools:** `get_workspace_info`, `get_node_connections`, `get_node_lacing`, `get_workspace_warnings`, `get_node_output_value` (paged).
  - **Edit tools:** `create_nodes`, `connect_nodes`, `set_node_lacing`, `set_node_port_list_at_level`, `create_group`, `create_notes`, `auto_layout_workspace`.
  - **Run tools:** `run_workspace`, `run_graph_from_path` (with input values), `run_graph_from_json`, `get_graph_info_from_path` (reads inputs and outputs).
  - Tools named `get_` are read-only. 4.3 added a review window that shows the assistant's edits to Python nodes before and after, so the user can accept or reject them ([Release Notes](https://github.com/DynamoDS/Dynamo/wiki/Release-Notes)).

### 1.2 Library and search
- **Hierarchy:** Library > Category > Subcategory > node, with each category split into **Create / Action / Query** ([Primer: Library](https://primer2.dynamobim.org/3_user_interface/2-library.md)).
- **Hover card:** description, icon, and the inputs and outputs with their types and list structure.
- **Search:** by keyword or by dotted hierarchy (`library.category.nodeName`). Enter places the highlighted node.
- **Add-ons section:** since 2.0, custom nodes, packages and DLLs appear in a separate "Add-ons" section. The library panel is a web component that can be resized and collapsed ([Dynamo 2.0](https://dynamobim.org/to-dynamo-2-0-and-beyond/)).

### 1.3 Nodes, wires and states
- **Node anatomy:**
  - The name follows `Category.Name`, and the node has a main body.
  - Ports carry type tooltips, and input ports can have right-click default values.
  - A lacing icon shows the lacing setting ([Primer: Nodes and Wires](https://primer2.dynamobim.org/4_nodes_and_wires)).
- **States:**
  - Precedence is Error > Warning > Info > Preview.
  - Input bars show which inputs are connected (blue) and which are missing (red). A grey output bar marks a function output.
  - Other states: selected (aqua), frozen (translucent blue), preview off (grey bar and eye icon).
  - Warning (yellow), error (red) and info (blue) each have a hover tooltip ([Nodes and Wires](https://primer2.dynamobim.org/4_nodes_and_wires)).
  - Info and warning bubbles have a copy-to-clipboard button ([Release Notes](https://github.com/DynamoDS/Dynamo/wiki/Release-Notes)).
- **Freeze:** stops a node from executing and freezes everything downstream of it ([Nodes and Wires](https://primer2.dynamobim.org/4_nodes_and_wires)). The Primer suggests freezing heavy Revit edits while you build the graph ([Editing](https://primer2.dynamobim.org/7_dynamo_for_revit/3-editing)).
- **Wires:**
  - You can start a wire from either end. Shift+click moves several wires at once; Ctrl+click duplicates a wire.
  - Wires can be hidden globally or per output ([Nodes and Wires](https://primer2.dynamobim.org/4_nodes_and_wires)).
  - Reroute points are saved as `ConnectorPins` in the view section ([sample .dyn](https://github.com/DynamoDS/Dynamo/blob/8a211c5f306e5b54541fa57081aa1ffa32a65e90/doc/distrib/Samples/en-US/Basics/Basics_Basic01.dyn)).
- **Preview bubbles:** hovering a node shows its output (the "node data preview"), and the bubble can be pinned ([Library](https://primer2.dynamobim.org/3_user_interface/2-library.md)). In 2.0 the pin state was not saved ([Dynamo 2.0](https://dynamobim.org/to-dynamo-2-0-and-beyond/)). Pinned previews are now a node property that MCP can set ([Dynamo MCP](https://primer2.dynamobim.org/12_ai_tools/1-what-is-dynamo-mcp)).
- **Watch nodes:** Watch (text and list tree), Watch 3D and Watch Image are in the Display/View category ([AU handout](https://static.au-uw2-prd.autodesk.com/Class_Handout_CI322842_Stacey_Morykin.pdf); [Library](https://primer2.dynamobim.org/3_user_interface/2-library.md)). Since 2.15 the Watch node resizes itself and colours values by type ([2.15](https://dynamobim.org/dynamo-core-2-15-release/)).
- **Groups, notes, pins, alignment and renaming:**
  - Groups take colours and styles (Inputs / Actions / Outputs / Review).
  - Notes can be pinned to a node.
  - Nodes can be aligned and renamed ("Rename Node…") ([Graph Strategies](https://primer2.dynamobim.org/9_best_practices/1-graph-strategies.md); [Dynamo MCP](https://primer2.dynamobim.org/12_ai_tools/1-what-is-dynamo-mcp)).
- **Node to Code:** turns a selection of nodes into one Code Block ([What's a Code Block](https://primer2.dynamobim.org/8_coding_in_dynamo/8-1_code-blocks-and-design-script/1-what-is-a-code-block.md)).
- **Custom nodes (`.dyf`):** a container of nodes. Editing the definition updates every instance. A saved `.dyf` appears under Add-ons ([Custom Nodes](https://primer2.dynamobim.org/6_custom_nodes_and_packages/6-1_custom-nodes/1-introduction.md)). Custom nodes support lacing and default values ([Release Notes](https://github.com/DynamoDS/Dynamo/wiki/Release-Notes)).
- **Packages:** an online Package Manager and the dynamopackages.com site show dependencies and host/version compatibility ([Packages](https://primer2.dynamobim.org/6_custom_nodes_and_packages/6-2_packages/1-introduction.md)). Package Manager requests are turned off in no-network mode ([Release Notes](https://github.com/DynamoDS/Dynamo/wiki/Release-Notes)).
- **Security (2.15+):**
  - With the default security warning enabled, untrusted graphs are blocked until trusted once/session or via a trusted folder. Autodesk's cited help also documents disabling the warning; blocking is not unconditional.
  - Trusted Locations are managed under Preferences > Security ([Graph Security](https://help.autodesk.com/cloudhelp/2023/ENU/RevitDynamo/files/RevitDynamo_About_Dynamo_Graph_Security_html.html); [2.15](https://dynamobim.org/dynamo-core-2-15-release/)).

### 1.4 Player and Generative Design
- **Dynamo Player** (Revit Manage tab > Visual Programming):
  - Lists the graphs in a folder, with thumbnail, description, documentation link and status.
  - Has search, input entry, and "Edit in Dynamo" ([Player 2027](https://help.autodesk.com/cloudhelp/2027/ENU/RevitDynamo/files/RevitDynamo_Dynamo_Player_html.html)).
- **Player inputs:** the author ticks **Is Input** on a node.
  - Supported input types include Select Model Element(s), Categories, Element Types, File/Directory Path, String, Number, Boolean, sliders, Levels and Views.
  - **Is Output** and named Watch nodes appear as outputs.
  - Unsupported input types are marked and ignored, so the run fails if that value was required ([Requiring Input](https://help.autodesk.com/cloudhelp/2023/ENU/RevitDynamo/files/Dynamo_Player/RevitDynamo_Dynamo_Player_About_Requiring_Input_for_Dynamo_Graphs_html.html)).
- **Generative Design for Revit:** uses the same Is Input / Is Output flags. Sliders need min, max and step, and outputs must be Watch nodes of type Number ([generativedesign.org](https://www.generativedesign.org/03-hello-gd-for-revit/03-02_setting-up-a-graph-for-gd-for-revit); [Autodesk GD for the Dynamo author](https://help.autodesk.com/cloudhelp/2024/ENU/Revit-GDiR/files/GUID-BA7949DA-C2F8-4B00-9EC0-16180E0A6663.htm)).
- **DynamoCLI (headless):** `-o file.dyn` runs a graph, `-v` writes every node's output to XML, `-g` writes geometry to JSON, and `-i` imports node DLLs ([CLI](https://primer2.dynamobim.org/1_developer_primer_intro/3_developing_for_dynamo/9-command-line-interface.md)).

### 1.5 DesignScript and Code Block
- **Basics:**
  - Double-click the canvas to create a Code Block.
  - Free variables become input ports. Each line ending in `;` becomes an output ([What's a Code Block](https://primer2.dynamobim.org/8_coding_in_dynamo/8-1_code-blocks-and-design-script/1-what-is-a-code-block.md)).
- **Calls in dot notation** ([DesignScript Syntax](https://primer2.dynamobim.org/8_coding_in_dynamo/8-1_code-blocks-and-design-script/2-design-script-syntax)):
  - Create: `Point.ByCoordinates(0,10)`.
  - Action: `pt.Add(vec)`.
  - Query: a property with no parentheses.
  - Most library nodes can be called this way. UI nodes such as Watch 3D cannot.
- **Shorthand** ([Shorthand](https://primer2.dynamobim.org/8_coding_in_dynamo/8-1_code-blocks-and-design-script/3-shorthand)):
  - Range `0..10..1`; sequence `0..#15..2`; count-divided range `0..1..#50`.
  - Indexing `list[0]`, nested `l[0][1]`, slices `l[0..15]`.
  - Operators, and `true`/`false` in lower case.
- **2.0 syntax:**
  - Lists are written `[a,b]`, dictionaries `{"k":v}`, and dictionary keys can only be strings ([Dictionaries](https://primer2.dynamobim.org/5_essential_nodes_and_concepts/5-5_dictionaries-in-dynamo/1-what-is-a-dictionary.md); [Dynamo 2.0](https://dynamobim.org/to-dynamo-2-0-and-beyond/)).
  - List@Level is written `@L2`.
  - Overloads that differ only by rank were removed.
  - Associative variables became immutable, and Imperative blocks have their own local scope ([Language Changes](https://primer2.dynamobim.org/8_coding_in_dynamo/1-dynamo-2.0-language-changes.md)).
- **Functions:** `def Name(a,b){ return a+b; };`. A function-defining Code Block has no ports and is called from other Code Blocks ([Functions](https://primer2.dynamobim.org/8_coding_in_dynamo/8-1_code-blocks-and-design-script/4-functions.md)).

### 1.6 Lists: lacing, replication and List@Level
- **Lacing:**
  - The Primer describes Shortest List as default, with Longest List and Cross Product available from the node menu ([What's a List](https://primer2.dynamobim.org/5_essential_nodes_and_concepts/5-4_designing-with-lists/1-whats-a-list.md)). This wording is not a version-independent statement about every node.
  - MCP exposes "Auto" as a fourth value ([Dynamo MCP](https://primer2.dynamobim.org/12_ai_tools/1-what-is-dynamo-mcp)); applicable nodes in the pinned sample store `"Replication": "Auto"`, while some store Disabled ([sample .dyn](https://github.com/DynamoDS/Dynamo/blob/8a211c5f306e5b54541fa57081aa1ffa32a65e90/doc/distrib/Samples/en-US/Basics/Basics_Basic01.dyn)). Pinned NodeModel defaults/lowering are scoped in 04b, not inferred from the Primer's Shortest description.
- **Replication:** a function automatically iterates over a list whose rank is higher than its parameter's rank, using Zip or Cartesian replication ([What's a List](https://primer2.dynamobim.org/5_essential_nodes_and_concepts/5-4_designing-with-lists/1-whats-a-list.md)).
- **Replication guides** `xs<1>`, `ys<2>`, `xs<1L>` ([wiki Part 3](https://github.com/DynamoDS/Dynamo/wiki/Replication-and-Replication-Guide-Part-3)):
  - Guides are processed level by level, and before replication.
  - At each level, an equal guide value on several arguments means Zip; a unique value means Cartesian.
  - The `L` suffix switches Zip to longest lacing.
  - The historical wiki (edited 2022) said UI shortest was "broken" and estimated 80% coverage. Neither is asserted as a measured/current 4.x defect or coverage figure.
- **List@Level:** introduced in 1.2.
  - A per-input-port level spinner, plus a "Keep list structure" option.
  - It replaced most uses of List.Map and List.Combine ([List@Level blog](https://dynamobim.org/introducing-listlevel-working-with-lists-made-easier/); [Lists of Lists](https://primer2.dynamobim.org/5_essential_nodes_and_concepts/5-4_designing-with-lists/3-lists-of-lists.md)).
  - Each input port is saved as `Level`, `UseLevels` and `KeepListStructure` ([sample .dyn](https://github.com/DynamoDS/Dynamo/blob/8a211c5f306e5b54541fa57081aa1ffa32a65e90/doc/distrib/Samples/en-US/Basics/Basics_Basic01.dyn)).
- **Top-down indexing:** `List.GetItemAtIndex(0)` on a list of lists returns the first sublist, not the first item of each sublist ([Lists of Lists](https://primer2.dynamobim.org/5_essential_nodes_and_concepts/5-4_designing-with-lists/3-lists-of-lists.md)).
- **List nodes:** Count, Flatten, Transpose, Chop, Map, Combine, and others ([Working with Lists](https://primer2.dynamobim.org/5_essential_nodes_and_concepts/5-4_designing-with-lists/2-working-with-lists.md); [Lists of Lists](https://primer2.dynamobim.org/5_essential_nodes_and_concepts/5-4_designing-with-lists/3-lists-of-lists.md)).

### 1.7 Python Script node
- **Basics:** opened from Script > Editor > Python Script. Double-click opens the editor. Inputs arrive in the `IN` list and the result goes in `OUT` ([Python Nodes](https://primer2.dynamobim.org/8_coding_in_dynamo/8-3_python/1-python.md)).
- **Engine history:**
  - 2.7 added CPython3 next to IronPython2, with an engine dropdown in each node's editor ([2.7](https://dynamobim.org/dynamo-core-2-7-release/)).
  - 2.13 made CPython3 the default. IronPython2 was removed from core and is now the `DynamoIronPython2.7` package ([2.13 pt 3](https://dynamobim.org/dynamo-core-2-13-release-part-3-3/)).
  - 4.0 made PythonNet3 the default, replacing CPython3 ([4.0](https://dynamobim.org/dynamo-core-4-0-release/); [Release Notes](https://github.com/DynamoDS/Dynamo/wiki/Release-Notes)).
  - PythonNet3 can load PyPI C-extension libraries (numpy, pandas, openpyxl) ([Migrating to PythonNet3](https://primer2.dynamobim.org/1_developer_primer_intro/0-introduction/2-migrating-to-pythonnet3.md)).
- **Revit access:**
  - Uses `clr.AddReference('RevitNodes')`, `RevitServices` and `DocumentManager` ([Python and Revit](https://primer2.dynamobim.org/8_coding_in_dynamo/8-3_python/2-python-revit.md)).
  - Writes must be wrapped in `TransactionManager.Instance.EnsureInTransaction(doc)` … `TransactionTaskDone()` ([Dynamo Python Primer](https://dynamopythonprimer.gitbook.io/dynamo-python-primer/4-revit-specific-topics/working-with-transactions)).
  - Wrapper-conversion details for `UnwrapElement` / `ToDSType` / `ToProtoType` remain **[unverified in this evidence set]** and are not OpenBIM requirements.
- **Performance tip:** keep geometry inside Python or ZeroTouch nodes to avoid tessellating every intermediate result ([Large Data Sets](https://primer2.dynamobim.org/9_best_practices/5-efficiently-working-with-large-datasets.md)).

### 1.8 Revit integration
- **Scope:** Dynamo runs only in the file it was opened from ([Revit Connection](https://primer2.dynamobim.org/7_dynamo_for_revit/1-the-revit-connection.md)).
- **Selection:**
  - `Select Model Element` is an interactive button.
  - `Categories` + `All Elements of Category` get elements without converting their geometry, which keeps previews light ([Selecting](https://primer2.dynamobim.org/7_dynamo_for_revit/2-selecting.md)).
- **Parameters:** `Element.Parameters` lists them. `Element.SetParameterByName` matches the **exact, case-sensitive** name ([Editing](https://primer2.dynamobim.org/7_dynamo_for_revit/3-editing)).
- **Units:**
  - Dynamo is unitless and uses the Revit project's units: "5" becomes 5 mm or 5 ft depending on the project ([Editing](https://primer2.dynamobim.org/7_dynamo_for_revit/3-editing); [Working with Revit](https://dynamobim.org/issues/working-with-revit/)).
  - Since 2.13, Forge unit nodes `Unit/Quantity/Symbol.ByTypeID`, `Convert By Units` and `ParseExpressionByUnit` are available ([Unit Nodes](https://help.autodesk.com/view/RVT/2025/ENU/?guid=RevitDynamo_Dynamo_for_Revit_Nodes_Unit_Nodes_html); [2.13 pt 3](https://dynamobim.org/dynamo-core-2-13-release-part-3-3/)).
  - Revit 2022 replaced the `DisplayUnitType` enum with `ForgeTypeId`, which broke unit code in packages and Python ([archi-lab](https://archi-lab.net/handling-the-revit-2022-unit-changes/)).
- **Element binding (trace):**
  - The integration guide demonstrates call-site trace in `.dyn` Bindings and modify/reuse/replace behavior with a **historical base64 SOAP example**, not verified current 4.x encoding. Release Notes explicitly describe SOAP as pre-3.0 trace data ([Element Binding doc](https://github.com/DynamoDS/Dynamo/blob/master/doc/integration_docs/2_elementBinding.md); [Release Notes](https://github.com/DynamoDS/Dynamo/wiki/Release-Notes)). Current exact encoding remains unverified here; OpenBIM ownership does not depend on copying it.
  - The historical integration guide describes default-on binding and integration-level control; this evidence does not establish that every current node/host lacks a per-node switch.
  - Bindings apply to one document only, so a graph reused across projects should have its bindings cleaned. Python and custom nodes do not necessarily bind ([forum](https://forum.dynamobim.com/t/element-binding-in-revit/31612/1)).
  - In DynamoRevit source snapshot `49cd43c5c3b21271640ae043b8b77374e4796843` (not asserted to be a released host version), SetupPythonEngine disables ElementBinder at evaluation start and OnPythonEvalFinished restores it ([source lines 445–525](https://github.com/DynamoDS/DynamoRevit/blob/49cd43c5c3b21271640ae043b8b77374e4796843/src/DynamoRevit/Models/RevitDynamoModel.cs#L445-L525)); this is source evidence, not a search snippet or guarantee for every Python engine.
- **Transactions and threading:**
  - Nodes call `TransactionManager.EnsureInTransaction`.
  - In Revit, execution is scheduled on the UI thread through IdlePromises "within the transaction envelope that Revit needs" ([Integration Guide](https://github.com/DynamoDS/Dynamo/wiki/Integration-Guide)). Revit's UI therefore hangs while a graph runs ([UI Performance](https://github.com/DynamoDS/Dynamo/wiki/Dynamo-UI-Performance)).
  - Users struggle with `Transaction.Start/End` nodes and with ordering: a collector does not see a sheet created earlier in the same run ([forum](https://forum.dynamobim.com/t/transactions-nodes-and-how-to-use-them/43759)).

### 1.9 Execution model
- **Compilation:** nodes compile to DesignScript AST, which the DesignScript VM runs. UI nodes "emit AST that calls the implementation" ([Integration Guide](https://github.com/DynamoDS/Dynamo/wiki/Integration-Guide)).
- **Incremental runs:** "delta compute": unchanged nodes keep their output, and Run executes only the nodes that changed ([3.3 / TuneUp](https://dynamobim.org/dynamo-core-3-3-release/)). Changing lacing triggers a re-run ([Release Notes](https://github.com/DynamoDS/Dynamo/wiki/Release-Notes)). These cited sources do not establish exact dirty propagation **[unverified detail]**; OpenBIM uses conservative generation-scoped invalidation (04b §4).
- **Threading:** in-process hosts run the scheduler on the UI thread. Out-of-process hosts (FormIt, Alias) do not hang their UI ([UI Performance](https://github.com/DynamoDS/Dynamo/wiki/Dynamo-UI-Performance)).

### 1.10 `.dyn` file structure (JSON since 2.0)
- **Why JSON:** 2.0 moved from XML to JSON so that graphs could run headless or in Player-like environments. The design splits **graph logic** from **UI description** ([Dynamo 2.0](https://dynamobim.org/to-dynamo-2-0-and-beyond/); proposal [#7747](https://github.com/DynamoDS/Dynamo/issues/7747)).
- **Top-level keys** (from the Basics sample): `Uuid`, `IsCustomNode`, `Name`, `Description`, `ElementResolver`, `Inputs[]`, `Outputs[]`, `Nodes[]`, `Connectors[]`, `Dependencies[]`, `NodeLibraryDependencies[]`, `Thumbnail` (base64 JPEG), `GraphDocumentationURL`, `ExtensionWorkspaceData[]`, `Bindings[]`, `View{}` ([sample .dyn](https://github.com/DynamoDS/Dynamo/blob/8a211c5f306e5b54541fa57081aa1ffa32a65e90/doc/distrib/Samples/en-US/Basics/Basics_Basic01.dyn)).
  - `Inputs` and `Outputs` are the **Player/GD interface**: Id, Name, Type, Value, Description.
  - `Nodes` entries hold `ConcreteType` (the .NET type), `Id`, `NodeType`, `FunctionSignature` (e.g. `+@var[]..[],var[]..[]`), `Replication` (lacing), and ports with `Level`, `UseLevels`, `KeepListStructure` and `UsingDefaultValue`.
  - `Connectors` hold Start port id, End port id and IsHidden.
  - `View` holds `Dynamo{Version, RunType, RunPeriod}`, `Camera`, `ConnectorPins`, `NodeViews[]` (X, Y, Name, IsSetAsInput/Output, ShowGeometry, Excluded) and `Annotations[]` (groups and notes with style).

### 1.11 MEP use and packages
- **Common MEP automations** suggested by MEP users:
  - Place fire dampers where ducts or pipes cross fire-rated walls in a linked model.
  - Tag pipes and ducts with their space or scope box.
  - Colour overrides for values Revit cannot filter on.
  - Place terminals, sprinklers and lights in repeated patterns from room data.
  - Sync space parameters across models.
  - Batch-create views and sheets ([RevitForum MEP thread](https://www.revitforum.org/forum/revit-all-flavors/dynamo-bim/23415-dynamo-for-mep)).
- **Packages** ([Primer packages](https://primer2.dynamobim.org/a_appendix/a-3_packages.md) unless noted):
  - **Clockwork:** Revit, list, string, unit and geometry utilities.
  - **archi-lab:** extended Revit interaction, including Analysis Visualization Framework nodes.
  - **Data-Shapes:** UI forms for Player workflows (`UI.MultipleInputForm++`) ([docs](https://datashapes.gitbook.io/primer/ui/1.-ui.multipleinputform-++)).
  - **Rhythm:** Revit and core utilities plus a view extension.
  - **MEPover:** creates pipes, ducts, cable trays and conduits from lines; reads fitting and connector data and MEP system info ([MEPover](https://sites.google.com/site/bimstallatie/dynamo/mepover-package)).
  - **Orchid:** a large set of Revit project, family and material data tools ([GitHub](https://github.com/erfajo/OrchidForDynamo)).
  - **Genius Loci:** linked files and documents.
  - **BimorphNodes:** clash detection and collectors for linked elements.

---

## 2. Gap table

Legend: **in 04?** Yes / Partial / No. **Rec:** Adopt (copy Dynamo), Adapt (same goal, different mechanism), Skip. **Pri:** P0 = needed for A2–A3 and the first acceptance test, P1 = first-release polish, P2 = later.

| Feature | in 04? | Rec | Pri | Rationale |
|---|---|---|---|---|
| Library left / canvas / Run bar layout | Yes | Adopt | P0 | Already decided; matches [UI](https://primer2.dynamobim.org/3_user_interface). |
| Library hierarchy Create/Action/Query, hover card with port types, dotted search | No | Adopt | P0 | Can be generated from the `openbim-commands` registry (rule 9). Search is the main way users find nodes. |
| Automatic / Manual run | Yes | Adopt | P0 | In 04. Automatic mode must never commit; it only refreshes the proposal (see §3). |
| Periodic run | No | Skip | P2 | Dynamo gates it on `DateTime.Now`. Little MEP value; file watching is the more useful trigger. |
| Run cancel / progress | No | Adopt+ | P0 | Dynamo hangs Revit ([UI Performance](https://github.com/DynamoDS/Dynamo/wiki/Dynamo-UI-Performance)). A separate process and parallel engine make cancel cheap. |
| Node states (error/warning/info, unconnected input, frozen, preview-off) with tooltips | No (only "graph is checked") | Adopt | P0 | Users rely on these for debugging ([Nodes and Wires](https://primer2.dynamobim.org/4_nodes_and_wires)). |
| Graph footer with state counters and jump-to; node manager | No | Adopt | P1 | Cheap; the [2.15](https://dynamobim.org/dynamo-core-2-15-release/) design is good. |
| Preview bubbles (hover, pin, saved in file) | Yes | Adopt | P0 | Save pin state; Dynamo 2.0 lost it. Show list shape and level labels (`@L1`…) in the bubble. |
| Watch node | Yes | Adopt | P0 | Show a list tree with indices and level labels, coloured by type. Values should show quantities with units. |
| Watch 3D / Watch Image | Partial (3D in main app) | Adapt | P1 | Main-app preview is P0. An in-canvas Watch 3D helps when no model view is visible. Watch Image is P2. |
| Background 3D preview toggle (Ctrl+B) | Partial | Adapt | P1 | Preview lives in the main app. Add per-node "preview on/off" (`ShowGeometry`). |
| Code Block | Yes | Adopt | P0 | The language is not specified in 04 (see §4). |
| DesignScript ranges, sequences, indexing, `def` | No | Adopt (subset) | P0 | `a..b..s`, `a..#n..s`, `a..b..#n`, `l[i]`, slices, `def`. Muscle memory for the team. |
| Lacing Auto/Shortest/Longest/Cross | Proposed | Adopt supported subset | P0 | Pinned lowering and exact output fixtures in 04b; unresolved rank interactions gate implementation. |
| Replication guides `<n>`, `<nL>` | Proposed | Adopt supported subset | P0 | Sorted guide loops per wiki and pinned code; mixed lacing/explicit-guide combinations remain gated (04b). |
| List@Level per port + Keep list structure | Proposed | Adopt supported subset | P0 | One dominant structure; pinned warnings/restoration conditions and unresolved combinations in 04b. |
| Dictionaries (string keys) and dictionary nodes | No | Adopt | P0 | CSV rows map naturally to dictionaries (acceptance test step 1). |
| List management node set (Count, Flatten, Transpose, Chop, GroupByKey, SortByKey, FilterByBoolMask, Map/Combine) | No | Adopt | P0 | Core vocabulary. |
| Python node, IN/OUT convention | Yes | Adopt | P0/A4 | Keep `IN[i]` / `OUT` for familiarity. |
| Python engine choice | Partial | Adapt | P1 | One CPython 3 engine, version pinned in the graph. Dynamo changed engines three times (2.7, 2.13, 4.0). |
| Python packages (numpy, pandas, openpyxl) | Proposed | Adapt | P1/A4 | Trusted local code, no filesystem/network sandbox; locked isolated environment plus separate installation consent (04b §5). |
| Python editor (syntax highlighting, completion) | No | Adopt | P1 | Use an egui code editor widget. |
| Python Revit API boilerplate (`RevitServices`, `TransactionManager`, Unwrap) | N/A | Adapt | P0/A4 | Give Python a typed `openbim` client (the same API as MCP) that returns proposals. No transactions in user code. |
| Selection input nodes (Select Model Element(s), Category, Type, Level, View pickers) | No | Adopt | P0 | Needed for the Player and for "match penetration". The picker runs in the main app. |
| `Element.SetParameterByName` | Partial (rule 8 labels with warning) | Adapt | P0 | Stable IDs first. Labels in EN/FR accepted with a warning. Never match on case-sensitive free text silently. |
| Units / ForgeTypeId conversion nodes | Yes (better) | Adapt | P0 | Quantities everywhere. Add `Quantity.Parse("6 m/s")`, `Convert`, and display-format nodes. |
| Element binding (trace) | **No** | Adapt | P0 | Must decide what re-running a creating graph does (see §3). |
| Transactions / undo | Yes (change set) | Adopt (04) | P0 | 04 is better than Dynamo. Add one undo entry per commit, labelled with the graph name. |
| Freeze node | No | Adopt | P1 | Still useful for slow calculations and Python. Saved in the file. |
| Graph-edit undo/redo (canvas) | No | Adopt | P0 | Separate from model undo. Dynamo has Ctrl+Z/Y on the canvas. |
| Groups (styles: Inputs/Actions/Outputs), notes, pin note to node, align, rename node | No | Adopt | P1 | Graph readability ([Graph Strategies](https://primer2.dynamobim.org/9_best_practices/1-graph-strategies.md)). Group styles also help AI layout. |
| Wire editing (Shift multi-move, Ctrl duplicate, hide, reroute pins) | No | Adopt | P1 | Expected ergonomics. |
| Node to Code | No | Adopt | P2 | Needs a Code Block AST printer; good for teaching. |
| Node Autocomplete | No | Adapt | P2 | Type-based suggestions from the registry. |
| Custom nodes `.dyf` (subgraph tabs) | No | Adopt | P1 | Reuse across the team; the Home + custom-node tab model. |
| Packages / package manager | Partial (library + version naming) | Adapt | P2 | Start with a shared team library folder plus a version lock in the graph (`NodeLibraryDependencies`). No public registry. |
| Player: folder list, thumbnail, description, status, inputs, outputs, Edit | Partial (Player named) | Adopt | P1/A5 | Is Input / Is Output flags in the schema; typed input widgets; refuse to run on unsupported inputs. |
| Player custom forms (Data-Shapes-style) | No | Adapt | P2 | A typed input schema should cover most needs. |
| Generative Design / optioneering | No | Skip (keep schema-ready) | P2 | Keep min/max/step on inputs and numeric outputs so a future study runner can use them. |
| Headless CLI (run graph, dump outputs) | Partial (MCP stdio, headless engine) | Adopt | P1 | `openbim-graph run file --in k=v --out json`. Needed for CI tests of graphs. |
| Graph-editing MCP tools (DynamoMCP-style) | Partial (MCP = model commands) | Adopt | P1 | Dynamo 4.2 already exposes canvas tools to AI ([DynamoMCP](https://primer2.dynamobim.org/12_ai_tools/1-what-is-dynamo-mcp)). Reuse proposal and review for graph and Python edits. |
| Profiling (execution order, time per node) | No | Adopt | P1 | TuneUp-equivalent. The parallel engine makes timing visible and cheap. |
| Trusted locations / security dialog | Proposed | Adapt | P0/A4 | Provenance is not code/install/commit approval. Model tokens do not sandbox Python; 04b §5 states the boundary without claiming stronger confinement. |
| Graph thumbnail, description, documentation URL | No | Adopt | P1 | Player and file browser. Store thumbnails as a sidecar PNG, not base64 in the JSON. |
| `.dyn`-like JSON: logic separate from view | Yes | Adopt | P0 | See §4 for required fields. |
| Export workspace as image | No | Adopt | P2 | Useful for documentation. |
| Multiple home graphs per window | N/A | Adapt | P2 | Dynamo allows one Home workspace per window. Tabs would be nicer but are not needed. |

---

## 3. Where Dynamo is weak and OpenBIM Graph should do better

1. **Element binding surprises (historical evidence).** The integration guide demonstrates opaque binding data, with legacy pre-3.0 SOAP rather than verified current encoding. The forum reports document scope/default control and Python/custom-node differences; these reports are not a uniform current-version guarantee ([Element Binding doc](https://github.com/DynamoDS/Dynamo/blob/master/doc/integration_docs/2_elementBinding.md); [Release Notes](https://github.com/DynamoDS/Dynamo/wiki/Release-Notes); [forum](https://forum.dynamobim.com/t/element-binding-in-revit/31612/1)).
   **Improve:**
   - Store graph-owned elements in the **project** op log, keyed by model/graph/node/output and stable external row/output key. Positional identity requires explicit reorder rejection, never silent row reassignment (04b §3).
   - Show them to the user. Give each creating node a mode: *update previous* (default) / *always create new* / *detach*.
   - In the proposal, show "will modify N, delete M, create K" before commit.
2. **Transactions and ordering.** Users cannot tell whether a collector sees elements created earlier in the same run. They reach for `Transaction.Start/End` nodes ([forum](https://forum.dynamobim.com/t/transactions-nodes-and-how-to-use-them/43759)), and Python needs `TransactionManager` boilerplate ([Python Primer](https://dynamopythonprimer.gitbook.io/dynamo-python-primer/4-revit-specific-topics/working-with-transactions)).
   **Improve:**
   - Reads see the fixed revision (04 rule 1). Writes are data: creating nodes return *pending handles* that downstream nodes can use.
   - A "query" never sees pending elements unless it is explicitly fed them.
   - Write this down and show "pending" in bubbles. There are no transaction nodes.
3. **Automatic mode and model churn.** Automatic evaluation can affect the host through writing nodes. **Improve (own policy):** preview/proposal only; mandatory per-run human confirmation in every client, including Player. No apply-without-review exception (04b §5).
4. **UI responsiveness.** The historical performance wiki describes host UI-thread execution ([UI Performance](https://github.com/DynamoDS/Dynamo/wiki/Dynamo-UI-Performance)); it is not proof that all current workloads hang or lack cancel. **Improve:** separate process, cancel/progress, diagnostic partial previews only; cancelled proposals cannot commit (04b §3).
5. **Determinism and spec clarity.**
   - Dynamo's own wiki said UI shortest lacing was "broken" relative to replication guides ([wiki](https://github.com/DynamoDS/Dynamo/wiki/Replication-and-Replication-Guide-Part-3)).
   - Auto is not simply Shortest: pinned NodeModel lowering and tested rank calls are recorded in 04b §2. The historical broken-Shortest statement above is not asserted to be a current bug.
   - **Improve:** a written replication spec, a conformance test corpus (ported from the wiki examples), and an "explain" popover on a node showing the resulting loop nest and output shape. Results stay in fixed order (04 rule 10).
6. **Units.**
   - Dynamo is unitless: values take the project's units, so "5" is 5 mm in one model and 5 ft in another ([Working with Revit](https://dynamobim.org/issues/working-with-revit/)).
   - Revit 2022's `ForgeTypeId` switch broke scripts ([archi-lab](https://archi-lab.net/handling-the-revit-2022-unit-changes/)).
   - **Improve:** 04 rule 7 already fixes this. Also: units in bubbles and Watch; number sliders with a unit; Code Block literals with units (`3.5 m`, `6 m/s`) **[proposal]**; dimension errors shown as node errors that name both dimensions.
7. **Python engine churn.** IronPython2 → CPython3 (2.7 / 2.13) → PythonNet3 (4.0), each needing migration ([2.13](https://dynamobim.org/dynamo-core-2-13-release-part-3-3/); [4.0](https://dynamobim.org/dynamo-core-4-0-release/)). Scripts are tied to the host .NET API.
   **Improve:**
   - Supported model access uses the versioned `openbim` client; this does not confine trusted Python's direct OS/filesystem/network access (04b §5).
   - The graph pins `python_abi` and API version.
   - Upgrades are explicit, with a migration report.
8. **Parameter-by-name fragility.** Exact case-sensitive names ([Editing](https://primer2.dynamobim.org/7_dynamo_for_revit/3-editing)). **Improve:** stable IDs (rule 8). Labels resolve to IDs at edit time and the ID is stored, so renaming a label does not break graphs.
9. **Player input gaps.** Unsupported input types are silently ignored and the run fails at runtime ([Requiring Input](https://help.autodesk.com/cloudhelp/2023/ENU/RevitDynamo/files/Dynamo_Player/RevitDynamo_Dynamo_Player_About_Requiring_Input_for_Dynamo_Graphs_html.html)). Teams install Data-Shapes to build forms. **Improve:** typed input schema (quantity with unit and range, enum, element filter, file), validation before run, and grouping or ordering of inputs from canvas groups.
10. **File diffability and size [reasoning].** `.dyn` embeds a base64 thumbnail, base64 bindings and floating-point layout, and the .NET `ConcreteType` names tie it to the host ([sample](https://github.com/DynamoDS/Dynamo/blob/8a211c5f306e5b54541fa57081aa1ffa32a65e90/doc/distrib/Samples/en-US/Basics/Basics_Basic01.dyn)). **Improve:** canonical key order, no binary blobs, layout in a separate section or file, and node types named `lib@version::name`. Graphs then diff in Git and in the operation log.
11. **Tessellation cost.** Every geometry node tessellates ([Large Data Sets](https://primer2.dynamobim.org/9_best_practices/5-efficiently-working-with-large-datasets.md)). **Improve:** lazy preview only for nodes with preview on and visible, with a hard cap on instance count and a "preview truncated" info state.
12. **Undo [unverified].** It was not verified how Revit's undo groups a Dynamo run. **Improve:** one undo step per committed change set, named "Graph: <name> run <n>", and a link from the undo entry back to the graph run log.

---

## 4. Proposed additions to 04

- **Language:** use familiar DesignScript-like syntax, not a compatibility claim:
  - literals with optional units;
  - lists `[..]` and dictionaries `{"k":v}`;
  - ranges `a..b..s`, `a..#n..s`, `a..b..#n`;
  - indexing and slices, `@Ln`, replication guides `<n>`/`<nL>`;
  - `def` functions, `if` expressions.
  - No imperative blocks in v1.
  - Promise "familiar syntax", not DesignScript compatibility.
- **Replication spec:** [04b](04b-replication-and-serialization.md) now defines pinned lowering, exact fixtures and explicit unresolved gates. No complete Dynamo compatibility or invented fallback semantics.
- **Dictionaries** as a first-class value type, with dictionary nodes. A CSV reader returns a list of dictionaries.
- **Node states** Error > Warning > Info > Preview, plus "pending (proposal)" and "stale (model changed since read)". Add a footer with counters and a node manager.
- **Run bar:** Automatic (preview only) / Manual, plus **Cancel** and progress. Periodic is skipped; consider a "re-run on file change" trigger.
- **Incremental engine:** conservative model identity + host/linked snapshot generations, captured resource content identities, definitions/library/API versions and restored read dependencies (04b §4). Withdraw the element-read-set-only optimization. Nonpure Python/network and dependents are uncached. Force-full-run equality tests precede finer dependency tracking.
- **Managed outputs (element binding replacement):**
  - Graph-owned elements use stable external row/output keys in project-only mappings; dependency groups and ownership/model ops commit/undo atomically (04b §3).
  - Per-node mode: update / create-new / detach.
  - The proposal shows modify, create and delete counts.
- **Pending handles:** run/proposal-scoped typed handles, explicit dependency groups and applied/flagged/requested/stale results (04b §3). Cancellation invalidates handles; partial previews are not committable. Queries read the snapshot only.
- **Selection inputs:** pick element(s) in the main app, category, type, level and view pickers, each usable as a Player input.
- **Graph file schema (v1)** must carry:
  - exact fields, own extension namespace, canonical policy and round-trip fixture in 04b §6 supersede this research summary;
  - stable graph/node/port/interface IDs and explicit interface bindings, connector endpoint identity, ordered per-call guides and levels/ranks;
  - creating-node management/reorder/shrink/detach/create-new settings; Python engine/ABI/client API and locked artifacts; explicit permission/trust requirements (not saved consent);
  - a separate `view{}` (positions, groups with styles, notes with pinned node, reroute pins, pinned bubbles, preview flags, run mode);
  - **no** bindings and **no** base64 thumbnail (use a sidecar PNG).
- **Canvas:**
  - undo/redo for graph edits;
  - groups with Inputs/Actions/Outputs styles;
  - notes pinned to nodes;
  - align and auto-layout;
  - rename node;
  - wire Shift/Ctrl editing, hide wires, reroute pins;
  - freeze; per-node preview toggle;
  - export image (P2).
- **Library:** generated from the command registry with Create/Action/Query buckets, hover cards (types, units, structure), dotted search, an Add-ons section for team libraries, and node help with an example graph.
- **Custom nodes (P1):** a subgraph file type (`.obgf`, analogous to `.dyf`) opened in tabs. It has its own lacing and default values and is versioned in the team library.
- **Python node (A4):**
  - `IN`/`OUT` convention; one pinned CPython 3 engine.
  - The `openbim` client API is the only model access, so there is no Revit-style boilerplate.
  - Quantities in/out; trusted local code warning with no filesystem/network confinement; restricted policy refuses arbitrary Python. Exact locked wheel artifacts, isolated environment and separate install consent; no graph-open side effects (04b §5).
  - Code editor with completion.
  - AI edits to Python appear as a diff the user accepts or rejects (Dynamo 4.3 has the same pattern).
- **Player (A5):**
  - Folder list with thumbnail, description, status and last-run result.
  - Typed inputs with validation before run; outputs from Watch / Is Output.
  - "Open in OpenBIM Graph"; refuses to run when inputs are unsupported.
  - Runs headless through the same engine and returns the same proposal UI.
- **MCP for graphs (P1):** source disclosure, graph editing, code editing and execution have distinct path-authorized capabilities; diff/trust/proposal approvals are revision-bound and invalidated by relevant edits. Model commit remains human-confirmed per run (04b §5).
- **CLI (P1):** proposed `openbim-graph run <file> --in name=value --json-out` for regression tests; depends on the A2 engine, not a claim it already exists.
- **First MEP node libraries (A5)**, in this order:
  1. MEP-system traversal (connectors, systems, fittings: the MEPover equivalents).
  2. Intersections of linked and host elements, for firestops and dampers.
  3. Space / zone tagging of MEP elements.
  4. Batch views and sheets.
  5. Calculation nodes (M4–M9), recording standard and edition.
- **Do not promise:** `.dyn` import, DesignScript imperative blocks, Periodic mode, the public package manager, or Generative Design. Keep the input/output schema ready for an optioneering runner later.
