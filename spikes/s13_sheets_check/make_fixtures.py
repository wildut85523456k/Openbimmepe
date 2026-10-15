"""Builds the small Excel files the Rust import tests read (crates/sheets/tests/data).
Needs: pip install openpyxl. Run from the repository root: python spikes/s13_sheets_check/make_fixtures.py"""
import datetime
import pathlib

from openpyxl import Workbook

out = pathlib.Path("crates/sheets/tests/data")
out.mkdir(parents=True, exist_ok=True)


def book(name, rows, extra_sheet=None):
    wb = Workbook()
    ws = wb.active
    ws.title = "Sheets"
    for r in rows:
        ws.append(r)
    if extra_sheet:
        w2 = wb.create_sheet("Second")
        for r in extra_sheet:
            w2.append(r)
    wb.save(out / name)


# header variants: punctuation, case and underscores differ from the field names
book(
    "basic.xlsx",
    [
        ["Sheet No.", "NAME", "Paper", "drawn_by", "design date", "Scale", "View", "Extra Col", "Current Revision"],
        [101, "Ground floor plan", "A1", "WD", datetime.datetime(2026, 10, 2), "1:100", "GF Architectural", "x", "9"],
        ["M-101", "Étage 0 – ventilation", "iso a1", "KS", None, 50, "GF Mechanical", "y", None],
        [None, None, None, None, None, None, None, None, None],
        ["C-001", "Overview", "A3", "AB", "2026-10-03", "1:200", "No Such View", None, None],
        ["S-1", "Not placed", "A3", None, None, None, None, None, None],
    ],
    extra_sheet=[["Sheet Number", "Sheet Name"], ["Z-1", "On the second worksheet"]],
)

book(
    "bad_rows.xlsx",
    [
        ["Sheet Number", "Sheet Name", "Sheet Size", "Scale", "View"],
        ["A/1", "slash in number", "A1", None, None],
        ["B-1", "ok row", "A1", None, None],
        ["B-1", "duplicate of the row above", "A1", None, None],
        ["B-2", "unknown size", "A0 extra", None, None],
        ["B-3", "", "A1", None, None],
        ["B-4", "bad scale", "A1", "one to ten", "GF Architectural"],
        [None, "no number", "A1", None, None],
        ["B-5", "x" * 2000, "A1", None, None],
    ],
)

book("ambiguous.xlsx", [["Sheet Number", "Number", "Sheet Name"], ["A-1", "A-2", "n"]])
book("no_number.xlsx", [["Sheet Name", "Sheet Size"], ["n", "A1"]])
book("empty.xlsx", [])

book(
    "update.xlsx",
    [
        ["Sheet Number", "Sheet Name", "Drawn By", "Checked By"],
        ["A-101", "Ground floor plan (revised)", None, "RM"],
        ["N-9", "New sheet", "ZZ", None],
    ],
)

book(
    "with_titleblock_column.xlsx",
    [
        ["Sheet Number", "Sheet Name", "Titleblock", "Project Name", "Approved By"],
        ["T-1", "Named title block", "Generic A3", "Override project", "PM"],
        ["T-2", "Unknown title block", "Generic A0", None, None],
    ],
)
# header not in A1: blank rows above and columns to the left, so spreadsheet row numbers differ from range rows
wb = Workbook()
ws = wb.active
ws.title = "Sheets"
ws["C3"], ws["D3"], ws["E3"] = "Sheet Number", "Sheet Name", "Sheet Size"
ws["C4"], ws["D4"], ws["E4"] = "O-1", "Offset ok", "A1"
ws["C5"], ws["D5"], ws["E5"] = "O/2", "Offset bad number", "A1"
wb.save(out / "offset.xlsx")
print("written to", out)
