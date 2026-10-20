"""Validate and import reviewed PNGs without relying on a ZIP's enclosing folders."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import zipfile

ROOT = Path(__file__).resolve().parent


def canonical_id(name):
    return re.sub(r'[^a-z0-9]+', '_', name.lower()).strip('_')


def collect(entries, known):
    assets = {}
    for filename, data in entries:
        path = Path(filename)
        if path.suffix.lower() != '.png' or '__MACOSX' in path.parts or path.name.startswith('._'):
            continue
        name = canonical_id(path.stem)
        if name not in known:
            continue  # Preview sheets and gallery images are not command assets.
        if name in assets:
            raise ValueError(f'Duplicate approved icon ID: {name} ({filename})')
        if len(data) < 33 or data[:8] != b'\x89PNG\r\n\x1a\n' or data[12:16] != b'IHDR':
            raise ValueError(f'Invalid PNG: {filename}')
        width, height = struct.unpack('>II', data[16:24])
        if not width or not height:
            raise ValueError(f'Empty PNG: {filename}')
        assets[name] = (data, filename)
    return assets


def load(directory, known):
    return collect(((str(p.relative_to(directory)), p.read_bytes())
                    for p in sorted(directory.rglob('*')) if p.is_file()), known)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    parser.add_argument('--destination', type=Path, default=ROOT / 'approved-3d')
    args = parser.parse_args()
    manifest = json.loads((ROOT / 'manifest.json').read_text())
    known = {name for group in manifest['categories'].values() for name in group}
    with zipfile.ZipFile(args.archive) as archive:
        assets = collect(((item.filename, archive.read(item)) for item in archive.infolist()
                          if not item.is_dir()), known)
    if not assets:
        raise ValueError('No recognized command PNGs in archive; inspect filenames before importing')
    args.destination.mkdir(parents=True, exist_ok=True)
    existing = load(args.destination, known)
    conflicts = [name for name in assets if name in existing and assets[name][0] != existing[name][0]]
    if conflicts:
        raise ValueError(f'Existing artwork differs; review before replacing: {conflicts}')
    inventory = {}
    for name, (data, source) in sorted(assets.items()):
        (args.destination / f'{name}.png').write_bytes(data)
        inventory[name] = {'source': source, 'sha256': hashlib.sha256(data).hexdigest()}
    (args.destination / 'import.json').write_text(json.dumps(inventory, indent=2) + '\n')
    print(f'Imported {len(assets)} approved command PNGs')
    print('Missing command IDs:', ', '.join(sorted(known - assets.keys())))


if __name__ == '__main__':
    main()
