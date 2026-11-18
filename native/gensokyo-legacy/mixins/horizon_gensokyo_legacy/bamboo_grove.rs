// The authored canopy stays at a fixed height while each stem meets the local floor.
use crate::{
    registry::BlockStateId,
    worldgen::{
        DecodeError,
        block_feature::FeatureBlockBindings,
        native_feature::{NativeAlgorithm, NativeRandom, NativeWorld},
        placement::PlacementQueryError,
        structure::{Mirror, Rotation, StructureTemplate},
        tree_blocks::{self as tb, TreeBlocks},
    },
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Debug)]
pub struct Grove {
    templates: Vec<(Arc<StructureTemplate>, i32)>,
    total: i32,
    palette: usize,
    blocks: Arc<TreeBlocks>,
    soil: Arc<BTreeSet<BlockStateId>>,
}

impl Grove {
    pub fn decode(v: Value, bindings: &FeatureBlockBindings<'_>) -> Result<Self, DecodeError> {
        let structures = bindings
            .structures
            .as_ref()
            .ok_or(DecodeError::Field("bamboo grove templates"))?;
        let entries = v["templates"]
            .as_array()
            .ok_or(DecodeError::Field("bamboo grove templates"))?;
        let mut total = 0i32;
        let mut templates = Vec::new();
        for e in entries {
            let id = e["id"]
                .as_str()
                .ok_or(DecodeError::Field("bamboo grove template id"))?;
            let weight = e["weight"]
                .as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .filter(|&n| n > 0)
                .ok_or(DecodeError::Field("bamboo grove weight"))?;
            total = total
                .checked_add(weight)
                .ok_or(DecodeError::Field("bamboo grove total weight"))?;
            let template = structures
                .template(id)
                .filter(|t| t.palettes().len() == 2 && t.size[0] <= 24 && t.size[2] <= 24)
                .ok_or_else(|| {
                    DecodeError::Registry(format!("invalid bamboo grove template {id}"))
                })?;
            templates.push((template, total));
        }
        if total == 0 {
            return Err(DecodeError::Field("empty bamboo grove templates"));
        }
        let registry = bindings.registry();
        let palette = usize::from(
            registry.default_state("lunatic:bamboo_leaves").is_none()
                || registry.default_state("lunatic:bushy_bamboo").is_none(),
        );
        let soil = bindings
            .tags
            .get("minecraft:dirt")
            .cloned()
            .ok_or(DecodeError::Field("bamboo grove soil"))?;
        Ok(Self {
            templates,
            total,
            palette,
            blocks: bindings.tree_blocks()?,
            soil,
        })
    }

    fn clear(&self, world: &mut dyn NativeWorld, p: [i32; 3]) -> Result<bool, PlacementQueryError> {
        if !world.context().ensure_can_write(p)? || world.context().outside_build_height(p[1])? {
            return Ok(false);
        }
        let old = world.block(p)?;
        Ok(!self.blocks.has(old, tb::ANY_FLUID)
            && self.blocks.has(old, tb::AIR | tb::LEAVES | tb::REPLACEABLE))
    }
}

impl NativeAlgorithm for Grove {
    fn place(
        &self,
        world: &mut dyn NativeWorld,
        random: &mut dyn NativeRandom,
        origin: [i32; 3],
    ) -> Result<bool, PlacementQueryError> {
        let pick = random.int(self.total)?;
        let template = &self.templates[self.templates.partition_point(|(_, end)| *end <= pick)].0;
        let rotation = Rotation::ALL[random.int(4)? as usize];
        let transform = |[x, y, z]: [i32; 3]| {
            let p = [x - template.size[0] / 2, y, z - template.size[2] / 2];
            let rotated = StructureTemplate::transform(p, Mirror::None, rotation, [0; 3]);
            std::array::from_fn(|i| origin[i].wrapping_add(rotated[i]))
        };
        let blocks = template.palettes()[self.palette].blocks();
        let mut floors = BTreeMap::new();
        let mut writes = Vec::with_capacity(blocks.len());
        for stem in blocks.iter().filter(|block| block.pos[1] == 0) {
            let [x, y, z] = transform(stem.pos);
            let mut floor = None;
            for ground in (y - 5..=y + 3).rev() {
                let pos = [x, ground, z];
                if !world.context().ensure_can_write(pos)?
                    || world.context().outside_build_height(ground)?
                {
                    return Ok(false);
                }
                let old = world.block(pos)?;
                if self.soil.contains(&old) {
                    floor = Some(ground);
                    break;
                }
                if self.blocks.has(old, tb::ANY_FLUID | tb::SOLID)
                    && !self.blocks.has(old, tb::LEAVES)
                {
                    return Ok(false);
                }
            }
            let Some(ground) = floor else {
                return Ok(false);
            };
            floors.insert((x, z), ground);
            for y in ground + 1..y {
                let p = [x, y, z];
                if !self.clear(world, p)? {
                    return Ok(false);
                }
                writes.push((p, stem.state));
            }
        }
        for block in blocks {
            let p = transform(block.pos);
            if floors
                .get(&(p[0], p[2]))
                .is_some_and(|&floor| p[1] <= floor)
            {
                continue;
            }
            if !self.clear(world, p)? {
                return Ok(false);
            }
            writes.push((p, block.state));
        }
        for (p, state) in writes {
            world.set(p, state, 2)?;
        }
        Ok(true)
    }
}
