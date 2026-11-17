#!/usr/bin/env python3
"""Package the maintained content snapshot and native generation bundle."""
import argparse,json,subprocess,zipfile,shutil,os
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def main():
 p=argparse.ArgumentParser();p.add_argument('--assemble',action='store_true');p.add_argument('--install',action='store_true');a=p.parse_args()
 workspace=next(x for x in ROOT.parents if (x/'horizon/tools/fork.py').is_file());engine=workspace/'horizon'
 if a.assemble:subprocess.run(['python3',ROOT/'tools/assemble.py'],check=True)
 tool=workspace/'Lunatic/target/tools/debug/cargo-horizon-mod'
 if not tool.is_file():raise SystemExit('Build cargo-horizon-mod with the managed cache first')
 subprocess.run([tool,'build'],cwd=ROOT/'content',check=True)
 output=ROOT/'target/horizon-mod';output.mkdir(parents=True,exist_ok=True)
 content=output/'horizon_gensokyo_legacy.hmod';shutil.copy2(ROOT/'content/target/horizon-mod'/content.name,content)
 subprocess.run([tool,'check',content,'--json'],check=True)
 revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=engine,text=True).strip()
 manifest={'hooks':359,'schema':1,'id':'horizon_gensokyo_legacy','version':'0.1.0','game':'26.3','horizon':revision,'mixins':'5a64af938ec86b00d53d16a408aabc0038fb3e57'}
 native=output/'horizon_gensokyo_legacy.hnmod'
 with zipfile.ZipFile(native,'w',zipfile.ZIP_DEFLATED,compresslevel=9) as z:
  def add(n,b):
   info=zipfile.ZipInfo(n,(1980,1,1,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;info.external_attr=0o100644<<16;z.writestr(info,b)
  add('horizon-native.json',json.dumps(manifest,indent=2).encode())
  for f in sorted((ROOT/'mixins').rglob('*')):
   if f.is_file():add('mixins/'+f.relative_to(ROOT/'mixins').as_posix(),f.read_bytes())
  add('licenses/GensokyoLegacy-LGPL-2.1.txt',(ROOT/'LICENSE').read_bytes());add('licenses/THIRD_PARTY_NOTICES.md',(ROOT/'THIRD_PARTY_NOTICES.md').read_bytes())
 if a.install:
  game=Path.home()/'.local/share/horizon'
  for src,folder in [(content,'horizon-mods'),(native,'horizon-native-mods')]:
   dest=game/folder/src.name;dest.parent.mkdir(parents=True,exist_ok=True);temp=dest.with_suffix(dest.suffix+'.new');shutil.copy2(src,temp);temp.replace(dest)
  flags=game/'horizon-mods.txt';lines=flags.read_text().splitlines() if flags.exists() else [];key='mod.horizon_gensokyo_legacy:';lines=[l for l in lines if not l.startswith(key)];lines.append(key+'true');temp=flags.with_suffix('.txt.new');temp.write_text('\n'.join(lines)+'\n');temp.replace(flags)
 print(json.dumps({'content':str(content),'native':str(native),'revision':revision,'installed':a.install}))
if __name__=='__main__':main()
