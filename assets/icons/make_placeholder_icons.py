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

def mep_settings():
    # a pipe with a slope tick marks and a gear-like dot: lists of sizes and slopes
    im, d = canvas()
    sheet(d, 20, 22, 88, 84, fill=WHITE, line=BLUE)
    for k in range(4):
        y = 38 + 17 * k
        d.line([30 * S, y * S, 70 * S, y * S], fill=BLUE, width=3 * S)
        d.ellipse([82 * S, (y - 5) * S, 92 * S, (y + 5) * S], fill=ORANGE, outline=DARK, width=S)
    d.line([30 * S, 100 * S, 98 * S, 88 * S], fill=DARK, width=4 * S)
    save(im, "mep_settings")

def system_icon(name, accent):
    # equipment boxes joined by a trunk and branches
    im, d = canvas()
    d.line([64 * S, 20 * S, 64 * S, 108 * S], fill=accent, width=6 * S)
    for y in (30, 64, 98):
        d.line([64 * S, y * S, 36 * S, y * S], fill=accent, width=4 * S)
        d.rectangle([12 * S, (y - 10) * S, 36 * S, (y + 10) * S], fill=LIGHT, outline=DARK, width=3 * S)
    d.rectangle([88 * S, 46 * S, 118 * S, 82 * S], fill=WHITE, outline=BLUE, width=3 * S)
    d.line([64 * S, 64 * S, 88 * S, 64 * S], fill=accent, width=4 * S)
    save(im, name)

def create_system():
    system_icon("create_mech_system", BLUE)

def create_piping_system():
    system_icon("create_pipe_system", ORANGE)

def generate_layout():
    # a dashed trunk with branches: the placeholders of a layout
    im, d = canvas()
    for y0 in range(16, 112, 16):
        d.line([70 * S, y0 * S, 70 * S, (y0 + 9) * S], fill=ORANGE, width=6 * S)
    for y in (28, 64, 100):
        for x0 in range(30, 70, 14):
            d.line([x0 * S, y * S, (x0 + 8) * S, y * S], fill=ORANGE, width=4 * S)
        d.rectangle([8 * S, (y - 9) * S, 26 * S, (y + 9) * S], fill=LIGHT, outline=DARK, width=3 * S)
    d.rectangle([96 * S, 50 * S, 120 * S, 78 * S], fill=WHITE, outline=BLUE, width=3 * S)
    save(im, "generate_layout")

def system_browser():
    im, d = canvas()
    sheet(d, 14, 16, 100, 96, fill=WHITE, line=BLUE)
    for k, y in enumerate((30, 56, 82)):
        d.rectangle([24 * S, y * S, 40 * S, (y + 14) * S], fill=LIGHT, outline=DARK, width=2 * S)
        d.line([48 * S, (y + 7) * S, 100 * S, (y + 7) * S], fill=BLUE if k else ORANGE, width=4 * S)
    save(im, "system_browser")

def view_range():
    # a side view of a building with the top, cut plane and bottom lines of a plan
    im, d = canvas()
    d.rectangle([34 * S, 22 * S, 70 * S, 112 * S], fill=LIGHT, outline=DARK, width=4 * S)
    for y, c, w in ((30, BLUE, 3), (62, ORANGE, 5), (96, BLUE, 3)):
        d.line([10 * S, y * S, 118 * S, y * S], fill=c, width=w * S)
    d.polygon([(100 * S, 30 * S), (112 * S, 44 * S), (88 * S, 44 * S)], fill=DARK)
    d.polygon([(100 * S, 96 * S), (112 * S, 82 * S), (88 * S, 82 * S)], fill=DARK)
    save(im, "view_range")

for f in (paste, cut, match_type, pin, select_instances, create_similar, mep_settings, create_system, create_piping_system, generate_layout, system_browser, view_range): f()
pin(slash=True)
