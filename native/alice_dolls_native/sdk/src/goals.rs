//! State and rules shared by the doll interaction hooks and the engine goals.
use crate::stacks::DollStack as ItemStack;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

/// The doll mob's entity type, from the assets mod.
pub const DOLL_TYPE: &str = "alice_dolls:doll";

/// The names that make a doll mob which doll. The api crate uses these too.
pub const SHANGHAI: &str = "Shanghai Doll";
pub const HORAI: &str = "Horai Doll";
pub const DIAMOND_HORAI: &str = "Diamond Horai Doll";
pub const NETHERITE_HORAI: &str = "Netherite Horai Doll";
pub const OOEDO: &str = "Ooedo Doll";
pub const GERMANY: &str = "Germany Doll";
pub const LONDON: &str = "London Doll";
pub const RUSSIA: &str = "Russia Doll";
pub const GRAND_GUIGNOL: &str = "Grand Guignol Doll";
pub const MARISA: &str = "Marisa Doll";
pub const REIMU: &str = "Reimu Doll";
pub const AYA: &str = "Aya Doll";
pub const WAKASAGI: &str = "Wakasagi Doll";
pub const GOLIATH: &str = "Goliath Doll";
pub const NETHERITE_GOLIATH: &str = "Netherite Goliath Doll";
pub const YUUKA: &str = "Yuuka Doll";
pub const MOSCOW: &str = "Moscow Doll";
pub const YUKARI: &str = "Yukari Doll";
pub const PATCHOULI: &str = "Patchouli Doll";
pub const ALICE: &str = "Alice Doll";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Shanghai,
    Horai,
    Ooedo,
    Germany,
    London,
    Russia,
    GrandGuignol,
    Marisa,
    Reimu,
    Aya,
    Wakasagi,
    Goliath,
    NetheriteGoliath,
    Yuuka,
    Yukari,
    Patchouli,
    Alice,
}

/// Each doll's `getSpeed` next to the base doll's 0.28: Aya is quicker, Yuuka and Wakasagi slower.
pub fn speed_factor(kind: Kind) -> f64 {
    match kind {
        Kind::Aya => 0.32 / 0.28,
        Kind::Yuuka => 0.24 / 0.28,
        Kind::Wakasagi => 0.22 / 0.28,
        // The Netherite Goliath is a quarter quicker on its feet.
        Kind::NetheriteGoliath => 1.25,
        // Patchouli is slow on her feet: two thirds of a doll's pace.
        Kind::Patchouli => 2.0 / 3.0,
        // Russia and Moscow (one kind) run twice as fast to wherever they are going.
        Kind::Russia => 2.0,
        _ => 1.0,
    }
}

pub const STANDBY: u8 = 0;
pub const PATROL: u8 = 1;
pub const FOLLOW: u8 = 2;
pub const RIDE: u8 = 3;
/// The Diamond and Netherite Horai's ore hunt: off to the nearest diamonds and ancient debris.
pub const SECRET: u8 = 4;
/// Its second step: every ore, not only diamonds and ancient debris.
pub const SECRET_ALL: u8 = 5;

pub const FOLLOW_START: f64 = 3.0;
pub const FOLLOW_STOP: f64 = 1.5;
pub const TELEPORT_FROM: f64 = 12.0;
/// `searchRange` and `searchHeight`: how far a doll notices anything.
pub const SEARCH_RANGE: f64 = 16.0;
pub const SEARCH_HEIGHT: f64 = 8.0;
/// A stone sword's hit (the original's `attackStrength` was 2), and the 9.0 its squared reach is
/// compared with.
pub const ATTACK_STRENGTH: f32 = 5.0;
pub const REACH_SQR: f64 = 6.25;
/// How far a fighting doll strikes: seven blocks across.
pub const ATTACK_REACH_SQR: f64 = 49.0;
/// How often a doll looks for a target, re-paths to it, and can strike it, in ticks.
pub const SCAN_EVERY: i64 = 10;
pub const PATH_EVERY: i64 = 5;
pub const HIT_EVERY: i64 = 12;
/// How far from its owner a following doll defends.
pub const OWNER_GUARD: f64 = 12.0;
/// Ticks without seeing its target before a doll gives up.
pub const LOST_AFTER: i32 = 60;
/// A doll moves 35% faster than its type's base speed (a chicken's, 0.25).
pub const SPEED_SCALE: f64 = 1.35;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DollState {
    pub mode: u8,
    pub enabled: bool,
    /// The owner's UUID.
    pub owner: u128,
}

/// A doll's item (`alice_dolls:*_doll`): the item hooks keep these from being destroyed.
pub fn is_doll_item(item: &str) -> bool {
    item.starts_with("alice_dolls:") && item.ends_with("_doll")
}

pub static STATES: LazyLock<Mutex<HashMap<u32, DollState>>> = LazyLock::new(Default::default);
pub static MESSAGES: Mutex<Vec<(u32, u8, String)>> = Mutex::new(Vec::new());
/// Sheep a doll can still shear: nothing another mob's goal can read says so.
pub static SHEARABLE: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Default::default);
/// Sheep a doll has just sheared: a goal cannot change another mob, so the sheep does it itself.
pub static SHEARED: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// The api crate's word on a doll. A mob is only a doll once this has named it.
pub fn set_state(id: u32, mode: u8, enabled: bool, owner: u128) {
    if let Ok(mut states) = STATES.lock() {
        states.insert(id, DollState { mode, enabled, owner });
    }
}

/// What dolls have said since the last call: the doll, the chat level it needs, and the text.
pub fn take_messages() -> Vec<(u32, u8, String)> {
    MESSAGES.lock().map(|mut m| std::mem::take(&mut *m)).unwrap_or_default()
}

pub fn say(id: u32, level: u8, text: impl Into<String>) {
    if let Ok(mut messages) = MESSAGES.lock() {
        messages.push((id, level, text.into()));
    }
}

/// Each doll's inventory while the game runs. The api crate saves it and puts it here; the goals
/// that use it (Russia's torches and pickings, Yuuka's seeds) change it here and mark it changed.
pub static INVENTORIES: LazyLock<Mutex<HashMap<u32, Vec<Option<ItemStack>>>>> = LazyLock::new(Default::default);
pub static CHANGED: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Default::default);

/// The api crate's copy of doll `id`'s inventory.
/// Whether doll `id` is riding on its owner's head.
pub fn riding(id: u32) -> bool {
    state(id).is_some_and(|s| s.mode == RIDE && s.enabled)
}

pub fn put_inventory(id: u32, items: Vec<Option<ItemStack>>) {
    if let Ok(mut all) = INVENTORIES.lock() {
        all.insert(id, items);
    }
}

pub fn inventory(id: u32) -> Option<Vec<Option<ItemStack>>> {
    INVENTORIES.lock().ok()?.get(&id).cloned()
}

/// The inventories the goals changed since the last call, for the api crate to save.
pub fn take_inventory_changes() -> Vec<(u32, Vec<Option<ItemStack>>)> {
    let ids: Vec<u32> = CHANGED.lock().map(|mut c| c.drain().collect()).unwrap_or_default();
    ids.into_iter().filter_map(|id| Some((id, inventory(id)?))).collect()
}

/// Changes doll `id`'s inventory, unless its owner has it open in a chest screen.
pub fn edit_inventory<R>(id: u32, f: impl FnOnce(&mut Vec<Option<ItemStack>>) -> R) -> Option<R> {
    if chest_open() == Some(id) {
        return None;
    }
    let mut all = INVENTORIES.lock().ok()?;
    let items = all.get_mut(&id)?;
    let r = f(items);
    drop(all);
    if let Ok(mut changed) = CHANGED.lock() {
        changed.insert(id);
    }
    Some(r)
}

/// Whether doll `id` holds an item `wanted` accepts, and which.
pub fn holds(id: u32, wanted: impl Fn(&str) -> bool) -> Option<Arc<str>> {
    inventory(id)?.into_iter().flatten().find(|s| s.count > 0 && wanted(&s.item)).map(|s| Arc::from(s.item))
}

/// Takes one `item` out of doll `id`'s inventory: false when it had none.
pub fn take_one(id: u32, item: &str) -> bool {
    edit_inventory(id, |items| {
        for slot in items.iter_mut() {
            if let Some(s) = slot
                && &*s.item == item
                && s.count > 0
            {
                s.count -= 1;
                if s.count == 0 {
                    *slot = None;
                }
                return true;
            }
        }
        false
    })
    .unwrap_or(false)
}

// ---- Favour ----

/// Each doll's favour level, as the api crate works it out from its points.
pub static LEVELS: LazyLock<Mutex<HashMap<u32, u32>>> = LazyLock::new(Default::default);

pub fn set_level(id: u32, level: u32) {
    if let Ok(mut l) = LEVELS.lock() {
        if level == 0 {
            l.remove(&id);
        } else {
            l.insert(id, level);
        }
    }
}

pub fn level_of(id: u32) -> u32 {
    LEVELS.lock().ok().and_then(|l| l.get(&id).copied()).unwrap_or(0)
}

/// How many jobs done earn a doll its love for work: every 20 blocks dug or chopped, crops tended,
/// fish caught, torches placed or blows landed.
pub const JOBS_PER_LOVE: u32 = 20;

/// Each doll's jobs done towards its next love for work.
pub static JOBS: LazyLock<Mutex<HashMap<u32, u32>>> = LazyLock::new(Default::default);
/// The dolls that have just done another twenty, for the api crate to give love to.
pub static WORKED: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Doll `id` did one job.
pub fn worked(id: u32) {
    let Ok(mut jobs) = JOBS.lock() else { return };
    let n = jobs.entry(id).or_default();
    *n += 1;
    if *n >= JOBS_PER_LOVE {
        *n = 0;
        if let Ok(mut w) = WORKED.lock() {
            w.push(id);
        }
    }
}

/// The dolls that did another twenty jobs since the last call, once for each twenty.
pub fn take_worked() -> Vec<u32> {
    WORKED.lock().map(|mut w| std::mem::take(&mut *w)).unwrap_or_default()
}

/// The dolls that have just dug a quarry down to bedrock, for the api crate to give love to, and
/// whether it was a giga quarry.
pub static QUARRIED: Mutex<Vec<(u32, bool)>> = Mutex::new(Vec::new());

pub fn quarried(id: u32, giga: bool) {
    if let Ok(mut q) = QUARRIED.lock() {
        q.push((id, giga));
    }
}

/// The dolls that finished a quarry since the last call, once for each, and whether it was a giga one.
pub fn take_quarried() -> Vec<(u32, bool)> {
    QUARRIED.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
}

/// The dolls that have just dug out a prize: a diamond in a Netherite Horai's quarry, or a diamond
/// or ancient debris on the ore hunt. Once for each.
pub static PRIZES: Mutex<Vec<u32>> = Mutex::new(Vec::new());

pub fn prize(id: u32) {
    if let Ok(mut p) = PRIZES.lock() {
        p.push(id);
    }
}

/// The prizes dug since the last call, one entry for each.
pub fn take_prizes() -> Vec<u32> {
    PRIZES.lock().map(|mut p| std::mem::take(&mut *p)).unwrap_or_default()
}

pub fn is_diamond(name: &str) -> bool {
    matches!(short(name), "diamond_ore" | "deepslate_diamond_ore")
}

/// The dolls that do nothing yet (Alice): never a rescuer either.
pub static IDLE: LazyLock<Mutex<std::collections::HashSet<u32>>> = LazyLock::new(Default::default);

pub fn idle(id: u32) -> bool {
    IDLE.lock().is_ok_and(|i| i.contains(&id))
}

/// An attack at `level`: each level adds 3, then a tenth more of all of it.
pub fn levelled_attack(base: f32, level: u32) -> f32 {
    (0..level).fold(base, |d, _| (d + 3.0) * 1.1)
}

/// Health at `level`: each level adds 5, then a tenth more of all of it.
pub fn levelled_health(base: f32, level: u32) -> f32 {
    (0..level).fold(base, |h, _| (h + 5.0) * 1.1)
}

/// How much better a doll at `level` does its work: a tenth more a level, compounded.
pub fn level_boost(level: u32) -> f64 {
    1.1f64.powi(level.min(60) as i32)
}

pub fn attack(id: u32, base: f32) -> f32 {
    if base <= 0.0 { base } else { levelled_attack(base, level_of(id)) }
}

pub fn boost(id: u32) -> f64 {
    level_boost(level_of(id))
}

/// A blast's power, which sets both its damage and its reach: a tenth more a level, and half a
/// point more, up to a wither's charged blast and a bit (12).
pub fn blast(id: u32, base: f32) -> f32 {
    let level = level_of(id);
    ((base + 0.5 * level as f32) * level_boost(level) as f32).min(12.0)
}

/// `ticks`, done `boost` times as fast.
pub fn quicker(ticks: i32, boost: f64) -> i32 {
    (f64::from(ticks) / boost.max(1.0)).round() as i32
}

// ---- Eating ----

/// What a doll eats, and the health it gives back: the food's nutrition, as vanilla fills hunger.
/// Rotten flesh too, which can't make a doll sick. Food that hurts or misleads (spider eyes,
/// pufferfish, poisonous potatoes, chorus fruit, suspicious stew) a doll won't touch.
pub const FOODS: [(&str, f32); 34] = [
    ("minecraft:apple", 4.0),
    ("minecraft:baked_potato", 5.0),
    ("minecraft:beetroot", 1.0),
    ("minecraft:beetroot_soup", 6.0),
    ("minecraft:bread", 5.0),
    ("minecraft:carrot", 3.0),
    ("minecraft:cooked_beef", 8.0),
    ("minecraft:cooked_chicken", 6.0),
    ("minecraft:cooked_cod", 5.0),
    ("minecraft:cooked_mutton", 6.0),
    ("minecraft:cooked_porkchop", 8.0),
    ("minecraft:cooked_rabbit", 5.0),
    ("minecraft:cooked_salmon", 6.0),
    ("minecraft:cookie", 2.0),
    ("minecraft:dried_kelp", 1.0),
    ("minecraft:enchanted_golden_apple", 20.0),
    ("minecraft:golden_apple", 10.0),
    ("minecraft:golden_carrot", 6.0),
    ("minecraft:glow_berries", 2.0),
    ("minecraft:honey_bottle", 6.0),
    ("minecraft:melon_slice", 2.0),
    ("minecraft:mushroom_stew", 6.0),
    ("minecraft:potato", 1.0),
    ("minecraft:pumpkin_pie", 8.0),
    ("minecraft:rabbit_stew", 10.0),
    ("minecraft:sweet_berries", 2.0),
    ("minecraft:beef", 3.0),
    ("minecraft:chicken", 2.0),
    ("minecraft:porkchop", 3.0),
    ("minecraft:mutton", 2.0),
    ("minecraft:rabbit", 3.0),
    ("minecraft:cod", 2.0),
    ("minecraft:salmon", 2.0),
    ("minecraft:rotten_flesh", 4.0),
];

pub fn food_heal(item: &str) -> Option<f32> {
    FOODS.iter().find(|(f, _)| *f == item).map(|(_, h)| *h)
}

/// Dolls that ate since the last call, where, how much the food heals, and whether it earns love,
/// for the api crate to play the sound and add the love.
pub static ATE: Mutex<Vec<(u32, [f64; 3], f32, bool)>> = Mutex::new(Vec::new());

pub fn take_eaten() -> Vec<(u32, [f64; 3], f32, bool)> {
    ATE.lock().map(|mut a| std::mem::take(&mut *a)).unwrap_or_default()
}

/// Dolls their own owner has hit and that aren't whole again: food eaten to heal that earns no love.
pub static OWNER_HURT: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Default::default);

/// Doll `id` was hit by its owner.
pub fn owner_hurt(id: u32) {
    if let Ok(mut h) = OWNER_HURT.lock() {
        h.insert(id);
    }
}

pub fn hurt_by_owner(id: u32) -> bool {
    OWNER_HURT.lock().is_ok_and(|h| h.contains(&id))
}

/// The doll whose inventory its owner has open in a chest menu: while it is open its items are the
/// menu's, and nothing else may change them.
pub static CHEST_CLOSED: Mutex<Vec<(u32, Vec<Option<ItemStack>>)>> = Mutex::new(Vec::new());
pub static CHEST_OPEN: Mutex<Option<u32>> = Mutex::new(None);

pub fn set_chest_open(id: Option<u32>) {
    if let Ok(mut open) = CHEST_OPEN.lock() {
        *open = id;
    }
}

/// The doll inventory menu closed, holding `items`, for the api crate to save on its next tick.
pub fn chest_closed(id: u32, items: Vec<Option<ItemStack>>) {
    if let Ok(mut open) = CHEST_OPEN.lock()
        && *open == Some(id)
    {
        *open = None;
    }
    if let Ok(mut closed) = CHEST_CLOSED.lock() {
        closed.push((id, items));
    }
}

/// The doll inventories closed since the last call, for the api crate to save.
pub fn take_chest_closed() -> Vec<(u32, Vec<Option<ItemStack>>)> {
    CHEST_CLOSED.lock().map(|mut c| std::mem::take(&mut *c)).unwrap_or_default()
}

pub fn chest_open() -> Option<u32> {
    *CHEST_OPEN.lock().ok()?
}

/// The dropped item each Russia doll is after, found by the api crate (which can see item
/// entities; a mob's goals cannot).
pub static ITEM_TARGETS: LazyLock<Mutex<HashMap<u32, [f64; 3]>>> = LazyLock::new(Default::default);

pub fn set_item_target(id: u32, at: Option<[f64; 3]>) {
    if let Ok(mut targets) = ITEM_TARGETS.lock() {
        match at {
            Some(at) => targets.insert(id, at),
            None => targets.remove(&id),
        };
    }
}

pub fn item_target(id: u32) -> Option<[f64; 3]> {
    ITEM_TARGETS.lock().ok()?.get(&id).copied()
}

pub fn state(id: u32) -> Option<DollState> {
    STATES.lock().ok()?.get(&id).copied()
}

/// Mounts players ride that the mob world can't tell from other mobs (a Magic Brooms broom is a
/// wandering trader underneath): the api crate names them.
pub static MOUNTS: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Default::default);

pub fn spare_mount(id: u32) {
    if let Ok(mut m) = MOUNTS.lock() {
        m.insert(id);
    }
}

pub fn is_doll(id: u32) -> bool {
    state(id).is_some()
}

// ---- Owners ----

/// A doll's owner as the api crate saw them on the server this tick. On a server the mob world shows
/// each mob only the player nearest it, who need not be its owner: a doll follows, rides and quarries
/// by its owner, wherever they stand, never by whoever is closest.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OwnerSeen {
    pub position: [f64; 3],
    pub eye: [f64; 3],
    pub velocity: [f64; 3],
    pub yaw: f32,
    /// Where they look, a unit vector.
    pub look: [f64; 3],
    pub sneaking: bool,
    pub spectator: bool,
    /// [`dimension_code`] of the level they are in.
    pub dimension: u8,
}

pub static OWNERS: LazyLock<Mutex<HashMap<u128, OwnerSeen>>> = LazyLock::new(Default::default);

/// Every player on the server, by UUID, as they are this tick: the api crate's word.
pub fn set_owners(owners: Vec<(u128, OwnerSeen)>) {
    if let Ok(mut o) = OWNERS.lock() {
        o.clear();
        o.extend(owners);
    }
}

/// 0 the overworld, 1 the nether, 2 the end, 3 any other level (which a mob world can't tell apart).
pub fn dimension_code(name: &str) -> u8 {
    match name {
        "minecraft:overworld" => 0,
        "minecraft:the_nether" => 1,
        "minecraft:the_end" => 2,
        _ => 3,
    }
}

/// A player's UUID from its four ints, most significant first (`UUIDUtil.uuidFromIntArray`).
pub fn uuid_of(ints: [i32; 4]) -> u128 {
    ints.iter().fold(0u128, |acc, &i| (acc << 32) | u128::from(i as u32))
}

/// Where a player looks, from their yaw and pitch in degrees (`Entity.calculateViewVector`).
pub fn look_of(yaw: f32, pitch: f32) -> [f64; 3] {
    let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()]
}


pub static GUARDED: LazyLock<Mutex<HashMap<u32, u128>>> = LazyLock::new(Default::default);
/// Where each of those owners is, as the api crate last saw them.
pub static OWNER_AT: LazyLock<Mutex<HashMap<u128, [f64; 3]>>> = LazyLock::new(Default::default);

pub fn guard(skeleton: u32, owner: u128) {
    if let Ok(mut g) = GUARDED.lock() {
        g.insert(skeleton, owner);
    }
}

/// The owners whose skeletons are about, for the api crate to say where they are.
pub fn guarded_owners() -> Vec<u128> {
    let mut v: Vec<u128> = GUARDED.lock().map(|g| g.values().copied().collect()).unwrap_or_default();
    v.sort_unstable();
    v.dedup();
    v
}

pub fn set_owner_at(owner: u128, at: Option<[f64; 3]>) {
    if let Ok(mut o) = OWNER_AT.lock() {
        match at {
            Some(at) => o.insert(owner, at),
            None => o.remove(&owner),
        };
    }
}

/// The Goliath dolls about: hostile mobs may go for them.
pub static GOLIATHS: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Default::default);
/// The form (its name) each doll's jobs were last set for.
pub static GOALS_FORM: LazyLock<Mutex<HashMap<u32, String>>> = LazyLock::new(Default::default);

/// The mobs a Goliath struck: which Goliath, and when it last did.
pub static GOLIATH_STRUCK: LazyLock<Mutex<HashMap<u32, (u32, std::time::Instant)>>> = LazyLock::new(Default::default);

/// Goliath `goliath` struck mob `mob`.
pub fn goliath_struck(mob: u32, goliath: u32) {
    if let Ok(mut s) = GOLIATH_STRUCK.lock() {
        s.retain(|_, (_, at)| at.elapsed() < GRUDGE);
        s.insert(mob, (goliath, std::time::Instant::now()));
    }
}

/// The Goliath that struck mob `mob` in the last half minute.
pub fn goliath_foe(mob: u32) -> Option<u32> {
    GOLIATH_STRUCK.lock().ok()?.get(&mob).filter(|(_, at)| at.elapsed() < GRUDGE).map(|(g, _)| *g)
}

/// A Goliath below half its health hits twice as hard.
pub fn goliath_fury(health: f32, max: f32) -> f32 {
    if max > 0.0 && health < max * 0.5 { 2.0 } else { 1.0 }
}

/// Five minutes: how often a doll at work eats.
pub const MEAL_TICKS: u32 = 6000;

/// A player has 20 health: every doll has half as much again.
pub const DOLL_HEALTH: f32 = 30.0;
/// The Goliath doll has three times a player's, and a tenth more; the Netherite one 5% on that.
pub const GOLIATH_HEALTH: f32 = 66.0;
pub const NETHERITE_GOLIATH_HEALTH: f32 = 69.3;

pub fn max_health(kind: Kind) -> f32 {
    match kind {
        Kind::Goliath => GOLIATH_HEALTH,
        Kind::NetheriteGoliath => NETHERITE_GOLIATH_HEALTH,
        _ => DOLL_HEALTH,
    }
}

pub static SNEAKING: AtomicBool = AtomicBool::new(false);

/// Whether the player was sneaking at the last doll's tick.
pub fn player_sneaking() -> bool {
    SNEAKING.load(Ordering::Relaxed)
}

/// How fast Wakasagi swims: a dolphin's pace, a third of a block a tick.
pub const SWIM_SPEED: f64 = 0.35;

/// How many of a doll's slots, the last, hold what it wears.
pub const ARMOUR_SLOTS: usize = 4;

/// Where an armour piece is worn: 0 the head, 1 the chest, 2 the legs, 3 the feet.
pub fn armour_part(item: &str) -> Option<usize> {
    let path = item.rsplit(':').next()?;
    if path.ends_with("_helmet") || path.ends_with("_head") || path.ends_with("_skull") || path == "carved_pumpkin" {
        Some(0)
    } else if path.ends_with("_chestplate") || path == "elytra" {
        Some(1)
    } else if path.ends_with("_leggings") {
        Some(2)
    } else if path.ends_with("_boots") {
        Some(3)
    } else {
        None
    }
}

pub const SHIELD: &str = "minecraft:shield";

/// The inventory slot, of `n`, where armour part `part` (0 the helmet, 3 the boots) is worn.
pub fn armour_slot(n: usize, part: usize) -> usize {
    n - 1 - part
}

/// Whether doll `id` carries a shield: the api crate's damage hook blocks most blows with it.
pub fn has_shield(id: u32) -> bool {
    holds(id, |item| item == SHIELD).is_some()
}

// ---- Grudges: who hurt a doll or its owner, for the dolls to fight back ----

/// Who hurt a doll or its owner: a player, by UUID, or a mob, by entity id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Foe {
    Player(u128),
    Mob(u32),
}

/// How long a doll remembers who hurt it or its owner.
pub const GRUDGE: std::time::Duration = std::time::Duration::from_secs(30);

/// Who last hurt each doll, and who last hurt each player, and when: the api crate's damage hook says.
pub static DOLL_GRUDGES: LazyLock<Mutex<HashMap<u32, (Foe, std::time::Instant)>>> = LazyLock::new(Default::default);
pub static OWNER_GRUDGES: LazyLock<Mutex<HashMap<u128, (Foe, std::time::Instant)>>> = LazyLock::new(Default::default);

/// Doll `doll` was hurt by `foe` (never its own owner: the api crate leaves those out).
pub fn doll_hurt_by(doll: u32, foe: Foe) {
    if let Ok(mut g) = DOLL_GRUDGES.lock() {
        g.insert(doll, (foe, std::time::Instant::now()));
    }
}

/// The player `owner` was hurt by `foe`: their dolls go after it.
pub fn owner_hurt_by(owner: u128, foe: Foe) {
    if let Ok(mut g) = OWNER_GRUDGES.lock() {
        g.insert(owner, (foe, std::time::Instant::now()));
    }
}

/// Below this share of its health a doll is badly hurt: the dolls about fight whoever hurt it.
pub const RESCUE_SHARE: f32 = 0.3;
/// How far off a doll stands up for a badly hurt one.
pub const DEFEND_RANGE: f64 = 16.0;
/// How far off a doll picks up a badly hurt doll of its owner's, with the owner away.
pub const RESCUE_RANGE: f64 = 8.0;
/// How near its owner must be for a badly hurt doll to be left to them.
pub const OWNER_NEAR: f64 = 24.0;

/// The badly hurt dolls: who hurt each, where it was, and when.
pub static DISTRESS: LazyLock<Mutex<HashMap<u32, (Foe, [f64; 3], std::time::Instant)>>> = LazyLock::new(Default::default);
/// Badly hurt dolls to be picked up, and by which doll: the api crate does it.
pub static RESCUES: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());

/// The badly hurt dolls to pick up since the last call, and who picks each up.
pub fn take_rescues() -> Vec<(u32, u32)> {
    RESCUES.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
}

/// The dolls with a fight of their own: the rest still defend a badly hurt doll.
pub fn fights(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Shanghai | Kind::Marisa | Kind::Reimu | Kind::Aya | Kind::Goliath | Kind::NetheriteGoliath | Kind::Patchouli
    )
}

/// [`grudge`] for doll `id` owned by `owner`.
pub fn same_owner(first: u32, second: u32) -> bool {
    match (state(first), state(second)) {
        (Some(first), Some(second)) => first.owner == second.owner,
        _ => false,
    }
}

pub fn grudges_of(id: u32, owner: u128) -> Vec<Foe> {
    let fresh = |(f, at): &(Foe, std::time::Instant)| (at.elapsed() < GRUDGE).then_some(*f);
    let mut out = Vec::new();
    if let Some(f) = DOLL_GRUDGES.lock().ok().and_then(|g| g.get(&id).and_then(fresh)) {
        out.push(f);
    }
    if owner != 0
        && let Some(f) = OWNER_GRUDGES.lock().ok().and_then(|g| g.get(&owner).and_then(fresh))
    {
        out.push(f);
    }
    out.retain(|f| match *f {
        Foe::Player(p) => p != owner,
        Foe::Mob(other) => other != id && !same_owner(id, other),
    });
    out
}

/// Blows dolls struck at players since the api crate's last look: doll, player, damage.
pub static PLAYER_HITS: Mutex<Vec<(u32, u128, f32)>> = Mutex::new(Vec::new());

pub fn take_player_hits() -> Vec<(u32, u128, f32)> {
    PLAYER_HITS.lock().map(|mut h| std::mem::take(&mut *h)).unwrap_or_default()
}

pub fn hit_player(doll: u32, player: u128, amount: f32) {
    if let Ok(mut h) = PLAYER_HITS.lock() {
        h.push((doll, player, amount));
    }
}

/// A fox's health: a doll's blow takes nine tenths of it.
pub const FOX_HEALTH: f32 = 10.0;
/// How hard a doll sends a fox up: about a hundred blocks before it falls.
pub const FOX_LAUNCH: f64 = 6.0;
/// How far a doll on standby, or riding its owner's head, strikes a fox from without moving.
pub const FOX_RANGE: f64 = 35.0;

/// Foxes some doll has already sent up: no other doll goes after them in the air.
pub static LAUNCHED: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Default::default);

pub fn launched(id: u32) -> bool {
    LAUNCHED.lock().is_ok_and(|l| l.contains(&id))
}

/// The foxes the dolls can see: their blows don't hurt a doll (the api crate's damage hook asks).
pub static FOXES: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Default::default);

/// Whether the mob `id` is a fox some doll can see.
pub fn is_fox(id: u32) -> bool {
    FOXES.lock().is_ok_and(|f| f.contains(&id))
}

/// When each doll last swung, for its model: the renderer is told nothing of a mob's swing.
pub static SWINGS: LazyLock<Mutex<HashMap<u32, std::time::Instant>>> = LazyLock::new(Default::default);
/// A swing's length: six ticks.
pub const SWING_SECONDS: f32 = 0.3;

pub fn mark_swing(id: u32) {
    if let Ok(mut swings) = SWINGS.lock() {
        swings.insert(id, std::time::Instant::now());
    }
}

/// Every doll swinging now, and how far through: the api crate sends these to the other players.
pub fn swings() -> Vec<(u32, f32)> {
    let Ok(swings) = SWINGS.lock() else { return Vec::new() };
    swings
        .iter()
        .map(|(&id, start)| (id, start.elapsed().as_secs_f32() / SWING_SECONDS))
        .filter(|&(_, t)| t < 1.0)
        .collect()
}

/// How far through its swing a doll is, 0 to 1, or 0 when it is not swinging.
pub fn swing_progress(id: u32) -> f32 {
    let Ok(swings) = SWINGS.lock() else { return 0.0 };
    let Some(start) = swings.get(&id) else { return 0.0 };
    let t = start.elapsed().as_secs_f32() / SWING_SECONDS;
    if t < 1.0 { t } else { 0.0 }
}

pub const WOOL: [&str; 16] = [
    "minecraft:white_wool",
    "minecraft:orange_wool",
    "minecraft:magenta_wool",
    "minecraft:light_blue_wool",
    "minecraft:yellow_wool",
    "minecraft:lime_wool",
    "minecraft:pink_wool",
    "minecraft:gray_wool",
    "minecraft:light_gray_wool",
    "minecraft:cyan_wool",
    "minecraft:purple_wool",
    "minecraft:blue_wool",
    "minecraft:brown_wool",
    "minecraft:green_wool",
    "minecraft:red_wool",
    "minecraft:black_wool",
];

pub fn shearable(id: u32) -> bool {
    SHEARABLE.lock().is_ok_and(|s| s.contains(&id))
}

// ---- Horai: EntityDollAIReportBlock and EntityDollAIMineBlock ----

/// `mineRange`: how far, in steps along the axes, Horai looks for ore and digs.
pub const MINE_RANGE: i32 = 3;
/// `mineSpeed`: a block takes 20 * hardness / mineSpeed ticks to dig.
pub const MINE_SPEED: f32 = 2.5;
/// Obsidian's hardness: nothing harder is dug.
pub const HARDEST: f32 = 50.0;
/// Below this a dug block would open onto the void.
pub const WORLD_BOTTOM: i32 = -64;

pub fn is_air(name: &str) -> bool {
    matches!(name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// `targetBlockRegex` (`ore.+|netherquartz`): every ore, deepslate and nether ones included.
/// An ore, to report and mine: anything named one, and ancient debris.
pub fn is_ore(name: &str) -> bool {
    name.ends_with("_ore") || short(name) == "ancient_debris"
}

pub fn short(name: &str) -> &str {
    name.strip_prefix("minecraft:").unwrap_or(name)
}

/// Every offset within `MINE_RANGE` steps.
pub fn around() -> impl Iterator<Item = [i32; 3]> {
    let r = MINE_RANGE;
    (-r..=r).flat_map(move |dy| {
        (-r..=r).flat_map(move |dx| {
            (-r..=r).filter_map(move |dz| {
                (dx.abs() + dy.abs() + dz.abs() <= r).then_some([dx, dy, dz])
            })
        })
    })
}

pub fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn steps(a: [i32; 3], b: [i32; 3]) -> i32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
}

// ---- Ooedo: EntityDollAIExplosion ----

/// `explodeStrength`: a creeper's blast.
pub const EXPLODE_STRENGTH: f32 = 3.0;
/// Ticks of fuse once close: half a creeper's.
pub const FUSE: i32 = 15;

/// Ticks Ooedo rests after a blast before it looks for the next mob.
pub const BLAST_REST: i64 = 100;

// ---- Germany: EntityDollAICutTree ----

/// `cutRange` and `cutSpeed`: how far Germany looks for a tree, and how fast it chops (twice the
/// original's 2.5).
pub const CUT_RANGE: i32 = 5;
pub const CUT_SPEED: f32 = 5.0;
/// How far below and above its eyes it looks for a trunk.
pub const TRUNK_BELOW: i32 = 2;
pub const TRUNK_ABOVE: i32 = 4;

/// `logBlockRegex` (`log|mushroom`): logs, nether stems, and giant mushrooms.
pub fn is_log(name: &str) -> bool {
    name.ends_with("_log") || name.ends_with("_stem") || name.contains("mushroom")
}

/// `leavesBlockRegex` (`leaves|mushroom|vine`).
pub fn is_leafy(name: &str) -> bool {
    name.contains("leaves") || name.contains("mushroom") || name.contains("vine") || name.ends_with("_wart_block")
}

/// `lightThreshold`: a spot this dark or darker gets a torch.
pub const LIGHT_THRESHOLD: i32 = 5;

// ---- Marisa and Reimu: EntityDollAIAttackWithBullet and EntityDollAIAttackWithAmulet ----

/// Marisa's beam, or Reimu's amulets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shot {
    Beam,
    Amulet,
}

/// Marisa's beam: a hit every half second (as often as a mob can be hurt) for three times her stars'
/// damage (twice, then half as much again).
pub const BEAM_EVERY: i32 = 10;
pub const BEAM_DAMAGE: f32 = 4.5;
/// How far her beam reaches.
pub const BEAM_RANGE: f64 = 16.0;

/// Each Marisa's beam this tick: from her, to her target, and when it was last aimed.
pub static BEAMS: LazyLock<Mutex<HashMap<u32, ([f64; 3], [f64; 3], std::time::Instant)>>> = LazyLock::new(Default::default);

pub fn set_beam(id: u32, beam: Option<([f64; 3], [f64; 3])>) {
    if let Ok(mut b) = BEAMS.lock() {
        match beam {
            Some((from, to)) => b.insert(id, (from, to, std::time::Instant::now())),
            None => b.remove(&id),
        };
    }
}

/// The Marisa dolls' beams firing now, for the api crate to draw: from, to.
/// Marisa's beams now, by the doll holding each.
pub fn beams_by_doll() -> Vec<(u32, [f64; 3], [f64; 3])> {
    let Ok(b) = BEAMS.lock() else { return Vec::new() };
    b.iter().filter(|(_, (_, _, t))| t.elapsed().as_millis() < 200).map(|(&id, (a, b, _))| (id, *a, *b)).collect()
}

pub fn doll_beams() -> Vec<([f64; 3], [f64; 3])> {
    let Ok(b) = BEAMS.lock() else { return Vec::new() };
    b.values().filter(|(_, _, t)| t.elapsed().as_millis() < 200).map(|(a, b, _)| (*a, *b)).collect()
}

/// One shot in flight: it homes in on its target and hits nothing else, so a doll's danmaku never
/// hurts its owner, another doll, or anything that just gets in the way.
#[derive(Clone, Debug)]
pub struct Bullet {
    pub at: [f64; 3],
    pub target: u32,
    /// Ticks in flight; at zero or below it has not left yet.
    pub age: i32,
}

/// Ticks a shot flies before it fades.
pub const BULLET_LIFE: i32 = 80;

/// Every doll's shots in flight, by doll: which kind (0 a star, 1 an amulet, 2 Patchouli's orb), and where each is now
/// and was a tick ago. The api crate draws them each frame; the particles alone didn't show.
pub static FLYING: LazyLock<Mutex<HashMap<u32, (u8, Vec<([f64; 3], [f64; 3])>)>>> = LazyLock::new(Default::default);

/// The dolls' shots in flight, by the doll that fired them: the api crate sends each player those in
/// their level.
pub fn shots_by_doll() -> Vec<(u32, u8, [f64; 3], [f64; 3])> {
    let Ok(f) = FLYING.lock() else { return Vec::new() };
    f.iter().flat_map(|(&id, (k, v))| v.iter().map(move |(at, was)| (id, *k, *at, *was))).collect()
}

/// The dolls' shots in flight: kind (0 a star, 1 an amulet), where it is, where it was.
pub fn flying_shots() -> Vec<(u8, [f64; 3], [f64; 3])> {
    let Ok(f) = FLYING.lock() else { return Vec::new() };
    f.values().flat_map(|(k, v)| v.iter().map(move |(at, was)| (*k, *at, *was))).collect()
}

impl Shot {
    /// Blocks a tick, damage, colour and the sparkle it trails.
    pub fn look(self) -> (f64, f32, [f32; 3], &'static str) {
        match self {
            Shot::Beam => (0.0, BEAM_DAMAGE, [1.0, 0.9, 0.3], "end_rod"),
            Shot::Amulet => (0.7, 24.0, [0.9, 0.1, 0.15], "crit"),
        }
    }
}

// ---- Patchouli: a sniper's orb ----

/// Patchouli's other mode: she stands where she was put.
pub const SNIPE: u8 = 8;
/// How far Patchouli shoots, in each of her modes.
pub const SNIPE_RANGE: f64 = 64.0;
pub const SNIPE_SECRET_RANGE: f64 = 128.0;
/// Ticks between her orbs: one every four seconds, the last one landed first.
pub const SNIPE_EVERY: i32 = 80;
/// An orb's damage before her level adds to it.
pub const SNIPE_DAMAGE: f32 = 20.0;
/// Blocks an orb flies a tick, and the ticks it flies before it fades.
pub const ORB_SPEED: f64 = 2.0;
pub const ORB_LIFE: i32 = 100;
/// How many of the nearest she looks along for a clear shot, at each look round.
pub const SNIPE_LOOKS: usize = 8;

/// Whether `a` is within `range` of `b`.
pub fn snipe_reach(a: [f64; 3], b: [f64; 3], range: f64) -> bool {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>() <= range * range
}

// ---- Aya: EntityDollAIAyaShot ----

/// `shotInterval`: Aya's photos are thirty seconds apart.
pub const AYA_INTERVAL: i32 = 30 * 20;

/// How often Aya keeps a photo as a screenshot while she's out: every five minutes.
pub const AYA_FILM: i32 = 5 * 60 * 20;

// ---- Goliath: EntityDollAIAttackWithGoliath ----

/// `attackStrengthGoliath`, and seven blocks of reach.
pub const GOLIATH_STRENGTH: f32 = 7.0;
pub const GOLIATH_REACH_SQR: f64 = 49.0;

// ---- Yuuka: EntityDollAIFarmer ----

/// Ticks between Yuuka's rounds in her other mode.
pub const BLOOM_GAP: i32 = 4;
/// Crops she tends each round in her other mode.
pub const BLOOM_PER_ROUND: usize = 3;
/// Wheat to a loaf, as at a crafting table.
pub const BREAD_WHEAT: u32 = 3;
pub const STACK: u32 = 64;

/// A crop's age after bonemeal: two to five more, as in the game, at most `max`.
pub fn bloom_age(age: u32, max: u32, roll: i32) -> u32 {
    (age + 2 + roll.clamp(0, 3) as u32).min(max)
}

/// Bakes the wheat in `items` into bread, three to a loaf, where there is room for it: the loaves baked.
pub fn bake_bread(items: &mut [Option<ItemStack>]) -> u32 {
    let wheat: u32 = items.iter().flatten().filter(|s| &*s.item == "minecraft:wheat").map(|s| s.count).sum();
    let mut loaves = wheat / BREAD_WHEAT;
    if loaves == 0 {
        return 0;
    }
    // Take the wheat out, then put the bread in; whatever bread finds no room goes back as wheat.
    let mut take = loaves * BREAD_WHEAT;
    for slot in items.iter_mut() {
        if take == 0 {
            break;
        }
        if let Some(s) = slot
            && &*s.item == "minecraft:wheat"
        {
            let n = s.count.min(take);
            s.count -= n;
            take -= n;
            if s.count == 0 {
                *slot = None;
            }
        }
    }
    let mut left = loaves;
    for slot in items.iter_mut() {
        if left == 0 {
            break;
        }
        if let Some(s) = slot
            && &*s.item == "minecraft:bread"
            && s.count < STACK
        {
            let n = (STACK - s.count).min(left);
            s.count += n;
            left -= n;
        }
    }
    for slot in items.iter_mut() {
        if left == 0 {
            break;
        }
        if slot.is_none() {
            let n = left.min(STACK);
            *slot = Some(ItemStack { item: "minecraft:bread".into(), count: n, damage: 0, extra: None });
            left -= n;
        }
    }
    if left > 0 {
        loaves -= left;
        let mut back = left * BREAD_WHEAT;
        for slot in items.iter_mut() {
            if back == 0 {
                break;
            }
            match slot {
                Some(s) if &*s.item == "minecraft:wheat" && s.count < STACK => {
                    let n = (STACK - s.count).min(back);
                    s.count += n;
                    back -= n;
                }
                None => {
                    let n = back.min(STACK);
                    *slot = Some(ItemStack { item: "minecraft:wheat".into(), count: n, damage: 0, extra: None });
                    back -= n;
                }
                _ => {}
            }
        }
    }
    loaves
}

/// `farmRange`: how far, in steps along the axes, Yuuka farms.
pub const FARM_RANGE: i32 = 5;

/// The crops Yuuka plants: the crop, the seed it grows from, and the age it is ripe at.
pub const CROPS: [(&str, &str, &str); 4] = [
    ("minecraft:wheat", "minecraft:wheat_seeds", "7"),
    ("minecraft:carrots", "minecraft:carrot", "7"),
    ("minecraft:potatoes", "minecraft:potato", "7"),
    ("minecraft:beetroots", "minecraft:beetroot_seeds", "3"),
];
/// Seeds that grow a stem: planted, but the stem itself is never harvested.
pub const STEMS: [(&str, &str); 2] = [
    ("minecraft:melon_stem", "minecraft:melon_seeds"),
    ("minecraft:pumpkin_stem", "minecraft:pumpkin_seeds"),
];

pub fn plant_for(seed: &str) -> Option<&'static str> {
    CROPS
        .iter()
        .find(|c| c.1 == seed)
        .map(|c| c.0)
        .or_else(|| STEMS.iter().find(|s| s.1 == seed).map(|s| s.0))
}

pub fn is_stem(name: &str) -> bool {
    name.ends_with("melon_stem") || name.ends_with("pumpkin_stem")
}

/// How long a trampled patch of farmland stays on Yuuka's list to be tilled again.
pub const TRAMPLE_MEMORY: std::time::Duration = std::time::Duration::from_secs(300);
/// Most trampled patches remembered at once.
pub const TRAMPLE_MAX: usize = 1024;

/// Farmland trampled to dirt lately: where, in which level ([`dimension_code`]), and when.
pub static TRAMPLED: LazyLock<Mutex<HashMap<[i32; 3], (u8, std::time::Instant)>>> = LazyLock::new(Default::default);

/// The farmland at `pos` in level `dimension` is being trampled (by a player or a mob jumping on it):
/// a Yuuka near it tills it again.
pub fn trampled(pos: [i32; 3], dimension: u8) {
    let Ok(mut t) = TRAMPLED.lock() else { return };
    t.retain(|_, (_, at)| at.elapsed() < TRAMPLE_MEMORY);
    if t.len() < TRAMPLE_MAX {
        t.insert(pos, (dimension, std::time::Instant::now()));
    }
}

/// The trampled patches in level `dimension` within `reach` steps of `from`, nearest first.
pub fn trampled_near(from: [i32; 3], dimension: u8, reach: i32) -> Vec<[i32; 3]> {
    let Ok(t) = TRAMPLED.lock() else { return Vec::new() };
    let mut near: Vec<[i32; 3]> = t
        .iter()
        .filter(|(p, (d, at))| *d == dimension && at.elapsed() < TRAMPLE_MEMORY && steps(**p, from) <= reach)
        .map(|(p, _)| *p)
        .collect();
    near.sort_by_key(|p| (steps(*p, from), *p));
    near
}

pub fn forget_trampled(pos: [i32; 3]) {
    if let Ok(mut t) = TRAMPLED.lock() {
        t.remove(&pos);
    }
}

/// What each Yuuka last planted or harvested: what she fills empty farmland with.
pub static LAST_CROP: LazyLock<Mutex<HashMap<u32, &'static str>>> = LazyLock::new(Default::default);

/// Ticks to dig a block of `hardness`, less one (at 0 it comes out on the next tick). The Horai doll
/// keeps the original's pace; the others dig as a pickaxe with Efficiency V does (a diamond one's 8
/// and 26 more; the netherite one's 9 and 26, and she is a quarter quicker on top).
pub fn dig_ticks(tier: u8, hardness: f32) -> i32 {
    match tier {
        0 => ((20.0 * hardness / MINE_SPEED).floor() as i32).max(0),
        t => {
            let speed = if t >= 2 { 35.0 * 1.25 } else { 34.0 };
            ((30.0 * hardness / speed).ceil() as i32 - 1).max(0)
        }
    }
}

/// Which ores the ore hunt digs out first: 0 ancient debris, diamonds, gold and emeralds; 2 coal,
/// last of all; 1 every other ore.
pub fn ore_rank(name: &str) -> u8 {
    let s = short(name);
    let s = s.strip_prefix("deepslate_").or_else(|| s.strip_prefix("nether_")).unwrap_or(s);
    match s {
        "ancient_debris" | "diamond_ore" | "gold_ore" | "emerald_ore" => 0,
        "coal_ore" => 2,
        _ => 1,
    }
}

/// Diamonds and ancient debris: what the ore hunt is after.
pub fn is_treasure(name: &str) -> bool {
    matches!(short(name), "diamond_ore" | "deepslate_diamond_ore" | "ancient_debris")
}

/// Structure Add-on's rocks, and the cobble they break into. Not its bricks (made, not dug) or
/// lignite (it drops coal).
const STRUCTURE_ADDON_ROCKS: [&str; 39] = [
    "andesite", "andesite_cobble", "basalt", "basalt_cobble", "black_granite", "black_granite_cobble",
    "blueschist", "blueschist_cobble", "chalk", "chert", "dacite", "dacite_cobble", "dolomite",
    "eclogite", "eclogite_cobble", "gabbro", "gabbro_cobble", "gneiss", "gneiss_cobble", "greenschist",
    "greenschist_cobble", "greywacke", "komatiite", "komatiite_cobble", "limestone",
    "marble", "marble_cobble", "migmatite", "migmatite_cobble", "quartzite", "quartzite_cobble",
    "red_granite", "red_granite_cobble", "rhyolite", "rhyolite_cobble", "shale", "siltstone",
    "soapstone", "soapstone_cobble",
];

/// Rock and earth a quarry leaves behind rather than drop by the hundred: the game's, and
/// Structure Add-on's.
pub fn is_filler(name: &str) -> bool {
    if let Some(rock) = name.strip_prefix("structure_addon:") {
        return STRUCTURE_ADDON_ROCKS.contains(&rock);
    }
    matches!(
        short(name),
        "stone" | "cobblestone" | "deepslate" | "cobbled_deepslate" | "dirt" | "coarse_dirt" | "rooted_dirt"
            | "grass_block" | "podzol" | "mycelium" | "gravel" | "sand" | "red_sand" | "sandstone"
            | "red_sandstone" | "tuff" | "granite" | "diorite" | "andesite" | "calcite" | "netherrack"
            | "basalt" | "smooth_basalt" | "blackstone" | "dripstone_block" | "clay" | "mud" | "terracotta"
    )
}

/// Whether a quarry voids `name` (with keep-rock off).
pub fn voided(name: &str) -> bool {
    is_filler(name)
}

/// What Patchouli's orb does to `target`: nothing to any doll, whoever's.
pub fn orb_damage(target: u32, damage: f32) -> f32 {
    if is_doll(target) { 0.0 } else { damage }
}

pub fn centre(p: [i32; 3]) -> [f64; 3] {
    p.map(|c| f64::from(c) + 0.5)
}

/// What coal and iron ore drop to a pickaxe with Fortune III, which the dolls dig them with: coal
/// and raw iron. Every other ore keeps Silk Touch.
pub fn fortune_drop(name: &str) -> Option<&'static str> {
    match short(name) {
        "coal_ore" | "deepslate_coal_ore" => Some("minecraft:coal"),
        "iron_ore" | "deepslate_iron_ore" => Some("minecraft:raw_iron"),
        _ => None,
    }
}

/// Fortune's `ore_drops` bonus (`ApplyBonusCount.OreDrops`): one drop times one more than
/// `max(0, nextInt(level + 2) - 1)`; `roll` is that `nextInt`. At level III: 1, 1, 2, 3 or 4.
pub fn fortune_count(roll: i32) -> u32 {
    (roll - 1).max(0) as u32 + 1
}

pub const FORTUNE: i32 = 3;

pub fn is_liquid(name: &str) -> bool {
    matches!(name, "minecraft:water" | "minecraft:lava")
}

pub const SIDES: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

/// The mod's own blocks, which no doll ever digs: its anchors and its scaffolding.
pub fn is_doll_block(name: &str) -> bool {
    matches!(name, "alice_dolls:doll_anchor" | "alice_dolls:phantom_doll_scaffolding")
}

/// The quarry's scaffolding: climbable, walked through, and it drops nothing.
pub const PHANTOM_SCAFFOLDING: &str = "alice_dolls:phantom_doll_scaffolding";

/// How far from lava the quarry and the ore hunt keep: seven blocks up or down, three across,
/// never close enough to fall or step in, not so far that they pass ore by.
pub const LAVA_UP: i32 = 7;
pub const LAVA_ACROSS: i32 = 3;

// ---- Strip-mining from the head ----

/// The last block each player broke, and how many they have broken: the api crate's loot hook says.
/// By player, so a Horai on her owner's head digs on after her owner, not after someone else nearby.
pub static PLAYER_DUG: LazyLock<Mutex<HashMap<u128, (u64, [i32; 3])>>> = LazyLock::new(Default::default);

/// The player `player` broke the block at `pos`.
pub fn player_dug(player: u128, pos: [i32; 3]) {
    if let Ok(mut d) = PLAYER_DUG.lock() {
        let e = d.entry(player).or_default();
        e.0 += 1;
        e.1 = pos;
    }
}

/// How many blocks doll `id`'s owner has broken, and the last.
pub fn owner_dug(id: u32) -> (u64, [i32; 3]) {
    let owner = state(id).map_or(0, |s| s.owner);
    PLAYER_DUG.lock().ok().and_then(|d| d.get(&owner).copied()).unwrap_or_default()
}

// ---- The Netherite Horai's quarry ----

/// The 3x3 of a quarry's layer: the middle first, so that, taken from the end, it is dug last.
pub fn layer_cells(site: [i32; 3], y: i32) -> Vec<[i32; 3]> {
    let mut v = vec![[site[0], y, site[2]]];
    for dx in -1..=1 {
        for dz in -1..=1 {
            if dx != 0 || dz != 0 {
                v.push([site[0] + dx, y, site[2] + dz]);
            }
        }
    }
    v
}

/// On patrol, the Netherite Horai digs a 3x3 shaft straight down, a few blocks behind her owner, to
/// bedrock. Any ore she uncovers on the way she follows through its vein, then goes back to the
/// shaft. She keeps seven blocks above or below lava and three beside it: a shaft that would come
/// closer she leaves for another spot. A finished (or left) shaft gets a column of phantom doll scaffolding up one corner, to
/// climb out by; it drops nothing when broken. Then she teleports back to her owner and starts again behind them.
/// The ores drop where they were, for the dolls to pick up; the rock and earth she leaves.
/// Ticks a Netherite Horai waits, once put on patrol, before she teleports off to quarry: four seconds.
pub const QUARRY_DELAY: i32 = 80;

// ---- The giga quarry ----

/// Sneak and right-click a Horai with a golden pickaxe: the giga quarry. She digs out a 20x20
/// pit, layer by layer, from where she stood down to bedrock, at her own pace (and the netherite
/// one twice over, as in her quarry). She never digs a block beside lava or water, so the pit
/// doesn't flood: those stay as walls. Again, or any other order, ends it. Finished or ended, she
/// leaves a line of ladders down the pit's wall from the block she dug first.
pub const GIGA: u8 = 6;
/// Yuuka's other mode.
pub const BLOOM: u8 = 7;
/// Its width and length.
pub const GIGA_SIZE: i32 = 20;

/// Each giga quarry's corner (its least x and z, and the top layer's y), as the api crate saves it.
pub static GIGA_SITES: LazyLock<Mutex<HashMap<u32, [i32; 3]>>> = LazyLock::new(Default::default);

pub fn set_giga_site(id: u32, site: Option<[i32; 3]>) {
    if let Ok(mut g) = GIGA_SITES.lock() {
        match site {
            Some(s) => g.insert(id, s),
            None => g.remove(&id),
        };
    }
}

pub fn giga_site(id: u32) -> Option<[i32; 3]> {
    GIGA_SITES.lock().ok()?.get(&id).copied()
}

/// The corner of a giga quarry centred on `feet`, its top layer the block under them.
pub fn giga_corner(feet: [i32; 3]) -> [i32; 3] {
    [feet[0] - GIGA_SIZE / 2, feet[1] - 1, feet[2] - GIGA_SIZE / 2]
}

/// The cells of layer `y` of the giga quarry at `site`, in the order they are taken from the end:
/// row by row, each row back and forth, so she works across the pit without long jumps.
pub fn giga_cells(site: [i32; 3], y: i32) -> Vec<[i32; 3]> {
    let mut v = Vec::with_capacity((GIGA_SIZE * GIGA_SIZE) as usize);
    for row in 0..GIGA_SIZE {
        for k in 0..GIGA_SIZE {
            let col = if row % 2 == 0 { k } else { GIGA_SIZE - 1 - k };
            v.push([site[0] + col, y, site[2] + row]);
        }
    }
    v.reverse();
    v
}

// ---- The ore hunt ----

/// How far around her the ore hunt looks, across and up or down, and how far from her owner it
/// will go.
pub const SEEK_ACROSS: i32 = 16;
pub const SEEK_UP: i32 = 12;
pub const SEEK_FROM_OWNER: f64 = 40.0;

// ---- Moscow: bound to another doll ----

/// Each Moscow doll's doll, as the api crate finds it.
pub static BOUND: LazyLock<Mutex<HashMap<u32, u32>>> = LazyLock::new(Default::default);

pub fn set_bound(id: u32, to: Option<u32>) {
    if let Ok(mut b) = BOUND.lock() {
        match to {
            Some(to) => b.insert(id, to),
            None => b.remove(&id),
        };
    }
}

pub fn bound_to(id: u32) -> Option<u32> {
    BOUND.lock().ok()?.get(&id).copied()
}

// ---- Mining dolls fetch what they dug ----

/// How far a mining doll teleports for something left lying, and how long it waits there for it.
pub const FETCH_RANGE_SQR: f64 = 400.0;
pub const FETCH_TICKS: i32 = 40;

// ---- Blocks players placed, which no doll digs; and whether a Horai keeps the rock it digs ----

/// Blocks a player placed: the api crate's place hook adds them and its loot hook takes them away
/// when broken. It saves them.
pub static PLACED: LazyLock<Mutex<(HashSet<[i32; 3]>, bool)>> = LazyLock::new(Default::default);

pub fn is_placed(p: [i32; 3]) -> bool {
    PLACED.lock().is_ok_and(|s| s.0.contains(&p))
}

pub fn mark_placed(p: [i32; 3]) {
    if let Ok(mut s) = PLACED.lock() {
        s.1 |= s.0.insert(p);
    }
}

pub fn unmark_placed(p: [i32; 3]) {
    if let Ok(mut s) = PLACED.lock() {
        s.1 |= s.0.remove(&p);
    }
}

/// The placed blocks as a file's text, when they changed since the last call.
pub fn placed_changes() -> Option<String> {
    let mut s = PLACED.lock().ok()?;
    if !s.1 {
        return None;
    }
    s.1 = false;
    Some(s.0.iter().map(|p| format!("{} {} {}\n", p[0], p[1], p[2])).collect())
}

/// Reads a list `placed_changes` wrote.
pub fn load_placed(text: &str) {
    let Ok(mut s) = PLACED.lock() else { return };
    for line in text.lines() {
        let v: Vec<i32> = line.split_whitespace().filter_map(|w| w.parse().ok()).collect();
        if let [x, y, z] = v[..] {
            s.0.insert([x, y, z]);
        }
    }
}

/// The Horai dolls set, in their inventory screen, to keep the stone and earth they dig.
pub static KEEP_ROCK: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Default::default);

pub fn set_keep_rock(id: u32, keep: bool) {
    if let Ok(mut k) = KEEP_ROCK.lock() {
        if keep {
            k.insert(id);
        } else {
            k.remove(&id);
        }
    }
}

pub fn keep_rock(id: u32) -> bool {
    KEEP_ROCK.lock().is_ok_and(|k| k.contains(&id))
}

// ---- The dolls' inventory screens: armour and shield slots, and Moscow's 128s ----
//
// As Touhou Little Maid's maid screen (`MaidMainContainer`): each armour slot holds one piece and
// takes only its own, cursed armour stays on outside creative, and the shield slot takes a shield.
// The api crate names each doll menu it opens; the game's menu engine asks these through the
// `doll-slot-place` and `doll-slot-max` injections.

/// The slot, of `n`, a doll holds its shield in: the one before its armour.
pub fn shield_slot(n: usize) -> usize {
    n - 1 - ARMOUR_SLOTS
}

/// Touhou Little Maid's baubles, as this mod's items, and the Totem of Undying.
pub const BAUBLES: [&str; 10] = [
    "alice_dolls:fire_protect_bauble",
    "alice_dolls:explosion_protect_bauble",
    "alice_dolls:projectile_protect_bauble",
    "alice_dolls:magic_protect_bauble",
    "alice_dolls:fall_protect_bauble",
    "alice_dolls:nimble_fabric",
    "alice_dolls:item_magnet_bauble",
    "alice_dolls:mute_bauble",
    "alice_dolls:ultramarine_orb_elixir",
    "minecraft:totem_of_undying",
];

pub fn is_bauble(item: &str) -> bool {
    BAUBLES.contains(&item)
}

/// How many baubles a doll wears: a 3x3 grid.
pub const BAUBLE_SLOTS: usize = 9;
/// The grid's columns in its three rows of a chest screen, and its rows' cells.
pub const BAUBLE_COLS: std::ops::Range<usize> = 3..6;
pub const BAUBLE_CELLS: usize = 27;

/// Each doll's baubles, as the api crate keeps them.
pub static WORN: LazyLock<Mutex<HashMap<u32, Vec<Arc<str>>>>> = LazyLock::new(Default::default);

pub fn set_baubles(id: u32, worn: Vec<Arc<str>>) {
    if let Ok(mut w) = WORN.lock() {
        w.insert(id, worn);
    }
}

pub fn baubles(id: u32) -> Vec<Arc<str>> {
    WORN.lock().ok().and_then(|w| w.get(&id).cloned()).unwrap_or_default()
}

/// Whether doll `id` wears bauble `item`.
pub fn wears(id: u32, item: &str) -> bool {
    WORN.lock().is_ok_and(|w| w.get(&id).is_some_and(|b| b.iter().any(|x| &**x == item)))
}

/// Which bauble cell, of the grid starting at menu slot `at`, menu slot `i` is: Some(Some(n)) for
/// the grid's `n`th, Some(None) for the filler around it, None outside its three rows.
pub fn bauble_cell(i: usize, at: usize) -> Option<Option<usize>> {
    if i < at || i >= at + BAUBLE_CELLS {
        return None;
    }
    let (row, col) = ((i - at) / 9, (i - at) % 9);
    Some(BAUBLE_COLS.contains(&col).then(|| row * BAUBLE_COLS.len() + col - BAUBLE_COLS.start))
}

/// The part of the inventory shown by a paged menu.
#[derive(Clone, Copy, Debug)]
pub struct MenuLayout {
    pub slots: usize,
    pub offset: usize,
    pub total: usize,
}

// ---- The nimble fabric ----

/// Dolls whose nimble fabric took a shot for them: they teleport on their next tick.
pub static DODGE: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Default::default);

pub fn request_dodge(id: u32) {
    if let Ok(mut d) = DODGE.lock() {
        d.insert(id);
    }
}

// ---- The doll anchor ----

/// Each patrolling doll's anchor, as the api crate finds it, and that anchor's radius.
pub static ANCHOR_OF: LazyLock<Mutex<HashMap<u32, ([i32; 3], f64)>>> = LazyLock::new(Default::default);

pub fn set_anchor(id: u32, at: Option<([i32; 3], f64)>) {
    if let Ok(mut a) = ANCHOR_OF.lock() {
        match at {
            Some(at) => a.insert(id, at),
            None => a.remove(&id),
        };
    }
}

// ---- Wakasagi fishes ----

/// What Wakasagi catches, for the api crate to put in her inventory: the doll and the item.
pub static CATCHES: Mutex<Vec<(u32, &'static str)>> = Mutex::new(Vec::new());

pub fn take_catches() -> Vec<(u32, &'static str)> {
    CATCHES.lock().map(|mut c| std::mem::take(&mut *c)).unwrap_or_default()
}

/// Vanilla's fishing (`gameplay/fishing`): fish 85 times in 100 (cod, salmon, tropical fish and
/// pufferfish, by their weights), junk 10, treasure 5.
/// The fishing rod Wakasagi fishes with: the first in her inventory, and its enchantments.
#[derive(Clone, Copy, Debug)]
pub struct Rod {
    pub slot: usize,
    pub lure: u32,
    pub luck: u32,
    pub unbreaking: u32,
}

pub fn rod_of(id: u32) -> Option<Rod> {
    let items = inventory(id)?;
    let (slot, s) = items.iter().enumerate().find_map(|(i, s)| s.as_ref().filter(|s| &*s.item == "minecraft:fishing_rod").map(|s| (i, s)))?;
    let level = |name| s.enchantment_level(name);
    Some(Rod { slot, lure: level("minecraft:lure"), luck: level("minecraft:luck_of_the_sea"), unbreaking: level("minecraft:unbreaking") })
}

/// The love level from which Wakasagi fishes with Lure III and Luck of the Sea III of her own,
/// whatever rod she has (a better rod still counts for more).
pub const NATURAL_FISHER_LEVEL: u32 = 3;

/// Her Lure and Luck of the Sea: her rod's (its Lure and Luck), and from love level 3 at least III
/// of each.
pub fn fishing_enchantments(rod: Option<(u32, u32)>, level: u32) -> (u32, u32) {
    let (lure, luck) = rod.unwrap_or((0, 0));
    if level >= NATURAL_FISHER_LEVEL { (lure.max(3), luck.max(3)) } else { (lure, luck) }
}

/// A fishing rod's durability.
pub const ROD_DURABILITY: u32 = 64;

/// How far Wakasagi casts at an enemy, and how often: her line reaches sixteen blocks, and she casts
/// again once a second and a half has passed since she reeled the last one in.
pub const HOOK_RANGE_SQR: f64 = 16.0 * 16.0;
pub const HOOK_EVERY: i64 = 30;
/// Ticks from her cast to her reel: the bobber's flight.
pub const HOOK_FLIGHT: i32 = 8;

/// `FishingHook.pullEntity`'s pull on a mob at `at` hooked by a line from `from`: a tenth of the way
/// between them, with a little lift so a mob on the ground is not dragged into it.
pub fn hook_pull(from: [f64; 3], at: [f64; 3]) -> [f64; 3] {
    [(from[0] - at[0]) * 0.1, (from[1] - at[1]) * 0.1 + 0.2, (from[2] - at[2]) * 0.1]
}

// ---- Aya's photos ----

/// Aya dolls that just took a photo worth keeping, for the api crate to save as a screenshot.
pub static PHOTOS: Mutex<Vec<Photo>> = Mutex::new(Vec::new());

/// A photo Aya took: where her eyes were and which way she faced (degrees, as a camera's), for the
/// api crate to take the screenshot from there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Photo {
    pub doll: u32,
    pub eye: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
}

/// Aya dolls punched with an empty hand since their last tick: each takes a photo of the puncher.
pub static SNAPPED: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Default::default);

pub fn snap(id: u32) {
    if let Ok(mut s) = SNAPPED.lock() {
        s.insert(id);
    }
}

/// Yaw and pitch, in degrees as vanilla's camera takes them, looking from `from` to `to`.
pub fn yaw_pitch(from: [f64; 3], to: [f64; 3]) -> (f32, f32) {
    let (dx, dy, dz) = (to[0] - from[0], to[1] - from[1], to[2] - from[2]);
    let flat = (dx * dx + dz * dz).sqrt();
    ((-dx).atan2(dz).to_degrees() as f32, (-dy.atan2(flat)).to_degrees() as f32)
}

pub fn take_photos() -> Vec<Photo> {
    PHOTOS.lock().map(|mut p| std::mem::take(&mut *p)).unwrap_or_default()
}

// ---- Ooedo, punched ----

/// Ooedo dolls punched with an empty hand: each lights its fuse and goes off.
pub static DETONATE: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Default::default);

/// Ooedo doll `id` was punched with an empty hand.
pub fn detonate(id: u32) {
    if let Ok(mut d) = DETONATE.lock() {
        d.insert(id);
    }
}

/// How long a punched Ooedo's fuse burns: half a second.
pub const PUNCH_FUSE: i32 = 10;


pub static ON_MISSION: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Default::default);
pub static BURST: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Default::default);

pub fn set_mission(id: u32, on: bool) {
    if let Ok(mut m) = ON_MISSION.lock() {
        if on {
            m.insert(id);
        } else {
            m.remove(&id);
        }
    }
}

pub fn on_mission(id: u32) -> bool {
    ON_MISSION.lock().is_ok_and(|m| m.contains(&id))
}

pub fn missions() -> Vec<u32> {
    ON_MISSION.lock().map(|m| m.iter().copied().collect()).unwrap_or_default()
}

/// Every doll on its owner's head now.
pub fn riders() -> Vec<u32> {
    STATES.lock().map(|s| s.iter().filter(|(_, s)| s.mode == RIDE && s.enabled).map(|(&id, _)| id).collect()).unwrap_or_default()
}

pub fn burst(id: u32) {
    if let Ok(mut b) = BURST.lock() {
        b.insert(id);
    }
}

/// A blast a little bigger than a creeper's.
pub const GUIGNOL_BLAST: f32 = 4.0;

/// When each Ooedo last went off: for ten seconds after, nothing hurts it.
pub static BLASTED: LazyLock<Mutex<HashMap<u32, std::time::Instant>>> = LazyLock::new(Default::default);
pub const BLAST_GUARD: std::time::Duration = std::time::Duration::from_secs(10);

pub fn blasted(id: u32) {
    if let Ok(mut b) = BLASTED.lock() {
        b.insert(id, std::time::Instant::now());
    }
}

/// Whether Ooedo doll `id` went off within the last ten seconds (the api crate's damage hook asks).
pub fn just_blasted(id: u32) -> bool {
    BLASTED.lock().is_ok_and(|b| b.get(&id).is_some_and(|t| t.elapsed() < BLAST_GUARD))
}

/// A doll that hasn't reached its anchor in this long, path or no path, teleports to it.
pub const ANCHOR_GIVE_UP: i32 = 30 * 20;
