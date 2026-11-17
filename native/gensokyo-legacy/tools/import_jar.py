#!/usr/bin/env python3
"""Import the requested biome dependency closure from the user's Gensokyo Legacy jar.
Never installs content. Unknown dependencies are errors, not silent substitutions.
"""
import argparse, collections, gzip, hashlib, io, json, struct
from pathlib import Path
from zipfile import ZipFile
ROOT = Path(__file__).resolve().parents[1]
BIOMES = '''magical_forest sakura_forest bamboo_forest_of_the_lost muenzuka
plain_of_the_past myouren_temple_barrens hill_of_the_nameless sunflower_plains
misty_lake misty_lake_shore untrodden_valley genbu_ravine youkai_mountain_foothills
sanctuary tengu_settlement secret_heaven_cliff youkai_mountain_summit'''.split()
NS = 'gensokyolegacy'
def strings(v):
    if isinstance(v, str): yield v
    elif isinstance(v, list):
        for x in v: yield from strings(x)
    elif isinstance(v, dict):
        for x in v.values(): yield from strings(x)
def read_nbt(b):
    f = io.BytesIO(gzip.decompress(b) if b[:2] == b'\x1f\x8b' else b)
    def n(t): return struct.unpack('>'+t, f.read(struct.calcsize('>'+t)))[0]
    def s(): return f.read(n('H')).decode()
    def tag(t):
        if 1 <= t <= 6: return n({1:'b',2:'h',3:'i',4:'q',5:'f',6:'d'}[t])
        if t == 7: return f.read(n('i'))
        if t == 8: return s()
        if t == 9:
            k=n('b'); return [tag(k) for _ in range(n('i'))]
        if t == 10:
            d={}
            while (k:=n('b')): key=s();d[key]=tag(k)
            return d
        if t in (11,12): return [n('i' if t==11 else 'q') for _ in range(n('i'))]
        raise ValueError(t)
    t=n('b');s();return tag(t)
def main():
    p=argparse.ArgumentParser();p.add_argument('jar',type=Path);a=p.parse_args()
    with ZipFile(a.jar) as z:
        available=set(z.namelist());seen={};templates={};blocks=set();todo=[f'data/{NS}/worldgen/biome/{n}.json' for n in BIOMES]
        def add_refs(v):
            for s in strings(v):
                if not s.startswith(NS+':'):continue
                name=s.split(':',1)[1]
                if f'assets/{NS}/blockstates/{name}.json' in available:blocks.add(name)
                for kind in ('placed_feature','configured_feature','processor_list','noise'):
                    q=f'data/{NS}/worldgen/{kind}/{name}.json'
                    if q in available and q not in seen:todo.append(q)
                q=f'data/{NS}/structures/{name}.nbt'
                if name in ('marisa_house','morichika_shop','hakurei_shrine','alice_house'):continue
                if q in available and q not in templates:
                    data=z.read(q);templates[q]=data;add_refs(read_nbt(data))
        while todo:
            q=todo.pop()
            if q in seen:continue
            v=json.loads(z.read(q));seen[q]=v;add_refs(v)
        source_surface=json.loads(z.read(f'data/{NS}/worldgen/noise_settings/gensokyo.json'))['surface_rule']
        def prune(v):
            if not isinstance(v,dict):return None
            if v.get('if_true',{}).get('type')=='minecraft:biome':
                ids=v['if_true']['biome_is'];ids=[ids] if isinstance(ids,str) else ids
                return v if any(i in {NS+':'+b for b in BIOMES} for i in ids) else None
            if 'sequence' in v:
                children=[p for c in v['sequence'] if (p:=prune(c))]
                return dict(v,sequence=children) if children else None
            if 'then_run' in v:
                child=prune(v['then_run']);return dict(v,then_run=child) if child else None
            return None
        surface=prune(source_surface)
        (ROOT/'source_surface.json').write_text(json.dumps(surface,indent=2)+'\n')
        add_refs(surface)
        while todo:
            q=todo.pop()
            if q in seen:continue
            v=json.loads(z.read(q));seen[q]=v;add_refs(v)
        # Preserve original assets and tags, but import only the biome/generation closure.
        files={n:z.read(n) for n in available if n.startswith(f'assets/{NS}/') and not n.endswith('/')}
        for n in available:
            if n.startswith(('data/'+NS+'/tags/','data/minecraft/tags/','data/c/tags/')) and n.endswith('.json'):
                tag=json.loads(z.read(n));values=[]
                for entry in tag.get('values',[]):
                    value=entry if isinstance(entry,str) else entry.get('id','')
                    if value.startswith(NS+':'):
                        path=value.split(':')[1]
                        if path not in blocks and path not in BIOMES:continue
                    elif value.startswith('#'+NS+':'):pass
                    elif n.startswith('data/'+NS+'/'):pass
                    else:continue
                    values.append(entry)
                if values:files[n]=(json.dumps(dict(tag,values=values),indent=2)+'\n').encode()
        files.update({n:(json.dumps(v,indent=2)+'\n').encode() for n,v in seen.items()});files.update(templates)
        files['META-INF/mods.toml']=z.read('META-INF/mods.toml')
        out=ROOT/'upstream'
        for n,b in files.items():
            target=out/n;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(b)
        kinds=collections.Counter()
        def count(v):
            if isinstance(v,dict):
                if 'type' in v:kinds[v['type']]+=1
                for x in v.values():count(x)
            elif isinstance(v,list):
                for x in v:count(x)
        for v in seen.values():count(v)
        report={'jar':a.jar.name,'sha256':hashlib.sha256(a.jar.read_bytes()).hexdigest(),'biomes':BIOMES,'blocks':sorted(blocks),'generation_files':len(seen),'templates':len(templates),'types':dict(sorted(kinds.items())),'hashes':{n:hashlib.sha256(b).hexdigest() for n,b in sorted(files.items())}}
        (ROOT/'inventory.json').write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps({k:v for k,v in report.items() if k not in ('hashes','types')},indent=2))
if __name__=='__main__':main()
