#!/usr/bin/env python3
"""Draw the Import IFC / Export IFC icons in the 3D style of the approved catalog.

The catalog's own IFC icons resemble the buildingSMART logo (a trademark), so they are not used.
These are original drawings: a brushed-metal page, a glossy blue model cube and a blue arrow
(into the page = import, out of it = export). Output: assets/icons/rust/ifc_import.png, ifc_export.png (128 px, RGBA).
Run from the repository root:  python3 assets/icons/make_ifc_icons.py
"""
import sys
from pathlib import Path
import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFilter

OUT = Path(__file__).resolve().parent / "rust"
SS, N = 4, 128          # supersampling and final size
W = N * SS


def P(x, y):  # design coordinates are 0..128
    return (x * SS, y * SS)


def vgrad(box, top, bottom):
    x0, y0, x1, y1 = [int(v) for v in box]
    h = max(1, y1 - y0)
    t = np.linspace(0, 1, h)[:, None, None]
    col = np.array(top, np.float32) * (1 - t) + np.array(bottom, np.float32) * t
    arr = np.repeat(col, max(1, x1 - x0), axis=1).astype(np.uint8)
    return Image.fromarray(arr, "RGB")


def hgrad(box, left, right):
    x0, y0, x1, y1 = [int(v) for v in box]
    w = max(1, x1 - x0)
    t = np.linspace(0, 1, w)[None, :, None]
    col = np.array(left, np.float32) * (1 - t) + np.array(right, np.float32) * t
    arr = np.repeat(col, max(1, y1 - y0), axis=0).astype(np.uint8)
    return Image.fromarray(arr, "RGB")


def fill_poly(canvas, pts, grad, outline=None, width=1.2):
    mask = Image.new("L", canvas.size, 0)
    ImageDraw.Draw(mask).polygon(pts, fill=255)
    canvas.paste(grad, (0, 0), ImageChops.multiply(mask, Image.new("L", canvas.size, 255)))
    if outline:
        ImageDraw.Draw(canvas).line(pts + [pts[0]], fill=outline, width=int(width * SS), joint="curve")
    return mask


def full(grad_fn, *args):
    return grad_fn((0, 0, W, W), *args)


def brushed(canvas, mask, strength=7):
    rng = np.random.default_rng(7)
    noise = rng.normal(0, 1, (W, W)).astype(np.float32)
    img = Image.fromarray(((noise * 40) + 128).clip(0, 255).astype(np.uint8), "L").filter(ImageFilter.GaussianBlur((10, 0.6)))
    n = (np.asarray(img).astype(np.float32) - 128) / 40 * strength
    a = np.asarray(canvas.convert("RGB")).astype(np.float32) + n[..., None]
    m = np.asarray(mask).astype(np.float32)[..., None] / 255
    base = np.asarray(canvas.convert("RGB")).astype(np.float32)
    canvas.paste(Image.fromarray((base * (1 - m) + a * m).clip(0, 255).astype(np.uint8), "RGB"))


def draw_icon(export: bool) -> Image.Image:
    im = Image.new("RGB", (W, W), (255, 255, 255))
    alpha = Image.new("L", (W, W), 0)

    # --- page: brushed metal, folded corner
    page = [P(22, 8), P(80, 8), P(100, 28), P(100, 108), P(22, 108)]
    g = hgrad((22 * SS, 8 * SS, 100 * SS, 108 * SS), (236, 239, 243), (176, 183, 194))
    full_g = Image.new("RGB", (W, W)); full_g.paste(g, (22 * SS, 8 * SS))
    pm = fill_poly(im, page, full_g, outline=(112, 122, 136), width=1.4)
    brushed(im, pm)
    fold = [P(80, 8), P(80, 28), P(100, 28)]
    fg = Image.new("RGB", (W, W)); fg.paste(vgrad((80 * SS, 8 * SS, 100 * SS, 28 * SS), (250, 251, 253), (190, 196, 206)), (80 * SS, 8 * SS))
    fm = fill_poly(im, fold, fg, outline=(112, 122, 136), width=1.2)
    d = ImageDraw.Draw(im)
    d.line([P(24, 10), P(79, 10)], fill=(255, 255, 255), width=SS)  # top highlight
    alpha = ImageChops.lighter(alpha, pm)

    # --- glossy blue model cube
    cx, cy, r = 56, 52, 24
    k = 0.866
    top = [P(cx, cy - r), P(cx + r * k, cy - r / 2), P(cx, cy), P(cx - r * k, cy - r / 2)]
    left = [P(cx - r * k, cy - r / 2), P(cx, cy), P(cx, cy + r), P(cx - r * k, cy + r / 2)]
    right = [P(cx + r * k, cy - r / 2), P(cx, cy), P(cx, cy + r), P(cx + r * k, cy + r / 2)]
    ol = (16, 62, 140)
    for pts, c1, c2 in ((left, (66, 150, 245), (28, 100, 214)), (right, (30, 100, 205), (16, 64, 160)), (top, (150, 205, 255), (76, 160, 248))):
        gg = Image.new("RGB", (W, W)); gg.paste(vgrad((0, int((cy - r) * SS), W, int((cy + r) * SS)), c1, c2), (0, int((cy - r) * SS)))
        fill_poly(im, pts, gg, outline=ol, width=1.0)
    d = ImageDraw.Draw(im)
    d.line([P(cx - r * k + 2, cy - r / 2 + 2), P(cx - 1, cy - 1)], fill=(190, 225, 255), width=SS)  # gloss on the left edge

    # --- blue arrow, bottom right (out of the page = export, into it = import)
    if export:   # points right, leaving the page
        arrow = [P(52, 90), P(90, 90), P(90, 78), P(124, 100), P(90, 122), P(90, 110), P(52, 110)]
    else:        # points left, entering the page
        arrow = [P(124, 90), P(86, 90), P(86, 78), P(52, 100), P(86, 122), P(86, 110), P(124, 110)]
    shadow = Image.new("L", (W, W), 0)
    ImageDraw.Draw(shadow).polygon([(x + 2 * SS, y + 3 * SS) for x, y in arrow], fill=110)
    shadow = shadow.filter(ImageFilter.GaussianBlur(2.2 * SS))
    am = Image.new("L", (W, W), 0); ImageDraw.Draw(am).polygon(arrow, fill=255)
    # soft shadow only where the arrow isn't
    sh_rgb = Image.new("RGB", (W, W), (40, 50, 70))
    im.paste(sh_rgb, (0, 0), ImageChops.subtract(shadow, am))
    ag = Image.new("RGB", (W, W)); ag.paste(vgrad((0, 78 * SS, W, 122 * SS), (120, 190, 255), (22, 84, 190)), (0, 78 * SS))
    fill_poly(im, arrow, ag, outline=(12, 52, 130), width=1.3)
    d = ImageDraw.Draw(im)
    hx0, hx1 = (54, 88) if export else (88, 122)
    d.line([P(hx0, 92.5), P(hx1, 92.5)], fill=(200, 230, 255), width=SS)  # gloss on the shaft
    alpha = ImageChops.lighter(alpha, ImageChops.lighter(am, shadow))

    # compose RGBA, soft drop shadow under the page
    sh = Image.new("L", (W, W), 0)
    ImageDraw.Draw(sh).polygon([(x + 2.5 * SS, y + 3.5 * SS) for x, y in page], fill=90)
    sh = sh.filter(ImageFilter.GaussianBlur(2.5 * SS))
    base = Image.new("RGBA", (W, W), (30, 40, 60, 0)); base.putalpha(sh)
    top_layer = im.convert("RGBA"); top_layer.putalpha(ImageChops.lighter(alpha, Image.new("L", (W, W), 0)))
    base.alpha_composite(top_layer)
    return base.resize((N, N), Image.LANCZOS)


def main() -> int:
    OUT.mkdir(exist_ok=True)
    draw_icon(True).save(OUT / "ifc_export.png", optimize=True)
    draw_icon(False).save(OUT / "ifc_import.png", optimize=True)
    print("wrote ifc_export.png and ifc_import.png")
    return 0


if __name__ == "__main__":
    sys.exit(main())
