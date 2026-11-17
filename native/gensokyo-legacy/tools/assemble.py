#!/usr/bin/env python3
"""Assemble the dependency-checked Horizon biome content. Native features retain their IDs."""
import json, shutil
from pathlib import Path
from import_jar import ROOT, NS, BIOMES
OUT=ROOT/'content'
def write(p,v):
    p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,indent=2)+'\n')
def main():
    inv=json.loads((ROOT/'inventory.json').read_text())
    if OUT.exists():shutil.rmtree(OUT)
    OUT.mkdir();shutil.copy2(ROOT/'horizon-mod.toml',OUT/'horizon-mod.toml')
    # Only files in the selected dependency closure; no stale reference extraction.
    for n in inv['hashes']:
        if n.startswith('data/'):
            p=OUT/n;p.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/'upstream'/n,p)
    # Resource models recursively retain their original parents and texture references.
    todo=[f'assets/{NS}/blockstates/{b}.json' for b in inv['blocks']]
    todo += [f'assets/{NS}/models/item/{b}.json' for b in inv['blocks'] if (ROOT/f'upstream/assets/{NS}/models/item/{b}.json').exists()]
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
        v=json.loads(p.read_text());v={k:x for k,x in v.items() if k.startswith(('block.','biome.'))}
        write(OUT/f'assets/{NS}/lang/{p.name}',v)
    for b in inv['blocks']:
        if b.endswith('_leaves'):template='oak_leaves'
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
        if b=='locust_hollow_log':decl['properties']['occludes']=False
        if b=='large_water_lily':decl['shapes']={'default':[[1/16,9/16,1/16,15/16,10.5/16,15/16]]}
        if b in ('ghost_fire_mushroom','ghost_fire_mushroom_block','eugune_ghost_fire'):decl['properties']['light']=8
        if b=='golden_lotus':decl['default_state']={'part':'single','size':'0'}
        write(OUT/f'data/{NS}/horizon/block/{b}.json',decl)
    write(OUT/f'data/{NS}/horizon/creative_tab/nature.json',{'title':'Gensokyo Legacy','icon':NS+':star_flower','items':[NS+':'+b for b in inv['blocks']]})
    from overworld import add
    add(OUT,write)
    from normalize import apply
    apply(OUT,write)
    from namespace import rebase
    rebase(OUT)
    licenses=OUT/'assets/horizon_gensokyo_legacy/licenses';licenses.mkdir(parents=True,exist_ok=True)
    for name,dest in [('LICENSE','license.txt'),('THIRD_PARTY_NOTICES.md','third_party_notices.txt')]:shutil.copy2(ROOT/name,licenses/dest)
    print(f'Assembled {len(BIOMES)} biomes, {len(inv["blocks"])} blocks, {inv["templates"]} templates')
if __name__=='__main__':main()
