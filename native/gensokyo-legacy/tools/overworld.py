"""Overworld adaptation: a separate, bounded regional pool alongside BOP/Tectonic."""
import json
from import_jar import NS,BIOMES

def add(out,write):
    entries=[]
    # Inland lowlands: three climate columns and three vegetation bands.
    rows=[['myouren_temple_barrens','plain_of_the_past','sunflower_plains'],['muenzuka','sakura_forest','hill_of_the_nameless'],['magical_forest','bamboo_forest_of_the_lost','youkai_mountain_foothills']]
    for row,names in enumerate(rows):
        for col,name in enumerate(names):
            entries.append(entry(name,[-.45+col*.3,-.15+col*.3],[-1+row*2/3,-1+(row+1)*2/3],[.0,1.0],[0.0,1.0]))
    for i,name in enumerate(['untrodden_valley','genbu_ravine','sanctuary','tengu_settlement','secret_heaven_cliff','youkai_mountain_summit']):
        entries.append(entry(name,[-.45,.55],[-1+2*(i%3)/3,-1+2*((i%3)+1)/3],[.2,1.0],([-.4,0.0] if i<3 else [-1.0,-.4])))
    entries.append(entry('misty_lake',[-.45,.55],[-1,1],[-.45,-.19],[-1,1]))
    entries.append(entry('misty_lake_shore',[-.45,.55],[-1,1],[-.19,0.0],[-1,1]))
    assert {e['biome'].split(':')[1] for e in entries}==set(BIOMES)
    # Surface and underground surface samples share the climate candidate.
    entries += [dict(e,parameters=dict(e['parameters'],depth=[1.,1.])) for e in entries[:]]
    write(out/f'data/{NS}/worldgen/biome_placement/overworld.json',{'preset':'minecraft:overworld','group':NS+':overworld','weight':4,'selector':'weirdness','biomes':entries,'region_order':5,'region_zooms':3,'vanilla_weight':10,'region_min_quarts':192,'climate_guard':True,'configuration':{'file':NS+'/generation.toml','key':'overworld.region_weight','scale_key':'overworld.biome_scale'}})
    # Reuse the source's biome-specific rules, retaining their enclosing floor/water guards.
    source=json.loads((out.parent/'source_surface.json').read_text())
    write(out/f'data/{NS}/worldgen/material_rule/overworld.json',source)
    write(out/f'data/{NS}/worldgen/generation_extension/overworld.json',{'settings':'minecraft:overworld','surface_rule':NS+':overworld'})
    tags={'is_forest':['magical_forest','sakura_forest','bamboo_forest_of_the_lost','muenzuka','youkai_mountain_foothills','sanctuary','tengu_settlement'],'is_plains':['plain_of_the_past','myouren_temple_barrens','hill_of_the_nameless','sunflower_plains'],'is_mountain':['untrodden_valley','genbu_ravine','youkai_mountain_foothills','sanctuary','tengu_settlement','secret_heaven_cliff','youkai_mountain_summit'],'is_beach':['misty_lake_shore'],'is_ocean':['misty_lake']}
    for tag,names in tags.items():write(out/f'data/minecraft/tags/worldgen/biome/{tag}.json',{'replace':False,'values':[NS+':'+n for n in names]})
def entry(name,t,h,c,e):
    return {'biome':NS+':'+name,'parameters':{'temperature':t,'humidity':h,'continentalness':c,'erosion':e,'depth':[0.,0.],'weirdness':[-1.,1.],'offset':0.}}
