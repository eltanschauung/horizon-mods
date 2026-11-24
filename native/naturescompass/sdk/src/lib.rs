//! Nature's Compass for Horizon: a port of Nature's Compass by ChaosTheDude (CC BY-NC-SA 4.0).
//!
//! Right-click the compass for the biome screen: every biome of the world, vanilla's and the mods',
//! in a list that scrolls, with a search field and sorting by name or by mod. Pick one and the
//! server looks for the nearest of it from where you stand (the game's own biome search, as
//! `/locate biome`). The compass then points at it, as a lodestone compass points at its
//! lodestone, and while you hold it the line over the hotbar gives the biome, its coordinates, how
//! far it is and which way to turn. Without the screen (a client that can't open it) a chest menu
//! of the biomes, paged, does the same.

mod ui;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use mod_api::*;

use ui::{COMPASS, MOD_ID, UI, Ui};

#[derive(Default)]
struct Mod;
const TRACKER: &str = "minecraft:lodestone_tracker";
const CUSTOM: &str = "minecraft:custom_data";

/// Biomes a player can't find from the Overworld are still listed: `/locate biome` looks in the
/// player's own dimension, so a Nether biome is found from the Nether.
const VANILLA: &[&str] = &[
    "badlands",
    "bamboo_jungle",
    "beach",
    "birch_forest",
    "cherry_grove",
    "cold_ocean",
    "dappled_forest",
    "dark_forest",
    "deep_cold_ocean",
    "deep_dark",
    "deep_frozen_ocean",
    "deep_lukewarm_ocean",
    "deep_ocean",
    "desert",
    "dripstone_caves",
    "eroded_badlands",
    "flower_forest",
    "forest",
    "frozen_ocean",
    "frozen_peaks",
    "frozen_river",
    "grove",
    "ice_spikes",
    "jagged_peaks",
    "jungle",
    "lukewarm_ocean",
    "lush_caves",
    "mangrove_swamp",
    "meadow",
    "mushroom_fields",
    "ocean",
    "old_growth_birch_forest",
    "old_growth_pine_taiga",
    "old_growth_spruce_taiga",
    "pale_garden",
    "plains",
    "river",
    "savanna",
    "savanna_plateau",
    "snowy_beach",
    "snowy_plains",
    "snowy_slopes",
    "snowy_taiga",
    "sparse_jungle",
    "stony_peaks",
    "stony_shore",
    "sulfur_caves",
    "sunflower_plains",
    "swamp",
    "taiga",
    "warm_ocean",
    "windswept_forest",
    "windswept_gravelly_hills",
    "windswept_hills",
    "windswept_savanna",
    "wooded_badlands",
    "basalt_deltas",
    "crimson_forest",
    "nether_wastes",
    "soul_sand_valley",
    "warped_forest",
    "end_barrens",
    "end_highlands",
    "end_midlands",
    "small_end_islands",
    "the_end",
];

// ---- Biome names and icons ----

/// `minecraft:old_growth_pine_taiga` -> "Old Growth Pine Taiga".
fn pretty(id: &str) -> String {
    let path = id.split(':').nth(1).unwrap_or(id);
    path.split(['_', '/'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The mod a biome comes from, by its namespace: `biomesoplenty:crag` -> "Biomes O' Plenty".
fn source(id: &str) -> String {
    match id.split(':').next().unwrap_or("") {
        "minecraft" => "Minecraft".to_owned(),
        "biomesoplenty" => "Biomes O' Plenty".to_owned(),
        "terralith" => "Terralith".to_owned(),
        "byg" => "Oh The Biomes You'll Go".to_owned(),
        "regions_unexplored" => "Regions Unexplored".to_owned(),
        ns => pretty(&format!("x:{ns}")),
    }
}

/// An item that looks like the biome, by the words in its name.
fn icon(id: &str) -> &'static str {
    let p = id.split(':').nth(1).unwrap_or(id);
    let has = |w: &str| p.contains(w);
    if has("warm_ocean") || has("coral") {
        return "minecraft:brain_coral_block";
    }
    if has("frozen_ocean") || has("frozen_river") || has("ice") || has("glacier") {
        return "minecraft:packed_ice";
    }
    if has("ocean") || has("sea") {
        return "minecraft:prismarine";
    }
    if has("river") || has("lake") || has("pond") {
        return "minecraft:water_bucket";
    }
    if has("beach") || has("shore") || has("dune") {
        return "minecraft:sand";
    }
    if has("mushroom") || has("fungal") {
        return "minecraft:red_mushroom_block";
    }
    if has("crimson") {
        return "minecraft:crimson_nylium";
    }
    if has("warped") {
        return "minecraft:warped_nylium";
    }
    if has("basalt") {
        return "minecraft:basalt";
    }
    if has("soul") {
        return "minecraft:soul_sand";
    }
    if has("nether") {
        return "minecraft:netherrack";
    }
    if has("end") && !has("tender") {
        return "minecraft:end_stone";
    }
    if has("void") {
        return "minecraft:black_concrete";
    }
    if has("deep_dark") || has("sculk") {
        return "minecraft:sculk";
    }
    if has("lush") {
        return "minecraft:moss_block";
    }
    if has("dripstone") {
        return "minecraft:pointed_dripstone";
    }
    if has("sulfur") {
        return "minecraft:yellow_concrete_powder";
    }
    if has("cave") {
        return "minecraft:stone";
    }
    if has("badlands") || has("mesa") || has("canyon") {
        return "minecraft:red_sand";
    }
    if has("desert") || has("dry") || has("waste") {
        return "minecraft:cactus";
    }
    if has("savanna") || has("steppe") || has("outback") {
        return "minecraft:acacia_sapling";
    }
    if has("jungle") || has("rainforest") || has("tropic") {
        return "minecraft:jungle_sapling";
    }
    if has("bamboo") {
        return "minecraft:bamboo";
    }
    if has("mangrove") {
        return "minecraft:mangrove_propagule";
    }
    if has("swamp") || has("bog") || has("marsh") || has("wetland") || has("bayou") || has("fen") {
        return "minecraft:lily_pad";
    }
    if has("cherry") || has("sakura") || has("blossom") {
        return "minecraft:cherry_sapling";
    }
    if has("pale") {
        return "minecraft:pale_oak_sapling";
    }
    if has("dark") || has("dead") || has("spooky") {
        return "minecraft:dark_oak_sapling";
    }
    if has("birch") || has("aspen") {
        return "minecraft:birch_sapling";
    }
    if has("snow") || has("frozen") || has("tundra") || has("cold") || has("alp") {
        return "minecraft:snow_block";
    }
    if has("peak")
        || has("slope")
        || has("mountain")
        || has("crag")
        || has("stony")
        || has("highland")
    {
        return "minecraft:stone";
    }
    if has("windswept") || has("hill") || has("gravel") {
        return "minecraft:gravel";
    }
    if has("taiga")
        || has("pine")
        || has("spruce")
        || has("conifer")
        || has("grove")
        || has("boreal")
    {
        return "minecraft:spruce_sapling";
    }
    if has("flower") || has("lavender") || has("field") || has("orchard") || has("meadow") {
        return "minecraft:poppy";
    }
    if has("forest") || has("wood") || has("thicket") {
        return "minecraft:oak_sapling";
    }
    if has("plain") || has("prairie") || has("grass") || has("pasture") || has("shrub") {
        return "minecraft:short_grass";
    }
    "minecraft:grass_block"
}

/// Every biome the world has, in name order: what the game names by id, else vanilla's.
fn biome_list(host: &dyn Host) -> Vec<String> {
    let mut list: Vec<String> = host
        .registry_entries("minecraft:worldgen/biome")
        .map(|ids| ids.into_iter().map(|id| id.to_string()).collect())
        .unwrap_or_else(|| VANILLA.iter().map(|b| format!("minecraft:{b}")).collect());
    list.retain(|b| b != "minecraft:the_void");
    list.sort_by_key(|b| pretty(b).to_lowercase());
    list.dedup();
    list
}

// ---- The menu ----

const PER_PAGE: usize = 45;
const PREV: usize = 45;
const INFO: usize = 49;
const NEXT: usize = 53;

fn key(host: &dyn Host, item: &str) -> Option<ItemKey> {
    host.item_key(&Identifier::parse_namespaced(item)?)
}

fn named(host: &dyn Host, item: &str, name: String) -> Option<ItemStack> {
    let mut stack = ItemStack::of(key(host, item)?);
    let Tag::Compound(patch) =
        Tag::compound([("minecraft:custom_name", Tag::String(name.as_str().into()))])
    else {
        return None;
    };
    stack.apply_components(&patch).ok()?;
    Some(stack)
}

fn page_items(host: &dyn Host, list: &[String], page: usize) -> Vec<Option<ItemStack>> {
    let pages = list.len().div_ceil(PER_PAGE).max(1);
    let mut items = vec![None; 54];
    for (i, b) in list.iter().skip(page * PER_PAGE).take(PER_PAGE).enumerate() {
        items[i] = named(host, icon(b), pretty(b));
    }
    if page > 0 {
        items[PREV] = named(host, "minecraft:arrow", "Previous page".into());
    }
    if page + 1 < pages {
        items[NEXT] = named(host, "minecraft:spectral_arrow", "Next page".into());
    }
    items[INFO] = named(
        host,
        COMPASS,
        format!("Select a biome  (page {} of {pages})", page + 1),
    );
    items
}

/// A player's open biome menu: its window, the biomes and the page shown.
struct Open {
    window: i32,
    list: Vec<String>,
    page: usize,
    hand: Hand,
}

#[derive(Default)]
struct Session {
    last_open: Option<Instant>,
    pending_open: Option<Hand>,
    screen: Option<Hand>,
    menu: Option<Open>,
    searching: bool,
}

static SESSIONS: LazyLock<Mutex<HashMap<u128, Session>>> = LazyLock::new(Default::default);
static PENDING: Mutex<VecDeque<(u128, Hand, String)>> = Mutex::new(VecDeque::new());
static SEARCH: Mutex<Option<Search>> = Mutex::new(None);
static RESULT: Mutex<Option<(Search, Option<Found>)>> = Mutex::new(None);

#[derive(Debug)]
pub struct Search {
    uuid: u128,
    hand: Hand,
    biome: String,
    dimension: String,
    at: [i32; 3],
}

impl Search {
    pub fn target(&self) -> (&str, &str, [i32; 3]) {
        (&self.dimension, &self.biome, self.at)
    }
}

/// Look in already-loaded chunks before consulting the generator. Saved
/// sections can still contain a mod biome after the world's generation
/// settings changed; reading them here never loads or generates a chunk.
pub fn nearby_saved_biome(
    host: &dyn ServerHost,
    level: mod_api::LevelRef,
    biome: &str,
    origin: [i32; 3],
) -> Option<[i32; 3]> {
    if level.biome_at(host, origin).as_deref() == Some(biome) {
        return Some(origin);
    }
    const RADIUS: i32 = 96;
    let [x, y, z] = origin;
    let mut nearest = None;
    let mut best = RADIUS * RADIUS + 1;
    for dx in (-RADIUS..=RADIUS).step_by(4) {
        for dz in (-RADIUS..=RADIUS).step_by(4) {
            let horizontal = dx * dx + dz * dz;
            if horizontal >= best {
                continue;
            }
            for dy in [0, -8, 8, -16, 16, -32, 32] {
                let distance = horizontal + dy * dy;
                if distance >= best {
                    break;
                }
                let at = [x.saturating_add(dx), y.saturating_add(dy), z.saturating_add(dz)];
                if level.biome_at(host, at).as_deref() == Some(biome) {
                    nearest = Some(at);
                    best = distance;
                    break;
                }
            }
        }
    }
    nearest
}

pub fn take_search() -> Option<Search> {
    SEARCH.lock().ok()?.take()
}

pub fn finish_search(search: Search, coords: Option<(i32, Option<i32>, i32)>) {
    let found = coords.map(|(x, y, z)| Found {
        biome: search.biome.clone(),
        dimension: search.dimension.clone(),
        x,
        y: y.unwrap_or(search.at[1]),
        z,
    });
    if let Ok(mut result) = RESULT.lock() {
        *result = Some((search, found));
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Found {
    biome: String,
    dimension: String,
    x: i32,
    y: i32,
    z: i32,
}


// ---- The compass's own data ----

fn tracker_tag(f: &Found) -> Tag {
    Tag::compound([
        (
            "target",
            Tag::compound([
                ("dimension", Tag::String(f.dimension.as_str().into())),
                ("pos", Tag::IntArray(vec![f.x, f.y, f.z])),
            ]),
        ),
        // Not tracked: no lodestone has to stay there.
        ("tracked", Tag::Byte(0)),
    ])
}

fn custom_tag(f: &Found) -> Tag {
    Tag::compound([(
        "naturescompass",
        Tag::compound([
            ("biome", Tag::String(f.biome.as_str().into())),
            ("dimension", Tag::String(f.dimension.as_str().into())),
            ("x", Tag::Int(f.x)),
            ("y", Tag::Int(f.y)),
            ("z", Tag::Int(f.z)),
        ]),
    )])
}

fn text_of(t: Option<&Tag>) -> Option<String> {
    match t? {
        Tag::String(s) => Some(String::from_utf16_lossy(&s.0)),
        _ => None,
    }
}

fn int_of(t: Option<&Tag>) -> Option<i32> {
    match t? {
        Tag::Int(v) => Some(*v),
        Tag::Short(v) => Some(i32::from(*v)),
        Tag::Byte(v) => Some(i32::from(*v)),
        Tag::Long(v) => i32::try_from(*v).ok(),
        _ => None,
    }
}

/// The target a compass stack holds, from its custom data.
fn found_on(stack: &ItemStack) -> Option<Found> {
    let data = stack.components()?.as_nbt().get(&CUSTOM.into())?;
    fn field<'a>(c: &'a Tag, key: &str) -> Option<&'a Tag> {
        let Tag::Compound(map) = c else { return None };
        map.iter()
            .find(|(k, _)| String::from_utf16_lossy(&k.0) == key)
            .map(|(_, v)| v)
    }
    let nc = field(data, "naturescompass")?;
    let get = |k: &str| field(nc, k);
    Some(Found {
        biome: text_of(get("biome"))?,
        dimension: text_of(get("dimension"))?,
        x: int_of(get("x"))?,
        y: int_of(get("y")).unwrap_or(64),
        z: int_of(get("z"))?,
    })
}

/// Which way to turn: an arrow for the target's bearing relative to where the player looks.
fn arrow(position: [f64; 3], yaw: f32, f: &Found) -> &'static str {
    let dx = f64::from(f.x) + 0.5 - position[0];
    let dz = f64::from(f.z) + 0.5 - position[2];
    // Minecraft's yaw: 0 looks south (+z), 90 west (-x).
    let bearing = (-dx).atan2(dz).to_degrees();
    let rel = (bearing - f64::from(yaw)).rem_euclid(360.0);
    const ARROWS: [&str; 8] = [
        "\u{2191}", "\u{2197}", "\u{2192}", "\u{2198}", "\u{2193}", "\u{2199}", "\u{2190}",
        "\u{2196}",
    ];
    ARROWS[(((rel + 22.5) / 45.0) as usize) % 8]
}

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

fn held(host: &mut dyn ServerHost, player: PlayerRef, hand: Hand) -> Option<ItemStack> {
    let stack = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand,
    }
    .stack(host)?;
    (Some(stack.item()) == key(host, COMPASS)).then_some(stack)
}

fn open(host: &mut dyn ServerHost, uuid: u128, hand: Hand) {
    let Some(player) = host.player(uuid) else {
        return;
    };
    let Some(stack) = held(host, player, hand) else {
        return;
    };
    let list = biome_list(host);
    let current = found_on(&stack).map(|f| f.biome);
    let sent = player
        .send_payload(
            host,
            &PayloadId(ui::OPEN.into()),
            PayloadBytes(ui::encode_open(&list, current.as_deref())),
        )
        .is_ok();
    let menu = if sent {
        None
    } else {
        let items = page_items(host, &list, 0);
        host.open_menu(
            uuid,
            6,
            Text::literal("Nature's Compass"),
            items,
            MenuReplace::CloseFirst,
        )
        .ok()
        .flatten()
        .map(|window| Open {
            window,
            list,
            page: 0,
            hand,
        })
    };
    if let Ok(mut sessions) = SESSIONS.lock() {
        let session = sessions.entry(uuid).or_default();
        session.screen = sent.then_some(hand);
        session.menu = menu;
    }
}

fn enqueue(host: &mut dyn ServerHost, player: PlayerRef, hand: Hand, biome: &str) {
    if biome.len() > 256
        || Identifier::parse_namespaced(biome).is_none()
        || biome == "minecraft:the_void"
        || host.biome_key(biome).is_none()
        || held(host, player, hand).is_none()
    {
        return;
    }
    if let Ok(mut sessions) = SESSIONS.lock()
        && let Some(session) = sessions.get_mut(&player.uuid())
        && !session.searching
        && let Ok(mut queue) = PENDING.lock()
    {
        session.searching = true;
        queue.push_back((player.uuid(), hand, biome.to_owned()));
    }
}

fn clear_search(uuid: u128) {
    if let Ok(mut sessions) = SESSIONS.lock()
        && let Some(session) = sessions.get_mut(&uuid)
    {
        session.searching = false;
    }
}

fn prepare_search(host: &mut dyn ServerHost) {
    let pending = PENDING.lock().ok().and_then(|mut queue| queue.pop_front());
    let Some((uuid, hand, biome)) = pending else {
        return;
    };
    let Some(player) = host.player(uuid) else {
        clear_search(uuid);
        return;
    };
    let (Some(dimension), Some(at), Some(_)) = (
        player.dimension(host),
        player.entity().block_position(host),
        held(host, player, hand),
    ) else {
        clear_search(uuid);
        return;
    };
    let _ = player.action_bar(
        host,
        Text::literal(format!("Searching for {}...", pretty(&biome))),
    );
    if let Ok(mut search) = SEARCH.lock() {
        *search = Some(Search {
            uuid,
            hand,
            biome,
            dimension: dimension.to_string(),
            at,
        });
    }
}

fn write_result(host: &mut dyn ServerHost, search: Search, found: Option<Found>) {
    let Some(player) = host.player(search.uuid) else {
        return;
    };
    if player
        .dimension(host)
        .is_none_or(|d| d.to_string() != search.dimension)
    {
        return;
    }
    let Some(found) = found else {
        let _ = player.tell(
            host,
            Text::literal(format!(
                "Nature's Compass: no {} found nearby",
                pretty(&search.biome)
            )),
        );
        return;
    };
    let selected = match search.hand {
        Hand::Main => player.selected_slot(host).map(|slot| slot as u32),
        Hand::Off => Some(40),
    };
    let Some(compass) = key(host, COMPASS) else {
        return;
    };
    let Some((slot, mut stack)) = player
        .inventory_slots(host)
        .into_iter()
        .flatten()
        .filter(|(_, stack)| stack.item() == compass)
        .min_by_key(|(slot, _)| (Some(*slot) != selected, *slot))
    else {
        return;
    };
    let Tag::Compound(patch) =
        Tag::compound([(TRACKER, tracker_tag(&found)), (CUSTOM, custom_tag(&found))])
    else {
        return;
    };
    if stack.apply_components(&patch).is_err() {
        return;
    }
    if player.set_inventory_slot(host, slot, Some(stack)).is_ok() {
        let [x, _, z] = search.at;
        let d = (f64::from(found.x) - f64::from(x)).hypot(f64::from(found.z) - f64::from(z));
        let _ = player.tell(
            host,
            Text::literal(format!(
                "Nature's Compass: {} found at {}, {} ({} blocks away)",
                pretty(&found.biome),
                found.x,
                found.z,
                d.round() as i64,
            )),
        );
    }
}

fn hud(host: &mut dyn ServerHost) {
    for player in host.players().server_players() {
        let Some(f) = held(host, player, Hand::Main)
            .or_else(|| held(host, player, Hand::Off))
            .as_ref()
            .and_then(found_on)
        else {
            continue;
        };
        let (Some(position), Some([yaw, _]), Some(dimension)) = (
            player.entity().position(host),
            player.entity().rotation(host),
            player.dimension(host),
        ) else {
            continue;
        };
        let d = (f64::from(f.x) + 0.5 - position[0]).hypot(f64::from(f.z) + 0.5 - position[2]);
        let line = if dimension.to_string() != f.dimension {
            format!("{}: in another dimension", pretty(&f.biome))
        } else if d < 8.0 {
            format!("{}: you are here ({}, {})", pretty(&f.biome), f.x, f.z)
        } else {
            format!(
                "{} {}  {}, {}  ({} blocks)",
                arrow(position, yaw, &f),
                pretty(&f.biome),
                f.x,
                f.z,
                d.round() as i64
            )
        };
        let _ = player.action_bar(host, Text::literal(line));
    }
}

struct SearchHandler;
impl IPayloadHandler for SearchHandler {
    fn handle(
        &self,
        bytes: PayloadBytes,
        ctx: &mut dyn IPayloadContext,
    ) -> Result<(), PayloadHandleError> {
        let (Ok(biome), Ok(player)) = (std::str::from_utf8(&bytes.0), ctx.player()) else {
            return Ok(());
        };
        if let Ok(host) = ctx.server_host()
            && let Some(hand) = SESSIONS
                .lock()
                .ok()
                .and_then(|mut sessions| sessions.get_mut(&player.uuid())?.screen.take())
        {
            enqueue(host, player, hand, biome);
        }
        Ok(())
    }
}

struct OpenHandler;
impl IPayloadHandler for OpenHandler {
    fn handle(
        &self,
        bytes: PayloadBytes,
        ctx: &mut dyn IPayloadContext,
    ) -> Result<(), PayloadHandleError> {
        let (Some((biomes, current)), Ok(host)) = (ui::decode_open(&bytes.0), ctx.client_host())
        else {
            return Ok(());
        };
        if let Ok(mut ui) = UI.lock() {
            *ui = Ui::Open(ui::Screen::new(biomes, current));
        }
        if host.mod_screen() != Some(MOD_ID)
            && !host.mod_screen_open(MOD_ID)
            && let Ok(mut ui) = UI.lock()
        {
            *ui = Ui::Closed;
        }
        Ok(())
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        registrar
            .payload_handlers()
            .registrar("1")
            .optional()
            .play_to_server(PayloadId(ui::SEARCH.into()), Arc::new(SearchHandler))
            .play_to_client(PayloadId(ui::OPEN.into()), Some(Arc::new(OpenHandler)));
    }

    fn right_click_item(&self, host: HostSide, event: &mut RightClickItemEvent<'_>) -> Flow {
        let HostSide::Server(host) = host else {
            return Flow::Continue;
        };
        if Some(event.click.stack.item()) != key(host, COMPASS) {
            return Flow::Continue;
        }
        if let Ok(mut sessions) = SESSIONS.lock() {
            let session = sessions.entry(event.click.player).or_default();
            let now = Instant::now();
            if !session.searching
                && session
                    .last_open
                    .is_none_or(|at| now.duration_since(at) >= Duration::from_millis(250))
            {
                session.last_open = Some(now);
                session.pending_open = Some(event.click.hand);
            }
        }
        *event.cancellation_result = InteractionResult::SuccessServer;
        Flow::Cancel
    }

    fn menu_clicked(&self, host: &mut dyn ServerHost, event: &mut MenuClickedEvent) {
        let Ok(slot) = usize::try_from(event.slot) else {
            return;
        };
        let action = SESSIONS.lock().ok().and_then(|mut sessions| {
            let menu = sessions.get_mut(&event.uuid)?.menu.as_mut()?;
            if menu.window != event.window {
                return None;
            }
            let pages = menu.list.len().div_ceil(PER_PAGE).max(1);
            let page = match slot {
                PREV if menu.page > 0 => menu.page - 1,
                NEXT if menu.page + 1 < pages => menu.page + 1,
                s if s < PER_PAGE => {
                    let biome = menu.list.get(menu.page * PER_PAGE + s)?.clone();
                    return Some(Ok((menu.hand, biome)));
                }
                _ => return None,
            };
            menu.page = page;
            Some(Err((page, menu.list.clone())))
        });
        match action {
            Some(Ok((hand, biome))) => {
                host.close_menu(event.uuid);
                if let Some(player) = host.player(event.uuid) {
                    enqueue(host, player, hand, &biome);
                }
            }
            Some(Err((page, list))) => {
                let items = page_items(host, &list, page);
                let _ = host.menu_set_items(event.uuid, event.window, items);
            }
            None => {}
        }
    }

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

    fn client_disconnecting(&self, _: &dyn ClientHost, _: &mut ClientDisconnectingEvent) {
        if let Ok(mut ui) = UI.lock() {
            *ui = Ui::Closed;
        }
        if let Ok(mut out) = ui::OUTBOX.lock() {
            *out = Default::default();
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
        ui::typed(event.screen.rect, event.c);
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
        ui::key(event.screen.rect, event.key);
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

    fn menu_closed(&self, _: &mut dyn ServerHost, event: &mut MenuClosedEvent) {
        if let Ok(mut sessions) = SESSIONS.lock()
            && let Some(session) = sessions.get_mut(&event.uuid)
            && session
                .menu
                .as_ref()
                .is_some_and(|menu| menu.window == event.window)
        {
            session.menu = None;
        }
    }

    fn player_logged_out(&self, _: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        if let Ok(mut sessions) = SESSIONS.lock() {
            sessions.remove(&event.player);
        }
        if let Ok(mut searches) = PENDING.lock() {
            searches.retain(|(uuid, _, _)| *uuid != event.player);
        }
        if let Ok(mut result) = RESULT.lock()
            && result
                .as_ref()
                .is_some_and(|(search, _)| search.uuid == event.player)
        {
            *result = None;
        }
    }

    fn server_stopping(&self, _: &mut dyn ServerHost, _: &mut ServerStoppingEvent) {
        if let Ok(mut sessions) = SESSIONS.lock() {
            sessions.clear();
        }
        if let Ok(mut pending) = PENDING.lock() {
            pending.clear();
        }
        let _ = take_search();
        if let Ok(mut result) = RESULT.lock() {
            *result = None;
        }
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if !matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
            return;
        }
        let result = RESULT.lock().ok().and_then(|mut result| result.take());
        if let Some((search, found)) = result {
            clear_search(search.uuid);
            write_result(host, search, found);
        }
        let pending = SESSIONS
            .lock()
            .map(|mut sessions| {
                sessions
                    .iter_mut()
                    .filter_map(|(&uuid, session)| {
                        session.pending_open.take().map(|hand| (uuid, hand))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for (uuid, hand) in pending {
            open(host, uuid, hand);
        }
        prepare_search(host);
        if event.context.tick.get() % 10 == 0 {
            hud(host);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_host() -> mod_api::testing::FakeHost {
        let mut host = mod_api::testing::FakeHost::new();
        host.registries
            .resolve_items(
                [
                    COMPASS,
                    "minecraft:compass",
                    "minecraft:arrow",
                    "minecraft:spectral_arrow",
                    "minecraft:grass_block",
                ]
                .map(|id| Identifier::parse_namespaced(id).unwrap()),
            )
            .unwrap();
        host
    }

    #[test]
    fn names_read_well() {
        assert_eq!(
            pretty("minecraft:old_growth_pine_taiga"),
            "Old Growth Pine Taiga"
        );
        assert_eq!(pretty("biomesoplenty:lavender_field"), "Lavender Field");
    }

    #[test]
    fn typed_search_keeps_mod_namespace_and_dimension() {
        let search = Search {
            uuid: 1,
            hand: Hand::Main,
            biome: "biomesoplenty:maple_woods".into(),
            dimension: "minecraft:overworld".into(),
            at: [-1234, 88, 5678],
        };
        assert_eq!(search.target(), ("minecraft:overworld", "biomesoplenty:maple_woods", [-1234, 88, 5678]));
    }

    #[test]
    fn biomes_name_their_mod() {
        assert_eq!(source("biomesoplenty:crag"), "Biomes O' Plenty");
        assert_eq!(source("minecraft:plains"), "Minecraft");
        assert_eq!(source("cool_mod:x"), "Cool Mod");
    }

    #[test]
    fn pages_hold_45_biomes_and_their_buttons() {
        let list: Vec<String> = (0..100).map(|i| format!("minecraft:b{i:03}")).collect();
        let host = fake_host();
        let first = page_items(&host, &list, 0);
        assert!(first[0].is_some() && first[44].is_some());
        assert!(first[PREV].is_none() && first[NEXT].is_some());
        let last = page_items(&host, &list, 2);
        assert!(last[9].is_some() && last[10].is_none());
        assert!(last[PREV].is_some() && last[NEXT].is_none());
    }

    #[test]
    fn the_found_biome_survives_on_the_stack() {
        let f = Found {
            biome: "minecraft:plains".into(),
            dimension: "minecraft:overworld".into(),
            x: 10,
            y: 70,
            z: -20,
        };
        let host = fake_host();
        let mut stack = ItemStack::of(key(&host, "minecraft:compass").unwrap());
        let Tag::Compound(patch) = Tag::compound([(CUSTOM, custom_tag(&f))]) else {
            unreachable!()
        };
        stack.apply_components(&patch).unwrap();
        assert_eq!(found_on(&stack), Some(f));
    }

    #[test]
    fn the_arrow_points_ahead_when_facing_the_target() {
        let at = [0.0, 64.0, 0.0];
        let f = Found {
            biome: "b".into(),
            dimension: "d".into(),
            x: 0,
            y: 64,
            z: 100,
        };
        assert_eq!(arrow(at, 0.0, &f), "\u{2191}"); // looking south, +z
        assert_eq!(arrow(at, 180.0, &f), "\u{2193}"); // looking north
        let east = Found {
            x: 100,
            z: 0,
            ..f.clone()
        };
        assert_eq!(arrow(at, 0.0, &east), "\u{2190}"); // east is on the left when facing south
    }
}
