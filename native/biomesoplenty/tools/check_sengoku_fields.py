"""Validate generated flower content, references and climate coverage."""
import json
from pathlib import Path
from sengoku_fields import FIELDS
root = Path(__file__).resolve().parents[1] / 'content'

def read(path):
    return json.loads((root / path).read_text())

def model_check(name, seen):
    if name in seen or not name.startswith('biomesoplenty:'):
        return
    seen.add(name)
    namespace, path = name.split(':', 1)
    model = read(f'assets/{namespace}/models/{path}.json')
    for texture in model.get('textures', {}).values():
        if texture.startswith('#'):
            continue
        ns, path = texture.split(':', 1)
        assert (root / f'assets/{ns}/textures/{path}.png').is_file(), texture
    if 'parent' in model:
        model_check(model['parent'], seen)

placement = {g: read(f'data/biomesoplenty/worldgen/biome_placement/overworld_{g}.json') for g in ['primary', 'secondary', 'rare', 'extra_rare']}
for biome, (flower, _, _) in FIELDS.items():
    definition = read(f'data/biomesoplenty/worldgen/biome/{biome}.json')
    assert f'biomesoplenty:flower_{biome}' in definition['features'][9]
    block = read(f'data/biomesoplenty/horizon/block/{flower}.json')
    assert block['properties']['occludes'] is False
    state = read(f'assets/biomesoplenty/blockstates/{flower}.json')
    for variant in state['variants']['']:
        model_check(variant['model'], set())
    group = 'rare' if biome == 'spider_lily_field' else 'primary'
    assert sum(e['biome'] == f'biomesoplenty:{biome}' for e in placement[group]['biomes']) == 166
    print(f'PASS {biome}: model/texture closure, non-occlusion, flower feature, 166 {group} climate entries')
assert sum(e['biome'] == 'biomesoplenty:overgrown_greens' for e in placement['extra_rare']['biomes']) == 166
assert placement['rare']['weight'] < placement['primary']['weight']
assert not any(e['biome'] == 'biomesoplenty:overgrown_greens' for e in placement['primary']['biomes'])
assert placement['extra_rare']['weight_divisor'] == 4
print('PASS Overgrown Greens exclusively extra rare; Spider Lily assigned the lower-weight rare region')

# Expected source colors, independent of the generator's namespace conversion.
colors = {
 'kerria_field': ('9dbc4d','649d3d','2585a0','6eb1ff','c0d8ff'),
 'lily_field': ('9dbc4d','649d3d','2585a0','78848c','c0d8ff'),
 'rose_field': ('ffd372','bc8939','1c7160','78bbfe','d5e4ff'),
 'spider_lily_field': (None,'10835f','2585a0','989fa0','aaaeb1'),
}
for biome, (grass, foliage, water, sky, fog) in colors.items():
 d=read(f'data/biomesoplenty/worldgen/biome/{biome}.json')
 for key, expected in [('grass_color',grass),('foliage_color',foliage),('water_color',water)]:
  assert d['effects'].get(key) == ('#'+expected if expected else None), (biome,key)
 for key, expected in [('sky_color',sky),('fog_color',fog),('water_fog_color',water)]:
  assert d['attributes'][f'minecraft:visual/{key}'] == '#'+expected, (biome,key)
print('PASS all Sengoku tint overrides mapped; Spider Lily grass remains climate-derived')
