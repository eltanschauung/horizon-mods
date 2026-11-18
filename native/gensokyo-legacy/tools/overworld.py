"""Overworld adaptation: a separate, bounded regional pool alongside BOP/Tectonic."""
import json
from import_jar import NS,BIOMES

def add(out,write):
    entries=[]
    # Inland lowlands: three climate columns and three vegetation bands.
    rows=[['myouren_temple_barrens','plain_of_the_past','sunflower_plains'],[None,'sakura_forest','hill_of_the_nameless'],[None,None,'youkai_mountain_foothills']]
    for row,names in enumerate(rows):
        for col,name in enumerate(names):
            if name is None:
                continue
            humidity = [-1+row*2/3,-1+(row+1)*2/3]
            # Leave the transition band to Forest of Magic in the shared
            # temperate lowlands so the two candidates cannot tie there.
            bands = ([humidity[0], .1], [.3, humidity[1]]) if name == 'sakura_forest' else [humidity]
            for band in bands:
                entries.append(entry(name,[-.45+col*.3,-.15+col*.3],band,[.0,1.0],[0.0,1.0]))
    # Use the cool-to-warm Oak Forest bands and extend into damper forest.
    # In the central temperature band, leave Spider Lily Field's humidity to Muenzuka.
    # Keep room for swamps in the low valleys and mountain biomes on steep terrain.
    forest_bands = [
        ([-.45, -.15], [-.33, .38]),
        ([-.15, .2], [.1, .3]),
        ([.2, .55], [-.33, .38]),
    ]
    for temperature, humidity in forest_bands:
        entries.append(entry('magical_forest',temperature,humidity,[-.19,1.0],[-.35,.5]))
    # Compete across the jungle bands, including their cooler, drier and warmer margins.
    entries.append(entry('bamboo_forest_of_the_lost',[.14,.66],[.08,1.0],[-.19,1.0],[-.78,.90]))
    entries.append(entry('bamboo_forest_of_the_lost',[-.13,.14],[.36,1.0],[0,1.0],[-.35,1.0]))
    # Use the BOP Spider Lily Field's surface climate, including its terrain bands.
    # The local snapshot keeps Gensokyo independently buildable and installable.
    spider_lily = json.loads((out.parent/'spider_lily_climate.json').read_text())
    for continentalness, erosion, weirdness in spider_lily:
        entries.append(entry('muenzuka',[-.15,.2],[-.1,.1],continentalness,erosion,weirdness))
    for i,name in [(0,'untrodden_valley'),(2,'sanctuary'),(3,'tengu_settlement'),(4,'secret_heaven_cliff'),(5,'youkai_mountain_summit')]:
        entries.append(entry(name,[-.45,.55],[-1+2*(i%3)/3,-1+2*((i%3)+1)/3],[.2,1.0],([-.4,0.0] if i<3 else [-1.0,-.4])))
    # Vanilla Swamp's three weirdness bands with slightly wider bounds.
    for valley in [(-1.,-.90),(-.43,.43),(.90,1.)]:
        entries.append(entry('genbu_ravine',[-.45,.25],[-1,1],[-.15,1.0],[.5,1.0],valley))
    entries.append(entry('misty_lake',[-.45,.55],[-1,1],[-.45,-.19],[-1,1]))
    # Leave vanilla's river band open where the shore extends inland.
    for shore in [(-1.,-.05),(.05,1.)]:
        entries.append(entry('misty_lake_shore',[-.45,.55],[-1,1],[-.19,0.0],[-1,1],shore))
    assert {e['biome'].split(':')[1] for e in entries}==set(BIOMES)
    # Surface and underground surface samples share the climate candidate.
    entries += [dict(e,parameters=dict(e['parameters'],depth=[1.,1.])) for e in entries[:]]
    write(out/f'data/{NS}/worldgen/biome_placement/overworld.json',{'preset':'minecraft:overworld','group':NS+':overworld','weight':4,'selector':'weirdness','biomes':entries,'region_order':5,'region_zooms':3,'vanilla_weight':10,'region_min_quarts':192,'climate_guard':True,'configuration':{'file':'horizon_gensokyo_legacy/generation.toml','key':'overworld.region_weight','scale_key':'overworld.biome_scale'}})
    # Reuse the source's biome-specific rules, retaining their enclosing floor/water guards.
    source=json.loads((out.parent/'source_surface.json').read_text())
    write(out/f'data/{NS}/worldgen/material_rule/overworld.json',source)
    write(out/f'data/{NS}/worldgen/generation_extension/overworld.json',{'settings':'minecraft:overworld','surface_rule':NS+':overworld'})
    tags={'is_jungle':['bamboo_forest_of_the_lost'],'is_forest':['magical_forest','sakura_forest','bamboo_forest_of_the_lost','muenzuka','youkai_mountain_foothills','sanctuary','tengu_settlement'],'is_plains':['plain_of_the_past','myouren_temple_barrens','hill_of_the_nameless','sunflower_plains'],'is_mountain':['untrodden_valley','youkai_mountain_foothills','sanctuary','tengu_settlement','secret_heaven_cliff','youkai_mountain_summit'],'is_beach':['misty_lake_shore'],'is_ocean':['misty_lake']}
    for tag,names in tags.items():write(out/f'data/minecraft/tags/worldgen/biome/{tag}.json',{'replace':False,'values':[NS+':'+n for n in names]})
def entry(name,t,h,c,e,weirdness=(-1.,1.)):
    return {'biome':NS+':'+name,'parameters':{'temperature':t,'humidity':h,'continentalness':c,'erosion':e,'depth':[0.,0.],'weirdness':weirdness,'offset':0.}}
