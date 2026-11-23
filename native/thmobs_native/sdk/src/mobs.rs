//! The mobs: what each is, how it looks, where it spawns, how it fights and its boss phases. Ported
//! from the original mods' entity classes; one row per mob, in the checklist's order.

use crate::attack::{self, Attack};
use crate::pattern::{Fire, rotate_default};
use crate::shot::shot;
use crate::vec::{Rng, V3, angle, rotate};

/// How a mob moves and fights (`EntityDanmakuMob`'s settings).
#[derive(Clone, Copy, Debug)]
pub struct Ai {
    pub speed: f64,
    pub attack_distance: f64,
    pub detection_distance: f64,
    /// How many blocks up it keeps from the ground; 0 walks.
    pub flying_height: i32,
}

const fn ai(speed: f64, attack_distance: f64, detection_distance: f64, flying_height: i32) -> Ai {
    Ai { speed, attack_distance, detection_distance, flying_height }
}

/// The light it spawns in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Light {
    /// In daylight, under open sky, on grass, dirt or sand, half the tries failing: the fairies.
    Fairy,
    /// In the dark, as a monster.
    Dark,
    Any,
    /// Anywhere, but only two tries in a hundred: Sakuya and Sanae.
    Rare,
}

/// When and where it appears on its own.
#[derive(Clone, Copy, Debug)]
pub struct Spawn {
    pub weight: u32,
    pub min: u32,
    pub max: u32,
    pub biomes: &'static [&'static str],
    pub light: Light,
}

const fn spawn(weight: u32, min: u32, max: u32, biomes: &'static [&'static str], light: Light) -> Option<Spawn> {
    Some(Spawn { weight, min, max, biomes, light })
}

/// How it is drawn.
#[derive(Clone, Copy, Debug)]
pub enum Look {
    /// Its Java model, `<mod>/<ModelClass>`, and its textures, `thmobs:entity/<texture>`.
    Model(&'static str, &'static [&'static str]),
    /// A glow facing the camera, as the phantoms and familiars were drawn: the sprite, and its size.
    Glow(&'static str, f32),
    /// A vanilla model layer and a vanilla texture, as the Hanabeeper is a creeper to look at.
    Vanilla(&'static str, &'static str),
}

/// A boss phase's end (`moveDanmakuAttack`): out of health in phase `from`, it comes back in phase
/// `to` with `hp`, waits `interval` ticks to attack and can't be hurt for `invincible`. A phase that
/// isn't listed ends in death.
#[derive(Clone, Copy, Debug)]
pub struct Phase {
    pub from: i32,
    pub to: i32,
    pub interval: i32,
    pub hp: f32,
    pub invincible: i32,
    pub ai: Option<Ai>,
}

const fn ph(from: i32, to: i32, interval: i32, hp: f32, invincible: i32, ai: Option<Ai>) -> Phase {
    Phase { from, to, interval, hp, invincible, ai }
}

#[derive(Debug)]
pub struct Mob {
    /// `thmobs:<id>`.
    pub id: &'static str,
    pub name: &'static str,
    pub look: Look,
    pub health: f32,
    pub ai: Ai,
    pub attack: Attack,
    /// A fairy's kind of counter: back to 0 past its span. A boss's attack resets its own.
    pub fairy_counter: bool,
    /// The phases (or forms) one is born in, one picked for each.
    pub start: &'static [i32],
    /// Its settings in a starting form that isn't the first, as a Nether fairy's zombie kind.
    pub start_ai: &'static [(i32, Ai)],
    pub phases: &'static [Phase],
    pub spawn: Option<Spawn>,
    /// It dies in sunlight under the open sky, as the phantoms did.
    pub burns: bool,
    /// Peaceful: it wanders and looks about, and doesn't fight.
    pub npc: bool,
    /// Its spawn egg's colours.
    pub egg: (u32, u32),
}

impl Mob {
    /// The most health it has in any phase: its type's maximum. A classic boss has all its spell
    /// cards' health in one bar, as the Wither: its health times its cards.
    pub fn max_health(&self) -> f32 {
        if self.classic() {
            return self.health * self.cards() as f32;
        }
        self.phases.iter().map(|p| p.hp).fold(self.health, f32::max)
    }

    /// Whether it is one of the classic bosses.
    pub fn classic(&self) -> bool {
        self.id.starts_with("classic_") || self.id == "yukari_great_doll"
    }

    /// Its spell cards: the phase it starts in and each one it goes on to.
    pub fn cards(&self) -> usize {
        self.phases.len() + 1
    }

    /// Which of its cards (from 0) the phase is.
    pub fn card_of(&self, phase: i32) -> usize {
        self.phases.iter().position(|p| p.to == phase).map_or(0, |i| i + 1)
    }

    /// The card a classic boss with this much health is on: one more for each card's worth of
    /// health it has lost, the last card down to nothing.
    pub fn card_at(&self, health: f32) -> usize {
        let lost = ((self.max_health() - health) / self.health).max(0.0);
        (lost.floor() as usize).min(self.cards() - 1)
    }

    /// The health a classic boss starts its card `card` with: all its later cards' and that one's.
    pub fn top_of(&self, card: usize) -> f32 {
        self.max_health() - self.health * card as f32
    }
}

const FAIRY_BIOMES: &[&str] = &[
    "plains", "desert", "forest", "taiga", "swamp", "jungle", "birch_forest", "dark_forest", "windswept_hills",
    "sunflower_plains", "flower_forest", "old_growth_birch_forest", "snowy_plains", "ice_spikes", "meadow",
// Biomes O' Plenty
    "biomesoplenty:field", "biomesoplenty:forested_field", "biomesoplenty:pasture", "biomesoplenty:prairie", "biomesoplenty:lavender_field", "biomesoplenty:orchard", "biomesoplenty:seasonal_forest", "biomesoplenty:shrubland", "biomesoplenty:woodland", "biomesoplenty:old_growth_woodland", "biomesoplenty:maple_woods", "biomesoplenty:coniferous_forest", "biomesoplenty:highland", "biomesoplenty:moor", "biomesoplenty:mediterranean_forest", "biomesoplenty:overgrown_greens", "biomesoplenty:pumpkin_patch", "biomesoplenty:snowblossom_grove", "biomesoplenty:fir_clearing", "biomesoplenty:mystic_grove", "biomesoplenty:rainforest", "biomesoplenty:floodplain"];
const EVERYWHERE: &[&str] = &[
    "plains", "desert", "forest", "taiga", "swamp", "jungle", "sparse_jungle", "bamboo_jungle", "birch_forest",
    "dark_forest", "windswept_hills", "windswept_forest", "sunflower_plains", "flower_forest", "old_growth_birch_forest",
    "old_growth_pine_taiga", "old_growth_spruce_taiga", "snowy_plains", "snowy_taiga", "ice_spikes", "savanna",
    "savanna_plateau", "badlands", "wooded_badlands", "eroded_badlands", "beach", "snowy_beach", "stony_shore", "river",
    "frozen_river", "meadow", "grove", "mangrove_swamp", "cherry_grove",
// Biomes O' Plenty
    "biomesoplenty:field", "biomesoplenty:forested_field", "biomesoplenty:pasture", "biomesoplenty:prairie", "biomesoplenty:lavender_field", "biomesoplenty:orchard", "biomesoplenty:seasonal_forest", "biomesoplenty:shrubland", "biomesoplenty:woodland", "biomesoplenty:old_growth_woodland", "biomesoplenty:maple_woods", "biomesoplenty:coniferous_forest", "biomesoplenty:highland", "biomesoplenty:moor", "biomesoplenty:mediterranean_forest", "biomesoplenty:overgrown_greens", "biomesoplenty:pumpkin_patch", "biomesoplenty:snowblossom_grove", "biomesoplenty:fir_clearing", "biomesoplenty:mystic_grove", "biomesoplenty:rainforest", "biomesoplenty:floodplain", "biomesoplenty:bayou", "biomesoplenty:bog", "biomesoplenty:cold_desert", "biomesoplenty:crag", "biomesoplenty:dead_forest", "biomesoplenty:dryland", "biomesoplenty:dune_beach", "biomesoplenty:gravel_beach", "biomesoplenty:hot_springs", "biomesoplenty:jade_cliffs", "biomesoplenty:lush_desert", "biomesoplenty:lush_savanna", "biomesoplenty:marsh", "biomesoplenty:muskeg", "biomesoplenty:old_growth_dead_forest", "biomesoplenty:ominous_woods", "biomesoplenty:origin_valley", "biomesoplenty:redwood_forest", "biomesoplenty:rocky_rainforest", "biomesoplenty:rocky_shrubland", "biomesoplenty:scrubland", "biomesoplenty:snowy_coniferous_forest", "biomesoplenty:snowy_fir_clearing", "biomesoplenty:snowy_maple_woods", "biomesoplenty:tropics", "biomesoplenty:tundra", "biomesoplenty:volcanic_plains", "biomesoplenty:wasteland", "biomesoplenty:wasteland_steppe", "biomesoplenty:wetland", "biomesoplenty:wintry_origin_valley", "biomesoplenty:subtropics"];
const COLD: &[&str] = &["snowy_taiga", "snowy_plains", "ice_spikes", "frozen_river", "snowy_slopes", "grove", "snowy_beach",
    // Biomes O' Plenty
    "biomesoplenty:tundra", "biomesoplenty:snowy_coniferous_forest", "biomesoplenty:snowy_fir_clearing", "biomesoplenty:snowy_maple_woods", "biomesoplenty:muskeg", "biomesoplenty:wintry_origin_valley", "biomesoplenty:cold_desert"];
const DARK_WOODS: &[&str] = &["forest", "plains", "dark_forest", "taiga", "old_growth_pine_taiga", "old_growth_spruce_taiga",
    // Biomes O' Plenty
    "biomesoplenty:ominous_woods", "biomesoplenty:dead_forest", "biomesoplenty:old_growth_dead_forest", "biomesoplenty:coniferous_forest", "biomesoplenty:redwood_forest", "biomesoplenty:maple_woods", "biomesoplenty:woodland", "biomesoplenty:seasonal_forest"];
const TOZIKO: &[&str] = &["forest", "plains", "swamp", "mangrove_swamp",
    // Biomes O' Plenty
    "biomesoplenty:bayou", "biomesoplenty:bog", "biomesoplenty:marsh", "biomesoplenty:wetland", "biomesoplenty:field", "biomesoplenty:woodland"];
const WRIGGLE: &[&str] = &["birch_forest", "old_growth_birch_forest", "forest", "jungle", "sparse_jungle", "bamboo_jungle", "river",
    // Biomes O' Plenty
    "biomesoplenty:rainforest", "biomesoplenty:rocky_rainforest", "biomesoplenty:tropics", "biomesoplenty:mystic_grove", "biomesoplenty:seasonal_forest", "biomesoplenty:floodplain"];
const MOUNTAINS: &[&str] = &[
    "windswept_hills", "windswept_gravelly_hills", "windswept_forest", "jagged_peaks", "frozen_peaks", "stony_peaks",
    "meadow", "grove", "snowy_slopes", "cherry_grove",
// Biomes O' Plenty
    "biomesoplenty:crag", "biomesoplenty:highland", "biomesoplenty:jade_cliffs", "biomesoplenty:volcano", "biomesoplenty:rocky_shrubland"];
const NETHER: &[&str] = &["nether_wastes", "crimson_forest", "warped_forest", "soul_sand_valley", "basalt_deltas",
    // Biomes O' Plenty
    "biomesoplenty:crystalline_chasm", "biomesoplenty:erupting_inferno", "biomesoplenty:undergrowth", "biomesoplenty:visceral_heap", "biomesoplenty:withered_abyss"];
const END: &[&str] = &["the_end", "end_highlands", "end_midlands", "small_end_islands", "end_barrens",
    // Biomes O' Plenty
    "biomesoplenty:end_corruption", "biomesoplenty:end_flats", "biomesoplenty:end_reef", "biomesoplenty:end_wilds"];

const FAIRY_AI: Ai = ai(0.3, 8.0, 4.0, 2);

pub const MOBS: &[Mob] = &[
    // ---- Touhou Items Mod ----
    Mob {
        id: "fairy", name: "Fairy", look: Look::Model("kaguya/ModelTHFairy", &["fairy_0", "fairy_1", "fairy_2"]),
        health: 2.0, ai: FAIRY_AI, attack: attack::fairy, fairy_counter: true, start: &[1], start_ai: &[], phases: &[],
        spawn: spawn(30, 1, 6, FAIRY_BIOMES, Light::Fairy), burns: false, npc: false, egg: (0x8FD3FF, 0xFFFFFF),
    },
    Mob {
        id: "sunflower_fairy", name: "Sunflower Fairy", look: Look::Model("kaguya/ModelSunFlowerFairy", &["fairy_10"]),
        health: 20.0, ai: FAIRY_AI, attack: attack::fairy, fairy_counter: true, start: &[1], start_ai: &[], phases: &[],
        spawn: spawn(3, 1, 1, FAIRY_BIOMES, Light::Fairy), burns: false, npc: false, egg: (0xFFD800, 0x8FD3FF),
    },
    Mob {
        id: "phantom", name: "Phantom", look: Look::Glow("light", 0.7),
        health: 2.0, ai: ai(0.18, 6.0, 30.0, 3), attack: attack::fairy, fairy_counter: true, start: &[1], start_ai: &[], phases: &[],
        spawn: spawn(20, 1, 3, EVERYWHERE, Light::Dark), burns: true, npc: false, egg: (0xD8F0FF, 0x7FA7FF),
    },
    Mob {
        id: "familiar", name: "Familiar", look: Look::Glow("familiar", 0.5),
        health: 2.0, ai: ai(0.3, 8.0, 8.0, 2), attack: attack::fairy, fairy_counter: true, start: &[1], start_ai: &[], phases: &[],
        spawn: None, burns: false, npc: false, egg: (0xFF7FD8, 0xFFFFFF),
    },
    Mob {
        id: "hanabeeper", name: "Hanabeeper", look: Look::Vanilla("minecraft:creeper#main", "entity/creeper/creeper"),
        health: 20.0, ai: ai(0.25, 2.0, 16.0, 0), attack: attack::hanabeeper, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[], spawn: spawn(2, 1, 1, &["desert", "biomesoplenty:lush_desert", "biomesoplenty:dryland", "biomesoplenty:wasteland", "biomesoplenty:scrubland"], Light::Dark), burns: false, npc: false, egg: (0x2DB12B, 0xFF9C00),
    },
    Mob {
        id: "cirno", name: "Cirno", look: Look::Model("kaguya/ModelCirno", &["cirno"]),
        health: 38.0, ai: ai(0.5, 14.0, 30.0, 2), attack: attack::cirno, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[ph(1, 101, 40, 46.0, 60, None), ph(101, 2, 60, 32.0, 20, None), ph(2, 102, 40, 58.0, 60, None)],
        spawn: spawn(2, 1, 1, COLD, Light::Any), burns: false, npc: false, egg: (0x60B8FF, 0xFFFFFF),
    },
    Mob {
        id: "rumia", name: "Rumia", look: Look::Model("kaguya/ModelRumia", &["rumia"]),
        health: 36.0, ai: ai(0.25, 14.0, 12.0, 2), attack: attack::rumia, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[ph(1, 101, 40, 44.0, 60, Some(ai(0.25, 14.0, 30.0, 2)))],
        spawn: spawn(4, 1, 1, DARK_WOODS, Light::Dark), burns: false, npc: false, egg: (0x202020, 0xFFE080),
    },
    Mob {
        id: "toziko", name: "Soga no Tojiko", look: Look::Model("kaguya/ModelToziko", &["toziko"]),
        health: 40.0, ai: ai(0.25, 14.0, 12.0, 2), attack: attack::toziko, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[ph(1, 101, 40, 44.0, 60, Some(ai(0.25, 14.0, 30.0, 2)))],
        spawn: spawn(9, 1, 1, TOZIKO, Light::Dark), burns: false, npc: false, egg: (0x70C070, 0xFFFFFF),
    },
    Mob {
        id: "wriggle", name: "Wriggle Nightbug", look: Look::Model("kaguya/ModelWriggle", &["wriggle"]),
        health: 30.0, ai: ai(0.25, 14.0, 12.0, 3), attack: attack::wriggle, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[
            ph(1, 101, 20, 36.0, 40, Some(ai(0.25, 16.0, 30.0, 2))),
            ph(101, 102, 60, 42.0, 80, Some(ai(0.25, 15.0, 30.0, 4))),
        ],
        spawn: spawn(20, 1, 3, WRIGGLE, Light::Dark), burns: false, npc: false, egg: (0x40A040, 0x202060),
    },
    Mob {
        id: "reimu", name: "Reimu Hakurei", look: Look::Model("kaguya/ModelReimu", &["reimu"]),
        health: 74.0, ai: ai(0.4, 14.0, 0.0, 4), attack: attack::reimu, fairy_counter: false, start: &[101], start_ai: &[],
        phases: &[ph(1, 101, 40, 60.0, 160, None), ph(101, 127, 90, 40.0, 160, None)],
        spawn: None, burns: false, npc: false, egg: (0xE02020, 0xFFFFFF),
    },
    // ---- Touhou Items Mod's people ----
    Mob {
        id: "sakuya", name: "Sakuya Izayoi", look: Look::Model("kaguya/ModelSakuya", &["sakuya"]),
        health: 44.0, ai: ai(0.5, 14.0, 0.0, 0), attack: attack::none, fairy_counter: false, start: &[0], start_ai: &[], phases: &[],
        spawn: spawn(1, 0, 1, EVERYWHERE, Light::Rare), burns: false, npc: true, egg: (0x404080, 0xE0E0E0),
    },
    Mob {
        id: "sanae", name: "Sanae Kochiya", look: Look::Model("kaguya/ModelSanae", &["sanae"]),
        health: 56.0, ai: ai(0.5, 14.0, 0.0, 0), attack: attack::none, fairy_counter: false, start: &[0], start_ai: &[], phases: &[],
        spawn: spawn(1, 0, 1, EVERYWHERE, Light::Rare), burns: false, npc: true, egg: (0x40A060, 0x4060E0),
    },
    Mob {
        id: "miko", name: "Toyosatomimi no Miko", look: Look::Model("kaguya/ModelMiko", &["miko"]),
        health: 74.0, ai: ai(0.4, 14.0, 0.0, 0), attack: attack::none, fairy_counter: false, start: &[0], start_ai: &[], phases: &[],
        spawn: None, burns: false, npc: true, egg: (0x8040C0, 0xE0C040),
    },
    Mob {
        id: "rinnosuke", name: "Rinnosuke Morichika", look: Look::Model("kaguya/ModelRinnosuke", &["rinnosuke"]),
        health: 20.0, ai: ai(0.5, 0.0, 0.0, 0), attack: attack::none, fairy_counter: false, start: &[0], start_ai: &[], phases: &[],
        spawn: spawn(1, 1, 1, EVERYWHERE, Light::Rare), burns: false, npc: true, egg: (0x404040, 0x6080C0),
    },
    Mob {
        id: "cursed_decoy_doll", name: "Cursed Decoy Doll", look: Look::Model("kaguya/ModelCursedDecoyDoll", &["decoy"]),
        health: 40.0, ai: ai(0.0, 0.0, 0.0, 0), attack: attack::none, fairy_counter: false, start: &[0], start_ai: &[], phases: &[],
        spawn: None, burns: false, npc: true, egg: (0x603020, 0xE0C0A0),
    },
    // ---- SPELLCARD PLUS ----
    Mob {
        id: "wakasagihime", name: "Wakasagihime", look: Look::Model("spellcard/ModelWkasagihime", &["wakasagihime"]),
        health: 20.0, ai: ai(1.0, 14.0, 0.0, 0), attack: attack::wakasagihime, fairy_counter: false, start: &[0], start_ai: &[],
        phases: &[
            ph(0, 1, 40, 40.0, 60, None),
            ph(1, 101, 40, 60.0, 60, None),
            ph(101, 2, 40, 50.0, 20, None),
            ph(2, 102, 40, 70.0, 60, None),
        ],
        spawn: spawn(10, 1, 1, &["river", "swamp", "mangrove_swamp", "biomesoplenty:bayou", "biomesoplenty:marsh", "biomesoplenty:wetland", "biomesoplenty:floodplain", "biomesoplenty:bog"], Light::Any), burns: false, npc: false, egg: (0x3070C0, 0x40C0A0),
    },
    Mob {
        id: "cirno_plus", name: "Cirno (SPELLCARD PLUS)", look: Look::Model("spellcard/ModelCirno", &["cirno"]),
        health: 60.0, ai: ai(0.5, 14.0, 30.0, 2), attack: attack::cirno_plus, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[
            ph(1, 101, 40, 80.0, 60, None),
            ph(101, 2, 60, 70.0, 20, None),
            ph(2, 102, 40, 90.0, 60, None),
            ph(102, 103, 40, 100.0, 60, None),
        ],
        spawn: spawn(2, 1, 1, COLD, Light::Any), burns: false, npc: false, egg: (0x3090FF, 0xFFFFFF),
    },
    Mob {
        id: "rumia_plus", name: "Rumia (SPELLCARD PLUS)", look: Look::Model("spellcard/ModelRumia", &["rumia"]),
        health: 40.0, ai: ai(0.25, 14.0, 12.0, 2), attack: attack::rumia_plus, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[
            ph(1, 101, 40, 60.0, 60, Some(ai(0.25, 14.0, 30.0, 2))),
            ph(101, 2, 40, 50.0, 60, None),
            ph(2, 102, 40, 70.0, 60, Some(ai(0.25, 14.0, 30.0, 2))),
            ph(102, 3, 40, 60.0, 60, None),
            ph(3, 103, 40, 80.0, 60, Some(ai(0.25, 14.0, 30.0, 2))),
        ],
        spawn: spawn(4, 1, 1, DARK_WOODS, Light::Dark), burns: false, npc: false, egg: (0x101010, 0xFFC040),
    },
    Mob {
        id: "hina", name: "Hina Kagiyama", look: Look::Model("spellcard/ModelHina", &["hina"]),
        health: 1.0, ai: ai(0.5, 14.0, 0.0, 1), attack: attack::hina, fairy_counter: false, start: &[0], start_ai: &[],
        phases: &[
            ph(0, 1, 40, 60.0, 60, Some(ai(0.5, 14.0, 0.0, 2))),
            ph(1, 101, 40, 80.0, 60, None),
            ph(101, 2, 40, 70.0, 20, None),
            ph(2, 102, 40, 90.0, 60, Some(ai(0.5, 14.0, 0.0, 1))),
            ph(102, 3, 40, 80.0, 60, None),
            ph(3, 103, 40, 100.0, 60, None),
            ph(103, 104, 40, 110.0, 60, Some(ai(2.0, 14.0, 0.0, 1))),
        ],
        spawn: spawn(10, 1, 1, &["river", "biomesoplenty:bayou", "biomesoplenty:marsh", "biomesoplenty:wetland", "biomesoplenty:floodplain"], Light::Any), burns: false, npc: false, egg: (0xC02040, 0x40A040),
    },
    Mob {
        id: "yin_yang_orb", name: "Yin-Yang Orb (Yukari)", look: Look::Model("spellcard/ModelYin_yang_orb_Yukari", &["yin_yang_orb"]),
        health: 20.0, ai: ai(0.0, 0.0, 0.0, 0), attack: attack::none, fairy_counter: false, start: &[0], start_ai: &[], phases: &[],
        spawn: None, burns: false, npc: true, egg: (0xFFFFFF, 0x202020),
    },
    // ---- Journey To Gensokyo ----
    Mob {
        id: "ice_fairy", name: "Ice Fairy", look: Look::Model("jtg/ModelTHFairyIce", &["fairy_ice"]),
        health: 10.0, ai: ai(0.6, 12.0, 12.0, 3), attack: attack::fairy, fairy_counter: true, start: &[1], start_ai: &[], phases: &[],
        spawn: spawn(20, 1, 3, COLD, Light::Any), burns: false, npc: false, egg: (0xB8E8FF, 0x4080FF),
    },
    Mob {
        id: "nether_fairy", name: "Nether Fairy", look: Look::Model("jtg/ModelTHFairyJTG", &["fairy_zombie"]),
        health: 2.0, ai: FAIRY_AI, attack: attack::nether_fairy, fairy_counter: true, start: &[1, 2],
        start_ai: &[(2, ai(0.03, 2.0, 8.0, 2))], phases: &[],
        spawn: spawn(20, 1, 3, NETHER, Light::Any), burns: false, npc: false, egg: (0x802020, 0x404040),
    },
    Mob {
        id: "moon_fairy", name: "Moon Fairy", look: Look::Model("jtg/ModelTHFairyJTG", &["fairy_end_0", "fairy_end_1", "fairy_end_2"]),
        health: 5.0, ai: ai(0.7, 16.0, 16.0, 4), attack: attack::fairy_end, fairy_counter: true, start: &[1, 2, 3], start_ai: &[], phases: &[],
        spawn: spawn(10, 1, 3, END, Light::Any), burns: false, npc: false, egg: (0xE8E0A0, 0x6040A0),
    },
    Mob {
        id: "moon_sunflower_fairy", name: "Moon Sunflower Fairy",
        look: Look::Model("jtg/ModelSunFlowerFairyJTG", &["fairy_sunflower_end"]),
        health: 20.0, ai: ai(0.7, 16.0, 16.0, 4), attack: attack::sunflower_fairy_end, fairy_counter: true, start: &[1, 2, 3],
        start_ai: &[], phases: &[],
        spawn: spawn(5, 1, 3, END, Light::Any), burns: false, npc: false, egg: (0xFFD800, 0x6040A0),
    },
    Mob {
        id: "crow_tengu", name: "Crow Tengu", look: Look::Model("jtg/ModelTenguCrow", &["tengu_crow"]),
        health: 10.0, ai: ai(0.25, 16.0, 16.0, 4), attack: attack::tengu_crow, fairy_counter: true, start: &[1, 2], start_ai: &[], phases: &[],
        spawn: spawn(3, 1, 3, MOUNTAINS, Light::Any), burns: false, npc: false, egg: (0x202020, 0xC02020),
    },
    // The crow tengu as it was before it was made slower and fewer: the original's speed (0.5) and its
    // full rate of fire. Rare, and always alone.
    Mob {
        id: "old_crow_tengu", name: "Old Crow Tengu", look: Look::Model("jtg/ModelTenguCrow", &["tengu_crow"]),
        health: 10.0, ai: ai(0.5, 16.0, 16.0, 4), attack: attack::tengu_crow, fairy_counter: true, start: &[1, 2], start_ai: &[], phases: &[],
        spawn: spawn(1, 1, 1, MOUNTAINS, Light::Rare), burns: false, npc: false, egg: (0x000000, 0xFF2020),
    },
    Mob {
        id: "hell_raven", name: "Hell Raven", look: Look::Model("jtg/ModelHellRaven", &["hell_raven"]),
        health: 15.0, ai: ai(0.3, 16.0, 16.0, 4), attack: attack::hell_raven, fairy_counter: true, start: &[1, 2], start_ai: &[], phases: &[],
        spawn: spawn(20, 1, 3, NETHER, Light::Any), burns: false, npc: false, egg: (0x301010, 0xFF6020),
    },
    Mob {
        id: "marisa", name: "Marisa Kirisame", look: Look::Model("jtg/ModelMarisa", &["marisa"]),
        health: 60.0, ai: ai(0.4, 20.0, 0.0, 0), attack: attack::marisa, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[
            ph(1, 101, 40, 60.0, 160, None),
            ph(101, 2, 90, 60.0, 160, None),
            ph(2, 102, 40, 60.0, 160, None),
            ph(102, 3, 90, 60.0, 160, None),
            ph(3, 103, 40, 60.0, 160, None),
        ],
        spawn: spawn(1, 1, 1, &["forest", "birch_forest", "dark_forest", "flower_forest", "biomesoplenty:woodland", "biomesoplenty:old_growth_woodland", "biomesoplenty:seasonal_forest", "biomesoplenty:maple_woods", "biomesoplenty:mystic_grove"], Light::Any), burns: false,
        npc: false, egg: (0x202020, 0xFFE040),
    },
    Mob {
        id: "reimu_hostile", name: "Reimu Hakurei (Journey To Gensokyo)", look: Look::Model("kaguya/ModelReimu", &["reimu"]),
        health: 74.0, ai: ai(0.4, 50.0, 0.0, 0), attack: attack::reimu_hostile, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[ph(1, 101, 40, 60.0, 160, Some(ai(0.4, 50.0, 0.0, 2)))],
        spawn: spawn(1, 1, 1, &["forest", "birch_forest", "dark_forest", "flower_forest", "biomesoplenty:woodland", "biomesoplenty:old_growth_woodland", "biomesoplenty:seasonal_forest", "biomesoplenty:maple_woods", "biomesoplenty:mystic_grove"], Light::Any), burns: false,
        npc: false, egg: (0xE02020, 0xFFE0E0),
    },
    Mob {
        id: "yukari", name: "Yukari Yakumo", look: Look::Model("jtg/ModelYukari", &["yukari"]),
        health: 100.0, ai: ai(0.45, 20.0, 20.0, 2), attack: attack::yukari, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[
            ph(1, 101, 10, 100.0, 160, None),
            ph(101, 2, 10, 100.0, 160, None),
            ph(2, 102, 10, 100.0, 160, None),
            ph(102, 3, 10, 100.0, 160, None),
            ph(3, 103, 10, 300.0, 160, None),
            ph(103, 4, 10, 100.0, 160, None),
            ph(4, 104, 10, 100.0, 160, None),
            ph(104, 5, 10, 100.0, 160, None),
            ph(5, 105, 10, 300.0, 160, None),
        ],
        spawn: None, burns: false, npc: false, egg: (0x8040C0, 0xFFE0A0),
    },
    Mob {
        id: "ice_familiar", name: "Ice Familiar", look: Look::Glow("familiar", 0.5),
        health: 2.0, ai: ai(0.6, 12.0, 8.0, 3), attack: attack::fairy, fairy_counter: true, start: &[1], start_ai: &[], phases: &[],
        spawn: None, burns: false, npc: false, egg: (0x80E0FF, 0xFFFFFF),
    },
    // ---- The classic bosses: the named characters as the original mods had them, at their full
    // rate of fire, with every spell card they declared, lasers and all. No spawn rules of their own
    // (`spawn: None`): from their eggs, or 1 in 20,000 in place of their counterpart above
    // (`classic_of`). Their cards share one bar, `health` each: each card's worth lost, on to the next. ----
    Mob {
        id: "classic_reimu", name: "Reimu Hakurei (Classic)", look: Look::Model("kaguya/ModelReimu", &["reimu"]),
        health: 74.0, ai: ai(0.4, 14.0, 0.0, 4), attack: attack::classic_reimu, fairy_counter: false, start: &[101], start_ai: &[],
        phases: &[ph(101, 102, 90, 74.0, 160, None)],
        spawn: None, burns: false, npc: false, egg: (0xFFFFFF, 0xE02020),
    },
    Mob {
        id: "classic_reimu_hostile", name: "Reimu Hakurei (Journey To Gensokyo, Classic)", look: Look::Model("kaguya/ModelReimu", &["reimu"]),
        health: 74.0, ai: ai(0.4, 50.0, 0.0, 0), attack: attack::classic_reimu_hostile, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[ph(1, 101, 40, 60.0, 160, Some(ai(0.4, 50.0, 0.0, 2))), ph(101, 102, 90, 74.0, 160, None)],
        spawn: None, burns: false, npc: false, egg: (0xFFE0E0, 0xE02020),
    },
    Mob {
        id: "classic_marisa", name: "Marisa Kirisame (Classic)", look: Look::Model("jtg/ModelMarisa", &["marisa"]),
        health: 60.0, ai: ai(0.4, 20.0, 0.0, 0), attack: attack::classic_marisa, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[
            ph(1, 101, 40, 60.0, 160, None),
            ph(101, 2, 90, 60.0, 160, None),
            ph(2, 102, 40, 60.0, 160, None),
            ph(102, 3, 90, 60.0, 160, None),
            ph(3, 103, 40, 60.0, 160, None),
            ph(103, 104, 90, 60.0, 160, None),
        ],
        spawn: None, burns: false, npc: false, egg: (0xFFE040, 0x202020),
    },
    Mob {
        id: "classic_yukari", name: "Yukari Yakumo (Classic)", look: Look::Model("jtg/ModelYukari", &["yukari"]),
        health: 100.0, ai: ai(0.45, 20.0, 20.0, 2), attack: attack::yukari, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[
            ph(1, 101, 10, 100.0, 160, None),
            ph(101, 2, 10, 100.0, 160, None),
            ph(2, 102, 10, 100.0, 160, None),
            ph(102, 3, 10, 100.0, 160, None),
            ph(3, 103, 10, 300.0, 160, None),
            ph(103, 4, 10, 100.0, 160, None),
            ph(4, 104, 10, 100.0, 160, None),
            ph(104, 5, 10, 100.0, 160, None),
            ph(5, 105, 10, 300.0, 160, None),
        ],
        spawn: None, burns: false, npc: false, egg: (0xFFE0A0, 0x8040C0),
    },
    // Classic Yukari with a quarter of her health and half her danmaku's damage (`GREAT_DOLL`): she
    // comes through a gap made global with a Yukari umbrella in place of Yukari, and always drops
    // an umbrella (Yukari Gaps).
    Mob {
        id: "yukari_great_doll", name: "Yukari Great Doll", look: Look::Model("jtg/ModelYukari", &["yukari"]),
        health: 25.0, ai: ai(0.45, 20.0, 20.0, 2), attack: attack::yukari, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[
            ph(1, 101, 10, 25.0, 160, None),
            ph(101, 2, 10, 25.0, 160, None),
            ph(2, 102, 10, 25.0, 160, None),
            ph(102, 3, 10, 25.0, 160, None),
            ph(3, 103, 10, 75.0, 160, None),
            ph(103, 4, 10, 25.0, 160, None),
            ph(4, 104, 10, 25.0, 160, None),
            ph(104, 5, 10, 25.0, 160, None),
            ph(5, 105, 10, 75.0, 160, None),
        ],
        spawn: None, burns: false, npc: false, egg: (0xFFE0A0, 0xE060C0),
    },
    Mob {
        id: "classic_cirno", name: "Cirno (Classic)", look: Look::Model("kaguya/ModelCirno", &["cirno"]),
        health: 38.0, ai: ai(0.5, 14.0, 30.0, 2), attack: attack::classic_cirno, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[ph(1, 101, 40, 46.0, 60, None), ph(101, 2, 60, 32.0, 20, None), ph(2, 102, 40, 58.0, 60, None)],
        spawn: None, burns: false, npc: false, egg: (0xFFFFFF, 0x60B8FF),
    },
    Mob {
        id: "classic_rumia", name: "Rumia (Classic)", look: Look::Model("kaguya/ModelRumia", &["rumia"]),
        health: 36.0, ai: ai(0.25, 14.0, 12.0, 2), attack: attack::classic_rumia, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[ph(1, 101, 40, 44.0, 60, Some(ai(0.25, 14.0, 30.0, 2))), ph(101, 102, 60, 44.0, 60, None)],
        spawn: None, burns: false, npc: false, egg: (0xFFE080, 0x202020),
    },
    Mob {
        id: "classic_toziko", name: "Soga no Tojiko (Classic)", look: Look::Model("kaguya/ModelToziko", &["toziko"]),
        health: 40.0, ai: ai(0.25, 14.0, 12.0, 2), attack: attack::classic_toziko, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[ph(1, 101, 40, 44.0, 60, Some(ai(0.25, 14.0, 30.0, 2))), ph(101, 102, 60, 44.0, 60, None)],
        spawn: None, burns: false, npc: false, egg: (0xFFFFFF, 0x70C070),
    },
    Mob {
        id: "classic_wriggle", name: "Wriggle Nightbug (Classic)", look: Look::Model("kaguya/ModelWriggle", &["wriggle"]),
        health: 30.0, ai: ai(0.25, 14.0, 12.0, 3), attack: attack::wriggle, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[
            ph(1, 101, 20, 36.0, 40, Some(ai(0.25, 16.0, 30.0, 2))),
            ph(101, 102, 60, 42.0, 80, Some(ai(0.25, 15.0, 30.0, 4))),
        ],
        spawn: None, burns: false, npc: false, egg: (0x202060, 0x40A040),
    },
    // The people Touhou Items Mod gave spell cards but never set fighting: here they fight, in the
    // phases their entities had for it (`moveDanmakuAttack`), from their spell-card phase.
    Mob {
        id: "classic_sakuya", name: "Sakuya Izayoi (Classic)", look: Look::Model("kaguya/ModelSakuya", &["sakuya"]),
        health: 44.0, ai: ai(0.5, 14.0, 0.0, 2), attack: attack::classic_sakuya, fairy_counter: false, start: &[101], start_ai: &[],
        phases: &[ph(101, 102, 90, 44.0, 160, None)],
        spawn: None, burns: false, npc: false, egg: (0xE0E0E0, 0x404080),
    },
    Mob {
        id: "classic_sanae", name: "Sanae Kochiya (Classic)", look: Look::Model("kaguya/ModelSanae", &["sanae"]),
        health: 56.0, ai: ai(0.5, 14.0, 0.0, 0), attack: attack::classic_sanae, fairy_counter: false, start: &[1], start_ai: &[],
        phases: &[
            ph(1, 101, 40, 60.0, 160, Some(ai(0.5, 14.0, 0.0, 2))),
            ph(101, 102, 90, 60.0, 160, None),
            ph(102, 103, 90, 60.0, 160, None),
            ph(103, 104, 90, 60.0, 160, None),
        ],
        spawn: None, burns: false, npc: false, egg: (0x4060E0, 0x40A060),
    },
    Mob {
        id: "classic_miko", name: "Toyosatomimi no Miko (Classic)", look: Look::Model("kaguya/ModelMiko", &["miko"]),
        health: 74.0, ai: ai(0.4, 14.0, 0.0, 2), attack: attack::classic_miko, fairy_counter: false, start: &[101], start_ai: &[],
        phases: &[ph(101, 102, 90, 74.0, 160, None), ph(102, 103, 90, 74.0, 160, None)],
        spawn: None, burns: false, npc: false, egg: (0xE0C040, 0x8040C0),
    },
];

/// The classic boss of `m`, if it has one: `classic_<id>`.
pub fn classic_of(m: &Mob) -> Option<&'static Mob> {
    by_kind(&format!("thmobs:classic_{}", m.id))
}

/// A mob kind as a hook names it, without the `minecraft:` the game's hooks put before every mob
/// kind, a modded one's too (`minecraft:thmobs:fairy`).
pub fn bare(kind: &str) -> &str {
    kind.strip_prefix("minecraft:").filter(|k| k.contains(':')).unwrap_or(kind)
}

pub fn by_kind(kind: &str) -> Option<&'static Mob> {
    let id = bare(kind).strip_prefix("thmobs:")?;
    MOBS.iter().find(|m| m.id == id)
}

/// The phase a mob of `spec` is born in: one of its starts, by its id.
pub fn start_of(spec: &Mob, id: u32) -> i32 {
    spec.start[(id as usize).wrapping_mul(2_654_435_761) % spec.start.len()]
}

/// The texture of the mob `id`: a moon fairy's by its form, the others' one by its id.
pub fn texture_of(spec: &Mob, id: u32) -> &'static str {
    let Look::Model(_, textures) = spec.look else { return "" };
    if spec.id == "moon_fairy" {
        return textures[(start_of(spec, id) % 3) as usize];
    }
    textures[id as usize % textures.len()]
}

/// One of `DanmakuPatternRegistry`'s fairy patterns: the shape code, ticks between volleys, the
/// shots' speed, and the shot (form times eight plus colour).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pattern {
    pub code: i32,
    pub span: i32,
    pub speed: f32,
    pub shot: i32,
}

const fn p(code: i32, span: i32, speed: f32, shot: i32) -> Pattern {
    Pattern { code, span, speed, shot }
}

/// The fairies' patterns at each danmaku level (easy, normal, hard, lunatic). The laser pattern
/// ("064") fires small shots until lasers are ported.
pub const FAIRY_PATTERNS: [[Pattern; 9]; 4] = [
    [p(0, 38, 0.3, 1), p(2003, 50, 0.2, 0), p(1902, 50, 0.2, 2), p(5015, 40, 0.1, 3), p(5, 40, 0.2, 36), p(3225, 50, 0.2, 13), p(4030, 50, 0.15, 94), p(300, 50, 0.2, 109), p(0, 38, 0.3, 0)],
    [p(0, 30, 0.3, 1), p(2007, 50, 0.2, 0), p(1804, 50, 0.2, 2), p(5021, 40, 0.2, 3), p(7, 35, 0.2, 36), p(3420, 45, 0.25, 13), p(4220, 50, 0.15, 94), p(400, 45, 0.25, 109), p(0, 30, 0.3, 0)],
    [p(0, 22, 0.4, 1), p(2011, 40, 0.3, 0), p(1608, 40, 0.4, 2), p(5027, 40, 0.3, 19), p(11, 25, 0.4, 36), p(3620, 35, 0.3, 5), p(4415, 50, 0.25, 94), p(500, 40, 0.35, 109), p(0, 26, 0.4, 0)],
    [p(0, 3, 0.5, 1), p(2015, 35, 0.4, 16), p(1410, 30, 0.6, 18), p(5027, 40, 0.4, 243), p(19, 10, 0.6, 36), p(3915, 24, 0.4, 45), p(4810, 35, 0.35, 94), p(700, 15, 0.45, 109), p(0, 20, 0.5, 0)],
];

/// `THKaguyaConfig.danmakuAccuracy`: how far off, in degrees, a mob's aim may be.
pub const ACCURACY: f32 = 80.0;

/// `THShotLib.angle_LimitRandom`: `look` turned up to half of `limit` about a random axis.
pub fn aim(look: V3, limit: f32, rng: &mut Rng) -> V3 {
    let axis = angle(rng.float() * 360.0, rng.float() * 180.0 - 90.0);
    rotate(axis, look, rng.float() * limit - limit / 2.0)
}

/// `EntityTHFairy.danmakuPattern`: one volley of `pattern` from `pos` along `look`.
pub fn fairy_volley(pattern: Pattern, pos: V3, look: V3, fire: &mut Fire) {
    let look = aim(look, ACCURACY, fire.rng);
    let form = (pattern.shot / 8) as u8;
    let color = (pattern.shot % 8) as u8;
    let data = shot(form, color, 0, 80, 0);
    let d = pattern.code;
    let speed_rate = f64::from(pattern.speed);
    match d {
        0..=999 => {
            let way = (d % 100 + 1) as usize;
            let num = d / 100 + 1;
            let mut speed = speed_rate;
            let slow = speed / f64::from(num);
            for _ in 0..num {
                fire.sphere(pos, look, 0.0, rotate_default(), 0.0, 9999, speed, speed, 0.0, V3::ZERO, data, way, 0.0, 0.0);
                speed -= slow;
            }
        }
        1000..=1999 => {
            let num = d % 100 + 1;
            let span = (d - 1000) as f32 / 50.0 + 1.0;
            fire.wide(pos, look, 0.0, 9999, speed_rate, speed_rate, 0.0, V3::ZERO, data, num, num as f32 * span, 0.0, 0.0);
        }
        2000..=2999 => {
            let way = d % 100 + 1;
            let num = (d - 2000) / 100 + 1;
            let mut speed = speed_rate;
            let slow = speed / f64::from(num);
            for _ in 0..num {
                fire.circle_simple(pos, look, speed, data, way);
                speed -= slow;
            }
        }
        3000..=3999 => {
            let span = (d % 100) as f32 + 1.0;
            let num = (d - 3000) / 100 + 1;
            fire.random_ring(pos, look, 0.0, 9999, speed_rate, speed_rate, 0.0, V3::ZERO, data, num, 0.0, span);
        }
        4000..=4999 => {
            let span = (((d - 4000) % 100) as f32 + 1.0) * 1.8;
            let way = (d - 4000) / 100 * (d % 100 / 10) + 3;
            let base = fire.rng.float() * 360.0;
            fire.ring(pos, look, 0.0, 9999, speed_rate, speed_rate, 0.0, V3::ZERO, data, way, 0.0, span, base);
        }
        5000..=5999 => {
            let num = d % 100 + 1;
            fire.random_ring(pos, look, 0.0, 9999, speed_rate, speed_rate, 0.0, V3::ZERO, data, num, 0.0, 360.0);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shot::Shot;

    #[test]
    fn every_fairy_pattern_fires() {
        for level in FAIRY_PATTERNS {
            for pattern in level {
                let mut out: Vec<Shot> = Vec::new();
                let mut rng = Rng::new(9);
                fairy_volley(pattern, V3::ZERO, angle(0.0, 0.0), &mut Fire { out: &mut out, rng: &mut rng });
                assert!(!out.is_empty(), "{pattern:?}");
                assert!(out.iter().all(|s| s.motion.length().is_finite()));
            }
        }
    }

    #[test]
    fn the_classic_bosses_never_spawn_and_fight() {
        let classic: Vec<_> = MOBS.iter().filter(|m| m.id.starts_with("classic_")).collect();
        assert_eq!(classic.len(), 11);
        for m in classic {
            assert!(m.spawn.is_none() && !m.npc, "{}", m.id);
            assert!(!m.phases.is_empty(), "{}", m.id);
            assert!(m.name.ends_with("Classic)"), "{}", m.id);
            // Every phase fights: none ends in the original's spent phase 127, so past the last
            // phase the boss dies.
            assert!(m.phases.iter().all(|p| p.to != 127), "{}", m.id);
            // Each stands in for its counterpart now and then.
            let base = by_kind(&format!("thmobs:{}", &m.id["classic_".len()..])).unwrap_or_else(|| panic!("{}", m.id));
            assert_eq!(classic_of(base).map(|c| c.id), Some(m.id));
        }
    }

    #[test]
    fn mobs_name_real_models_and_have_starts() {
        for m in MOBS {
            if let Look::Model(model, textures) = m.look {
                assert!(crate::model::find(model).is_some(), "{model}");
                assert!(!textures.is_empty());
            }
            assert!(!m.start.is_empty(), "{}", m.id);
            assert_eq!(by_kind(&format!("thmobs:{}", m.id)).map(|x| x.id), Some(m.id));
            for p in m.phases {
                assert!(p.hp <= m.max_health());
            }
        }
    }

    #[test]
    fn a_hook_s_minecraft_prefixed_kind_is_still_ours() {
        assert_eq!(by_kind("minecraft:thmobs:fairy").map(|m| m.id), Some("fairy"));
        assert_eq!(by_kind("thmobs:fairy").map(|m| m.id), Some("fairy"));
        assert_eq!(bare("minecraft:zombie"), "minecraft:zombie");
        assert!(by_kind("minecraft:zombie").is_none());
    }

    #[test]
    fn a_classic_boss_goes_through_its_cards_as_its_health_falls() {
        let reimu = by_kind("thmobs:classic_reimu_hostile").unwrap();
        assert_eq!(reimu.cards(), 3);
        assert_eq!(reimu.max_health(), 74.0 * 3.0);
        assert_eq!(reimu.card_at(222.0), 0);
        assert_eq!(reimu.card_at(148.5), 0);
        assert_eq!(reimu.card_at(148.0), 1);
        assert_eq!(reimu.card_at(74.0), 2);
        assert_eq!(reimu.card_at(1.0), 2);
        assert_eq!(reimu.card_of(1), 0);
        assert_eq!(reimu.card_of(101), 1);
        assert_eq!(reimu.card_of(102), 2);
        assert_eq!(reimu.top_of(1), 148.0);
        let doll = by_kind("thmobs:yukari_great_doll").unwrap();
        let yukari = by_kind("thmobs:classic_yukari").unwrap();
        assert!(doll.classic());
        assert_eq!(doll.max_health(), yukari.max_health() * 0.25);
        for m in MOBS.iter().filter(|m| m.classic()) {
            assert_eq!(m.start.len(), 1, "{}", m.id);
            // The phases run in order from the one it starts in.
            let mut at = m.start[0];
            for p in m.phases {
                assert_eq!(p.from, at, "{}", m.id);
                at = p.to;
            }
        }
    }
}
