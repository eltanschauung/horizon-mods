#!/usr/bin/env python3
"""Regenerate local legacy mappings, pack WorldEdit and optionally install it."""
import argparse, importlib.util, os, shutil, subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--horizon',type=Path,default=ROOT.parents[2]/'horizon')
    p.add_argument('--install',action='store_true')
    a=p.parse_args()
    spec=importlib.util.spec_from_file_location('worldedit_legacy_generator',ROOT/'tools/gen_legacy.py')
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    import sys
    module.OUT=ROOT/'mixins/worldedit/legacy_ids.rs'
    previous=sys.argv
    try:
        sys.argv=['gen_legacy.py',str(a.horizon/'crates/horizon-world/src/datafix/block_state_data.txt')]
        module.main()
    finally: sys.argv=previous
    subprocess.run([sys.executable,ROOT/'tools/pack.py','--out',ROOT/'target'],check=True)
    import json
    version=json.loads((ROOT/'horizon-native.json').read_text())['version']
    package=ROOT/'target'/f'worldedit-native-{version}.hnmod'
    if a.install:
        home=Path(os.environ.get('HORIZON_HOME',str(Path.home()/'.local/share/horizon')))
        dest=home/'horizon-native-mods/worldedit.hnmod';dest.parent.mkdir(parents=True,exist_ok=True)
        tmp=dest.with_suffix('.hnmod.tmp');shutil.copy2(package,tmp);os.replace(tmp,dest)
        print('Installed',dest)
if __name__=='__main__': main()
