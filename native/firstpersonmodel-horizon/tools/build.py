#!/usr/bin/env python3
import json,zipfile
from pathlib import Path
r=Path(__file__).resolve().parents[1]; c=json.loads((r/'compatibility.json').read_text())
out=r/'target/firstpersonmodel_horizon.hnmod';out.parent.mkdir(exist_ok=True)
with zipfile.ZipFile(out,'w',zipfile.ZIP_DEFLATED) as z:
 z.writestr('horizon-native.json',json.dumps({'schema':1,'id':'firstpersonmodel_horizon','version':'0.2.6','game':c['game'],'hooks':c['hooks'],'horizon':c['horizon_revision'],'mixins':c['horizon_mixins_revision']}))
 for p in sorted((r/'mixins').rglob('*')):
  if p.is_file():z.write(p,p.relative_to(r).as_posix())
print(out)
