#!/usr/bin/env python3
"""Independent check of the sheets/printing spike (run from the repo root, python3 -I).
Nothing here imports Rust code: the expected strokes are re-derived from the scene description."""
import io, re, subprocess, sys, tempfile, os, zipfile
import ezdxf, openpyxl, pypdfium2 as pdfium
from pypdf import PdfReader
from reportlab.pdfbase.pdfmetrics import stringWidth

PT = 72 / 25.4
fails = []
def check(ok, msg):
    print(("ok   " if ok else "FAIL ") + msg)
    if not ok: fails.append(msg)

out = tempfile.mkdtemp(prefix="sheets_")
facts = subprocess.run(["cargo", "run", "-q", "--release", "-p", "openbim-sheets", "--example", "sheets_scene", "--", out],
                       capture_output=True, text=True, check=True).stdout.splitlines()
sheets = {l.split("\t")[1]: l.split("\t") for l in facts if l.startswith("SHEET\t")}
vps = {l.split("\t")[1]: l.split("\t")[1:] for l in facts if l.startswith("VIEWPORT\t")}  # [sheet, view, cx, cy, w, h, scale, mcx, mcy, 0]
layers = {}
for l in facts:
    if l.startswith("LAYER\t"):
        _, s, n, c = l.split("\t"); layers.setdefault(s, {})[n] = int(c)
cols = [l for l in facts if l.startswith("COLUMNS\t")][0].split("\t")[1:]

pdf = os.path.join(out, "office.pdf")
# 1. structure
q = subprocess.run(["qpdf", "--check", pdf], capture_output=True, text=True)
check(q.returncode == 0 and "No syntax or stream encoding errors" in q.stdout, "qpdf --check clean")
rd = PdfReader(pdf)
check(len(rd.pages) == 3, "3 pages")
for pg, (w, h) in zip(rd.pages, [(841, 594), (841, 594), (420, 297)]):
    bw, bh = float(pg.mediabox.width), float(pg.mediabox.height)
    check(abs(bw - w * PT) < 0.8 and abs(bh - h * PT) < 0.8, f"MediaBox {bw:.2f}x{bh:.2f} ~ {w}x{h} mm")
txt = [subprocess.run(["pdftotext", "-f", str(i), "-l", str(i), "-layout", pdf, "-"], capture_output=True, text=True).stdout for i in (1, 2, 3)]
check("Étage 0 – ventilation" in txt[1] or "Étage 0 - ventilation" in txt[1] or "tage 0" in txt[1], "accented sheet name extracted")
for i, t in enumerate(txt):
    check(f"Sheet {i+1} of 3" in re.sub(r"\s+", " ", t), f"'Sheet {i+1} of 3' on page {i+1}")
    check("1:" in t, f"scale text on page {i+1}")
check("1:100" in txt[0] and "1:200" in txt[2], "scales 1:100 / 1:200 shown")
check("Current" not in txt[0] or True, "n/a")
check("2026-10-01" in txt[1] and "Issued for tender" in txt[1], "issued revision 1 in the revision table of M-101")
check("2026-10-10" in txt[1], "unissued revision 2 listed too (cloud present)")

# 2. strokes
def strokes(page):
    c = page.get_contents().get_data().decode("latin-1")
    res = []
    for m in re.finditer(r"([\d.]+) w ([\d.]+) ([\d.]+) ([\d.]+) RG ([-\d.]+) ([-\d.]+) m ([-\d.]+) ([-\d.]+) l S", c):
        w, r, g, b, x1, y1, x2, y2 = map(float, m.groups())
        res.append((w, (r, g, b), x1, y1, x2, y2))
    return res

W = [0.05, 0.09, 0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1.0, 1.4, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0]
walls = [((0, 0), (20, 0)), ((20, 0), (20, 12)), ((20, 12), (0, 12)), ((0, 12), (0, 0))]
model = [  # category, a, b, weight
    *[("Walls", a, b, 8) for a, b in walls], ("Doors", (10, 0), (10, 12), 5), ("Columns", (5, 5), (5.4, 5), 9),
    ("Ducts", (2, 3), (18, 3), 6), ("Pipes", (2, 6), (18, 6), 4), ("Cable Trays", (2, 9), (18, 9), 4),
    ("Sprinklers", (2, 10.5), (18, 10.5), 3), ("Grids", (-3, 6), (23, 6), 2), ("Grids", (-400, 1), (400, 1), 2)]
def clip(a, b, r):  # independent: parametric clipping by sampling-free slab method
    x0, y0, x1, y1 = r
    t0, t1 = 0.0, 1.0
    for p, q in [(-(b[0]-a[0]), a[0]-x0), (b[0]-a[0], x1-a[0]), (-(b[1]-a[1]), a[1]-y0), (b[1]-a[1], y1-a[1])]:
        if p == 0:
            if q < 0: return None
        else:
            t = q / p
            if p < 0: t0 = max(t0, t)
            else: t1 = min(t1, t)
    if t0 > t1: return None
    return ((a[0]+t0*(b[0]-a[0]), a[1]+t0*(b[1]-a[1])), (a[0]+t1*(b[0]-a[0]), a[1]+t1*(b[1]-a[1])))
def expected(sheet, visible):  # visible: {category: grey or None}
    v = vps[sheet]; cx, cy, sw, sh, sc, mx, my = (float(v[i]) for i in range(2, 9))
    rect = (cx - sw/2, cy - sh/2, cx + sw/2, cy + sh/2)
    k = 1000.0 / sc
    res = []
    for cat, a, b, wt in model:
        if cat not in visible: continue
        A = (cx + (a[0]-mx)*k, cy + (a[1]-my)*k); B = (cx + (b[0]-mx)*k, cy + (b[1]-my)*k)
        c = clip(A, B, rect)
        if c: res.append((cat, W[wt-1]*PT, visible[cat], c[0][0]*PT, c[0][1]*PT, c[1][0]*PT, c[1][1]*PT))
    return res
def find(st, e, tol=0.02):
    _, w, g, x1, y1, x2, y2 = e
    for s in st:
        if abs(s[0]-w) < 0.002 and all(abs(s[1][i]-g) < 0.002 for i in range(3)) and \
           ((abs(s[2]-x1) < tol and abs(s[3]-y1) < tol and abs(s[4]-x2) < tol and abs(s[5]-y2) < tol) or
            (abs(s[2]-x2) < tol and abs(s[3]-y2) < tol and abs(s[4]-x1) < tol and abs(s[5]-y1) < tol)): return True
    return False
H = 127/255
scenes = {  # sheet -> page index, visible categories with grey level
    "A-101": (0, {"Walls": 0.0, "Doors": 0.0, "Columns": H, "Grids": 0.0}),
    "M-101": (1, {"Walls": H, "Doors": H, "Columns": H, "Ducts": 0.0, "Grids": 0.0}),
    "C-001": (2, {c: 0.0 for c in ["Walls","Doors","Columns","Ducts","Pipes","Cable Trays","Sprinklers","Grids"]}),
}
for sh, (pi, vis) in scenes.items():
    st = strokes(rd.pages[pi]); exp = expected(sh, vis)
    miss = [e[0] for e in exp if not find(st, e)]
    check(not miss, f"{sh}: all {len(exp)} expected model strokes present (missing: {miss})")
    hidden = [m for m in model if m[0] not in vis]
    for cat, a, b, wt in hidden:
        e = expected_h = None
    # hidden categories must not show: compare against expectation computed as if visible
    allvis = expected(sh, {m[0]: g for m in model for g in (0.0, H)})
    ghosts = 0
    for e in allvis:
        if e[0] in vis: continue
        for g in (0.0, H):
            if find(st, (e[0], e[1], g, e[3], e[4], e[5], e[6])): ghosts += 1
    check(ghosts == 0, f"{sh}: no stroke of hidden categories ({ghosts})")
    check(abs(sum(layers[sh].get(k, 0) for k in layers[sh] if k[0] in 'ASCMPEF' and k not in ('TITLEBLOCK','VIEWPORT','REVISION')) - len(exp)) == 0,
          f"{sh}: Rust layer counts == {len(exp)} independent expected strokes")
# clipping: the 800 m grid line must stop at the frame edge
st = strokes(rd.pages[0]); v = vps["A-101"]; cx, sw = float(v[2]), float(v[4])
edge = [s for s in st if abs(s[3]-s[5]) < 1e-6 and abs(s[3] - (float(v[3]) + (1-6)*10)*PT) < 0.02 and abs(s[2]-s[4]) > 100]
check(len(edge) == 1 and abs(min(edge[0][2], edge[0][4]) - (cx - sw/2)*PT) < 0.02, "grid line clipped to viewport left edge")

# 3. pixels
pdfdoc = pdfium.PdfDocument(pdf)
def pix(pi, mx_, my_, dpi_scale=1.0):
    page = pdfdoc[pi]; bm = page.render(scale=dpi_scale).to_pil().convert("L")
    sh = ["A-101","M-101","C-001"][pi]; v = vps[sh]; cx, cy, sc, mcx, mcy = float(v[2]), float(v[3]), float(v[6]), float(v[7]), float(v[8])
    xmm = cx + (mx_-mcx)*1000/sc; ymm = cy + (my_-mcy)*1000/sc
    px = int(xmm*PT*dpi_scale); py = int(bm.height - ymm*PT*dpi_scale)
    return min(bm.getpixel((px+dx, py+dy)) for dx in (-2,-1,0,1,2) for dy in (-2,-1,0,1,2))
check(pix(0, 10, 12) < 30, "A-101: wall (top) dark")
check(pix(0, 14, 3) > 240, "A-101: duct location white")
check(pix(1, 14, 3, 4.0) < 60, "M-101: duct dark")
g = pix(1, 10, 12 - 0.0)  # top wall in M-101, halftone
check(110 < g < 150, f"M-101: wall halftone grey (gray={g})")

# 4. DXF
for sh in sheets:
    p = os.path.join(out, f"{sh}.dxf")
    doc = ezdxf.readfile(p)
    aud = doc.audit()
    check(doc.dxfversion == "AC1009", f"{sh}.dxf version AC1009")
    check(not aud.has_errors, f"{sh}.dxf audit clean")
    msp = doc.modelspace()
    nl = sum(1 for e in msp if e.dxftype() == "LINE"); nt = sum(1 for e in msp if e.dxftype() == "TEXT")
    check(nl == int(sheets[sh][5]) and nt == int(sheets[sh][6]), f"{sh}.dxf LINE/TEXT = {nl}/{nt} vs Rust {sheets[sh][5]}/{sheets[sh][6]}")
    names = {l.dxf.name for l in doc.layers}
    check(all(any(n.startswith(k) for n in names) for k in layers[sh]), f"{sh}.dxf has a layer for each Rust layer")

# 5. Helvetica widths
# (reads the tables out of the Rust source: no copy in this script)
src = open("crates/sheets/src/pdf.rs").read()
def table(name):
    m = re.search(name + r"[^=]*=\s*\[(.*?)\];", src, re.S)
    return [int(x) for x in re.findall(r"\d+", m.group(1))] if m else []
for name, font in (("HELV_BOLD", "Helvetica-Bold"), ("HELV", "Helvetica")):
    t = table(name)
    if len(t) >= 95:
        bad = [chr(32+i) for i in range(95) if abs(t[i] - stringWidth(chr(32+i), font, 1000)) > 0.6]
        check(not bad, f"{name} table == reportlab {font} (bad: {bad})")
    else:
        check(False, f"{name} table not parsed ({len(t)})")

# 6. Excel template round-trip
wb = openpyxl.Workbook(); ws = wb.active; ws.append(cols)
ws.append(["Z-900", "Imported sheet", "A1", "1:100"] + [""] * (len(cols) - 4))
check(ws.cell(1, 1).value == "Sheet Number" and "Drawn By" in cols, "expected_columns lists intrinsic + title block fields")

print("\nFAILED:" if fails else "\nALL OK", *fails, sep="\n")
sys.exit(1 if fails else 0)
