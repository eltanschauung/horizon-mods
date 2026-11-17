// Gensokyo Legacy biome feature port, LGPL-2.1.
// Original algorithms: lcy0x1 and Signifier-Dollhouse; Horizon adaptation: eltanschauung.
use super::*;
use crate::worldgen::{
    anchor::WorldGenerationContext, feature_sort::FeatureIdentity, placement::LayerBlock,
};
use crate::worldgen::{
    state_provider::BlockStateProvider,
    structure::{Mirror, Rotation, StructureResources, StructureTemplate},
    tree::{TreeDecoder, read, sample_state, write},
    tree_blocks::{self as tb, TreeBlocks},
};
use crate::{heightmap::HeightmapType, registry::BlockStateId};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Debug)]
pub struct Feature {
    kind: String,
    v: Value,
    blocks: Arc<TreeBlocks>,
    states: BTreeMap<String, BlockStateId>,
    providers: BTreeMap<String, BlockStateProvider>,
    trunks: BTreeSet<BlockStateId>,
    caps: BTreeSet<BlockStateId>,
    templates: Vec<(Arc<StructureTemplate>, i32)>,
    resources: Option<Arc<StructureResources>>,
    posts: Vec<PlacedFeatureDefinition>,
    delegate: Option<Box<FeatureDefinition>>,
}
pub fn decode(
    mut v: Value,
    b: &FeatureBlockBindings<'_>,
    depth: usize,
    remaining: &mut usize,
    empty_tags: &BTreeSet<String>,
) -> Result<Feature, DecodeError> {
    if depth > 128 || *remaining == 0 {
        return Err(DecodeError::Field("Gensokyo feature budget"));
    }
    *remaining -= 1;
    let kind = v["type"].as_str().unwrap().to_owned();
    if let Some(c) = v.get("config").cloned() {
        v = c;
    }
    let mut f = Feature {
        kind: kind.clone(),
        v: v.clone(),
        blocks: b.tree_blocks()?,
        states: BTreeMap::new(),
        providers: BTreeMap::new(),
        trunks: BTreeSet::new(),
        caps: BTreeSet::new(),
        templates: Vec::new(),
        resources: b.structures.clone(),
        posts: Vec::new(),
        delegate: None,
    };
    fn collect(
        v: &Value,
        b: &FeatureBlockBindings<'_>,
        d: &mut BTreeMap<String, BlockStateId>,
    ) -> Result<(), DecodeError> {
        match v {
            Value::Object(o) => {
                if o.contains_key("id") {
                    d.insert(v.to_string(), b.predicate_state(&v.to_string())?);
                }
                for x in o.values() {
                    collect(x, b, d)?;
                }
            }
            Value::Array(a) => {
                for x in a {
                    collect(x, b, d)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    collect(&v, b, &mut f.states)?;
    if let Some(trunks) = b.tags.get("horizon_gensokyo_legacy:template_trunk") {
        f.trunks.extend(trunks.iter().copied());
    }
    for (name, states) in b.blocks.iter() {
        if name.ends_with("_mushroom_block")
            || name.ends_with("_mushroom_stem")
            || name == "minecraft:mushroom_stem"
        {
            f.caps.extend(states.iter().copied());
        }
    }

    if let Some(processors) = v["processors"]["processors"].as_array() {
        for proc in processors {
            if let Some(map) = proc["replacements"].as_object() {
                for (from, to) in map {
                    for name in [from.as_str(), to.as_str().unwrap()] {
                        f.states.insert(
                            name.into(),
                            b.predicate_state(&json!({"id":name}).to_string())?,
                        );
                    }
                }
            }
        }
    }

    for name in [
        "air",
        "cave_air",
        "dirt",
        "water",
        "big_dripleaf",
        "big_dripleaf_stem",
        "stone",
        "cobblestone",
        "end_stone",
        "obsidian",
        "mud",
    ] {
        f.states.insert(
            name.into(),
            b.predicate_state(&json!({"id":format!("minecraft:{name}")}).to_string())?,
        );
    }
    for key in ["to_place", "cap_provider", "stem_provider"] {
        if let Some(v) = v.get(key) {
            let raw: Box<RawValue> = serde_json::from_value(v.clone())?;
            let mut d = TreeDecoder {
                bindings: b,
                depth,
                remaining,
            };
            f.providers.insert(key.into(), d.provider(&raw)?);
        }
    }
    match kind.as_str() {
        "horizon_gensokyo_legacy:template" | "horizon_gensokyo_legacy:template_structure" => {
            let resources = f
                .resources
                .as_ref()
                .ok_or(DecodeError::Field("Gensokyo templates"))?;
            for e in v["templates"]
                .as_array()
                .ok_or(DecodeError::Field("Gensokyo template list"))?
            {
                let id = e["data"]
                    .as_str()
                    .ok_or(DecodeError::Field("Gensokyo template id"))?;
                let template = resources.template(id).ok_or_else(|| {
                    DecodeError::Registry(format!("missing Gensokyo template {id}"))
                })?;
                if template.palettes().is_empty() || template.size.iter().any(|v| *v <= 0) {
                    return Err(DecodeError::Registry(format!(
                        "empty Gensokyo template {id}"
                    )));
                }
                f.templates
                    .push((template, e["weight"].as_i64().unwrap_or(1) as i32));
            }
            if let Some(posts) = v["post_features"].as_array() {
                for p in posts {
                    f.posts.push(PlacedFeatureDefinition::decode(
                        &p.to_string(),
                        depth + 1,
                        remaining,
                        empty_tags,
                        Some(b),
                    )?);
                }
            }
        }
        "horizon_gensokyo_legacy:mud_waterlogged_vegetation_patch" => {
            let mut d = v.clone();
            d["type"] = json!("minecraft:waterlogged_vegetation_patch");
            d["vegetation_feature"] = json!({"feature":{"type":"horizon_gensokyo_legacy:mud_support","post_features":[d["vegetation_feature"].clone()]},"placement":[]});
            f.delegate = Some(Box::new(FeatureDefinition::decode(
                &d.to_string(),
                depth + 1,
                remaining,
                empty_tags,
                Some(b),
            )?));
        }
        "horizon_gensokyo_legacy:lake" => {
            for (i, e) in v["decorations"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                let raw: Box<RawValue> = serde_json::from_value(e["block"].clone())?;
                let mut d = TreeDecoder {
                    bindings: b,
                    depth,
                    remaining,
                };
                f.providers.insert(format!("deco{i}"), d.provider(&raw)?);
            }
        }
        "horizon_gensokyo_legacy:mud_support" => {
            for p in v["post_features"].as_array().unwrap() {
                f.posts.push(PlacedFeatureDefinition::decode(
                    &p.to_string(),
                    depth + 1,
                    remaining,
                    empty_tags,
                    Some(b),
                )?);
            }
        }
        "horizon_gensokyo_legacy:double_block"
        | "horizon_gensokyo_legacy:golden_lotus"
        | "horizon_gensokyo_legacy:dripleaf_scatter"
        | "horizon_gensokyo_legacy:ravine_cave"
        | "horizon_gensokyo_legacy:stair_pool"
        | "horizon_gensokyo_legacy:demonic_miasma_mushroom"
        | "horizon_gensokyo_legacy:dream_mushroom"
        | "horizon_gensokyo_legacy:ghost_fire_mushroom" => (),
        _ => {
            return Err(DecodeError::Registry(format!(
                "unsupported Gensokyo feature {kind}"
            )));
        }
    }
    Ok(f)
}
type ResultOf<T, E> = Result<T, FeatureExecutionError<E>>;
fn q<E>(e: PlacementQueryError) -> FeatureExecutionError<E> {
    FeatureExecutionError::Query(e)
}
fn off(p: [i32; 3], x: i32, y: i32, z: i32) -> [i32; 3] {
    [p[0] + x, p[1] + y, p[2] + z]
}
impl Feature {
    fn s(&self, v: &Value) -> BlockStateId {
        self.states[&v.to_string()]
    }
    fn air(&self) -> BlockStateId {
        self.states["air"]
    }
    fn soft(&self, s: BlockStateId) -> bool {
        self.blocks.is_air(s)
            || self
                .blocks
                .has(s, tb::REPLACEABLE | tb::REPLACEABLE_BY_TREES)
                && !self.blocks.has(s, tb::ANY_FLUID)
    }
    fn passable(&self, s: BlockStateId) -> bool {
        self.soft(s) || (self.blocks.has(s, tb::LOGS | tb::LEAVES) || self.caps.contains(&s))
    }
    fn solid(&self, s: BlockStateId) -> bool {
        self.blocks.has(s, tb::SOLID)
    }
    fn change(&self, s: BlockStateId, k: &str, v: &str) -> BlockStateId {
        self.blocks.with(s, k, v).unwrap_or(s)
    }
    fn put<W: FeatureWorld + PlacementContext>(
        &self,
        w: &mut W,
        p: [i32; 3],
        s: BlockStateId,
    ) -> ResultOf<bool, W::Error> {
        if !w.ensure_can_write(p).map_err(q)? || w.outside_build_height(p[1]).map_err(q)? {
            return Ok(false);
        }
        if self.blocks.has(read(w, p)?, tb::FEATURES_CANNOT_REPLACE) {
            return Ok(false);
        }
        write(w, p, s, 2)
    }
    fn place<W: FeatureWorld + PlacementContext>(
        &self,
        w: &mut W,
        r: &mut impl RandomSource,
        p: [i32; 3],
        run: &Run,
    ) -> ResultOf<bool, W::Error> {
        if let Some(f) = &self.delegate {
            return f.place_inner(w, r, p, run);
        }
        match self.kind.as_str() {
            "horizon_gensokyo_legacy:mud_support" => {
                self.put(w, off(p, 0, -1, 0), self.states["mud"])?;
                for post in &self.posts {
                    post.place_in(&mut FeaturePlacer::new(), w, r, p, run)?;
                }
                Ok(true)
            }
            "horizon_gensokyo_legacy:template" | "horizon_gensokyo_legacy:template_structure" => {
                self.template(w, r, p, run)
            }
            "horizon_gensokyo_legacy:double_block" | "horizon_gensokyo_legacy:golden_lotus" => {
                if !self.blocks.is_air(read(w, p)?) {
                    return Ok(false);
                }
                let state = sample_state(&self.providers["to_place"], w, r, p)?;
                let lotus = self.kind.ends_with("golden_lotus");
                let large = lotus && self.blocks.with(state, "size", "2") == Some(state);
                if !lotus || large {
                    let top = off(p, 0, 1, 0);
                    if !self.blocks.is_air(read(w, top)?) {
                        return Ok(false);
                    }
                    self.put(
                        w,
                        p,
                        self.change(
                            state,
                            if lotus { "part" } else { "half" },
                            if lotus { "lower" } else { "bottom" },
                        ),
                    )?;
                    self.put(
                        w,
                        top,
                        self.change(
                            state,
                            if lotus { "part" } else { "half" },
                            if lotus { "upper" } else { "top" },
                        ),
                    )?;
                } else {
                    self.put(w, p, state)?;
                }
                Ok(true)
            }
            "horizon_gensokyo_legacy:lake" => self.lake(w, r, p),
            "horizon_gensokyo_legacy:ravine_cave" => self.ravine(w, r, p),
            "horizon_gensokyo_legacy:stair_pool" => self.stair(w, p),
            "horizon_gensokyo_legacy:dripleaf_scatter" => self.dripleaf(w, r, p),
            _ => self.mushroom(w, r, p),
        }
    }
    fn template<W: FeatureWorld + PlacementContext>(
        &self,
        w: &mut W,
        r: &mut impl RandomSource,
        mut p: [i32; 3],
        run: &Run,
    ) -> ResultOf<bool, W::Error> {
        for _ in 0..8 {
            if self.passable(read(w, p)?) {
                break;
            }
            p[1] += 1;
        }
        let mut found = false;
        for _ in 0..48 {
            if !self.passable(read(w, p)?) {
                found = true;
                break;
            }
            p[1] -= 1;
        }
        if !found
            || !w
                .block_matches_tag(
                    p,
                    self.v["ground"].as_str().unwrap().trim_start_matches('#'),
                )
                .map_err(q)?
        {
            return Ok(false);
        }
        let radius = self.v["footprint_radius"].as_i64().unwrap() as i32;
        let slope = self.v["max_slope"].as_i64().unwrap() as i32;
        let mut low = p[1];
        let mut high = p[1];
        if radius > 0 {
            for x in -1..=1 {
                for z in -1..=1 {
                    if x == 0 && z == 0 {
                        continue;
                    }
                    let mut t = off(p, x * radius, slope + 1, z * radius);
                    if !self.passable(read(w, t)?) {
                        return Ok(false);
                    }
                    let mut hit = false;
                    for _ in 0..=2 * slope {
                        t[1] -= 1;
                        if !self.passable(read(w, t)?) {
                            hit = true;
                            break;
                        }
                    }
                    if !hit || self.blocks.has(read(w, off(t, 0, 1, 0))?, tb::ANY_FLUID) {
                        return Ok(false);
                    }
                    low = low.min(t[1]);
                    high = high.max(t[1]);
                }
            }
            if high - low > slope {
                return Ok(false);
            }
        }
        if self.blocks.has(read(w, off(p, 0, 1, 0))?, tb::ANY_FLUID) {
            return Ok(false);
        }
        for dx in -1..=1 {
            for dz in -1..=1 {
                for dy in 1..=5 {
                    let old = read(w, off(p, dx, dy, dz))?;
                    if self.trunks.contains(&old) || self.caps.contains(&old) {
                        return Ok(false);
                    }
                }
            }
        }
        p[1] += 1 + self.v["y_offset"].as_i64().unwrap_or(0) as i32;
        let total = self.templates.iter().map(|e| e.1).sum();
        let mut n = r.next_int_bound(total)?;
        let mut template = &self.templates[0].0;
        for (t, weight) in &self.templates {
            n -= weight;
            if n < 0 {
                template = t;
                break;
            }
        }
        if w.outside_build_height(p[1] + template.size[1]).map_err(q)? {
            return Ok(false);
        }
        let rotation = Rotation::ALL[r.next_int_bound(4)? as usize];
        let mirror = if r.next_boolean() {
            Mirror::None
        } else {
            Mirror::FrontBack
        };
        let resources = self.resources.as_ref().unwrap();
        let anchor = StructureTemplate::transform(
            [template.size[0] / 2, 0, template.size[2] / 2],
            mirror,
            rotation,
            [0; 3],
        );
        let start = off(p, -anchor[0], 0, -anchor[2]);
        let palette = &template.palettes()[template.palette_index(r)];
        let root = self.v["root_depth"].as_i64().unwrap() as i32;
        for info in palette.blocks() {
            let shift = StructureTemplate::transform(info.pos, mirror, rotation, [0; 3]);
            let at = off(start, shift[0], shift[1], shift[2]);
            let old = read(w, at)?;
            let mut state = resources
                .transforms
                .rotate(resources.transforms.mirror(info.state, mirror), rotation);
            if self.blocks.is_air(state) {
                continue;
            }
            if let Some(processors) = self.v["processors"]["processors"].as_array() {
                for proc in processors {
                    if let Some(map) = proc["replacements"].as_object() {
                        for (from, to) in map {
                            if let (Some(a), Some(b)) =
                                (self.states.get(from), self.states.get(to.as_str().unwrap()))
                            {
                                if self.blocks.same_block(state, *a) {
                                    state = self.blocks.properties.copy(*b, state).unwrap_or(*b);
                                }
                            }
                        }
                    }
                }
            }
            let trunk = self.trunks.contains(&state) || self.blocks.has(state, tb::LOGS);
            let foliage = self.blocks.has(state, tb::LEAVES) || self.caps.contains(&state);
            let can = if self.kind.ends_with("template_structure") {
                !self.blocks.has(old, tb::FEATURES_CANNOT_REPLACE)
            } else if trunk {
                self.soft(old) || w.block_matches_tag(at, "minecraft:dirt").map_err(q)?
            } else if foliage {
                self.soft(old) && !self.blocks.has(old, tb::LEAVES)
            } else {
                self.blocks.is_air(old)
            };
            if can {
                self.put(w, at, state)?;
                if info.pos[1] == 0 && trunk {
                    for dy in 1..=root {
                        let below = off(at, 0, -dy, 0);
                        if !self.soft(read(w, below)?) {
                            break;
                        }
                        self.put(w, below, state)?;
                    }
                }
            }
        }
        for post in &self.posts {
            post.place_in(&mut FeaturePlacer::new(), w, r, p, run)?;
        }
        Ok(true)
    }
}
impl Feature {
    fn mushroom<W: FeatureWorld + PlacementContext>(
        &self,
        w: &mut W,
        r: &mut impl RandomSource,
        p: [i32; 3],
    ) -> ResultOf<bool, W::Error> {
        let ghost = self.kind.ends_with("ghost_fire_mushroom");
        let dream = self.kind.ends_with("dream_mushroom");
        let height = if ghost {
            3 + r.next_int_bound(2)?
        } else {
            let h = 4 + r.next_int_bound(3)?;
            if r.next_int_bound(12)? == 0 { h * 2 } else { h }
        };
        let radius = self.v["foliage_radius"].as_i64().unwrap_or(2) as i32;
        let layers = if dream { 3 } else { 2 };
        if !w
            .block_matches_tag(off(p, 0, -1, 0), "minecraft:dirt")
            .map_err(q)?
            && !w
                .block_matches_tag(off(p, 0, -1, 0), "minecraft:mushroom_grow_block")
                .map_err(q)?
        {
            return Ok(false);
        }
        if w.outside_build_height(p[1] + height + 3).map_err(q)? {
            return Ok(false);
        }
        for y in 0..=height {
            let rad = if y >= height - layers { radius } else { 0 };
            for x in -rad..=rad {
                for z in -rad..=rad {
                    let old = read(w, off(p, x, y, z))?;
                    if !self.soft(old) && !self.blocks.has(old, tb::LEAVES) {
                        return Ok(false);
                    }
                }
            }
        }
        let lo = if ghost { height - 1 } else { height - layers };
        let hi = if ghost { height + 2 } else { height };
        for y in lo..=hi {
            let rad = if ghost {
                if y == lo { 2 } else { 1 }
            } else {
                if y < height { radius } else { radius - 1 }
            };
            for x in -rad..=rad {
                for z in -rad..=rad {
                    let horizontal = x.abs() == rad;
                    let vertical = z.abs() == rad;
                    if !ghost && y < height && horizontal == vertical {
                        continue;
                    }
                    let at = off(p, x, y, z);
                    if self.blocks.has(read(w, at)?, tb::SOLID_RENDER) {
                        continue;
                    }
                    let mut state = sample_state(&self.providers["cap_provider"], w, r, p)?;
                    let k = if ghost { 0 } else { radius - 2 };
                    let up = if ghost {
                        y >= height + 1 || y == lo && x.abs().max(z.abs()) == rad
                    } else {
                        y >= height - 1
                    };
                    for (name, val) in [
                        ("up", up),
                        ("west", x < -k),
                        ("east", x > k),
                        ("north", z < -k),
                        ("south", z > k),
                    ] {
                        state = self.change(state, name, if val { "true" } else { "false" });
                    }
                    self.put(w, at, state)?;
                }
            }
        }
        for y in 0..height {
            let at = off(p, 0, y, 0);
            if !self.blocks.has(read(w, at)?, tb::SOLID_RENDER) {
                let state = sample_state(&self.providers["stem_provider"], w, r, p)?;
                self.put(w, at, state)?;
            }
        }
        Ok(true)
    }
    fn dripleaf<W: FeatureWorld + PlacementContext>(
        &self,
        w: &mut W,
        r: &mut impl RandomSource,
        p: [i32; 3],
    ) -> ResultOf<bool, W::Error> {
        let ground = off(p, 0, -1, 0);
        if !self
            .blocks
            .same_block(read(w, ground)?, self.s(&self.v["stone"]))
        {
            return Ok(false);
        }
        for (x, z) in [(0, 1), (-1, 0), (0, -1), (1, 0)] {
            if self
                .blocks
                .has(read(w, off(ground, x, 0, z))?, tb::WATER_FLUID)
            {
                return Ok(false);
            }
        }
        let facing = ["south", "west", "north", "east"][r.next_int_bound(4)? as usize];
        let h = if r.next_int_bound(3)? == 0 {
            0
        } else {
            r.next_int_bound(5)?
        };
        for y in 0..=h {
            if !self.soft(read(w, off(p, 0, y, 0))?) {
                return Ok(false);
            }
        }
        self.put(w, ground, self.s(&self.v["ground"]))?;
        for y in 0..=h {
            let state = self.change(
                self.states[if y == h {
                    "big_dripleaf"
                } else {
                    "big_dripleaf_stem"
                }],
                "facing",
                facing,
            );
            self.put(w, off(p, 0, y, 0), state)?;
        }
        Ok(true)
    }
    fn middle<W: FeatureWorld + PlacementContext>(
        &self,
        w: &mut W,
        p: [i32; 3],
    ) -> ResultOf<bool, W::Error> {
        let stair = self.s(&self.v["stair"]);
        if !self.blocks.same_block(read(w, p)?, stair)
            || !self.blocks.is_air(read(w, off(p, 0, 1, 0))?)
        {
            return Ok(false);
        }
        let mut higher = false;
        let mut lower = false;
        for (x, z) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let side = off(p, x, 0, z);
            let state = read(w, side)?;
            if self.blocks.has(state, tb::WATER_FLUID) {
                return Ok(false);
            }
            if self.blocks.same_block(state, stair) {
                higher |= self.blocks.same_block(read(w, off(side, 0, 1, 0))?, stair);
            } else if self.blocks.is_air(state) {
                let low = read(w, off(side, 0, -1, 0))?;
                if self.blocks.same_block(low, stair) {
                    lower = true
                } else if !self.solid(low) && !self.blocks.has(low, tb::WATER_FLUID) {
                    return Ok(false);
                }
            }
        }
        Ok(higher
            && lower
            && self.solid(read(w, off(p, 0, -1, 0))?)
            && self.solid(read(w, off(p, 0, -2, 0))?))
    }
    fn stair<W: FeatureWorld + PlacementContext>(
        &self,
        w: &mut W,
        p: [i32; 3],
    ) -> ResultOf<bool, W::Error> {
        let mut start = off(p, 0, -1, 0);
        if !self
            .blocks
            .same_block(read(w, start)?, self.s(&self.v["stair"]))
        {
            return Ok(false);
        }
        if !self.middle(w, start)? {
            let mut found = false;
            'outer: for x in -2..=2 {
                for z in -2..=2 {
                    let y = w
                        .height(HeightmapType::MotionBlockingNoLeaves, p[0] + x, p[2] + z)
                        .map_err(q)?;
                    let at = [p[0] + x, y, p[2] + z];
                    if self.middle(w, at)? {
                        start = at;
                        found = true;
                        break 'outer;
                    }
                }
            }
            if !found {
                return Ok(false);
            }
        }
        let reach = self.v["reach"].as_i64().unwrap() as i32;
        let mut queue = std::collections::VecDeque::from([start]);
        let mut seen = BTreeSet::new();
        while let Some(at) = queue.pop_front() {
            let dx = at[0] - start[0];
            let dz = at[2] - start[2];
            if dx * dx + dz * dz > reach * reach || !seen.insert(at) || !self.middle(w, at)? {
                continue;
            }
            self.put(w, at, self.air())?;
            self.put(w, off(at, 0, -1, 0), self.s(&self.v["fill"]))?;
            for x in -1..=1 {
                for z in -1..=1 {
                    if x != 0 || z != 0 {
                        queue.push_back(off(at, x, 0, z));
                    }
                }
            }
        }
        Ok(true)
    }
}
impl Feature {
    fn height<W: FeatureWorld + PlacementContext>(
        &self,
        w: &mut W,
        x: i32,
        z: i32,
    ) -> ResultOf<i32, W::Error> {
        w.height(HeightmapType::MotionBlockingNoLeaves, x, z)
            .map_err(q)
    }
    fn ravine<W: FeatureWorld + PlacementContext>(
        &self,
        w: &mut W,
        r: &mut impl RandomSource,
        p: [i32; 3],
    ) -> ResultOf<bool, W::Error> {
        let mut best = None;
        let mut margin = self.v["gap"].as_f64().unwrap();
        for across in [true, false] {
            let mut means = [0.; 2];
            for (side, index) in [-1, 1].iter().enumerate() {
                for along in -3..=3 {
                    means[side] += self.height(
                        w,
                        p[0] + if across { *index } else { along },
                        p[2] + if across { along } else { *index },
                    )? as f64
                        / 7.;
                }
            }
            let diff = (means[0] - means[1]).abs();
            if diff <= margin {
                continue;
            }
            let h = [
                self.height(
                    w,
                    p[0] - if across { 1 } else { 0 },
                    p[2] - if across { 0 } else { 1 },
                )?,
                self.height(w, p[0], p[2])?,
                self.height(
                    w,
                    p[0] + if across { 1 } else { 0 },
                    p[2] + if across { 0 } else { 1 },
                )?,
            ];
            let i = if (h[1] - h[0]).abs() >= (h[2] - h[1]).abs() {
                0
            } else {
                1
            };
            let (outer, inner) = if h[i] <= h[i + 1] {
                (i as i32 - 1, i as i32)
            } else {
                (i as i32, i as i32 - 1)
            };
            let dir = if inner > outer { 1 } else { -1 };
            best = Some((
                p[0] + if across { outer } else { 0 },
                p[2] + if across { 0 } else { outer },
                if across { dir } else { 0 },
                if across { 0 } else { dir },
            ));
            margin = diff;
        }
        let Some((x, z, dx, dz)) = best else {
            return Ok(false);
        };
        let floor = self.height(w, x, z)?;
        let top = self.height(w, x + dx, z + dz)?;
        let height = (self.v["height"].as_i64().unwrap() as i32).min(top - 2 - floor);
        if height < 3 {
            return Ok(false);
        }
        let width = self.v["width"].as_i64().unwrap() as i32;
        let depth = self.v["depth"].as_i64().unwrap() as i32;
        let mut cols = Vec::new();
        for u in -width / 2..=width / 2 {
            for d in 1..=depth {
                let span = 1.
                    - (u as f64 / (width as f64 / 2.)).powi(2)
                    - (d as f64 / depth as f64).powi(2);
                if span <= 0. {
                    continue;
                }
                let cx = x + d * dx - u * dz;
                let cz = z + d * dz + u * dx;
                let top = floor + (height as f64 * span.sqrt()).floor() as i32;
                if !self.solid(read(w, [cx, floor - 2, cz])?)
                    || !self.solid(read(w, [cx, top + 1, cz])?)
                {
                    return Ok(false);
                }
                cols.push((cx, cz, top, d));
            }
        }
        let mut lining = BTreeMap::new();
        for &(cx, cz, top, _) in &cols {
            for (sx, sz) in [(0, 1), (-1, 0), (0, -1), (1, 0)] {
                lining
                    .entry((cx + sx, cz + sz))
                    .and_modify(|v: &mut i32| *v = (*v).max(top))
                    .or_insert(top);
            }
        }
        for &(cx, cz, top, d) in &cols {
            let mut room = false;
            for y in floor..=top {
                let at = [cx, y, cz];
                if !self.blocks.is_air(read(w, at)?) {
                    self.put(w, at, self.states["cave_air"])?;
                    room = true;
                }
            }
            if !room {
                continue;
            }
            let basalt = self.s(&self.v["floor"]);
            self.put(
                w,
                [cx, floor - 1, cz],
                if d <= 1 {
                    basalt
                } else {
                    self.s(&self.v["pool"])
                },
            )?;
            self.put(w, [cx, floor - 2, cz], basalt)?;
            self.put(w, [cx, top + 1, cz], basalt)?;
            if top - floor > 4 && r.next_boolean() {
                self.put(w, [cx, top, cz], basalt)?;
                if top - floor > 5 && r.next_boolean() {
                    self.put(w, [cx, top - 1, cz], basalt)?;
                }
            }
        }
        for ((cx, cz), top) in lining {
            for y in floor..=top {
                if !self.blocks.is_air(read(w, [cx, y, cz])?) {
                    self.put(w, [cx, y, cz], self.s(&self.v["floor"]))?;
                }
            }
        }
        Ok(true)
    }
    fn lake<W: FeatureWorld + PlacementContext>(
        &self,
        w: &mut W,
        r: &mut impl RandomSource,
        p: [i32; 3],
    ) -> ResultOf<bool, W::Error> {
        let shape = &self.v["shape"];
        let get = |key: &str| shape[key].as_i64().unwrap() as i32;
        let width = get("max_width");
        let height = get("max_height");
        let depth = get("depth");
        let margin = get("margin");
        let radius = get("component_radius") as f64;
        let mut ground = p;
        for _ in 0..48 {
            if self.solid(read(w, off(ground, 0, -1, 0))?) {
                break;
            }
            ground[1] -= 1;
        }
        let origin = off(ground, -width / 2, -depth, -width / 2);
        let count =
            get("min_component") + r.next_int_bound(get("max_component") - get("min_component"))?;
        let mut cells = BTreeSet::new();
        let thick = (depth + 1).min(height - 2) as f64;
        for _ in 0..count {
            let rx = (r.next_double() + 0.5) * radius;
            let ry = (r.next_double() + 0.5) * thick;
            let rz = (r.next_double() + 0.5) * radius;
            let x = r.next_double() * (width as f64 - rx - 2. * margin as f64)
                + margin as f64
                + rx / 2.;
            let y = -1. + ry / 2. + r.next_double() * (depth as f64 - ry).max(0.);
            let z = r.next_double() * (width as f64 - rz - 2. * margin as f64)
                + margin as f64
                + rz / 2.;
            for xx in margin..width - margin {
                for zz in margin..width - margin {
                    for yy in 0..height {
                        if ((xx as f64 - x) / (rx / 2.)).powi(2)
                            + ((yy as f64 - y) / (ry / 2.)).powi(2)
                            + ((zz as f64 - z) / (rz / 2.)).powi(2)
                            < 1.
                        {
                            cells.insert([xx, yy, zz]);
                        }
                    }
                }
            }
        }
        // Level the pond's connected water body and validate its complete wet rim before writes.
        let mut footprint = BTreeSet::new();
        for &[x, y, z] in &cells {
            if y < depth {
                footprint.insert((x, z));
            }
        }
        for &(x, z) in &footprint {
            for y in 0..depth {
                cells.insert([x, y, z]);
            }
        }
        let mut edge = BTreeSet::new();
        for &at in &cells {
            for (x, y, z) in [
                (1, 0, 0),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ] {
                let q = off(at, x, y, z);
                if !cells.contains(&q) {
                    edge.insert(q);
                }
            }
        }
        for at in cells.iter().chain(edge.iter()) {
            let pos = off(origin, at[0], at[1], at[2]);
            if !w.ensure_can_write(pos).map_err(q)? || w.outside_build_height(pos[1]).map_err(q)? {
                return Ok(false);
            }
            let old = read(w, pos)?;
            if self.blocks.has(old, tb::FEATURES_CANNOT_REPLACE) {
                return Ok(false);
            }
            if edge.contains(at) && at[1] < depth && !self.solid(old) {
                return Ok(false);
            }
            if at[1] >= depth && self.blocks.has(old, tb::ANY_FLUID) {
                return Ok(false);
            }
        }
        let material = &self.v["material"];
        let fluid = self.s(&material["fluid"]);
        let padding = self.s(&material["padding"]);
        let barrier = material
            .get("barrier")
            .map(|v| self.s(v))
            .unwrap_or(padding);
        let surface = material
            .get("surface")
            .map(|v| self.s(v))
            .unwrap_or(padding);
        for at in edge {
            let pos = off(origin, at[0], at[1], at[2]);
            if at[1] < depth {
                self.put(w, pos, barrier)?;
            } else if self.solid(read(w, pos)?) {
                self.put(w, pos, padding)?;
            }
        }
        for at in cells {
            self.put(
                w,
                off(origin, at[0], at[1], at[2]),
                if at[1] < depth { fluid } else { self.air() },
            )?;
        }
        for &(x, z) in &footprint {
            let water = off(origin, x, depth - 1, z);
            if !self.blocks.same_block(read(w, water)?, fluid) {
                continue;
            }
            for (i, deco) in self.v["decorations"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                let target = deco["target"].as_str().unwrap();
                let at = off(water, 0, 1, 0);
                if target == "floating"
                    && self.blocks.is_air(read(w, at)?)
                    && r.next_float() < (deco["chance"].as_f64().unwrap() as f32)
                {
                    let state = sample_state(&self.providers[&format!("deco{i}")], w, r, at)?;
                    self.put(w, at, state)?;
                    break;
                }
                if target == "shallow"
                    && self.solid(read(w, off(water, 0, -1, 0))?)
                    && w.block_matches_tag(
                        off(water, 0, -1, 0),
                        deco["soil"]
                            .as_str()
                            .unwrap_or("#minecraft:dirt")
                            .trim_start_matches('#'),
                    )
                    .map_err(q)?
                    && r.next_float() < (deco["chance"].as_f64().unwrap() as f32)
                {
                    let state = sample_state(&self.providers[&format!("deco{i}")], w, r, water)?;
                    self.put(w, water, state)?;
                    if let Some(upper) = deco.get("upper") {
                        self.put(w, at, self.s(upper))?;
                    }
                    break;
                }
            }
        }
        // Cap exposed dry rim with the source biome's surface material.
        for x in 0..width {
            for z in 0..width {
                if footprint.contains(&(x, z)) {
                    continue;
                }
                let at = off(origin, x, depth - 1, z);
                if self.solid(read(w, at)?) && self.blocks.is_air(read(w, off(at, 0, 1, 0))?) {
                    self.put(w, at, surface)?;
                    let mut wet = false;
                    for (dx, dz) in [(0, 1), (-1, 0), (0, -1), (1, 0)] {
                        wet |= self
                            .blocks
                            .has(read(w, off(at, dx, 0, dz))?, tb::WATER_FLUID);
                        wet |= self
                            .blocks
                            .has(read(w, off(at, dx, 1, dz))?, tb::WATER_FLUID);
                    }
                    if wet {
                        for (i, deco) in self.v["decorations"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .enumerate()
                        {
                            if deco["target"].as_str() != Some("shore") {
                                continue;
                            }
                            let top = off(at, 0, 1, 0);
                            if !self.blocks.is_air(read(w, top)?) {
                                break;
                            }
                            if w.block_matches_tag(
                                at,
                                deco["soil"]
                                    .as_str()
                                    .unwrap_or("#minecraft:dirt")
                                    .trim_start_matches('#'),
                            )
                            .map_err(q)?
                                && r.next_float() < deco["chance"].as_f64().unwrap() as f32
                            {
                                let state =
                                    sample_state(&self.providers[&format!("deco{i}")], w, r, top)?;
                                self.put(w, top, state)?;
                            }
                        }
                    }
                }
            }
        }
        Ok(!footprint.is_empty())
    }
}

use crate::worldgen::native_feature::{NativeAlgorithm, NativeRandom, NativeWorld};
struct WorldAdapter<'a>(
    &'a mut dyn NativeWorld,
    crate::worldgen::anchor::WorldGenerationContext,
    Option<FeatureIdentity>,
);
impl FeatureWorld for WorldAdapter<'_> {
    type Error = PlacementQueryError;
    fn block(&mut self, p: [i32; 3]) -> Result<BlockStateId, Self::Error> {
        self.0.block(p)
    }
    fn set_block(&mut self, p: [i32; 3], s: BlockStateId, f: i32) -> Result<bool, Self::Error> {
        self.0.set(p, s, f)
    }
}
impl PlacementContext for WorldAdapter<'_> {
    fn update_shape_at_face(
        &mut self,
        _position: [i32; 3],
        _direction: horizon_core::geometry::Direction,
        _flags: i32,
    ) -> Result<(), PlacementQueryError> {
        self.0
            .context()
            .update_shape_at_face(_position, _direction, _flags)
    }
    fn update_from_neighbour_shapes(
        &mut self,
        _position: [i32; 3],
        _flags: i32,
    ) -> Result<(), PlacementQueryError> {
        self.0
            .context()
            .update_from_neighbour_shapes(_position, _flags)
    }
    fn ensure_can_write(&mut self, _position: [i32; 3]) -> Result<bool, PlacementQueryError> {
        self.0.context().ensure_can_write(_position)
    }
    fn section_block(
        &mut self,
        position: [i32; 3],
    ) -> Result<Option<crate::registry::BlockStateId>, PlacementQueryError> {
        self.0.context().section_block(position)
    }
    fn set_section_block(
        &mut self,
        _position: [i32; 3],
        _state: crate::registry::BlockStateId,
    ) -> Result<(), PlacementQueryError> {
        self.0.context().set_section_block(_position, _state)
    }
    fn set_chunk_block(
        &mut self,
        position: [i32; 3],
        state: crate::registry::BlockStateId,
    ) -> Result<(), PlacementQueryError> {
        self.0.context().set_chunk_block(position, state)
    }
    fn collision_full_block(
        &mut self,
        _state: crate::registry::BlockStateId,
        _position: [i32; 3],
    ) -> Result<bool, PlacementQueryError> {
        self.0.context().collision_full_block(_state, _position)
    }
    fn spawn_end_crystal(
        &mut self,
        _position: [f64; 3],
        _yaw: f32,
        _beam_target: Option<[i32; 3]>,
        _invulnerable: bool,
    ) -> Result<(), PlacementQueryError> {
        self.0
            .context()
            .spawn_end_crystal(_position, _yaw, _beam_target, _invulnerable)
    }
    fn world_seed(&mut self) -> Result<i64, PlacementQueryError> {
        self.0.context().world_seed()
    }
    fn configure_spawner(
        &mut self,
        _position: [i32; 3],
        _entity: &str,
    ) -> Result<(), PlacementQueryError> {
        self.0.context().configure_spawner(_position, _entity)
    }
    fn store_beehive_occupants(
        &mut self,
        _position: [i32; 3],
        _ticks_in_hive: &[i32],
    ) -> Result<(), PlacementQueryError> {
        self.0
            .context()
            .store_beehive_occupants(_position, _ticks_in_hive)
    }
    fn state_collision_empty(
        &mut self,
        _state: crate::registry::BlockStateId,
        _position: [i32; 3],
    ) -> Result<bool, PlacementQueryError> {
        self.0.context().state_collision_empty(_state, _position)
    }
    fn configure_end_gateway(
        &mut self,
        _position: [i32; 3],
        _exit: [i32; 3],
        _exact: bool,
    ) -> Result<(), PlacementQueryError> {
        self.0
            .context()
            .configure_end_gateway(_position, _exit, _exact)
    }
    fn biome_should_freeze(
        &mut self,
        _biome: crate::worldgen::biome::BiomeId,
        _position: [i32; 3],
        _require_edge: bool,
    ) -> Result<bool, PlacementQueryError> {
        self.0
            .context()
            .biome_should_freeze(_biome, _position, _require_edge)
    }
    fn biome_should_snow(
        &mut self,
        _biome: crate::worldgen::biome::BiomeId,
        _position: [i32; 3],
    ) -> Result<bool, PlacementQueryError> {
        self.0.context().biome_should_snow(_biome, _position)
    }
    fn block_brightness(&mut self, _position: [i32; 3]) -> Result<i32, PlacementQueryError> {
        self.0.context().block_brightness(_position)
    }
    fn fluid_state(
        &mut self,
        _position: [i32; 3],
    ) -> Result<crate::fluid::FluidState, PlacementQueryError> {
        self.0.context().fluid_state(_position)
    }
    fn destroy_block_without_entity(
        &mut self,
        _position: [i32; 3],
        _drop_resources: bool,
    ) -> Result<bool, PlacementQueryError> {
        self.0
            .context()
            .destroy_block_without_entity(_position, _drop_resources)
    }
    fn sea_level(&mut self) -> Result<i32, PlacementQueryError> {
        self.0.context().sea_level()
    }
    fn schedule_fluid_tick(
        &mut self,
        _position: [i32; 3],
        _fluid: crate::fluid::FluidKind,
        _delay: i32,
    ) -> Result<(), PlacementQueryError> {
        self.0
            .context()
            .schedule_fluid_tick(_position, _fluid, _delay)
    }
    fn mark_postprocessing(&mut self, _position: [i32; 3]) -> Result<(), PlacementQueryError> {
        self.0.context().mark_postprocessing(_position)
    }
    fn water_at(&mut self, _position: [i32; 3]) -> Result<bool, PlacementQueryError> {
        self.0.context().water_at(_position)
    }
    fn can_occlude(
        &mut self,
        _state: crate::registry::BlockStateId,
    ) -> Result<bool, PlacementQueryError> {
        self.0.context().can_occlude(_state)
    }
    fn attachment_at(
        &mut self,
        _position: [i32; 3],
        _direction: horizon_core::geometry::Direction,
    ) -> Result<bool, PlacementQueryError> {
        self.0.context().attachment_at(_position, _direction)
    }
    fn next_world_boolean(&mut self) -> Result<bool, PlacementQueryError> {
        self.0.context().next_world_boolean()
    }
    fn next_world_float(&mut self) -> Result<f32, PlacementQueryError> {
        self.0.context().next_world_float()
    }
    fn next_world_int(&mut self, _bound: i32) -> Result<i32, PlacementQueryError> {
        self.0.context().next_world_int(_bound)
    }
    fn schedule_block_tick(
        &mut self,
        _position: [i32; 3],
        _block: crate::registry::BlockStateId,
        _delay: i32,
    ) -> Result<(), PlacementQueryError> {
        self.0
            .context()
            .schedule_block_tick(_position, _block, _delay)
    }
    fn block_survives(
        &mut self,
        _state: crate::registry::BlockStateId,
        _position: [i32; 3],
    ) -> Result<bool, PlacementQueryError> {
        self.0.context().block_survives(_state, _position)
    }
    fn state_face_sturdy(
        &mut self,
        _state: crate::registry::BlockStateId,
        _position: [i32; 3],
        _direction: horizon_core::geometry::Direction,
    ) -> Result<bool, PlacementQueryError> {
        self.0
            .context()
            .state_face_sturdy(_state, _position, _direction)
    }
    fn face_sturdy(
        &mut self,
        _position: [i32; 3],
        _direction: horizon_core::geometry::Direction,
    ) -> Result<bool, PlacementQueryError> {
        self.0.context().face_sturdy(_position, _direction)
    }
    fn center_sturdy(
        &mut self,
        _position: [i32; 3],
        _direction: horizon_core::geometry::Direction,
    ) -> Result<bool, PlacementQueryError> {
        self.0.context().center_sturdy(_position, _direction)
    }
    fn unobstructed_block(&mut self, _position: [i32; 3]) -> Result<bool, PlacementQueryError> {
        self.0.context().unobstructed_block(_position)
    }
    fn biome_identity(
        &mut self,
        _position: [i32; 3],
    ) -> Result<crate::worldgen::biome::BiomeId, PlacementQueryError> {
        self.0.context().biome_identity(_position)
    }
    fn top_feature(&self) -> Option<FeatureIdentity> {
        self.2
    }
    fn biome_allows_feature(
        &mut self,
        _position: [i32; 3],
        _feature: FeatureIdentity,
    ) -> Result<bool, PlacementQueryError> {
        self.0.context().biome_allows_feature(_position, _feature)
    }
    fn block_state(
        &mut self,
        _position: [i32; 3],
    ) -> Result<crate::registry::BlockStateId, PlacementQueryError> {
        self.0.context().block_state(_position)
    }
    fn block_matches_tag(
        &mut self,
        _position: [i32; 3],
        _tag: &str,
    ) -> Result<bool, PlacementQueryError> {
        self.0.context().block_matches_tag(_position, _tag)
    }
    fn max_y(&mut self) -> Result<i32, PlacementQueryError> {
        self.0.context().max_y()
    }
    fn inside_build_height(&mut self, position: [i32; 3]) -> Result<bool, PlacementQueryError> {
        self.0.context().inside_build_height(position)
    }
    fn outside_build_height(&mut self, y: i32) -> Result<bool, PlacementQueryError> {
        self.0.context().outside_build_height(y)
    }
    fn layer_block(&mut self, _position: [i32; 3]) -> Result<LayerBlock, PlacementQueryError> {
        self.0.context().layer_block(_position)
    }
    fn add_entity(&mut self, _nbt: crate::nbt::Compound) -> Result<(), PlacementQueryError> {
        self.0.context().add_entity(_nbt)
    }
    fn set_block_entity(
        &mut self,
        _position: [i32; 3],
        _nbt: crate::nbt::Compound,
    ) -> Result<(), PlacementQueryError> {
        self.0.context().set_block_entity(_position, _nbt)
    }
    fn generation(&self) -> crate::worldgen::anchor::WorldGenerationContext {
        self.1
    }
    fn predicate_generation(&mut self) -> Result<WorldGenerationContext, PlacementQueryError> {
        self.0.context().predicate_generation()
    }
    fn min_y(&mut self) -> Result<i32, PlacementQueryError> {
        self.0.context().min_y()
    }
    fn height(&mut self, kind: HeightmapType, x: i32, z: i32) -> Result<i32, PlacementQueryError> {
        self.0.context().height(kind, x, z)
    }
    fn any_height_at_least(
        &mut self,
        _kind: HeightmapType,
        _corner: [i32; 2],
        _size: i32,
        _y: i32,
    ) -> Result<Option<bool>, PlacementQueryError> {
        self.0
            .context()
            .any_height_at_least(_kind, _corner, _size, _y)
    }
}
struct RandomAdapter<'a>(&'a mut dyn NativeRandom);
struct UnusedPositional<'a>(std::marker::PhantomData<&'a mut ()>);
impl<'a> horizon_core::random::PositionalRandom for UnusedPositional<'a> {
    type Source = RandomAdapter<'a>;
    fn at(&self, _position: horizon_core::position::BlockPos) -> Self::Source {
        panic!("native positional fork unavailable")
    }
    fn from_hash_of(&self, _name: &str) -> Self::Source {
        panic!("native positional fork unavailable")
    }
    fn from_seed(&self, _seed: i64) -> Self::Source {
        panic!("native positional fork unavailable")
    }
    fn from_hash_of_utf16(&self, _name: &[u16]) -> Self::Source {
        panic!("native positional fork unavailable")
    }
}
impl<'a> RandomSource for RandomAdapter<'a> {
    type Positional = UnusedPositional<'a>;
    fn set_seed(&mut self, _seed: i64) {
        panic!("native random reseed unavailable")
    }
    fn next_int(&mut self) -> i32 {
        self.0.integer()
    }
    fn next_int_bound(&mut self, b: i32) -> Result<i32, InvalidBound> {
        self.0.int(b).map_err(|_| InvalidBound)
    }
    fn next_long(&mut self) -> i64 {
        self.0.long()
    }
    fn next_boolean(&mut self) -> bool {
        self.0.boolean()
    }
    fn next_float(&mut self) -> f32 {
        self.0.float()
    }
    fn next_double(&mut self) -> f64 {
        self.0.double()
    }
    fn next_gaussian(&mut self) -> f64 {
        self.0.gaussian()
    }
    fn fork_positional(&mut self) -> Self::Positional {
        panic!("native positional fork unavailable")
    }
    fn fork(&mut self) -> Self {
        panic!("native random fork unavailable")
    }
}
impl NativeAlgorithm for Feature {
    fn place(
        &self,
        w: &mut dyn NativeWorld,
        r: &mut dyn NativeRandom,
        p: [i32; 3],
    ) -> Result<bool, PlacementQueryError> {
        let generation = w.context().generation();
        let feature = w.context().top_feature();
        let mut world = WorldAdapter(w, generation, feature);
        let mut random = RandomAdapter(r);
        let blocks = FeatureBlocks {
            air: self.air(),
            stone: self.states["stone"],
            cobblestone: self.states["cobblestone"],
            end_stone: self.states["end_stone"],
            obsidian: self.states["obsidian"],
        };
        let run = Run {
            blocks,
            depth: Cell::new(0),
            remaining: Cell::new(65536),
        };
        self.place(&mut world, &mut random, p, &run)
            .map_err(|e| PlacementQueryError::Failed(e.to_string()))
    }
}
