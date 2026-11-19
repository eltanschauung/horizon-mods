"""Selection/collision bounds from the source models for solid decorative blocks."""
import json,itertools,math
from import_jar import NS

def model_boxes(root,model,seen=()):
    if model in seen:raise ValueError('Model parent cycle')
    ns,path=model.split(':',1)
    p=root/f'assets/{ns}/models/{path}.json'
    if not p.exists():return []
    v=json.loads(p.read_text())
    if 'elements' not in v:return model_boxes(root,v['parent'],seen+(model,)) if 'parent' in v else []
    return [(e['from'],e['to']) for e in v['elements'] if 'from' in e and 'to' in e]
def shape(root,b):
    v=json.loads((root/f'assets/{NS}/blockstates/{b}.json').read_text());variants=v.get('variants',{})
    props=sorted({s.split('=')[0] for key in variants for s in key.split(',') if '=' in s});out={}
    for key,entries in variants.items():
        state=dict(s.split('=') for s in key.split(',') if '=' in s)
        half = state.get('half')
        if half == 'top':
            # The source renders the full model from the bottom half. Its
            # invisible top still needs its own targeting and collision bounds.
            entries = variants[key.replace('half=top', 'half=bottom')]
        if isinstance(entries,dict):entries=[entries]
        points=[]
        for entry in entries:
            for lo,hi in model_boxes(root,entry['model']):
                for p in itertools.product(*zip(lo,hi)):
                    x,y,z=[c-8 for c in p]
                    for axis in ('x','y'):
                        angle=math.radians(entry.get(axis,0));a,c=math.sin(angle),math.cos(angle)
                        if axis=='x':y,z=y*c-z*a,y*a+z*c
                        else:x,z=x*c+z*a,-x*a+z*c
                    points.append([(x+8)/16,(y+8)/16,(z+8)/16])
        if not points:
            out[','.join(state[p] for p in props)]=[]
            continue
        bounds=[min(p[i] for p in points) for i in range(3)]+[max(p[i] for p in points) for i in range(3)]
        if half in ('top', 'bottom'):
            origin = 1 if half == 'top' else 0
            bounds[1] = max(0, bounds[1] - origin)
            bounds[4] = min(1, bounds[4] - origin)
            if bounds[4] <= bounds[1]:
                out[','.join(state[p] for p in props)]=[]
                continue
        out[','.join(state[p] for p in props)]=[[round(x,8) for x in bounds]]
    if not out:return None
    return {'properties':props,'values':out} if props else {'default':next(iter(out.values()))}
