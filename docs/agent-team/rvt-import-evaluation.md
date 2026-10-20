# RVT-to-IFC import: converter evaluation

## User requirement

Provide an in-app RVT-to-IFC conversion/import step. This does not introduce RVT authoring or make RVT the native project format. Preserve the existing main design: native operation-log/snapshot storage, Rust IFC handling and the Rust/egui desktop. This is an added import requirement, not a replacement architecture.

## Retrieved candidates

| Candidate | Evidence | Assessment |
|---|---|---|
| Rust `rvt` / rvt-rs, published 0.4.0 | Apache-2.0 Rust library and CLI; exposes `RevitFile`, `RvtDocExporter`, IFC4 STEP output and export diagnostics.[1] | Strong candidate for a local Rust feasibility test, not a universal production converter. |
| ODA BimRv | Vendor documents standalone Revit-to-IFC conversion without the Revit application and paid component/membership requirements.[4] | Commercial fallback to evaluate with vendor licensing and a supported-version/schema matrix; no license purchased and no SDK evaluated. |
| Autodesk Revit Automation API | Autodesk documents RVT-to-IFC export through a Revit API app bundle without installing/opening Revit Desktop.[5] | Autodesk-hosted conversion route; not an all-Rust converter. Cloud credentials, service terms/cost and explicit upload approval are prerequisites. |

## Rust candidate limitations

The published crate describes typed building recovery primarily for 2024/2025 project files and explicitly calls itself a reader, not a converter for every model.[1] The current main-branch supported profile distinguishes opening metadata from recovering elements, geometry and parameters; it describes bounding-box approximations for several categories and missing parameters/phases.[2] Its machine-readable matrix explicitly marks converter-grade RVT-to-IFC unsupported.[3]

The main-branch profile and matrix differ in detail, including experimental version coverage; do not combine claims from unreleased main with a pinned published crate. Pin the exact version/commit and validate its actual diagnostic output. No performance or conversion-fidelity result has been measured here.

## Proposed application boundary

RVT source -> local converter worker -> IFC4 plus conversion diagnostics -> IFC validation -> user reviews completeness/approximations -> Rust IFC importer -> native document.

The conversion worker must be cancellable, bounded in time/memory, and run off the UI thread. Preserve the source RVT unchanged; do not overwrite existing files. Default to local conversion, never upload automatically. Unsupported releases/models must fail explicitly. Geometry-only or bounding-box recovery must be clearly labelled and must not be presented as complete BIM conversion.

Do not bypass the missing native IFC importer: producing an IFC file is not equivalent to importing that model into the editable document. Preserve stable identity and source provenance through both stages. Document categories, counts, units, coordinates, levels, materials, type/instance properties, openings/hosts and MEP ports/systems; report losses rather than silently discard them.

## Validation gate before production integration

Use representative architectural, structural and MEP RVT files, with their Revit version and redistribution/privacy permission. Where available, compare against IFC exports from the same files made by Revit. Inspect identity/count/geometry/property discrepancies and fail unsupported or below-threshold conversions. Measure wall time, peak memory and cancellation; test the real in-app workflow separately.

Current result: research only. No converter installed, proprietary SDK licensed, RVT uploaded, or real model converted. A representative RVT and ideally its reference IFC are needed to validate suitability for this project.

## Sources

[1] https://docs.rs/rvt/0.4.0/rvt
[2] https://raw.githubusercontent.com/DrunkOnJava/rvt-rs/main/docs/supported-profile.md
[3] https://raw.githubusercontent.com/DrunkOnJava/rvt-rs/main/docs/support-matrix.json
[4] https://www.opendesign.com/blog/2022/january/oda-adds-revit-ifc-conversion
[5] https://aps.autodesk.com/blog/export-ifc-rvt-using-revit-automation-api-part-i
