#!/usr/bin/env python3
import subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];launcher=Path.home()/'.local/share/horizon-launcher';deps=launcher/'fork-native/target/fast/deps';work=ROOT/'.local/smoke';work.mkdir(parents=True,exist_ok=True)
a=[str(launcher/'rust/1.99.1-x86_64-unknown-linux-gnu/bin/rustc'),'--edition=2024',str(ROOT/'tools/prepare.rs'),'-L','dependency='+str(deps),'-o',str(work/'prepare')]
for n in ['horizon_client_lib','horizon_mods']:
 p=max(deps.glob('lib'+n+'-*.rlib'),key=lambda p:p.stat().st_mtime);a+=['--extern',n+'='+str(p)]
for p in (launcher/'fork-native/target/fast/build').glob('*/out'):a+=['-L','native='+str(p)]
subprocess.run(a,check=True)
base=max((p/'.local/horizon-client/26.3' for p in (launcher/'fork-versions').iterdir() if (p/'.local/horizon-client/26.3/facts.json').exists()),key=lambda p:p.stat().st_mtime)
packs=[ROOT/'content/target/horizon-mod/horizon_gensokyo_legacy.hmod']
if '--bop' in sys.argv:packs.append(Path.home()/'.local/share/horizon/horizon-mods/biomesoplenty.hmod')
cache=work/('combined' if '--bop' in sys.argv else 'world')/'cache'
result=subprocess.check_output([work/'prepare',base,cache,*packs],text=True);print(result,end='')
(work/('combined-root.txt' if '--bop' in sys.argv else 'root.txt')).write_text(result.split('root=')[-1].strip())
