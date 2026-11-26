#!/usr/bin/env python3
"""Assemble the authorized BOP data and finite feature variants for Horizon."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
from sengoku_fields import add_content, extend_climate
ROOT = Path(__file__).resolve().parents[1]

def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, separators=(',', ':')) + '\n')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workspace', type=Path, default=ROOT)
    parser.add_argument('--sengoku-jar', type=Path)
    parser.add_argument('--pack-tool', type=Path)
    parser.add_argument('--native-dev', action='store_true')
    parser.add_argument('--assemble-only', action='store_true')
    args = parser.parse_args()
    root = args.workspace.resolve()
    out = root / ('content-native-dev' if args.native_dev else 'content')
    sengoku = args.sengoku_jar or root.parent/'sengokuFabric-2.1.5+1.21.1-Fabric.jar'
    inputs = [root/'upstream/assets', root/'upstream/data', root/'baked/data',
              root/'import/carry/data/biomesoplenty/horizon/item',
              root/'tools/oracle/export/blocks.json', sengoku]
    if missing := [str(path) for path in inputs if not path.exists()]:
        parser.error('Missing assembly inputs: ' + ', '.join(missing))
    if out.exists(): shutil.rmtree(out)
    out.mkdir()
    for kind in ('assets', 'data'):
        shutil.copytree(root / 'upstream' / kind, out / kind)
    for p in list((out/'assets').iterdir()):
        if p.name != 'biomesoplenty': shutil.rmtree(p)
    for p in list((out/'data').iterdir()):
        if p.name not in ('biomesoplenty','minecraft','c'):
            shutil.rmtree(p);continue
        if p.name != 'biomesoplenty':
            for f in list(p.iterdir()):
                if f.name != 'tags':
                    shutil.rmtree(f) if f.is_dir() else f.unlink()
    # Only declarations generated from registered blocks; never carry's block guesses.
    blocks = json.loads((root / 'tools/oracle/export/blocks.json').read_text())
    for name, doc in blocks.items():
        doc = {k:v for k,v in doc.items() if not k.startswith('_')}
        if doc['template'] in ('minecraft:kelp_plant','minecraft:weeping_vines_plant','minecraft:twisting_vines_plant','minecraft:cave_vines_plant','minecraft:bamboo_sapling','minecraft:piston_head','minecraft:moving_piston'):
            doc['item']=False
        if doc['template'] in ('minecraft:kelp','minecraft:kelp_plant','minecraft:cave_vines','minecraft:cave_vines_plant','minecraft:weeping_vines','minecraft:weeping_vines_plant','minecraft:twisting_vines','minecraft:twisting_vines_plant'):
            # Horizon's vanilla growing-family evaluator is restricted to vanilla IDs.
            # Preserve models/states, using a supported plant survival template.
            doc['template']='minecraft:hanging_roots' if any(x in name for x in ('hanging','tendons','cobweb')) else 'minecraft:short_grass'
            props=doc.setdefault('state_properties',{})
            statefile=out/f'assets/biomesoplenty/blockstates/{name}.json'
            states=json.loads(statefile.read_text()).get('variants',{})
            for key,value in doc.get('default_state',{}).items():
                values={v.split('=',1)[1] for k in states for v in k.split(',') if v.startswith(key+'=')}
                values.add(value)
                props[key]={'type':'enum','values':sorted(values)}
        # Partial/alpha-cutout geometry must not inherit full-cube face occlusion
        # from the closest behavior template (bookshelf / carved pumpkin).
        if name in ('dead_branch', 'huge_clover_petal', 'spider_egg'):
            doc.setdefault('properties', {})['occludes'] = False
        if args.native_dev and name == 'high_grass':
            doc.setdefault('state_properties', {})['age'] = {'type':'enum','values':[str(i) for i in range(26)]}
        write(out / f'data/biomesoplenty/horizon/block/{name}.json', doc)
    # Explicit upstream block color providers; templates do not imply a color source.
    color_groups={
      'grass':['flower_stem','mossy_black_sand','sprout','high_grass','high_grass_plant','clover','huge_clover_petal','barley','watergrass','potted_sprout'],
      'foliage':['flowering_oak_leaves','pine_leaves','mahogany_leaves','palm_leaves','willow_leaves','willow_vine','bramble_leaves'],
      'dry_foliage':['dead_leaves','desert_grass'],
      'lily_pad':['huge_lily_pad']}
    colors={'biomesoplenty:'+name:[color] for color,names in color_groups.items() for name in names}
    colors.update({'biomesoplenty:'+name:['none','grass'] for name in ['white_petals','purple_wildflowers']})
    write(out/'assets/biomesoplenty/horizon/block_colors.json',colors)
    items = root / 'import/carry/data/biomesoplenty/horizon/item'
    shutil.copytree(items, out / 'data/biomesoplenty/horizon/item')
    add_content(out, sengoku)
    declarations=out/'data/biomesoplenty/horizon/block'
    names = ['biomesoplenty:'+p.stem for p in declarations.glob('*.json') if json.loads(p.read_text()).get('item',True)]
    names += ['biomesoplenty:'+p.stem for p in items.glob('*.json')]
    write(out/'data/biomesoplenty/horizon/creative_tab/biomesoplenty.json', {
        'title':"Biomes O' Plenty", 'icon':'biomesoplenty:jacaranda_sapling', 'items':names})
    shutil.copytree(root/'baked/data', out/'data', dirs_exist_ok=True)
    # These checked templates accept the forest's podzol surface as well as grass.
    baked_path = Path('data/biomesoplenty/structure/baked')
    fixes = ROOT.parents[1]/'content-fixes/biomesoplenty'/baked_path
    for tree in ('redwood_tree', 'redwood_tree_medium', 'redwood_tree_large'):
        shutil.copytree(fixes/tree, out/baked_path/tree, dirs_exist_ok=True)
    feature_path = Path('data/biomesoplenty/worldgen/feature')
    for relative in ('saplings', 'huge_saplings'):
        fixes = ROOT.parents[1]/'content-fixes/biomesoplenty'/feature_path/relative
        shutil.copytree(fixes, out/feature_path/relative, dirs_exist_ok=True)
    if args.native_dev:
        for name in ('high_grass','small_fumarole'):
            shutil.copy2(root/f'upstream/data/biomesoplenty/worldgen/feature/{name}.json', out/f'data/biomesoplenty/worldgen/feature/{name}.json')
    for name,block in [('blood_spring','blood'),('liquid_null_spring','liquid_null')]:
        spring=out/f'data/biomesoplenty/worldgen/feature/{name}.json'
        doc=json.loads(spring.read_text());doc.pop('state',None);doc['block_state']='biomesoplenty:'+block;write(spring,doc)
    coral=out/'data/biomesoplenty/worldgen/feature/dead_coral.json'
    doc=json.loads(coral.read_text())
    def inline(value):
        if isinstance(value,dict):
            if value.get('type')=='biomesoplenty:dead_coral_block':
                return {'type':'minecraft:simple_block','to_place':value['to_place']}
            return {k:inline(v) for k,v in value.items()}
        if isinstance(value,list): return [inline(v) for v in value]
        return value
    write(coral,inline(doc))
    # Large pumpkins are landmarks: one attempt per 16 chunks instead of every chunk.
    pumpkin = out/'data/biomesoplenty/worldgen/placed_feature/big_pumpkin.json'
    doc = json.loads(pumpkin.read_text())
    doc['placement'][0] = {'type':'minecraft:rarity_filter','chance':16}
    write(pumpkin, doc)
    # Normalize the generic weighted-template codec.
    for p in (out/'data/biomesoplenty/worldgen/feature').glob('*.json'):
        doc=json.loads(p.read_text())
        if doc.get('type')=='minecraft:template':
            doc['templates']=[{'data':{k:v for k,v in t.items() if k!='weight'},'weight':t.get('weight',1)} if 'data' not in t else t for t in doc['templates']]
            write(p,doc)
    # Every exported feature retains its original placement anchor and input requirements.
    for p in (out/'data/biomesoplenty/worldgen/feature').glob('*.json'):
        doc=json.loads(p.read_text())
        if doc.get('type')=='minecraft:template':
            assert doc.get('require_environment') is True, f'unprotected template: {p}'
            assert not doc.get('processors'), f'export unexpectedly relocates template: {p}'
    known={'biomesoplenty:'+p.stem for p in (out/'data/biomesoplenty/worldgen/biome').glob('*.json')}
    for filename,weights,preset,selector in (
        ('climate.json',{'primary':10,'secondary':8,'rare':2,'extra_rare':1},'overworld','weirdness'),
        ('nether.json',{'common':13,'rare':2},'nether','humidity')):
        groups=json.loads((root/'tools/oracle/export'/filename).read_text())
        if preset == 'overworld':
            import copy
            groups['extra_rare'] = copy.deepcopy(groups['primary'])
            for entry in groups['primary']:
                if entry['biome'] == 'biomesoplenty:overgrown_greens':
                    entry['biome'] = 'biomesoplenty:field'
        for group,entries in groups.items():
            if preset == 'overworld':
                entries = extend_climate(group, entries)
            assert all(e['biome'].startswith('minecraft:') or e['biome'] in known for e in entries), 'unregistered climate biome'
            write(out/f'data/biomesoplenty/worldgen/biome_placement/{preset}_{group}.json', {
                'preset':'minecraft:'+preset,'group':f'biomesoplenty:{preset}_{group}',
                'weight':weights[group],'selector':selector,'biomes':entries,
                'region_order': ({'primary':1,'secondary':2,'rare':3,'extra_rare':4} if preset=='overworld' else {'common':1,'rare':2})[group],
                'region_zooms':3 if preset=='overworld' else 2,'vanilla_weight':10,
                'weight_divisor':4 if group=='extra_rare' else 1,
                'region_min_quarts':192 if preset=='overworld' else 0,
                'configuration':{'file':'biomesoplenty/generation.toml','key':(
                    {'primary':'overworld.bop_primary_overworld_region_weight','secondary':'overworld.bop_secondary_overworld_region_weight','rare':'overworld.bop_overworld_rare_region_weight','extra_rare':'overworld.bop_overworld_extra_rare_region_weight'} if preset=='overworld' else
                    {'common':'nether.bop_nether_region_weight','rare':'nether.bop_nether_rare_region_weight'})[group]}})
    rules=out/'data/biomesoplenty/worldgen/material_rule';rules.mkdir(parents=True,exist_ok=True)
    for dimension in ('overworld','nether','end'):
        shutil.copy2(root/f'tools/oracle/export/surface_{dimension}.json', rules/f'surface_{dimension}.json')
        extension={'settings':'minecraft:'+dimension,'surface_rule':'biomesoplenty:surface_'+dimension}
        if dimension=='end': extension['biome_replacements']={'minecraft:end_highlands':[
            {'biome':'minecraft:end_highlands','weight':1},
            *[{'biome':'biomesoplenty:'+n,'weight':w} for n,w in [('end_wilds',10),('end_reef',8),('end_flats',4),('end_corruption',2)]]]}
        write(out/f'data/biomesoplenty/worldgen/generation_extension/{dimension}.json', extension)
    (out/'horizon-mod.toml').write_text('''id = "biomesoplenty"
name = "Biomes O' Plenty – Horizon"
description = "World-generation test port: original biomes and assets, finite custom-feature shape variants. Choose before creating worlds."
version = "0.2.7"
api = "0.2"
side = "both"
capabilities = ["registries"]
''')
    if args.native_dev:
        p=out/'horizon-mod.toml'
        p.write_text(p.read_text().replace('version = "0.2.7"','version = "0.3.0-dev"'))
    if args.assemble_only: return
    tool=args.pack_tool or root.parent/'Lunatic/target/tools/debug/cargo-horizon-mod'
    subprocess.run([tool,'build'],cwd=out,check=True)
    package=root/('target/biomesoplenty-native-dev.hmod' if args.native_dev else 'target/biomesoplenty.hmod');package.parent.mkdir(exist_ok=True)
    shutil.copy2(out/'target/horizon-mod/biomesoplenty.hmod',package)
    subprocess.run([tool,'check',package,'--json'],check=True)
    print(package)

if __name__ == '__main__': main()
