#!/usr/bin/env python3
"""Rasterize the symbolic BIM sprite into standalone transparent PNG artwork.
Install: python -m pip install cairosvg
Run from anywhere: python export-png.py --size 128
Validate source without dependencies: python export-png.py --check-only
CairoSVG also needs the Cairo runtime on platforms where it is not bundled.
"""
import argparse
from pathlib import Path
import xml.etree.ElementTree as ET

NS = "{http://www.w3.org/2000/svg}"
def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--size", type=int, default=128, help="Physical pixel width/height; default 128")
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parent / "png")
    parser.add_argument("--check-only", action="store_true")
    args = parser.parse_args()
    if not 16 <= args.size <= 2048:
        parser.error("--size must be between 16 and 2048")
    source = Path(__file__).resolve().parent / "symbols.svg"
    root = ET.parse(source).getroot()
    symbols = root.findall(NS + "symbol")
    ids = [s.attrib["id"] for s in symbols]
    if len(symbols) != 96 or len(set(ids)) != 96:
        raise SystemExit("Expected 48 unique icons in two themes")
    light = {i[:-6] for i in ids if i.endswith("-light")}
    dark = {i[:-5] for i in ids if i.endswith("-dark")}
    if len(light) != 48 or light != dark:
        raise SystemExit("Light/dark icon IDs do not match")
    if args.check_only:
        print("Validated 48 matching light/dark pairs")
        return
    try:
        import cairosvg
    except (ImportError, OSError) as exc:
        raise SystemExit("Install CairoSVG and its Cairo runtime: python -m pip install cairosvg\n" + str(exc))
    args.output.mkdir(parents=True, exist_ok=True)
    for symbol in symbols:
        ident = symbol.attrib["id"]
        theme = "dark" if ident.endswith("-dark") else "light"
        name = ident[:-(len(theme) + 1)].replace("-", "_")
        svg = ET.Element(NS + "svg", {"viewBox": "0 0 24 24", "width": "24", "height": "24"})
        for child in symbol:
            svg.append(child)
        target = args.output / (name + (".dark" if theme == "dark" else "") + ".png")
        cairosvg.svg2png(bytestring=ET.tostring(svg), write_to=str(target),
                        output_width=args.size, output_height=args.size)
        if target.read_bytes()[:8] != b"\x89PNG\r\n\x1a\n":
            raise SystemExit("Invalid PNG: " + str(target))
    print(f"Exported 96 transparent {args.size}x{args.size} PNGs to {args.output}")
if __name__ == "__main__":
    main()

