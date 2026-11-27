//! Yukari Gaps Beta for Horizon: gaps to travel between, inspired by Waystones. MIT licensed.
//!
//! A gap is one block; its block entity draws the gap, pipef0x's Yukari Gap Demo look with its two
//! swinging bows, turned on its upright axis to face whoever looks at it. Place one and name it, and lock it if only you should use it.
//! Right-click any gap to see every other gap you may use, and click one to go there for free.
//! Others have to find a gap (right-click it once) before they can travel to it, unless its owner
//! made it global with a Yukari umbrella, which makes it a Giga Gap. A locked gap is only its owner's, and of the players
//! they share it with. Sneak and right-click your own to rename it, lock it or share it.

pub mod shader;
mod store;
mod ui;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use mod_api::{
    ActiveEffect, BlockEntityDrawContext, BlockEntityPart, BlockEntityRenderBoundingBoxEvent,
    BlockEntitySubmitEvent, BlockEntityUpdateTagEvent, BlockPart, BlockUpdateFlags,
    BreakBlockEvent, ClientHost, ClientTickPostEvent, Compound, EntityInteractEvent,
    EntityPlaceEvent, Flow, Hand, HookId, Hooks, Host, HostBlockStates, HostSide, HostTransaction,
    IPayloadContext, IPayloadHandler, InteractionResult, ItemPlace, ItemResource, ItemStack,
    LevelRef, ParticleOptions, PayloadBytes, PayloadHandleError, PayloadId, PlayerRef,
    RandomSource, Registrar, RegistrationContext, RightClickBlockEvent, ScreenCharTypedPreEvent,
    ScreenKeyPressedPreEvent, ScreenMouseButtonPressedPreEvent, ScreenMouseButtonReleasedPreEvent,
    ScreenMouseDraggedPreEvent, ScreenMouseScrolledPreEvent, ScreenRenderPostEvent, ServerHost,
    ServerTickEvent, ServerTickPhase, SoundSource, Tag, Text,
};

use store::{Entry, Gap, Key, Naming, Player, Store, clean};
use ui::{MOD_ID, UI, Ui};

#[derive(Default)]
struct Mod;

/// The content block and item ID.
const GAP: &str = "yukari_gap:yukari_gap";
/// The SDK type shared by regular and Giga Gap block entities.
pub const GAP_ENTITY: &str = "yukari_gap_native:gap";
/// The Yukari umbrella: its owner right-clicks a gap with it to make it global, or not, which
/// uses the umbrella up. It can't be crafted: Yukari drops it, 70 times in 100, and Yukari Great
/// Doll every time (Touhou Mobs drops it, when one of its bosses is beaten for good).
const UMBRELLA: &str = "yukari_gap:yukari_umbrella";
/// The picture of the gap, from the content mod's `textures/entity/gap.png`: pipef0x's Yukari Gap
/// Demo shader (MIT), baked into an animated sprite. The gap breathes and its fog and eyes drift.
const GAP_SPRITE: &str = "yukari_gap:entity/gap";
/// The bow tied at each tip of the gap, pipef0x's, from `textures/entity/bow.png`.
const BOW_SPRITE: &str = "yukari_gap:entity/bow";
const BOW_COLOR: &str = "BowColor";
/// The vanilla Nether portal ambience shared by both gap sizes.
pub const AMBIENT_SOUND: &str = "minecraft:block.portal.ambient";
const DIZZINESS: &str = "yukari_gap:dizziness";

fn dye(item: &str) -> Option<u32> {
    Some(
        match item.strip_prefix("minecraft:")?.strip_suffix("_dye")? {
            "white" => 0xf9fffe,
            "orange" => 0xf9801d,
            "magenta" => 0xc74ebd,
            "light_blue" => 0x3ab3da,
            "yellow" => 0xfed83d,
            "lime" => 0x80c71f,
            "pink" => 0xf38baa,
            "gray" => 0x474f52,
            "light_gray" => 0x9d9d97,
            "cyan" => 0x169c9c,
            "purple" => 0x8932b8,
            "blue" => 0x3c44aa,
            "brown" => 0x835432,
            "green" => 0x5e7c16,
            "red" => store::DEFAULT_BOW,
            "black" => 0x1d1d21,
            _ => return None,
        },
    )
}

fn saved_bow_color(data: &Compound) -> Option<u32> {
    match data.get(&BOW_COLOR.into()) {
        Some(Tag::Int(color)) => Some((*color as u32) & 0xffffff),
        _ => None,
    }
}

fn bow_color(data: &Compound) -> u32 {
    saved_bow_color(data).unwrap_or(store::DEFAULT_BOW)
}

fn current_bow_color(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<u32> {
    let data = host.block_entity(level, pos)?.data(host)?;
    saved_bow_color(&data)
}

fn sync_bow(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], color: u32) {
    let Some(entity) = host.block_entity(level, pos) else {
        return;
    };
    let data: Compound = [(BOW_COLOR.into(), Tag::Int(color as i32))].into();
    if matches!(entity.merge_data(host, data), Ok(true)) {
        host.block_entity_sync(pos);
    }
}

fn take_one(host: &mut dyn ServerHost, player: u128, hand: Hand, stack: &ItemStack) -> bool {
    let place = ItemPlace::PlayerHand { player, hand };
    let resource = ItemResource::of(stack);
    host.transaction(|host, tx| Ok::<_, ()>(place.extract(host, tx, &resource, 1) == 1))
        .unwrap_or(false)
}

fn recolor(
    host: &mut dyn ServerHost,
    key: &Key,
    player: &Traveler,
    hand: Hand,
    stack: &ItemStack,
    color: u32,
) {
    let Some(level) = host.level(&key.0) else {
        return;
    };
    let stored = with_store(host, |s| (s.find(key).map(|gap| gap.color), false)).flatten();
    let previous = current_bow_color(host, level, key.1).or(stored);
    if previous.is_none_or(|previous| previous == color)
        || (!player.creative && !take_one(host, player.uuid, hand, stack))
    {
        return;
    }
    if with_store(host, |s| {
        let Some(gap) = s.find_mut(key) else {
            return (false, false);
        };
        gap.color = color;
        (true, true)
    }) == Some(true)
    {
        sync_bow(host, level, key.1, color);
        level.play_sound(
            host,
            "minecraft:item.dye.use",
            center(key.1),
            SoundSource::Blocks,
            1.0,
            1.0,
        );
    }
}

fn center(pos: [i32; 3]) -> [f64; 3] {
    pos.map(|v| f64::from(v) + 0.5)
}

/// The Giga Gap: what a gap becomes when it is made global, and turns back from. A gap in every
/// way, drawn bigger with its own picture (`textures/entity/giga_gap.png`, baked by
/// `bake_gap_alt.py` from a back wall and an eye). It has no item: it drops a Yukari Gap.
const GIGA: &str = "yukari_gap:giga_gap";
const GIGA_SPRITE: &str = "yukari_gap:entity/giga_gap";

/// Either gap block.
fn is_gap(block: &str) -> bool {
    block == GAP || block == GIGA
}

/// The picture a gap block is drawn with.
fn gap_sprite(block: &str) -> &'static str {
    if block == GIGA {
        GIGA_SPRITE
    } else {
        GAP_SPRITE
    }
}
/// How big the picture is, in blocks.
const GAP_SIZE: f32 = 2.25;
/// Where the gap's tips are in the picture, from its top left and from its bottom right corner,
/// as a share of its side: the demo's Bezier ends, (0.125, 0.125) and (0.875, 0.875).
const GAP_TIP: f32 = 0.125;
/// How big a bow is, as a share of its gap: the demo's 0.45 of its 2 block gap.
const BOW_SHARE: f32 = 0.225;
/// How much bigger the Giga Gap is drawn than the gap, bows and all.
const GIGA_SCALE: f32 = 1.25;

/// How big a gap block's picture is, in blocks.
fn gap_size(block: &str) -> f32 {
    if block == GIGA {
        GAP_SIZE * GIGA_SCALE
    } else {
        GAP_SIZE
    }
}

/// How far above the block a picture `size` blocks big has its bottom edge: low enough that the
/// gap's lower tip sits on the top of the block, where the second block starts.
fn gap_lift(size: f32) -> f32 {
    1.0 - size * GAP_TIP
}

/// How far a bow swings each way, in degrees (the demo's 2π/16), and how long a swing there and
/// back takes, in ticks (the demo's 4 seconds).
const BOW_SWING: f32 = 22.5;
const BOW_PERIOD: f64 = 80.0;
/// Full bright, sky and block light 15: the demo's gap and bows ignore the light.
const FULL_BRIGHT: u32 = 240 | 240 << 8;

// ---- The world's gaps ----

/// The open world's store, and the file it came from: another world loads its own.
static STORE: LazyLock<Mutex<Option<(PathBuf, Store)>>> = LazyLock::new(Default::default);

fn store_path(host: &dyn ServerHost) -> Option<PathBuf> {
    Some(host.world_dir()?.join("yukari_gap").join("gaps.tsv"))
}

/// Runs `f` on the open world's store, loading it first if this world's isn't loaded; saves it
/// when `f` says it changed it.
fn with_store<R>(host: &dyn ServerHost, f: impl FnOnce(&mut Store) -> (R, bool)) -> Option<R> {
    let path = store_path(host)?;
    let mut guard = STORE.lock().ok()?;
    if guard.as_ref().is_none_or(|(p, _)| *p != path) {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        *guard = Some((path.clone(), Store::decode(&text)));
    }
    let (_, store) = guard.as_mut()?;
    let (result, changed) = f(store);
    if changed {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, store.encode());
    }
    Some(result)
}

// ---- Names ----

const FIRST: [&str; 24] = [
    "Ael", "Bran", "Cor", "Dun", "Eld", "Fen", "Gal", "Har", "Ith", "Kel", "Lor", "Mor", "Nor",
    "Orm", "Quel", "Ros", "Sil", "Thal", "Ul", "Vor", "Wen", "Yr", "Ash", "Brin",
];
const LAST: [&str; 16] = [
    "ford", "wick", "mere", "hold", "vale", "stead", "moor", "crest", "fell", "haven", "rest",
    "wyn", "dale", "gate", "hollow", "watch",
];

/// A name for a gap at `pos` until its owner names it: where to start among the names comes from
/// `pos`, and the first one no gap has from there is taken.
fn new_name(pos: [i32; 3], store: &Store) -> String {
    let hash = (i64::from(pos[0]).wrapping_mul(73_856_093)
        ^ i64::from(pos[1]).wrapping_mul(19_349_663)
        ^ i64::from(pos[2]).wrapping_mul(83_492_791))
    .unsigned_abs() as usize;
    let all = FIRST.len() * LAST.len();
    let name_at = |n: usize| {
        format!(
            "{}{}",
            FIRST[n % FIRST.len()],
            LAST[(n / FIRST.len()) % LAST.len()]
        )
    };
    let free = |name: &str| !store.gaps.iter().any(|g| g.name.eq_ignore_ascii_case(name));
    (0..all)
        .map(|i| name_at(hash.wrapping_add(i) % all))
        .find(|n| free(n))
        .unwrap_or_else(|| store.unique_name(&name_at(hash % all), None))
}

// ---- The two halves ----

fn above(pos: [i32; 3]) -> [i32; 3] {
    [pos[0], pos[1] + 1, pos[2]]
}

fn below(pos: [i32; 3]) -> [i32; 3] {
    [pos[0], pos[1] - 1, pos[2]]
}

fn is_air(block: &str) -> bool {
    matches!(
        block,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

/// Where the gap whose half is at `pos` is kept: its lower half.
fn lower_half(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> [i32; 3] {
    if level.block_property(host, pos, "half").as_deref() == Some("upper") {
        below(pos)
    } else {
        pos
    }
}

/// Breaking one half breaks the other: a guard, so that break doesn't come back here.

// ---- Facing ----

/// The way a gap faces when a player looking along `yaw` places it: towards them.
fn facing_towards(yaw: f32) -> &'static str {
    let yaw = yaw.rem_euclid(360.0);
    // Minecraft's yaw: 0 looks south, 90 west, 180 north, 270 east.
    match ((yaw + 45.0) / 90.0) as i32 % 4 {
        0 => "north",
        1 => "east",
        2 => "south",
        _ => "west",
    }
}

fn step(facing: &str) -> [i32; 3] {
    match facing {
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        _ => [0, 0, -1],
    }
}

/// The yaw that looks along `facing`.
fn yaw_of(facing: &str) -> f32 {
    match facing {
        "south" => 0.0,
        "west" => 90.0,
        "north" => 180.0,
        _ => 270.0,
    }
}

/// The turn about the upright axis that points a quad's front (+z) along (`dx`, `dz`).
fn facing_yaw(dx: f64, dz: f64) -> f32 {
    dx.atan2(dz).to_degrees() as f32
}

// ---- The server's side ----

/// A right-click reaches the game once per hand: one counts only after a pause.
static LAST_CLICK: LazyLock<Mutex<HashMap<u128, Instant>>> = LazyLock::new(Default::default);
const CLICK_GAP: Duration = Duration::from_millis(250);

fn fresh_click(player: u128) -> bool {
    let Ok(mut clicks) = LAST_CLICK.lock() else {
        return true;
    };
    let now = Instant::now();
    let fresh = clicks
        .get(&player)
        .is_none_or(|at| now.duration_since(*at) >= CLICK_GAP);
    clicks.insert(player, now);
    fresh
}

/// The gap each player last opened the travel screen at: a trip has to leave from it.
static OPENED_AT: LazyLock<Mutex<HashMap<u128, Key>>> = LazyLock::new(Default::default);
/// How near the gap a player has to stay for the trip to go.
const REACH: f64 = 8.0;

/// Alice Dolls Restringed's Yukari Doll: her owner's portable gap. A click opens the travel screen
/// where she is, and she follows them through; she is no gap to travel to. A doll is its own mob,
/// `alice_dolls:doll`, since Alice Dolls 0.43; before, it was a chicken (one not yet converted
/// still is, until its first tick).
const DOLL_KINDS: [&str; 2] = ["alice_dolls:doll", "minecraft:chicken"];
const YUKARI_DOLL: &str = "Yukari Doll";
/// The tag Alice Dolls gives her, naming her owner.
fn owner_tag(owner: u128) -> String {
    format!("alice_dolls.owner.{owner:032x}")
}
/// The Yukari Doll each player last opened the travel screen at, instead of a gap.
static PORTABLE: LazyLock<Mutex<HashMap<u128, u32>>> = LazyLock::new(Default::default);

/// Whether Yukari Doll `id` is within reach of `player`.
fn doll_near(host: &dyn Host, player: &Traveler, id: u32) -> bool {
    let Some(e) = host.entity(id) else {
        return false;
    };
    let here = e
        .level(host)
        .is_some_and(|l| l.dimension_id() == player.dimension);
    here && e.position(host).is_some_and(|at| {
        (0..3)
            .map(|i| (player.position[i] - at[i]).powi(2))
            .sum::<f64>()
            <= REACH * REACH
    })
}

/// Sends `player` the travel screen from their Yukari Doll `id`: every gap they may go to.
fn show_portable(host: &mut dyn ServerHost, player: &Traveler, id: u32) {
    let from = host
        .entity(id)
        .and_then(|entity| entity.position(host))
        .unwrap_or(player.position);
    let entries = with_store(host, |s| {
        let list = s
            .destinations(player.uuid, None)
            .into_iter()
            .map(|g| travel_entry(g, from, player.creative))
            .collect::<Vec<_>>();
        (list, false)
    })
    .unwrap_or_default();
    if let Ok(mut opened) = OPENED_AT.lock() {
        opened.remove(&player.uuid);
    }
    if let Ok(mut p) = PORTABLE.lock() {
        p.insert(player.uuid, id);
    }
    player.handle.play_sound(
        host,
        "minecraft:block.amethyst_block.resonate",
        SoundSource::Master,
        1.0,
        1.2,
    );
    send(host, player.uuid, store::LIST, store::encode_list(&entries));
}

struct Traveler {
    handle: PlayerRef,
    entity: mod_api::EntityRef,
    uuid: u128,
    dimension: String,
    position: [f64; 3],
    name: String,
    yaw: f32,
    health: f32,
    creative: bool,
    spectator: bool,
    sneaking: bool,
}

impl Traveler {
    fn read(host: &dyn ServerHost, player: PlayerRef) -> Option<Self> {
        let entity = player.entity();
        let game_mode = player.game_mode(host)?;
        Some(Self {
            handle: player,
            entity,
            uuid: player.uuid(),
            dimension: player.level(host)?.dimension_id().to_owned(),
            position: entity.position(host)?,
            name: player.name(host)?,
            yaw: entity.rotation(host)?[0],
            health: host.living(entity)?.health(host)?,
            creative: game_mode == mod_api::GameMode::Creative,
            spectator: game_mode == mod_api::GameMode::Spectator,
            sneaking: player.is_secondary_use_active(host) == Some(true),
        })
    }
}

fn player_named(host: &dyn ServerHost, uuid: u128) -> Option<Traveler> {
    Traveler::read(host, host.player(uuid)?)
}

fn action_bar(host: &mut dyn ServerHost, uuid: u128, text: Text) {
    if let Some(player) = host.player(uuid) {
        let _ = player.action_bar(host, text);
    }
}

fn send(host: &mut dyn ServerHost, uuid: u128, channel: &str, bytes: Vec<u8>) {
    if let Some(player) = host.player(uuid) {
        let _ = player.send_payload(host, &PayloadId(channel.into()), PayloadBytes(bytes));
    }
}

fn near(player: &Traveler, key: &Key) -> bool {
    let (dimension, pos) = key;
    let d = |i: usize| player.position[i] - (f64::from(pos[i]) + 0.5);
    player.dimension == *dimension && d(0) * d(0) + d(1) * d(1) + d(2) * d(2) <= REACH * REACH
}

/// Who the owner of `gap` can share it with: everyone online but them, then those it is shared
/// with who are offline, by name.
fn share_list(host: &dyn ServerHost, gap: &Gap, owner: u128) -> Vec<Player> {
    let mut players: Vec<Player> = host
        .players()
        .server_players()
        .into_iter()
        .filter(|p| p.uuid() != owner)
        .filter_map(|p| {
            Some(Player {
                uuid: p.uuid(),
                shared: gap.is_shared_with(p.uuid()),
                online: true,
                name: p.name(host)?,
            })
        })
        .collect();
    players.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    for (uuid, name) in &gap.shared {
        if !players.iter().any(|p| p.uuid == *uuid) {
            players.push(Player {
                uuid: *uuid,
                name: name.clone(),
                shared: true,
                online: false,
            });
        }
    }
    players
}

/// Asks the owner of the gap at `key` to name it: the client opens the naming screen.
fn ask_name(host: &mut dyn ServerHost, player: u128, gap: &Gap) {
    let players = share_list(host, gap, player);
    let naming = Naming {
        key: gap.key(),
        name: gap.name.clone(),
        locked: gap.locked,
        global: gap.global,
        players,
    };
    send(host, player, store::NAME, store::encode_naming(&naming));
}

/// The owner shares their gap with a player, or stops: only while it is locked does it matter,
/// but the list is kept either way.
fn share(host: &mut dyn ServerHost, uuid: u128, s: store::Share) {
    let Some(player) = player_named(host, uuid) else {
        return;
    };
    if !near(&player, &s.key) {
        return;
    }
    // A name from the server's own list, when they are online.
    let name = player_named(host, s.player).map_or(s.name.clone(), |p| p.name);
    let result = with_store(host, |st| match st.find_mut(&s.key) {
        None => (Err("That gap is gone".to_owned()), false),
        Some(g) if g.owner != uuid => (Err("Only its owner can share this gap".to_owned()), false),
        Some(g) => {
            let changed = g.share(s.player, &name, s.on);
            (Ok(g.name.clone()), changed)
        }
    });
    match result {
        Some(Ok(gap)) if s.on => {
            action_bar(host, uuid, Text::literal(format!("{name} can use {gap}")));
            action_bar(
                host,
                s.player,
                Text::literal(format!("{} shared the gap {gap} with you", player.name)),
            );
        }
        Some(Ok(gap)) => {
            action_bar(
                host,
                uuid,
                Text::literal(format!("{name} can no longer use {gap}")),
            );
        }
        Some(Err(why)) => {
            action_bar(host, uuid, Text::literal(why));
        }
        None => {}
    }
}

/// Repair gaps already saved without an entity during the SDK migration.
/// A half-state rewrite runs the declared block factory without replacing the gap block.
fn restore_gap_entity(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], color: u32) {
    let Some(state) = level.block_state(host, pos) else {
        return;
    };
    if !is_gap(&state.block().id(host)) {
        return;
    }
    if host.block_entity(level, pos).is_none()
        && let Ok(transient) = state.with(&host.block_states(), "half", "upper")
        && transient != state
        && matches!(
            level.set_block(host, pos, transient, BlockUpdateFlags::KNOWN_SHAPE),
            Ok(true)
        )
    {
        let _ = level.set_block(
            host,
            pos,
            state,
            BlockUpdateFlags::CLIENTS | BlockUpdateFlags::KNOWN_SHAPE,
        );
    }
    // Only a missing tag needs recovery from the world store. Keep the dye
    // on a loaded entity, including tags migrated from the legacy type.
    if current_bow_color(host, level, pos).is_none() {
        sync_bow(host, level, pos, color);
    }
}

fn block_updates() -> BlockUpdateFlags {
    BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS
}

fn block_id(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> String {
    level
        .block(host, pos)
        .map(|block| block.id(host))
        .unwrap_or_default()
}

/// Switch both halves between regular and Giga Gap blocks.
fn set_kind(host: &mut dyn ServerHost, dimension: &str, lower: [i32; 3], giga: bool, color: u32) {
    let block = if giga { GIGA } else { GAP };
    let Some(level) = host.level(dimension) else {
        return;
    };
    let Some(old) = level.block(host, lower) else {
        return;
    };
    if old.id(host) == block || !is_gap(&old.id(host)) {
        return;
    }
    let Some(state) = host
        .block_key(block)
        .and_then(|key| host.block_state_default(key))
    else {
        return;
    };
    let up = above(lower);
    if is_gap(&block_id(host, level, up))
        && let Ok(upper) = state.with(&host.block_states(), "half", "upper")
    {
        let _ = level.set_block(host, up, upper, block_updates());
    }
    let _ = level.set_block(host, lower, state, block_updates());
    sync_bow(host, level, lower, color);
}

/// Its owner right-clicked the gap with a Yukari umbrella: it turns global, a Giga Gap, or back.
fn toggle_global(host: &mut dyn ServerHost, uuid: u128, key: &Key) {
    let result = with_store(host, |s| match s.find_mut(key) {
        Some(g) if g.owner == uuid || g.owner == 0 => {
            g.owner = uuid;
            g.global = !g.global;
            (Some((g.name.clone(), g.global, g.locked, g.color)), true)
        }
        _ => (None, false),
    })
    .flatten();
    if let Some((_, global, _, color)) = &result {
        set_kind(host, &key.0, key.1, *global, *color);
    }
    let text = match result {
        Some((name, true, true, _)) => {
            format!("{name} is a Giga Gap, but locked: unlock it to open it to everyone")
        }
        Some((name, true, false, _)) => format!("{name} is a Giga Gap: everyone can use it"),
        Some((name, false, _, _)) => {
            format!("{name} is no longer a Giga Gap: players have to find it")
        }
        None => "Only its owner can make this gap a Giga Gap".to_owned(),
    };
    action_bar(host, uuid, Text::literal(text));
}

/// Sends `player` the travel screen for the gap at `origin`.
fn show_list(host: &mut dyn ServerHost, player: &Traveler, origin: &Key) {
    let from = store::block_origin(origin.1);
    let entries = with_store(host, |s| {
        let list = s
            .destinations(player.uuid, Some(origin))
            .into_iter()
            .map(|g| travel_entry(g, from, player.creative))
            .collect::<Vec<_>>();
        (list, false)
    })
    .unwrap_or_default();
    if let Ok(mut opened) = OPENED_AT.lock() {
        opened.insert(player.uuid, origin.clone());
    }
    if let Ok(mut p) = PORTABLE.lock() {
        p.remove(&player.uuid);
    }
    send(host, player.uuid, store::LIST, store::encode_list(&entries));
}

fn travel_entry(gap: &Gap, from: [f64; 3], creative: bool) -> Entry {
    Entry {
        key: gap.key(),
        name: gap.name.clone(),
        locked: gap.locked,
        global: gap.global,
        color: gap.color,
        hunger: if creative {
            0
        } else {
            store::hunger_cost(from, gap.pos)
        },
    }
}

struct Arrival {
    at: [f64; 3],
    dimension: String,
    until: Instant,
    damage: i32,
    duration: i32,
}

static ARRIVALS: LazyLock<Mutex<HashMap<u128, Arrival>>> = LazyLock::new(Default::default);

fn travel_fx(host: &mut dyn ServerHost, dimension: &str, [x, y, z]: [f64; 3], arriving: bool) {
    let Some(level) = host.level(dimension) else {
        return;
    };
    let at = [x, y + 0.9, z];
    let (sound, pitch) = if arriving {
        ("minecraft:block.portal.travel", 1.25)
    } else {
        ("minecraft:block.portal.trigger", 0.95)
    };
    level.play_sound(host, sound, at, SoundSource::Blocks, 0.65, pitch);
    let Some(particle) = PORTAL_PARTICLES
        .get()
        .map(|keys| keys[usize::from(arriving)])
    else {
        return;
    };
    let options = ParticleOptions::simple(particle);
    level.send_particles(host, &options, None, at, [0.36, 0.65, 0.36], 0.12, 36);
}

fn finish_arrivals(host: &mut dyn ServerHost) {
    let mut ready = Vec::new();
    if let Ok(mut arrivals) = ARRIVALS.lock() {
        arrivals.retain(|uuid, trip| {
            if Instant::now() >= trip.until {
                return false;
            }
            let Some(player) = player_named(host, *uuid).filter(|p| p.health > 0.0) else {
                return false;
            };
            if player.dimension != trip.dimension
                || (0..3).any(|i| (trip.at[i] - player.position[i]).abs() > 3.0)
            {
                return true;
            }
            ready.push((
                player.entity,
                player.health,
                player.creative,
                trip.at,
                trip.dimension.clone(),
                trip.damage,
                trip.duration,
            ));
            false
        });
    }
    for (entity, health, creative, at, dimension, damage, duration) in ready {
        travel_fx(host, &dimension, at, true);
        let Some(living) = host.living(entity) else {
            continue;
        };
        if !creative && health > 1.0 {
            living.set_health(host, (health - damage as f32).max(1.0));
        }
        for id in ["minecraft:nausea", "minecraft:slowness", DIZZINESS] {
            if let Some(key) = host.effect_key(id) {
                living.add_effect(
                    host,
                    &ActiveEffect::new(key, duration, 0, false, false, false),
                );
            }
        }
    }
}

/// Takes `uuid` to the gap at `to`, if they may go there from where they are.
fn travel(host: &mut dyn ServerHost, uuid: u128, to: &Key) {
    let Some(player) = player_named(host, uuid).filter(|p| p.health > 0.0 && !p.spectator) else {
        return;
    };
    if ARRIVALS.lock().is_ok_and(|trips| trips.contains_key(&uuid)) {
        return;
    }
    let origin = OPENED_AT.lock().ok().and_then(|o| o.get(&uuid).cloned());
    let origin = origin.filter(|o| o != to && near(&player, o));
    let (source, gap) = with_store(host, |s| {
        let source = origin
            .as_ref()
            .and_then(|key| s.find(key))
            .filter(|g| g.usable_by(uuid));
        ((source.map(|g| g.pos), s.find(to).cloned()), false)
    })
    .unwrap_or_default();
    let doll = if source.is_some() {
        None
    } else {
        PORTABLE.lock().ok().and_then(|p| p.get(&uuid).copied())
    };
    let doll = doll.filter(|&id| doll_near(host, &player, id));
    let from = if let Some(pos) = source {
        store::block_origin(pos)
    } else if let Some(pos) = doll
        .and_then(|id| host.entity(id))
        .and_then(|e| e.position(host))
    {
        pos
    } else {
        return;
    };
    let Some(gap) = gap else {
        action_bar(host, uuid, Text::literal("That gap is gone"));
        return;
    };
    if !gap.usable_by(uuid) {
        let why = if gap.locked {
            "is locked"
        } else {
            "hasn't been found by you yet"
        };
        action_bar(host, uuid, Text::literal(format!("{} {why}", gap.name)));
        return;
    }
    let food = if player.creative {
        None
    } else {
        let cost = store::hunger_cost(from, gap.pos);
        let Some(food) = player.handle.food(host).filter(|food| food.level >= cost) else {
            action_bar(
                host,
                uuid,
                Text::literal(format!("Requires {cost} hunger points.")),
            );
            return;
        };
        Some(food.level - cost)
    };
    // In front of the gap, looking away from it.
    let [dx, _, dz] = step(&gap.facing);
    let at = [
        f64::from(gap.pos[0] + dx) + 0.5,
        f64::from(gap.pos[1]),
        f64::from(gap.pos[2] + dz) + 0.5,
    ];
    if !player
        .entity
        .teleport_to_level(host, &gap.dimension, at, Some([yaw_of(&gap.facing), 0.0]))
    {
        action_bar(
            host,
            uuid,
            Text::literal(format!("Could not reach {}", gap.name)),
        );
        return;
    }
    if let Some(food) = food {
        player.handle.set_food_level(host, food);
    }
    if let Ok(mut opened) = OPENED_AT.lock() {
        opened.remove(&uuid);
    }
    if let Some(id) = doll {
        PORTABLE.lock().ok().map(|mut p| p.remove(&uuid));
        let beside = [at[0] + f64::from(dx), at[1], at[2] + f64::from(dz)];
        if let Some(entity) = host.entity(id) {
            entity.teleport_to_level(host, &gap.dimension, beside, None);
        }
    }
    travel_fx(host, &player.dimension, player.position, false);
    let mut duration = 400;
    let mut damage = 6;
    if let Some(level) = host.level(&player.dimension)
        && let Some(mut random) = level.random(host)
    {
        duration = 20 * random.next_int_between_inclusive(15, 30).unwrap_or(20);
        damage = random.next_int_between_inclusive(3, 10).unwrap_or(6);
        level.set_random(host, random);
    }
    if let Ok(mut arrivals) = ARRIVALS.lock() {
        arrivals.insert(
            uuid,
            Arrival {
                at,
                dimension: gap.dimension,
                until: Instant::now() + Duration::from_secs(10),
                damage,
                duration,
            },
        );
    }
    action_bar(host, uuid, Text::literal(gap.name));
}

/// The owner named the gap: the name (a number added when another gap has it already) and lock.
fn rename(host: &mut dyn ServerHost, uuid: u128, naming: Naming) {
    let Some(player) = player_named(host, uuid) else {
        return;
    };
    if !near(&player, &naming.key) {
        return;
    }
    let wanted = clean(&naming.name);
    let result = with_store(host, |s| {
        let Some(old) = s.find(&naming.key).map(|g| g.name.clone()) else {
            return (Err("That gap is gone"), false);
        };
        if s.find(&naming.key)
            .is_some_and(|g| g.owner != 0 && g.owner != uuid)
        {
            return (Err("Only its owner can name this gap"), false);
        }
        let name = if wanted.is_empty() {
            old
        } else {
            s.unique_name(&wanted, Some(&naming.key))
        };
        let gap = s.find_mut(&naming.key).expect("found above");
        gap.name = name.clone();
        gap.locked = naming.locked;
        gap.owner = uuid;
        (Ok(name), true)
    });
    let text = match result {
        Some(Ok(name)) if naming.locked => format!("Gap named {name}, locked to you"),
        Some(Ok(name)) => format!("Gap named {name}"),
        Some(Err(why)) => why.to_owned(),
        None => return,
    };
    action_bar(host, uuid, Text::literal(text));
}

// ---- The client's side ----

/// Whether the screen open is this mod's own.
fn ours(host: &dyn ClientHost, id: &str) -> bool {
    id == "mod" && host.mod_screen() == Some(MOD_ID)
}

fn blink() -> bool {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    ms / 500 % 2 == 0
}

struct GapRequest(&'static str);

impl IPayloadHandler for GapRequest {
    fn handle(
        &self,
        bytes: PayloadBytes,
        cx: &mut dyn IPayloadContext,
    ) -> Result<(), PayloadHandleError> {
        let (Ok(player), Ok(host)) = (cx.player(), cx.server_host()) else {
            return Ok(());
        };
        let uuid = player.uuid();
        match self.0 {
            store::NAMED => {
                if let Some(naming) = store::decode_naming(&bytes.0) {
                    rename(host, uuid, naming);
                }
            }
            store::GO => {
                if let Some(to) = store::decode_go(&bytes.0) {
                    travel(host, uuid, &to);
                }
            }
            store::SHARE => {
                if let Some(request) = store::decode_share(&bytes.0) {
                    share(host, uuid, request);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

struct GapResponse(&'static str);

impl IPayloadHandler for GapResponse {
    fn handle(
        &self,
        bytes: PayloadBytes,
        cx: &mut dyn IPayloadContext,
    ) -> Result<(), PayloadHandleError> {
        if let Ok(host) = cx.client_host() {
            open_screen(host, self.0, &bytes.0);
        }
        Ok(())
    }
}

struct PendingPlacement {
    player: u128,
    level: LevelRef,
    pos: [i32; 3],
    facing: String,
    name: Option<String>,
}

static PORTAL_PARTICLES: std::sync::OnceLock<[mod_api::ParticleKey; 2]> =
    std::sync::OnceLock::new();

static PLACEMENTS: Mutex<Vec<PendingPlacement>> = Mutex::new(Vec::new());

fn finish_placements(host: &mut dyn ServerHost, active: LevelRef) {
    let placements: Vec<_> = PLACEMENTS
        .lock()
        .map(|mut queue| {
            let (ready, pending) = std::mem::take(&mut *queue)
                .into_iter()
                .partition(|placement| placement.level == active);
            *queue = pending;
            ready
        })
        .unwrap_or_default();
    for placement in placements {
        let PendingPlacement {
            player,
            level,
            pos,
            facing,
            name: given,
        } = placement;
        if !is_gap(&block_id(host, level, pos)) {
            continue;
        }
        let up = above(pos);
        let Some(state) = level.block_state(host, pos) else {
            continue;
        };
        let Ok(upper) = state.with(&host.block_states(), "half", "upper") else {
            continue;
        };
        if is_air(&block_id(host, level, up)) {
            let _ = level.set_block(host, up, upper, block_updates());
        }
        let key = (level.dimension_id().to_owned(), pos);
        let gap = with_store(host, |s| {
            s.gaps.retain(|g| g.key() != key);
            let name = match &given {
                Some(name) => s.unique_name(name, None),
                None => new_name(pos, s),
            };
            let gap = Gap {
                dimension: key.0.clone(),
                pos: key.1,
                facing,
                name,
                owner: player,
                locked: false,
                global: false,
                color: store::DEFAULT_BOW,
                known: Vec::new(),
                shared: Vec::new(),
            };
            s.gaps.push(gap.clone());
            (gap, true)
        });
        if let Some(gap) = gap {
            ask_name(host, player, &gap);
        }
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, cx: &RegistrationContext) {
        let kind = r.block_entity_type(GAP_ENTITY).blocks(&[GAP, GIGA]).build();
        if let Err(error) = r.block_entity_part(&kind, GapBlock) {
            cx.refuse(&format!("cannot attach gap block behavior: {error:?}"));
            return;
        }
        let _ = PORTAL_PARTICLES.set([
            r.particle_key("minecraft:portal"),
            r.particle_key("minecraft:reverse_portal"),
        ]);
        r.payload_handlers()
            .registrar("1")
            .play_to_server(
                PayloadId(store::NAMED.into()),
                Arc::new(GapRequest(store::NAMED)),
            )
            .play_to_server(PayloadId(store::GO.into()), Arc::new(GapRequest(store::GO)))
            .play_to_server(
                PayloadId(store::SHARE.into()),
                Arc::new(GapRequest(store::SHARE)),
            )
            .play_to_client(
                PayloadId(store::NAME.into()),
                Some(Arc::new(GapResponse(store::NAME))),
            )
            .play_to_client(
                PayloadId(store::LIST.into()),
                Some(Arc::new(GapResponse(store::LIST))),
            );
    }

    fn entity_place(&self, host: &mut dyn ServerHost, event: &mut EntityPlaceEvent) -> Flow {
        if event.state.block().id(host) != GAP {
            return Flow::Continue;
        }
        let Some(player) = host.players().acting_player() else {
            return Flow::Continue;
        };
        let entity = player.entity();
        if event.placer != Some(entity) {
            return Flow::Continue;
        }
        let Some(level) = player.level(host) else {
            return Flow::Continue;
        };
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        if !is_air(&block_id(host, level, above(pos))) {
            action_bar(
                host,
                player.uuid(),
                Text::literal("There is no room for a gap here: it needs two blocks"),
            );
            return Flow::Cancel;
        }
        let facing = facing_towards(entity.rotation(host).unwrap_or_default()[0]).to_owned();
        let name = [Hand::Main, Hand::Off].into_iter().find_map(|hand| {
            let stack = ItemPlace::PlayerHand {
                player: player.uuid(),
                hand,
            }
            .stack(host)?;
            if stack.item().id(host) != GAP {
                return None;
            }
            let name = stack
                .components()?
                .as_nbt()
                .get(&"minecraft:custom_name".into())?;
            let Tag::String(name) = name else { return None };
            let name = clean(&name.to_string_lossy());
            (!name.is_empty()).then_some(name)
        });
        if let Ok(mut queue) = PLACEMENTS.lock() {
            queue.push(PendingPlacement {
                player: player.uuid(),
                level,
                pos,
                facing,
                name,
            });
        }
        Flow::Continue
    }

    /// A broken gap is gone for everyone. A locked one only its owner can break.
    fn break_block(&self, host: &mut dyn ServerHost, event: &mut BreakBlockEvent<'_>) -> Flow {
        if !is_gap(&event.state.block().id(host)) {
            return Flow::Continue;
        }
        let Some(player) = host
            .players()
            .acting_player()
            .and_then(|p| Traveler::read(host, p))
        else {
            return Flow::Continue;
        };
        let Some(level) = player.handle.level(host) else {
            return Flow::Continue;
        };
        let upper = event
            .state
            .property(&host.block_states(), "half")
            .ok()
            .flatten()
            == Some("upper");
        let lower = if upper { below(event.pos) } else { event.pos };
        let key = (player.dimension.clone(), lower);
        let gone = with_store(host, |s| match s.find(&key) {
            Some(g) if !g.breakable_by(player.uuid) => (Err(g.name.clone()), false),
            Some(g) => {
                let name = g.name.clone();
                s.gaps.retain(|g| g.key() != key);
                (Ok(Some(name)), true)
            }
            None => (Ok(None), false),
        });
        match gone {
            Some(Err(name)) => {
                action_bar(
                    host,
                    player.uuid,
                    Text::literal(format!("{name} is locked: only its owner can break it")),
                );
                Flow::Cancel
            }
            gone => {
                // The gap drops itself, whatever it was mined with: the block asks for a pickaxe,
                // and its loot is only rolled for the right tool, so the item is made here and the
                // halves broken with what they'd drop collected, not popped (the assets mod stays
                // as servers know it). In creative nothing drops, as with any block.
                let gone = gone.and_then(Result::ok).flatten();
                let creative = player.creative;
                // The player breaks the upper half, which drops nothing of its own; the lower one,
                // which the loot table drops the gap for, is broken here. Should the lower half
                // itself be the one mined, both are broken here.
                for pos in [above(lower), lower] {
                    if is_gap(&block_id(host, level, pos)) {
                        level.remove_block(host, pos, false);
                    }
                }
                if !creative {
                    let at = [
                        f64::from(lower[0]) + 0.5,
                        f64::from(lower[1]) + 0.5,
                        f64::from(lower[2]) + 0.5,
                    ];
                    // A Giga Gap drops the Yukari Gap it was made from.
                    if let Some(item) = host.item_key(&mod_api::Identifier::parse(GAP).unwrap()) {
                        let _ =
                            level.spawn_item(host, at[0], at[1], at[2], ItemStack::of(item), 10);
                    }
                }
                if let Some(name) = gone {
                    action_bar(
                        host,
                        player.uuid,
                        Text::literal(format!("Gap closed: {name}")),
                    );
                }
                Flow::Cancel
            }
        }
    }

    /// Right-click a gap to find it and choose where to go. Its owner, sneaking, renames it,
    /// locks it or shares it; with a Yukari umbrella, makes it global or not.
    /// Its owner clicked their Yukari Doll with an empty hand: the travel screen, from her.
    fn entity_interact(
        &self,
        host: HostSide<'_>,
        event: &mut EntityInteractEvent<'_>,
    ) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        if !DOLL_KINDS.iter().any(|k| *k == event.kind) || event.stack.is_some() {
            return None;
        }
        let Ok(id) = u32::try_from(event.entity) else {
            return None;
        };
        let Some(e) = host.entity(id) else {
            return None;
        };
        if e.name(host).as_deref() != Some(YUKARI_DOLL) {
            return None;
        }
        let Some(player) = host
            .players()
            .acting_player()
            .and_then(|p| Traveler::read(host, p))
        else {
            return None;
        };
        if player.sneaking
            || !e
                .tags(host)
                .is_some_and(|t| t.contains(&owner_tag(player.uuid)))
        {
            return None;
        }
        if fresh_click(player.uuid) {
            show_portable(host, &player, id);
        }
        Some(InteractionResult::Consume)
    }

    fn right_click_block(&self, host: HostSide<'_>, event: &mut RightClickBlockEvent<'_>) -> Flow {
        let HostSide::Server(host) = host else {
            return Flow::Continue;
        };
        let Some(player) = player_named(host, event.click.player) else {
            return Flow::Continue;
        };
        let Some(level) = player.handle.level(host) else {
            return Flow::Continue;
        };
        if !is_gap(&block_id(host, level, event.click.pos)) {
            return Flow::Continue;
        }
        *event.cancellation_result = InteractionResult::Consume;
        if !fresh_click(player.uuid) {
            return Flow::Cancel;
        }
        let key = (
            player.dimension.clone(),
            lower_half(host, level, event.click.pos),
        );
        let facing = facing_towards(player.yaw).to_owned();
        let held = event.click.stack.map(|stack| stack.item().id(host));
        let umbrella = held.as_deref() == Some(UMBRELLA);
        let held_dye = held.as_deref().and_then(dye);
        let saved_color = current_bow_color(host, level, key.1);
        let found = with_store(host, |s| {
            // A gap set with a command is taken in as it is found, as its finder's.
            if s.find(&key).is_none() {
                let name = new_name(key.1, s);
                let gap = Gap {
                    dimension: key.0.clone(),
                    pos: key.1,
                    facing,
                    name,
                    owner: player.uuid,
                    locked: false,
                    global: false,
                    color: saved_color.unwrap_or(store::DEFAULT_BOW),
                    known: Vec::new(),
                    shared: Vec::new(),
                };
                s.gaps.push(gap);
                return (s.find(&key).cloned().map(|g| (g, false)), true);
            }
            // Touching it is finding it.
            let new = s.find_mut(&key).is_some_and(|g| g.discover(player.uuid));
            (s.find(&key).cloned().map(|g| (g, new)), new)
        })
        .flatten();
        let Some((gap, newly_found)) = found else {
            return Flow::Cancel;
        };
        // A gap made global before Giga Gaps were is made one now.
        set_kind(host, &key.0, key.1, gap.global, gap.color);
        restore_gap_entity(host, level, key.1, gap.color);
        let owner = gap.owner == player.uuid || gap.owner == 0;
        if let (Some(stack), Some(color)) = (event.click.stack, held_dye) {
            if owner {
                recolor(host, &key, &player, event.click.hand, stack, color);
            } else {
                action_bar(
                    host,
                    player.uuid,
                    Text::literal("Only the owner can dye this gap."),
                );
            }
        } else if umbrella && owner {
            if let Some(stack) = event.click.stack
                && take_one(host, player.uuid, event.click.hand, stack)
            {
                toggle_global(host, player.uuid, &key);
            }
        } else if player.sneaking && owner {
            ask_name(host, player.uuid, &gap);
        } else if gap.locked && !gap.usable_by(player.uuid) {
            action_bar(
                host,
                player.uuid,
                Text::literal(format!("{} is locked by its owner", gap.name)),
            );
        } else {
            if newly_found {
                action_bar(
                    host,
                    player.uuid,
                    Text::literal(format!("Found a gap: {}", gap.name)),
                );
            }
            player.handle.play_sound(
                host,
                "minecraft:block.amethyst_block.resonate",
                SoundSource::Master,
                1.0,
                1.0,
            );
            show_list(host, &player, &key);
        }
        Flow::Cancel
    }

    /// Sends what the screens chose, closes them after, and forgets a screen Escape closed.
    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        let out = ui::OUTBOX
            .lock()
            .map(|mut o| std::mem::take(&mut *o))
            .unwrap_or_default();
        for (channel, data) in out.messages {
            let _ = host.send_payload_to_server(&PayloadId(channel.into()), PayloadBytes(data));
        }
        if out.close {
            host.mod_screen_close(MOD_ID);
        }
        if host.mod_screen() != Some(MOD_ID)
            && let Ok(mut ui) = UI.lock()
        {
            *ui = Ui::Closed;
        }
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if ours(host, event.screen.id) {
            ui::draw(
                host,
                event.draw,
                event.screen.rect,
                event.screen.mouse,
                blink(),
            );
        }
    }

    fn screen_char_typed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenCharTypedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        ui::typed(event.c);
        Flow::Cancel
    }

    /// Every key but Escape is the screen's: the inventory key doesn't close it while typing.
    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) || event.key == "key.keyboard.escape" {
            return Flow::Continue;
        }
        ui::key(event.key);
        Flow::Cancel
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        ui::pressed(event.screen.rect, event.screen.mouse, event.button);
        Flow::Cancel
    }

    fn screen_mouse_button_released_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonReleasedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        ui::released();
        Flow::Cancel
    }

    fn screen_mouse_dragged_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseDraggedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        ui::dragged(event.screen.rect, event.screen.mouse);
        Flow::Cancel
    }

    fn screen_mouse_scrolled_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseScrolledPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        ui::scrolled(event.screen.rect, event.scroll[1]);
        Flow::Cancel
    }

    /// Every gap has a block entity, which draws it.
    fn level_tick_post(&self, host: HostSide<'_>, event: &mut mod_api::LevelTickPostEvent) {
        if let HostSide::Server(host) = host {
            finish_placements(host, event.level);
        }
    }

    fn chunk_load(&self, host: &mut dyn ServerHost, event: &mut mod_api::ChunkLoadEvent<'_>) {
        if event.new {
            return;
        }
        let Some(level) = host.level(event.dimension) else {
            return;
        };
        let stored = with_store(host, |store| {
            let gaps: Vec<_> = store
                .gaps
                .iter()
                .filter(|gap| {
                    gap.dimension == event.dimension
                        && [gap.pos[0] >> 4, gap.pos[2] >> 4] == event.chunk
                })
                .map(|gap| (gap.pos, gap.color))
                .collect();
            (gaps, false)
        })
        .unwrap_or_default();
        for (pos, color) in stored {
            restore_gap_entity(host, level, pos, color);
        }
    }

    fn server_stopped(&self, _: &mut dyn ServerHost, _: &mut mod_api::ServerStoppedEvent) {
        if let Ok(mut placements) = PLACEMENTS.lock() {
            placements.clear();
        }
        if let Ok(mut arrivals) = ARRIVALS.lock() {
            arrivals.clear();
        }
        if let Ok(mut opened) = OPENED_AT.lock() {
            opened.clear();
        }
        if let Ok(mut portable) = PORTABLE.lock() {
            portable.clear();
        }
        if let Ok(mut store) = STORE.lock() {
            *store = None;
        }
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if matches!(
            event.phase,
            ServerTickPhase::ServerTickEnd {
                world_running: true
            }
        ) {
            if ARRIVALS.lock().is_ok_and(|arrivals| !arrivals.is_empty()) {
                finish_arrivals(host);
            }
        }
    }

    fn camera_shake(&self, host: &dyn ClientHost, event: &mut mod_api::CameraShakeEvent<'_>) {
        let dizzy = host
            .local_player()
            .map(PlayerRef::entity)
            .and_then(|e| host.living(e))
            .and_then(|living| living.effect(host, host.effect_key(DIZZINESS)?))
            .is_some();
        if dizzy {
            let seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0.0, |time| time.as_secs_f64());
            event.shake.roll += (seconds * 2.4).sin() as f32 * 2.5;
        }
    }
}

struct GapBlock;

impl BlockPart for GapBlock {
    fn block_subscribed(&self) -> &'static [Option<HookId>] {
        &[]
    }
}

impl BlockEntityPart for GapBlock {
    fn block_entity_subscribed(&self) -> &'static [Option<HookId>] {
        &[
            Some(HookId::BlockEntityUpdateTag),
            Some(HookId::BlockEntitySubmit),
            Some(HookId::BlockEntityRenderBoundingBox),
        ]
    }

    fn block_entity_update_tag(
        &self,
        event: &mut BlockEntityUpdateTagEvent<'_>,
    ) -> Option<Compound> {
        Some(
            [(
                BOW_COLOR.into(),
                Tag::Int(bow_color(&event.entity.nbt) as i32),
            )]
            .into(),
        )
    }

    /// The gap's picture, above the block, turned on its upright axis to face the camera
    /// wherever it is, with a bow swinging at each tip, as pipef0x's Yukari Gap Demo draws it.
    fn block_entity_submit(&self, host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
        if !is_gap(event.block)
            || event
                .state
                .property(&host.block_states(), "half")
                .ok()
                .flatten()
                == Some("upper")
        {
            return;
        }
        let Some(camera) = host.camera() else { return };
        let dx = camera.position[0] - (f64::from(event.pos[0]) + 0.5);
        let dz = camera.position[2] - (f64::from(event.pos[2]) + 0.5);
        let sprite = gap_sprite(event.block);
        let size = gap_size(event.block);
        let lift = gap_lift(size);
        let draw = &mut *event.draw;
        let swing = bow_swing(draw.game_time() as f64 + f64::from(draw.partial()));
        let light = draw.light();
        draw.set_light(FULL_BRIGHT);
        draw.push();
        draw.rotate_about([0.0, 1.0, 0.0], facing_yaw(dx, dz), [0.5, 0.0, 0.5]);
        let (left, right) = (0.5 - size / 2.0, 0.5 + size / 2.0);
        let (bottom, top) = (lift, lift + size);
        // The front, towards the camera; and the back, a hair behind it and the right way round
        // from there, should the turn ever leave the camera behind it.
        let z = 0.5;
        let front = [
            [left, top, z],
            [left, bottom, z],
            [right, bottom, z],
            [right, top, z],
        ];
        draw.quad(sprite, front, [0.0, 0.0, 16.0, 16.0], 0xFF_FF_FF);
        let z = 0.5 - 0.002;
        let back = [
            [right, top, z],
            [right, bottom, z],
            [left, bottom, z],
            [left, top, z],
        ];
        draw.quad(sprite, back, [16.0, 0.0, 0.0, 16.0], 0xFF_FF_FF);
        // The bows, a little in front of the gap: the top one swings one way as the bottom one
        // swings the other.
        let inset = size * GAP_TIP;
        let bow = size * BOW_SHARE;
        let color = bow_color(&event.entity.nbt);
        draw_bow(draw, [left + inset, top - inset], bow, swing, color);
        draw_bow(draw, [right - inset, bottom + inset], bow, -swing, color);
        draw.pop();
        draw.set_light(light);
    }

    /// The tilted picture reaches past the block's usual margin: it is drawn while any of it is in view.
    fn block_entity_render_bounding_box(
        &self,
        _: &dyn ClientHost,
        event: &mut BlockEntityRenderBoundingBoxEvent<'_>,
    ) {
        if !is_gap(event.block) {
            return;
        }
        // Its corners, turned any way about the middle, stay within half the diagonal of it; and
        // the bows, centred on the tips, reach half a bow past those.
        let size = gap_size(event.block);
        let reach = size * std::f32::consts::FRAC_1_SQRT_2 + size * BOW_SHARE * 0.5;
        let middle = gap_lift(size) + size / 2.0;
        *event.bounds = [
            0.5 - reach,
            middle - reach,
            0.5 - reach,
            0.5 + reach,
            middle + reach,
            0.5 + reach,
        ];
    }
}

/// Open the screen selected by the server.
fn open_screen(host: &mut dyn ClientHost, channel: &str, data: &[u8]) {
    let next = match channel {
        store::NAME => match store::decode_naming(data) {
            Some(n) => Ui::Naming {
                key: n.key,
                current: n.name,
                text: String::new(),
                locked: n.locked,
                global: n.global,
                players: n.players,
                sharing: false,
                first_row: 0,
            },
            None => return,
        },
        store::LIST => Ui::List {
            entries: store::decode_list(data),
            first_row: 0,
            dragging: false,
        },
        _ => return,
    };
    if let Ok(mut ui) = UI.lock() {
        *ui = next;
    }
    if host.mod_screen() != Some(MOD_ID) && !host.mod_screen_open(MOD_ID) {
        if let Ok(mut ui) = UI.lock() {
            *ui = Ui::Closed;
        }
    }
}

/// How far the top bow is turned at `ticks` (game time plus partial tick), in degrees: there and
/// back across `BOW_SWING` each way every `BOW_PERIOD` ticks, eased in and out cubically.
fn bow_swing(ticks: f64) -> f32 {
    let cycle = (ticks / BOW_PERIOD).rem_euclid(1.0);
    let ramp = if cycle < 0.5 {
        cycle * 2.0
    } else {
        2.0 - cycle * 2.0
    };
    let eased = if ramp < 0.5 {
        4.0 * ramp * ramp * ramp
    } else {
        1.0 - (-2.0 * ramp + 2.0).powi(3) / 2.0
    };
    (-1.0 + 2.0 * eased) as f32 * BOW_SWING
}

/// A bow `size` blocks across centred at `at` (x, y) on the gap's plane, a hair in front of it and
/// turned `degrees` about its middle; its back too, a hair behind the gap's back.
fn draw_bow(
    draw: &mut dyn BlockEntityDrawContext,
    at: [f32; 2],
    size: f32,
    degrees: f32,
    color: u32,
) {
    let h = size / 2.0;
    let [x, y] = at;
    draw.push();
    draw.rotate_about([0.0, 0.0, 1.0], degrees, [x, y, 0.5]);
    let z = 0.5 + 0.005;
    let front = [
        [x - h, y + h, z],
        [x - h, y - h, z],
        [x + h, y - h, z],
        [x + h, y + h, z],
    ];
    draw.quad(BOW_SPRITE, front, [0.0, 0.0, 16.0, 16.0], color);
    let z = 0.5 - 0.007;
    let back = [
        [x + h, y + h, z],
        [x + h, y - h, z],
        [x - h, y - h, z],
        [x - h, y + h, z],
    ];
    draw.quad(BOW_SPRITE, back, [16.0, 0.0, 0.0, 16.0], color);
    draw.pop();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bows_swing_there_and_back() {
        assert_eq!(bow_swing(0.0), -BOW_SWING);
        assert!((bow_swing(BOW_PERIOD / 2.0) - BOW_SWING).abs() < 1e-4);
        assert!(bow_swing(BOW_PERIOD / 4.0).abs() < 1e-4);
        assert_eq!(bow_swing(BOW_PERIOD), bow_swing(0.0));
        assert_eq!(bow_swing(-BOW_PERIOD / 2.0), bow_swing(BOW_PERIOD / 2.0));
    }

    #[test]
    fn the_giga_gap_is_a_gap_with_its_own_picture() {
        assert!(is_gap(GAP) && is_gap(GIGA));
        assert!(!is_gap("minecraft:glass"));
        assert_eq!(gap_sprite(GAP), GAP_SPRITE);
        assert_eq!(gap_sprite(GIGA), GIGA_SPRITE);
    }

    #[test]
    fn the_lower_tip_sits_on_the_block() {
        assert!((gap_lift(GAP_SIZE) + GAP_SIZE * GAP_TIP - 1.0).abs() < 1e-6);
        // The bigger Giga Gap's lower tip sits on the block too.
        let alt = gap_size(GIGA);
        assert!((gap_lift(alt) + alt * GAP_TIP - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_yukari_doll_is_found_as_alice_dolls_own_mob() {
        assert!(DOLL_KINDS.contains(&"alice_dolls:doll"));
        assert!(DOLL_KINDS.contains(&"minecraft:chicken"));
    }

    #[test]
    fn yukaris_tag_is_the_one_alice_dolls_gives_her() {
        assert_eq!(
            owner_tag(0xab),
            "alice_dolls.owner.000000000000000000000000000000ab"
        );
    }

    #[test]
    fn the_giga_gap_is_a_quarter_bigger() {
        assert_eq!(gap_size(GAP), GAP_SIZE);
        assert!((gap_size(GIGA) - GAP_SIZE * 1.25).abs() < 1e-6);
    }

    fn gap(name: &str, x: i32) -> Gap {
        Gap {
            dimension: "minecraft:overworld".into(),
            pos: [x, 64, 0],
            facing: "east".into(),
            name: name.into(),
            owner: 1,
            locked: false,
            global: false,
            color: store::DEFAULT_BOW,
            known: Vec::new(),
            shared: Vec::new(),
        }
    }

    #[test]
    fn made_up_names_are_never_taken_twice() {
        let mut store = Store::default();
        for i in 0..500 {
            let name = new_name([0, 64, 0], &store);
            assert!(
                !store
                    .gaps
                    .iter()
                    .any(|g| g.name.eq_ignore_ascii_case(&name)),
                "{name}"
            );
            store.gaps.push(gap(&name, i));
        }
    }

    #[test]
    fn yukari_comes_through_only_by_touhou_mobs_roll() {
        // This mod summons no Yukari itself: Touhou Mobs rolls once in 2,000 on gap travel.
        let src = include_str!("lib.rs");
        let summons = src
            .matches(&format!("host.summon(&gap.dimension, {}", "YUKARI"))
            .count();
        assert_eq!(summons, 0);
        assert!(!src.contains(&format!("const {}: &str = \"thmobs:yukari\"", "YUKARI")));
    }

    #[test]
    fn a_gap_faces_whoever_placed_it() {
        assert_eq!(facing_towards(0.0), "north"); // looking south
        assert_eq!(facing_towards(90.0), "east"); // looking west
        assert_eq!(facing_towards(180.0), "south");
        assert_eq!(facing_towards(-90.0), "west"); // looking east
    }

    #[test]
    fn the_gap_turns_to_face_the_camera() {
        assert_eq!(facing_yaw(0.0, 1.0), 0.0);
        assert_eq!(facing_yaw(1.0, 0.0), 90.0);
        assert_eq!(facing_yaw(0.0, -1.0), 180.0);
        assert_eq!(facing_yaw(-1.0, 0.0), -90.0);
    }
}
