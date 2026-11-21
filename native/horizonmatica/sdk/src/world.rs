//! The game as the mod reads it, through `mod_api`'s client host: the local player, the view, the
//! client world's blocks and the inventory. Everything the rest of the mod knows of the world
//! comes through here.

use horizonmatica_core::BlockState;
use mod_api::{ClientHost, EntityRef, HostBlockStates, LevelRef, PlayerRef};
use std::collections::HashMap;

/// A game block state as the API names it (process-local; never saved).
pub type StateId = mod_api::BlockState;

/// One hook call's view of the world.
pub struct World<'a> {
    pub host: &'a dyn ClientHost,
    pub player: Option<PlayerRef>,
    pub level: Option<LevelRef>,
}

impl<'a> World<'a> {
    pub fn new(host: &'a dyn ClientHost) -> Self {
        let player = host.local_player();
        let level = player.and_then(|p| p.level(host));
        Self { host, player, level }
    }

    /// In a world with a player.
    pub fn in_world(&self) -> bool {
        self.player.is_some() && self.level.is_some()
    }

    fn entity(&self) -> Option<EntityRef> {
        self.player.map(EntityRef::from)
    }

    pub fn position(&self) -> Option<[f64; 3]> {
        self.entity()?.position(self.host)
    }

    /// The block the player stands in.
    pub fn feet(&self) -> Option<[i32; 3]> {
        let p = self.position()?;
        Some(p.map(|v| v.floor() as i32))
    }

    pub fn eye(&self) -> Option<[f64; 3]> {
        self.entity()?.eye_position(self.host)
    }

    /// Yaw and pitch in degrees.
    pub fn look(&self) -> Option<(f32, f32)> {
        let [pitch, yaw] = self.entity()?.rotation(self.host)?;
        Some((yaw, pitch))
    }

    /// The eye and the unit direction the player looks along.
    pub fn aim(&self) -> Option<([f64; 3], [f64; 3])> {
        let eye = self.eye()?;
        let dir = self.entity()?.view_vector(self.host)?;
        Some((eye, dir))
    }

    /// The first block along the view within `distance` (at most 1024): the block and the point hit.
    pub fn ray(&self, distance: f64) -> Option<([i32; 3], [f64; 3])> {
        let hit = self.host.raycast(distance.min(1024.0) as f32)?;
        Some(([hit.block_x, hit.block_y, hit.block_z], [hit.x, hit.y, hit.z]))
    }

    /// The game state at `w`; None where the client holds no chunk (or outside the world).
    pub fn block(&self, w: [i32; 3]) -> Option<StateId> {
        self.level?.block_state(self.host, w)
    }

    pub fn loaded(&self, w: [i32; 3]) -> bool {
        self.block(w).is_some()
    }

    /// The name of the game's dimension (`minecraft:overworld`).
    pub fn dimension(&self) -> Option<String> {
        self.host.camera().map(|c| c.dimension)
    }

    /// The main inventory's stacks (their items and counts), for counting what is carried.
    pub fn inventory(&self) -> Vec<(String, u64)> {
        let Some(p) = self.player else { return Vec::new() };
        let Some(stacks) = p.inventory(self.host) else { return Vec::new() };
        stacks.into_iter().map(|st| (st.item().id(self.host), u64::from(st.count()))).collect()
    }

    /// The item in the main hand.
    pub fn held(&self) -> Option<String> {
        let living = self.host.living(EntityRef::from(self.player?))?;
        let stack = living.hand_item(self.host, mod_api::Hand::Main)?;
        Some(stack.item().id(self.host))
    }
}

/// Lookups between the schematic's block states and the game's, cached for the session (the
/// API's state values are process-local, and the game's registry doesn't change in a session).
#[derive(Default)]
pub struct Blocks {
    /// Wanted state → game state (None: the game doesn't know the block).
    ids: HashMap<BlockState, Option<StateId>>,
    /// Game state → its name and properties.
    states: HashMap<StateId, BlockState>,
    /// Game state → whether a placement may go into it (air, grass, water...).
    replaceable: HashMap<StateId, bool>,
    /// Game state → its item ("" for none).
    items: HashMap<StateId, String>,
}

impl Blocks {
    /// The game's state for a schematic state; properties the game doesn't know are left out.
    pub fn resolve(&mut self, w: &World<'_>, s: &BlockState) -> Option<StateId> {
        if let Some(id) = self.ids.get(s) {
            return *id;
        }
        let id = w.host.block_state_id(&s.to_string_full()).or_else(|| {
            // Unknown properties (an older file's): the default state with those that fit.
            let mut best = w.host.block_state_id(&s.name)?;
            for (k, v) in &s.properties {
                let mut probe = self.state(w, best);
                if probe.properties.contains_key(k) {
                    probe.properties.insert(k.clone(), v.clone());
                    if let Some(id) = w.host.block_state_id(&probe.to_string_full()) {
                        best = id;
                    }
                }
            }
            Some(best)
        });
        self.ids.insert(s.clone(), id);
        id
    }

    /// The name and properties of a game state.
    pub fn state(&mut self, w: &World<'_>, id: StateId) -> BlockState {
        self.states
            .entry(id)
            .or_insert_with(|| w.host.block_state_string(id).and_then(|t| BlockState::parse(&t)).unwrap_or_else(BlockState::air))
            .clone()
    }

    /// Whether a block placed there replaces what is there (air, grass, snow layers, fluids).
    pub fn replaceable(&mut self, w: &World<'_>, id: StateId) -> bool {
        *self.replaceable.entry(id).or_insert_with(|| w.host.block_states().replaceable(id).unwrap_or(false))
    }

    /// The item that places a game state (None for air, fire and the like).
    pub fn item(&mut self, w: &World<'_>, id: StateId) -> Option<String> {
        let item = self.items.entry(id).or_insert_with(|| {
            w.host
                .block_item(id.block())
                .map(|k| k.id(w.host))
                .filter(|i| !i.is_empty() && i != "minecraft:air")
                .unwrap_or_default()
        });
        (!item.is_empty()).then(|| item.clone())
    }
}
