# 04c — Dynamo engine port: feasibility study and plan

**Status:** companion to 04, 20 Oct 2026 (proposed). Research report; the design decisions it feeds are in 04 "Engine port from Dynamo".
**Pinned source snapshot:** `DynamoDS/Dynamo` commit `8a211c5f306e5b54541fa57081aa1ffa32a65e90`
("Merge master-localization into master (#17406)"), verified by `git rev-parse HEAD` in a sparse
shallow checkout. Reproduce the checkout with:

```bash
git clone --depth 1 --filter=blob:none --sparse https://github.com/DynamoDS/Dynamo.git <dir>
git -C <dir> sparse-checkout set src/Engine src/DynamoCore src/DynamoSandbox src/DynamoCLI src/Libraries test extern
git -C <dir> fetch --depth 1 origin 8a211c5f306e5b54541fa57081aa1ffa32a65e90
git -C <dir> checkout 8a211c5f306e5b54541fa57081aa1ffa32a65e90
```
The snapshot's own `ABOUT.txt` says "@DYNAMO v.4.4.0 © 2026 Autodesk" — that is the source's
self-description, **not** an assertion that this commit is a released 4.x build [unverified].
All file paths below are cited at that SHA. Every effort figure is marked **[estimate]**.
Claims not established from the cited source are marked **[unverified]**.

**Bottom line up front:** Dynamo's engine is *not* a drop-in Rust port. Roughly 43 k lines of
`ProtoCore` are a .NET VM (GC, `StackValue` tagged union, reflection-emit, FFI), and the graph
model is MVVM-coupled. What *is* worth taking is small and precise: the **language front-end
grammar/AST**, the **replication-guide + List@Level algorithm** (~1 k lines), the **node/port data
model**, and the **list-function semantics**. Recommendation: **Port-subset** the algorithm and
front-end *specification*, **Reference-only** for the VM/scheduler/serialization, **Skip**
`ProtoImperative` (the project's subset excludes imperative blocks) and `DynamoSandbox` (pulls WPF).
The project writes its own Rust evaluator, scheduler and serializer against its own execution
contract (04 rule 10 / 04b §4), which Dynamo's mechanisms do not satisfy anyway.

---

## 0. Method and evidence boundary

- LOC was measured over the sparse checkout with a Python counter (code / blank / comment for
  `.cs`, plus `wc -l`); `cloc` is not installed on this host. Numbers are **code lines** unless
  marked "wc-l" (all files, including `.resx`).
- The sparse checkout contains `src/{Engine,DynamoCore,DynamoSandbox,DynamoCLI,Libraries,
  DynamoUtilities,NodeServices,DynamoApplications}`, `test/`, `extern/`, `LICENSE.txt`, `ABOUT.txt`.
  `src/DynamoCoreWpf`, `src/Tools`, `src/DynamoRevit*`, `doc/` (except via `git show`) are **not**
  checked out; statements about them are marked **[unverified]**.
- Dynamo is Apache-2.0 (`LICENSE.txt`), but `ABOUT.txt` and `Directory.Packages.props` show that
  several components it links are **closed-source Autodesk code** (§2.4). That is the single
  largest license blocker and it is unrelated to the Apache grant.

---

## 1. Component inventory

Measured code lines (`.cs`), sparse checkout at `8a211c5`. Counting convention: each file is split on `\n`, so every file contributes one terminal empty element; "code" lines exclude blank lines and lines whose first non-space characters begin a comment. Exact totals differ by a few lines between counters — treat them as approximate.

| Component | .cs files | .cs lines | code | WPF-coupled files | Role |
|---|---:|---:|---:|---:|---|
| `src/Engine/ProtoCore` | 104 | 56 331 | 43 091 | **0 / 104** | DesignScript parser, AST, VM (`DSASM/`), dispatch (`Lang/`), replication (`Lang/Replication/`), FFI |
| `src/Engine/ProtoAssociative` | 7 | 7 949 | 6 031 | 0 / 7 | Associative-language code generator (`CodeGen.cs`, `CodeGen_SSA.cs`) |
| `src/Engine/ProtoImperative` | 7 | 3 025 | 2 229 | 0 / 7 | Imperative-block code generator + executive |
| `src/Engine/ProtoScript` | 8 | 4 023 | 2 804 | 0 / 8 | Runners (`Runners/LiveRunner.cs` is what `DynamoCore/LiveRunnerServices` uses) |
| `src/DynamoCore/Scheduler` | 13 | 2 372 | 1 448 | 0 / 13 | .NET `Task`-based async scheduler, modified-node closure |
| `src/DynamoCore/Graph` | 51 | 23 711 | 15 341 | 0 / 51 | Node/port/connector/workspace models, serialization converters |
| `src/DynamoCore/Models` | 7 | 8 931 | 5 743 | 0 / 7 | `DynamoModel`, `RunSettings`, recordable commands |
| `src/DynamoSandbox` | 5 | 443 | 288 | **3 / 5** | Headless host bootstrap (`Program.cs`, `DynamoCoreSetup.cs`) |
| `src/DynamoCLI` | 3 | 399 | 310 | 0 / 3 | CLI (`-o/-v/-g/-i`), `CommandLineRunner` |
| `src/Libraries/CoreNodeModels` | 39 | 9 539 | 7 219 | 0 / 39 | Node models (Watch, Range, Sequence, Formula, Selection, …) |
| *(ref)* `src/Libraries/CoreNodes` | 36 | 9 307 | 5 678 | 0 / 36 | Static `Function` implementations: List/Math/String/Compare/Sorting/Data/… |
| *(ref)* `src/DynamoUtilities` | 34 | 5 026 | 3 294 | 1 / 34 | Shared utilities |
| *(ref)* `src/NodeServices` | 17 | 4 393 | 1 969 | 0 / 17 | Node attributes (`IsVisibleInDynamoLibrary`, `AllowRankReduction`, …) |
| *(ref)* `src/Libraries/DesignScriptBuiltin` | 5 | 418 | 270 | 0 / 5 | `Builtin.Get`, `Builtin.Dictionary` |
| *(ref)* `src/Engine/GraphLayout` | 2 | 867 | 491 | 0 / 2 | Standalone auto-layout, no project references |
| *(ref)* `src/DynamoApplications` | 5 | 869 | 617 | 0 / 5 | Application bootstrap shared by CLI/sandbox |

WPF coupling was measured by grepping each `.cs` for WPF namespaces
(`System.Windows*`, `PresentationFramework/Core`). Result: **the engine core is WPF-free**.
`CoreNodeModelsWpf` (61 `.cs`, not counted above) is the separate WPF view layer — the engine
keeps logic in `CoreNodeModels` and views in `CoreNodeModelsWpf`. The real coupling in
`DynamoCore/Graph` and `Models` is **MVVM**, not WPF: `NodeModel`/`WorkspaceModel` implement
`INotifyPropertyChanged`, `IUndoRedoRecorderClient`, `ILogSource` and raise
`PropertyChanged` events (`NodeModel.cs`, `WorkspaceModel.cs:206`). That coupling must be
designed out of any Rust port.

### Key files per component

- **ProtoCore** — `Parser/Parser.cs` (4 091), `Parser/Scanner.cs` (946), `Parser/AssociativeAST.cs`
  (3 588), `Parser/AST.cs` (61), `Parser/DictionaryBuilder.cs`, `DSASM/Executive.cs` (5 170),
  `DSASM/Heap.cs`, `DSASM/Stack.cs`, `Lang/CallSite.cs` (1 942), `Lang/FunctionTable.cs`,
  `Lang/Replication/{Replicator,ArgumentAtLevel,ReplicationInstruction}.cs`, `CodeGen.cs` (2 403),
  `Core.cs` (931), `RuntimeCore.cs` (403), `FFI/*`. Grammar sources: `Parser/atg/{Associative.atg
  (2 119), Imperative.atg (1 269), Start.atg (740), End.atg (1)}`.
- **ProtoAssociative** — `CodeGen.cs` (6 138), `CodeGen_SSA.cs`, `Compiler.cs` (133).
- **ProtoImperative** — `CodeGen.cs`, `Compiler.cs` (35), `Executive.cs`.
- **ProtoScript** — `Runners/LiveRunner.cs` (2 088), `Runners/ProtoRunner.cs`, `Runners/DebugRunner.cs`.
- **Scheduler** — `DynamoScheduler.cs` (314), `UpdateGraphAsyncTask.cs` (293),
  `PreviewGraphAsyncTask.cs`, `AsyncTask.cs` (250), `SchedulerThread.cs`, `TimeStampGenerator.cs`.
- **Graph** — `Nodes/NodeModel.cs` (3 123), `Nodes/PortModel.cs` (652), `Nodes/CodeBlockNode.cs`,
  `Connectors/ConnectorModel.cs`, `Workspaces/WorkspaceModel.cs` (2 797),
  `Workspaces/HomeWorkspaceModel.cs`, `Workspaces/SerializationConverters.cs` (1 490),
  `Nodes/NodeInputData.cs`, `Nodes/NodeOutputData.cs`.
- **Models** — `DynamoModel.cs` (4 190), `RunSettings.cs` (124).
- **DynamoCLI** — `Program.cs` (128), `CommandLineRunner.cs`, `Properties/`.
- **CoreNodeModels** — `Watch.cs`, `Range.cs`, `Formula.cs`, `Selection.cs`, `WebRequest.cs`,
  `ColorRange.cs`, `CurveMapperNodeModel.cs`, `Equals.cs`, `DynamoConvert.cs`.

---

## 2. Dependencies

### 2.1 Internal dependency graph (ProjectReference, verified)

```
ProtoCore ──▶ DynamoUtilities, DesignScriptBuiltin, NodeServices(DynamoServices)
ProtoAssociative ──▶ ProtoCore
ProtoImperative   ──▶ ProtoCore
ProtoScript       ──▶ ProtoCore
GraphLayout       ──▶ (none)

DynamoCore ──▶ GraphLayout, ProtoAssociative, ProtoCore, ProtoImperative, ProtoScript,
               DynamoUtilities, DesignScriptBuiltin, VMDataBridge, NodeServices

DynamoSandbox ──▶ DynamoApplications, DynamoCoreWpf, DynamoCore, DynamoUtilities,
                  NodeServices, Tools/{DynamoInstallDetective,DynamoShapeManager}
DynamoCLI     ──▶ DynamoApplications, DynamoCore, DynamoUtilities, ProtoCore,
                  DesignScriptBuiltin, NodeServices

CoreNodeModels ──▶ DynamoCore, DynamoUtilities, ProtoCore, NodeServices, CoreNodes,
                   DynamoConversions, VMDataBridge
CoreNodes      ──▶ ProtoCore, DynamoUtilities, NodeServices, LibG (closed-source)
```

Two structural facts for the port:

1. **The VM core depends upward on the node layer.** `ProtoCore` references `NodeServices`
   (`DynamoServices`), whose `Attributes.cs` defines `IsVisibleInDynamoLibrary`,
   `AllowRankReduction`, `DefaultArgument`, `MultiReturn`, `CanUpdatePeriodically`
   (`src/NodeServices/Attributes.cs`). `ProtoCore/FFI/*` uses these to scan imported .NET
   types. A Rust port replaces these attributes with the project's own `lib@version::name` registry and
   must not carry the dependency.
2. **Geometry is the only closed-source leak into the node library.** `CoreNodes` references
   `DynamoVisualProgramming.LibG_233_0_0`; only `CoreNodes/Data.cs` uses `LibG`/`Autodesk.Geometry`
   — `List.cs`, `Math.cs`, `String.cs`, `Compare.cs`, `Sorting.cs`, `Object.cs` are pure
   (verified by grep). So the core list/math/string nodes are portable without LibG.

### 2.2 NuGet dependencies per project (verified from `*.csproj`)

| Project | PackageReference |
|---|---|
| `ProtoCore` | `System.CodeDom`, `System.Reflection.Emit` |
| `ProtoAssociative`, `ProtoImperative`, `ProtoScript` | *(none)* |
| `DynamoCore` | `Autodesk.IDSDK`, `Greg`, `DynamoVisualProgramming.LibG_*`, `Newtonsoft.Json`, `RestSharp`, `Lucene.Net*`, `DynamoVisualProgramming.PythonEngine.PythonNet3`, `DynamoVisualProgramming.Analytics`, plus MS packages |
| `CoreNodeModels` | `Newtonsoft.Json` |
| `CoreNodes` | `DynamoVisualProgramming.LibG_233_0_0` |
| `DynamoUtilities` | `Newtonsoft.Json`, MS packages |

### 2.3 Third-party libraries and whether they block a Rust port

| Library | Used by | License (per `ABOUT.txt` / package metadata) | Blocker? |
|---|---|---|---|
| `System.CodeDom`, `System.Reflection.Emit` | ProtoCore | MS .NET (MIT/Apache-2.0) | No — but they are exactly the .NET-runtime features not wanted in Rust |
| `Newtonsoft.Json` | DynamoCore, CoreNodeModels, DynamoUtilities | MIT | No — replaced by `serde_json` |
| `RestSharp` | DynamoCore | Apache-2.0 | No — HTTP client; not needed for the engine |
| `Lucene.Net*` | DynamoCore | Apache-2.0 | No — library search index; not engine logic |
| `Greg` | DynamoCore | MIT | No — string utilities |
| `LibG_*` | DynamoCore, CoreNodes | **closed-source Autodesk** (`doc/distrib/Autodesk.rtf`) | **Yes if geometry is ported**; avoid by not porting geometry |
| `Autodesk.IDSDK` | DynamoCore | **closed-source** | Yes — auth/ID; not needed |
| `DynamoVisualProgramming.Analytics` | DynamoCore, ProtoCore/FFI | **closed-source** | Yes — telemetry; drop it |
| `PythonNet3` | DynamoCore | **licence not established by the pinned tree [unverified]** — the `ABOUT.txt` closed-source list does not include it | Yes — out of scope because the architecture runs Python in a separate worker (A4); do not infer the licence from a NuGet dependency |
| `ForgeUnits.NET` / `ForgeUnits.Schemas` | `Libraries/DynamoUnits` (not fetched) | **closed-source** | Yes — units; the project uses its own quantity model (04 rule 7) |
| `ProtoGeometry` v2.7.0 | geometry nodes | **closed-source** | Yes — not ported |
| `AvalonEdit` | installer/UI | MIT | No |
| `HelixToolkit`, `SharpDX` | 3D preview | MIT | No — not ported (3D lives in the main app) |
| `ICSharpCode.AvalonEdit` source zip | `extern/Installer/3rdParty/Source` | MIT (zip present) | No — UI only |
| LGPL-2.1 / LGPL-3.0 texts | `extern/Installer/3rdParty/Licenses/` | LGPL | **Installer-only**; nothing in the engine path links LGPL |

**Copyleft verdict:** the only LGPL artefacts are installer-bundled and are not on the engine
path that would be ported. No GPL code appears on the engine path. The real license risk is the
**closed-source Autodesk packages**, not copyleft.

### 2.4 Closed-source Autodesk components (must not be ported)

`ABOUT.txt` states verbatim: *"LIBG, ProtoGeometry v.2.7.0, DynamoVisualProgramming.Analytics,
CER, ADP, GRegRevitAuth, AGET, IDSDK, IDSDK Wrapper, ForgeUnits.NET, ForgeUnits.Schemas, and
Autodesk.GeometryPrimitives.Dynamo are closed source files licensed by Autodesk under the
license that can be found here `doc/distrib/Autodesk.rtf`."* `doc/distrib/Autodesk.rtf` exists in
the tree (118 277 bytes). Porting any of these is **out of scope**; the port must be built only
from Apache-2.0-licensed engine code plus the project's own re-implementations.

---

## 3. Language-subset mapping (04b Code Block subset → ProtoCore components)

The project's subset (04 line 81, 04b §2): literals with units, lists, dictionaries, ranges, indexing and
slices, `@Ln`, replication guides, `def` functions, `if` expressions; lacing Auto/Shortest/
Longest/Cross; List@Level + Keep list structure. **No imperative blocks.**

| Feature | Implementing component (file @ `8a211c5` : class/construct) | Notes |
|---|---|---|
| Numeric/string/bool/null literals | `src/Engine/ProtoCore/Parser/AssociativeAST.cs` : `IntNode`, `DoubleNode`, `StringNode`, `BooleanNode`, `CharNode`, `NullNode` | Coco/R grammar emits these (`Parser/atg/Associative.atg`) |
| **Literals with units** (`3.5 m`, `6 m/s`) | **Not in ProtoCore.** Units are a host concern: `src/NodeServices/DynamoUnitsDisplay.cs` and `src/Libraries/DynamoUnits` (ForgeUnits, closed-source) | The project implements its own quantity type (04 rule 7); do **not** port DynamoUnits |
| Lists `[..]` | `AssociativeAST.cs` : `ArrayNode`; runtime `src/Engine/ProtoCore/DSASM/DSArray.cs`; node helpers `src/Libraries/CoreNodes/List.cs` | |
| Dictionaries `{"k": v}` | `src/Engine/ProtoCore/Parser/DictionaryBuilder.cs` lowers the literal to a call to `DesignScript.Builtin.Dictionary.ByKeysValues`; `src/Libraries/DesignScriptBuiltin/Dictionary.cs` : `Dictionary` | Verified: *"Dictionary syntax … isn't part of the AST"* |
| Ranges `a..b..s`, `a..#n..s`, `a..b..#n` | `AssociativeAST.cs` : `RangeExprNode` (line 2213); grammar rule `Associative.atg:1226–1239` with `Associative_rangeAmountOperator` / `Associative_rangeStepOperator`; lowered to `%generate_range` (`DSASM/Defs.cs:383`, `Lang/BuiltInMethods.cs:120`) | The `#` count form is a parser-level transform into `RangeExprNode`; exact encoding **[unverified detail]** |
| Indexing `l[i]` and slices | `AssociativeAST.cs` : `ArrayNode` with index; `src/Engine/ProtoCore/DSASM/Executive.cs` : `GetIndexedArray` (line 2574), `GetIndexedArrayW` (2603) | |
| `@Ln` (List@Level) | `AssociativeAST.cs` : `AtLevelNode` (line 309); `src/Engine/ProtoCore/Lang/Replication/ArgumentAtLevel.cs` : `AtLevelHandler` | |
| Replication guides `<n>` / `<nL>` | `AssociativeAST.cs` : `ReplicationGuideNode`; `Lang/Replication/Replicator.cs` : `ReplicationGuide`, `Replicator.BuildPartialReplicationInstructions` | |
| Lacing Auto/Shortest/Longest/Cross | `src/DynamoCore/Graph/Nodes/NodeModel.cs` : `NodeModel.UseLevelAndReplicationGuide` (lines 1641–1690) → `AstFactory.AddReplicationGuide` | Auto appends `<1>` only to ports with levels; Shortest `<1>` to all; Longest `<1L>` to all; CrossProduct `<1>,<2>,…` in port order |
| `def` functions | `AssociativeAST.cs` : `FunctionDefinitionNode`; dispatch `Lang/FunctionTable.cs`, `Lang/FunctionEndPoint.cs` | |
| `if` expressions | `AssociativeAST.cs` : `InlineConditionalNode` (line 1940); `DSASM/Defs.cs:385` `%inlineconditional` | Grammar **rejects** `if`/`for`/`while` statements (`Associative.atg:87–89`, `243–244`) — confirms the associative subset excludes imperative blocks |
| Keep list structure | `ArgumentAtLevel.cs` : `ArgumentAtLevel.IsDominant`, `DominantListStructure`, `AtLevelHandler.RestoreDominantStructure` | One dominant structure only; >1 logs `MoreThanOneDominantList` |
| **Imperative blocks** | `src/Engine/ProtoImperative/*`, `Parser/atg/Imperative.atg`, `ImperativeAST.cs` | **Explicitly excluded by the project's subset (04 line 81). Do not port.** |

**Confirmation that the project's subset avoids ProtoImperative:** the associative grammar actively errors
on `if`/`for`/`while` statements (`Associative.atg:87–89`, `243–244`), and imperative constructs
live in a separate grammar (`Imperative.atg`) and codegen (`ProtoImperative/CodeGen.cs`). The v1
subset is therefore expressible entirely in the associative front-end; the port therefore covers the
associative path and skips `ProtoImperative`.

---

## 4. Execution model

### 4.1 How Dynamo schedules and executes

- **Graph → AST.** `DynamoCore/Engine/CodeGeneration/AstBuilder.cs` turns each node into
  `AssociativeNode`s; `NodeModel.UseLevelAndReplicationGuide` attaches `@Ln` and lacing guides
  to the AST before compile.
- **Dirty propagation.** `UpdateGraphAsyncTask.ComputeModifiedNodes`
  (`Scheduler/UpdateGraphAsyncTask.cs:256`) selects nodes where `IsModified && !IsFrozen &&
  !IsTransient`, then `GetDownstreamNodes` (line 275) walks `node.OutputNodes` recursively to
  build the run set. `EngineController.ComputeSyncData` (`Engine/EngineController.cs:282`)
  produces a `GraphSyncData`; `UpdateGraphImmediate` (line 513) applies it. `Subtree` in
  `ProtoScript/Runners/LiveRunner.cs` carries `ModifiedAstNodes` and a `DeltaComputation` flag —
  this is Dynamo's "delta compute" (unchanged nodes keep cached values).
- **Replication-guide algorithm location.** `Lang/Replication/Replicator.cs`:
  `BuildPartialReplicationInstructions` (line 40) walks guide levels left-to-right, groups
  arguments by guide number, sorts guide numbers ascending, emits a **zip** instruction when a
  guide number is shared and a **Cartesian** instruction when unique, and upgrades the zip to
  `Longest` if any matching guide is `IsLongest` (lines 79–88). `ReductionToInstructions`
  (line 129) and `BuildReplicationCombinations` (line 426) build residual rank replication.
- **Execution pipeline.** `Executive.Callr` (`DSASM/Executive.cs:799–800`) calls
  `AtLevelHandler.GetArgumentAtLevelStructure` then `CallSite.JILDispatch`;
  `CallSite.JILDispatch` (`Lang/CallSite.cs:1254`) runs `PerformRepGuideDemotion` (line 1346) →
  `Replicator.BuildPartialReplicationInstructions` (1352) → `ComputeFeps` (1358) → `Execute`
  (1376) → `AtLevelHandler.RestoreDominantStructure` (1379). `Executive.Call` (line 926) is the
  second entry point with the same shape.
- **Threading.** `DynamoScheduler` (`Scheduler/DynamoScheduler.cs`) is a `Task`-based queue with
  `ProcessMode` and task coalescing (`UpdateGraphAsyncTask.CanReplace`, line 180). In-process
  hosts run on the UI thread; the README notes the engine itself can build for other platforms.

### 4.2 What a Rust port must preserve for **the project's** contract (04 rule 10, refined by 04b §4)

04 rule 10 requires: independent nodes run in parallel with **ordered results**; conservative
cache keyed on model identity + snapshot generations + definitions/inputs/library/API versions +
captured resource identities; nonpure Python/network nodes uncached; force-full-run exists.
04b §4 withdrew the element-read-set-only cache in favour of generation-scoped invalidation.

A Rust port must therefore **not** copy Dynamo's mechanisms; it must implement:

1. **Fixed read revision** — one immutable model snapshot per run; no node observes another
   node's writes. Dynamo's `ElementResolver`/trace binding is the opposite of this.
2. **Parallel independent nodes with fixed output order** — a dataflow executor (e.g. `rayon`)
   that evaluates ready nodes concurrently but materialises results in a deterministic,
   schema-defined order (04b §6.2: unordered ID-bearing arrays sort by `id`; list values keep
   order).
3. **Cache keyed on read set + snapshot generation** — the project's own key derivation; not
   `GraphSyncData`/`Subtree` delta flags.
4. **Proposed change set, no transactions** — nodes return pending handles; commits are
   dependency-atomic groups (04b §3). Dynamo's transaction envelope is a host (Revit) concept
   and is absent from `ProtoCore` itself.
5. **Replication profile** — the *semantics* of §3 above, re-expressed as a pure function over
   the project's value model, with the exact fixtures in `fixtures/replication-v1.json` as the oracle.

### 4.3 Mechanisms deliberately not ported, and why it is safe

| Dynamo mechanism | Why it is dropped | Why it is safe |
|---|---|---|
| **Element binding / trace** (`SerializationConverters.cs:663–683` reads `Bindings`; `ElementResolver`; `NodeModel` binding) | 04 §3 replaces it with managed outputs keyed by stable external row/output keys; 04 promises no `.dyn` compatibility | Reads see the fixed revision; writes are data (pending handles). No host trace is needed, and `.dyn` bindings are host-specific (Revit) anyway |
| **Transactions** (`TransactionManager` in Revit; `Transaction.Start/End` nodes) | 04 rule 5/6 define per-element commit and stale checks; 04b §3 defines dependency-atomic groups | `ProtoCore` is already a pure evaluator with no transaction concept; the project's commit layer is the app's op log, not the engine's |
| **`GraphSyncData` / `Subtree` delta compute** | The project's cache policy is generation-scoped and conservative (04b §4), not dirty-flag based | A full re-evaluation of the supported subset is cheap; the fixture corpus is small. Delta compute is an optimisation, not a semantic requirement |
| **.NET GC, `StackValue`, reflection-emit, FFI** | Rust ownership + a plain value enum; no dynamic codegen | The node set is a closed registry of pure functions; no arbitrary .NET type import |
| **WPF/MVVM node views, `INotifyPropertyChanged`** | egui is immediate-mode; the project's schema is data-only | Presentation is regenerated from the graph JSON, so no observable model is needed |
| **Closed-source packages** (LibG, ProtoGeometry, ForgeUnits, Analytics, IDSDK) | License (§2.4) | None are needed for the supported subset: geometry lives in the main app, units are the project's own quantity type, telemetry is dropped, Python is a separate worker. `PythonNet3` is excluded for the same architectural reason, but its licence is not established by the pinned tree [unverified]. |

---

## 5. Node model and serialization

### 5.1 What maps onto the project's schema v1 (04b §6)

| Dynamo (`.dyn`) | OpenBIM schema v1 | Keep / drop |
|---|---|---|
| `Uuid`, stable node `Id`, port `Id` | stable `id` namespaces (graph/node/port/connector/interface) | **Keep** (own IDs, own namespaces) |
| `Nodes[]` with `ConcreteType`, `NodeType` | `nodes[]` with `type: lib@version::name` | **Adapt** — the port drops `ConcreteType` (.NET type) |
| `Replication` (`Auto/Shortest/Longest/CrossProduct/Disabled`) | `lacing` (`Auto|Shortest|Longest|CrossProduct|Disabled`) | **Keep** |
| Port `Level`, `UseLevels`, `KeepListStructure`, `UsingDefaultValue`, `DefaultValue` | `ports[]` `level`, `use_levels`, `keep_structure`, `using_default`, typed `default` | **Keep** |
| `FunctionSignature` (e.g. `+@var[]..[],var[]..[]`) | `ports[]` `type`, `rank`, `nullable` | **Adapt** — rank/nullability are declared per port |
| Explicit guides (Code Block `xs<1>`) | `call.arguments[].guides[{value,longest}]`, `guide_source` | **Keep** |
| `Connectors[]` (`Start`/`End` port ids, `IsHidden`) | `connectors[]` with `start`/`end` `{node_id,port_id}` | **Keep**; `IsHidden` → `view` |
| `Inputs[]` / `Outputs[]` (Player/GD interface) | `inputs[]` / `outputs[]` with explicit `binding` | **Keep** |
| `View{}` (`Dynamo{RunType,RunPeriod}`, `Camera`, `ConnectorPins`, `NodeViews[]`, `Annotations[]`) | `view` (positions, groups, notes, reroutes, preview flags, run mode) | **Keep** (presentation only); **drop** `RunPeriod` (no Periodic mode) |
| `NodeLibraryDependencies[]` | `dependencies[]` `{id,version,artifact_sha256}` | **Keep** (own digests) |
| `Thumbnail` (base64 JPEG) | sidecar PNG | **Drop** the embedded blob (04 line 67) |
| `Bindings[]` (element trace) | *(none)* — replaced by `extensions.openbim.managed_output` | **Drop** |
| `ElementResolver` | *(none)* | **Drop** |
| `ExtensionWorkspaceData[]`, `Linting`, `Author` | `extensions.openbim` / the project's own | **Adapt** |
| `Dependencies[]` (internal node deps) | `dependencies[]` (library deps only) | **Adapt** |

### 5.2 Serialization implementation facts

- Reader: `SerializationConverters.cs` — `WorkspaceReadConverter` (line 453), `NodeReadConverter`
  (line 38), `ConnectorConverter` (line 1242), `NodeLibraryDependencyConverter` (line 1064),
  `DescriptionConverter` (line 425), `LinterManagerConverter` (line 998). Port/level/keep settings
  are restored here (e.g. `inPorts[portIndex].KeepListStructure`, `NodeModel.cs:2475`).
- Writer: `WorkspaceWriteConverter` (line 797) writes `ElementResolver`, inputs, outputs, nodes,
  connectors, `referencesList`, `EnableLegacyPolyCurveBehavior`, `ExtensionData`, `linterManager`
  (lines 849–944). `Thumbnail` is a property of `HomeWorkspaceModel` (line 183) and is read back
  at `SerializationConverters.cs:714–716`.
- Entry points: `WorkspaceModel.Save` (line 1599) and `WorkspaceModel.FromJson` (line 2362).

**Port decision:** reference-only. The project writes its own canonical JSON (04b §6.2) with `serde`; the project does not
transliterate Newtonsoft converters, because the project's schema is deliberately different
(no `ConcreteType`, no bindings, no thumbnail, canonical key order, quantity encoding).

---

## 6. Test corpus

| Suite | Size | Notes |
|---|---|---|
| `test/Engine/ProtoTest` | 109 `.cs`, 129 477 lines, **5 274 `[Test]`** | Language/VM conformance: `Associative/`, `Imperative/`, `AtLevel/`, `DSASM/`, `ParserTest/`, `FFITests/`, `GraphCompiler/`, `RegressionTests/`, `TD/` |
| `test/Engine/ProtoTest/TD/Associative/Replication.cs` | 5 081 lines, 483 test methods | The replication/lacing oracle — directly re-expressible as Rust fixtures |
| `test/Engine/ProtoTest/AtLevel/AtLevelTest.cs` | 492 lines, 56 tests | List@Level + dominant structure |
| `test/Engine/ProtoTest/Associative/ReplicatorTest.cs` | 54 lines | `ComputeReducedParams` reduction-unit tests |
| `test/DynamoCoreTests` | 164 `.cs`, 47 348 lines, **1 730 `[Test]`** | Graph/engine integration; `Engine/` subset is only 536 lines |
| `test/DynamoCoreTests/ListAtLevelTests.cs` | 84 lines | UI-integration graphs (`testdomlacing1.dyn`, `testdomlacing2.dyn`) |
| `test/Libraries` | 87 `.cs`, 20 265 lines | Node-library tests |
| Test **data** | **1 493 `.dyn`, 108 `.dyf`, 65 `.ds`, 13 `.csv`, 41 `.xlsx`, 58 `.json`, 39 `.png`** | `test/core` alone is 645 237 lines of data |

**Re-expressibility as Rust conformance fixtures:** yes, for the *cases*, not the code. The
`replication-v1.json` corpus (9 cases + 6 gated cases) is already the right shape: inputs,
lacing, optional levels/guides, exact expected output, and an evidence citation. The pinned
`Replication.cs` (483 methods) and `AtLevelTest.cs` (56) are a much larger source of
input→output triples that can be transcribed into the same JSON shape and asserted against a
Rust evaluator. The `.ds` files are standalone DesignScript programs usable as parser/eval
fixtures. **Test data license:** the repository is Apache-2.0, so the test corpus is Apache-2.0
*except* third-party package fixtures under `test/core/packageDependencyTests/`
(`ClockworkPackage/extra/LICENSE`, `PythonEnginePackage.zip`, `ZTTestPackage`) — those carry
their own licences and must be excluded or re-licensed before reuse **[unverified: individual
package licences not audited]**.

---

## 7. Port plan (mapped to milestones A0–A5 / AUTO-000..009)

All effort figures are **[estimate]** and derived from the LOC measurements in §1. They assume a
Rust implementation by an engineer who knows both codebases, and are given as ranges.

### Phase 0 — Spike (pre-A0, ~2–4 engineer-weeks)
Smallest end-to-end slice: **parse + eval one Code Block expression with lacing, with tests.**
- A Rust crate `obg-ds` with a hand-written lexer/parser for the associative grammar subset
  (literals, lists, binary operators, range, function call, `def`, `if` expression), an AST, and
  an evaluator over the project's value model.
- Port `Replicator.BuildPartialReplicationInstructions` (510 lines) and the zip/Cartesian executor
  as a pure function.
- Tests: all nine non-gated `replication-v1.json` cases (`auto-flat`, `shortest-flat`,
  `longest-flat`, `cross-flat`, `guides-zip`, `guides-cross-reversed`, `auto-scalar`) plus the 2
  engine-level cases (`level-flat`, `level-keep`).
- Exit: `cargo test` green; outputs byte-equal to the fixture values.
- LOC reasoning: reference ~1 000 lines of algorithm; Rust equivalent ~1.5–2 k lines plus
  parser ~2–3 k lines → **~4–5 k LOC [estimate]**.

### Phase 1 — A0/A1: registry + typed API (AUTO-001, AUTO-003)
- `openbim-commands` registry with typed quantity I/O; the graph node set is generated from it.
- Engine work is limited to the data model (nodes/ports/connectors) as plain Rust structs — no
  Dynamo code ported.
- **~3–5 k LOC [estimate]** (model + serde + registry).

### Phase 2 — A2: headless engine + schema v1 (AUTO-004)
- `openbim-graph` crate: parser/eval (Phase 0), replication profile, scheduler, cache,
  canonical serializer.
- Port-subset reference: `NodeModel.UseLevelAndReplicationGuide` semantics, `Replicator`,
  `ArgumentAtLevel`; list-function semantics from `CoreNodes/{List,Math,String,Compare,Sorting}.cs`.
- **~8–12 k LOC [estimate]** for the engine; **~3–4 k LOC [estimate]** for the schema/serializer.
- Risks: C#→Rust semantic gaps (see §7.1); unresolved replication combinations must remain gated
  (04b §2.2) — do not invent fallback semantics.

### Phase 3 — A3: OpenBIM Graph app (AUTO-005 canvas subtask)
- egui canvas, library, Watch, preview. **No Dynamo UI code ported** (CoreNodeModelsWpf is WPF).
- **~10–15 k LOC [estimate]** [unverified: UI effort not measured here].

### Phase 4 — A4: Python workers + trust (AUTO-006)
- Separate CPython worker process; `openbim` client; trusted-local-code boundary; locked
  artifacts. **Do not port `PythonNet3`** (out of scope: the architecture runs Python in a separate worker; licence not established by the pinned tree [unverified]). **~5–8 k LOC [estimate]**.

### Phase 5 — A5: libraries + Player (AUTO-005 Player, AUTO-007)
- Discipline node libraries calling M4–M9; Player. **~5–10 k LOC [estimate]**.

### 7.1 C# → Rust semantic gaps and risks

| Gap | Impact | Mitigation |
|---|---|---|
| **GC / `StackValue` tagged union** | Dynamo relies on a tracing GC and a universal value type; Rust has neither | Design a closed `Value` enum (scalar/list/dict/function) with `Rc`/`Arc`; avoid reference cycles by construction |
| **Null semantics** | `StackValue.Null` is pervasive; Rust has no null | Model null explicitly as `Value::Null` and enforce nullability in port signatures (04b §2.2) |
| **Mutability** | Dynamo mutates arrays in place (`DSArray.SetValueForIndices`) | Use persistent/structural-sharing lists or copy-on-write; keep `RestoreDominantStructure` semantics |
| **Threading** | `DynamoScheduler` is `Task`/`SynchronizationContext`-based, UI-thread affine | Use `rayon`/async with a deterministic reduction order; never depend on scheduler timing |
| **Reflection-emit / FFI** | `ProtoCore` emits code and imports arbitrary .NET types | Closed registry of pure Rust nodes; no dynamic import |
| **Version drift** | `8a211c5` is a moving master snapshot; `ABOUT.txt` self-describes 4.4.0 | Pin the SHA in the report and in any ported file header; re-verify on bump |
| **Scope creep** | The full VM is 43 k code lines and would pull in geometry/units/Python | Freeze scope to the §3 subset; gate everything else behind explicit review |

---

## 8. License checklist (Apache-2.0 compliance for porting)

Apache-2.0 §4 requires, for redistributed/derivative works: retain the licence and copyright
notices, state changes, and include a NOTICE file if one exists. Concretely:

1. **Include `LICENSE.txt`** (the Apache-2.0 text, present at the pinned SHA) in the repository
   root or the ported-code directory, **and deliver the Apache-2.0 licence text with every
   redistributed source or binary artifact** — an `about`/licence screen or a `LICENSE` file
   inside the distribution, not only in the source tree.
2. **NOTICE file:** the pinned tree has **no `NOTICE.txt`** (verified: absent at root and
   throughout the tree), so Apache-2.0 §4(c)'s obligation to retain applicable copyright, patent,
   trademark and attribution notices applies to the notices actually carried. The project's
   convention is to copy **`ABOUT.txt`** (939 lines: third-party credits and the closed-source
   statement) verbatim as `THIRD_PARTY_NOTICES` and keep it unmodified; any notice carried must
   be retained in redistributed artifacts too. `ABOUT.txt` is a project attribution convention,
   not a substitute for a legally required NOTICE.
3. **Per-file headers for ported code.** For every file copied or transliterated from Dynamo,
   add a header stating: the source path and SHA
   (`DynamoDS/Dynamo@8a211c5f306e5b54541fa57081aa1ffa32a65e90`), the original copyright
   ("Copyright 2026 Autodesk, Inc." / "Copyright 2017 Ian Keough" as applicable), the Apache-2.0
   licence reference, and a **modification statement** ("Ported to Rust; modified from the
   original"). Do not claim the file is unmodified, and keep the statement **in the file itself**.
4. **Audit every `extern/` and NuGet dependency actually ported or vendored.** The ones on the
   path are MIT (`Newtonsoft.Json` — not ported, replaced by `serde_json`), Apache-2.0
   (`RestSharp`, `Lucene.Net` — not ported), and **closed-source** (`LibG`, `ProtoGeometry`,
   `ForgeUnits`, `Analytics`, `IDSDK`) which must **not** be ported, plus `PythonNet3` (out of
   scope; licence not established by the pinned tree [unverified]). If any third-party *source*
   is vendored, keep its licence text alongside it.
5. **State modifications.** Apache-2.0 §4(b) requires marking changed files. Keep a
   `PORTING.md` index of each ported file, its source SHA and the nature of the change —
   **in addition to**, not instead of, the per-file modification statement required by item 3.
6. **Do NOT:** claim `.dyn` or DesignScript compatibility; use the "Dynamo" name, logo or
   trademarks to endorse or promote the product (Apache-2.0 §6 / `ABOUT.txt` trademark notice);
   redistribute the closed-source Autodesk packages; or imply the port is an official Dynamo
   build.
7. **Test data:** if `test/` fixtures are reused, keep Apache-2.0 attribution and **exclude**
   `test/core/packageDependencyTests/*` third-party package fixtures unless their licences are
   audited and cleared.
8. **Patent grant:** Apache-2.0 §3 grants a patent licence from each contributor covering patent
   claims that contributor could license and that are necessarily infringed by its contribution
   alone or by its combination with the Work to which it was submitted. The licence terminates
   for that Work if the licensee institutes patent litigation alleging that the Work or an
   incorporated Contribution constitutes direct or contributory patent infringement (including
   cross-claims and counterclaims), effective as of the filing. Absence of a blanket patent
   clearance for material written here is not a claim that none of it can benefit from licensed
   claims. No proprietary-source publication is required by Apache-2.0 §4.
---

## 9. Go / No-Go per component

| Component | Verdict | One-line reason |
|---|---|---|
| `src/Engine/ProtoCore` | **Port-subset** | Take the grammar/AST and the replication algorithm as a specification and re-implement; skip the 43 k-line .NET VM, GC, FFI and reflection-emit |
| `src/Engine/ProtoAssociative` | **Reference-only** | Its AST node set is the spec for the ported parser; `CodeGen`/`CodeGen_SSA` target a bytecode not used |
| `src/Engine/ProtoImperative` | **Skip** | The project's subset explicitly excludes imperative blocks (04 line 81); the associative grammar rejects `if`/`for`/`while` statements |
| `src/Engine/ProtoScript` | **Reference-only** | `LiveRunner`'s `Subtree`/`ModifiedAstNodes`/`DeltaComputation` is the idea behind the project's cache; the runner is .NET-coupled |
| `src/DynamoCore/Scheduler` | **Reference-only** | `Task`/`SynchronizationContext` scheduler is not portable; only the modified-node downstream-closure algorithm is worth transcribing |
| `src/DynamoCore/Graph` (+ `Models`) | **Port-subset** | Node/port/connector/workspace data model and lacing/level semantics are the project's schema v1 spec; the MVVM coupling is designed out |
| `src/DynamoSandbox` | **Skip** | Headless host bootstrap that references `DynamoCoreWpf`; not usable as a headless model |
| `src/DynamoCLI` | **Reference-only** | `-o/-v/-g/-i` semantics are the spec for `openbim-graph run`; the implementation depends on `DynamoApplications`/WPF |
| `src/Libraries/CoreNodeModels` | **Port-subset** | Node-model definitions and `CoreNodes` list/math/string function semantics are the project's node-library spec; `CoreNodeModelsWpf` is skipped |
| `src/Libraries/CoreNodes` | **Port-subset** | Pure list/math/string/sorting implementations are portable; `Data.cs` (LibG) is skipped |
| `src/Libraries/DynamoUnits` | **Skip** | Closed-source ForgeUnits; the project uses its own quantity model (04 rule 7) |
| `src/NodeServices` | **Reference-only** | Defines the node attributes replaced by the project's own `lib@version::name` registry |

---

## Appendix — file-path index at `8a211c5`

- `src/Engine/ProtoCore/Lang/Replication/Replicator.cs` — `Replicator`, `ReplicationGuide`
- `src/Engine/ProtoCore/Lang/Replication/ArgumentAtLevel.cs` — `AtLevelHandler`, `ArgumentAtLevel`, `DominantListStructure`
- `src/Engine/ProtoCore/Lang/Replication/ReplicationInstruction.cs` — `ReplicationInstruction`
- `src/Engine/ProtoCore/Lang/CallSite.cs` — `CallSite.JILDispatch` (1254), `PerformRepGuideDemotion` (1805)
- `src/Engine/ProtoCore/DSASM/Executive.cs` — `Executive.Callr` (799), `Executive.Call` (926), `GetIndexedArray` (2574)
- `src/Engine/ProtoCore/DSASM/Defs.cs` — `%generate_range` (383), `%inlineconditional` (385)
- `src/Engine/ProtoCore/Parser/AssociativeAST.cs` — AST nodes (`RangeExprNode` 2213, `AtLevelNode` 309, `InlineConditionalNode` 1940)
- `src/Engine/ProtoCore/Parser/DictionaryBuilder.cs` — dictionary literal lowering
- `src/Engine/ProtoCore/Parser/atg/Associative.atg` — associative grammar (2 119 lines)
- `src/Engine/ProtoCore/Parser/atg/Imperative.atg` — imperative grammar (1 269 lines, not ported)
- `src/DynamoCore/Graph/Nodes/NodeModel.cs` — `UseLevelAndReplicationGuide` (1641–1690)
- `src/DynamoCore/Graph/Nodes/PortModel.cs` — `PortModel` defaults (`Level = 2`, `UseLevels = false`)
- `src/DynamoCore/Graph/Workspaces/SerializationConverters.cs` — `.dyn` readers/writers
- `src/DynamoCore/Scheduler/UpdateGraphAsyncTask.cs` — `ComputeModifiedNodes` (256), `GetDownstreamNodes` (275)
- `src/DynamoCore/Engine/EngineController.cs` — `ComputeSyncData` (282), `UpdateGraphImmediate` (513)
- `src/DynamoCore/Models/RunSettings.cs` — `RunType { Manual, Automatic, Periodic }`
- `src/Engine/ProtoScript/Runners/LiveRunner.cs` — `Subtree`, `DeltaComputation`
- `src/Libraries/CoreNodes/{List,Math,String,Compare,Sorting}.cs` — pure list/math/string functions
- `src/Libraries/CoreNodeModels/*.cs` — node models
- `ABOUT.txt` — third-party credits and the closed-source statement
- `LICENSE.txt` — Apache-2.0
- `doc/distrib/Autodesk.rtf` — closed-source Autodesk licence (not ported)
- `test/Engine/ProtoTest/TD/Associative/Replication.cs` — 483 replication test methods
- `test/Engine/ProtoTest/AtLevel/AtLevelTest.cs` — 56 List@Level tests
