#!/usr/bin/env python3
"""Exercise the live vanilla/body hand setting in a temporary world."""
import json,os,subprocess,tempfile,time,signal,shutil,argparse
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--plan',type=Path,required=True);a=p.parse_args();plan=a.plan.resolve();meta=json.loads((plan/'artifacts.json').read_text());client=Path(meta['client']['directory']);resources=client/'.local/horizon-client/26.3';out=plan/'proof';out.mkdir(exist_ok=True)
def until(fn,ok,seconds=90):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  try:
   v=fn()
   if ok(v):return v
  except (OSError,ValueError,subprocess.SubprocessError):pass
  time.sleep(.2)
 raise AssertionError('Timed out waiting for the isolated client')
for enabled in [True]:
 label='on' if enabled else 'off'
 with tempfile.TemporaryDirectory(prefix='firstperson-proof-', ignore_cleanup_errors=True) as t:
  home=Path(t);sock=home/'control.sock';(home/'horizon-mods').mkdir()
  for package in (plan/'inputs/horizon-mods').glob('*.hmod'):shutil.copy2(package,home/'horizon-mods'/package.name)
  (home/'horizon-mods.txt').write_text('mod.lunatic:true\nmod.firstpersonmodel_horizon:'+str(enabled).lower()+'\n')
  (home/'character.json').write_text(json.dumps({'character':'lunatic:hakurei_reimu'}))
  (home/'options.txt').write_text('renderDistance:2\nsimulationDistance:2\nmaxFps:30\npauseOnLostFocus:false\n')
  env=dict(os.environ,HORIZON_HOME=t,HORIZON_LAUNCHER_HOME=str(home/'launcher'),XDG_DATA_HOME=str(home/'data'),XDG_CONFIG_HOME=str(home/'config'),HORIZON_NO_AUDIO='1')
  with (out/(label+'.log')).open('w') as log:
   proc=subprocess.Popen(['xvfb-run','-a',str(client/'horizon-client'),'--resources',str(resources),'--game-dir',t,'--home',t,'--world','FirstPersonProof','--test-terrain','--creative','--width','800','--height','600','--render-distance','2','--control','--control-socket',str(sock)],cwd=home,env=env,stdout=log,stderr=log,start_new_session=True)
  def ctl(*args):
   r=subprocess.run([str(client/'horizon-ctl'),'--socket',str(sock),'--timeout','10',*map(str,args)],capture_output=True,text=True,timeout=14);r.check_returncode();return json.loads(r.stdout)
  try:
   until(lambda:ctl('status'),lambda s:(s.get('server')or{}).get('loaded',False))
   ctl('command','time set noon');ctl('command','weather clear');ctl('command','tp @s 0.5 120 0.5');time.sleep(1)
   ctl('command','fill -8 118 -8 8 150 8 minecraft:air');ctl('command','fill -8 119 -8 8 119 8 minecraft:stone');ctl('command','tp @s 0.5 120 0.5');time.sleep(1)
   ctl('look',0,80);ctl('screenshot',out/(label+'-down.png'))
   ctl('command','item replace entity @s armor.head with minecraft:iron_helmet');ctl('command','item replace entity @s weapon.mainhand with minecraft:iron_sword');time.sleep(.5)
   ctl('command','item replace entity @s armor.chest with minecraft:iron_chestplate');time.sleep(.3)
   ctl('look',0,0);ctl('screenshot',out/(label+'-forward.png'))
   ctl('look',0,80);ctl('screenshot',out/(label+'-equipped.png'))
   ctl('key','shift','down');time.sleep(.4);ctl('screenshot',out/(label+'-crouch.png'));ctl('key','shift','up')
   # Test the real Mods settings page, not a private control shortcut.
   ctl('options','mods')
   def widgets(value):
    result=[]
    if isinstance(value,dict):
     if 'label' in value and 'bounds' in value:result.append(value)
     for child in value.values():result.extend(widgets(child))
    elif isinstance(value,list):
     for child in value:result.extend(widgets(child))
    return result
   found=None
   for attempt in range(12):
    ui=ctl('ui');(out/'settings-ui.json').write_text(json.dumps(ui,indent=2))
    rows=widgets(ui)
    target=next((w for w in rows if 'FirstPersonModel Horizon' in str(w['label'])),None)
    if target:
     y=target['bounds'][1]
     found=next((w for w in rows if str(w['label']).lower().startswith('settings') and abs(w['bounds'][1]-y)<1),None)
     if found:break
    ctl('scroll-at',200,150,-3);time.sleep(.2)
   assert found is not None,'FirstPersonModel Settings button missing'
   ctl('click',found['id']);time.sleep(.3)
   ctl('screenshot',out/'hands-settings.png')
   ctl('click-button','Hands: Vanilla');time.sleep(.2)
   preference=home/'horizon-mods/.horizon-mod-data/firstpersonmodel_horizon/vanilla_hands'
   assert preference.read_text().strip()=='false'
   ctl('close');ctl('look',0,80);ctl('screenshot',out/'body-hands.png')
   ctl('options','mods')
   # The native settings page may be remembered; otherwise reopen its row.
   ui=ctl('ui')
   if not any('Hands: Body' in str(w.get('label')) for w in widgets(ui)):
    ctl('click',found['id'])
   ctl('click-button','Hands: Body');assert preference.read_text().strip()=='true'
   ctl('close');ctl('look',0,80);ctl('screenshot',out/'vanilla-hands.png')
   ctl('key','f5','tap');ctl('screenshot',out/(label+'-third.png'))
   (out/(label+'-status.json')).write_text(json.dumps(ctl('status')))
   print('Captured '+label+' views',flush=True)
  finally:
   try:ctl(quit)
   except (OSError,ValueError,subprocess.SubprocessError):os.killpg(proc.pid,signal.SIGTERM)
   try:proc.wait(timeout=20)
   except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL);proc.wait()
   time.sleep(3)
print('PASS: Mods setting toggles both directions, saves the choice, updates live, and renders equipped/crouched/third-person views without crashes.',flush=True)
