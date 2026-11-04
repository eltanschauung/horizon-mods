// SPDX-License-Identifier: MIT
// Port of LambdAurora's MIT revision aecdeccc27aaaac38995c577b2605476b41d8ba4.
// Horizon integration and extensions are documented in the accompanying README.
use serde_json::Value;
use std::{collections::HashMap, time::Duration};

pub const ITEMS: &[&[u8]] = &[
    br##"{
  "item": "adorn:black_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:blue_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:brown_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:cyan_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:gray_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:green_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:light_blue_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:light_gray_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:lime_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:magenta_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:orange_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:pink_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:purple_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:red_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:stone_torch",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:white_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "adorn:yellow_table_lamp",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "minecraft:blaze_powder",
  "luminance": 10
}"##,
    br##"{
  "item": "minecraft:blaze_rod",
  "luminance": 10
}"##,
    br##"{
  "item": "minecraft:campfire",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "minecraft:fire_charge",
  "luminance": 10,
  "water_sensitive": true
}"##,
    br##"{
  "item": "minecraft:glow_berries",
  "luminance": 14
}"##,
    br##"{
  "item": "minecraft:glow_ink_sac",
  "luminance": 12
}"##,
    br##"{
  "item": "minecraft:glowstone_dust",
  "luminance": 8
}"##,
    br##"{
  "item": "minecraft:lava_bucket",
  "luminance": "minecraft:lava",
  "water_sensitive": true
}"##,
    br##"{
  "item": "minecraft:nether_star",
  "luminance": 8
}"##,
    br##"{
  "item": "minecraft:prismarine_crystals",
  "luminance": 8
}"##,
    br##"{
  "item": "minecraft:redstone_torch",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "minecraft:soul_campfire",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "minecraft:soul_torch",
  "luminance": "block",
  "water_sensitive": true
}"##,
    br##"{
  "item": "minecraft:spectral_arrow",
  "luminance": 10
}"##,
    br##"{
  "item": "minecraft:torch",
  "luminance": "block",
  "water_sensitive": true
}"##,
];

pub const ID: &str = "lambdynamiclights_horizon";
pub const VERSION: &str = "0.1.0";
pub const DESCRIPTION: &str = "LambDynamicLights for Horizon: held and dropped items, fire, creatures and projectiles. Client-side; by LambdAurora, ported for Horizon.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    Fastest,
    Fast,
    Fancy,
}
impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Fastest => "fastest",
            Self::Fast => "fast",
            Self::Fancy => "fancy",
        }
    }
    pub fn delay(self) -> Duration {
        Duration::from_millis(match self {
            Self::Fastest => 500,
            Self::Fast => 250,
            _ => 0,
        })
    }
    fn next(self) -> Self {
        match self {
            Self::Off => Self::Fastest,
            Self::Fastest => Self::Fast,
            Self::Fast => Self::Fancy,
            Self::Fancy => Self::Off,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Explosive {
    Off,
    Simple,
    Fancy,
}
impl Explosive {
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Simple => "simple",
            Self::Fancy => "fancy",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Off => Self::Simple,
            Self::Simple => Self::Fancy,
            Self::Fancy => Self::Off,
        }
    }
    pub fn level(self, progress: f32, flashing: bool) -> u8 {
        if !progress.is_finite() || progress <= 0.0 {
            return 0;
        }
        match self {
            Self::Off => 0,
            Self::Simple => {
                if flashing {
                    15
                } else {
                    0
                }
            }
            Self::Fancy => (progress.clamp(0.0, 1.0) * 15.0) as u8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub mode: Mode,
    pub entities: bool,
    pub own: bool,
    pub water: bool,
    pub creeper: Explosive,
    pub tnt: Explosive,
    pub frames: bool,
    pub particles: bool,
    pub beams: bool,
}
impl Settings {
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let flag = |key| get(key).is_none_or(|v| v.trim() != "false");
        let explosive = |key, default| match get(key).as_deref().map(str::trim) {
            Some("off") => Explosive::Off,
            Some("simple") => Explosive::Simple,
            Some("fancy") => Explosive::Fancy,
            _ => default,
        };
        Self {
            mode: match get("mode").as_deref().map(str::trim) {
                Some("off") => Mode::Off,
                Some("fastest") => Mode::Fastest,
                Some("fast") => Mode::Fast,
                _ => Mode::Fancy,
            },
            entities: flag("entities"),
            own: flag("self"),
            water: flag("water_sensitive_check"),
            creeper: explosive("creeper", Explosive::Simple),
            tnt: explosive("tnt", Explosive::Off),
            frames: flag("block_entities"),
            particles: flag("particles"),
            beams: flag("beams"),
        }
    }
    pub fn labels(&self) -> Vec<String> {
        let on = |b| if b { "ON" } else { "OFF" };
        vec![
            format!("Lighting: {}", self.mode.name()),
            format!("Entity lights: {}", on(self.entities)),
            format!("Own light: {}", on(self.own)),
            format!("Water-sensitive lights: {}", on(self.water)),
            format!("Creeper lighting: {}", self.creeper.name()),
            format!("TNT lighting: {}", self.tnt.name()),
            format!("Frames and displays: {}", on(self.frames)),
            format!("Firefly / sonic boom lights: {}", on(self.particles)),
            format!("Beacon and guardian beams: {}", on(self.beams)),
        ]
    }
    pub fn click(&mut self, button: usize) {
        match button {
            0 => self.mode = self.mode.next(),
            1 => self.entities = !self.entities,
            2 => self.own = !self.own,
            3 => self.water = !self.water,
            4 => self.creeper = self.creeper.next(),
            5 => self.tnt = self.tnt.next(),
            6 => self.frames = !self.frames,
            7 => self.particles = !self.particles,
            8 => self.beams = !self.beams,
            _ => {}
        }
    }
    pub fn stored(&self) -> Vec<(&'static str, String)> {
        vec![
            ("mode", self.mode.name().into()),
            ("entities", self.entities.to_string()),
            ("self", self.own.to_string()),
            ("water_sensitive_check", self.water.to_string()),
            ("creeper", self.creeper.name().into()),
            ("tnt", self.tnt.name().into()),
            ("block_entities", self.frames.to_string()),
            ("particles", self.particles.to_string()),
            ("beams", self.beams.to_string()),
        ]
    }
}

#[derive(Clone, Debug)]
pub enum Emission {
    Fixed(u8),
    BlockSelf,
    Block(String),
}
#[derive(Clone, Debug)]
pub struct Rule {
    pub emission: Emission,
    pub water_sensitive: bool,
}
#[derive(Default)]
pub struct Rules {
    items: HashMap<String, Rule>,
    entities: HashMap<String, u8>,
}
fn names(value: &Value) -> Option<Vec<String>> {
    if let Some(s) = value.as_str() {
        return Some(vec![s.to_owned()]);
    }
    value
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect()
}
fn level(value: &Value) -> Option<u8> {
    let n = value.as_u64()?;
    (n <= 15).then_some(n as u8)
}
impl Rules {
    /// Atomic per document: invalid selectors/values never replace an earlier valid rule.
    /// Accepts the MIT item schema and the newer simple match/items schema.
    pub fn item_document(&mut self, bytes: &[u8]) -> bool {
        let Ok(v) = serde_json::from_slice::<Value>(bytes) else {
            return false;
        };
        let selector = v.get("item").or_else(|| v.get("match")?.get("items"));
        let Some(ids) = selector.and_then(names) else {
            return false;
        };
        if ids.is_empty() || ids.iter().any(|s| !valid_id(s)) {
            return false;
        }
        let Some(lum) = v.get("luminance") else {
            return false;
        };
        let emission = if let Some(n) = level(lum) {
            Emission::Fixed(n)
        } else if lum.as_str() == Some("block")
            || lum.get("type").and_then(Value::as_str) == Some("block_self")
        {
            Emission::BlockSelf
        } else if let Some(id) = lum.as_str().filter(|id| valid_id(id)) {
            Emission::Block(id.to_owned())
        } else if lum.get("type").and_then(Value::as_str) == Some("block") {
            let Some(id) = lum
                .get("block")
                .and_then(Value::as_str)
                .filter(|s| valid_id(s))
            else {
                return false;
            };
            Emission::Block(id.to_owned())
        } else {
            return false;
        };
        let water_sensitive = match v.get("water_sensitive") {
            None => false,
            Some(Value::Bool(b)) => *b,
            _ => return false,
        };
        for id in ids {
            self.items.insert(
                id,
                Rule {
                    emission: emission.clone(),
                    water_sensitive,
                },
            );
        }
        true
    }
    pub fn entity_document(&mut self, bytes: &[u8]) -> bool {
        let Ok(v) = serde_json::from_slice::<Value>(bytes) else {
            return false;
        };
        let Some(ids) = v.get("match").and_then(|m| m.get("type")).and_then(names) else {
            return false;
        };
        let Some(n) = v.get("luminance").and_then(level) else {
            return false;
        };
        if ids.is_empty() || ids.iter().any(|s| !valid_id(s)) {
            return false;
        }
        for id in ids {
            self.entities.insert(id, n);
        }
        true
    }
    /// Block items fall back to the actual loaded registry, including content mods.
    pub fn item(&self, item: &str, wet: bool, water_check: bool, block: impl Fn(&str) -> u8) -> u8 {
        match self.items.get(item) {
            Some(rule) if wet && water_check && rule.water_sensitive => 0,
            Some(rule) => match &rule.emission {
                Emission::Fixed(n) => *n,
                Emission::BlockSelf => block(item).min(15),
                Emission::Block(id) => block(id).min(15),
            },
            None => block(item).min(15),
        }
    }
    pub fn entity(&self, kind: &str, natural: u8) -> u8 {
        self.entities.get(kind).copied().unwrap_or(natural).min(15)
    }
}
fn valid_id(s: &str) -> bool {
    let Some((ns, path)) = s.split_once(':') else {
        return false;
    };
    !ns.is_empty()
        && !path.is_empty()
        && ns
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-".contains(&c))
        && path
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"/_.-".contains(&c))
}

/// Brighter equipment wins; an extinguished item cannot conceal an entity's own glow.
pub fn combine(natural: u8, equipment: u8, burning: bool) -> u8 {
    natural
        .max(equipment)
        .max(if burning { 15 } else { 0 })
        .min(15)
}
pub fn glow_squid(dark_ticks: i32) -> u8 {
    ((1.0 - dark_ticks as f32 / 10.0).clamp(0.0, 1.0) * 12.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_mit_reference_documents_load() {
        let mut rules = Rules::default();
        for bytes in ITEMS {
            assert!(
                rules.item_document(bytes),
                "{}",
                String::from_utf8_lossy(bytes)
            );
        }
        assert_eq!(rules.item("minecraft:blaze_rod", false, true, |_| 0), 10);
        assert_eq!(rules.item("minecraft:glow_berries", false, true, |_| 0), 14);
        assert_eq!(
            rules.item("minecraft:lava_bucket", false, true, |id| {
                if id == "minecraft:lava" { 15 } else { 0 }
            }),
            15
        );
    }
    #[test]
    fn settings_round_trip_and_upstream_delays() {
        let mut s = Settings::read(|_| None);
        assert_eq!(s.mode, Mode::Fancy);
        assert_eq!(s.creeper, Explosive::Simple);
        assert_eq!(s.tnt, Explosive::Off);
        for button in 0..9 {
            s.click(button);
        }
        let pairs = s.stored();
        assert_eq!(
            Settings::read(|k| pairs
                .iter()
                .find(|(key, _)| *key == k)
                .map(|(_, v)| v.clone())),
            s
        );
        assert_eq!(Mode::Fast.delay().as_millis(), 250);
        assert_eq!(Mode::Fastest.delay().as_millis(), 500);
    }
    #[test]
    fn water_block_modded_and_pack_precedence() {
        let mut rules = Rules::default();
        assert!(rules.item_document(
            br#"{"item":"minecraft:torch","luminance":"block","water_sensitive":true}"#
        ));
        assert_eq!(rules.item("minecraft:torch", true, true, |_| 14), 0);
        assert_eq!(rules.item("minecraft:torch", true, false, |_| 14), 14);
        assert_eq!(rules.item("test:lantern", false, true, |_| 11), 11);
        assert!(
            rules.item_document(
                br#"{"match":{"items":["minecraft:torch","test:rod"]},"luminance":6}"#
            )
        );
        assert_eq!(rules.item("minecraft:torch", true, true, |_| 14), 6);
        assert!(!rules.item_document(br#"{"item":"minecraft:torch","luminance":999}"#));
        assert_eq!(rules.item("minecraft:torch", false, true, |_| 14), 6);
        assert!(!rules.item_document(br#"{"item":["minecraft:torch","bad"],"luminance":0}"#));
        assert_eq!(rules.item("minecraft:torch", false, true, |_| 14), 6);
        assert!(rules.item_document(
            br#"{"item":"test:bucket","luminance":{"type":"block","block":"minecraft:lava"}}"#
        ));
        assert_eq!(
            rules.item("test:bucket", false, true, |id| if id == "minecraft:lava" {
                15
            } else {
                0
            }),
            15
        );
    }
    #[test]
    fn creature_states_and_entity_overrides() {
        assert_eq!([0, 5, 10, 100].map(glow_squid), [12, 6, 0, 0]);
        assert_eq!(combine(8, 14, false), 14);
        assert_eq!(combine(0, 0, true), 15);
        assert_eq!(Explosive::Off.level(1.0, true), 0);
        assert_eq!(Explosive::Simple.level(0.8, true), 15);
        assert_eq!(Explosive::Simple.level(0.8, false), 0);
        assert_eq!(Explosive::Fancy.level(0.5, false), 7);
        let mut rules = Rules::default();
        assert!(rules.entity_document(br#"{"match":{"type":"minecraft:blaze"},"luminance":0}"#));
        assert_eq!(rules.entity("minecraft:blaze", 10), 0);
        assert_eq!(rules.entity("test:glow", 9), 9);
    }
}
