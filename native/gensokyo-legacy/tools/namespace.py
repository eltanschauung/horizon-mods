"""Rebase source resources into the port's own namespace, including typed NBT strings."""
import gzip,io,struct
OLD='gensokyolegacy';NEW='horizon_gensokyo_legacy'
def text(s):
    if s=='minecraft:grass':return 'minecraft:short_grass'
    return s.replace(OLD+':',NEW+':').replace(OLD+'.',NEW+'.')
def nbt(data):
    zipped=data[:2]==b'\x1f\x8b';f=io.BytesIO(gzip.decompress(data) if zipped else data)
    def num(fmt):
        b=f.read(struct.calcsize('>'+fmt));return struct.unpack('>'+fmt,b)[0],b
    def string():
        n,_=num('H');s=text(f.read(n).decode()).encode();return struct.pack('>H',len(s))+s
    def payload(t):
        if t in range(1,7):return f.read({1:1,2:2,3:4,4:8,5:4,6:8}[t])
        if t==8:return string()
        if t==10:
            b=b''
            while (t:=f.read(1))!=b'\0':b+=t+string()+payload(t[0])
            return b+b'\0'
        if t==9:
            k=f.read(1);n,b=num('i');return k+b+b''.join(payload(k[0]) for _ in range(n))
        if t in (7,11,12):
            n,b=num('i');return b+f.read(n*{7:1,11:4,12:8}[t])
        raise ValueError(t)
    kind=f.read(1);out=kind+string()+payload(kind[0]);assert not f.read(1)
    return gzip.compress(out,mtime=0) if zipped else out
def rebase(root):
    for p in list(root.rglob('*')):
        if not p.is_file():continue
        if p.suffix in ('.json','.mcmeta'):p.write_text(text(p.read_text()))
        elif p.suffix=='.nbt':p.write_bytes(nbt(p.read_bytes()))
        parts=list(p.relative_to(root).parts)
        if len(parts)>2 and parts[0]=='data' and parts[2]=='structures':parts[2]='structure'
        if OLD in parts or tuple(parts)!=p.relative_to(root).parts:
            q=root.joinpath(*(NEW if x==OLD else x for x in parts));q.parent.mkdir(parents=True,exist_ok=True);p.rename(q)
    for d in sorted(root.rglob('*'),key=lambda p:len(p.parts),reverse=True):
        if d.is_dir() and not any(d.iterdir()):d.rmdir()
