"""Check portrait_scene exports independently with pypdf and ezdxf.

Usage: python spikes/s13_sheets_check/check_portrait.py OUTDIR
Requires pypdf and ezdxf.
"""
from pathlib import Path
import math
import sys

import ezdxf
from pypdf import PdfReader


def close(actual, expected, tolerance):
    if not math.isclose(actual, expected, rel_tol=0, abs_tol=tolerance):
        raise AssertionError(f"{actual} != {expected}")


def main(folder):
    reader = PdfReader(folder / "orientations.pdf")
    if len(reader.pages) != 10:
        raise AssertionError("Expected ten PDF pages")
    page_index = 0
    for n, (short, long) in enumerate([(841, 1189), (594, 841), (420, 594), (297, 420), (210, 297)]):
        for mode, width, height in [("portrait", short, long), ("landscape", long, short)]:
            page = reader.pages[page_index]
            close(float(page.mediabox.width) * 25.4 / 72, width, 0.001)
            close(float(page.mediabox.height) * 25.4 / 72, height, 0.001)
            page_index += 1
            doc = ezdxf.readfile(folder / f"A{n}-{mode}.dxf")
            close(doc.header["$EXTMIN"][0], 0, 0.0001)
            close(doc.header["$EXTMIN"][1], 0, 0.0001)
            close(doc.header["$EXTMAX"][0], width, 0.0001)
            close(doc.header["$EXTMAX"][1], height, 0.0001)
            points = [p for line in doc.modelspace().query("LINE") for p in [line.dxf.start, line.dxf.end]]
            close(min(p.x for p in points), 10, 0.0001)
            close(min(p.y for p in points), 10, 0.0001)
            close(max(p.x for p in points), width - 10, 0.0001)
            close(max(p.y for p in points), height - 10, 0.0001)
            print(f"PASS A{n} {mode}: PDF and DXF {width} x {height} mm; border 10 mm")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
