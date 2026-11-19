"""Flower-field adaptation; credit to Sengoku Jidai by QwikSand."""
import copy
import json
import zipfile
from pathlib import Path

FIELDS = {
    'kerria_field': ('kerria', 'dandelion', ['dandelion']),
    'lily_field': ('lily', 'orange_tulip', ['orange_tulip', 'orange_tulip2']),
    'spider_lily_field': ('spider_lily', 'red_tulip', ['red_tulip', 'red_tulip2']),
    'rose_field': ('rose', 'poppy', ['poppy']),
}

def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n')

def add_content(out, jar):
    base = out / 'data/biomesoplenty'
    assets = out / 'assets/biomesoplenty'
    language_path = assets / 'lang/en_us.json'
    language = json.loads(language_path.read_text())
    with zipfile.ZipFile(jar) as source:
        files = set(source.namelist())
        for biome, (flower, vanilla, models) in FIELDS.items():
            variants = []
            for model in models:
                resource = f'assets/minecraft/models/block/{model}.json'
                definition = json.loads(source.read(resource)) if resource in files else {
                    'parent': 'minecraft:block/cross', 'textures': {'cross': f'minecraft:block/{vanilla}'}}
                for key, texture in definition.get('textures', {}).items():
                    if texture.startswith('#'):
                        continue
                    path = texture.split(':', 1)[-1]
                    texture_file = f'assets/minecraft/textures/{path}.png'
                    assert texture_file in files, texture_file
                    target = assets / f'textures/block/sengoku_fields/{Path(path).name}.png'
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(source.read(texture_file))
                    definition['textures'][key] = f'biomesoplenty:block/sengoku_fields/{Path(path).name}'
                write(assets / f'models/block/sengoku_fields/{model}.json', definition)
                variants.append({'model': f'biomesoplenty:block/sengoku_fields/{model}'})
            write(assets / f'blockstates/{flower}.json', {'variants': {'': variants}})
            write(assets / f'models/item/{flower}.json', {'parent': variants[0]['model']})
            write(assets / f'items/{flower}.json', {'model': {'type': 'minecraft:model', 'model': f'biomesoplenty:item/{flower}'}})
            # Explicit non-occlusion also protects all alpha-cutout model variants.
            write(base / f'horizon/block/{flower}.json', {'template': f'minecraft:{vanilla}', 'properties': {'occludes': False}})
            write(base / f'loot_table/blocks/{flower}.json', {
                'type': 'minecraft:block', 'pools': [{'rolls': 1, 'entries': [{'type': 'minecraft:item', 'name': f'biomesoplenty:{flower}'}],
                'conditions': [{'condition': 'minecraft:survives_explosion'}]}]})
            feature = f'flower_{biome}'
            write(base / f'worldgen/feature/{feature}.json', {'type': 'minecraft:simple_block', 'to_place': {'id': f'biomesoplenty:{flower}'}})
            placement = json.loads((base / 'worldgen/placed_feature/flower_lavender.json').read_text())
            placement['feature'] = f'biomesoplenty:{feature}'
            placement['placement'][0]['count'] = 6
            write(base / f'worldgen/placed_feature/{feature}.json', placement)
            definition = json.loads((base / 'worldgen/biome/field.json').read_text())
            original = json.loads(source.read(f'data/sengoku/worldgen/biome/{biome}.json'))
            effects = original['effects']
            for key in ('grass_color', 'foliage_color', 'water_color'):
                definition['effects'].pop(key, None)
                if key in effects:
                    definition['effects'][key] = f"#{effects[key]:06x}"
            for key in ('sky_color', 'fog_color', 'water_fog_color'):
                if key in effects:
                    definition['attributes'][f'minecraft:visual/{key}'] = f"#{effects[key]:06x}"
            if 'grass_color' not in effects:
                definition['temperature'] = original['temperature']
                definition['downfall'] = original['downfall']
            definition['features'][9] = ['minecraft:glow_lichen', 'biomesoplenty:patch_grass_24',
                'biomesoplenty:patch_tall_grass_64', f'biomesoplenty:{feature}']
            # Existing BOP climate, mobs, carvers and geology; no Sengoku terrain rules.
            write(base / f'worldgen/biome/{biome}.json', definition)
            language[f'block.biomesoplenty.{flower}'] = flower.replace('_', ' ').title()
            language[f'biome.biomesoplenty.{biome}'] = biome.replace('_', ' ').title()
        for tag in ['small_flowers', 'flowers']:
            path = out / f'data/minecraft/tags/block/{tag}.json'
            data = json.loads(path.read_text()) if path.exists() else {'replace': False, 'values': []}
            for flower, _, _ in FIELDS.values():
                if f'biomesoplenty:{flower}' not in data['values']:
                    data['values'].append(f'biomesoplenty:{flower}')
            write(path, data)
            item_path = out / f'data/minecraft/tags/item/{tag}.json'
            item_data = json.loads(item_path.read_text()) if item_path.exists() else {'replace': False, 'values': []}
            item_data['values'] = list(dict.fromkeys(item_data['values'] + [f'biomesoplenty:{flower}' for flower, _, _ in FIELDS.values()]))
            write(item_path, item_data)
        # All four flower fields belong to the common plains habitat category.
        path = out / 'data/c/tags/worldgen/biome/is_plains.json'
        data = json.loads(path.read_text()) if path.exists() else {'replace': False, 'values': []}
        existing = {value if isinstance(value, str) else value['id'] for value in data['values']}
        data['values'].extend({'id': f'biomesoplenty:{biome}', 'required': False}
                              for biome in FIELDS if f'biomesoplenty:{biome}' not in existing)
        write(path, data)
        # Inherit the base Field biome's existing structure/habitat tags.
        for path in (out / 'data').glob('*/tags/worldgen/biome/**/*.json'):
            data = json.loads(path.read_text())
            if 'biomesoplenty:field' in data.get('values', []):
                data['values'] = list(dict.fromkeys(data['values'] + [f'biomesoplenty:{biome}' for biome in FIELDS]))
                write(path, data)
    write(language_path, language)

def extend_climate(group, entries):
    # Each common field gets the same 166 terrestrial parameter boxes as
    # Overgrown Greens, in a comparable climate cell. Donor biomes retain
    # their other climate cells. No oceans, rivers or mountains are replaced.
    replacements = [
        ('primary', 'field', [-0.45, -0.15], [0.1, 0.3], 'kerria_field'),
        ('primary', 'coniferous_forest', [-0.45, -0.15], [-0.1, 0.1], 'lily_field'),
        ('primary', 'rainforest', [0.2, 0.55], [0.1, 0.3], 'rose_field'),
        ('rare', 'redwood_forest', [-0.15, 0.2], [-0.1, 0.1], 'spider_lily_field'),
    ]
    entries = copy.deepcopy(entries)
    # Extend bamboo into the adjacent wet jungle band nearest river weirdness.
    # Keep the donor's temperature, humidity, terrain and depth constraints intact.
    for entry in entries:
        p = entry['parameters']
        if (entry['biome'] == 'minecraft:jungle'
                and p['temperature'] == [0.2, 0.55]
                and p['humidity'] == [0.3, 1.0]
                and p['weirdness'] == [-0.2666, -0.05]):
            entry['biome'] = 'minecraft:bamboo_jungle'
    for region, donor, temperature, humidity, biome in replacements:
        if region != group:
            continue
        count = 0
        for entry in entries:
            p = entry['parameters']
            if entry['biome'] == f'biomesoplenty:{donor}' and p['temperature'] == temperature and p['humidity'] == humidity:
                entry['biome'] = f'biomesoplenty:{biome}'
                count += 1
        assert count == 166, (biome, count)
    return entries
