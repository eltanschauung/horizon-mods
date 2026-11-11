"""Convert the supplied Gensou Works JAR to dependency-free Horizon content.
Build-time dependency: nbtlib. Original structures remain ZZY's, All Rights Reserved.
"""
from pathlib import Path
import argparse,collections,gzip,io,json,hashlib,zipfile
import nbtlib as n
p=argparse.ArgumentParser();p.add_argument('jar',type=Path);p.add_argument('--resources',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();out=a.output
facts=json.loads((a.resources/'facts.json').read_text());known={s[0] for s in facts['states']};defaults={k:facts['states'][v][4].get('properties',{}) for k,v in facts['defaults'].items()};allowed=collections.defaultdict(lambda:collections.defaultdict(set))
for s in facts['states']:
 for k,v in s[4].get('properties',{}).items():allowed[s[0]][k].add(v)
def put(path,data):
 path=out/path;path.parent.mkdir(parents=True,exist_ok=True)
 if isinstance(data,bytes):path.write_bytes(data)
 else:path.write_text(json.dumps(data,indent=2,ensure_ascii=False)+'\n')
def replacement(old):
 if old in known:return old
 if old=='minecraft:chain':return 'minecraft:iron_chain'
 q=old.split(':')[-1]
 if 'dining_chair' in q:return 'minecraft:'+next((w+'_stairs' for w in ['dark_oak','mangrove','spruce','oak'] if q.startswith(w)), 'oak_stairs')
 if 'dining_table' in q:return 'minecraft:'+next((w+'_fence' for w in ['dark_oak','mangrove','spruce','oak'] if q.startswith(w)), 'oak_fence')
 if 'cabinet' in q or 'crate' in q or 'tank' in q or 'basket' in q:return 'minecraft:barrel'
 if 'tatami' in q or 'rug' in q:return 'minecraft:yellow_carpet'
 if 'bag' in q or 'bale' in q:return 'minecraft:hay_block'
 if 'stove' in q:return 'minecraft:smoker'
 if 'pot' in q or 'basin' in q or 'skillet' in q or 'kettle' in q:return 'minecraft:cauldron'
 if 'lantern' in q:return 'minecraft:lantern'
 if q=='waystone':return 'minecraft:chiseled_stone_bricks'
 if 'trunk' in q:return 'minecraft:oak_log'
 if any(k in q for k in ['branches','fruits','vines']):return 'minecraft:oak_leaves'
 if 'wild_' in q or 'shrub' in q:return 'minecraft:fern'
 if 'rope' in q:return 'minecraft:iron_bars'
 if 'rack' in q or 'shelf' in q:return 'minecraft:bookshelf'
 if 'computer' in q:return 'minecraft:observer'
 if 'cutting_board' in q or 'keyboard' in q or 'gomoku' in q:return 'minecraft:oak_pressure_plate'
 if 'farmland' in q:return 'minecraft:farmland'
 if any(k in q for k in ['soup','stew','ham','chicken','burger','tofu','yokan','lune','loin','cod','borscht','pastitsio']):return 'minecraft:cake'
 return 'minecraft:flower_pot'
remaps={};counts=collections.Counter();sizes={};foreign_items=0
def sanitize(tag):
 global foreign_items
 if isinstance(tag,n.Compound):
  for key in list(tag):
   value=tag[key]
   if key=='LootTable' and str(value).startswith('gensouworks:'):
    tag[key]=n.String('minecraft:chests/simple_dungeon');counts['loot_fallbacks']+=1
   elif key=='id' and isinstance(value,n.String) and ':' in str(value) and not str(value).startswith('minecraft:'):
    tag[key]=n.String('minecraft:air');foreign_items+=1
   else:sanitize(value)
 elif isinstance(tag,n.List):
  for value in tag:sanitize(value)
biomes=set()
with zipfile.ZipFile(a.jar) as z:
 structures={Path(k).stem:json.loads(z.read(k))for k in z.namelist() if k.startswith('data/gensouworks/worldgen/structure/')and k.endswith('.json')}
 assert len(structures)==8
 for k in z.namelist():
  if k.startswith('data/gensouworks/tags/worldgen/biome/') and k.endswith('.json'):
   for v in json.loads(z.read(k)).get('values',[]):
    if isinstance(v,str):biomes.add(v if ':' in v else ('#minecraft:'+v[1:] if v.startswith('#')else'minecraft:'+v))
 for k in z.namelist():
  if k.startswith('data/gensouworks/worldgen/template_pool/')and k.endswith('.json'):put(k,json.loads(z.read(k)))
  if not k.startswith('data/gensouworks/structures/')or not k.endswith('.nbt'):continue
  t=n.File.parse(io.BytesIO(gzip.decompress(z.read(k))));palette=t['palette'];sizes[k]=list(map(int,t['size']))
  foreign_indices=set()
  for index,entry in enumerate(palette):
   old=str(entry['Name']);new=replacement(old);assert new in known,(old,new)
   if old!=new:remaps[old]=new;foreign_indices.add(index)
   entry['Name']=n.String(new)
   props={str(k):str(v)for k,v in entry.get('Properties',{}).items()}
   props={k:v for k,v in props.items()if v in allowed[new].get(k,set())}
   if props:entry['Properties']=n.Compound({k:n.String(v)for k,v in props.items()})
   else:entry.pop('Properties',None)
  for block in t.get('blocks',[]):
   if 'nbt' not in block:continue
   if int(block['state'])in foreign_indices:
    mapped=str(palette[int(block['state'])]['Name'])
    if mapped in ['minecraft:barrel','minecraft:smoker']:
     block['nbt']['id']=n.String(mapped)
    else:block.pop('nbt');continue
   sanitize(block['nbt'])
  entities=[]
  for entity in t.get('entities',[]):
   if not str(entity.get('nbt',{}).get('id','')).startswith('minecraft:'):counts['removed_entities']+=1;continue
   sanitize(entity);entities.append(entity)
  t['entities']=n.List[n.Compound](entities)
  data=io.BytesIO();t.write(data);put(k.replace('/structures/','/structure/'),gzip.compress(data.getvalue(),mtime=0));counts['templates']+=1
 for name,s in structures.items():
  s['biomes']='#gensouworks:has_structure/all';put('data/gensouworks/worldgen/structure/'+name+'.json',s)
 # Equal eligibility, not only equal weights, avoids the original rare/biome bias.
 biomes.discard('#minecraft:is_hill')
 put('data/gensouworks/tags/worldgen/biome/has_structure/all.json',{'replace':False,'values':sorted(biomes)})
 put('data/gensouworks/worldgen/structure_set/all.json',{'structures':[{'structure':'gensouworks:'+name,'weight':1}for name in sorted(structures)],'placement':{'type':'minecraft:random_spread','spacing':32,'separation':8,'salt':20260804}})
from apply_placement import apply
apply(out)
put('conversion-report.json',{'jar_sha256':hashlib.sha256(a.jar.read_bytes()).hexdigest(),'structures':sorted(structures),'template_sizes':sizes,'block_substitutions':remaps,'counts':dict(counts),'foreign_items_removed':foreign_items})
print(json.dumps({'structures':len(structures),'templates':counts['templates'],'block_substitutions':len(remaps),'loot_fallbacks':counts['loot_fallbacks']}))
