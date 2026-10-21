# Reference material

`revit-books/` holds working references distilled from three CADCIM textbooks supplied by the project owner:

| File | Source | Covers |
|---|---|---|
| `arch-part1-ch1-5.md` | Exploring Revit 2026 for Architecture | project setup, walls, doors/windows/openings, modify tools |
| `arch-part2-ch6-10.md` | same | levels, grids, reference planes, floors, roofs, ceilings, stairs, railings, curtain walls, toposolid, massing |
| `arch-part3-ch11-16.md` | same | annotation, schedules, sheets, 3D, rendering, phasing, design options, links, worksharing |
| `mep-part1-ch1-3.md` | Exploring Autodesk Revit 2024 for MEP | templates, units, linking, levels/grids, architectural tools used by MEP |
| `mep-part2-ch4-6.md` | same | spaces/zones, analysis, air terminals, equipment, duct systems, Generate Layout, duct placement and sizing, electrical |
| `mep-part3-ch7-10.md` | same | piping/plumbing, fire protection, documentation, massing and families, worksharing |

Structure book (Exploring Autodesk Revit 2024 for Structure): **not yet distilled**, deferred by the owner to a later stage.

## How to read them

- Each file has a tutorial spine (build order from scratch), a per-tool section (where, preconditions, mouse sequence, options bar, parameters, rules, gotchas, IFC4 mapping), an acceptance checklist and open questions.
- `[book]` = stated by the book. `[knowledge]` = general Revit/IFC knowledge added by the author, not verified against the book or a running Revit. Treat `[knowledge]` as a hypothesis to confirm.
- Text is paraphrased. The books are copyrighted; do not paste book text into this repository.
- The books describe Revit as shipped in their edition and contain a few errors; each file lists those it found under "Open questions".

The comparison between these references and the app is in `docs/gap-analysis-2026-10-21.md`; line-level evidence is in `docs/audits/`.

## Verification

- Factual spot-check (2026-10-21) against the source books: five claims checked, all five held -
  Generate Layout solution counts (Network up to six, Perimeter five, Intersection up to eight),
  sprinkler coverage (one head per 130 sq ft, i.e. per 12 m2, schedule formula `Area/130` / `Area/12`),
  the colour-scheme "By value / By range" distinction, and the zone perimeter rule (shared edges of
  adjacent spaces count once). The pipe chapter's Network example showing five solutions while the duct
  chapter says up to six is the book's own inconsistency, recorded here rather than corrected.
- The `file:line` citations in `docs/gap-analysis-2026-10-21.md` and its three companion audits were
  re-checked against the SHA they name; see `docs/gap-remediation-plan-2026-10-21.md` section 8.
- The books are the owner's licensed copies. Only paraphrase and the book's own field names are stored
  here; the extraction itself is not committed. To reproduce a citation, record the source file name,
  its SHA-256 and the extraction tool in an extraction manifest.
