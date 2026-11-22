//! Sculk you make yourself, and crops harvested with a right click.
//!
//! The recipes are pushed into the data the server loads (`data_pack_files`), not shipped in the
//! content mod: that file stays the same as the one servers such as Bantculture check.
//!
//! - Sculk: dirt and a bottle o' enchanting. Sculk veins, sensors, catalysts and shriekers from
//!   sculk (a shrieker made this way never summons a warden, as with any placed one).
//! - A bottle o' enchanting from any mob's head or skull.
//! - Right-clicking a fully grown crop harvests it and leaves it planted: wheat, carrots, potatoes,
//!   beetroots, nether wart and cocoa drop what breaking them would, less the one replanted.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use mod_api::{Flow, HostBlockStates};

/// The recipes, as (data pack key, JSON).
pub fn recipes() -> Vec<(String, Vec<u8>)> {
    let shapeless = |items: &[&str], result: &str, count: u32| {
        format!(
            r#"{{"type":"minecraft:crafting_shapeless","category":"building","ingredients":[{}],"result":{{"id":"{result}","count":{count}}}}}"#,
            items.iter().map(|i| format!("\"{i}\"")).collect::<Vec<_>>().join(",")
        )
    };
    let shaped = |rows: &[&str], key: &[(char, &str)], result: &str| {
        format!(
            r#"{{"type":"minecraft:crafting_shaped","category":"redstone","pattern":[{}],"key":{{{}}},"result":{{"id":"{result}","count":1}}}}"#,
            rows.iter().map(|r| format!("\"{r}\"")).collect::<Vec<_>>().join(","),
            key.iter().map(|(c, i)| format!("\"{c}\":\"{i}\"")).collect::<Vec<_>>().join(",")
        )
    };
    let heads = r#"["minecraft:skeleton_skull","minecraft:wither_skeleton_skull","minecraft:zombie_head","minecraft:creeper_head","minecraft:piglin_head","minecraft:dragon_head"]"#;
    let list = [
        ("sculk", shapeless(&["minecraft:dirt", "minecraft:experience_bottle"], "minecraft:sculk", 1)),
        ("sculk_vein", shapeless(&["minecraft:sculk"], "minecraft:sculk_vein", 4)),
        (
            "sculk_sensor",
            shaped(&["V V", "SRS"], &[('V', "minecraft:sculk_vein"), ('S', "minecraft:sculk"), ('R', "minecraft:redstone")], "minecraft:sculk_sensor"),
        ),
        (
            "sculk_catalyst",
            shaped(&["BXB", "SSS", "SSS"], &[('B', "minecraft:bone"), ('X', "minecraft:experience_bottle"), ('S', "minecraft:sculk")], "minecraft:sculk_catalyst"),
        ),
        (
            "sculk_shrieker",
            shaped(&["B B", "SXS", "SSS"], &[('B', "minecraft:bone"), ('X', "minecraft:experience_bottle"), ('S', "minecraft:sculk")], "minecraft:sculk_shrieker"),
        ),
        (
            "experience_bottle_from_head",
            format!(
                r#"{{"type":"minecraft:crafting_shapeless","category":"misc","ingredients":[{heads}],"result":{{"id":"minecraft:experience_bottle","count":1}}}}"#
            ),
        ),
    ];
    list.into_iter().map(|(name, json)| (format!("recipe/misogynist_unexpansion:{name}"), json.into_bytes())).collect()
}

/// A crop: its oldest age, and what it drops when broken grown, less the one replanted.
struct Crop {
    block: &'static str,
    ripe: u32,
}

const CROPS: [Crop; 6] = [
    Crop { block: "minecraft:wheat", ripe: 7 },
    Crop { block: "minecraft:carrots", ripe: 7 },
    Crop { block: "minecraft:potatoes", ripe: 7 },
    Crop { block: "minecraft:beetroots", ripe: 3 },
    Crop { block: "minecraft:nether_wart", ripe: 3 },
    Crop { block: "minecraft:cocoa", ripe: 2 },
];

/// The fortune-less loot of a grown crop, less the seed (or produce) it is replanted with.
/// `roll` gives 0..100.
fn harvest(block: &str, roll: &mut dyn FnMut() -> u64) -> Vec<(&'static str, i32)> {
    // Vanilla's bonus: three tries at 4 in 7 each.
    let mut bonus = || (0..3).filter(|_| roll() < 57).count() as i32;
    match block {
        "minecraft:wheat" => vec![("minecraft:wheat", 1), ("minecraft:wheat_seeds", bonus())],
        "minecraft:carrots" => vec![("minecraft:carrot", bonus())],
        "minecraft:potatoes" => {
            let mut v = vec![("minecraft:potato", bonus())];
            if roll() < 2 {
                v.push(("minecraft:poisonous_potato", 1));
            }
            v
        }
        "minecraft:beetroots" => vec![("minecraft:beetroot", 1), ("minecraft:beetroot_seeds", bonus())],
        "minecraft:nether_wart" => vec![("minecraft:nether_wart", 1 + (roll() % 3) as i32)],
        "minecraft:cocoa" => vec![("minecraft:cocoa_beans", 2)],
        _ => Vec::new(),
    }
}

static LAST_CLICK: LazyLock<Mutex<HashMap<u128, Instant>>> = LazyLock::new(Default::default);
const CLICK_GAP: Duration = Duration::from_millis(200);

/// A right click on a block: a grown crop is harvested and left planted, age 0.
pub fn right_click(host: &mut dyn mod_api::ServerHost, click: &mod_api::BlockClick<'_>, roll: &mut dyn FnMut() -> u64) -> Flow {
    let Some(player) = host.player(click.player) else { return Flow::Continue };
    if player.is_secondary_use_active(host) == Some(true)
        || player.game_mode(host) == Some(mod_api::GameMode::Spectator)
    {
        return Flow::Continue;
    }
    let pos = click.pos;
    let Some(level) = player.level(host) else { return Flow::Continue };
    let Some(state) = level.block_state(host, pos) else { return Flow::Continue };
    let block = state.block().id(host);
    let Some(crop) = CROPS.iter().find(|c| block == c.block) else { return Flow::Continue };
    let ripe = level.block_property(host, pos, "age").and_then(|a| a.parse::<u32>().ok()) == Some(crop.ripe);
    if !ripe {
        return Flow::Continue;
    }
    // Both hands' clicks arrive: the first harvests, the second only stops the use.
    let fresh = LAST_CLICK.lock().map_or(true, |mut c| {
        let now = Instant::now();
        let fresh = c.get(&click.player).is_none_or(|at| now.duration_since(*at) >= CLICK_GAP);
        c.insert(click.player, now);
        fresh
    });
    if !fresh {
        return Flow::Cancel;
    }
    let Ok(replanted) = state.with(&host.block_states(), "age", "0") else { return Flow::Continue };
    if !matches!(level.set_block(host, pos, replanted, mod_api::BlockUpdateFlags::NEIGHBORS | mod_api::BlockUpdateFlags::CLIENTS), Ok(true)) {
        return Flow::Continue;
    }
    let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.3, f64::from(pos[2]) + 0.5];
    for (item, count) in harvest(crop.block, roll) {
        if count <= 0 {
            continue;
        }
        let Some(stack) = mod_api::Identifier::parse(item).ok()
            .and_then(|id| host.item_key(&id))
            .and_then(|key| mod_api::ItemStack::new(key, count as u32)) else { continue };
        let _ = level.spawn_item(host, at[0], at[1], at[2], stack, 5);
    }
    let sound = if crop.block == "minecraft:nether_wart" { "minecraft:item.nether_wart.plant" } else { "minecraft:item.crop.plant" };
    // 4: the blocks sound category.
    level.play_sound(host, sound, at, mod_api::SoundSource::Blocks, 1.0, 1.0);
    Flow::Cancel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_recipe_is_json_under_this_mod() {
        for (key, json) in recipes() {
            assert!(key.starts_with("recipe/misogynist_unexpansion:"), "{key}");
            mod_api::serde_json::from_slice::<mod_api::serde_json::Value>(&json).unwrap_or_else(|e| panic!("{key}: {e}"));
        }
    }

    #[test]
    fn a_harvest_never_drops_less_than_nothing() {
        let mut n = 0u64;
        let mut roll = || {
            n = (n + 37) % 100;
            n
        };
        for c in &CROPS {
            for (_, count) in harvest(c.block, &mut roll) {
                assert!(count >= 0);
            }
        }
        assert_eq!(harvest("minecraft:cocoa", &mut roll), vec![("minecraft:cocoa_beans", 2)]);
    }
}
