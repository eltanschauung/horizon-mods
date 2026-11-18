"""Assemble the bamboo forest and convert its authored Sponge tree schematics."""
import gzip
import io
import json
import struct

from import_jar import NS


def template_bytes(path):
    src = io.BytesIO(gzip.decompress(path.read_bytes()))

    def number(fmt):
        return struct.unpack('>' + fmt, src.read(struct.calcsize('>' + fmt)))[0]

    def string():
        return src.read(number('H')).decode()

    def parse(kind):
        if kind == 2:
            return number('h')
        if kind == 3:
            return number('i')
        if kind == 7:
            return src.read(number('i'))
        if kind == 11:
            return [number('i') for _ in range(number('i'))]
        if kind == 10:
            result = {}
            while child := number('B'):
                key = string()
                result[key] = parse(child)
            return result
        raise ValueError(f'Unexpected Sponge tag {kind}')

    assert number('B') == 10 and string() == 'Schematic'
    schematic = parse(10)
    assert schematic['Version'] == 2 and not src.read(1)
    w, h, d = [schematic[key] for key in ('Width', 'Height', 'Length')]
    ids = {v: k for k, v in schematic['Palette'].items()}
    data = schematic['BlockData']
    assert len(data) == w*h*d and max(ids) < 128, 'Expected a one-byte palette'

    def text(s):
        b = s.encode()
        return struct.pack('>H', len(b)) + b

    def integer(i):
        return struct.pack('>i', i)

    def array(kind, values):
        return bytes([kind]) + integer(len(values)) + b''.join(values)

    def compound(fields):
        return b''.join(bytes([kind]) + text(key) + value for kind, key, value in fields) + b'\0'

    def state(spec, vanilla):
        name, _, props = spec.partition('[')
        props = dict(pair.split('=') for pair in props.rstrip(']').split(',') if pair)
        if vanilla and name == 'lunatic:bushy_bamboo':
            name = 'minecraft:bamboo'
        if vanilla and name == 'lunatic:bamboo_leaves':
            name = 'minecraft:azalea_leaves'
            props.update(distance='7', persistent='true')
        return compound([
            (8, 'Name', text(name)),
            (10, 'Properties', compound([(8, k, text(v)) for k, v in props.items()])),
        ])

    blocks = []
    for i, index in enumerate(data):
        spec = ids[index]
        if spec == 'minecraft:air':
            continue
        blocks.append(compound([
            (9, 'pos', array(3, [integer(v) for v in (i % w, i // (w*d), i // w % d)])),
            (3, 'state', integer(index)),
        ]))
    palettes = [array(10, [state(ids[i], vanilla) for i in range(len(ids))])
                for vanilla in (False, True)]
    body = compound([
        (3, 'DataVersion', integer(schematic['DataVersion'])),
        (9, 'size', array(3, list(map(integer, (w, h, d))))),
        (9, 'palettes', array(9, palettes)),
        (9, 'blocks', array(10, blocks)),
        (9, 'entities', array(10, [])),
    ])
    return gzip.compress(b'\x0a\0\0' + body, mtime=0)


def add(out, write):
    worldgen = out / f'data/{NS}/worldgen'
    directory = out / f'data/{NS}/structure/bamboo_forest'
    directory.mkdir(parents=True, exist_ok=True)
    weights = {'prototype_1': 4, 'short': 3, 'wide': 1, 'tall': 3, 'swept': 2, 'twin_crown': 3}
    for name in weights:
        source = out.parent / 'bamboo-trees' / (name + '.schem')
        (directory / (name + '.nbt')).write_bytes(template_bytes(source))

    def modifier(name, **params):
        return {'type': name, **params}

    def configured(name, **params):
        return modifier('minecraft:' + name, **params)

    def placed(name, feature, placement):
        file = worldgen / f'placed_feature/bamboo_forest/{name}.json'
        write(file, {'feature': feature, 'placement': placement})
        return f'{NS}:bamboo_forest/{name}'

    def gradient(edge, interior, salt):
        return modifier(NS + ':biome_density', edge=edge, interior=interior, salt=salt)

    surface = configured('heightmap', heightmap='OCEAN_FLOOR_WG')
    dry = configured('surface_water_depth_filter', max_water_depth=0)
    biome = configured('biome')
    square = configured('in_square')
    grove = placed('grove', modifier(NS + ':bamboo_grove', templates=[
        {'id': f'{NS}:bamboo_forest/{name}', 'weight': weight} for name, weight in weights.items()
    ]), [
        modifier(NS + ':jittered_grid', value={'cell': 7, 'margin': 2, 'salt': 41321, 'chance': 1}),
        surface, dry, biome, gradient(.20, 1, 4991),
    ])
    jungle = placed('jungle_trees', 'minecraft:jungle_tree', [
        configured('count', count=5), square, surface, dry, biome, gradient(.95, .30, 4992),
    ])
    oak = placed('edge_oaks', 'minecraft:oak', [
        configured('count', count=3), square, surface, dry, biome, gradient(.55, 0, 4993),
    ])
    bushes = placed('bushes', 'minecraft:jungle_bush', [
        configured('count', count=3), square, surface, dry, biome,
    ])
    lake = json.loads((worldgen / 'configured_feature/magical_forest/lake.json').read_text())
    lake['shape'].update(max_width=12, component_radius=6)
    pond = placed('pond', lake, [configured('rarity_filter', chance=10), square, surface, dry, biome])

    path = worldgen / 'biome/bamboo_forest_of_the_lost.json'
    forest = json.loads(path.read_text())
    # Jungle uses the foliage colormap at its temperature and downfall.
    forest.update(temperature=.95, downfall=.90)
    forest['effects'] = {
        'water_color': '#3f76e4',
        'mood_sound': forest['effects']['mood_sound'],
    }
    forest['attributes'].pop('minecraft:visual/water_fog_color', None)
    forest['features'][1].append(pond)
    forest['features'][6].remove(f'{NS}:gensokyo_biome/bamboo_forest_mud')
    forest['features'][9] = [
        'minecraft:glow_lichen', grove, jungle, oak, bushes, 'minecraft:bamboo',
        'minecraft:patch_grass_jungle', 'minecraft:patch_sugar_cane',
        'minecraft:patch_firefly_bush_swamp', 'minecraft:patch_firefly_bush_near_water_swamp',
    ]
    write(path, forest)
