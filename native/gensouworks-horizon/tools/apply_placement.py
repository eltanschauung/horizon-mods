"""Deterministic per-structure biome and frequency policy for the local port."""
import json
from pathlib import Path
RULES = {
    'koumakan': ('#c:is_plains', 0.06),
    'eientei': ('#minecraft:is_jungle', 0.06),
    'margatroid_house': ('#minecraft:is_forest', 0.09),
    'kourindou': ('#minecraft:is_forest', 0.09),
    'kirisame_house': ('#minecraft:is_forest', 0.09),
    'sanyousei_new_house': ('#minecraft:is_forest', 0.07),
    'signal_tower': ('#minecraft:is_forest', 0.06),
}
def apply(root):
    root = Path(root)
    def put(path, data):
        p=root/path;p.parent.mkdir(parents=True,exist_ok=True)
        p.write_text(json.dumps(data,indent=2)+'\n')
    for index, (name, (biome, frequency)) in enumerate(RULES.items()):
        path=f'data/gensouworks/worldgen/structure/{name}.json'
        data=json.loads((root/path).read_text());data['biomes']=biome;put(path,data)
        put(f'data/gensouworks/worldgen/structure_set/{name}.json', {
            'structures':[{'structure':'gensouworks:'+name,'weight':1}],
            'placement':{'type':'minecraft:random_spread','spacing':32,'separation':8,
                'salt':20260805+index,'frequency':frequency,'frequency_reduction_method':'default'}})
    # Leave the unrequested cave at its former one-in-eight share and original salt.
    put('data/gensouworks/worldgen/structure_set/all.json', {
        'structures':[{'structure':'gensouworks:larva_cave','weight':1}],
        'placement':{'type':'minecraft:random_spread','spacing':32,'separation':8,
            'salt':20260804,'frequency':0.125,'frequency_reduction_method':'default'}})
    # Common-tag fallback; contributions from other enabled packs remain merged.
    put('data/c/tags/worldgen/biome/is_plains.json', {
        'replace':False,'values':['minecraft:plains','minecraft:sunflower_plains']})
if __name__=='__main__':
    import sys
    apply(Path(sys.argv[1]))
