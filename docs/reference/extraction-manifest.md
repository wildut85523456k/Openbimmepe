# Extraction manifest — Revit reference books

Purpose: the reference docs under `docs/reference/revit-books/` cite line ranges of a text
extraction (`arch.txt`, `mep.txt`) — for example `arch-part1-ch1-5.md:3` cites "Text extraction
`arch.txt`, lines 1134-10368". The extraction file itself is **not** committed (the books are
copyrighted; only paraphrase is stored here), so a reviewer cannot reproduce a line reference from
the repository alone. This manifest records what the citations point at and what is needed to
reproduce them. Required by `docs/gap-remediation-plan-2026-10-21.md` row **R6** and by
`docs/reference/README.md:36-37`.

Copyright: the books are the owner's licensed copies. **Neither the book text nor the extraction is
committed to this repository.** Only paraphrase and the book's own field names live here. To
reproduce a citation, take the owner's licensed PDF, run the recorded tool with the recorded
convention, and confirm the SHA-256 below matches.

## Source files

| Cited file | Book (owner's licensed copy) | Publisher / author | Status in repo |
|---|---|---|---|
| `arch.txt` | *Exploring Revit 2026 for Architecture* / *Exploring Autodesk Revit 2026 for Architecture* (the Architecture book, 16 chapters) | CADCIM | Not committed; not in the tree at `b6ef0ee` |
| `mep.txt` | *Exploring Autodesk Revit 2024 for MEP* (10 chapters, 2 case studies) | CADCIM, Tickoo | Not committed; not in the tree at `b6ef0ee` |

`git ls-tree -r` at `b6ef0ee` has no `arch.txt` and no `mep.txt`; the only tracked `.txt` files are
`crates/keymap/presets/revit-style.txt` and `revit-style-mouse.txt` (unrelated). `docs/reference/README.md:36-37`
confirms the extraction is not committed.

## Required manifest fields

| Field | Value | Notes |
|---|---|---|
| **Source file name** | `arch.txt` (Architecture book), `mep.txt` (MEP book) | Confirmed from the citations in `docs/reference/revit-books/*.md`. |
| **SHA-256** | **Not recorded.** | Must be taken from the owner's licensed PDF. It cannot be derived from the paraphrase stored here, and it is not present anywhere in the tree. **Action to close R6: owner computes the SHA-256 of the licensed PDF (one per book) and records it here.** |
| **Tool used** | **Not recorded anywhere in the repo.** | `docs/reference/README.md:36-37` and `docs/agent-team/roles/bim-reference.md:25-26` ask for the tool in a manifest, but no manifest existed. The only PDF-text extraction tool in the repository is `pdftotext` (`spikes/s13_sheets_check/check_sheets.py:35`, `docs/sheets-printing.md:38`), and it is used for **sheet printing**, not the books — so it is only a candidate and is **unconfirmed**. **Action to close R6: owner confirms the tool (and version/flags) that produced `arch.txt`/`mep.txt`.** |
| **Page / line convention** | Line numbers of the **extracted text file**, not PDF pages. | Citations are "lines 1134-10368" (`arch.txt`) and "lines 7102-12820" (`mep.txt`); no PDF page number is cited anywhere in `docs/reference/`. Reproduction must use the same text-extraction layout the tool produces, since line numbers depend on the extractor's wrapping and blank-line handling. |

## Citation index (what each reference doc cites)

| Reference doc | Line | Cites |
|---|---|---|
| `docs/reference/revit-books/arch-part1-ch1-5.md` | `:3` | Source: "Exploring Revit 2026 for Architecture" (CADCIM). Text extraction `arch.txt`, lines 1134-10368. |
| `docs/reference/revit-books/arch-part1-ch1-5.md` | `:5` | Chapter / Title / Approx. line range in `arch.txt` table header. |
| `docs/reference/revit-books/arch-part2-ch6-10.md` | `:3` | Source: "Exploring Autodesk Revit 2026 for Architecture" (CADCIM), text extraction `arch.txt`, lines 10368-19842. |
| `docs/reference/revit-books/arch-part3-ch11-16.md` | `:3` | Source: "Exploring Autodesk Revit 2026 for Architecture" (CADCIM), extracted text `arch.txt`. |
| `docs/reference/revit-books/mep-part2-ch4-6.md` | `:3` | Source: "Exploring Autodesk Revit 2024 for MEP" (CADCIM, Tickoo). Text extract `mep.txt`, lines 7102-12820. |

All line numbers above are as they stand at `b6ef0ee`.

## To close R6 (owner action)

1. From the licensed PDF of each book, compute the SHA-256 and fill it into the table above
   (one hash per book, keyed to the PDF the extraction was made from).
2. Confirm the extraction tool and its version/flags (the only in-repo candidate is `pdftotext`,
   used elsewhere for sheet printing and **not** confirmed for the books).
3. Once the tool and convention are fixed, re-extract and confirm the cited line ranges
   (`arch.txt` 1134-10368 / 10368-19842; `mep.txt` 7102-12820) still land on the same content;
   adjust a citation only with a note of the extractor version, since line numbers are
   extractor-dependent.

Until then, every `arch.txt` / `mep.txt` line reference in `docs/reference/` is reproducible only
against the owner's licensed PDF with the (still unrecorded) tool and convention.
