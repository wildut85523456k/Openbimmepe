#!/usr/bin/env python3
"""Derive the egui desktop icon set from the approved 3D catalog PNGs.

The approved PNGs (assets/icons/approved-3d/) sit on an opaque near-white background. The egui app
also has a dark theme, so this script removes that background (flood fill from the image border,
soft edge) and writes RGBA PNGs to assets/icons/rust/. The approved files are never modified.
Run from the repository root:  python3 assets/icons/make_rust_icons.py
IFC import/export are redrawn separately by make_ifc_icons.py and are skipped here.
"""
import sys
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent
SRC, DST = ROOT / "approved-3d", ROOT / "rust"
SKIP = {"ifc_import", "ifc_export"}
LINE_ART = {"grid", "level", "tag", "dimension", "section", "elevation", "detail"}  # dark-theme variants
SENTINEL = (255, 0, 255)
THRESH = 22  # colour distance from the corner pixel still counted as background


def cut_background(path: Path) -> Image.Image:
    im = Image.open(path).convert("RGB")
    w, h = im.size
    work = im.copy()
    for x in range(w):
        for y in (0, h - 1):
            if work.getpixel((x, y)) != SENTINEL:
                ImageDraw.floodfill(work, (x, y), SENTINEL, thresh=THRESH)
    for y in range(h):
        for x in (0, w - 1):
            if work.getpixel((x, y)) != SENTINEL:
                ImageDraw.floodfill(work, (x, y), SENTINEL, thresh=THRESH)
    a = np.asarray(work)
    bg = (a[..., 0] == 255) & (a[..., 1] == 0) & (a[..., 2] == 255)
    rgb = np.asarray(im).astype(np.float32)
    alpha = np.where(bg, 0.0, 1.0).astype(np.float32)
    # Soft edge: pixels touching the background are partly transparent, with white removed from their colour.
    pad = np.pad(bg, 1, constant_values=True)
    near = np.zeros_like(bg)
    for dy in (-1, 0, 1):
        for dx in (-1, 0, 1):
            near |= pad[1 + dy:1 + dy + h, 1 + dx:1 + dx + w]
    fringe = near & ~bg
    whiteness = rgb.min(axis=2) / 255.0
    fa = np.clip((1.0 - whiteness) * 2.5, 0.25, 1.0)
    alpha = np.where(fringe, fa, alpha)
    a3 = np.maximum(alpha[..., None], 1e-3)
    unmix = np.clip((rgb - (1 - a3) * 255.0) / a3, 0, 255)
    rgb = np.where(fringe[..., None], unmix, rgb)
    out = np.dstack([rgb, alpha * 255.0]).round().astype(np.uint8)
    return Image.fromarray(out, "RGBA")


def dark_variant(im: Image.Image) -> Image.Image:
    """Line drawings: dark navy strokes become light, white fills become transparent; blue stays blue."""
    a = np.asarray(im).astype(np.float32)
    rgb, alpha = a[..., :3], a[..., 3].copy()
    lum = rgb @ np.array([0.299, 0.587, 0.114], dtype=np.float32)
    t = np.clip((150.0 - lum) / 150.0, 0, 1)[..., None]
    rgb = rgb * (1 - t) + np.array([226, 233, 246], dtype=np.float32) * t
    alpha = np.where(lum > 238, 0.0, alpha)
    return Image.fromarray(np.dstack([rgb, alpha]).round().astype(np.uint8), "RGBA")


def main() -> int:
    DST.mkdir(exist_ok=True)
    n = 0
    for p in sorted(SRC.glob("*.png")):
        if p.stem in SKIP:
            continue
        light = cut_background(p)
        light.save(DST / p.name, optimize=True)
        n += 1
        if p.stem in LINE_ART:
            dark_variant(light).save(DST / f"{p.stem}.dark.png", optimize=True)
    print(f"wrote {n} icons to {DST}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
