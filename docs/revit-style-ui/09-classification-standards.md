# 09 — Family classification standards

Status: **design specification and adaptation decision**, not implemented application code.
Companion to the family library (`crates/family/src/library.rs`, row **C3** of
[Gap remediation plan](../gap-remediation-plan-2026-10-21.md)) and to
[08 — Gap closure decisions](08-gap-closure-plan-2026-10-21.md), decision 3.

## 1. Decision

The family library is classified with **Uniclass 2015** (NBS, United Kingdom), using the
`Pr` (Products) table for family codes, `Ss` (Systems) for the system layer and `EF`
(Elements/functions) for the element layer. Uniclass is adopted **over** Revit's American
`BuiltInCategory` codes and over OmniClass, because:

- it is the British standard the owner asked to follow;
- it is aligned to **ISO 12006-2** and is maintained quarterly (latest release July 2026);
- it classifies MEP products by function in the process (`Pr_60` source, `Pr_65`
  distribution, `Pr_70` outlet, `Pr_75` control), which is a better fit for an MEP family
  library than Revit's category list;
- it is the only one of the three that is freely usable under an open licence (see §5).

Revit's `BuiltInCategory` remains the reference for the *internal* `Category` type (it is
what the IFC class derives from), but it is **not** a classification system: it is a closed
API enum (972 members) and it is coarser than any standard — it has no `Valve`, `Pump`,
`Fan`, `Boiler`, `Chiller` or `Transformer` category; those are distinguished by subcategory
and connector system classification instead [18][19].

## 2. The three stacks

| Stack | System | Publisher | Tables | Status | Licence |
|---|---|---|---|---|---|
| American | **OmniClass** | CSI (+ ICC) | 15 hierarchical tables: 11–14 entities/spaces, 21 Elements, 22 Work Results, **23 Products**, 31–35 phases/roles, 36/41–46 | maintained; "based on ISO 12006-2" [22] | **commercial** — company licence required; CSI trademark [15][16] |
| American | MasterFormat / UniFormat | CSI + CSC | 50 spec divisions / elemental (ASTM E1557) | live | licensed |
| British | **Uniclass 2015** | NBS | **15 tables** (see below), quarterly updates | **July 2026 release**; aligned to ISO 12006-2:2015 [21] | **CC BY-ND 4.0** [1] |

Uniclass 2015 tables, July 2026 [1]:

| | Table | Version |
|---|---|---|
| `Ac` | Activities | v1.27 |
| `Co` | Complexes | v1.22 |
| `En` | Entities | v1.34 |
| `SL` | Spaces / locations | v1.36 |
| `EF` | Elements / functions | v1.16 |
| `Ss` | **Systems** | v1.43 |
| `Pr` | **Products** | v1.43 |
| `TE` | Tools and equipment | v1.18 |
| `PM` | Project management | v1.30 |
| `Ro` / `RK` / `Ma` / `PC` / `FI` / `Zz` | Roles / Risk / Materials / Properties / Form of information / CAD | v1.13 / v1.0 / v1.1 / v1.3 / v1.7 / v1.3 |

Mapping to OmniClass: OmniClass Table 23 ≈ Uniclass `Pr`, Table 21 ≈ `EF`, Table 22 ≈ `Ss` —
but OmniClass is licence-gated and Uniclass is not.

## 3. ISO / European status

| Standard | Title | European adoption | Status |
|---|---|---|---|
| **ISO 12006-2:2015** | Framework for **classification** | **EN ISO 12006-2:2020**, CEN/TC 442 [11] | **under revision** — `ISO/DIS 12006-2:2024` → `prEN ISO 12006-2:2025`, retitled *"Framework for classification and breakdown structures"* [14] |
| **ISO 12006-3:2022** | Framework for object-oriented information | **EN ISO 12006-3:2022** (CEN approved 7 Jun 2022; supersedes the 2016 edition) [23] | published |
| **ISO 16739-1** | **IFC** data schema | EN ISO 16739-1 | published |
| **ISO 19650** series | BIM information management | EN ISO 19650-1…7 | published |
| ISO 23386 / 23387 | Data dictionary / data templates | — | published |
| **ISO 81346-12:2018** | Reference Designation System, construction works + building services | — | published, confirmed [12] |
| eCl@ss / ETIM | Product data models (ISO 13584 / IEC 61360) | — | paid associations [20] |

The decisive point: **ISO 12006-2 is a framework only.** Its scope says it *"does not provide
a complete operational classification system, nor does it provide the content of the tables,
though it does give examples. It is intended for use by organizations which develop and
publish such classification systems and tables"* [11]. So there is no free ISO classification
table — ISO names the tables and leaves the content to national bodies. That is why Uniclass
exists, and why adopting one carries a licensing consequence (§5).

The revision matters: the draft table set is now **Asset** (Complex, **Zone**, Space, Entity,
Element, **Component**), **Process** (Control, Activity, Task), **Resource** (Actor, Aid,
Information) and **Supporting** (Property, Uncertainty, Purposes) [14] — a `Component` table
and a `Zone` table are new, which matters if FM/COBie export is planned.

## 4. Family → code mapping

Codes verified against the live Uniclass July 2026 tables.

### 4.1 The library families (all coded)

| Family (`library.rs`) | Uniclass code | Name | Confidence |
|---|---|---|---|
| Lever handle | `Pr_30_36_59` | Opening hardware | best fit |
| Door, single flush | `Pr_30_59_24` | Doorsets | solid |
| Window, fixed with mullions | `Pr_30_59_98` | Window units | solid |
| Door, arched | `Pr_30_59_24` | Doorsets (same product family, different type) | solid |
| Fan coil unit, ducted | `Pr_70_65_04_88` | Terminal fan units | best fit |
| Air handling unit | `Pr_60_65_03` | Air-handling units | solid |
| Ceiling diffuser, square | `Pr_70_65_04_02` | Air diffusers | solid |
| VAV terminal unit, single duct | `Pr_70_65_04_94` | Variable air volume (VAV) terminal units | solid |
| Pump, end suction | `Pr_65_53` | Pump products | solid |
| Valve, globe | `Pr_65_54` | Valve products | solid |
| WC, close coupled | `Pr_40_20_93` | Urinal and WC fittings | solid |
| Washbasin, wall hung | `Pr_40_20_96` | Washbasins, sinks and troughs | solid |
| Sprinkler head, pendent | `Pr_70_55_84` | Sprinkler heads | solid |
| Luminaire, recessed | `Pr_70_70_48` | Luminaires, lighting track and lighting accessories | solid |
| Distribution board, 12-way | `Pr_60_70_22` | Distribution boxes and switchboards | solid |
| Pipe hanger, clevis | `Pr_20_85_47` | Legs, pedestals, hangers, stringers | best fit (no dedicated code) |

All sixteen are authored in `library.rs` and asserted by two tests:
`crates/family/tests/mep.rs::every_library_family_carries_a_uniclass_code` (the invariant: every
family carries a code) and `::library_families_carry_the_decided_uniclass_codes` (the decided
code per family). The IFC export test
`crates/ifc/tests/ifc.rs::every_library_family_classifies_in_ifc_export` asserts the codes reach
the export. Adding a family without a code or a decided-code entry fails these tests.

### 4.2 Row C3 — the MEP library (built)

These are now built in `library.rs` (see §4.1). The WC/basin row is realised as two families,
`WC, close coupled` and `Washbasin, wall hung`; the hanger/support row as `Pipe hanger, clevis`.

| Family | Uniclass code | Name |
|---|---|---|
| AHU | `Pr_60_65_03` | Air-handling units [4] |
| Ceiling diffuser | `Pr_70_65_04_02` | Air diffusers [3] |
| VAV | `Pr_70_65_04_94` | Variable air volume (VAV) terminal units [3] |
| Pump | `Pr_65_53` | Pump products — `_24` drainage, `_86` supply/exhaust, `_96` water supply and wastewater [5] |
| Valve | `Pr_65_54` | Valve products — `_95` building services, `_94` water supply and wastewater, `_30` firefighting [6] |
| WC / basin | `Pr_40_20_93` / `Pr_40_20_96` | Urinal and WC fittings / Washbasins, sinks and troughs [7] |
| Sprinkler head | `Pr_70_55_97` (`_84` sprinkler heads) | Water fire-extinguishing outlets [8] |
| Luminaire | `Pr_70_70_48` | Luminaires, lighting track and lighting accessories [9] |
| Panel board | `Pr_60_70_22` | Distribution boxes and switchboards (also `Pr_60_70_48` LV switchgear, `Pr_60_70_15` control and protection) [10] |
| Hanger / support | — | **no dedicated `Pr` code**; nearest are `Pr_20_85_47` (legs, pedestals, hangers, stringers) and `Pr_20_29_31_44` (joist hangers). Pipe/duct supports sit in the `Ss` pipework/ductwork systems, not in `Pr` |

Structural note: Uniclass splits MEP products by function in the process — `Pr_60` *source*
(boilers, chillers, AHUs, power supply), `Pr_65` *distribution* (pipe, duct, cable, **pump,
valve**), `Pr_70` *outlet* (diffusers, sprinklers, WC, luminaires, receptacles), `Pr_75`
*control* (dampers, sensors, actuators, control panels). This lines up with the existing
`System` enum (`SupplyAir`, `ChilledWaterSupply`, `Power`, …) as the `Ss` layer.

### 4.3 Rows C15, C16, C17, C18

| Row | What it adds | Uniclass home |
|---|---|---|
| **C15** — plumbing fixtures | fixture families with WFU / HWFU / CWFU type parameters | `Pr_40_20` (sanitary appliances) — same table as the WC/basin row above |
| **C16** — sprinkler heads | pendent and upright head families, design area, wet/dry system | `Pr_70_55` (water fire-extinguishing outlets) — sprinkler heads at `Pr_70_55_84` |
| **C17** — electrical as objects | panels, devices and luminaires as elements; circuits; panel schedules; cable tray/conduit | `Pr_70_70` (lighting), `Pr_60_70` (power supply), `Pr_60_70_22` (distribution boards) |
| **C18** — hangers / supports | MEP families hosted on beams, slabs or trusses, auto-placed along a run | no dedicated `Pr` code; `Ss` systems table, nearest `Pr_20_85_47` |

## 5. Licence finding (blocker)

- **Uniclass is CC BY-ND 4.0** — NoDerivatives. NBS states: *"users should not adapt the
  tables and add their own codes, but instead contact NBS to suggest additions and changes"*
  [1]. Storing a code plus the string "Uniclass" on a family is **use**; embedding the full
  table, or a mapping file that copies Uniclass names and structure, is **adaptation** and is
  the risky form.
- **OmniClass is worse** — commercial licence and a CSI trademark [16].
- **eCl@ss / ETIM are paid** associations [20].
- **ISO / CEN standards are sold** by national standards bodies; only previews are free.

**Consequence for the repo:** ship **only** the code + edition + the literal string
"Uniclass" on a family; never vendor the Uniclass XLS/JSON and never copy the table content
into the codebase; record the licence in the provenance audit. The app already stores
classification as author-typed `(system, code)` pairs (`crates/family/src/lib.rs:456`), so no
mechanism change is needed — only the authoring discipline.

## 6. How the app stores and exports it

`Family::classification: Vec<(String, String)>` — "(classification system, code) pairs typed
in by the author, for example a Uniclass or OmniClass code" (`crates/family/src/lib.rs:456`),
validated at compile (max 8, no duplicate system, 64-char limit, `lib.rs:1451-1466`), exposed
via `Compiled::classification()` (`lib.rs:1612`), and exported to IFC as a
`Pset_OpenBIM_Family.Classification` label (`crates/ifc/src/families.rs:237-238`).

**Improvement (not yet done):** IFC has a native `IfcClassification` +
`IfcClassificationReference` (source, edition, editionDate, name). Today the code writes
classification as a property-set label, which a viewer shows as text but does not treat as a
classification. Emitting `IfcClassificationReference` would make Uniclass codes appear as
classifications natively in Solibri / Navis / other viewers — an `openbim-ifc` change.

## 7. Sources

[1] https://uniclass.thenbs.com/download
[2] https://uniclass.thenbs.com/taxon/pr
[3] https://uniclass.thenbs.com/taxon/pr_70_65_04
[4] https://uniclass.thenbs.com/taxon/pr_60_65
[5] https://uniclass.thenbs.com/taxon/pr_65_53
[6] https://uniclass.thenbs.com/taxon/pr_65_54
[7] https://uniclass.thenbs.com/taxon/pr_40_20
[8] https://uniclass.thenbs.com/taxon/pr_70_55
[9] https://uniclass.thenbs.com/taxon/pr_70_70
[10] https://uniclass.thenbs.com/taxon/pr_60_70
[11] https://standards.iteh.ai/catalog/tc/cen/b0e05107-d6bf-4c1c-b222-54dd974f1a96/cen-tc-442
[12] https://www.iso.org/standard/63886.html
[13] https://interoperable-europe.ec.europa.eu/collection/rolling-plan-ict-standardisation/construction-building-information-modelling-rp2025
[14] https://www.buildwise.be/media/nipnohqc/nabimdigitalconstruction_overviewcentc442_nl.pdf
[15] https://www.csiresources.org/standards/omniclass/standards-omniclass-about
[16] https://theconstructionstandard.com/omniclass-table-23
[17] https://www.thenbs.com/knowledge/uniclass-2015-january-2022-update
[18] https://interoperability.autodesk.com/classificationmanager/hosteddbs/subcat.xlsx
[19] https://github.com/CodeCavePro/revitless-toolkit/blob/master/src/Decompiled/BuiltInCategory.cs
[20] https://www.etim-international.com/about-us/collaborations
[21] https://www.thenbs.com.au/about-nbs/latest-news/classification-and-transport-for-new-south-wales
[22] https://legacy.wbdg.org/resources/omniclass
[23] https://standards.iteh.ai/catalog/standards/sist/40a274bf-8b44-44ff-ae9f-b4939f992f1a/sist-en-iso-12006-3-2022
