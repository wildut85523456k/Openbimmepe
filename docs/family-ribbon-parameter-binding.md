# Family Ribbon + Parameter Binding — Design Contract

Status: **deferred — next major feature**. This document is the acceptance contract to be
implemented in a dedicated track. It follows Revit's family/parameter model so the app behaves
the way users expect.

## 1. Goal

Let a user create a custom family (parametric component) and **bind any parameter or
constraint to it — visibility, color, dimensions, materials, nested family swaps — so that
changing one driving parameter re-evaluates the model**, exactly as Revit's Family Editor does.
UI surface: a **Family ribbon tab** (Create / Modify / Family Properties / Parameters) plus a
**Family Types** dialog and a **Parameter Properties** dialog.

## 2. Revit reference model (the approach we copy)

### 2.1 Parameter taxonomy
- **Family parameters** — belong to the family; not shared across families. Created in the
  Family Types dialog with a name, discipline (Common/Structural/Electrical/…), type
  (Length/Angle/Integer/Number/Text/YesNo/Material/…), and group (Dimensions/Identity Data/…).
- **Shared parameters** — defined in an external shared-parameter file (GUID + name + group +
  discipline + type); bindable to multiple families/categories; required for schedules/tags.
- **Type parameters** — value is per type (e.g. "Width" of a door type).
- **Instance parameters** — value is per placed instance (e.g. "Sill Height").
- **Built-in parameters** — read-only-ish system params (Width, Height, …) exposed by the
  category.

### 2.2 Binding & formulas
- Each family parameter may have a **formula** referencing other parameters
  (e.g. `Width = Height * 2 + 10 mm`). Formulas support `+ - * / ^ ( )`, conditionals
  `if(cond, a, b)`, and functions (`sqrt`, `abs`, `min`, `max`, `round`, …).
- A parameter can drive **geometry**: dimensions (parametric length), angles, visibility
  (YesNo → show/hide an element), materials (Material param → element material),
  nested family type (Family Type param → which nested family is placed).
- **Constraints**: equality/locking between dimensions; "is driving" vs "is driven".

### 2.3 Family editor workflow
1. Create Family (pick a template category) → opens Family Editor mode (ribbon switches to
   Family tabs: Create, Modify, Family Properties, Parameters).
2. Draw geometry, add dimensions, label dimensions with parameters.
3. Add parameters (Family Types dialog), set formulas.
4. Test (Family Types dialog → change a value → geometry updates).
5. Load into Project (and optionally save .rfa).

### 2.4 Family Types dialog
Table of types (rows) × parameters (columns); edit in place; Add/Duplicate/Delete/Rename;
Apply commits the whole table as one undo step; invalid row aborts the whole commit.

## 3. Architecture for our app (Rust, egui)

### 3.1 Data model (openbim_document)
- `Parameter { id, name, discipline, kind, group, is_instance, formula: Option<String>,
  value: ParamValue }` stored on `FamilyDef` (and shared params in a separate registry).
- `ParamValue = Number(f64) | Integer(i64) | Text(String) | YesNo(bool) | Material(Id) |
  FamilyType(Id)`.
- `FamilyDef.parameters: Vec<Parameter>`; `FamilyType` (existing type row) gains
  `param_values: BTreeMap<ParamId, ParamValue>`.
- Geometry elements inside a family reference parameters by id in their dimensions/visibility/
  material slots (e.g. `Line.length_param: Option<ParamId>`).

### 3.2 Expression engine (new crate/module)
- A small safe evaluator: tokenize → shunting-yard/RPN → evaluate against a parameter
  context. No side effects, no I/O.
- Cycle detection before commit; error reported per parameter.
- Units: lengths stored in metres internally; formulas in mm in the UI, converted.

### 3.3 Re-evaluation
- When a driving parameter changes, recompute all dependent parameters (topological order),
  then regenerate the family's geometry instance for each placed instance.
- Undoable as one transaction.

### 3.4 UI
- **Ribbon**: new "Family" tab group (Create/Modify/Family Properties/Parameters) shown only in
  Family Editor mode; toggle Family Editor mode via "Create Family" / "Load into Project".
- **Family Types dialog** (extends existing `family_types_ui.rs`) with a "Parameters" button →
  Parameter Properties dialog.
- **Parameter Properties dialog**: name, discipline, type, group, instance/type toggle, formula
  editor with live validation.
- **Family Editor viewport**: edit geometry with dimension labels; click a dimension → assign
  parameter.

## 4. Acceptance criteria (v1)

1. User can create a family from a template category and enter Family Editor mode.
2. User can add a family parameter (name/type/group/instance-vs-type) and see it as a column in
   Family Types.
3. User can enter a formula for a parameter referencing other parameters; cycles and type
   mismatches are reported; valid formula re-evaluates values.
4. A dimension can be bound to a length parameter; changing the parameter updates geometry.
5. A YesNo parameter can drive element visibility.
6. A Material parameter can drive an element's material.
7. Family Types table commits as one undo step; invalid row aborts whole commit.
8. Load into Project places instances whose type parameters are editable in the project.
9. Shared parameters can be added from a shared-parameter file and bound to a category.

## 5. Out of scope for v1
- Nested families with type-parameter mapping (v2).
- Constraints/equality locking between dimensions (v2).
- Type catalogs on disk (v2 — CSV already supported for types).
- Array/loop constructs in formulas (v2).

## 6. Implementation order (suggested)
1. Data model + persistence (Parameter on FamilyDef, param_values on types).
2. Expression engine crate with tests (golden cases).
3. Family Types dialog: parameter columns + add/remove (no formulas).
4. Formula editor + re-evaluation.
5. Geometry binding (dimension → param, visibility, material).
6. Family Editor mode + ribbon tab + Load into Project.
7. Shared parameters.

## 7. Open questions
- Should formulas be stored in metres or in the parameter's display unit? (Revit stores in
  the parameter's unit; recommend storing in display unit and converting at eval.)
- Shared-parameter file format: reuse Revit's .txt shared-parameter format for
  interoperability? (Recommend yes for import compatibility.)
- Undo granularity for formula edits: per-keystroke vs on-apply. (Recommend on-apply.)
