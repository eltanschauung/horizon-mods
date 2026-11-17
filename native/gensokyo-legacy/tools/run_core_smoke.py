#!/usr/bin/env python3
"""Compile a direct core generator probe without rebuilding the renderer/client library."""
from pathlib import Path
import subprocess,json,os
os.environ['HORIZON_WORLDGEN_DIAGNOSTICS']='1'
ROOT=Path(__file__).resolve().parents[1];launcher=Path.home()/'.local/share/horizon-launcher';deps=launcher/'fork-native/target/fast/deps';work=ROOT/'.local/smoke'
a=[str(launcher/'rust/1.99.1-x86_64-unknown-linux-gnu/bin/rustc'),'--edition=2024',str(ROOT/'tools/core_smoke.rs'),'-L','dependency='+str(deps),'-o',str(work/'core-smoke')]
for n in ['horizon_world','horizon_core','serde_json']:
 p=max(deps.glob('lib'+n+'-*.rlib'),key=lambda p:p.stat().st_mtime);a+=['--extern',n+'='+str(p)]
for p in (launcher/'fork-native/target/fast/build').glob('*/out'):a+=['-L','native='+str(p)]
subprocess.run(a,check=True)
resources=Path((work/'combined-root.txt').read_text().strip()) if '--combined' in __import__('sys').argv else max((work/'world/cache/mod-content').iterdir(),key=lambda p:p.stat().st_mtime)
biomes=json.loads((ROOT/'inventory.json').read_text())['biomes']
if '--natural' in __import__('sys').argv:biomes=['--natural']
if os.environ.get('GENSOKYO_TEST_BIOME'):biomes=[os.environ['GENSOKYO_TEST_BIOME']]
for b in biomes:
 print('CHECK',b,flush=True)
 with (work/(b+'-core.log')).open('w') as log:
  result=subprocess.run([work/'core-smoke',resources,('--natural' if b=='--natural' else 'horizon_gensokyo_legacy:'+b)],stdout=log,stderr=subprocess.STDOUT)
 print((work/(b+'-core.log')).read_text()[-4000:],flush=True)
 if result.returncode:raise SystemExit(result.returncode)
