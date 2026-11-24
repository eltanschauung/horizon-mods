//! The Giga Torch: Torchmaster's Mega Torch (by Xalcon, MIT), brought into this mod. It keeps
//! hostile monsters from spawning naturally within 64 blocks of it in each direction (a cube), and
//! stops phantoms, the hostile mobs of Touhou Mobs, Arcadian Dream's fairies, and zombie village
//! sieges there.
//!
//! The torches of a world are kept in `<world>/thkaguyamod/gigatorches.tsv`, as Torchmaster keeps
//! its lights in the level's saved data, so a torch blocks spawns in chunks around it whether or
//! not its own chunk is loaded. Settings are in `config/thkaguyamod-gigatorch.toml`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use crate::sdk::{SpellHost, block_id};
use mod_api::{
    BlockChangedEvent, BlockEntityTickEvent, EntityJoinLevelEvent, Flow, Host, LevelHost,
    PotentialSpawnsEvent, RightClickBlockEvent, ServerHost, ServerTickEvent, ServerTickPhase, Text,
    VillageSiegeEvent,
};

pub const GIGATORCH: &str = "thkaguyamod:gigatorch";
pub const MAX_DOLL_POINT: &str = "thkaguyamod:max_doll_point";
const PHANTOM: &str = "minecraft:phantom";

/// The Touhou mobs the torch keeps away: every hostile mob Touhou Mobs spawns by itself (and the
/// classic versions it sometimes sends instead, and the familiar a sunflower fairy brings), and
/// Arcadian Dream's fairies. Both mods spawn them with their own code, not natural spawning, so
/// they are caught as they join the level. Touhou Mobs' friendly ones (Sakuya, Sanae, Rinnosuke),
/// Wakasagihime, and its bosses that only come when called are left alone.
pub const TOUHOU_MOBS: &[&str] = &[
    "thmobs:fairy",
    "thmobs:sunflower_fairy",
    "thmobs:familiar",
    "thmobs:ice_fairy",
    "thmobs:nether_fairy",
    "thmobs:moon_fairy",
    "thmobs:moon_sunflower_fairy",
    "thmobs:phantom",
    "thmobs:hanabeeper",
    "thmobs:cirno",
    "thmobs:cirno_plus",
    "thmobs:rumia",
    "thmobs:rumia_plus",
    "thmobs:toziko",
    "thmobs:wriggle",
    "thmobs:hina",
    "thmobs:crow_tengu",
    "thmobs:old_crow_tengu",
    "thmobs:hell_raven",
    "thmobs:marisa",
    "thmobs:reimu_hostile",
    "thmobs:classic_cirno",
    "thmobs:classic_rumia",
    "thmobs:classic_toziko",
    "thmobs:classic_wriggle",
    "thmobs:classic_marisa",
    "thmobs:classic_reimu_hostile",
    "arcadiandream:fairy",
    "arcadiandream:ice_fairy",
    "arcadiandream:sunflower_fairy",
];

/// A mob kind as the game's hooks name it, without the `minecraft:` they put before every kind, a
/// modded one's too (`minecraft:thmobs:fairy` is `thmobs:fairy`).
pub fn bare(kind: &str) -> &str {
    kind.strip_prefix("minecraft:")
        .filter(|k| k.contains(':'))
        .unwrap_or(kind)
}

pub fn is_touhou_mob(entity: &str) -> bool {
    TOUHOU_MOBS.contains(&bare(entity))
}

// ---- Settings ----

/// What the game writes when it finds no settings file: every setting at its default.
pub const DEFAULT_FILE: &str = r#"# The Giga Torch (Touhou Items Mod for Horizon). Changes take effect when the game starts again.

# The radius of the giga torch in each direction (cube) with the torch at its center
gigaTorchRadius = 64

# If enabled, the giga torch blocks village sieges from zombies
blockVillageSieges = true

# Changes to the list of entities the giga torch blocks: every monster, phantoms, every hostile mob
# of Touhou Mobs (fairies, Cirno, Rumia, Wriggle, Toziko, Hina, the tengu and ravens, Marisa,
# hostile Reimu and their classic versions) and Arcadian Dream's fairies. Wakasagihime still spawns.
# The + prefix adds an entity to the list (its spawns are denied),
# the - prefix removes it (its spawns are allowed).
# Allow creepers: ["-minecraft:creeper"]   Allow Cirno: ["-thmobs:cirno", "-thmobs:classic_cirno"]
gigaTorchEntityBlockListOverrides = []
"#;

/// The `+` and `-` entries of the block list override.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides {
    pub add: HashSet<String>,
    pub remove: HashSet<String>,
}

impl Overrides {
    /// `["+minecraft:pig", "-gaia:dryad"]`; entries that aren't `[+-]namespace:path` are skipped,
    /// as Torchmaster skips them. The last word on an entity wins.
    pub fn parse(list: &str) -> Self {
        let mut out = Self::default();
        let inner = list.trim().trim_start_matches('[').trim_end_matches(']');
        for raw in inner.split(',') {
            let entry = raw.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            if !valid(entry) {
                continue;
            }
            let (sign, id) = entry.split_at(1);
            if sign == "+" {
                out.remove.remove(id);
                out.add.insert(id.to_owned());
            } else {
                out.add.remove(id);
                out.remove.insert(id.to_owned());
            }
        }
        out
    }
}

/// Torchmaster's filter pattern, `[+-][a-z0-9_-]+:[a-z0-9_-]+`.
fn valid(entry: &str) -> bool {
    let Some(rest) = entry.strip_prefix(['+', '-']) else {
        return false;
    };
    let Some((ns, path)) = rest.split_once(':') else {
        return false;
    };
    let ok = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    };
    ok(ns) && ok(path)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub radius: i32,
    pub block_village_sieges: bool,
    pub overrides: Overrides,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            radius: 64,
            block_village_sieges: true,
            overrides: Overrides::default(),
        }
    }
}

impl Config {
    /// `key = value` lines; `#` starts a comment. Unknown keys and bad values keep the default.
    pub fn parse(text: &str) -> Self {
        let mut c = Self::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            match key {
                "gigaTorchRadius" => {
                    c.radius = value
                        .parse::<i64>()
                        .map_or(c.radius, |v| v.clamp(0, i64::from(i32::MAX)) as i32);
                }
                "blockVillageSieges" => {
                    c.block_village_sieges = value.parse().unwrap_or(c.block_village_sieges)
                }
                "gigaTorchEntityBlockListOverrides" => c.overrides = Overrides::parse(value),
                _ => {}
            }
        }
        c
    }

    /// Whether the torch keeps `entity`, of spawn category `category`, from spawning: every
    /// monster and the Touhou mobs, with the overrides added and taken away.
    pub fn blocks(&self, category: &str, entity: &str) -> bool {
        let entity = bare(entity);
        if self.overrides.remove.contains(entity) {
            return false;
        }
        category == "monster" || is_touhou_mob(entity) || self.overrides.add.contains(entity)
    }
}

static CONFIG: OnceLock<Arc<Config>> = OnceLock::new();

/// The settings, read once (written with the defaults when missing).
fn config(host: &(impl Host + ?Sized)) -> Arc<Config> {
    CONFIG
        .get_or_init(|| {
            let dir = host.instance_dir().join("config");
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("thkaguyamod-gigatorch.toml");
            let text = std::fs::read_to_string(&path).unwrap_or_else(|_| {
                let _ = std::fs::write(&path, DEFAULT_FILE);
                DEFAULT_FILE.to_owned()
            });
            Arc::new(Config::parse(&text))
        })
        .clone()
}

// ---- The world's torches ----

#[derive(Clone, Debug, PartialEq)]
pub struct Torch {
    pub dimension: String,
    pub pos: [i32; 3],
}

impl Torch {
    /// Torchmaster's cubic range: `pos` is within `radius` of the block's middle on every axis.
    pub fn reaches(&self, pos: [f64; 3], radius: i32) -> bool {
        (0..3).all(|i| (pos[i] - (f64::from(self.pos[i]) + 0.5)).abs() <= f64::from(radius))
    }
}

#[derive(Debug, Default)]
pub struct Store {
    pub torches: Vec<Torch>,
    /// The next torch the clean-up looks at.
    pub cursor: usize,
}

impl Store {
    /// `dimension x y z` a line, tab-separated.
    pub fn decode(text: &str) -> Self {
        let torches = text
            .lines()
            .filter_map(|line| {
                let f: Vec<&str> = line.split('\t').collect();
                let [dim, x, y, z] = f.as_slice() else {
                    return None;
                };
                Some(Torch {
                    dimension: (*dim).to_owned(),
                    pos: [x.parse().ok()?, y.parse().ok()?, z.parse().ok()?],
                })
            })
            .collect();
        Self { torches, cursor: 0 }
    }

    pub fn encode(&self) -> String {
        self.torches
            .iter()
            .map(|t| {
                format!(
                    "{}\t{}\t{}\t{}\n",
                    t.dimension, t.pos[0], t.pos[1], t.pos[2]
                )
            })
            .collect()
    }

    /// Adds a torch; false when it was already known.
    pub fn add(&mut self, dimension: &str, pos: [i32; 3]) -> bool {
        if self
            .torches
            .iter()
            .any(|t| t.dimension == dimension && t.pos == pos)
        {
            return false;
        }
        self.torches.push(Torch {
            dimension: dimension.to_owned(),
            pos,
        });
        true
    }

    /// Forgets the torch at `pos`; false when none was there.
    pub fn remove(&mut self, dimension: &str, pos: [i32; 3]) -> bool {
        let before = self.torches.len();
        self.torches
            .retain(|t| !(t.dimension == dimension && t.pos == pos));
        before != self.torches.len()
    }

    /// Whether a torch in `dimension` reaches `pos`.
    pub fn reaches(&self, dimension: &str, pos: [f64; 3], config: &Config) -> bool {
        self.torches
            .iter()
            .any(|t| t.dimension == dimension && t.reaches(pos, config.radius))
    }

    /// Whether a torch near `pos` keeps `entity` (of `category`) from spawning there.
    pub fn blocks_spawn(
        &self,
        dimension: &str,
        category: &str,
        entity: &str,
        pos: [f64; 3],
        config: &Config,
    ) -> bool {
        config.blocks(category, entity) && self.reaches(dimension, pos, config)
    }
}

static STORE: LazyLock<Mutex<Option<(PathBuf, Store)>>> = LazyLock::new(Default::default);

/// Runs `f` on the open world's torches, loading them first if another world's are loaded; saves
/// them when `f` says it changed them.
fn with_store<R>(host: &dyn ServerHost, f: impl FnOnce(&mut Store) -> (R, bool)) -> Option<R> {
    let path = host
        .world_dir()?
        .join("thkaguyamod")
        .join("gigatorches.tsv");
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

/// Server ticks counted since the game started.
static TICKS: AtomicU64 = AtomicU64::new(0);

fn block_middle(pos: [i32; 3]) -> [f64; 3] {
    pos.map(|v| f64::from(v) + 0.5)
}

pub fn tooltip_for(item: &str) -> Option<&'static str> {
    Some(match item {
        GIGATORCH => {
            "Prevents spawning of hostile monsters and Touhou mobs in a big radius around the torch"
        }
        MAX_DOLL_POINT => "Four Doll Points, bound in gold",
        _ => return None,
    })
}

// ---- What the hooks do ----

pub fn block_changed(host: &dyn LevelHost, event: &BlockChangedEvent) {
    let old = host.state_block(event.old) == Some(GIGATORCH);
    let new = host.state_block(event.new) == Some(GIGATORCH);
    if old == new {
        return;
    }
    let dimension = event.dimension.key().id();
    let pos = [event.pos.x, event.pos.y, event.pos.z];
    if let Ok(mut updates) = CHANGES.lock() {
        updates.push((dimension.to_owned(), pos, new));
    }
}

static CHANGES: Mutex<Vec<(String, [i32; 3], bool)>> = Mutex::new(Vec::new());

pub fn clear() {
    if let Ok(mut store) = STORE.lock() {
        *store = None;
    }
    if let Ok(mut changes) = CHANGES.lock() {
        changes.clear();
    }
}

pub fn block_entity_tick(host: &mut dyn ServerHost, event: &BlockEntityTickEvent<'_>) {
    // Every 5 seconds, at its own moment.
    let spread = (event.pos[0] ^ event.pos[2]).unsigned_abs() as u64;
    if (TICKS.load(Ordering::Relaxed) + spread) % 100 == 0 {
        let dimension = host.dimension().to_owned();
        with_store(host, |s| ((), s.add(&dimension, event.pos)));
    }
}

/// Once a second, one known torch is looked at: one whose block is gone is forgotten
/// (Torchmaster's `cleanupCheck`).
pub fn server_tick(host: &mut dyn ServerHost, event: &ServerTickEvent<'_>) {
    if !matches!(
        event.phase,
        ServerTickPhase::ServerTickEnd {
            world_running: true
        }
    ) {
        return;
    }
    let changes = CHANGES
        .lock()
        .map(|mut changes| std::mem::take(&mut *changes))
        .unwrap_or_default();
    with_store(host, |store| {
        let mut changed = false;
        for (dimension, pos, present) in changes {
            changed |= if present {
                store.add(&dimension, pos)
            } else {
                store.remove(&dimension, pos)
            };
        }
        ((), changed)
    });
    if TICKS.fetch_add(1, Ordering::Relaxed) % 20 != 0 {
        return;
    }
    let Some(torch) = with_store(host, |s| {
        if s.torches.is_empty() {
            return (None, false);
        }
        s.cursor = (s.cursor + 1) % s.torches.len();
        (Some(s.torches[s.cursor].clone()), false)
    })
    .flatten() else {
        return;
    };
    let Some(level) = host.level(&torch.dimension) else {
        return;
    };
    if level.is_loaded(host, torch.pos)
        && block_id(host, level, torch.pos).as_deref() != Some(GIGATORCH)
    {
        with_store(host, |s| ((), s.remove(&torch.dimension, torch.pos)));
    }
}

/// Natural spawning near a torch leaves out what it blocks.
pub fn potential_spawns(host: &dyn Host, event: &mut PotentialSpawnsEvent<'_>) {
    let config = config(host);
    let at = block_middle(event.pos);
    let (dimension, category) = (event.dimension, event.category);
    let entries = &mut *event.entries;
    if let Ok(held) = STORE.lock()
        && let Some((_, store)) = held.as_ref()
        && !store.torches.is_empty()
    {
        entries
            .retain(|entry| !store.blocks_spawn(dimension, category, &entry.entity, at, &config));
    }
}

/// No phantoms near a torch (Torchmaster's `PlayerSpawnPhantomsEvent`), and no Touhou mobs:
/// Touhou Mobs and Arcadian Dream spawn theirs with their own code, so they are caught as they
/// join. Spawn eggs and `/summon` of them fail there too; ones loaded with a chunk stay.
pub fn entity_join_level(host: &mut dyn ServerHost, event: &EntityJoinLevelEvent<'_>) -> Flow {
    if event.loaded_from_disk || (event.kind != PHANTOM && !is_touhou_mob(event.kind)) {
        return Flow::Continue;
    }
    let config = config(host);
    let blocked = with_store(&*host, |s| {
        (
            s.blocks_spawn(event.dimension, "monster", event.kind, event.pos, &config),
            false,
        )
    });
    if blocked == Some(true) {
        Flow::Cancel
    } else {
        Flow::Continue
    }
}

/// No village siege near a torch.
pub fn village_siege(host: &mut dyn ServerHost, event: &VillageSiegeEvent<'_>) -> Flow {
    let config = config(host);
    if !config.block_village_sieges {
        return Flow::Continue;
    }
    let near = with_store(&*host, |s| {
        (
            s.reaches(event.dimension, event.attempted_pos, &config),
            false,
        )
    });
    if near == Some(true) {
        Flow::Cancel
    } else {
        Flow::Continue
    }
}

/// Right-click a torch with an empty hand to see how far it reaches.
pub fn right_click_block(host: &mut dyn ServerHost, event: &RightClickBlockEvent<'_>) {
    let Some(player) = host.acting_caster() else {
        return;
    };
    let Some(level) = host.level(&player.dimension) else {
        return;
    };
    if player.sneaking
        || event.click.stack.is_some()
        || block_id(host, level, event.click.pos).as_deref() != Some(GIGATORCH)
    {
        return;
    }
    let line = format!(
        "Giga Torch: hostile mobs and Touhou mobs don't spawn within {} blocks in each direction",
        config(host).radius
    );
    host.action_bar(player.uuid, Text::literal(line));
}

#[cfg(test)]
mod tests {
    use super::*;

    const OW: &str = "minecraft:overworld";

    #[test]
    fn the_default_file_reads_as_the_defaults() {
        assert_eq!(Config::parse(DEFAULT_FILE), Config::default());
    }

    #[test]
    fn settings_are_read() {
        let c = Config::parse(
            "gigaTorchRadius = 32\nblockVillageSieges = false\nnonsense = 1\ngigaTorchRadius = x",
        );
        assert_eq!(c.radius, 32);
        assert!(!c.block_village_sieges);
    }

    #[test]
    fn overrides_add_and_remove_and_skip_bad_entries() {
        let o = Overrides::parse(
            r#"["+minecraft:pig", "-gaia:dryad", "zombie", "+Bad:Name", '-minecraft:creeper']"#,
        );
        assert!(o.add.contains("minecraft:pig"));
        assert!(o.remove.contains("gaia:dryad") && o.remove.contains("minecraft:creeper"));
        assert_eq!(o.add.len() + o.remove.len(), 3);
        let o = Overrides::parse(r#"["+minecraft:pig", "-minecraft:pig"]"#);
        assert!(o.remove.contains("minecraft:pig") && !o.add.contains("minecraft:pig"));
    }

    #[test]
    fn the_torch_blocks_monsters_and_touhou_mobs_unless_allowed() {
        let c = Config::default();
        assert!(c.blocks("monster", "minecraft:zombie"));
        assert!(!c.blocks("creature", "minecraft:pig"));
        for m in TOUHOU_MOBS {
            assert!(c.blocks("creature", m), "{m}");
            // As the join hook names them.
            assert!(c.blocks("creature", &format!("minecraft:{m}")), "{m}");
        }
        for friendly in [
            "thmobs:rinnosuke",
            "thmobs:sakuya",
            "thmobs:sanae",
            "minecraft:thmobs:yukari",
            "thmobs:yin_yang_orb",
            "minecraft:thmobs:wakasagihime",
        ] {
            assert!(!c.blocks("creature", friendly), "{friendly}");
        }
        let c = Config::parse(
            "gigaTorchEntityBlockListOverrides = [\"-thmobs:fairy\", \"-minecraft:creeper\", \"+minecraft:pig\"]",
        );
        assert!(!c.blocks("creature", "thmobs:fairy"));
        assert!(!c.blocks("creature", "minecraft:thmobs:fairy"));
        assert!(c.blocks("creature", "arcadiandream:fairy"));
        assert!(!c.blocks("monster", "minecraft:creeper"));
        assert!(c.blocks("creature", "minecraft:pig"));
    }

    #[test]
    fn spawns_are_blocked_only_near_a_torch_in_its_dimension() {
        let c = Config::default();
        let mut s = Store::default();
        assert!(s.add(OW, [0, 64, 0]));
        assert!(!s.add(OW, [0, 64, 0]));
        assert!(s.blocks_spawn(OW, "monster", "minecraft:zombie", [10.5, 60.0, 10.5], &c));
        assert!(s.blocks_spawn(
            OW,
            "creature",
            "arcadiandream:sunflower_fairy",
            [30.5, 70.0, -40.5],
            &c
        ));
        assert!(s.blocks_spawn(
            OW,
            "monster",
            "minecraft:thmobs:cirno",
            [-60.5, 100.0, 60.5],
            &c
        ));
        assert!(!s.blocks_spawn(
            OW,
            "creature",
            "arcadiandream:sunflower_fairy",
            [90.5, 70.0, 0.5],
            &c
        ));
        assert!(!s.blocks_spawn(OW, "monster", "minecraft:zombie", [100.5, 60.0, 10.5], &c));
        assert!(!s.blocks_spawn(
            "minecraft:the_nether",
            "monster",
            "minecraft:zombie",
            [0.5, 60.0, 0.5],
            &c
        ));
        assert!(!s.blocks_spawn(OW, "creature", "minecraft:cow", [0.5, 60.0, 0.5], &c));
    }

    #[test]
    fn the_range_is_a_cube_about_the_block_middle() {
        let t = Torch {
            dimension: OW.into(),
            pos: [0, 64, 0],
        };
        assert!(t.reaches([64.5, 64.5, -63.5], 64));
        assert!(!t.reaches([64.6, 64.5, 0.5], 64));
        assert!(!t.reaches([0.5, 128.6, 0.5], 64));
    }

    #[test]
    fn the_store_survives_a_round_trip() {
        let mut s = Store::default();
        s.add(OW, [1, 64, -3]);
        s.add("minecraft:the_nether", [-100, 30, 7]);
        assert_eq!(Store::decode(&s.encode()).torches, s.torches);
        assert_eq!(
            Store::decode("junk\nminecraft:overworld\t1\t2\n").torches,
            []
        );
        assert!(s.remove(OW, [1, 64, -3]));
        assert!(!s.remove(OW, [1, 64, -3]));
    }

    #[test]
    fn modded_kinds_lose_the_games_prefix() {
        assert_eq!(bare("minecraft:thmobs:fairy"), "thmobs:fairy");
        assert_eq!(bare("thmobs:fairy"), "thmobs:fairy");
        assert_eq!(bare("minecraft:phantom"), "minecraft:phantom");
    }

    #[test]
    fn both_new_items_have_tooltips() {
        assert!(tooltip_for(GIGATORCH).is_some() && tooltip_for(MAX_DOLL_POINT).is_some());
    }
}
