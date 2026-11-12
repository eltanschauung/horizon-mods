#!/usr/bin/env python3
"""Isolated voice-client/relay handshake, group and menu proof."""
import argparse
import json
import os
import re
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
def until(observe,accept,seconds=120):
 end=time.monotonic()+seconds;last=None
 while time.monotonic()<end:
  try:
   last=observe()
   if accept(last):return last
  except (OSError,ValueError,subprocess.SubprocessError):pass
  time.sleep(.1)
 raise AssertionError(f'Timeout: {last}')

p=argparse.ArgumentParser(description=__doc__)
for name in ('game','ctl','resources','mod'):p.add_argument('--'+name,type=Path,required=True)
p.add_argument('--output',type=Path,required=True)
a=p.parse_args()
for key in ('game','ctl','resources','mod','output'):setattr(a,key,getattr(a,key).resolve())
a.output.mkdir(parents=True,exist_ok=True)
children=[]
with tempfile.TemporaryDirectory(prefix='horizon-voice-proof-') as temp:
 root=Path(temp)
 def home(name):
  d=root/name;d.mkdir();(d/'horizon-mods').mkdir();shutil.copy2(a.mod,d/'horizon-mods/lunatic.hmod');(d/'horizon-mods.txt').write_text('mod.lunatic:true\nmod.voicechat:true\nmod.voicechat_server:true\nmod.firstpersonmodel_horizon:false\n');(d/'options.txt').write_text('renderDistance:2\nsimulationDistance:2\nautoJump:false\npauseOnLostFocus:false\n');voice=d/'horizon-mods/.horizon-mod-data/voicechat';voice.mkdir(parents=True);(voice/'muted').write_text('true');return d
 def launch(d,args,label):
  env=dict(os.environ,HORIZON_HOME=str(d),HORIZON_LAUNCHER_HOME=str(d/'launcher'),XDG_CONFIG_HOME=str(d/'config'),XDG_DATA_HOME=str(d/'data'),XDG_CACHE_HOME=str(d/'cache'),HORIZON_NO_AUDIO='1')
  with (a.output/(label+'.log')).open('w') as log:
   child=subprocess.Popen([str(a.game),'--resources',str(a.resources),'--game-dir',str(d),'--home',str(d),*args],cwd=d,env=env,stdin=subprocess.PIPE,stdout=log,stderr=log)
  children.append(child);return child
 def ctl(d,*args):
  q=subprocess.run([str(a.ctl),'--socket',str(d/'c.sock'),'--timeout','10',*args],capture_output=True,text=True,timeout=15)
  if q.returncode:raise ValueError(q.stdout+q.stderr)
  return json.loads(q.stdout)
 def player(d):return ctl(d,'state')['player']
 def connect(d,char,label):
  (d/'character.json').write_text(json.dumps({'character':'lunatic:'+char}))
  child=launch(d,['--offscreen','--server',f'127.0.0.1:{port}','--name','FlailProof','--control','--control-socket',str(d/'c.sock'),'--width','640','--height','480'],label)
  status=until(lambda:ctl(d,'status'),lambda s:(s.get('server') or {}).get('loaded',False),seconds=90)
  return child,'@a'
 def console(text):server.stdin.write((text+'\n').encode());server.stdin.flush()
 srv=home('server')
 with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
 with socket.socket(socket.AF_INET,socket.SOCK_DGRAM) as s:s.bind(('127.0.0.1',0));voice_port=s.getsockname()[1]
 (srv/'config').mkdir();(srv/'config/voicechat-server.json').write_text(json.dumps({'port':voice_port,'bind':'127.0.0.1'}))
 props={'server-ip':'127.0.0.1','server-port':port,'online-mode':'false','enforce-secure-profile':'false','white-list':'false','view-distance':2,'simulation-distance':2,'spawn-monsters':'false','spawn-animals':'false','level-type':'minecraft:flat','level-seed':123,'generator-settings':json.dumps({'layers':[{'block':'minecraft:bedrock','height':1},{'block':'minecraft:dirt','height':2},{'block':'minecraft:grass_block','height':1}],'biome':'minecraft:plains','features':False,'lakes':False,'structure_overrides':[]})}
 (srv/'server.properties').write_text(''.join(f'{k}={v}\n' for k,v in props.items()));(srv/'eula.txt').write_text('eula=true\n')
 evidence={}
 try:
  server=launch(srv,['--dedicated','--nogui'],'server')
  until(lambda:socket.create_connection(('127.0.0.1',port),1).close(),lambda _:True,seconds=90)
  one=home('one');client,_=connect(one,'ibuki_suika','one')
  two=home('two');client2,_=connect(two,'hakurei_reimu','two')
  def log(label):return (root/label/'logs/latest.log').read_text()
  def connected(d,label):
   ctl(d,'chat','/vc status')
   return 'Voice chat Connected (protocol 2.6.x' in log(label)
  until(lambda:connected(one,'one'),bool,seconds=30)
  until(lambda:connected(two,'two'),bool,seconds=30)
  evidence['two_clients_authenticated_over_udp']=True
  ctl(one,'chat','/vc create VoiceProof')
  until(lambda:log('one'),lambda s:'Voice chat: joined the group VoiceProof' in s,seconds=10)
  time.sleep(.5);ctl(two,'chat','/vc join VoiceProof')
  until(lambda:log('two'),lambda s:'Voice chat: joined the group VoiceProof' in s,seconds=10)
  ctl(two,'chat','/vc groups')
  until(lambda:log('two'),lambda s:'VoiceProof (yours)' in s,seconds=10)
  evidence['group_creation_and_join_through_native_transport']=True
  ctl(two,'key','v','tap');time.sleep(.4);ctl(two,'screenshot',str(a.output/'voice-menu.png'))
  assert 'panicked at' not in log('one') and 'panicked at' not in log('two')
  (a.output/'result.json').write_text(json.dumps(evidence,indent=2)+'\n');print(json.dumps({'passed':True,'checks':evidence}),flush=True)
 finally:
  for label in ('one','two','server'):
   source=root/label/'logs/latest.log'
   if source.exists():shutil.copy2(source,a.output/(label+'-game.log'))
  for child in reversed(children):
   if child.poll() is None:
    try:child.stdin.write(b'stop\n');child.stdin.flush();child.wait(timeout=3)
    except (BrokenPipeError,OSError,subprocess.TimeoutExpired):child.terminate()
  for child in children:
   try:child.wait(timeout=10)
   except subprocess.TimeoutExpired:child.kill();child.wait()
