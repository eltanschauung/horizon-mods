"""Lossless 1.20 feature codec migration and explicit expansion of custom post-features."""
import json

def migrate(v):
    if isinstance(v,list):return [migrate(x) for x in v]
    if not isinstance(v,dict):return v
    v={k:migrate(x) for k,x in v.items()}
    if 'Name' in v:
        v['id']=v.pop('Name')
        if 'Properties' in v:v['properties']=v.pop('Properties')
    if v.get('id')=='minecraft:grass':v['id']='minecraft:short_grass'
    if 'type' in v and isinstance(v.get('config'),dict):
        config=v.pop('config');v.update(config)
    if v.get('type') in ('minecraft:uniform','minecraft:biased_to_bottom','minecraft:clamped','minecraft:clamped_normal','minecraft:trapezoid') and isinstance(v.get('value'),dict):v.update(v.pop('value'))
    if v.get('type')=='minecraft:tree' and 'dirt_provider' in v:v['below_trunk_provider']=v.pop('dirt_provider')
    kinds={'minecraft:simple_state_provider':'minecraft:simple','minecraft:weighted_state_provider':'minecraft:weighted'}
    if v.get('type') in kinds:v['type']=kinds[v['type']]
    if 'type' not in v and 'fallback' in v and 'rules' in v:v['type']='minecraft:rule_based'
    if v.get('type')=='minecraft:random_patch':
        def triangular(n):return {'type':'minecraft:trapezoid','min':-n,'max':n,'plateau':0}
        return {'type':'minecraft:sequence','features':[{'feature':{'type':'minecraft:sequence','features':[v['feature']]},'placement':[{'type':'minecraft:count','count':v['tries']},{'type':'minecraft:offset','x':triangular(v['xz_spread']),'y':triangular(v['y_spread']),'z':triangular(v['xz_spread'])}]}]}
    return v

def apply(root,write):
    for p in (root/'data/gensokyolegacy/worldgen').rglob('*.json'):write(p,migrate(json.loads(p.read_text())))
    feature=root/'data/gensokyolegacy/worldgen/configured_feature'
    placed=root/'data/gensokyolegacy/worldgen/placed_feature'
    docs={p: migrate(json.loads(p.read_text())) for p in [*feature.rglob('*.json'),*placed.rglob('*.json')]}
    def inline(v,depth=0):
        assert depth<128,'Feature cycle'
        if isinstance(v,list):return [inline(x,depth+1) for x in v]
        if not isinstance(v,dict):return v
        out={}
        for key,x in v.items():
            if isinstance(x,str) and x.startswith('gensokyolegacy:') and key in ('feature','vegetation_feature'):
                name=x.split(':')[1]
                path=(feature if key=='feature' else placed)/(name+'.json')
                if path in docs:x=docs[path]
            out[key]=inline(x,depth+1)
        return out
    for p in (root/'data/gensokyolegacy/worldgen/biome').glob('*.json'):
        biome=json.loads(p.read_text());effects=biome.get('effects',{});attributes=biome.setdefault('attributes',{})
        if isinstance(biome.get('carvers'),dict):biome['carvers']=[x for group in biome['carvers'].values() for x in group]
        for key in ('sky_color','fog_color','water_fog_color'):
            if key in effects:attributes['minecraft:visual/'+key]=effects.pop(key)
        attributes['minecraft:gameplay/natural_mob_spawns']={'spawns_by_category':biome.pop('spawners',{}),'spawn_costs':biome.pop('spawn_costs',{})}
        biome['features']=[[('minecraft:trees_birch_and_oak_leaf_litter' if x=='minecraft:trees_birch_and_oak' else x) for x in row] for row in biome['features']]
        write(p,biome)
    aliases={'minecraft:patch_grass':'minecraft:patch_grass_plain','minecraft:patch_taiga_grass':'minecraft:patch_taiga_grass','minecraft:patch_large_fern':'minecraft:patch_large_fern'}
    def alias(v):
        if isinstance(v,list):return [alias(x) for x in v]
        if not isinstance(v,dict):return v
        v={k:alias(x) for k,x in v.items()}
        if v.get('feature') in aliases if isinstance(v.get('feature'),str) else False:
            v['feature']={'type':'minecraft:sequence','features':[{'horizon:placed_feature':aliases[v['feature']],'horizon:placement_after':'minecraft:biome'}]}
        return v
    # Inline holders inside custom algorithms; the host catalog expands vanilla holders normally.
    for p,v in docs.items():write(p,alias(inline(v)))
