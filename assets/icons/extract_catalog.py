#!/usr/bin/env python3
"""Reproduce ribbon PNGs from the user's catalog without redrawing its artwork.
Requires Pillow only when extracting; normal application builds use committed PNGs.
"""
import hashlib
import json
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT / 'catalog-source' / 'catalog.png'
# Icons that are replacements, not catalog crops. Skip extraction to preserve them.
SKIP = {'socket', 'switch'}
# Bounds exclude the catalog's labels, tile borders and headings.
ROWS = [
 ('architecture', (46,130), [12,96,181,267,359,456,547,638,724,811,902,992,1081,1171,1262,1351,1440,1525],
  'wall door window curtain_wall floor roof ceiling column room area stair railing railing_solid ramp model_line component opening'.split()),
 ('structure', (218,310), [12,95,180,265,354,445,541,633,720,806,901,999,1091,1182,1267,1352,1438,1525],
  'structural_column beam _brace_duplicate truss foundation structural_floor structural_wall rebar connection slab framing footing anchor pier shear_wall brace_frame structural_section'.split()),
 ('hvac', (395,478), [12,96,181,264,350,441,534,616,710,808,894,991,1077,1171,1294,1420,1525],
  'duct duct_fitting duct_elbow duct_transition flex_duct air_terminal vav_box ahu fan damper duct_insulation grille diffuser mechanical_equipment chiller cooling_tower'.split()),
 ('plumbing', (569,652), [12,96,181,265,350,442,533,620,710,809,898,991,1087,1167,1248,1330,1417,1525],
  'pipe pipe_fitting pipe_elbow pipe_tee pipe_transition valve valve_ball valve_butterfly pump plumbing_fixture sprinkler pipe_insulation hanger support drain cleanout backflow_preventer'.split()),
 ('electrical', (743,824), [12,98,182,268,353,456,547,639,723,808,896,994,1097,1181,1270,1351,1444,1525],
  'cable_tray cable_tray_fitting conduit conduit_fitting lighting_fixture light_pendant light_surface switch socket panel transformer generator disconnect junction_box pull_box raceway device'.split()),
 ('coordination', (912,977), [12,95,182,270,381,490,575,661,743,846,934,1023,1114,1220,1324,1438,1525],
  'ifc_import ifc_export link_model clash interference_check workset phase grid level section elevation detail dimension tag revision view'.split()),
]


def main():
    source = Image.open(SOURCE).convert('RGB')
    if source.size != (1536,1024):
        raise ValueError('Crop coordinates require the original 1536 x 1024 catalog')
    destination = ROOT / 'approved-3d'
    destination.mkdir(exist_ok=True)
    inventory = {'source': 'catalog-source/catalog.png',
                 'source_sha256': hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
                 'assets': {}}
    categories = {}
    for category, (top,bottom), edges, names in ROWS:
        assert len(edges) == len(names)+1
        categories[category] = []
        for i,name in enumerate(names):
            if name.startswith('_'):
                continue  # Keep only the labeled Brace Frame as requested.
            if name in SKIP:
                continue  # Preserve British replacements, not catalog crops.
            bounds = [edges[i]+4, top, edges[i+1]-4, bottom]
            tile = source.crop(bounds)
            side = max(tile.size)
            icon = Image.new('RGB',(side,side),'white')
            icon.paste(tile,((side-tile.width)//2,(side-tile.height)//2))
            output = destination / (name+'.png')
            icon.save(output)
            categories[category].append(name)
            inventory['assets'][name] = {'crop': bounds, 'sha256': hashlib.sha256(output.read_bytes()).hexdigest()}
    # Compatibility for the former generic Brace command, deliberately the labeled frame.
    (destination/'brace.png').write_bytes((destination/'brace_frame.png').read_bytes())
    inventory['assets']['brace'] = {'alias_of': 'brace_frame', 'reason': 'Unlabeled duplicate omitted'}
    (destination/'catalog-provenance.json').write_text(json.dumps(inventory,indent=2)+'\n')
    manifest = json.loads((ROOT/'manifest.json').read_text())
    common = manifest['categories']['common']
    categories['hvac'] += ['duct_tee','duct_accessory']
    categories['electrical'] += ['circuit','cable']
    categories['structure'].append('brace')
    manifest['version'] = '0.3.0'
    extensions = {key:value for key,value in manifest['categories'].items() if key not in categories and key != 'common'}
    manifest['categories'] = {'common': common, **categories, **extensions}
    manifest['artwork'] = 'User-supplied BIM/CAD catalog crops and generated common/supplemental 3D icons'
    (ROOT/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(f'Extracted {len(inventory["assets"])} catalog resources')


if __name__ == '__main__':
    main()
