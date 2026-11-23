// Better Dynamic Lights for Horizon: the mod's settings, texts and light levels, with no game
// types, so they build and test on their own (`cargo test`).
//
// The lights themselves are the game's (the mod API's `visuals` lights): the game hands them to
// its terrain and entity shaders, which light the world around each one. This mod chooses the
// sources and how far and how brightly each one shines.

use std::sync::Mutex;

pub const ID: &str = "dynamiclights";
pub const NAME: &str = "Better Dynamic Lights";
pub const VERSION: &str = "1.1.0";

/// The mod's one key: turns the lights on and off while playing.
pub const TOGGLE_KEY: &str = "toggle_key";
/// A key that is not bound (the game's `InputConstants.UNKNOWN`).
pub const UNBOUND: &str = "key.keyboard.unknown";

/// A light level is at most 15.
pub const MAX_LEVEL: u8 = 15;
/// How many of a mod's lights the game takes each frame.
pub const MAX_SOURCES: u8 = 8;
/// How far from the player the game takes a mod's lights, in blocks.
pub const MAX_RANGE: u8 = 32;

/// How a key's name reads (`InputConstants.Key.getDisplayName`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum KeyName {
    /// A key that types a character: that character, upper case (`glfwGetKeyName`).
    Literal(String),
    /// The game's own name for it (`key.keyboard.space`, `key.mouse.left`).
    Translate(String),
    /// Another mouse button, by its number ("Button 4": `key.mouse` with the number).
    Button(u32),
}

pub fn key_name(key: &str) -> KeyName {
    if let Some(rest) = key.strip_prefix("key.keyboard.") {
        let typed = match rest {
            "apostrophe" => Some("'"),
            "comma" => Some(","),
            "minus" => Some("-"),
            "period" => Some("."),
            "slash" => Some("/"),
            "semicolon" => Some(";"),
            "equal" => Some("="),
            "left.bracket" => Some("["),
            "backslash" => Some("\\"),
            "right.bracket" => Some("]"),
            "grave.accent" => Some("`"),
            _ => None,
        };
        if let Some(c) = typed {
            return KeyName::Literal(c.to_owned());
        }
        if rest.chars().count() == 1 && rest.chars().all(|c| c.is_ascii_alphanumeric()) {
            return KeyName::Literal(rest.to_ascii_uppercase());
        }
    }
    if let Some(n) = key.strip_prefix("key.mouse.").and_then(|n| n.parse::<u32>().ok()) {
        return KeyName::Button(n);
    }
    KeyName::Translate(key.to_owned())
}

/// One text in both languages.
fn t(ru: bool, en: &'static str, russian: &'static str) -> &'static str {
    if ru { russian } else { en }
}

pub fn description(ru: bool) -> &'static str {
    t(
        ru,
        "Horizon's built-in dynamic lights, with more: besides held and dropped items and burning mobs, glowing \
         creatures, projectiles, TNT and minecarts shine too, water puts out fire, and every kind of light has its \
         own switch.",
        "Встроенный динамический свет Horizon, только больше: кроме предметов в руке, на земле и горящих мобов \
         светятся ещё светящиеся существа, снаряды, динамит и вагонетки, вода гасит огонь, и у каждого вида света \
         свой переключатель.",
    )
}

// ---- light sources ---------------------------------------------------------

/// The kinds of light sources, each with its own switch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Glowing items in the player's own hands.
    OwnHands,
    /// Glowing items held by other players.
    Players,
    /// Glowing items held by mobs and armor stands.
    Mobs,
    /// Glowing items lying on the ground.
    Dropped,
    /// Anything on fire.
    Burning,
    /// Creatures that glow by themselves.
    Creatures,
    /// Fireballs, skulls, bullets, spectral arrows, rockets.
    Projectiles,
    /// Primed TNT, lit TNT minecarts, running furnace minecarts.
    Explosives,
}

impl Kind {
    pub const ALL: [Kind; 8] = [
        Kind::OwnHands,
        Kind::Players,
        Kind::Mobs,
        Kind::Dropped,
        Kind::Burning,
        Kind::Creatures,
        Kind::Projectiles,
        Kind::Explosives,
    ];

    fn index(self) -> usize {
        Kind::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }

    /// Its setting's name (a file in the mod's data folder).
    fn key(self) -> &'static str {
        match self {
            Kind::OwnHands => "own_hands",
            Kind::Players => "players",
            Kind::Mobs => "mobs",
            Kind::Dropped => "dropped_items",
            Kind::Burning => "burning",
            Kind::Creatures => "creatures",
            Kind::Projectiles => "projectiles",
            Kind::Explosives => "explosives",
        }
    }
}

/// How brightly an entity on fire shines: as fire does (Horizon's `BURNING`).
pub const BURNING: u8 = 15;

/// How brightly an item glows when held or dropped: as its block does when placed, in its
/// default state (`BlockState.getLightEmission`): torches 14, lanterns and glowstone 15, a soul
/// torch 10, a redstone torch 7; glow berries as cave vines with berries (14), a lava bucket as
/// lava (15). Unlit blocks (a furnace, a candle, a redstone lamp) give none.
pub fn item_light(item: &str) -> u8 {
    let name = item.strip_prefix("minecraft:").unwrap_or(item);
    match name {
        "beacon" | "conduit" | "glowstone" | "jack_o_lantern" | "lantern" | "sea_lantern" | "shroomlight"
        | "ochre_froglight" | "verdant_froglight" | "pearlescent_froglight" | "campfire" | "light"
        | "lava_bucket" => 15,
        "torch" | "copper_torch" | "end_rod" | "glow_berries" => 14,
        "crying_obsidian" | "soul_torch" | "soul_lantern" | "soul_campfire" => 10,
        "redstone_torch" | "enchanting_table" | "ender_chest" => 7,
        "sculk_catalyst" | "sea_pickle" | "vault" => 6,
        "amethyst_cluster" => 5,
        "large_amethyst_bud" => 4,
        "magma_block" => 3,
        "medium_amethyst_bud" | "firefly_bush" => 2,
        "small_amethyst_bud" | "brewing_stand" | "brown_mushroom" | "dragon_egg" | "end_portal_frame"
        | "sculk_sensor" | "calibrated_sculk_sensor" => 1,
        // Copper lanterns, weathered or waxed.
        _ if name.ends_with("copper_lantern") => 15,
        _ => 0,
    }
}

/// Items whose light is a flame or lava, which water puts out: torches
/// (not a redstone torch), campfires and a lava bucket. Lanterns keep their
/// flame behind glass.
pub fn fire_item(item: &str) -> bool {
    let name = item.rsplit(':').next().unwrap_or(item);
    (name.ends_with("torch") && !name.contains("redstone")) || name.ends_with("campfire") || name == "lava_bucket"
}

/// The light of a creature that glows by itself (vanilla draws these at
/// full brightness), 0 for the rest. `size` is a magma cube's size, and
/// `dark_ticks` how long a hurt glow squid stays dark.
pub fn creature_light(kind: &str, size: i32, dark_ticks: i32) -> u8 {
    match kind {
        "minecraft:blaze" => 10,
        "minecraft:magma_cube" => match size {
            ..=1 => 6,
            2..=3 => 8,
            _ => 10,
        },
        // As `GlowSquidRenderer` fades it: dark while hurt, back over the
        // last 10 dark ticks.
        "minecraft:glow_squid" => {
            let glow = (1.0 - dark_ticks as f32 / 10.0).clamp(0.0, 1.0);
            (8.0 * glow).round() as u8
        }
        "minecraft:allay" | "minecraft:vex" => 6,
        _ => 0,
    }
}

/// Creatures made of fire, which water puts out.
pub fn fire_creature(kind: &str) -> bool {
    kind == "minecraft:blaze"
}

/// The light of each projectile that glows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Glow {
    LargeFireball,
    SmallFireball,
    DragonFireball,
    WitherSkull,
    ShulkerBullet,
    SpectralArrow,
    Rocket,
    PrimedTnt,
    TntMinecart,
    FurnaceMinecart,
}

impl Glow {
    pub fn level(self) -> u8 {
        match self {
            Glow::LargeFireball => 15,
            Glow::SmallFireball => 12,
            Glow::DragonFireball => 10,
            Glow::WitherSkull => 6,
            Glow::ShulkerBullet | Glow::SpectralArrow | Glow::Rocket => 8,
            Glow::PrimedTnt | Glow::TntMinecart => 10,
            // A lit furnace.
            Glow::FurnaceMinecart => 13,
        }
    }

    /// Flames, which water puts out.
    pub fn fire(self) -> bool {
        matches!(self, Glow::LargeFireball | Glow::SmallFireball)
    }

    pub fn kind(self) -> Kind {
        match self {
            Glow::PrimedTnt | Glow::TntMinecart | Glow::FurnaceMinecart => Kind::Explosives,
            _ => Kind::Projectiles,
        }
    }

    /// The glow of an entity of type `kind` that glows whatever its state (a lit minecart is
    /// told apart by the caller).
    pub fn of(kind: &str) -> Option<Glow> {
        Some(match kind {
            "minecraft:fireball" => Glow::LargeFireball,
            "minecraft:small_fireball" => Glow::SmallFireball,
            "minecraft:dragon_fireball" => Glow::DragonFireball,
            "minecraft:wither_skull" => Glow::WitherSkull,
            "minecraft:shulker_bullet" => Glow::ShulkerBullet,
            "minecraft:spectral_arrow" => Glow::SpectralArrow,
            "minecraft:firework_rocket" => Glow::Rocket,
            "minecraft:tnt" => Glow::PrimedTnt,
            _ => return None,
        })
    }

    /// How high above an entity's feet its light sits.
    pub fn lift(self) -> f64 {
        match self {
            Glow::LargeFireball | Glow::DragonFireball | Glow::PrimedTnt | Glow::TntMinecart | Glow::FurnaceMinecart => 0.5,
            Glow::Rocket => 0.125,
            Glow::SpectralArrow => 0.25,
            _ => 0.15,
        }
    }
}

// ---- what a source is made of ----------------------------------------------

/// Whose hands a source's held light is in, which picks its switch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Who {
    /// The player playing.
    Me,
    /// Another player.
    Player,
    /// A mob or an armor stand.
    Mob,
    /// A dropped item (its "held" light is its own).
    Dropped,
    /// Anything else: only fire or its own glow.
    Other,
}

impl Who {
    pub fn code(self) -> u8 {
        self as u8
    }
    pub fn from_code(code: u8) -> Who {
        match code {
            0 => Who::Me,
            1 => Who::Player,
            2 => Who::Mob,
            3 => Who::Dropped,
            _ => Who::Other,
        }
    }
    fn kind(self) -> Option<Kind> {
        match self {
            Who::Me => Some(Kind::OwnHands),
            Who::Player => Some(Kind::Players),
            Who::Mob => Some(Kind::Mobs),
            Who::Dropped => Some(Kind::Dropped),
            Who::Other => None,
        }
    }
}

/// A light level and whether it is a flame (which water puts out): `level | 0x80` for a flame.
pub fn flame(level: u8, fire: bool) -> u8 {
    level.min(MAX_LEVEL) | if fire && level > 0 { 0x80 } else { 0 }
}

/// What one entity gives light by, before the settings choose: the brighter glowing item it
/// holds (or is, dropped), whether it burns, and its own glow and that glow's switch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fact {
    pub who: Who,
    /// The held (or dropped) item's light, as [`flame`] packs it.
    pub held: u8,
    pub burning: bool,
    /// Its own glow (a creature, a projectile, TNT, a lit minecart), as [`flame`] packs it.
    pub glow: u8,
    /// The glow's switch: [`Kind::Creatures`], [`Kind::Projectiles`] or [`Kind::Explosives`].
    pub glow_kind: Kind,
}

impl Fact {
    pub fn new(who: Who) -> Fact {
        Fact { who, held: 0, burning: false, glow: 0, glow_kind: Kind::Creatures }
    }

    /// The brighter of two held items: `(item light, flame)` each.
    pub fn hands(mut self, items: impl IntoIterator<Item = (u8, bool)>) -> Fact {
        for (level, fire) in items {
            if level > self.held & 0x7F {
                self.held = flame(level, fire);
            }
        }
        self
    }

    /// The light it gives with settings `s`; `underwater` tells whether it is in water there.
    pub fn level(&self, s: &Settings, underwater: impl Fn() -> bool) -> u8 {
        let lit = |packed: u8| {
            let level = packed & 0x7F;
            if level > 0 && packed & 0x80 != 0 && s.water && underwater() { 0 } else { level }
        };
        let held = match self.who.kind() {
            Some(k) if s.on(k) => lit(self.held),
            _ => 0,
        };
        let fire = if self.burning && s.on(Kind::Burning) { BURNING } else { 0 };
        let glow = if s.on(self.glow_kind) { lit(self.glow) } else { 0 };
        held.max(fire).max(glow)
    }
}

/// The facts the server sends a client, as bytes: per entity its id, where its light is, and
/// the fact. 33 bytes each, at most [`MAX_FACTS`].
pub const FACT_BYTES: usize = 33;
pub const MAX_FACTS: usize = 512;

pub fn encode_facts(facts: &[(u32, [f64; 3], Fact)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(facts.len().min(MAX_FACTS) * FACT_BYTES);
    for (id, at, f) in facts.iter().take(MAX_FACTS) {
        out.extend_from_slice(&id.to_le_bytes());
        for v in at {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&[f.who.code(), f.held, u8::from(f.burning), f.glow, f.glow_kind.index() as u8]);
    }
    out
}

/// The facts back; a record that doesn't read (a short or hostile message) ends the list.
pub fn decode_facts(bytes: &[u8]) -> Vec<(u32, [f64; 3], Fact)> {
    bytes
        .chunks_exact(FACT_BYTES)
        .take(MAX_FACTS)
        .map_while(|r| {
            let id = u32::from_le_bytes(r[0..4].try_into().ok()?);
            let mut at = [0.0; 3];
            for (i, v) in at.iter_mut().enumerate() {
                *v = f64::from_le_bytes(r[4 + i * 8..12 + i * 8].try_into().ok()?);
            }
            if !at.iter().all(|v| v.is_finite()) {
                return None;
            }
            let kind = *Kind::ALL.get(usize::from(r[32]))?;
            let fact = Fact { who: Who::from_code(r[28]), held: r[29], burning: r[30] != 0, glow: r[31], glow_kind: kind };
            Some((id, at, fact))
        })
        .collect()
}

// ---- settings --------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    pub enabled: bool,
    pub kinds: [bool; 8],
    /// Flames and lava give no light underwater.
    pub water: bool,
    /// Percent of the full level, 25 to 100.
    pub brightness: u8,
    /// How many of the nearest sources light up, 1 to 8.
    pub max_sources: u8,
    /// Blocks from the camera beyond which sources give no light.
    pub range: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: true,
            kinds: [true; 8],
            water: true,
            brightness: 100,
            max_sources: MAX_SOURCES,
            range: MAX_RANGE,
        }
    }
}

/// A slider's numbers: smallest, largest and the step between.
#[derive(Clone, Copy, Debug)]
struct Steps {
    min: u8,
    max: u8,
    step: u8,
}

impl Steps {
    fn clamp(self, v: u8) -> u8 {
        let v = v.clamp(self.min, self.max);
        self.min + (v - self.min) / self.step * self.step
    }
    /// Where `v` sits on the slider, 0 to 1.
    fn fraction(self, v: u8) -> f32 {
        f32::from(v.saturating_sub(self.min)) / f32::from(self.max - self.min)
    }
    /// The step nearest `f` (0 to 1) along the slider.
    fn at(self, f: f32) -> u8 {
        let steps = f32::from((self.max - self.min) / self.step);
        let n = (f.clamp(0.0, 1.0) * steps).round() as u8;
        self.min + n * self.step
    }
}

const BRIGHTNESS: Steps = Steps { min: 25, max: 100, step: 5 };
const SOURCES: Steps = Steps { min: 1, max: MAX_SOURCES, step: 1 };
const RANGE: Steps = Steps { min: 16, max: MAX_RANGE, step: 4 };

/// One option of the settings screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Opt {
    Enabled,
    Kind(Kind),
    Water,
    Brightness,
    MaxSources,
    Range,
}

impl Opt {
    fn steps(self) -> Option<Steps> {
        match self {
            Opt::Brightness => Some(BRIGHTNESS),
            Opt::MaxSources => Some(SOURCES),
            Opt::Range => Some(RANGE),
            _ => None,
        }
    }

    /// A slider rather than an ON/OFF button.
    pub fn slider(self) -> bool {
        self.steps().is_some()
    }

    pub fn caption(self, ru: bool) -> &'static str {
        match self {
            Opt::Enabled => t(ru, "Dynamic Lights", "Динамический свет"),
            Opt::Kind(Kind::OwnHands) => t(ru, "Your Hands", "Ваши руки"),
            Opt::Kind(Kind::Players) => t(ru, "Other Players", "Другие игроки"),
            Opt::Kind(Kind::Mobs) => t(ru, "Mobs", "Мобы"),
            Opt::Kind(Kind::Dropped) => t(ru, "Dropped Items", "Предметы на земле"),
            Opt::Kind(Kind::Burning) => t(ru, "Burning Entities", "Горящие сущности"),
            Opt::Kind(Kind::Creatures) => t(ru, "Glowing Creatures", "Светящиеся существа"),
            Opt::Kind(Kind::Projectiles) => t(ru, "Projectiles", "Снаряды"),
            Opt::Kind(Kind::Explosives) => t(ru, "TNT and Minecarts", "Динамит и вагонетки"),
            Opt::Water => t(ru, "Water Puts Out Fire", "Вода гасит огонь"),
            Opt::Brightness => t(ru, "Brightness", "Яркость"),
            Opt::MaxSources => t(ru, "Max Light Sources", "Макс. источников"),
            Opt::Range => t(ru, "Range", "Дальность"),
        }
    }

    pub fn tooltip(self, ru: bool) -> &'static str {
        match self {
            Opt::Enabled => t(
                ru,
                "Glowing items, burning entities and glowing creatures light up the blocks and mobs around them \
                 as they move. Horizon Extras' own Dynamic lights are separate: leave them off, as they are at first.",
                "Светящиеся предметы, горящие сущности и светящиеся существа освещают блоки и мобов вокруг себя \
                 на ходу. Динамический свет из «Дополнений Horizon» — отдельный: оставьте его выключенным, как по умолчанию.",
            ),
            Opt::Kind(Kind::OwnHands) => t(
                ru,
                "A torch, lantern or other glowing item in your main hand or off hand lights your way.",
                "Факел, фонарь или другой светящийся предмет в основной или второй руке освещает вам путь.",
            ),
            Opt::Kind(Kind::Players) => t(
                ru,
                "Glowing items held by other players.",
                "Светящиеся предметы в руках других игроков.",
            ),
            Opt::Kind(Kind::Mobs) => t(
                ru,
                "Glowing items held by mobs and armor stands, such as a zombie with a torch.",
                "Светящиеся предметы в руках мобов и стоек для брони, например зомби с факелом.",
            ),
            Opt::Kind(Kind::Dropped) => t(
                ru,
                "Glowing items lying on the ground.",
                "Светящиеся предметы, лежащие на земле.",
            ),
            Opt::Kind(Kind::Burning) => t(
                ru,
                "Players, mobs, items and arrows on fire shine as brightly as fire.",
                "Игроки, мобы, предметы и стрелы в огне светят ярко, как сам огонь.",
            ),
            Opt::Kind(Kind::Creatures) => t(
                ru,
                "Blazes, magma cubes, glow squids, allays and vexes give off light of their own.",
                "Всполохи, магмовые кубы, светящиеся спруты, тихони и вредины светятся сами.",
            ),
            Opt::Kind(Kind::Projectiles) => t(
                ru,
                "Fireballs, dragon fireballs, wither skulls, shulker bullets, spectral arrows and firework rockets \
                 light their way.",
                "Файерболы, файерболы дракона, черепа визера, снаряды шалкера, спектральные стрелы и фейерверки \
                 освещают свой путь.",
            ),
            Opt::Kind(Kind::Explosives) => t(
                ru,
                "Primed TNT, a lit minecart with TNT and a running minecart with furnace glow.",
                "Светятся активированный динамит, подожжённая вагонетка с динамитом и едущая самоходная вагонетка.",
            ),
            Opt::Water => t(
                ru,
                "Torches, campfires, lava buckets, fireballs and blazes give no light underwater.",
                "Факелы, костры, вёдра лавы, файерболы и всполохи не светят под водой.",
            ),
            Opt::Brightness => t(
                ru,
                "How bright the lights are, compared with the light the same blocks give when placed.",
                "Насколько ярок свет по сравнению со светом тех же блоков, если их поставить.",
            ),
            Opt::MaxSources => t(
                ru,
                "How many of the light sources nearest to you shine at once (8 at most). Fewer is faster on a \
                 weak graphics card.",
                "Сколько ближайших источников света светят одновременно (не больше 8). Чем меньше, тем быстрее \
                 на слабой видеокарте.",
            ),
            Opt::Range => t(
                ru,
                "Light sources farther than this from the camera give no light (32 blocks at most).",
                "Источники света дальше этого расстояния от камеры не светят (не больше 32 блоков).",
            ),
        }
    }
}

/// "64 Blocks", "64 блока".
pub fn blocks(n: u8, ru: bool) -> String {
    if !ru {
        return format!("{n} Blocks");
    }
    let word = match (n % 10, n % 100) {
        (_, 11..=14) => "блоков",
        (1, _) => "блок",
        (2..=4, _) => "блока",
        _ => "блоков",
    };
    format!("{n} {word}")
}

impl Settings {
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Settings {
        let d = Settings::default();
        let flag = |k: &str, d: bool| match get(k).as_deref().map(str::trim) {
            Some("true") => true,
            Some("false") => false,
            _ => d,
        };
        let number = |k: &str, steps: Steps, d: u8| {
            get(k).and_then(|v| v.trim().parse::<u8>().ok()).map_or(d, |v| steps.clamp(v))
        };
        let mut kinds = d.kinds;
        for k in Kind::ALL {
            kinds[k.index()] = flag(k.key(), d.kinds[k.index()]);
        }
        Settings {
            enabled: flag("enabled", d.enabled),
            kinds,
            water: flag("water_puts_out_fire", d.water),
            brightness: number("brightness", BRIGHTNESS, d.brightness),
            max_sources: number("max_sources", SOURCES, d.max_sources),
            range: number("range", RANGE, d.range),
        }
    }

    /// Every setting as it is kept: `(name, value)`.
    pub fn stored(&self) -> Vec<(&'static str, String)> {
        let mut out = vec![("enabled", self.enabled.to_string())];
        out.extend(Kind::ALL.iter().map(|k| (k.key(), self.on(*k).to_string())));
        out.push(("water_puts_out_fire", self.water.to_string()));
        out.push(("brightness", self.brightness.to_string()));
        out.push(("max_sources", self.max_sources.to_string()));
        out.push(("range", self.range.to_string()));
        out
    }

    pub fn on(&self, kind: Kind) -> bool {
        self.kinds[kind.index()]
    }

    /// A switch's state; None for a slider.
    pub fn switch(&self, opt: Opt) -> Option<bool> {
        match opt {
            Opt::Enabled => Some(self.enabled),
            Opt::Kind(k) => Some(self.on(k)),
            Opt::Water => Some(self.water),
            _ => None,
        }
    }

    /// A switch pressed: it flips.
    pub fn flip(&mut self, opt: Opt) {
        match opt {
            Opt::Enabled => self.enabled = !self.enabled,
            Opt::Kind(k) => self.kinds[k.index()] = !self.kinds[k.index()],
            Opt::Water => self.water = !self.water,
            _ => {}
        }
    }

    fn number(&mut self, opt: Opt) -> Option<&mut u8> {
        match opt {
            Opt::Brightness => Some(&mut self.brightness),
            Opt::MaxSources => Some(&mut self.max_sources),
            Opt::Range => Some(&mut self.range),
            _ => None,
        }
    }

    /// Where a slider's handle is, 0 to 1.
    pub fn fraction(&self, opt: Opt) -> f32 {
        let mut s = *self;
        match (opt.steps(), s.number(opt)) {
            (Some(steps), Some(v)) => steps.fraction(*v),
            _ => 0.0,
        }
    }

    /// A slider moved to `f` (0 to 1): true when its value changed.
    pub fn set_fraction(&mut self, opt: Opt, f: f32) -> bool {
        let Some(steps) = opt.steps() else { return false };
        let Some(v) = self.number(opt) else { return false };
        let new = steps.at(f);
        std::mem::replace(v, new) != new
    }

    /// A slider stepped once with the keyboard: up or down.
    pub fn step(&mut self, opt: Opt, up: bool) -> bool {
        let Some(steps) = opt.steps() else { return false };
        let Some(v) = self.number(opt) else { return false };
        let new = if up { v.saturating_add(steps.step) } else { v.saturating_sub(steps.step) };
        let new = steps.clamp(new.max(steps.min));
        std::mem::replace(v, new) != new
    }

    /// A slider's value as it shows: "100%", "16", "64 Blocks".
    pub fn number_text(&self, opt: Opt, ru: bool) -> String {
        match opt {
            Opt::Brightness => format!("{}%", self.brightness),
            Opt::MaxSources => self.max_sources.to_string(),
            Opt::Range => blocks(self.range, ru),
            _ => String::new(),
        }
    }

    /// A light level with the brightness applied.
    pub fn shown(&self, level: f32) -> f32 {
        level.min(f32::from(MAX_LEVEL)) * f32::from(self.brightness) / 100.0
    }
}

/// The screen's title.
pub fn title(_ru: bool) -> &'static str {
    NAME
}

/// The headings over the screen's groups of options.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Heading {
    Sources,
    Lighting,
    Controls,
}

impl Heading {
    pub fn text(self, ru: bool) -> &'static str {
        match self {
            Heading::Sources => t(ru, "Light Sources", "Источники света"),
            Heading::Lighting => t(ru, "Lighting", "Освещение"),
            Heading::Controls => t(ru, "Controls", "Управление"),
        }
    }
}

/// The toggle key's name in the controls row and on the Horizon page.
pub fn key_label(ru: bool) -> &'static str {
    t(ru, "Toggle Dynamic Lights", "Вкл./выкл. динамический свет")
}

static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);

/// The settings, read once from the game's store.
pub fn settings(get: impl Fn(&str) -> Option<String>) -> Settings {
    let mut kept = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    *kept.get_or_insert_with(|| Settings::read(get))
}

pub fn set_settings(settings: Settings) {
    *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = Some(settings);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn settings_round_trip() {
        let mut s = Settings::default();
        s.enabled = false;
        s.flip(Opt::Kind(Kind::Dropped));
        s.water = false;
        s.brightness = 60;
        s.max_sources = 4;
        s.range = 20;
        let kept: HashMap<_, _> = s.stored().into_iter().collect();
        assert_eq!(kept.len(), 13);
        let back = Settings::read(|k| kept.get(k).cloned());
        assert_eq!(back, s);
    }

    #[test]
    fn missing_and_bad_values_are_defaults() {
        let s = Settings::read(|k| match k {
            "enabled" => Some("maybe".into()),
            "brightness" => Some("7".into()),
            "range" => Some("250".into()),
            "max_sources" => Some(" 5 \n".into()),
            _ => None,
        });
        assert!(s.enabled);
        assert_eq!(s.brightness, 25);
        assert_eq!(s.range, 32);
        assert_eq!(s.max_sources, 5);
        // 1.0.4's larger values come back as the largest the game takes.
        let old = Settings::read(|k| match k {
            "range" => Some("64".into()),
            "max_sources" => Some("16".into()),
            _ => None,
        });
        assert_eq!((old.range, old.max_sources), (32, 8));
        assert_eq!(Settings::read(|_| None), Settings::default());
    }

    #[test]
    fn sliders_move_in_steps() {
        let mut s = Settings::default();
        assert_eq!(s.fraction(Opt::Brightness), 1.0);
        assert!(s.set_fraction(Opt::Brightness, 0.0));
        assert_eq!(s.brightness, 25);
        assert!(!s.set_fraction(Opt::Brightness, 0.01));
        assert!(s.set_fraction(Opt::Range, 0.5));
        assert_eq!(s.range, 24);
        assert!(s.step(Opt::Range, true));
        assert_eq!(s.range, 28);
        s.range = 32;
        assert!(!s.step(Opt::Range, true));
        s.max_sources = 1;
        assert!(!s.step(Opt::MaxSources, false));
        assert!(s.step(Opt::MaxSources, true));
        assert_eq!(s.max_sources, 2);
        assert!(!s.set_fraction(Opt::Water, 0.5));
    }

    #[test]
    fn fire_goes_out_but_lanterns_stay() {
        for item in ["minecraft:torch", "minecraft:soul_torch", "minecraft:copper_torch", "minecraft:campfire",
            "minecraft:soul_campfire", "minecraft:lava_bucket"]
        {
            assert!(fire_item(item), "{item}");
        }
        for item in ["minecraft:redstone_torch", "minecraft:lantern", "minecraft:glowstone", "minecraft:sea_lantern"] {
            assert!(!fire_item(item), "{item}");
        }
    }

    #[test]
    fn items_glow_as_their_blocks() {
        assert_eq!(item_light("minecraft:torch"), 14);
        assert_eq!(item_light("minecraft:lantern"), 15);
        assert_eq!(item_light("minecraft:soul_torch"), 10);
        assert_eq!(item_light("minecraft:redstone_torch"), 7);
        assert_eq!(item_light("minecraft:glow_berries"), 14);
        assert_eq!(item_light("minecraft:lava_bucket"), 15);
        assert_eq!(item_light("minecraft:waxed_oxidized_copper_lantern"), 15);
        assert_eq!(item_light("minecraft:furnace"), 0);
        assert_eq!(item_light("minecraft:stone"), 0);
    }

    #[test]
    fn creatures_glow() {
        assert_eq!(creature_light("minecraft:blaze", 0, 0), 10);
        assert_eq!(creature_light("minecraft:magma_cube", 1, 0), 6);
        assert_eq!(creature_light("minecraft:magma_cube", 4, 0), 10);
        assert_eq!(creature_light("minecraft:glow_squid", 0, 0), 8);
        assert_eq!(creature_light("minecraft:glow_squid", 0, 100), 0);
        assert_eq!(creature_light("minecraft:glow_squid", 0, 5), 4);
        assert_eq!(creature_light("minecraft:zombie", 0, 0), 0);
        assert!(fire_creature("minecraft:blaze") && !fire_creature("minecraft:magma_cube"));
    }

    #[test]
    fn russian_block_counts() {
        assert_eq!(blocks(16, true), "16 блоков");
        assert_eq!(blocks(24, true), "24 блока");
        assert_eq!(blocks(64, true), "64 блока");
        assert_eq!(blocks(72, true), "72 блока");
        assert_eq!(blocks(96, false), "96 Blocks");
        assert_eq!(blocks(21, true), "21 блок");
        assert_eq!(blocks(12, true), "12 блоков");
    }

    #[test]
    fn brightness_scales_levels() {
        let mut s = Settings::default();
        assert_eq!(s.shown(14.0), 14.0);
        assert_eq!(s.shown(20.0), 15.0);
        s.brightness = 50;
        assert_eq!(s.shown(14.0), 7.0);
    }

    #[test]
    fn facts_follow_the_switches_and_water() {
        let mut s = Settings::default();
        let torch = Fact::new(Who::Mob).hands([(0, false), (14, true)]);
        assert_eq!(torch.held, 14 | 0x80);
        assert_eq!(torch.level(&s, || false), 14);
        assert_eq!(torch.level(&s, || true), 0);
        let lantern = Fact::new(Who::Player).hands([(15, false)]);
        assert_eq!(lantern.level(&s, || true), 15);
        s.flip(Opt::Kind(Kind::Mobs));
        assert_eq!(torch.level(&s, || false), 0);
        let mut burning = Fact::new(Who::Mob);
        burning.burning = true;
        assert_eq!(burning.level(&s, || false), BURNING);
        s.flip(Opt::Kind(Kind::Burning));
        assert_eq!(burning.level(&s, || false), 0);
        let mut blaze = Fact::new(Who::Other);
        blaze.glow = flame(10, true);
        assert_eq!(blaze.level(&s, || false), 10);
        s.water = false;
        assert_eq!(blaze.level(&s, || true), 10);
        let mut tnt = Fact::new(Who::Other);
        tnt.glow = flame(Glow::PrimedTnt.level(), false);
        tnt.glow_kind = Kind::Explosives;
        s.flip(Opt::Kind(Kind::Explosives));
        assert_eq!(tnt.level(&s, || false), 0);
        assert_eq!(Who::from_code(Who::Dropped.code()), Who::Dropped);
    }

    #[test]
    fn key_names_read_as_the_game_writes_them() {
        assert_eq!(key_name("key.keyboard.g"), KeyName::Literal("G".into()));
        assert_eq!(key_name("key.keyboard.7"), KeyName::Literal("7".into()));
        assert_eq!(key_name("key.keyboard.semicolon"), KeyName::Literal(";".into()));
        assert_eq!(key_name("key.keyboard.left.shift"), KeyName::Translate("key.keyboard.left.shift".into()));
        assert_eq!(key_name("key.mouse.left"), KeyName::Translate("key.mouse.left".into()));
        assert_eq!(key_name("key.mouse.4"), KeyName::Button(4));
        assert_eq!(key_name(UNBOUND), KeyName::Translate(UNBOUND.into()));
    }

    #[test]
    fn facts_go_over_the_wire() {
        let mut f = Fact::new(Who::Dropped).hands([(15, false)]);
        f.burning = true;
        f.glow = flame(13, false);
        f.glow_kind = Kind::Explosives;
        let list = vec![(7, [1.5, -60.25, 1e6], f), (9, [0.0; 3], Fact::new(Who::Me))];
        let bytes = encode_facts(&list);
        assert_eq!(bytes.len(), 2 * FACT_BYTES);
        assert_eq!(decode_facts(&bytes), list);
        // A short tail and a bad switch are dropped.
        assert_eq!(decode_facts(&bytes[..FACT_BYTES + 5]).len(), 1);
        let mut bad = bytes.clone();
        bad[32] = 200;
        assert!(decode_facts(&bad).is_empty());
    }

    #[test]
    fn torch_uses_its_block_light_level_at_default_brightness() {
        let mut settings = Settings::default();
        let torch = f32::from(item_light("minecraft:torch"));
        assert_eq!(settings.shown(torch), 14.0);
        assert_eq!(settings.shown(16.0), 15.0);
        settings.brightness = 50;
        assert_eq!(settings.shown(torch), 7.0);
    }
}
