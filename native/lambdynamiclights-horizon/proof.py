#!/usr/bin/env python3
"""Exercise the installed native client on temporary data, never a player's saves."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--ctl', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='lambdynamiclights-proof-') as tmp:
        home = Path(tmp)
        sock = home / 'control.sock'
        env = dict(os.environ, HORIZON_HOME=str(home), HORIZON_LAUNCHER_HOME=str(home/'launcher'),
                   XDG_DATA_HOME=str(home/'data'), XDG_CONFIG_HOME=str(home/'config'), HORIZON_NO_AUDIO='1')
        log = (args.output/'proof.log').open('w')
        process = subprocess.Popen(['xvfb-run','-a',str(args.game),'--resources',str(args.resources),
            '--game-dir',str(home),'--home',str(home),'--world','LambLightProof','--test-terrain','--creative',
            '--render-distance','2','--width','640','--height','480','--control','--control-socket',str(sock)],
            cwd=home, env=env, stdout=log, stderr=log, start_new_session=True)

        def ctl(*parts):
            result = subprocess.run([str(args.ctl),'--socket',str(sock),*parts], cwd=home, env=env,
                                    capture_output=True, text=True, timeout=40, check=True)
            return json.loads(result.stdout)

        def command(text):
            return ctl('chat','/'+text)

        def lights():
            time.sleep(0.3)
            reply = command('ldlights status')
            def strings(value):
                if isinstance(value,str): yield value
                elif isinstance(value,list):
                    for entry in value: yield from strings(entry)
                elif isinstance(value,dict):
                    for entry in value.values(): yield from strings(entry)
            messages=ctl('messages')
            for line in strings(messages.get('styled',[])):
                if 'LambDynamicLights: {' in line:
                    return json.loads(line.split('LambDynamicLights: ',1)[1])['lights']
            raise AssertionError(f'No local light diagnostic: {reply}')

        def levels():
            return sorted(round(light[3]) for light in lights())

        evidence = {}
        try:
            deadline = time.monotonic()+100
            while not sock.exists():
                if process.poll() is not None:
                    raise RuntimeError('Game exited during startup; see proof.log')
                if time.monotonic()>deadline:
                    raise TimeoutError('Control socket did not appear')
                time.sleep(0.5)
            deadline = time.monotonic()+100
            while True:
                status=ctl('status')
                if status.get('player') and status.get('pending_chunks',1)==0:
                    break
                if time.monotonic()>deadline:
                    raise TimeoutError('World did not finish loading')
                time.sleep(0.5)
            # A virtual display may pause on focus loss. Disable only in this temporary home.
            ctl('resume')
            command('gamerule doMobSpawning false')
            command('kill @e[type=!minecraft:player]')
            command('time set midnight')
            command('weather clear')
            command('tp @s 0.5 65 0.5')
            command('clear @s')
            evidence['empty']=levels()
            assert evidence['empty']==[], evidence
            command('item replace entity @s weapon.mainhand with minecraft:torch')
            evidence['main_hand']=levels()
            assert evidence['main_hand']==[14], evidence
            settings_dir=home/'horizon-mods/.horizon-mod-data/lambdynamiclights_horizon'
            settings_dir.mkdir(parents=True,exist_ok=True)
            (settings_dir/'mode').write_text('off')
            command('ldlights reload')
            evidence['mode_off']=levels()
            assert evidence['mode_off']==[],evidence
            (settings_dir/'mode').write_text('fancy')
            (settings_dir/'self').write_text('false')
            command('ldlights reload')
            evidence['own_disabled']=levels()
            assert evidence['own_disabled']==[],evidence
            (settings_dir/'self').write_text('true')
            command('ldlights reload')
            command('item replace entity @s weapon.offhand with minecraft:glowstone')
            evidence['brighter_offhand']=levels()
            assert evidence['brighter_offhand']==[15], evidence
            command('clear @s')
            command('item replace entity @s weapon.mainhand with minecraft:torch')
            command('fill -1 64 -1 1 68 1 minecraft:water')
            time.sleep(1)
            evidence['underwater_torch']=levels()
            assert evidence['underwater_torch']==[], evidence
            command('fill -1 64 -1 1 68 1 minecraft:air')
            command('clear @s')
            command('summon minecraft:item 2.5 66 0.5 {Item:{id:"minecraft:glowstone_dust",count:1},PickupDelay:32767s}')
            evidence['dropped_item']=levels()
            assert 8 in evidence['dropped_item'], evidence
            command('kill @e[type=minecraft:item]')
            command('summon minecraft:blaze 3.5 66 0.5 {NoAI:1b,NoGravity:1b}')
            evidence['blaze']=levels()
            assert 10 in evidence['blaze'], evidence
            command('kill @e[type=minecraft:blaze]')
            command('summon minecraft:pig 3.5 66 0.5 {NoAI:1b,Fire:200s}')
            evidence['burning_mob']=levels()
            assert 15 in evidence['burning_mob'], evidence
            command('kill @e[type=minecraft:pig]')
            command('summon minecraft:item_display 3.5 66 0.5 {item:{id:"minecraft:torch",count:1}}')
            evidence['item_display']=levels()
            assert 14 in evidence['item_display'], evidence
            command('kill @e[type=minecraft:item_display]')
            command('item replace entity @s weapon.mainhand with minecraft:torch')
            command('fill -10 60 -10 10 74 10 minecraft:air')
            command('fill -4 63 -4 4 63 4 minecraft:stone')
            command('tp @s 0.5 64 0.5')
            time.sleep(0.5)
            evidence['restored_held_torch']=levels()
            assert 14 in evidence['restored_held_torch'],evidence
            ctl('look','0','30')
            ctl('screenshot',str((args.output/'held-torch.png').resolve()))
            print(json.dumps({'passed':True,'checks':evidence},sort_keys=True))
            (args.output/'result.json').write_text(json.dumps(evidence,indent=2)+'\n')
        finally:
            if process.poll() is None:
                try:
                    ctl('quit')
                    process.wait(timeout=20)
                except (Exception, subprocess.TimeoutExpired):
                    os.killpg(process.pid,signal.SIGTERM)
                    process.wait(timeout=10)
            log.close()


if __name__=='__main__':
    main()
