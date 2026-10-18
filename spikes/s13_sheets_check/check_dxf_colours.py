"""Independent DXF colour check using the ezdxf 1.4.4 modelspace palette.

python check_dxf_colours.py make-input OUTDIR
cargo run -p openbim-sheets --release --example colour_scene -- OUTDIR/input.csv OUTDIR
python check_dxf_colours.py check OUTDIR
"""
from itertools import product
from pathlib import Path
import random
import sys

import ezdxf
from ezdxf.colors import DXF_DEFAULT_COLORS, int2rgb
from pypdf import PdfReader


def expected(rgb):
    if rgb == (0, 0, 0):
        return 7
    candidates = (i for i in range(1, 256) if i != 7)
    return min(candidates, key=lambda i: (
        sum((a - b) ** 2 for a, b in zip(rgb, int2rgb(DXF_DEFAULT_COLORS[i]))), i
    ))


def make_input(folder):
    rng = random.Random(4001)
    colours = set(product([0, 32, 64, 96, 128, 160, 192, 255], repeat=3))
    colours.update(tuple(int2rgb(c)) for c in DXF_DEFAULT_COLORS[1:])
    colours.update(tuple(rng.randrange(256) for _ in range(3)) for _ in range(256))
    colours.update([(122, 122, 0), (112, 0, 223), (183, 0, 183)])
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "input.csv").write_text("".join(",".join(map(str, rgb)) + "\n" for rgb in sorted(colours)))
    print(f"Created {len(colours)} RGB cases")


def check_dxf(path, source):
    doc = ezdxf.readfile(path)
    assert doc.dxfversion == "AC1009"
    lines = list(doc.modelspace().query("LINE"))
    assert len(lines) == len(source)
    for line, rgb in zip(lines, source):
        index = expected(rgb)
        assert line.dxf.color == index, (rgb, line.dxf.color, index)
        assert doc.layers.get(line.dxf.layer).color == index
    return doc


def check(folder):
    source = [tuple(map(int, row.split(","))) for row in (folder / "input.csv").read_text().splitlines()]
    doc = check_dxf(folder / "colours.dxf", source)
    doc.saveas(folder / "roundtrip.dxf")
    check_dxf(folder / "roundtrip.dxf", source)
    # PDF retains source RGB; DXF palette quantization must not affect its rendering.
    page = PdfReader(folder / "colours.pdf").pages[0]
    strokes = [tuple(float(v) for v in args) for args, op in page.get_contents().operations if op == b"RG"]
    assert len(strokes) == len(source)
    for actual, rgb in zip(strokes, source):
        assert all(abs(a - b / 255) <= 0.000501 for a, b in zip(actual, rgb)), (actual, rgb)
    print(f"PASS {len(source)} RGB cases: entity/layer ACI, ezdxf save/reopen, original PDF RGB")
    for rgb in [(122, 122, 0), (112, 0, 223), (183, 0, 183)]:
        print(f"PASS team RGB {rgb} -> ACI {expected(rgb)}")


if __name__ == "__main__":
    if ezdxf.__version__ != "1.4.4":
        raise RuntimeError("Use ezdxf==1.4.4 for the pinned reference palette")
    {"make-input": make_input, "check": check}[sys.argv[1]](Path(sys.argv[2]))
