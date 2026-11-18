#!/usr/bin/env python3
"""Assemble the dependency-checked Horizon biome content. Native features retain their IDs."""
import json, shutil
from pathlib import Path
from import_jar import ROOT, NS, BIOMES, MUSHROOM_GROWTH
OUT=ROOT/'content'
def write(p,v):
    p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,indent=2)+'\n')
def convert_mushroom_loot(value):
    """Convert the JAR's 1.20 loot conditions and modifiers to Horizon's game version."""
    if isinstance(value, list):
        return [convert_mushroom_loot(entry) for entry in value]
    if not isinstance(value, dict):
        return value
    result = {}
    for key, entry in value.items():
        if key == 'condition':
            if entry == 'minecraft:match_tool' and 'enchantments' in value['predicate']:
                return 'minecraft:tool/can_silk_touch'
            result['type'] = entry
        elif key == 'function':
            result['type'] = entry
        elif key in ('conditions', 'functions'):
            result['condition' if key == 'conditions' else 'modifier'] = convert_mushroom_loot(entry)
        else:
            result[key] = convert_mushroom_loot(entry)
    return result

def main():
    inv=json.loads((ROOT/'inventory.json').read_text())
    if OUT.exists():shutil.rmtree(OUT)
    OUT.mkdir();shutil.copy2(ROOT/'horizon-mod.toml',OUT/'horizon-mod.toml')
    # Only files in the selected dependency closure; no stale reference extraction.
    for n in inv['hashes']:
        if '/loot_tables/' in n:
            write(OUT/n.replace('/loot_tables/', '/loot_table/'), convert_mushroom_loot(json.loads((ROOT/'upstream'/n).read_text())))
        elif n.startswith('data/'):
            p=OUT/n;p.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/'upstream'/n,p)
    # Resource models recursively retain their original parents and texture references.
    todo=[f'assets/{NS}/blockstates/{b}.json' for b in inv['blocks']]
    todo += [f'assets/{NS}/models/item/{b}.json' for b in inv['blocks'] if (ROOT/f'upstream/assets/{NS}/models/item/{b}.json').exists()]
    for color in ('pink', 'purple'):
        particle = f'assets/{NS}/particles/{color}_cherry_blossom.json'
        definition = json.loads((ROOT/'upstream'/particle).read_text())
        definition['horizon:behaviour'] = {'falling_petal': True}
        write(OUT/particle, definition)
        for sprite in definition['textures']:
            todo.append(f"assets/{NS}/textures/particle/{sprite.split(':', 1)[1]}.png")
    for particle, size in [('miasma', 0.20), ('miasma_small', 0.12)]:
        path = f'assets/{NS}/particles/{particle}.json'
        definition = json.loads((ROOT/'upstream'/path).read_text())
        definition['horizon:behaviour'] = {'rising_spore': size}
        write(OUT/path, definition)
        for sprite in definition['textures']:
            todo.append(f"assets/{NS}/textures/particle/{sprite.split(':', 1)[1]}.png")
    todo.extend([f'assets/{NS}/blockstates/miasma_air.json', f'assets/{NS}/textures/mob_effect/miasma.png'])
    done=set()
    def refs(v):
        if isinstance(v,str) and v.startswith(NS+':'):yield v.split(':',1)[1]
        elif isinstance(v,list):
            for x in v:yield from refs(x)
        elif isinstance(v,dict):
            for x in v.values():yield from refs(x)
    while todo:
        n=todo.pop()
        if n in done:continue
        done.add(n);src=ROOT/'upstream'/n
        if not src.exists():raise ValueError(f'Missing resource {n}')
        p=OUT/n;p.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(src,p)
        if n.endswith('.json'):
            for r in refs(json.loads(src.read_text())):
                for typ,ext in [('models','json'),('textures','png')]:
                    q=f'assets/{NS}/{typ}/{r}.{ext}'
                    if (ROOT/'upstream'/q).exists():todo.append(q)
        elif n.endswith('.png') and src.with_suffix('.png.mcmeta').exists():shutil.copy2(src.with_suffix('.png.mcmeta'),p.with_suffix('.png.mcmeta'))
    langs=ROOT/f'upstream/assets/{NS}/lang'
    for p in langs.glob('*.json'):
        v=json.loads(p.read_text());v={k:x for k,x in v.items() if k.startswith(('block.','biome.')) or k == f'effect.{NS}.miasma'}
        write(OUT/f'assets/{NS}/lang/{p.name}',v)
    for b in inv['blocks']:
        if b.endswith('_cherry_fallen_leaves'):template='moss_carpet'
        elif b.endswith('_leaves'):template='oak_leaves'
        elif b.endswith(('_log','_wood')) or b=='locust_hollow_log':template='oak_log'
        elif b.endswith('_fallen_leaves'):template='moss_carpet'
        elif b.endswith(('_mushroom_block','_mushroom_stem')):template='brown_mushroom_block'
        elif b.endswith('_mushroom'):template='brown_mushroom'
        elif b=='broom_grass':template='tall_grass'
        elif b.startswith('eugune_'):template='ladder'
        elif b.startswith('evergreen_vine'):template='hanging_roots'
        elif b=='small_water_lily':template='lily_pad'
        elif b=='large_water_lily':template='lily_pad'
        elif b in ('gravestone','jizo','jizo_straw_hat','stone_lantern'):template='stone'
        else:template='short_grass'
        # State domains are sourced from the actual blockstate variants/conditions.
        doc=json.loads((ROOT/f'upstream/assets/{NS}/blockstates/{b}.json').read_text());domains={}
        for key in doc.get('variants',{}):
            for pair in key.split(','):
                if '=' in pair:
                    k,v=pair.split('=');domains.setdefault(k,set()).add(v)
        def conditions(v):
            if isinstance(v,dict):
                for k,x in v.items():
                    if isinstance(x,str):domains.setdefault(k,set()).update(x.split('|'))
                    else:conditions(x)
            elif isinstance(v,list):
                for x in v:conditions(x)
        for x in doc.get('multipart',[]):conditions(x.get('when',{}))
        inherited={'oak_log':{'axis'},'oak_leaves':{'distance','persistent','waterlogged'},'brown_mushroom_block':{'up','down','north','south','east','west'},'tall_grass':{'half'},'ladder':{'facing','waterlogged'}}.get(template,set())
        domains={k:v for k,v in domains.items() if k not in inherited}
        decl={'template':'minecraft:'+template,'state_properties':{k:{'type':'enum','values':sorted(v)} for k,v in domains.items()},'properties':{'occludes':False} if template not in ('oak_log','brown_mushroom_block') else {}}
        if b in ('gravestone','jizo','jizo_straw_hat','stone_lantern') or b.startswith('eugune_'):
            from shapes import shape
            bounds=shape(OUT,b)
            if bounds:decl['shapes']=bounds
            if b.startswith('eugune_'):decl['collision_shapes']={'default':[]}
            elif bounds:decl['collision_shapes']=bounds
        if b.endswith('_fallen_leaves'):decl['collision_shapes']={'default':[]}
        if b in ('pink_cherry_leaves', 'purple_cherry_leaves'):decl['properties']['random_ticks']=True
        if b=='locust_hollow_log':decl['properties']['occludes']=False
        if b=='large_water_lily':decl['shapes']={'default':[[1/16,9/16,1/16,15/16,10.5/16,15/16]]}
        if b in ('ghost_fire_mushroom','ghost_fire_mushroom_block','eugune_ghost_fire'):decl['properties']['light']=8
        if b=='golden_lotus':decl['default_state']={'part':'single','size':'0'}
        if b == 'demonic_miasma_mushroom_block':
            decl['properties']['random_ticks'] = True
        write(OUT/f'data/{NS}/horizon/block/{b}.json',decl)
    write(OUT/f'data/{NS}/horizon/block/miasma_air.json', {
        'template': 'minecraft:air', 'item': False,
        'properties': {'random_ticks': True},
        'state_properties': {'intensity': {'type': 'enum', 'values': [str(i) for i in range(1, 9)]}},
        'default_state': {'intensity': '8'},
    })
    write(OUT/f'data/{NS}/horizon/mob_effect/miasma.json', {'category': 'harmful', 'color': 0x7a4ba1})
    write(OUT/f'data/{NS}/horizon/creative_tab/nature.json',{'title':'Gensokyo Legacy','icon':NS+':star_flower','items':[NS+':'+b for b in inv['blocks']]})
    from overworld import add
    add(OUT,write)
    # The 1.20.1 dirt tag included grass, podzol, mycelium, moss and mud.
    mushroom_ground = f'{NS}:mushroom_template_ground'
    write(OUT/f'data/{NS}/tags/block/mushroom_template_ground.json', {
        'values': ['#minecraft:dirt', 'minecraft:grass_block', 'minecraft:podzol',
                   'minecraft:mycelium', 'minecraft:moss_block', 'minecraft:mud',
                   'minecraft:muddy_mangrove_roots'],
    })
    for name in MUSHROOM_GROWTH:
        path = OUT/f'data/{NS}/worldgen/configured_feature/{name}.json'
        feature = json.loads(path.read_text())
        feature['config']['ground'] = '#'+mushroom_ground
        write(path, feature)
    from normalize import apply
    apply(OUT,write)
    from bamboo_forest import add as add_bamboo_forest
    add_bamboo_forest(OUT, write)
    from namespace import rebase
    rebase(OUT)
    licenses=OUT/'assets/horizon_gensokyo_legacy/licenses';licenses.mkdir(parents=True,exist_ok=True)
    for name,dest in [('LICENSE','license.txt'),('THIRD_PARTY_NOTICES.md','third_party_notices.txt')]:shutil.copy2(ROOT/name,licenses/dest)
    print(f'Assembled {len(BIOMES)} biomes, {len(inv["blocks"])} blocks, {inv["templates"]} templates')
if __name__=='__main__':main()
