from pathlib import Path
from zipfile import ZipFile
import struct
r=Path(__file__).resolve().parent
rules={}
with ZipFile('/home/xxlib/Downloads/BiomesOPlenty-forge-26.3-26.3.0.0.7.jar') as z:
 for line in z.read('biomesoplenty.accesswidener').decode().splitlines():
  p=line.split()
  if p and p[0]=='accessible':rules.setdefault(p[2],[]).append(p[1:])
with ZipFile('/home/xxlib/.minecraft/versions/26.3/26.3.jar') as z:
 for name,rs in rules.items():
  if name+'.class' not in z.namelist():continue
  b=bytearray(z.read(name+'.class'));off=10;cp={};i=1
  u2=lambda o:struct.unpack_from('>H',b,o)[0]
  while i<u2(8):
   t=b[off];off+=1
   if t==1:
    n=u2(off);off+=2;cp[i]=bytes(b[off:off+n]).decode();off+=n
   elif t in (3,4,9,10,11,12,17,18):off+=4
   elif t in (5,6):off+=8;i+=1
   elif t in (7,8,16,19,20):off+=2
   elif t==15:off+=3
   else:raise ValueError(t)
   i+=1
  if any(x[0]=='class' for x in rs):struct.pack_into('>H',b,off,(u2(off)&~6)|1)
  off+=6;off+=2+2*u2(off)
  for kind in ['field','method']:
   count=u2(off);off+=2
   for _ in range(count):
    acc=off;n=cp[u2(off+2)];d=cp[u2(off+4)];num=u2(off+6);off+=8
    if any(x[0]==kind and x[2]==n and (len(x)<4 or x[3]==d) for x in rs):struct.pack_into('>H',b,acc,(u2(acc)&~6)|1)
    for _ in range(num):size=struct.unpack_from('>I',b,off+2)[0];off+=6+size
  out=r/'patches'/(name+'.class');out.parent.mkdir(parents=True,exist_ok=True);out.write_bytes(b)
print('Applied the supplied BOP access widener to private oracle class copies')
