#!/usr/bin/env python3
"""Package Jade with the shared tool; --install records this fork as its update source."""
from pathlib import Path
import argparse, datetime, hashlib, json, os, shutil, subprocess

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parents[1]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--install', action='store_true')
    args = parser.parse_args()
    manifest = json.loads((ROOT/'horizon-native.json').read_text())
    subprocess.run(['python3', REPO/'native/tools/native.py', 'package', 'jade'], cwd=REPO, check=True)
    package = REPO/'native/target'/f"jade-{manifest['version']}.hnmod"
    if not args.install:
        return
    subprocess.run(['git', 'diff', '--quiet', 'HEAD', '--', 'native/jade'], cwd=REPO, check=True)
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip()
    home = Path(os.environ.get('HORIZON_HOME', Path.home()/'.local/share/horizon'))
    directory = home/'horizon-native-mods'
    directory.mkdir(parents=True, exist_ok=True)
    target = directory/'jade.hnmod'
    origin = directory/'jade.origin.json'
    backup = REPO/'.local/jade-package-backups'/datetime.datetime.now(datetime.UTC).strftime('%Y%m%dT%H%M%S%fZ')
    backup.mkdir(parents=True)
    for path in (target, origin):
        if path.exists():
            shutil.copy2(path, backup/path.name)
    record = dict(id='jade', version=manifest['version'], owner='eltanschauung', repo='horizon-mods',
                  tag=None, commit=revision, asset=None, sha256=hashlib.sha256(package.read_bytes()).hexdigest())
    try:
        temp = target.with_suffix('.hnmod.tmp');shutil.copy2(package,temp);os.replace(temp,target)
        temp = origin.with_suffix('.json.tmp');temp.write_text(json.dumps(record,indent=2)+'\n');os.replace(temp,origin)
    except OSError:
        for path in (target,origin):
            if (backup/path.name).exists():shutil.copy2(backup/path.name,path)
            else:path.unlink(missing_ok=True)
        raise
    print(json.dumps({'installed':str(target),'version':manifest['version'],'commit':revision}))

if __name__ == '__main__':
    main()
