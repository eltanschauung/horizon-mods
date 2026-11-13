// SPDX-License-Identifier: MIT
use super::Game;
use std::{cell::RefCell, collections::HashMap};
pub(super) const ID: &str = "horizon_old_combat_mod";
thread_local! { static APPLIED: RefCell<HashMap<u128, bool>> = RefCell::new(HashMap::new()); }
impl Game<'_> {
    pub(super) fn old_combat_tick(&mut self) {
        if self.remote.is_some() { return; }
        let enabled = self.native_on(ID) && self.ui.rules.bool("old_combat");
        let players = self.native_server_players();
        APPLIED.with_borrow_mut(|cache| cache.retain(|id, _| players.iter().any(|p| &p.uuid == id)));
        let dimension = self.dim.id();
        for p in players.into_iter().filter(|p| p.dimension == dimension) {
            let changed = APPLIED.with_borrow(|cache| cache.get(&p.uuid).copied() != Some(enabled));
            if changed && self.native_set_attribute_base(p.uuid, "minecraft:attack_speed", if enabled {1024.0} else {4.0}) {
                APPLIED.with_borrow_mut(|cache| {cache.insert(p.uuid, enabled);});
            }
        }
    }
    pub(super) fn old_combat_keeps_sprint(&self) -> bool {
        // The server's normal attribute packets carry the rule to prediction and
        // hand animation too; no per-hit custom packets or cooldown patches.
        self.native_on(ID) && self.main_hand_attack_speed() >= 1000.0
    }
}
pub(super) fn stop() { APPLIED.with_borrow_mut(HashMap::clear); }
