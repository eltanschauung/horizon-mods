use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use horizon_client::game::Game;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::{NativeLootDrops, NativeScreenView};
use horizon_world::nbt::{Compound, JavaString, Tag};

pub const MOD_ID: &str = "gravestone_core";
pub const VERSION: &str = "1.0.1";
pub const AUTHOR: &str = "wareal-nah";
pub const LICENSE: &str = "MIT";
pub const DESCRIPTION: &str = "Gravestone mechanics and death recovery for Horizon";
pub const GRAVESTONE_BLOCK: &str = "gravestone:gravestone";
pub const OBITUARY_ITEM: &str = "gravestone:obituary";
pub const MIN_BUILD_HEIGHT: i32 = -64;
pub const MAX_BUILD_HEIGHT: i32 = 320;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StoredItem {
    pub slot_tag: String,
    pub count: u32,
    pub item: String,
    pub damage: u32,
    pub nbt: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GraveRecord {
    pub death_id: String,
    pub player_name: String,
    pub player_uuid: u128,
    pub dimension: String,
    pub pos: [i32; 3],
    pub timestamp: i64,
    pub obituary_given: bool,
    pub items: Vec<StoredItem>,
}

#[derive(Clone, Debug)]
pub struct ObituaryData {
    pub death_id: String,
    pub player_name: String,
    pub dimension: String,
    pub pos: [i32; 3],
    pub timestamp: i64,
    pub items: Vec<StoredItem>,
}

pub struct GuiState {
    pub active_tab: usize, // 0: Info/Summary, 1: Items
    pub items_page: usize,
}

type Graves = HashMap<(String, [i32; 3]), GraveRecord>;
static GRAVESTONES: Mutex<Option<Graves>> = Mutex::new(None);
static PENDING_OBITUARIES: Mutex<Option<HashMap<u128, GraveRecord>>> = Mutex::new(None);
static PENDING_DEATH_SLOTS: Mutex<Option<(u128, Vec<(String, ItemStack)>)>> = Mutex::new(None);
static OPEN_OBITUARY: Mutex<Option<ObituaryData>> = Mutex::new(None);
static GUI_STATE: Mutex<GuiState> = Mutex::new(GuiState {
    active_tab: 0,
    items_page: 0,
});
static CLOSE_REQUESTED: AtomicBool = AtomicBool::new(false);
static OPEN_SCREEN_REQUESTED: AtomicBool = AtomicBool::new(false);
static WORLD_FILE: Mutex<Option<PathBuf>> = Mutex::new(None);

fn is_ru(game: &Game<'_>) -> bool {
    let code = horizon_client::lang::code();
    if !code.is_empty() {
        if code.to_lowercase().starts_with("ru") {
            return true;
        }
        if code.to_lowercase().starts_with("en") {
            return false;
        }
    }
    let settings_lang = &game.settings.language;
    if !settings_lang.is_empty() {
        if settings_lang.to_lowercase().starts_with("ru") {
            return true;
        }
        if settings_lang.to_lowercase().starts_with("en") {
            return false;
        }
    }
    let opt_path = game.native_instance_dir().join("options.txt");
    if let Ok(content) = std::fs::read_to_string(&opt_path) {
        for line in content.lines() {
            if let Some((k, v)) = line.split_once(':') {
                if k.trim() == "lang" {
                    return v.trim().to_lowercase().starts_with("ru");
                }
            }
        }
    }
    false
}

fn format_dim_locative(dim: &str, is_ru: bool) -> String {
    if is_ru {
        match dim {
            "minecraft:overworld" | "overworld" => "Верхнем мире".to_string(),
            "minecraft:the_nether" | "the_nether" | "nether" => "Нижнем мире".to_string(),
            "minecraft:the_end" | "the_end" | "end" => "Крае".to_string(),
            other => other.to_string(),
        }
    } else {
        match dim {
            "minecraft:overworld" | "overworld" => "the Overworld".to_string(),
            "minecraft:the_nether" | "the_nether" | "nether" => "the Nether".to_string(),
            "minecraft:the_end" | "the_end" | "end" => "the End".to_string(),
            other => other.to_string(),
        }
    }
}

fn format_dim_nominative(dim: &str, is_ru: bool) -> String {
    if is_ru {
        match dim {
            "minecraft:overworld" | "overworld" => "Верхний мир".to_string(),
            "minecraft:the_nether" | "the_nether" | "nether" => "Нижний мир".to_string(),
            "minecraft:the_end" | "the_end" | "end" => "Край".to_string(),
            other => other.to_string(),
        }
    } else {
        match dim {
            "minecraft:overworld" | "overworld" => "Overworld".to_string(),
            "minecraft:the_nether" | "the_nether" | "nether" => "The Nether".to_string(),
            "minecraft:the_end" | "the_end" | "end" => "The End".to_string(),
            other => other.to_string(),
        }
    }
}

fn format_time(timestamp: i64) -> String {
    let secs = timestamp;
    if secs <= 0 {
        return "Unknown".to_string();
    }
    let days = secs / 86400;
    let rem = secs % 86400;
    let hours = rem / 3600;
    let rem = rem % 3600;
    let mins = rem / 60;
    let s = rem % 60;

    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1024 + doe / 1461 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        y, m, d, hours, mins, s
    )
}

fn compound_get<'a>(c: &'a Compound, key: &str) -> Option<&'a Tag> {
    c.get(&JavaString::from(key))
}

fn tag_string(tag: Option<&Tag>) -> Option<String> {
    match tag {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn is_replaceable(block: &str) -> bool {
    block == "minecraft:air"
        || block == "minecraft:cave_air"
        || block == "minecraft:water"
        || block == "minecraft:lava"
        || block == "minecraft:short_grass"
        || block == "minecraft:tall_grass"
        || block == "minecraft:fern"
        || block == "minecraft:large_fern"
        || block == "minecraft:snow"
        || block == "minecraft:vine"
        || block == "minecraft:glow_lichen"
        || block == "minecraft:seagrass"
        || block == "minecraft:tall_seagrass"
}

fn yaw_to_grave_facing(yaw: f32) -> &'static str {
    let normalized = (yaw % 360.0 + 360.0) % 360.0;
    if (45.0..135.0).contains(&normalized) {
        "west"
    } else if (135.0..225.0).contains(&normalized) {
        "north"
    } else if (225.0..315.0).contains(&normalized) {
        "east"
    } else {
        "south"
    }
}

fn grave_file(game: &Game<'_>) -> Option<PathBuf> {
    Some(
        game.native_world_dir()?
            .join("gravestone")
            .join("graves.json"),
    )
}

fn save_graves_to_disk(game: &Game<'_>, graves: &Graves) -> bool {
    let Some(path) = grave_file(game) else {
        return false;
    };
    let records: Vec<_> = graves.values().collect();
    let result = (|| -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec(&records)?;
        crate::save::write_durable(&path, &bytes)?;
        Ok(())
    })();
    if let Err(error) = result {
        log::error!("Gravestone could not save {}: {error}", path.display());
        return false;
    }
    true
}

fn load_from_disk(game: &Game<'_>) -> bool {
    if game.remote.is_some() || !game.native_on(MOD_ID) {
        return false;
    }
    let Some(path) = grave_file(game) else {
        return false;
    };
    let mut loaded = WORLD_FILE.lock().unwrap();
    if loaded.as_ref() == Some(&path) {
        return true;
    }
    let records = match std::fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<Vec<GraveRecord>>(&bytes) {
            Ok(records) => records,
            Err(error) => {
                log::error!("Gravestone could not read {}: {error}", path.display());
                return false;
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            log::error!("Gravestone could not read {}: {error}", path.display());
            return false;
        }
    };
    let mut graves = GRAVESTONES.lock().unwrap();
    let mut pending = PENDING_OBITUARIES.lock().unwrap();
    *pending = Some(
        records
            .iter()
            .filter(|g| !g.obituary_given)
            .map(|g| (g.player_uuid, g.clone()))
            .collect(),
    );
    *graves = Some(
        records
            .into_iter()
            .map(|g| ((g.dimension.clone(), g.pos), g))
            .collect(),
    );
    *loaded = Some(path);
    true
}

pub fn publish_waypoints(game: &mut Game<'_>, dimension: &str) {
    if game.world.game_time % 20 != 0 || dimension.len() > u8::MAX as usize || !load_from_disk(game)
    {
        return;
    }
    let players = game.native_server_players();
    let messages = {
        let graves = GRAVESTONES.lock().unwrap();
        let Some(graves) = graves.as_ref() else {
            return;
        };
        let mut by_owner = HashMap::<u128, Vec<&GraveRecord>>::new();
        for grave in graves
            .values()
            .filter(|g| g.dimension == dimension && !g.items.is_empty())
        {
            by_owner.entry(grave.player_uuid).or_default().push(grave);
        }
        players
            .into_iter()
            .filter(|p| p.dimension == dimension)
            .map(|p| {
                let mut records = by_owner.remove(&p.uuid).unwrap_or_default();
                records.sort_unstable_by_key(|g| std::cmp::Reverse((g.timestamp, g.pos)));
                let mut bytes = vec![1, dimension.len() as u8];
                bytes.extend_from_slice(dimension.as_bytes());
                for grave in records.into_iter().take(64) {
                    let [x, y, z] = grave.pos.map(f64::from);
                    for v in [x + 0.5, y, z + 0.5] {
                        bytes.extend_from_slice(&v.to_be_bytes());
                    }
                }
                (p.uuid, bytes)
            })
            .collect::<Vec<_>>()
    };
    for (player, bytes) in messages {
        let _ = game.native_net_send_to_player(player, "gravestone_core:waypoints", &bytes);
    }
}

fn create_obituary_stack(grave: &GraveRecord, is_ru: bool) -> ItemStack {
    let mut stack = ItemStack::new(OBITUARY_ITEM, 1);
    let mut custom_data = Compound::default();

    custom_data.insert(
        JavaString::from("DeathId"),
        Tag::String(JavaString::from(grave.death_id.as_str())),
    );
    custom_data.insert(
        JavaString::from("PlayerName"),
        Tag::String(JavaString::from(grave.player_name.as_str())),
    );
    custom_data.insert(
        JavaString::from("PlayerUuid"),
        Tag::String(JavaString::from(grave.player_uuid.to_string().as_str())),
    );
    custom_data.insert(
        JavaString::from("Dimension"),
        Tag::String(JavaString::from(grave.dimension.as_str())),
    );
    custom_data.insert(JavaString::from("X"), Tag::Int(grave.pos[0]));
    custom_data.insert(JavaString::from("Y"), Tag::Int(grave.pos[1]));
    custom_data.insert(JavaString::from("Z"), Tag::Int(grave.pos[2]));
    custom_data.insert(JavaString::from("Timestamp"), Tag::Long(grave.timestamp));

    let items_ser = grave
        .items
        .iter()
        .map(|i| format!("{}|{}|{}", i.slot_tag, i.count, i.item))
        .collect::<Vec<_>>()
        .join(";");
    custom_data.insert(
        JavaString::from("Items"),
        Tag::String(JavaString::from(items_ser.as_str())),
    );

    let mut stack_data = horizon_client::item_data::StackData::default();
    stack_data.set_other("minecraft:custom_data", Some(Tag::Compound(custom_data)));

    let custom_name_str = if is_ru {
        format!("§6§lНекролог: {}", grave.player_name)
    } else {
        format!("§6§lObituary: {}", grave.player_name)
    };
    stack_data.set_other(
        "minecraft:custom_name",
        Some(Tag::String(JavaString::from(custom_name_str.as_str()))),
    );

    let date_str = format_time(grave.timestamp);
    let dim_display = format_dim_nominative(&grave.dimension, is_ru);
    let mut lore_list = Vec::new();

    if is_ru {
        lore_list.push(Tag::String(JavaString::from(
            format!("§7Погиб: {}", date_str).as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            format!("§2Мир: {}", dim_display).as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            format!(
                "§eКоординаты: {}, {}, {}",
                grave.pos[0], grave.pos[1], grave.pos[2]
            )
            .as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            format!("§bПредметов в могиле: {}", grave.items.len()).as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            "§3§o[Нажмите ПКМ для чтения]",
        )));
    } else {
        lore_list.push(Tag::String(JavaString::from(
            format!("§7Died: {}", date_str).as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            format!("§2Dimension: {}", dim_display).as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            format!(
                "§eCoordinates: {}, {}, {}",
                grave.pos[0], grave.pos[1], grave.pos[2]
            )
            .as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            format!("§bItems in grave: {}", grave.items.len()).as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from("§3§o[Right-click to read]")));
    }
    stack_data.set_other("minecraft:lore", Some(Tag::List(lore_list)));

    stack.extra = Some(Box::new(stack_data));
    stack
}

fn parse_obituary_stack(stack: &ItemStack) -> Option<ObituaryData> {
    let cd = stack.custom_data()?;

    let death_id = tag_string(compound_get(cd, "DeathId")).unwrap_or_default();
    let player_name =
        tag_string(compound_get(cd, "PlayerName")).unwrap_or_else(|| "Игрок".to_string());
    let dimension = tag_string(compound_get(cd, "Dimension"))
        .unwrap_or_else(|| "minecraft:overworld".to_string());
    let x = match compound_get(cd, "X") {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    };
    let y = match compound_get(cd, "Y") {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    };
    let z = match compound_get(cd, "Z") {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    };
    let timestamp = match compound_get(cd, "Timestamp") {
        Some(Tag::Long(v)) => *v,
        Some(Tag::Int(v)) => *v as i64,
        _ => 0,
    };

    let mut items = Vec::new();
    if let Some(items_str) = tag_string(compound_get(cd, "Items")) {
        for entry in items_str.split(';') {
            if entry.is_empty() {
                continue;
            }
            if entry.contains('|') {
                let parts: Vec<&str> = entry.split('|').collect();
                if parts.len() >= 3 {
                    let slot_tag = parts[0].to_string();
                    let count = parts[1].parse::<u32>().unwrap_or(1);
                    let item = parts[2].to_string();
                    items.push(StoredItem {
                        slot_tag,
                        count,
                        item,
                        damage: 0,
                        nbt: None,
                    });
                }
            } else {
                let parts: Vec<&str> = entry.split(':').collect();
                if parts.len() >= 3 {
                    let raw_slot = parts[0].parse::<usize>().unwrap_or(0);
                    let slot_tag = match raw_slot {
                        39 => "armor:head".to_string(),
                        38 => "armor:chest".to_string(),
                        37 => "armor:legs".to_string(),
                        36 => "armor:feet".to_string(),
                        40 => "offhand:0".to_string(),
                        s => format!("main:{}", s),
                    };
                    let count = parts[1].parse::<u32>().unwrap_or(1);
                    let item = parts[2..].join(":");
                    items.push(StoredItem {
                        slot_tag,
                        count,
                        item,
                        damage: 0,
                        nbt: None,
                    });
                }
            }
        }
    }

    Some(ObituaryData {
        death_id,
        player_name,
        dimension,
        pos: [x, y, z],
        timestamp,
        items,
    })
}

fn refresh_obituary_stack(slot: &mut Option<ItemStack>, ru: bool) {
    let Some(stack) = slot else {
        return;
    };
    let item_id = &*stack.item;
    if item_id != OBITUARY_ITEM && !item_id.ends_with(":obituary") {
        return;
    }
    let Some(data) = parse_obituary_stack(stack) else {
        return;
    };

    let custom_name_str = if ru {
        format!("§6§lНекролог: {}", data.player_name)
    } else {
        format!("§6§lObituary: {}", data.player_name)
    };

    let date_str = format_time(data.timestamp);
    let dim_display = format_dim_nominative(&data.dimension, ru);
    let mut lore_list = Vec::new();

    if ru {
        lore_list.push(Tag::String(JavaString::from(
            format!("§7Погиб: {}", date_str).as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            format!("§2Мир: {}", dim_display).as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            format!(
                "§eКоординаты: {}, {}, {}",
                data.pos[0], data.pos[1], data.pos[2]
            )
            .as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            format!("§bПредметов в могиле: {}", data.items.len()).as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            "§3§o[Нажмите ПКМ для чтения]",
        )));
    } else {
        lore_list.push(Tag::String(JavaString::from(
            format!("§7Died: {}", date_str).as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            format!("§2Dimension: {}", dim_display).as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            format!(
                "§eCoordinates: {}, {}, {}",
                data.pos[0], data.pos[1], data.pos[2]
            )
            .as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from(
            format!("§bItems in grave: {}", data.items.len()).as_str(),
        )));
        lore_list.push(Tag::String(JavaString::from("§3§o[Right-click to read]")));
    }

    if let Some(ref mut data_box) = stack.extra {
        data_box.set_other(
            "minecraft:custom_name",
            Some(Tag::String(JavaString::from(custom_name_str.as_str()))),
        );
        data_box.set_other("minecraft:lore", Some(Tag::List(lore_list)));
    }
}

fn is_player_dead(game: &Game<'_>) -> bool {
    game.screen == crate::game::Screen::Dead
        || game
            .player
            .as_ref()
            .map_or(true, |m| m.health.health() <= 0.0)
}

pub fn check_pending_obituaries(game: &mut Game<'_>) {
    let acting = game.native_acting_id();
    if acting == 0 {
        return;
    }

    // STRICT: Never give the obituary while the player is dead or on the death screen!
    if is_player_dead(game) {
        return;
    }

    if !load_from_disk(game) {
        return;
    }

    let mut pending_guard = PENDING_OBITUARIES.lock().unwrap();
    let pending = pending_guard.get_or_insert_with(HashMap::new);

    let key = acting;

    let Some(grave) = pending.get(&key).cloned() else {
        return;
    };

    if grave.obituary_given {
        pending.remove(&key);
        return;
    }

    let ru = is_ru(game);
    let stack = create_obituary_stack(&grave, ru);
    let leftover = game.native_give_stack(acting, stack);

    let mut given = false;
    if leftover.is_none() {
        given = true;
    } else if let Some(rem) = leftover {
        if rem.count == 0 {
            given = true;
        } else if let Some(player) = game.player.as_ref() {
            // Inventory was full, drop item at player's feet safely
            let drop_pos = [
                player.position.x,
                player.position.y + 0.5,
                player.position.z,
            ];
            game.spawn_item(rem, drop_pos, [0.0, 0.1, 0.0], 10);
            given = true;
        }
    }

    if given {
        pending.remove(&key);

        let mut gravestones_guard = GRAVESTONES.lock().unwrap();
        let gravestones = gravestones_guard.get_or_insert_with(HashMap::new);
        if let Some(g) = gravestones.get_mut(&(grave.dimension.clone(), grave.pos)) {
            g.obituary_given = true;
        }
        save_graves_to_disk(game, gravestones);

        let prefix = if ru {
            "§6[Могилы]"
        } else {
            "§6[Gravestone]"
        };
        let short_notice = if ru {
            format!(
                "{prefix} §eВы возродились. В инвентарь выдан некролог с координатами гибели (ПКМ для чтения)."
            )
        } else {
            format!(
                "{prefix} §eYou respawned. An obituary with death coordinates has been added to your inventory (Right-Click to read)."
            )
        };
        game.native_tell(acting, horizon_client::text::Text::legacy(&short_notice));
    }
}

pub fn listed(mods: &mut Vec<horizon_client::mod_api::NativeMod>) {
    mods.push(horizon_client::mod_api::NativeMod {
        id: MOD_ID.to_string(),
        title: "Gravestone Mod".to_string(),
        description: DESCRIPTION.to_string(),
        keys: Vec::new(),
    });
}

pub fn block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == GRAVESTONE_BLOCK && entity.is_none() {
        let mut nbt = Compound::default();
        nbt.insert(
            JavaString::from("id"),
            Tag::String(JavaString::from("gravestone:gravestone")),
        );
        *entity = Some(("gravestone:gravestone".to_string(), nbt));
    }
}

/// Called right before `death_drops` drains `game.inventory`, `game.armor`, and `game.offhand`.
/// We capture every non-empty slot with its precise tag (`main:0..35`, `armor:head`, `armor:chest`, `armor:legs`, `armor:feet`, `offhand:0`).
pub fn drop_inventory(game: &mut Game<'_>, _at: [f64; 3], who: (u128, i32)) {
    if !game.native_on(MOD_ID) || game.ui.rules.bool("keep_inventory") {
        return;
    }
    let mut tagged = Vec::new();
    for (i, opt) in game.inventory.iter().enumerate() {
        if let Some(s) = opt {
            if s.count > 0 && &*s.item != "minecraft:air" {
                tagged.push((format!("main:{}", i), s.clone()));
            }
        }
    }
    if let Some(s) = &game.armor[0] {
        if s.count > 0 && &*s.item != "minecraft:air" {
            tagged.push(("armor:head".to_string(), s.clone()));
        }
    }
    if let Some(s) = &game.armor[1] {
        if s.count > 0 && &*s.item != "minecraft:air" {
            tagged.push(("armor:chest".to_string(), s.clone()));
        }
    }
    if let Some(s) = &game.armor[2] {
        if s.count > 0 && &*s.item != "minecraft:air" {
            tagged.push(("armor:legs".to_string(), s.clone()));
        }
    }
    if let Some(s) = &game.armor[3] {
        if s.count > 0 && &*s.item != "minecraft:air" {
            tagged.push(("armor:feet".to_string(), s.clone()));
        }
    }
    if let Some(s) = &game.offhand {
        if s.count > 0 && &*s.item != "minecraft:air" {
            tagged.push(("offhand:0".to_string(), s.clone()));
        }
    }
    *PENDING_DEATH_SLOTS.lock().unwrap() = Some((who.0, tagged));
}

pub fn loot_drops(game: &mut Game<'_>, context: &NativeLootDrops<'_>, drops: &mut Vec<ItemStack>) {
    let is_player_death = matches!(context.entity, Some((_, "minecraft:player")));
    let Some(player_uuid) = context.player else {
        return;
    };
    if !is_player_death {
        return;
    }

    if drops.is_empty() || !load_from_disk(game) {
        return;
    }

    let origin = context.origin;
    let x = origin[0].floor() as i32;
    let mut y = origin[1].floor() as i32;
    let z = origin[2].floor() as i32;

    if y <= MIN_BUILD_HEIGHT {
        y = MIN_BUILD_HEIGHT + 1;
    }

    let mut target_pos = None;
    while y < MAX_BUILD_HEIGHT {
        let candidate = [x, y, z];
        let block = game.native_block(candidate);
        if is_replaceable(&block) {
            target_pos = Some(candidate);
            break;
        }
        y += 1;
    }

    let Some(grave_pos) = target_pos else {
        return;
    };

    let below_pos = [grave_pos[0], grave_pos[1] - 1, grave_pos[2]];
    let below_block = game.native_block(below_pos);
    if is_replaceable(&below_block) {
        if let Some(dirt_state) = game.native_default_state("minecraft:dirt") {
            let _ = game.native_set_block(context.dimension, below_pos, dirt_state, 3);
        }
    }

    let Some(grave_state) = game.native_default_state(GRAVESTONE_BLOCK) else {
        return;
    };
    let placed = game.native_set_block(context.dimension, grave_pos, grave_state, 3);
    if !placed {
        return;
    }

    let facing = yaw_to_grave_facing(game.yaw);
    let _ = game.native_set_block_property(grave_pos, "facing", facing);

    let death_id = format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (origin[0].abs() as u32) ^ 0x12345678,
        (origin[1].abs() as u16) ^ 0x9ABC,
        (origin[2].abs() as u16) ^ 0xDEF0,
        (player_uuid as u16) ^ 0x4321,
        player_uuid & 0xFFFFFFFFFFFF
    );

    let player_name = game
        .native_server_players()
        .into_iter()
        .find(|p| p.uuid == player_uuid)
        .map(|p| p.name)
        .unwrap_or_else(|| "Player".into());

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // Keep slots from the snapshot, but save only stacks that actually survived the
    // death-drop rules (including Soulbound and Curse of Vanishing).
    let mut tagged = PENDING_DEATH_SLOTS
        .lock()
        .unwrap()
        .take()
        .filter(|(owner, _)| *owner == player_uuid)
        .map(|(_, slots)| slots)
        .unwrap_or_default();
    let stored_items: Vec<_> = drops
        .iter()
        .enumerate()
        .map(|(i, stack)| {
            let slot_tag = tagged
                .iter()
                .position(|(_, saved)| saved == stack)
                .map(|at| tagged.remove(at).0)
                .unwrap_or_else(|| format!("main:{}", i.min(35)));
            StoredItem {
                slot_tag,
                count: stack.count,
                item: stack.item.to_string(),
                damage: stack.damage,
                nbt: Some(horizon_client::snbt::to_snbt(&crate::save::item_to_nbt(
                    stack,
                ))),
            }
        })
        .collect();

    let record = GraveRecord {
        death_id: death_id.clone(),
        player_name: player_name.clone(),
        player_uuid,
        dimension: context.dimension.to_string(),
        pos: grave_pos,
        timestamp,
        obituary_given: false,
        items: stored_items.clone(),
    };

    {
        let mut gravestones_guard = GRAVESTONES.lock().unwrap();
        let gravestones = gravestones_guard.get_or_insert_with(HashMap::new);
        let key = (record.dimension.clone(), grave_pos);
        let previous = gravestones.insert(key.clone(), record.clone());
        if !save_graves_to_disk(game, gravestones) {
            gravestones.remove(&key);
            if let Some(previous) = previous {
                gravestones.insert(key, previous);
            }
            if let Some(state) = game.native_default_state("minecraft:air") {
                let _ = game.native_set_block(context.dimension, grave_pos, state, 3);
            }
            return;
        }
    }
    {
        let mut pending_guard = PENDING_OBITUARIES.lock().unwrap();
        let pending = pending_guard.get_or_insert_with(HashMap::new);
        pending.insert(player_uuid, record.clone());
    }

    // All items successfully saved in grave: clear world drops!
    drops.clear();
}

pub fn player_respawned(game: &mut Game<'_>, _player: u128, _name: &str) {
    check_pending_obituaries(game);
}

/// Unified function to restore all grave contents to the player:
/// Strict slot mapping:
/// - `armor:head` -> `game.armor[0]` (helmet)
/// - `armor:chest` -> `game.armor[1]` (chestplate)
/// - `armor:legs` -> `game.armor[2]` (leggings)
/// - `armor:feet` -> `game.armor[3]` (boots)
/// - `offhand:0` -> `game.offhand`
/// - `main:X` (0..35) -> `game.inventory[X]`
/// If any target slot is already occupied, places in the first free inventory slot (0..35),
/// and drops any remaining overflow safely into the world via `game.spawn_item`.
pub fn restore_grave_to_player(game: &mut Game<'_>, pos: [i32; 3]) -> bool {
    if !load_from_disk(game) {
        return false;
    }
    let mut gravestones_guard = GRAVESTONES.lock().unwrap();
    let gravestones = gravestones_guard.get_or_insert_with(HashMap::new);
    let key = (game.dim.id().to_owned(), pos);
    let Some(grave) = gravestones.remove(&key) else {
        return false;
    };
    if !save_graves_to_disk(game, gravestones) {
        gravestones.insert(key, grave);
        return false;
    }
    drop(gravestones_guard);

    let mut unplaced = Vec::new();

    for stored in grave.items {
        if stored.count == 0 {
            continue;
        }
        let stack = if let Some(nbt_str) = &stored.nbt {
            if let Ok((tag, _)) = horizon_world::nbt::parse_snbt(nbt_str) {
                crate::save::item_from_nbt(&tag).unwrap_or_else(|| {
                    let mut s = ItemStack::new(&stored.item, stored.count);
                    s.damage = stored.damage;
                    s
                })
            } else {
                let mut s = ItemStack::new(&stored.item, stored.count);
                s.damage = stored.damage;
                s
            }
        } else {
            let mut s = ItemStack::new(&stored.item, stored.count);
            s.damage = stored.damage;
            s
        };

        let tag = stored.slot_tag.as_str();
        if tag == "armor:head" {
            if game.armor[0].is_none() {
                game.armor[0] = Some(stack);
            } else {
                unplaced.push(stack);
            }
        } else if tag == "armor:chest" {
            if game.armor[1].is_none() {
                game.armor[1] = Some(stack);
            } else {
                unplaced.push(stack);
            }
        } else if tag == "armor:legs" {
            if game.armor[2].is_none() {
                game.armor[2] = Some(stack);
            } else {
                unplaced.push(stack);
            }
        } else if tag == "armor:feet" {
            if game.armor[3].is_none() {
                game.armor[3] = Some(stack);
            } else {
                unplaced.push(stack);
            }
        } else if tag == "offhand:0" || tag == "offhand" {
            if game.offhand.is_none() {
                game.offhand = Some(stack);
            } else {
                unplaced.push(stack);
            }
        } else if let Some(slot_num_str) = tag.strip_prefix("main:") {
            if let Ok(slot_idx) = slot_num_str.parse::<usize>() {
                if slot_idx < 36 && game.inventory[slot_idx].is_none() {
                    game.inventory[slot_idx] = Some(stack);
                } else {
                    unplaced.push(stack);
                }
            } else {
                unplaced.push(stack);
            }
        } else if let Ok(slot_idx) = tag.parse::<usize>() {
            // Numeric backwards compatibility
            if slot_idx == 39 && game.armor[0].is_none() {
                game.armor[0] = Some(stack);
            } else if slot_idx == 38 && game.armor[1].is_none() {
                game.armor[1] = Some(stack);
            } else if slot_idx == 37 && game.armor[2].is_none() {
                game.armor[2] = Some(stack);
            } else if slot_idx == 36 && game.armor[3].is_none() {
                game.armor[3] = Some(stack);
            } else if slot_idx == 40 && game.offhand.is_none() {
                game.offhand = Some(stack);
            } else if slot_idx < 36 && game.inventory[slot_idx].is_none() {
                game.inventory[slot_idx] = Some(stack);
            } else {
                unplaced.push(stack);
            }
        } else {
            unplaced.push(stack);
        }
    }

    // Place unplaced items into first empty inventory slots
    for stack in unplaced {
        let mut free_slot = None;
        for i in 0..36 {
            if game.inventory[i].is_none() {
                free_slot = Some(i);
                break;
            }
        }
        if let Some(i) = free_slot {
            game.inventory[i] = Some(stack);
        } else {
            let drop_pos = [
                pos[0] as f64 + 0.5,
                pos[1] as f64 + 0.5,
                pos[2] as f64 + 0.5,
            ];
            game.spawn_item(stack, drop_pos, [0.0, 0.2, 0.0], 10);
        }
    }

    // Recalculate equipment & protection values
    game.apply_equipment();

    // Clear block
    let dim = grave.dimension.clone();
    if let Some(air_state) = game.native_default_state("minecraft:air") {
        let _ = game.native_set_block(&dim, pos, air_state, 3);
    }

    let ru = is_ru(game);
    let prefix = if ru {
        "§6[Могилы]"
    } else {
        "§6[Gravestone]"
    };
    let msg = if ru {
        format!(
            "{prefix} §aВещи из могилы игрока {} успешно возвращены!",
            grave.player_name
        )
    } else {
        format!(
            "{prefix} §aSuccessfully restored {}'s items!",
            grave.player_name
        )
    };
    let acting = game.native_acting_id();
    game.native_tell(acting, horizon_client::text::Text::legacy(&msg));

    true
}

pub fn destroy_block(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    let block = game.native_block(pos);
    if block == GRAVESTONE_BLOCK {
        *canceled = !restore_grave_to_player(game, pos);
    }
}

static LAST_GRAVE_CLICK: Mutex<(u128, [i32; 3])> = Mutex::new((0, [0, 0, 0]));

pub fn right_click_block(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    let block = game.native_block(pos);
    if block == GRAVESTONE_BLOCK {
        let is_sneaking = game.native_sneaking();
        if is_sneaking {
            restore_grave_to_player(game, pos);
            *canceled = true;
        } else {
            let now_millis = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let mut last_click = LAST_GRAVE_CLICK.lock().unwrap();
            let is_duplicate =
                (now_millis.saturating_sub(last_click.0) < 350) && (last_click.1 == pos);
            *last_click = (now_millis, pos);

            if !is_duplicate {
                if !load_from_disk(game) {
                    return;
                }
                let mut gravestones_guard = GRAVESTONES.lock().unwrap();
                let gravestones = gravestones_guard.get_or_insert_with(HashMap::new);
                if let Some(grave) = gravestones.get(&(game.dim.id().to_owned(), pos)) {
                    let ru = is_ru(game);
                    let prefix = if ru {
                        "§6[Могилы]"
                    } else {
                        "§6[Gravestone]"
                    };
                    let dim_name = format_dim_locative(&grave.dimension, ru);
                    let msg = if ru {
                        format!(
                            "{prefix} §eМогила игрока §f{} §e(погиб в §b{} §eна координатах §a[{}, {}, {}]§e). §7Нажмите Shift+ПКМ или разрушьте могилу, чтобы забрать вещи.",
                            grave.player_name, dim_name, grave.pos[0], grave.pos[1], grave.pos[2]
                        )
                    } else {
                        format!(
                            "{prefix} §eGrave of §f{} §e(died in §b{} §eat §a[{}, {}, {}]§e). §7Shift + Right-Click or break the grave to recover items.",
                            grave.player_name, dim_name, grave.pos[0], grave.pos[1], grave.pos[2]
                        )
                    };
                    let acting = game.native_acting_id();
                    game.native_tell(acting, horizon_client::text::Text::legacy(&msg));
                }
            }
            *canceled = true;
        }
    }
}

/// Right click on obituary item opens GUI window
pub fn use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    let item_id = &*stack.item;
    if item_id == OBITUARY_ITEM || item_id.ends_with(":obituary") {
        *claimed = true;
        let ob_data = parse_obituary_stack(stack);

        // Fallback to disk record by death_id if items list was empty
        if let Some(data) = ob_data {
            *OPEN_OBITUARY.lock().unwrap() = Some(data);
            *GUI_STATE.lock().unwrap() = GuiState {
                active_tab: 0,
                items_page: 0,
            };
            OPEN_SCREEN_REQUESTED.store(true, Ordering::SeqCst);
        }
    }
}

pub fn tick(game: &mut Game<'_>) {
    if CLOSE_REQUESTED.swap(false, Ordering::Relaxed) {
        game.native_mod_screen_close(MOD_ID);
    }
    if OPEN_SCREEN_REQUESTED.swap(false, Ordering::SeqCst) {
        let _ = game.native_mod_screen_open(MOD_ID);
    }
    let ru = is_ru(game);
    for slot in game.inventory.iter_mut() {
        refresh_obituary_stack(slot, ru);
    }
    refresh_obituary_stack(&mut game.offhand, ru);
}

/// Renders the Obituary GUI book screen when the mod screen is open
pub fn screen_render_post(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if screen.id != "mod" || game.native_mod_screen() != Some(MOD_ID) {
        return;
    }
    let obituary_guard = OPEN_OBITUARY.lock().unwrap();
    let Some(ref data) = *obituary_guard else {
        return;
    };
    let gui_state = GUI_STATE.lock().unwrap();

    let bw = batch.width;
    let bh = batch.height;

    // 1. Semi-transparent dark background overlay
    game.native_fill(batch, [0.0, 0.0, bw, bh], 0x99000000);

    // 2. Book Panel Dimensions
    let pw = 270.0;
    let ph = 215.0;
    let (ox, oy) = batch.centered_origin(pw, ph);

    // 3. Render Book Parchment Frame
    // Outer leather border
    game.native_fill(batch, [ox - 3.0, oy - 3.0, pw + 6.0, ph + 6.0], 0xFF2A1F18);
    // Gold trim
    game.native_fill(batch, [ox - 1.0, oy - 1.0, pw + 2.0, ph + 2.0], 0xFF7D6548);
    // Parchment paper
    game.native_fill(batch, [ox, oy, pw, ph], 0xFFEDE2CD);
    // Header banner
    game.native_fill(batch, [ox, oy, pw, 25.0], 0xFFDFD1B8);
    // Header divider line
    game.native_fill(batch, [ox, oy + 25.0, pw, 2.0], 0xFF8B7765);

    let ru = is_ru(game);

    // 4. Tab Buttons
    // Tab 0: Summary
    let tab0_rect = [ox + 8.0, oy + 5.0, 75.0, 16.0];
    let tab0_bg = if gui_state.active_tab == 0 {
        0xFFC6B292
    } else {
        0xFFD8C9B0
    };
    game.native_fill(batch, tab0_rect, tab0_bg);
    let tab0_label = if gui_state.active_tab == 0 {
        if ru {
            "§0§lСводка"
        } else {
            "§0§lSummary"
        }
    } else {
        if ru { "§8Сводка" } else { "§8Summary" }
    };
    game.native_text(batch, tab0_label, [ox + 18.0, oy + 8.0], 0, false);

    // Tab 1: Items
    let tab1_rect = [ox + 88.0, oy + 5.0, 85.0, 16.0];
    let tab1_bg = if gui_state.active_tab == 1 {
        0xFFC6B292
    } else {
        0xFFD8C9B0
    };
    game.native_fill(batch, tab1_rect, tab1_bg);
    let tab1_label = if gui_state.active_tab == 1 {
        if ru {
            "§0§lПредметы"
        } else {
            "§0§lItems"
        }
    } else {
        if ru {
            "§8Предметы"
        } else {
            "§8Items"
        }
    };
    game.native_text(batch, tab1_label, [ox + 98.0, oy + 8.0], 0, false);

    // Close button (X) at top right: 16x16 button with 1px border
    let close_x_rect = [ox + pw - 22.0, oy + 5.0, 16.0, 16.0];
    let mouse = screen.mouse;
    let hovered_x = mouse[0] >= close_x_rect[0]
        && mouse[0] <= close_x_rect[0] + close_x_rect[2]
        && mouse[1] >= close_x_rect[1]
        && mouse[1] <= close_x_rect[1] + close_x_rect[3];
    game.native_fill(
        batch,
        [
            close_x_rect[0] - 1.0,
            close_x_rect[1] - 1.0,
            close_x_rect[2] + 2.0,
            close_x_rect[3] + 2.0,
        ],
        0xFF8B7765,
    );
    game.native_fill(
        batch,
        close_x_rect,
        if hovered_x { 0xFFD9534F } else { 0xFFDFD1B8 },
    );
    game.native_text(
        batch,
        "§4✕",
        [close_x_rect[0] + 4.0, close_x_rect[1] + 3.0],
        0,
        false,
    );

    // Bottom Close Button: [ Закрыть (Esc) ]
    let close_btn_rect = [ox + pw / 2.0 - 45.0, oy + ph - 22.0, 90.0, 16.0];
    let hovered_close = mouse[0] >= close_btn_rect[0]
        && mouse[0] <= close_btn_rect[0] + close_btn_rect[2]
        && mouse[1] >= close_btn_rect[1]
        && mouse[1] <= close_btn_rect[1] + close_btn_rect[3];
    game.native_fill(
        batch,
        close_btn_rect,
        if hovered_close {
            0xFFBFA98B
        } else {
            0xFFD2C1A5
        },
    );
    game.native_fill(
        batch,
        [
            close_btn_rect[0] - 1.0,
            close_btn_rect[1] - 1.0,
            close_btn_rect[2] + 2.0,
            1.0,
        ],
        0xFF8B7765,
    );
    let close_str = if ru {
        "§0Закрыть (Esc)"
    } else {
        "§0Close (Esc)"
    };
    game.native_text(
        batch,
        close_str,
        [ox + pw / 2.0 - 35.0, oy + ph - 19.0],
        0,
        false,
    );

    // 5. Render Tab Content
    if gui_state.active_tab == 0 {
        // --- SUMMARY TAB ---
        let title_hdr = if ru {
            "§6§lНекролог"
        } else {
            "§6§lObituary"
        };
        game.native_text(batch, title_hdr, [ox + 16.0, oy + 35.0], 0, false);
        game.native_fill(batch, [ox + 16.0, oy + 47.0, pw - 32.0, 1.0], 0xFF8B7765);

        let name_label = if ru { "Погибший" } else { "Player" };
        let name_str = format!("§8{}: §0§l{}", name_label, data.player_name);
        game.native_text(batch, &name_str, [ox + 16.0, oy + 54.0], 0, false);

        let time_fmt = format_time(data.timestamp);
        let time_label = if ru {
            "Время гибели"
        } else {
            "Time of death"
        };
        let time_str = format!("§8{}: §0{}", time_label, time_fmt);
        game.native_text(batch, &time_str, [ox + 16.0, oy + 70.0], 0, false);

        let dim_display = format_dim_nominative(&data.dimension, ru);
        let dim_label = if ru {
            "Измерение"
        } else {
            "Dimension"
        };
        let dim_str = format!("§8{}: §2{}", dim_label, dim_display);
        game.native_text(batch, &dim_str, [ox + 16.0, oy + 86.0], 0, false);

        let coords_label = if ru {
            "Координаты"
        } else {
            "Coordinates"
        };
        let coords_str = format!(
            "§8{}: §1X: §0{}  §1Y: §0{}  §1Z: §0{}",
            coords_label, data.pos[0], data.pos[1], data.pos[2]
        );
        game.native_text(batch, &coords_str, [ox + 16.0, oy + 102.0], 0, false);

        let total_items: u32 = data.items.iter().map(|i| i.count).sum();
        let items_hdr = if ru {
            "Всего предметов"
        } else {
            "Total items"
        };
        let slots_hdr = if ru { "слотов" } else { "slots" };
        let items_count_str = format!(
            "§8{}: §4§l{} §8({}: {})",
            items_hdr,
            total_items,
            slots_hdr,
            data.items.len()
        );
        game.native_text(batch, &items_count_str, [ox + 16.0, oy + 120.0], 0, false);

        game.native_fill(batch, [ox + 16.0, oy + 136.0, pw - 32.0, 1.0], 0xFFC6B292);

        let msg_l1 = if ru {
            "§8§o\"Ваши вещи бережно сохранены"
        } else {
            "§8§o\"Your items are safely preserved"
        };
        let msg_l2 = if ru {
            "§8§oв могиле на месте вашей гибели.\""
        } else {
            "§8§oin the grave at your death location.\""
        };
        let msg_l3 = if ru {
            "§7Shift+ПКМ или разрушение могилы вернёт вещи."
        } else {
            "§7Shift+Right-Click or break grave to recover."
        };

        game.native_text(batch, msg_l1, [ox + 16.0, oy + 146.0], 0, false);
        game.native_text(batch, msg_l2, [ox + 16.0, oy + 158.0], 0, false);
        game.native_text(batch, msg_l3, [ox + 16.0, oy + 173.0], 0, false);
    } else {
        // --- ITEMS TAB ---
        let items_hdr = if ru {
            "§0§lЭкипировка и предметы в могиле:"
        } else {
            "§0§lEquipment & items in grave:"
        };
        game.native_text(batch, items_hdr, [ox + 16.0, oy + 32.0], 0, false);

        // Armor & Offhand row (5 boxes: Helmet, Chest, Legs, Boots, Offhand)
        let slot_tags = [
            "armor:head",
            "armor:chest",
            "armor:legs",
            "armor:feet",
            "offhand:0",
        ];
        let slot_names = if ru {
            ["Шлем", "Нагр.", "Поножи", "Обувь", "2-я рука"]
        } else {
            ["Head", "Chest", "Legs", "Feet", "Offhand"]
        };

        for (idx, (st, sname)) in slot_tags.iter().zip(slot_names.iter()).enumerate() {
            let sx = ox + 16.0 + (idx as f32) * 48.0;
            let sy = oy + 46.0;

            // Box outline & fill
            game.native_fill(batch, [sx - 1.0, sy - 1.0, 20.0, 20.0], 0xFF8B7765);
            game.native_fill(batch, [sx, sy, 18.0, 18.0], 0xFFDFD1B8);

            // Find item in this slot
            if let Some(it) = data.items.iter().find(|i| i.slot_tag == *st) {
                game.native_item(batch, &it.item, [sx + 1.0, sy + 1.0], 16.0);
                if it.count > 1 {
                    let c_str = format!("{}", it.count);
                    game.native_text(batch, &c_str, [sx + 10.0, sy + 10.0], 0xFFFFFF, true);
                }
            }

            // Label below
            game.native_text(batch, sname, [sx - 1.0, sy + 21.0], 0x555555, false);
        }

        // Main Inventory List
        let main_items: Vec<&StoredItem> = data
            .items
            .iter()
            .filter(|i| !slot_tags.contains(&i.slot_tag.as_str()))
            .collect();

        let items_per_page = 4;
        let total_pages = if main_items.is_empty() {
            1
        } else {
            (main_items.len() + items_per_page - 1) / items_per_page
        };
        let page = gui_state.items_page.min(total_pages - 1);
        let start_idx = page * items_per_page;
        let page_items = &main_items[start_idx..main_items.len().min(start_idx + items_per_page)];

        let list_y_start = oy + 82.0;
        for (i, it) in page_items.iter().enumerate() {
            let row_y = list_y_start + (i as f32) * 20.0;
            // Slot box
            game.native_fill(batch, [ox + 16.0, row_y, 18.0, 18.0], 0xFFDFD1B8);
            game.native_fill(batch, [ox + 15.0, row_y - 1.0, 20.0, 20.0], 0xFF8B7765);
            game.native_fill(batch, [ox + 16.0, row_y, 18.0, 18.0], 0xFFDFD1B8);

            game.native_item(batch, &it.item, [ox + 17.0, row_y + 1.0], 16.0);

            let short_id = it.item.strip_prefix("minecraft:").unwrap_or(&it.item);
            let row_str = format!("§0{} §8(x{})", short_id, it.count);
            game.native_text(batch, &row_str, [ox + 40.0, row_y + 4.0], 0, false);

            let slot_label = format!("§7[{}]", it.slot_tag);
            game.native_text(batch, &slot_label, [ox + pw - 65.0, row_y + 4.0], 0, false);
        }

        // Pagination buttons
        if total_pages > 1 {
            let prev_btn = [ox + 16.0, oy + ph - 22.0, 20.0, 16.0];
            let next_btn = [ox + pw - 36.0, oy + ph - 22.0, 20.0, 16.0];
            game.native_fill(batch, prev_btn, 0xFFD2C1A5);
            game.native_text(batch, "§0<", [ox + 22.0, oy + ph - 19.0], 0, false);

            let page_label = if ru { "Стр." } else { "Page" };
            let page_text = format!("§8{} {} / {}", page_label, page + 1, total_pages);
            game.native_text(batch, &page_text, [ox + 45.0, oy + ph - 19.0], 0, false);

            game.native_fill(batch, next_btn, 0xFFD2C1A5);
            game.native_text(batch, "§0>", [ox + pw - 30.0, oy + ph - 19.0], 0, false);
        }
    }
}

pub fn screen_mouse_button_pressed_pre(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    _modifiers: u32,
    canceled: &mut bool,
) {
    if screen.id != "mod" || game.native_mod_screen() != Some(MOD_ID) {
        return;
    }
    // Only handle left mouse button (0)
    if button != 0 {
        return;
    }

    let pw = 270.0;
    let ph = 215.0;
    let (ox, oy) = (
        ((screen.rect[2] - pw) / 2.0).max(0.0),
        ((screen.rect[3] - ph) / 2.0).max(0.0),
    );

    let [mx, my] = screen.mouse;

    // Check click on top-right close 'X' (matches close_x_rect exactly [ox + pw - 22.0, oy + 5.0, 16.0, 16.0])
    let close_x_rect = [ox + pw - 22.0, oy + 5.0, 16.0, 16.0];
    if mx >= close_x_rect[0]
        && mx <= close_x_rect[0] + close_x_rect[2]
        && my >= close_x_rect[1]
        && my <= close_x_rect[1] + close_x_rect[3]
    {
        CLOSE_REQUESTED.store(true, Ordering::Relaxed);
        *canceled = true;
        return;
    }

    // Check click on bottom close button
    let close_btn_rect = [ox + pw / 2.0 - 45.0, oy + ph - 22.0, 90.0, 16.0];
    if mx >= close_btn_rect[0]
        && mx <= close_btn_rect[0] + close_btn_rect[2]
        && my >= close_btn_rect[1]
        && my <= close_btn_rect[1] + close_btn_rect[3]
    {
        CLOSE_REQUESTED.store(true, Ordering::Relaxed);
        *canceled = true;
        return;
    }

    let mut gui_state = GUI_STATE.lock().unwrap();

    // Check Tab 0 click: [ Сводка ]
    if mx >= ox + 8.0 && mx <= ox + 83.0 && my >= oy + 5.0 && my <= oy + 21.0 {
        gui_state.active_tab = 0;
        *canceled = true;
        return;
    }

    // Check Tab 1 click: [ Предметы ]
    if mx >= ox + 88.0 && mx <= ox + 173.0 && my >= oy + 5.0 && my <= oy + 21.0 {
        gui_state.active_tab = 1;
        *canceled = true;
        return;
    }

    // Check Pagination in Tab 1
    if gui_state.active_tab == 1 {
        // Prev button '<'
        if mx >= ox + 16.0 && mx <= ox + 36.0 && my >= oy + ph - 22.0 && my <= oy + ph - 6.0 {
            if gui_state.items_page > 0 {
                gui_state.items_page -= 1;
            }
            *canceled = true;
            return;
        }
        // Next button '>'
        if mx >= ox + pw - 36.0
            && mx <= ox + pw - 16.0
            && my >= oy + ph - 22.0
            && my <= oy + ph - 6.0
        {
            gui_state.items_page += 1;
            *canceled = true;
            return;
        }
    }

    // Click inside the dialog window consumes click to prevent punching blocks behind
    if mx >= ox && mx <= ox + pw && my >= oy && my <= oy + ph {
        *canceled = true;
    }
}
