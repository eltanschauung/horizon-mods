//! Minecraft Misogynist Unexpansion: the game as it was, more or less.
//!
//! - The mobs in `removed_mobs.cfg` (dolphins, foxes, bees, goats and the rest of the newer ones by
//!   default) never join a world: none spawn, and those already in one are gone when they load.
//! - No new looks for old mobs: cows, pigs and chickens only temperate, wolves only the original
//!   (pale) wolf, old ones included (checked every two seconds; the mixin `original-variants` draws
//!   them so, even on a server without this mod).
//! - Kelp only in warm, lukewarm and frozen oceans, in new terrain (the mixin `kelp-biomes`);
//!   crafted from seagrass and bone meal.
//! - Animals make their classic sounds: no sound variants, and babies sound as they did (the mixin
//!   `classic-sounds`).
//! - Enchanting costs the full level it asks for: a level 30 enchantment takes 30 levels
//!   (`enchant.rs`, and the mixin `half-level-enchant` in single player).
//! - Shields block half the damage (mixin `shield-half-block`). Crossbow rockets spare their shooter
//!   and load from the inventory (mixins `crossbow-rocket-*`, `crossbow-inventory-rockets`).
//! - Powder snow is walked on, leather boots or not, and never sunk into (the mixin
//!   `powder-snow-walk`); it slows you as soul sand does (`powder-snow-slow`), and 15 seconds
//!   standing on it freezes you (`powder-snow-cold`).
//! - Babies look as they did before the baby redesign: villagers, zombies, piglins, hoglins, farm
//!   animals, wolves, cats, rabbits, horses and squid drawn with the grown model and textures at the
//!   old sizes (`babies.rs`).
//! - Tropical fish drop fish scales (turtle scutes), nine times in ten; turtle scutes and eggs are
//!   called fish scales and fish eggs, everywhere their name shows (their default item name).
//! - Fishing as before the open-water rule: treasure anywhere, a pond as an ocean; and tropical fish
//!   five in a hundred more often (7, out of cod's share).
//! - No sculk patches or sculk veins in new terrain (the mixins `no-sculk-placed` and
//!   `no-sculk-feature`); sculk is crafted instead, and a bottle o' enchanting from mob heads
//!   (`farming.rs`).
//! - Right-clicking a grown crop harvests it and replants it (`farming.rs`).
//! - Villagers are picked up by sneaking and right-clicking them, and set down again with all their
//!   trades and experience (`villagers.rs`, after Easy Villagers by henkelmax).
//! - Netherite in an anvil: a diamond item and a netherite ingot, no smithing template (`anvil.rs`).
//! - Deepslate digs only 15% slower than stone (cobbled deepslate than cobblestone, its ores than
//!   theirs), not twice as slow.
//! - No leaf litter, wildflowers or pink petals in new terrain (the same mixins); those already
//!   in a world stay. They and kelp are crafted instead (`ground.rs`).
//! - The old water: beta's textures (the content mod's), no biome tint and the old water fog, the
//!   last two switched in `old_water.cfg` (`water.rs`, after Nostalgic Tweaks' old water fog).
//! - No Dried Ghast, and no Pigstep: C418's discs in its place (`retired.rs`).
//!
//! The first recipes and the Potion of Dolphin's Grace are the content mod `misogynist_unexpansion`;
//! the sculk ones come from this native half, so that content file stays as servers know it.

mod anvil;
mod babies;
mod config;
mod enchant;
mod farming;
mod ground;
mod retired;
mod villagers;
mod water;

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use mod_api::{
    AnvilUpdateEvent, ClientHost, ClientPacketEvent, DataLoadContext, DataPackFilesEvent,
    EnchantItemEvent, EntityInteractEvent, EntityJoinLevelEvent, Flow, Hooks, Host, HostSide,
    InteractionResult, ItemStack, LevelTickPostEvent, LootDropsEvent, LootTableLoadEvent,
    PlayerBreakSpeedEvent, PrepareItemEnchantEvent, Registrar, RegistrationContext,
    RenderLivingPreEvent, RightClickBlockEvent, ServerHost, Tag, Text, serde_json::Value,
};
use mod_api::client::fog::FogType;

#[derive(Default)]
struct Mod;

static CONFIG: LazyLock<Mutex<config::Loaded>> = LazyLock::new(|| Mutex::new(config::Loaded::new()));
/// `old_water.cfg` (`water.rs`), read on this client.
static WATER: Mutex<water::Loaded> = Mutex::new(water::Loaded::new());
/// A client world is open, and whether its water colours are still to be asked for.
static IN_WORLD: AtomicBool = AtomicBool::new(false);
static RECOLOUR: AtomicBool = AtomicBool::new(false);
/// The dimension this client is in, for the sky light at the eye.
static CLIENT_DIMENSION: Mutex<String> = Mutex::new(String::new());

fn water_settings() -> water::Settings {
    WATER.lock().map(|w| w.settings).unwrap_or_default()
}

/// The names Misogyny gives vanilla items, as their default `item_name`.
const RENAMED: [(&str, &str); 2] = [("minecraft:turtle_scute", "Fish Scales"), ("minecraft:turtle_egg", "Fish Eggs")];

/// Cows, pigs and chickens that just joined a world, to set to the temperate variant next tick.
static TO_REVERT: Mutex<Vec<(u32, String)>> = Mutex::new(Vec::new());
/// Ticks between reads of the babies' looks.
static SCAN: AtomicU32 = AtomicU32::new(0);
static ROLL: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);

/// A number from 0 to 99.
fn percent() -> u64 {
    let mut x = ROLL.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    ROLL.store(x, Ordering::Relaxed);
    x % 100
}

/// The mobs that have newer looks: only the original one is kept.
fn has_variant(kind: &str) -> bool {
    original_look(kind).is_some()
}

/// The one look a mob keeps, as (key, value) pairs of its saved data: the newer variants are gone.
fn original_look(kind: &str) -> Option<&'static [(&'static str, &'static str)]> {
    match kind {
        "minecraft:cow" | "minecraft:pig" | "minecraft:chicken" => Some(&[("variant", "minecraft:temperate")]),
        "minecraft:wolf" => Some(&[("variant", "minecraft:pale"), ("sound_variant", "minecraft:classic")]),
        _ => None,
    }
}

/// A mob with a newer look set to its original one. A key it doesn't save (yet) is left alone.
fn revert_look(host: &mut dyn ServerHost, id: u32, kind: &str) {
    let Some(look) = original_look(kind) else { return };
    let Some(entity) = host.entity(id) else { return };
    let Some(tag) = entity.nbt(host) else { return };
    let mut change = mod_api::Compound::default();
    for &(key, want) in look {
        let now = match tag.get(&key.into()) {
            Some(Tag::String(v)) => String::from_utf16_lossy(&v.0),
            _ => continue,
        };
        if now != want {
            change.insert(key.into(), Tag::String(want.into()));
        }
    }
    if !change.is_empty() {
        entity.merge_nbt(host, change);
    }
}

/// How much faster deepslate digs: its hardness made 15% more than its stone counterpart's
/// (vanilla: deepslate 3, stone 1.5; cobbled deepslate 3.5, cobblestone 2; deepslate ores 4.5,
/// ores 3; polished deepslate, its bricks and tiles 3.5, stone bricks 1.5). None for anything else.
fn deepslate_speedup(block: &str) -> Option<f32> {
    let name = block.strip_prefix("minecraft:")?;
    if !name.contains("deepslate") || name.starts_with("infested") || name.starts_with("reinforced") {
        return None;
    }
    let (hardness, stone) = if name == "deepslate" {
        (3.0, 1.5)
    } else if name.ends_with("_ore") {
        (4.5, 3.0)
    } else if name.starts_with("cobbled_deepslate") {
        (3.5, 2.0)
    } else {
        (3.5, 1.5)
    };
    Some(hardness / (stone * 1.15))
}

/// How often tropical fish drop fish scales, in 100.
const SCALES_CHANCE: u64 = 90;

/// Takes out every loot condition that asks for open water (`FishingHook.isOpenWaterFishing`):
/// treasure is caught anywhere again, as before 1.16.
fn drop_open_water(v: &mut Value) -> usize {
    let open_water = |c: &Value| c.to_string().contains("in_open_water");
    let mut dropped = 0;
    match v {
        Value::Object(map) => {
            if map.get("condition").is_some_and(open_water) {
                map.remove("condition");
                dropped += 1;
            }
            if let Some(Value::Array(list)) = map.get_mut("conditions") {
                let before = list.len();
                list.retain(|c| !open_water(c));
                dropped += before - list.len();
            }
            for child in map.values_mut() {
                dropped += drop_open_water(child);
            }
        }
        Value::Array(list) => {
            for child in list {
                dropped += drop_open_water(child);
            }
        }
        _ => {}
    }
    dropped
}

/// Tropical fish five in a hundred more often, out of cod's share: 7 and 55, of 100.
fn more_tropical_fish(v: &mut Value) {
    let Some(pools) = v.get_mut("pools").and_then(Value::as_array_mut) else { return };
    for pool in pools {
        let Some(entries) = pool.get_mut("entries").and_then(Value::as_array_mut) else { continue };
        for e in entries {
            let weight = match e.get("name").and_then(Value::as_str) {
                Some("minecraft:tropical_fish") => 7,
                Some("minecraft:cod") => 55,
                _ => continue,
            };
            e["weight"] = Value::from(weight);
        }
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let egg = r.item_key(villagers::ITEM);
        r.item_part(egg, villagers::CarriedVillager);
        for id in ["minecraft:turtle_scute", "minecraft:turtle_egg"] {
            let key = r.item_key(id);
            r.item_part(key, FishNames);
        }
    }

    fn entity_interact(&self, host: HostSide<'_>, event: &mut EntityInteractEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else { return None };
        villagers::interact_entity(host, event)
    }

    /// The sculk, bottle o' enchanting and flora recipes, into the data the server loads.
    fn data_pack_files(&self, _: &mut dyn ServerHost, event: &mut DataPackFilesEvent<'_>) {
        event.files.extend(farming::recipes());
        event.files.extend(ground::recipes());
        event.files.extend(retired::files());
    }

    /// Fish scales and fish eggs by name, wherever the name shows: the hotbar, a tooltip, a
    /// dropped item's label.
    fn modify_default_components(&self, _: &dyn DataLoadContext, event: &mut mod_api::ModifyDefaultComponentsEvent<'_>) {
        if let Some((_, name)) = RENAMED.iter().find(|(id, _)| *id == event.item) {
            event.components.insert("minecraft:item_name".into(), mod_api::serde_json::json!({"text": name}));
        }
    }

    /// A grown crop right-clicked: harvested and replanted.
    fn right_click_block(&self, host: HostSide<'_>, event: &mut RightClickBlockEvent<'_>) -> Flow {
        let HostSide::Server(host) = host else { return Flow::Continue };
        if let Flow::Cancel = farming::right_click(host, event.click, &mut percent) {
            *event.cancellation_result = InteractionResult::SuccessServer;
            return Flow::Cancel;
        }
        Flow::Continue
    }

    /// A diamond item and a netherite ingot in an anvil: its netherite kind, no template needed.
    fn anvil_update(&self, host: HostSide<'_>, event: &mut AnvilUpdateEvent<'_>) -> Flow {
        let host: &dyn Host = match host {
            HostSide::Client(host) => host,
            HostSide::Server(host) => host,
        };
        let update = &mut *event.update;
        if let Some(out) = anvil::upgrade(host, &update.left, update.right.as_ref()) {
            update.output = Some(out);
            update.xp_cost = anvil::XP_COST;
            update.material_cost = 1;
        }
        Flow::Continue
    }

    /// A server player's enchanting offers, kept to charge their full level.
    fn prepare_item_enchant(&self, _: &mut dyn ServerHost, event: &mut PrepareItemEnchantEvent<'_>) {
        enchant::offered(event.player, *event.costs);
    }

    /// A server player enchanted: the full level asked, less the 1 to 3 levels already taken.
    fn enchant_item(&self, host: &mut dyn ServerHost, event: &mut EnchantItemEvent<'_>) {
        let owed = enchant::still_owed(event.player, event.levels);
        if owed > 0 && let Some(player) = host.player(event.player) {
            player.give_levels(host, -owed);
        }
    }

    /// On a client: the baby looks its server worked out.
    fn client_packet(&self, _: &mut dyn ClientHost, event: &mut ClientPacketEvent<'_>) {
        if let Some(payload) = &event.packet.custom_payload
            && payload.channel.as_str() == babies::CHANNEL
        {
            babies::received(&payload.data);
        }
    }

    fn client_level_unload(&self, _: &mut dyn ClientHost, _: &mut mod_api::ClientLevelUnloadEvent<'_>) {
        babies::clear();
        IN_WORLD.store(false, Ordering::Relaxed);
    }

    /// Apply the same mining speed on the client and server.
    fn player_break_speed(&self, host: HostSide<'_>, event: &mut PlayerBreakSpeedEvent) -> Flow {
        let host: &dyn Host = match host {
            HostSide::Client(host) => host,
            HostSide::Server(host) => host,
        };
        let block = host.entity(event.player).and_then(|player| player.level(host))
            .and_then(|level| level.block(host, event.pos));
        if let Some(faster) = block.and_then(|block| deepslate_speedup(&block.id(host))) {
            event.new_speed *= faster;
        }
        Flow::Continue
    }

    /// Fishing as it was: no open-water rule, and more tropical fish.
    fn loot_table_load(&self, _: &dyn DataLoadContext, event: &mut LootTableLoadEvent<'_>) {
        match event.id {
            "minecraft:gameplay/fishing" => {
                drop_open_water(event.table);
            }
            "minecraft:gameplay/fishing/fish" => more_tropical_fish(event.table),
            _ => {}
        }
        retired::replace_pigstep(event.table);
    }

    /// A removed mob never joins: not spawned, not loaded. A cow, pig or chicken that joins is set
    /// to its temperate variant on the next tick.
    fn entity_join_level(&self, host: &mut dyn ServerHost, event: &mut EntityJoinLevelEvent<'_>) -> Flow {
        let removes = CONFIG
            .lock()
            .map(|mut c| {
                c.refresh(host.config_files(), &host.instance_dir());
                c.config.removes(event.kind)
            })
            .unwrap_or(false);
        if removes {
            return Flow::Cancel;
        }
        if has_variant(event.kind)
            && let Ok(mut v) = TO_REVERT.lock()
        {
            v.push((event.entity, event.kind.to_string()));
        }
        Flow::Continue
    }

    fn level_tick_post(&self, host: HostSide<'_>, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else { return };
        let level = event.level;
        let n = SCAN.fetch_add(1, Ordering::Relaxed);
        if n % 10 == 0 {
            babies::scan(host, level);
        }
        if n % 40 == 5 {
            for entity in level.all_entities(host).unwrap_or_default() {
                if let Some(kind) = entity.kind(host).map(|key| key.id(host))
                    && has_variant(&kind)
                {
                    revert_look(host, entity.id(), &kind);
                }
            }
        }
        if let Ok(mut c) = CONFIG.lock() {
            c.refresh(host.config_files(), &host.instance_dir());
        }
        let ids: Vec<(u32, String)> = TO_REVERT.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default();
        for (id, kind) in ids {
            revert_look(host, id, &kind);
        }
    }

    /// Fish scales from tropical fish.
    fn loot_drops(&self, host: &mut dyn ServerHost, event: &mut LootDropsEvent<'_>) {
        if let Some((_, "minecraft:tropical_fish")) = event.context.entity
            && percent() < SCALES_CHANCE
            && let Some(key) = host.item_key(&mod_api::Identifier::parse("minecraft:turtle_scute").unwrap())
        {
            event.drops.push(ItemStack::of(key));
        }
    }

    fn render_living_pre(&self, host: &dyn ClientHost, event: &mut RenderLivingPreEvent<'_>) -> Flow {
        babies::draw(host, event)
    }

    /// One water colour everywhere (white, under the old textures' own blue), while `colour` is on.
    fn biome_colors(&self, _: &dyn ClientHost, event: &mut mod_api::BiomeColorsEvent<'_>) {
        if let Some(tint) = water_settings().water_tint(event.biome, event.colors.water) {
            event.colors.water = tint;
        }
    }

    fn client_level_load(&self, _: &mut dyn ClientHost, event: &mut mod_api::ClientLevelLoadEvent<'_>) {
        if let Ok(mut d) = CLIENT_DIMENSION.lock() {
            *d = event.dimension.to_owned();
        }
        IN_WORLD.store(true, Ordering::Relaxed);
        RECOLOUR.store(true, Ordering::Relaxed);
    }

    /// `old_water.cfg` read again; the water colours asked for once a world is open, and again
    /// when the file changed.
    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut mod_api::ClientTickPostEvent) {
        let changed = WATER.lock().map(|mut w| w.refresh(host.config_files())).unwrap_or(false);
        if changed {
            RECOLOUR.store(true, Ordering::Relaxed);
        }
        if IN_WORLD.load(Ordering::Relaxed) && RECOLOUR.swap(false, Ordering::Relaxed) {
            host.refresh_biome_colors();
        }
    }

    /// Old water fog's colour: dark blue, darker the less sky reaches the eye.
    fn compute_fog_color(&self, host: &dyn ClientHost, event: &mut mod_api::ComputeFogColorEvent<'_>) {
        if event.fog_type != FogType::Water || !water_settings().fog {
            return;
        }
        let dimension = CLIENT_DIMENSION.lock().map(|d| d.clone()).unwrap_or_default();
        let eye = event.camera.map(|c| c.floor() as i32);
        let sky = host
            .level(&dimension)
            .and_then(|level| level.brightness(host, mod_api::LightLayer::Sky, eye))
            .unwrap_or(15);
        *event.rgb = water::fog_rgb(sky);
    }

    /// Old water fog's distance: thick, gone 16 blocks out. Closer fog (blindness) stays.
    fn render_fog(&self, _: &dyn ClientHost, event: &mut mod_api::RenderFogEvent<'_>) {
        if event.fog_type != FogType::Water || !water_settings().fog || event.fog.environmental_end <= water::FOG_END {
            return;
        }
        let fog = &mut *event.fog;
        fog.environmental_start = water::FOG_START;
        fog.environmental_end = water::FOG_END;
        fog.sky_end = water::FOG_END;
        fog.cloud_end = water::FOG_END;
    }
}

struct FishNames;

impl mod_api::ItemPart for FishNames {
    fn item_subscribed(&self) -> &'static [Option<mod_api::HookId>] {
        &[Some(mod_api::HookId::Tooltip)]
    }
    fn tooltip(&self, host: &dyn ClientHost, event: &mut mod_api::TooltipEvent<'_>) {
        let name = match event.stack.item().id(host).as_str() {
            "minecraft:turtle_scute" => "Fish Scales",
            "minecraft:turtle_egg" => "Fish Eggs",
            _ => return,
        };
        if event.stack.components().and_then(|patch| patch.as_nbt().get(&"minecraft:custom_name".into())).is_none()
            && let Some(first) = event.lines.first_mut()
        {
            first.0 = Text::literal(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vanilla's tables as the game loads them.
    const FISHING: &str = r#"{"type": "minecraft:fishing", "pools": [{"entries": [{"type": "minecraft:loot_table", "quality": -2, "value": "minecraft:gameplay/fishing/junk", "weight": 10}, {"type": "minecraft:loot_table", "condition": {"type": "minecraft:entity_properties", "entity": "this", "predicate": {"minecraft:type_specific/fishing_hook": {"in_open_water": true}}}, "quality": 2, "value": "minecraft:gameplay/fishing/treasure", "weight": 5}, {"type": "minecraft:loot_table", "quality": -1, "value": "minecraft:gameplay/fishing/fish", "weight": 85}], "rolls": 1}]}"#;
    const FISH: &str = r#"{"pools": [{"entries": [{"type": "minecraft:item", "name": "minecraft:cod", "weight": 60}, {"type": "minecraft:item", "name": "minecraft:salmon", "weight": 25}, {"type": "minecraft:item", "name": "minecraft:tropical_fish", "weight": 2}, {"type": "minecraft:item", "name": "minecraft:pufferfish", "weight": 13}], "rolls": 1}]}"#;

    #[test]
    fn deepslate_digs_fifteen_percent_slower_than_stone() {
        // Effective hardness: vanilla's divided by the speed-up.
        let eff = |b: &str, h: f32| h / deepslate_speedup(b).unwrap();
        assert!((eff("minecraft:deepslate", 3.0) - 1.725).abs() < 1e-4);
        assert!((eff("minecraft:cobbled_deepslate", 3.5) - 2.3).abs() < 1e-4);
        assert!((eff("minecraft:deepslate_diamond_ore", 4.5) - 3.45).abs() < 1e-4);
        assert!((eff("minecraft:deepslate_bricks", 3.5) - 1.725).abs() < 1e-4);
        assert_eq!(deepslate_speedup("minecraft:stone"), None);
        assert_eq!(deepslate_speedup("minecraft:infested_deepslate"), None);
        assert_eq!(deepslate_speedup("minecraft:reinforced_deepslate"), None);
    }

    #[test]
    fn treasure_no_longer_needs_open_water() {
        let mut t: Value = mod_api::serde_json::from_str(FISHING).unwrap();
        assert_eq!(drop_open_water(&mut t), 1);
        assert!(!t.to_string().contains("in_open_water"));
        // The treasure entry itself stays, at its weight.
        assert!(t.to_string().contains("gameplay/fishing/treasure"));
    }

    #[test]
    fn tropical_fish_come_five_in_a_hundred_more_often() {
        let mut t: Value = mod_api::serde_json::from_str(FISH).unwrap();
        more_tropical_fish(&mut t);
        let entries = t["pools"][0]["entries"].as_array().unwrap();
        let w = |n: &str| entries.iter().find(|e| e["name"] == n).unwrap()["weight"].as_i64().unwrap();
        assert_eq!((w("minecraft:tropical_fish"), w("minecraft:cod")), (7, 55));
        assert_eq!(entries.iter().map(|e| e["weight"].as_i64().unwrap()).sum::<i64>(), 100);
    }
}
