"""Check print_modes_scene output independently: python check_print_modes.py OUTDIR."""
from pathlib import Path
import sys

import ezdxf
from ezdxf.colors import DXF_DEFAULT_COLORS, int2rgb
from pypdf import PdfReader

EXPECTED = {
    "colour": [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 127, 127), (127, 255, 127), (127, 127, 255)],
    "grayscale": [(76,) * 3, (150,) * 3, (29,) * 3, (165,) * 3, (202,) * 3, (142,) * 3],
    "black": [(0,) * 3] * 3 + [(127,) * 3] * 3,
}


def main(folder):
    geometry = None
    for mode, wanted in EXPECTED.items():
        reader = PdfReader(folder / f"{mode}.pdf")
        assert len(reader.pages) == 1
        page = reader.pages[0]
        assert abs(float(page.mediabox.width) * 25.4 / 72 - 841) < 0.001
        assert abs(float(page.mediabox.height) * 25.4 / 72 - 1189) < 0.001
        strokes = [tuple(float(v) for v in args) for args, op in page.get_contents().operations if op == b"RG"]
        assert all(rgb == (0, 0, 0) for rgb in strokes[:-6]), "sheet graphics stay black"
        assert len(strokes[-6:]) == 6
        for actual, expected in zip(strokes[-6:], wanted):
            assert all(abs(a - b / 255) <= 0.000501 for a, b in zip(actual, expected)), (mode, actual, expected)
        doc = ezdxf.readfile(folder / f"{mode}.dxf")
        assert doc.dxfversion == "AC1009"
        model = {e.dxf.layer: e for e in doc.modelspace().query("LINE") if e.dxf.layer.startswith("A-C")}
        assert len(model) == 6
        current = []
        for i, rgb in enumerate(wanted):
            line = model[f"A-C{i}_W025"]
            index = line.dxf.color
            if rgb == (0, 0, 0):
                assert index == 7
            else:
                candidates = [j for j in range(1, 256) if j != 7]
                if len(set(rgb)) == 1:
                    candidates = [j for j in candidates if len(set(int2rgb(DXF_DEFAULT_COLORS[j]))) == 1]
                best = min(candidates, key=lambda j: (sum((a-b)**2 for a, b in zip(rgb, int2rgb(DXF_DEFAULT_COLORS[j]))), j))
                assert index == best, (mode, i, index, best)
            assert doc.layers.get(line.dxf.layer).color == index
            if mode != "colour" and index != 7:
                assert len(set(int2rgb(DXF_DEFAULT_COLORS[index]))) == 1, "DXF gray must stay neutral"
            current.append((tuple(line.dxf.start), tuple(line.dxf.end)))
        if geometry is None:
            geometry = current
        assert current == geometry, "mode must not change geometry"
        print(f"PASS {mode}: six PDF/DXF strokes, halftones, black sheet graphics, identical geometry")


if __name__ == "__main__":
    if ezdxf.__version__ != "1.4.4":
        raise RuntimeError("Use ezdxf==1.4.4 for the pinned palette")
    main(Path(sys.argv[1]))
