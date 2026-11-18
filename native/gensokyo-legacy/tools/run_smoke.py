#!/usr/bin/env python3
"""Run headless biome loading, generation, and save/reopen checks in scratch data."""
import os,subprocess,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];launcher=Path.home()/'.local/share/horizon-launcher';deps=launcher/'fork-native/target/fast/deps';work=ROOT/'.local/smoke';work.mkdir(parents=True,exist_ok=True)
a=[str(launcher/'rust/1.99.1-x86_64-unknown-linux-gnu/bin/rustc'),'--edition=2024',str(ROOT/'tools/smoke.rs'),'-L','dependency='+str(deps),'-o',str(work/'smoke')]
for name in ['horizon_core','horizon_world','horizon_mods','horizon_client_lib','serde_json']:
 p=max(deps.glob('lib'+name+'-*.rlib'),key=lambda p:p.stat().st_mtime);a+=['--extern',name+'='+str(p)]
for p in (launcher/'fork-native/target/fast/build').glob('*/out'):a+=['-L','native='+str(p)]
subprocess.run(a,check=True)
base=max((p/'.local/horizon-client/26.3' for p in (launcher/'fork-versions').iterdir() if (p/'.local/horizon-client/26.3/facts.json').exists()),key=lambda p:p.stat().st_mtime)
pack=ROOT/'content/target/horizon-mod/horizon_gensokyo_legacy.hmod'
biomes=json.loads((ROOT/'inventory.json').read_text())['biomes']
if os.environ.get('GENSOKYO_PREPARE_ONLY'):biomes=biomes[:1]
expected={}
for p in (ROOT/'upstream/data/gensokyolegacy/worldgen/biome').glob('*.json'):
 expected['horizon_gensokyo_legacy:'+p.stem]={k:v for k,v in json.loads(p.read_text())['effects'].items() if k.endswith('_color')}
# The forest now intentionally uses the current vanilla jungle's colormap.
expected.pop('horizon_gensokyo_legacy:bamboo_forest_of_the_lost')
os.environ['GENSOKYO_EXPECT_TINTS']=json.dumps(expected)
for biome in biomes:
 print('CHECK',biome,flush=True)
 with (work/(biome+'.log')).open('w') as log:
  subprocess.run([work/'smoke',base,work/'world',pack,'false','horizon_gensokyo_legacy:'+biome],stdout=log,stderr=subprocess.STDOUT,check=True)
 print((work/(biome+'.log')).read_text()[-700:],flush=True)
