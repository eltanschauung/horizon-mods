#!/usr/bin/env python3
"""Extract the authorized BOP JAR's data/assets and inventory native work.

This is an importer, not a Java-to-Rust behavior converter. It never installs
content into a game, changes a world, or declares missing algorithms supported.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('jar', type=Path)
    args = parser.parse_args()
    files = {}
    features = Counter()
    placement = Counter()
    carvers = Counter()
    kinds = Counter()
    with ZipFile(args.jar) as jar:
        for info in jar.infolist():
            p = Path(info.filename)
            if p.is_absolute() or '..' in p.parts:
                raise ValueError(f'Unsafe archive path: {info.filename}')
            if info.is_dir() or not (info.filename.startswith(('assets/', 'data/'))
                                    or info.filename == 'META-INF/mods.toml'):
                continue
            data = jar.read(info)
            dest = ROOT / 'upstream' / p
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(data)
            files[info.filename] = hashlib.sha256(data).hexdigest()
            prefix = 'data/biomesoplenty/worldgen/'
            if info.filename.startswith(prefix) and info.filename.endswith('.json'):
                kind = info.filename[len(prefix):].split('/')[0]
                kinds[kind] += 1
                doc = json.loads(data)
                if kind == 'feature':
                    features[doc.get('type', '?')] += 1
                if kind == 'carver':
                    carvers[doc.get('type', '?')] += 1
                if kind == 'placed_feature':
                    placement.update(p.get('type', '?') for p in doc.get('placement', []))
    custom = {k: v for k, v in sorted(features.items()) if not k.startswith('minecraft:')}
    report = {
        'jar': args.jar.name,
        'sha256': hashlib.sha256(args.jar.read_bytes()).hexdigest(),
        'license': 'All Rights Reserved; user states permission for faithful port',
        'resource_files': len(files),
        'resource_hashes': dict(sorted(files.items())),
        'worldgen_counts': dict(sorted(kinds.items())),
        'configured_feature_types': dict(sorted(features.items())),
        'native_feature_types_to_port': custom,
        'native_feature_type_count': len(custom),
        'native_feature_definition_count': sum(custom.values()),
        'placement_modifier_types': dict(sorted(placement.items())),
        'carver_types': dict(sorted(carvers.items())),
        'faithful_port_complete': False,
        'installed': False,
    }
    (ROOT / 'asset-inventory.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f'Extracted and hash recorded {len(files)} files to {ROOT / "upstream"}')
    print(f'{kinds["biome"]} biomes; {len(custom)} custom feature algorithms still require native implementations')

if __name__ == '__main__':
    main()
