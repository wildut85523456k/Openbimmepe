"""Placeholder ribbon icons for commands the approved 3D package has no artwork for yet (same palette as the
Offset placeholder). Replace each with approved art when it exists.  Run: python3 make_placeholder_icons.py"""
from PIL import Image, ImageDraw
import os
OUT = os.path.join(os.path.dirname(__file__), "rust")
BLUE, LIGHT, DARK, ORANGE, RED, WHITE = (70, 110, 150, 255), (130, 160, 190, 255), (40, 70, 100, 255), (220, 140, 0, 255), (190, 40, 40, 255), (245, 245, 245, 255)
S = 4  # draw 4x and shrink for smooth edges

def canvas():
    im = Image.new("RGBA", (128 * S, 128 * S), (0, 0, 0, 0))
    return im, ImageDraw.Draw(im)

def save(im, name):
    im.resize((128, 128), Image.LANCZOS).save(os.path.join(OUT, name + ".png"))

def sheet(d, x, y, w, h, fill=WHITE, line=BLUE):
    d.rectangle([x * S, y * S, (x + w) * S, (y + h) * S], fill=fill, outline=line, width=4 * S // 2)

def paste():
    im, d = canvas()
    sheet(d, 26, 30, 62, 80, fill=LIGHT, line=DARK)
    d.rounded_rectangle([46 * S, 20 * S, 68 * S, 40 * S], radius=4 * S, fill=ORANGE, outline=DARK, width=2 * S)
    sheet(d, 52, 52, 56, 62)
    for k in range(3): d.line([62 * S, (68 + 12 * k) * S, 98 * S, (68 + 12 * k) * S], fill=BLUE, width=3 * S)
    save(im, "paste")

def cut():
    im, d = canvas()
    d.line([40 * S, 100 * S, 84 * S, 20 * S], fill=DARK, width=7 * S)
    d.line([88 * S, 100 * S, 44 * S, 20 * S], fill=BLUE, width=7 * S)
    for cx in (40, 88): d.ellipse([(cx - 14) * S, 92 * S, (cx + 14) * S, 120 * S], outline=ORANGE, width=6 * S)
    save(im, "cut")

def match_type():
    im, d = canvas()
    sheet(d, 14, 24, 44, 56, fill=LIGHT, line=DARK)
    sheet(d, 70, 48, 44, 56)
    d.line([58 * S, 52 * S, 70 * S, 70 * S], fill=ORANGE, width=6 * S)
    d.polygon([(72 * S, 76 * S), (58 * S, 70 * S), (68 * S, 60 * S)], fill=ORANGE)
    d.rectangle([28 * S, 38 * S, 44 * S, 66 * S], fill=BLUE)
    save(im, "match_type")

def pin(color=BLUE, slash=False):
    im, d = canvas()
    d.ellipse([40 * S, 14 * S, 88 * S, 62 * S], fill=ORANGE, outline=DARK, width=4 * S)
    d.polygon([(52 * S, 56 * S), (76 * S, 56 * S), (64 * S, 84 * S)], fill=color)
    d.line([64 * S, 84 * S, 64 * S, 116 * S], fill=DARK, width=5 * S)
    if slash:
        d.line([22 * S, 112 * S, 106 * S, 20 * S], fill=RED, width=8 * S)
    save(im, "unpin" if slash else "pin")

def select_instances():
    im, d = canvas()
    for i, (x, y) in enumerate([(12, 14), (72, 14), (12, 74), (72, 74)]):
        d.rectangle([x * S, y * S, (x + 42) * S, (y + 42) * S], fill=LIGHT, outline=DARK, width=4 * S)
        d.rectangle([(x + 8) * S, (y + 8) * S, (x + 34) * S, (y + 34) * S], fill=BLUE)
    d.rectangle([4 * S, 6 * S, 124 * S, 124 * S], outline=ORANGE, width=3 * S)
    save(im, "select_instances")

def create_similar():
    im, d = canvas()
    sheet(d, 14, 40, 56, 56, fill=LIGHT, line=DARK)
    d.rectangle([26 * S, 52 * S, 58 * S, 84 * S], fill=BLUE)
    d.rectangle([60 * S, 24 * S, 112 * S, 76 * S], outline=ORANGE, width=4 * S)
    d.line([86 * S, 34 * S, 86 * S, 66 * S], fill=ORANGE, width=5 * S)
    d.line([70 * S, 50 * S, 102 * S, 50 * S], fill=ORANGE, width=5 * S)
    save(im, "create_similar")

for f in (paste, cut, match_type, pin, select_instances, create_similar): f()
pin(slash=True)
