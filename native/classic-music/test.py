#!/usr/bin/env python3
"""Test Classic Music against Horizon's real SoundRegistry and prepared assets."""
import argparse
import json
import os
from pathlib import Path
import subprocess

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--horizon',type=Path,required=True)
parser.add_argument('--resources',type=Path,required=True)
parser.add_argument('--cargo',default='cargo')
a=parser.parse_args()
mod=Path(__file__).resolve().parent
horizon=a.horizon.resolve()
resources=a.resources.resolve()
target=mod.parent/'target/classic-music-tests'
target.mkdir(parents=True,exist_ok=True)
(target/'Cargo.toml').write_text('[package]\nname="classic-music-registry-tests"\nversion="0.1.0"\nedition="2024"\n[workspace]\n[lib]\npath="lib.rs"\n[dependencies]\nhorizon-core={path='+json.dumps(str(horizon/'crates/horizon-core'))+'}\nserde_json="=1.0.151"\n[profile.fast]\ninherits="dev"\nopt-level=1\n')
source=(horizon/'crates/horizon-client-render/src/sound_events.rs').read_text()
for module,file in [('classic_music_policy','policy.rs'),('classic_music_registry','registry.rs')]:
    source+='\n#[path='+json.dumps(str(mod/'mixins/classic_music'/file))+']\npub mod '+module+';\n'
(target/'sound_events.rs').write_text(source)
(target/'lib.rs').write_text('pub mod sound_events;\n')
env=dict(os.environ,CLASSIC_MUSIC_RESOURCES=str(resources))
subprocess.run([a.cargo,'test','--offline','--profile','fast','--manifest-path',str(target/'Cargo.toml')],env=env,check=True)
