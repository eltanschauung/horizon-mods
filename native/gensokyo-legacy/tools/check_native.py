#!/usr/bin/env python3
"""Compile the port in an isolated engine stage using the existing managed cache."""
import os,subprocess,tarfile,io,shutil,sys,zipfile,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];WORK=ROOT/'.local/native-check'
ENGINE=next(p/'horizon' for p in ROOT.parents if (p/'horizon/tools/fork.py').is_file())
source=WORK/'source';stage=WORK/'stage'
if source.exists():shutil.rmtree(source)
source.mkdir(parents=True)
# Tracked source plus the two pending extension changes; never modify the real checkout.
b=subprocess.check_output(['git','archive','HEAD'],cwd=ENGINE)
with tarfile.open(fileobj=io.BytesIO(b)) as t:t.extractall(source,filter='data')
for link in source.rglob('*'):
 if link.is_symlink():
  data=link.read_bytes();link.unlink();link.write_bytes(data)
for name in ['crates/horizon-world/src/worldgen/feature_graph.rs','crates/horizon-world/src/worldgen/placement.rs','crates/horizon-world/src/worldgen/native_feature.rs','crates/horizon-world/src/worldgen.rs','crates/horizon-world/src/worldgen/placement/context.rs','crates/horizon-world/src/worldgen/feature_catalog.rs','crates/horizon-world/src/worldgen/generator_decoration.rs','crates/horizon-world/src/worldgen/placement/chain.rs','crates/horizon-world/src/worldgen/biome.rs','crates/horizon-world/src/worldgen/biome_placement.rs','crates/horizon-world/src/worldgen/region_weights.rs']:shutil.copy2(ENGINE/name,source/name)
(source/'.horizon-revision').write_text(subprocess.check_output(['git','rev-parse','HEAD'],cwd=ENGINE,text=True))
launcher=Path.home()/'.local/share/horizon-launcher'
tool=launcher/'fork-native/tools/5a64af938ec8/bin/horizon-mixins'
mods=WORK/'mods'
if mods.exists():shutil.rmtree(mods)
shutil.copytree(ROOT/'mixins',mods)
if '--tectonic' in sys.argv:
 with zipfile.ZipFile(Path.home()/'.local/share/horizon/horizon-native-mods/tectonic_horizon.hnmod') as z:
  for n in z.namelist():
   if n.startswith('mixins/') and not n.endswith('/'):
    p=mods/Path(n).relative_to('mixins');p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(z.read(n))
for p in mods.glob('*/horizon-mixin.toml'):
 text=p.read_text();text=re.sub(r'horizon_revision\s*=\s*"[a-f0-9]+"','horizon_revision = "'+(source/'.horizon-revision').read_text().strip()+'"',text);p.write_text(text)
transformed=WORK/'transformed' 
if transformed.exists():shutil.rmtree(transformed)
subprocess.run([tool,'apply','--workspace',source,'--mods',mods,'--out',transformed],check=True)
for p in transformed.rglob('*'):
 if p.is_file():
  dest=stage/p.relative_to(transformed)
  if not dest.exists() or dest.read_bytes()!=p.read_bytes():
   dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,dest)

env=os.environ.copy();compiler=launcher/'rust/1.99.1-x86_64-unknown-linux-gnu/bin'
env.update(PATH=str(compiler)+':'+env['PATH'],RUSTC=str(compiler/'rustc'),CARGO_TARGET_DIR=str(launcher/'fork-native/target'),CARGO_INCREMENTAL='0')
command=[compiler/'cargo','test','--locked','--profile','fast','-p','horizon-world','--lib','native_','-j','2'] if '--tests' in sys.argv else [compiler/'cargo',('build' if '--smoke-library' in sys.argv or '--world-library' in sys.argv else 'check'),'--locked','--profile','fast','-p',('horizon-client-lib' if '--smoke-library' in sys.argv else 'horizon-world'),'-j','2']
subprocess.run(command,cwd=stage,env=env,check=True)
