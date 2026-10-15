#!/usr/bin/env python3
"""Package versioned catalog and generated 3D command artwork into Qt resources.
Legacy schematic SVGs are retained only for optional incomplete development builds.
Usage: python3 assets/icons/generate.py --output build/icons
"""
import argparse
import json
import shutil
from pathlib import Path
from approved_assets import load
import xml.etree.ElementTree as ET
from isometric import SUPPORTED as ISOMETRIC_SUPPORTED, svg as isometric_svg

ROOT = Path(__file__).resolve().parent
# Individual technical motifs; all coordinates are drawn on a 24x24 grid.
# Each icon has a distinct geometry or distinct equipment/detail marking.
SHAPES = {
"select": '<path d="M4 2 7 20l4-5 4 7 3-2-4-7 7-1Z"/>',
"save": '<path d="M4 3h14l3 3v15H3V3h2m3 0v7h9V3M7 21v-8h10v8"/>',
"open": '<path d="M3 7V4h7l2 3h9v12H3L1 9h17l-3 10"/>',
"new": '<path d="M5 2h10l4 4v16H5zM15 2v5h4M8 14h8M12 10v8"/>',
"undo": '<path d="M9 6 3 12l6 6M3 12h11a6 6 0 1 1 0 12" transform="translate(0 -3)"/>',
"redo": '<path d="m15 6 6 6-6 6m6-6H10a6 6 0 1 0 0 12" transform="translate(0 -3)"/>',
"search": '<circle cx="10" cy="10" r="6"/><path d="m15 15 7 7"/>',
"settings": '<circle cx="12" cy="12" r="3"/><path d="m10 2-1 3-3 1-3-1-2 4 3 2v3l-3 2 2 4 3-1 3 1 1 3h4l1-3 3-1 3 1 2-4-3-2v-3l3-2-2-4-3 1-3-1-1-3z"/>',
"zoom_in": '<circle cx="10" cy="10" r="6"/><path d="M7 10h6m-3-3v6m5 2 7 7"/>',
"zoom_out": '<circle cx="10" cy="10" r="6"/><path d="M7 10h6m2 5 7 7"/>',
"pan": '<path d="M6 20V11a2 2 0 0 1 3 0V4a2 2 0 0 1 4 0v7-3a2 2 0 0 1 4 0v3a2 2 0 0 1 3 1v4l-3 6H9Z"/>',
"orbit": '<circle cx="12" cy="12" r="5"/><ellipse cx="12" cy="12" rx="11" ry="5" transform="rotate(-25 12 12)"/><path d="m18 4 3-1-1 4"/>',
"copy": '<rect x="7" y="7" width="14" height="14" rx="1"/><path d="M4 17H2V3h14v2"/>',
"move": '<path d="M12 2v20M2 12h20M9 5l3-3 3 3m-6 14 3 3 3-3M5 9l-3 3 3 3m14-6 3 3-3 3"/>',
"rotate": '<path d="M19 9a8 8 0 1 0 1 6m0-12v6h-6"/><circle cx="12" cy="13" r="2"/>',
"mirror": '<path d="M12 2v20" stroke-dasharray="2 2"/><path d="M10 5 3 12l7 7zM14 5l7 7-7 7z"/>',
"align": '<path d="M3 2v20M6 7h15M6 12h10M6 17h12"/>',
"trim": '<path d="M4 2v20M2 8h20m-4 4 4 4M3 3l5 5" stroke-linecap="round"/>',
"delete": '<path d="M4 6h16M9 3h6m-9 3 1 16h10l1-16M10 10v8m4-8v8"/>',
"measure": '<path d="M2 17 17 2l5 5L7 22zM13 6l3 3m-8 2 3 3m-8 2 3 3"/>',
"wall": '<rect x="3" y="5" width="18" height="14"/><path d="M3 12h18M9 5v7m6 0v7" />',
"door": '<path d="M4 21V3h16v18M4 21h16M4 5h13v13M4 18a14 14 0 0 0 14-14" />',
"window": '<rect x="3" y="3" width="18" height="18"/><path d="M12 3v18M3 12h18M6 6l4 4m4 4 4 4"/>',
"floor": '<path d="M2 16h20v5H2zM2 12l10-8 10 8m-10-8v12"/>',
"roof": '<path d="m2 14 10-10 10 10M4 14v7h16v-7M9 20v-5h6v5"/>',
"stair": '<path d="M2 21v-5h6v-5h6V6h8M3 3l5 5m-5 0 5-5" />',
"railing": '<path d="M2 7h20M2 20h20M5 7v13m7-13v13m7-13v13"/>',
"ramp": '<path d="M2 20 21 5v15H2Zm4-5h6m-3-3 3 3-3 3"/>',
"curtain_wall": '<rect x="3" y="3" width="18" height="18"/><path d="M9 3v18m6-18v18M3 10h18m-18 6h18"/>',
"ceiling": '<rect x="2" y="3" width="20" height="18"/><path d="M2 9h20M2 15h20M8 3v18m8-18v18"/>',
"room": '<path d="M3 21V3h18v18zM9 3v7h7v11"/><text x="5" y="19" font-size="6" stroke="none" fill="currentColor">101</text>',
"area": '<path d="M3 4h18v16H3z" stroke-dasharray="3 2"/><path d="M8 15 12 8l4 7m-6-3h4"/>',
"opening": '<path d="M3 3h18v18H3zM8 3v18m8-18v18M8 8h8v8H8z"/>',
"component": '<path d="m12 2 9 5v10l-9 5-9-5V7zM3 7l9 5 9-5m-9 5v10"/>',
"model_line": '<path d="M3 20 21 4M3 20l8-2-6-6  -2 8" />',
"column": '<path d="M8 3h8v18H8zM4 3h16M4 21h16m-7-18v18"/>',
"beam": '<path d="M3 7h18v10H3zM8 7v10m8-10v10M3 11h18"/>',
"brace": '<rect x="4" y="4" width="16" height="16"/><path d="M4 20 20 4M4 4l16 16"/>',
"foundation": '<path d="M9 2h6v13H9zM3 15h18v6H3zM2 21h20"/>',
"structural_floor": '<path d="M2 9h20v5H2zM2 18h20M4 14v4m4-4v4m4-4v4m4-4v4m4-4v4"/>',
"rebar": '<path d="M3 20V4h18v16M6 17h12M6 12h12M6 7h12M9 4v16m6-16v16"/>',
"connection": '<path d="M3 3h7v18H3zM10 12h11v9H10zM7 9h10M7 15h10"/><circle cx="14" cy="15" r="1"/>',
"truss": '<path d="M2 20 12 3l10 17H2ZM7 12h10m-10 0 5 8 5-8"/>',
"footing": '<path d="M9 2h6v11H9zM4 13h16l2 7H2zM2 22h20"/>',
"anchor": '<path d="M12 2v15M7 8h10m-13 5a8 8 0 0 0 16 0m-18 0h5m10 0h5"/>',
"duct": '<rect x="2" y="8" width="20" height="8" rx="1"/><path d="M4 12h16" stroke-dasharray="2 2"/>',
"duct_fitting": '<path d="M4 3h7v10h10v7H4zM11 6a8 8 0 0 0 8 8"/>',
"duct_elbow": '<path d="M3 3h9v9h9v9H3zM12 3a9 9 0 0 0 9 9"/>',
"duct_tee": '<path d="M2 9h20v6H2zM9 15h6v7H9zM12 9v13"/>',
"duct_transition": '<path d="M2 6h6l8 5h6v3h-6l-8 5H2zM3 12h18" stroke-dasharray="3 2"/>',
"flex_duct": '<path d="M2 17C7 2 11 21 16 7s5 2 6 2M3 19l2-4m2 7 2-5m2 4 2-5"/>',
"mechanical_equipment": '<rect x="3" y="3" width="18" height="18"/><path d="M3 3 21 21M21 3 3 21"/><circle cx="12" cy="12" r="4"/>',
"air_terminal": '<rect x="3" y="3" width="18" height="18"/><path d="M3 3 21 21M21 3 3 21M7 12h10M12 7v10"/>',
"diffuser": '<rect x="3" y="3" width="18" height="18"/><rect x="7" y="7" width="10" height="10"/><path d="M12 2v4M12 18v4M2 12h4m12 0h4"/>',
"ahu": '<rect x="2" y="4" width="20" height="16"/><circle cx="9" cy="12" r="5"/><path d="m9 7 2 5-5 2M15 7h4m-4 4h4m-4 4h4"/>',
"fan": '<circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="2"/><path d="M12 10c-8-10-11 3-2 2m2 2c2 12 11 4 2-1m-3-1c8-9 0-14-1-3"/>',
"vav_box": '<rect x="2" y="7" width="20" height="10"/><path d="M5 12h14M9 7l6 10M15 7 9 17"/>',
"damper": '<path d="M3 5h18v14H3zM5 17 19 7M12 2v3m0 14v3"/>',
"duct_insulation": '<rect x="2" y="7" width="20" height="10" stroke-dasharray="3 2"/><rect x="4" y="10" width="16" height="4"/>',
"duct_accessory": '<rect x="2" y="8" width="20" height="8"/><path d="M9 6v12m6-12v12M9 6l6 12"/>',
"pipe": '<path d="M3 9h18v6H3z"/><ellipse cx="20" cy="12" rx="2" ry="3"/>',
"pipe_fitting": '<path d="M6 2v15h16v5H2V2zM6 16a10 10 0 0 0 10 6"/>',
"pipe_elbow": '<path d="M4 2v17h18v3H2V2zM8 8a8 8 0 0 0 8 8"/>',
"pipe_tee": '<path d="M2 9h20v6H2zM10 15v7h4v-7"/>',
"pipe_transition": '<path d="M2 7h6l8 3h6v4h-6l-8 3H2zM9 7v10"/>',
"valve": '<path d="M2 7 12 12 2 17V7Zm20 0L12 12l10 5V7ZM12 12V3m-5 0h10"/>',
"pump": '<circle cx="12" cy="12" r="8"/><path d="m9 6 10 6-10 6zM1 12h3m16 0h3"/>',
"plumbing_fixture": '<path d="M3 5h18v8a9 9 0 0 1-18 0zM12 22v-4M7 22h10"/><circle cx="12" cy="11" r="2"/>',
"sprinkler": '<path d="M12 2v11M6 13h12M3 21c3-8 15-8 18 0M6 18v4m6-6v6m6-4v4"/>',
"pipe_insulation": '<path d="M2 8h20v8H2z" stroke-dasharray="3 2"/><path d="M4 10h16v4H4z"/>',
"cable_tray": '<path d="M3 6h18v12H3zM3 9h18m-14 0v9m5-9v9m5-9v9"/>',
"conduit": '<path d="M3 11h18m-18 4h18M4 11v4m16-4v4"/><ellipse cx="20" cy="13" rx="2" ry="2"/>',
"lighting_fixture": '<path d="M7 10a5 5 0 0 1 10 0c0 3-3 4-3 7h-4c0-3-3-4-3-7zM9 20h6m-5 2h4m-2-20v3m-9 6h3m12 0h3"/>',
"device": '<rect x="6" y="2" width="12" height="20" rx="2"/><path d="M9 7h6m-6 5h6M9 17h6"/>',
"switch": '<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M9 7h6v10H9zM10 9l4 6"/>',
"socket": '<rect x="5" y="3" width="14" height="18" rx="4"/><circle cx="9" cy="10" r="1"/><circle cx="15" cy="10" r="1"/><path d="M10 16h4"/>',
"panel": '<rect x="4" y="2" width="16" height="20"/><path d="M8 6h8v5H8zM8 15h2m4 0h2M8 19h8"/>',
"circuit": '<path d="M3 12h5m8 0h5M8 7v10l8-5zM3 4v16m18-16v16"/>',
"cable": '<path d="M3 21C3 11 10 16 10 7S21 1 21 10M3 17v4h4M17 6l4 4-4 2"/>',
"transformer": '<path d="M2 12h3m14 0h3M5 5v14m14-14v14M7 6c6-3 6 15 0 12m10-12c-6-3-6 15 0 12"/>',
"ifc_import": '<path d="M3 3h13l4 4v14H3zM16 3v5h4M6 13h9m-4-4 4 4-4 4"/>',
"ifc_export": '<path d="M3 3h13l4 4v14H3zM16 3v5h4M6 13h9m-5-4-4 4 4 4"/>',
"clash": '<path d="M2 4h13v13H2zM9 10h13v12H9z"/><path d="m10 13 6 6m0-6-6 6"/>',
"link_model": '<path d="m9 5 3-3 9 9-3 3m-9-9L3 11l9 9 6-6M8 9l7 7"/>',
"revision": '<path d="M12 3a9 9 0 1 0 9 9m-3-9h4v5M22 3 12 13m0-6v6l4 3"/>',
}

# The outline follows Tabler-like proportions but is independently authored.
COLORS = {
    "light": {"fg":"#264a63","fill":"#d9edf8","accent":"#2384b1"},
    "dark": {"fg":"#e4f2fb","fill":"#37576b","accent":"#80d4f5"},
}
NAMESPACE = "http://www.w3.org/2000/svg"


def svg(name, theme):
    if name in ISOMETRIC_SUPPORTED:
        return isometric_svg(name,theme)
    palette=COLORS[theme]
    shape=SHAPES[name]
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" '
        'viewBox="0 0 24 24" fill="none" '
        'stroke="{}" stroke-width="1.8" stroke-linecap="round" '
        'stroke-linejoin="round" color="{}">'.format(palette["fg"],palette["fg"])
        + '<g fill="none">' + shape + '</g>'
        + '</svg>\n'
    )

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--output",required=True)
    parser.add_argument("--require-approved", action="store_true", help="Fail unless every manifest command has approved artwork")
    args=parser.parse_args()
    output=Path(args.output).resolve()
    manifest=json.loads((ROOT/"manifest.json").read_text(encoding="utf-8"))
    named=[name for group in manifest["categories"].values() for name in group]
    if len(named)!=len(set(named)):
        raise ValueError("Duplicate icon IDs in manifest")
    supported=set(SHAPES) | set(ISOMETRIC_SUPPORTED)
    approved_assets = load(ROOT / "approved-3d", set(named))
    unavailable = set(named) - supported - approved_assets.keys()
    if unavailable:
        raise ValueError("No artwork for commands: " + ", ".join(sorted(unavailable)))
    missing = sorted(set(named) - approved_assets.keys())
    if args.require_approved and missing:
        raise ValueError("Missing approved command artwork: " + ", ".join(missing))
    entries=[]
    for theme in COLORS:
        for name in named:
            if name in approved_assets or name not in supported:
                continue
            target=output/theme/(name+".svg")
            target.parent.mkdir(parents=True,exist_ok=True)
            contents=svg(name,theme)
            ET.fromstring(contents)
            target.write_text(contents,encoding="utf-8")
            entries.append((theme,name))
    qrc=['<RCC>','  <qresource prefix="/icons">']
    for theme,name in entries:
        qrc.append('    <file alias="{}/{}.svg">{}/{}.svg</file>'.format(
            theme,name,theme,name))
    # Priority: approved rendered icons supplied as individual PNG crops
    # from the actual concept sheet. These are not the procedurally redrawn
    # SVGs. The corresponding source files may be placed under approved-3d/.
    overrides=set(approved_assets)
    approved_output = output / "approved"
    if approved_output.exists():
        shutil.rmtree(approved_output)
    for name, (data, source) in sorted(approved_assets.items()):
        approved_output.mkdir(parents=True, exist_ok=True)
        (approved_output / (name + ".png")).write_bytes(data)
        qrc.append('    <file alias="approved/{0}.png">approved/{0}.png</file>'.format(name))
    report = {"approved": sorted(overrides), "fallback": missing}
    (output / "coverage.json").write_text(json.dumps(report, indent=2) + "\n")
    print("Approved concept illustration overrides:", len(overrides))
    if missing:
        print("WARNING: schematic fallback icons remain for: " + ", ".join(missing))
    qrc.append('    <file alias="coverage.json">coverage.json</file>')
    qrc.extend(['  </qresource>','</RCC>'])
    (output/"icons.qrc").write_text("\n".join(qrc)+"\n",encoding="utf-8")

    # One-page QA gallery: inspect all command glyphs in both appearances.
    generated_ids = set(manifest["categories"]["common"]) | set(manifest["categories"].get("ribbon", [])) | {"duct_tee", "duct_accessory", "circuit", "cable"}
    cards=[]
    for category,icons in manifest["categories"].items():
        cards.append('<h2>{}</h2><div class="grid">'.format(category.title()))
        for name in icons:
            if name in overrides:
                cards.append('<div class="item"><div class="light"><img src="approved/{0}.png"></div>'
                             '<div class="dark"><img src="approved/{0}.png"></div>'
                             '<small>{0} ({1})</small></div>'.format(name, 'generated 3D' if name in generated_ids else 'catalog artwork'))
            else:
                cards.append('<div class="item"><div class="light"><img src="light/{0}.svg"></div>'
                             '<div class="dark"><img src="dark/{0}.svg"></div>'
                             '<small>{0} (fallback SVG)</small></div>'.format(name))
        cards.append('</div>')
    gallery = '''<!doctype html><html lang="en"><head><meta charset="utf-8">
    <title>OpenBIM Icon Library — Catalog Artwork</title>
    <style>
    body{font:14px system-ui,sans-serif;margin:30px;max-width:1200px;background:#f5f7f9;color:#263946}
    .grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(128px,1fr));gap:12px;margin-bottom:26px}
    .item{background:white;border:1px solid #cbd7df;border-radius:7px;overflow:hidden;text-align:center}
    .item>div{display:inline-flex;align-items:center;justify-content:center;width:49%;height:74px}
    .light{background:#f8fafc}.dark{background:#26323c}
    img{width:34px;height:34px;object-fit:contain}small{display:block;word-wrap:break-word;padding:10px;font-size:11px}
    </style></head><body><h1>OpenBIM Icon Library</h1>
    <p>Catalog artwork and generated 3D command symbols are identified separately; missing artwork is marked “fallback SVG”.
    See coverage.json for the complete command inventory.</p>'''+''.join(cards)+'</body></html>'
    (output/"gallery.html").write_text(gallery,encoding="utf-8")

    print("Packaged {} command icons ({} PNGs, {} fallback SVG files)".format(len(named),len(overrides),len(entries)))

if __name__=="__main__":
    main()
