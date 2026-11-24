#!/usr/bin/env python3
"""Build Horizon content from the supplied Magic Brooms 3.0.0 JAR."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import shutil
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[1]
NS = 'horizon_magic_brooms'
FAMILIES = ('magic', 'fire', 'ice', 'lightning', 'ender', 'grove', 'tide')
CATALYSTS = ('blaze_powder', 'packed_ice', 'redstone', 'chorus_fruit', 'vine', 'prismarine')
WORKSTATIONS = ('broom_workbench', 'scroll_table', 'broom_rack')
FACING_ROTATIONS = {'north': 0, 'east': 90, 'south': 180, 'west': 270}

WORKBENCH_BOXES = [
    (0, 0, 0, 16, 3, 16), (0, 11, 0, 16, 13, 16),
    *((x, 3, z, x+3, 13, z+3) for x in (1, 12) for z in (1, 12)),
]
SCROLL_TABLE_BOXES = [
    (2, 6, 3, 14, 8, 13),
    *((x, 0, z, x+2, 6, z+2) for x in (3, 11) for z in (4, 10)),
    *((4, 3, z, 12, 4, z+2) for z in (4, 10)),
]
RACK_BOXES = [
    (-6, 0, 4, 22, 2, 12), (-5, 2, 10, -3, 24, 12), (19, 2, 10, 21, 24, 12),
    (-5, 13, 10, 21, 15, 12), (-5, 24, 10, 21, 26, 12),
    *((x, 12, 5, x+4, 14, 10) for x in (-4, 6, 16)),
]


def block_boxes(boxes):
    return [[coordinate / 16 for coordinate in box] for box in boxes]


def rack_boxes(boxes):
    turned = [(16-z1, y0, x0, 16-z0, y1, x1) for x0, y0, z0, x1, y1, z1 in boxes]
    return {'property': 'facing', 'values': {
        facing: block_boxes(turned if facing in ('east', 'west') else boxes)
        for facing in FACING_ROTATIONS
    }}


def write(root, path, value):
    file = root / path
    file.parent.mkdir(parents=True, exist_ok=True)
    file.write_text(json.dumps(value, indent=2) + '\n')


def rotate(point, transform):
    # GeckoLib bakes in a reflected X coordinate system and applies the
    # authored rotations as Rz * Ry * Rx around the reflected pivot.
    x, y, z = transform.get('pivot', (0, 0, 0))
    pivot = (-x, y, z)
    x, y, z = transform.get('rotation', (0, 0, 0))
    angles = (-x, -y, z)
    p = [v - o for v, o in zip(point, pivot)]
    for axis, degrees in enumerate(angles):
        a, b = (axis + 1) % 3, (axis + 2) % 3
        theta = math.radians(degrees)
        c, s = math.cos(theta), math.sin(theta)
        p[a], p[b] = p[a] * c - p[b] * s, p[a] * s + p[b] * c
    return [v + o for v, o in zip(p, pivot)]


def entity_model(geometry, mounted=True):
    """Convert rigid GeckoLib box-UV geometry to the game's OBJ model loader."""
    lines = ['mtllib broom.mtl', 'usemtl surface']
    next_vertex = 1
    next_uv = 0
    bones = {bone['name']: bone for bone in geometry['bones']}
    for bone in geometry['bones']:
        for cube in bone.get('cubes', []):
            x, y, z = cube['origin']
            w, h, d = cube['size']
            x = -x - w
            box_uv = cube['uv']
            u, v = box_uv if isinstance(box_uv, list) else (0, 0)
            # The bristle layers have different negative inflation values so
            # their top and bottom faces never occupy the same depth.
            inflate = cube.get('inflate', bone.get('inflate', 0))
            corners = [(x - inflate + a*(w + 2*inflate),
                        y - inflate + b*(h + 2*inflate),
                        z - inflate + c*(d + 2*inflate))
                       for c in range(2) for b in range(2) for a in range(2)]
            for point in corners:
                posed = rotate(point, cube)
                transform = bone
                while transform:
                    posed = rotate(posed, transform)
                    transform = bones.get(transform.get('parent'))
                a, b, c = posed
                center = 0.5 if mounted else 0.0
                lines.append(f'v {a/16+0.5:.6f} {b/16+center:.6f} {c/16+0.5:.6f}')
            w, h, d = map(math.floor, (w, h, d))
            faces = {
                'north': ((2, 3, 1, 0), u+d, v+d, w, h),
                'south': ((7, 6, 4, 5), u+2*d+w, v+d, w, h),
                'west': ((6, 2, 0, 4), u+d+w, v+d, d, h),
                'east': ((3, 7, 5, 1), u, v+d, d, h),
                'up': ((6, 7, 3, 2), u+d, v, w, d),
                'down': ((0, 1, 5, 4), u+d+w, v+d, w, -d),
            }
            width = geometry['description']['texture_width']
            height = geometry['description']['texture_height']
            for direction, (indices, sx, sy, sw, sh) in faces.items():
                if isinstance(box_uv, dict):
                    if direction not in box_uv:
                        continue
                    face_uv = box_uv[direction]
                    sx, sy = face_uv['uv']
                    sw, sh = face_uv.get('uv_size', (sw, sh))
                if sw == 0 or sh == 0:
                    continue
                uv = [(sx+sw, sy), (sx, sy), (sx, sy+sh), (sx+sw, sy+sh)]
                for a, b in uv:
                    lines.append(f'vt {a/width:.7f} {1-b/height:.7f}')
                # OBJ vertex and UV indices have independent counters.
                next_uv += 4
                face = ' '.join(f'{next_vertex+i}/{next_uv-3+j}' for j, i in enumerate(indices))
                lines.append('f '+face)
            next_vertex += 8
    return '\n'.join(lines)+'\n'


def assemble(source, root):
    assets = f'assets/{NS}'
    data = f'data/{NS}'
    lang = {}
    icons = []

    def item(name, title, max_stack=64):
        lang[f'item.{NS}.{name}'] = title
        icons.append(f'{NS}:{name}')
        write(root, f'{data}/horizon/item/{name}.json', {
            'template': 'minecraft:stick', 'max_stack_size': max_stack,
        })
        model = {'type': 'minecraft:model', 'model': f'{NS}:item/{name}'}
        if name.startswith('broom_'):
            model = {'type': 'minecraft:select', 'property': 'minecraft:display_context',
                     'cases': [{'when': ['fixed'], 'model': {'type': 'minecraft:model',
                                                            'model': f'{NS}:item/{name}_mount'}}],
                     'fallback': model}
        write(root, f'{assets}/items/{name}.json', {'model': model})
        write(root, f'{assets}/models/item/{name}.json', {
            'parent': 'minecraft:item/generated', 'textures': {'layer0': f'{NS}:item/{name}'},
        })

    def texture(path, destination=None):
        out = root / assets / 'textures' / (destination or path)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_bytes(source.read(f'assets/brooms/textures/{path}'))

    models = root / assets / 'models'
    models.mkdir(parents=True, exist_ok=True)
    (models/'broom.mtl').write_text('newmtl surface\nmap_Kd #surface\n')
    original_lang = json.loads(source.read('assets/brooms/lang/en_us.json'))
    for upgraded in (False, True):
        geo_name = 'broom_upgraded' if upgraded else 'broom'
        for name in (geo_name, f'rack_{geo_name}'):
            geometry = json.loads(source.read(f'assets/brooms/geo/{name}.geo.json'))['minecraft:geometry'][0]
            (models/f'{name}.obj').write_text(entity_model(geometry))
        for family in FAMILIES:
            name = f'broom_{family}' + ('_upgraded' if upgraded else '')
            item(name, original_lang[f'item.brooms.{name}'], 1)
            lang[f'entity.{NS}.{name}'] = original_lang[f'item.brooms.{name}']
            texture(f'item/{name}.png')
            texture(f'entity/{name}.png', f'item/{name}_mount.png')
            for display, mesh in (('mount', geo_name), ('rack', f'rack_{geo_name}')):
                write(root, f'{assets}/models/item/{name}_{display}.json', {
                    'loader': 'neoforge:obj', 'flip_v': True, 'model': f'{NS}:models/{mesh}.obj',
                    'textures': {'surface': f'{NS}:item/{name}_mount', 'particle': f'{NS}:item/{name}_mount'},
                })
            write(root, f'{assets}/items/{name}_rack.json', {
                'model': {'type': 'minecraft:model', 'model': f'{NS}:item/{name}_rack'},
            })
            write(root, f'{data}/horizon/entity_type/{name}.json', {
                'template': 'minecraft:wandering_trader',
            })
    for family, catalyst in zip(FAMILIES[1:], CATALYSTS):
        scroll = f'{family}_scroll'
        item(scroll, original_lang[f'item.brooms.{scroll}'])
        texture(f'item/{scroll}.png')
        write(root, f'{data}/recipe/{scroll}.json', {
            'type': 'minecraft:crafting_shapeless', 'ingredients': ['minecraft:paper', f'minecraft:{catalyst}'],
            'result': {'id': f'{NS}:{scroll}', 'count': 1},
        })
    for block in WORKSTATIONS:
        lang[f'block.{NS}.{block}'] = original_lang[f'block.brooms.{block}']
        icons.append(f'{NS}:{block}')
        state = ({'stage': {'type': 'enum', 'values': ['empty', 'handle', 'bristles', *FAMILIES[1:],
                                                     *(f'{family}_upgraded' for family in FAMILIES)]}}
                 if block == 'broom_workbench' else
                 {'stage': {'type': 'enum', 'values': ['empty', 'paper', *FAMILIES[1:]]}}
                 if block == 'scroll_table' else {})
        block_decl = {
            'template': 'minecraft:crafting_table', 'state_properties': state,
            'properties': {'occludes': False},
        }
        if block == 'broom_rack':
            # Loom's horizontal placement faces the block toward its placer.
            block_decl.update(template='minecraft:loom', default_state={'facing': 'north'})
            block_decl['shapes'] = rack_boxes([(-6, 0, 3, 22, 26, 13)])
            block_decl['collision_shapes'] = rack_boxes(RACK_BOXES)
        else:
            block_decl['shapes'] = {'default': block_boxes(
                WORKBENCH_BOXES if block == 'broom_workbench' else SCROLL_TABLE_BOXES)}
            block_decl['collision_shapes'] = block_decl['shapes']
        write(root, f'{data}/horizon/block/{block}.json', block_decl)
        def station_model(path, geometry, sprite):
            mesh = models/f'{geometry}.obj'
            if not mesh.exists():
                geo = json.loads(source.read(f'assets/brooms/geo/{geometry}.geo.json'))['minecraft:geometry'][0]
                mesh.write_text(entity_model(geo, mounted=False))
            display = {'gui': {'rotation': [30, 225, 0], 'scale': [0.65] * 3}}
            if path == 'broom_workbench':
                for hand, yaw in (('righthand', 45), ('lefthand', 225)):
                    display[f'thirdperson_{hand}'] = {
                        'rotation': [75, 45, 0], 'translation': [0, 2.5, 0], 'scale': [0.375] * 3}
                    display[f'firstperson_{hand}'] = {'rotation': [0, yaw, 0], 'scale': [0.4] * 3}
            write(root, f'{assets}/models/block/{path}.json', {
                'loader': 'neoforge:obj', 'flip_v': True, 'model': f'{NS}:models/{geometry}.obj',
                'textures': {'surface': f'{NS}:{sprite}', 'particle': f'{NS}:{sprite}'},
                'display': display,
            })
            return {'model': f'{NS}:block/{path}'}

        if block == 'broom_workbench':
            variants = {}
            for stage in state['stage']['values']:
                family = stage.removesuffix('_upgraded')
                if family not in FAMILIES:
                    family = 'magic'
                if stage.endswith('_upgraded'):
                    mesh = 'workbench_upgraded'
                else:
                    mesh = {'empty': 'workbench_empty', 'handle': 'workbench_handle'}.get(stage, 'workbench_complete')
                sprite = f'block/workbench/{family}'
                texture(sprite+'.png')
                variants[f'stage={stage}'] = station_model(
                    block if stage == 'empty' else f'{block}_{stage}', mesh, sprite)
        else:
            sprite = 'block/scroll_table/base' if block == 'scroll_table' else 'block/broom_rack'
            texture(sprite+'.png')
            model = station_model(block, block, sprite)
            variants = ({f'facing={facing}': dict(model, y=degrees)
                         for facing, degrees in FACING_ROTATIONS.items()}
                        if block == 'broom_rack' else {'': model})
        blockstates = {'variants': variants}
        if block == 'scroll_table':
            blockstates = {'multipart': [{'apply': model}]}
            for stage in ('paper', *FAMILIES[1:]):
                material = 'minecraft:item/paper' if stage == 'paper' else f'{NS}:item/{stage}_scroll'
                half_width = 3.68 if stage == 'paper' else 4.16
                path = f'scroll_table_preview_{stage}'
                write(root, f'{assets}/models/block/{path}.json', {
                    'textures': {'page': material, 'particle': f'{NS}:block/scroll_table/base'},
                    'elements': [{
                        'from': [8-half_width, 8.24, 8-half_width],
                        'to': [8+half_width, 8.24, 8+half_width],
                        'faces': {'up': {'texture': '#page', 'uv': [0, 0, 16, 16]}},
                    }],
                })
                blockstates['multipart'].append({
                    'when': {'stage': stage}, 'apply': {'model': f'{NS}:block/{path}'},
                })
        write(root, f'{assets}/blockstates/{block}.json', blockstates)
        write(root, f'{assets}/items/{block}.json', {
            'model': {'type': 'minecraft:model', 'model': f'{NS}:block/{block}'},
        })
    for name in source.namelist():
        if not name.startswith('data/brooms/recipes/') or not name.endswith('.json'):
            continue
        recipe = json.loads(source.read(name))
        for key in ('ingredients', 'key'):
            if key in recipe:
                entries = recipe[key]
                def ingredients(part):
                    if isinstance(part, list):
                        return [ingredients(p) for p in part]
                    if 'item' in part:
                        return part['item'].replace('brooms:', NS+':')
                    return '#'+part['tag'].replace('brooms:', NS+':')
                recipe[key] = ({k: ingredients(v) for k,v in entries.items()} if isinstance(entries, dict)
                               else [ingredients(v) for v in entries])
        result = recipe['result']
        result['id'] = result.pop('item').replace('brooms:', NS+':')
        write(root, f'{data}/recipe/{Path(name).name}', recipe)
    write(root, f'{assets}/lang/en_us.json', lang)
    write(root, f'{data}/horizon/creative_tab/magic_brooms.json', {
        'title': 'Magic Brooms', 'icon': f'{NS}:broom_magic', 'items': icons,
    })


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('jar', type=Path)
    args = parser.parse_args()
    if hashlib.sha256(args.jar.read_bytes()).hexdigest() != ORIGINAL_SHA256:
        raise SystemExit('Expected the supplied Magic Brooms Forge 1.18.2 version 3.0.0 JAR')
    root = ROOT/'content'
    if root.exists():
        shutil.rmtree(root)
    root.mkdir()
    with ZipFile(args.jar) as z:
        assemble(z, root)
    shutil.copyfile(ROOT/'horizon-mod.toml', root/'horizon-mod.toml')
    notice = root/f'assets/{NS}/licenses/notice.txt'
    notice.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT/'NOTICE', notice)


ORIGINAL_SHA256 = '2606a566f7d3fe77e0e280ca3b04c34145452423caed59c68ba8a8a5a656301c'
if __name__ == '__main__':
    main()
