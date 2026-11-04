#!/usr/bin/env python3
"""Exercise the converter CLI on synthetic Anvil files, including negative coordinates."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import zlib

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('binary',type=Path)
a=parser.parse_args()
binary=str(a.binary.resolve())
def text(s):
    b=s.encode();return struct.pack('>H',len(b))+b
def tag(ty,name,payload):return bytes([ty])+text(name)+payload
def compound(entries):return b''.join(entries)+b'\0'
def state(name):return compound([tag(8,'Name',text(name))])
def nbt(copper):
    names=['minecraft:stone','minecraft:copper_ore','minecraft:coal_ore','minecraft:deepslate_copper_ore'] if copper else ['minecraft:stone']
    blocks=[tag(9,'palette',b'\x08'+struct.pack('>i',len(names))+b''.join(map(text,names)))]
    if copper:blocks.append(tag(12,'data',struct.pack('>i',256)+struct.pack('>q',0x3210321032103210)*256))
    section=compound([tag(1,'Y',b'\0'),tag(10,'block_states',compound(blocks))])
    return tag(10,'',compound([tag(9,'sections',b'\x0a'+struct.pack('>i',1)+section),tag(8,'inventory_test',text('minecraft:copper_ore'))]))
def record(data):
    data=b'\x02'+zlib.compress(data);raw=struct.pack('>I',len(data))+data
    return raw+b'\0'*((-len(raw))%4096)
def payload(path,index):
    b=path.read_bytes();loc=int.from_bytes(b[index*4:index*4+4],'big');off=(loc>>8)*4096;n=int.from_bytes(b[off:off+4],'big')
    return b[off+4:off+4+n],b[4096+index*4:4100+index*4]
with tempfile.TemporaryDirectory(prefix='ore-converter-proof-') as tmp:
    root=Path(tmp);region=root/'world/dimensions/minecraft/overworld/region/r.-1.-1.mca';region.parent.mkdir(parents=True)
    plain=record(nbt(False));ore=record(nbt(True));header=bytearray(8192)
    header[0:4]=((2<<8)|(len(plain)//4096)).to_bytes(4,'big')
    header[1023*4:1024*4]=(((2+len(plain)//4096)<<8)|(len(ore)//4096)).to_bytes(4,'big')
    header[4096:4100]=(123).to_bytes(4,'big');header[4096+1023*4:8192]=(456).to_bytes(4,'big')
    region.write_bytes(header+plain+ore);original=hashlib.sha256(region.read_bytes()).digest()
    before=json.loads(subprocess.check_output([binary,'scan',str(root/'world')]))
    assert before['copper_ore']==1024 and before['deepslate_copper_ore']==1024,before
    subprocess.run([binary,'stage',str(root/'world'),str(root/'converted')],check=True,capture_output=True)
    converted=root/'converted'/region.relative_to(root/'world')
    after=json.loads(subprocess.check_output([binary,'scan',str(root/'converted')]))
    assert after['copper_ore']==0 and after['deepslate_copper_ore']==0,after
    assert after['chunks_scanned']==2
    assert payload(region,0)==payload(converted,0),'untouched compressed chunk or timestamp changed'
    assert payload(region,1023)[1]==payload(converted,1023)[1],'changed chunk timestamp changed'
    assert b'minecraft:copper_ore' in zlib.decompress(payload(converted,1023)[0][1:]),'non-block sentinel was changed'
    assert hashlib.sha256(region.read_bytes()).digest()==original,'source region was modified'
    print('Synthetic Anvil CLI proof passed: exact counts, negative coordinates, untouched bytes/timestamps and source preservation')
