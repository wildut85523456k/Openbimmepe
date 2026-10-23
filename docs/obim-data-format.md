# What the app keeps that IFC has no place for

IFC cannot say everything this app knows (a Compute dialog's inputs, the layout of a sheet, which fitting is joined to
which duct, the fingerprint that tells an import a member was edited elsewhere). So the app keeps its own data in one
format and carries it in two places. This note says what is in it and what for, and how to add more without breaking
anything. Code: `crates/document/src/native.rs` (records), `crates/ifc/src/lib.rs` (property sets),
`crates/ifc/src/pset_map.rs` (parameter mapping), `apps/desktop/src/import.rs` (reading it back).

## 1. Where it lives

| Place | What it is | Written by | Read by |
|---|---|---|---|
| `.obim` file | The saved model: one record per line, first line `F\|obim-records-1`. | Save | Open |
| `OpenBIM_Document` property set on the `IfcProject` | The same lines, one per property (`R0000000`, `R0000001`...), each a text. Only when the export is asked to carry the app's own data. | Export IFC | Import IFC (`restore`): elements come back exactly, with the same numbers; what another program changed is read from the file's geometry; what it deleted stays deleted. |
| Per-product property sets (section 3) | A few values on each exported product, readable by any IFC tool. | Export IFC | Import IFC, other programs |

A file without `OpenBIM_Document` (own data switched off, or written by another program) still imports from its geometry
and property sets, with less kept.

## 2. The records

One line per record: a tag, then `key=value` fields between `|`. Numbers are written to read back to the same bits. Text
has every character that is not a letter, digit, space, `.`, `-`, `_` or `+` written as `~` and six hex digits, so a line
never holds a `|`, `;`, `=` or line break of its own.

| Tag | Holds | For |
|---|---|---|
| `F` | the format name | refuse a file of another format |
| `S` | site: latitude, longitude, angle to true north, UTC offset | the map and the sun |
| `Y` | project key (hex) | keeps the IFC GlobalIds of one project apart from every other's, stable across exports |
| `E` | an element: id, category, name, sizes, elevation, geometry, outline, material, usage, family and its overrides, texts, type, layers, pinned... | the model itself. `Generic` is a category (products kept from a foreign file). |
| `T` `L` `P` `Z` | duct and pipe fittings, links between ports, taps, the system a run was given | what is joined to what (IFC has ports, not the designer's intent) |
| `X` | schematic zones | grouping of end points for the single-line views |
| `N` | schematic end points: role, demand | MEP schematics |
| `M` | mechanical settings lists | sizes, insulation and lining types |
| `J` | insulation and lining on runs and fittings | quantities |
| `G` | named systems of equipment | the user's own system names |
| `B` `C` | electrical nodes (source, boards, loads) and cables | the electrical network, sent to edesign through the exchange file (`docs/decisions/0003-electrical-exchange.md`) |
| `Q` | run ends joined to equipment connectors | |
| `H` | schedule definitions | schedules |
| `W` | view settings (scale, detail, view range, hidden categories and elements) | views |
| `R` | sheets, with title block and the views on them | sheets |
| `U` | the app's own settings, one line per kind (section 4) | anything that is not the model |

## 3. Property sets written on products

| Set | On | Holds | For |
|---|---|---|---|
| `OpenBIM_Document` | project | the records (section 1) | exact re-import |
| `OpenBIM_Element` | beams, columns, slabs, footings, walls, stairs, railings, roofs, ducts, pipes, generic models | `ElementNumber`, `Category`, `TypeName`, `Fingerprint` | an import matches a product to its element, and knows the file was edited elsewhere (the fingerprint no longer fits) |
| `OpenBIM_Source` | generic models (IfcBuildingElementProxy) | `IfcClass`, `PredefinedType`, then `<set>.<property>` for each property the source file had | nothing the source said is lost; the class a proxy was is read back |
| `OpenBIM_Type` | type objects | the type's values IFC has no attribute for | types come back with their sizes |
| `OpenBIM_WallType` | wall types | core first and last layer, wrapping layers, which side the layers run from | layered walls |
| `OpenBIM_Family` / `OpenBIM_Instance` | family types / occurrences | the family parameters that the mapping table (below) does not name | family values come back |
| `OpenBIM_Space` / `OpenBIM_SpaceVolumes` | spaces | `Usage`, `Occupancy`, `Number`; slab-to-slab, ceiling void and floor build-up volumes | rooms come back with number and department |
| `OpenBIM_Equipment` | placeholder equipment | `Placeholder` | marks a stand-in the app made for equipment that has no product of its own |

Family and shared parameters go to the property set and name the table says (`pset_map`: tab-separated text, one row per
parameter: IFC class or `*`, parameter, set, property, value kind). It is edited by hand and nothing is shipped; what the
table does not name stays in `OpenBIM_Family` and `OpenBIM_Instance`.

## 4. Adding something: the `U` line

A new setting or piece of project data that is not the model gets its own **kind**, and needs no change to the format:

```
U|kind=<kind>|<key>=<value>|<key>=<value>...
```

- A kind is lower-case letters, digits, `_` or `-`; one line per kind. Use the text escaping above for values.
- Read and write with `Document::app_record(kind)` and `Document::put_app_record(kind, Some(line))` (`None` removes). Not an edit:
  no revision, nothing to undo.
- It is saved in the `.obim` file and exported in `OpenBIM_Document` without any further code. The app keeps a kind it does
  not own as it is when it saves (`sync_app_records` rewrites only `compute` and `georef`) and when it reads the file.
- On IFC import, a kind the model has no line of yet is taken, and the import says so; the model's own line wins.
- Kinds in use: `compute` (Compute dialog inputs), `georef` (map name, datum, projection, shared coordinates, elevation).
- Put the fields of a new kind in this table when it is added. Add fields to an existing kind at the end and read a missing
  one as its default; never reuse a key for another meaning.

For a value that belongs to one element (a Generic Model's source properties, a family's marks), use the element's `texts`
(the `tx=` field of its `E` record): at most 64 name/value pairs of 64 and 256 bytes.

For a new kind of **element**, add a `Category` and its name in the table at the top of `native.rs`; the record already
carries every field an element has.

## 5. Rules that keep it open

1. Never change the meaning of a tag or a key. Add a key; read a missing key as its default.
2. A reader ignores a `U` kind it does not know and keeps it.
3. A line holds one record and no line break; text is escaped.
4. Every record that matters for a re-import has a test that writes, reads back and compares (`crates/document/tests`, `apps/desktop/src/import.rs`).
5. What IFC can say goes in IFC first (a property set any program reads); the record is for what it cannot.
