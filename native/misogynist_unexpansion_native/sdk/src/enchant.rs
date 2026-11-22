//! Enchanting costs the full level it asks for: a level 30 enchantment takes 30 levels, a level 8 one
//! 8, not vanilla's 1 to 3. In single player the mixin `half-level-enchant` charges it;
//! a server's players are charged here, from the offers their table showed.

use std::collections::HashMap;
use std::sync::Mutex;

/// Each player's last offers that had anything in them: the table asks again with the enchanted
/// item (all zeros) before it says what was enchanted, so those are kept.
static OFFERS: Mutex<Option<HashMap<u128, [i32; 3]>>> = Mutex::new(None);

/// Offers shown to `player`.
pub fn offered(player: u128, costs: [i32; 3]) {
    if costs.iter().all(|&c| c <= 0) {
        return;
    }
    if let Ok(mut o) = OFFERS.lock() {
        o.get_or_insert_with(HashMap::new).insert(player, costs);
    }
}

/// Levels still to take from `player`, who was charged `charged` (the slot, 1 to 3) for an offer.
pub fn still_owed(player: u128, charged: i32) -> i32 {
    let Some(slot) = usize::try_from(charged - 1).ok().filter(|&s| s < 3) else { return 0 };
    let cost = OFFERS.lock().ok().and_then(|o| o.as_ref()?.get(&player).map(|c| c[slot])).unwrap_or(0);
    owed(cost, charged)
}

/// All of `cost`, less the `charged` levels already taken.
pub fn owed(cost: i32, charged: i32) -> i32 {
    (cost - charged).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_full_level_asked() {
        assert_eq!(owed(30, 3), 27); // 30 in all
        assert_eq!(owed(8, 1), 7); // 8 in all
        assert_eq!(owed(1, 1), 0); // never less than vanilla
        offered(7, [5, 17, 30]);
        offered(7, [0, 0, 0]); // the enchanted item, asked before the enchant is told
        assert_eq!(still_owed(7, 3), 27);
        assert_eq!(still_owed(7, 2), 15);
        assert_eq!(still_owed(9, 3), 0);
    }
}
