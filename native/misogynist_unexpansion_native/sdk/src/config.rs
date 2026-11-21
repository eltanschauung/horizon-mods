//! `removed_mobs.cfg` in the mod's config folder: which of the listed mobs are taken out of the game.
//! Only these mobs are listed, each `removed` or `kept`; everything else is always kept. Written with
//! the defaults (all removed) when missing, and read again when it changes, within ten seconds.

use std::collections::HashMap;

/// The mobs this mod can take out, all of them taken out by default.
pub const MOBS: [&str; 32] = [
    "dolphin", "fox", "sniffer", "copper_golem", "allay", "turtle", "bee", "parrot", "husk", "stray",
    "polar_bear", "panda", "camel", "llama", "phantom", "frog", "glow_squid", "strider", "camel_husk",
    "armadillo", "axolotl", "happy_ghast", "sulfur_cube", "nautilus", "trader_llama", "zombie_nautilus",
    "bogged", "parched", "breeze", "creaking", "tadpole", "goat",
];

pub const FILE: &str = "removed_mobs.cfg";

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Each listed mob (no namespace), and whether it is removed.
    pub removed: HashMap<&'static str, bool>,
}

impl Default for Config {
    fn default() -> Config {
        Config { removed: MOBS.iter().map(|&m| (m, true)).collect() }
    }
}

impl Config {
    /// Whether the mob type `kind` (`minecraft:fox`) is taken out.
    pub fn removes(&self, kind: &str) -> bool {
        kind.strip_prefix("minecraft:").and_then(|k| self.removed.get(k)).copied().unwrap_or(false)
    }

    pub fn text(&self) -> String {
        let mut out = String::from(
            "# Minecraft Misogynist Unexpansion: the mobs it takes out of the game.\n\
             # removed: none spawn, and those already in a world are gone when they load.\n\
             # kept: back as in vanilla. Saved changes take hold within ten seconds.\n\
             # Delete this file to get the defaults back.\n\n",
        );
        for m in MOBS {
            let removed = self.removed.get(m).copied().unwrap_or(true);
            out += &format!("{m} = {}\n", if removed { "removed" } else { "kept" });
        }
        out
    }

    /// Reads a file in `text`'s form: a line it doesn't understand keeps its mob's default.
    pub fn parse(text: &str) -> Config {
        let mut c = Config::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else { continue };
            let Some(slot) = c.removed.get_mut(key.trim()) else { continue };
            match value.trim().to_ascii_lowercase().as_str() {
                "removed" | "remove" | "on" | "true" | "yes" => *slot = true,
                "kept" | "keep" | "off" | "false" | "no" => *slot = false,
                _ => {}
            }
        }
        c
    }
}

/// The config and its file, with when it was last read.
pub struct Loaded {
    pub config: Config,
    loaded: bool,
    ticks: u32,
}

impl Loaded {
    pub fn new() -> Loaded {
        Loaded { config: Config::default(), loaded: false, ticks: 0 }
    }

    /// Every 200 calls: writes the defaults if the file is missing, and reads it again if it changed.
    pub fn refresh(&mut self, files: &dyn mod_api::ConfigFiles, instance: &std::path::Path) {
        self.ticks = self.ticks.wrapping_add(1);
        if self.loaded && self.ticks % 200 != 0 {
            return;
        }
        let current = mod_api::ConfigPath::new(&format!("misogynist_unexpansion_native/{FILE}")).unwrap();
        match files.read(&current) {
            Ok(bytes) => {
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    self.config = Config::parse(text);
                }
            }
            Err(_) if matches!(files.stat(&current), Ok(None)) => {
                use std::io::Read;
                let old = instance.join("config").join(FILE);
                let legacy = std::fs::File::open(old).ok().and_then(|file| {
                    let mut bytes = Vec::new();
                    file.take(65_537).read_to_end(&mut bytes).ok()?;
                    (bytes.len() <= 65_536).then(|| String::from_utf8(bytes).ok()).flatten()
                });
                self.config = legacy.as_deref().map_or_else(Config::default, Config::parse);
                let _ = files.write(&current, self.config.text().as_bytes());
            }
            Err(_) => {}
        }
        self.loaded = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_remove_every_listed_mob_and_read_back() {
        let c = Config::default();
        assert!(MOBS.iter().all(|m| c.removes(&format!("minecraft:{m}"))));
        assert_eq!(Config::parse(&c.text()), c);
    }

    #[test]
    fn only_the_listed_mobs_can_be_removed() {
        let c = Config::parse("zombie = removed\ncreeper = removed\nfox = kept\n");
        assert!(!c.removes("minecraft:zombie"));
        assert!(!c.removes("minecraft:creeper"));
        assert!(!c.removes("minecraft:fox"));
        assert!(c.removes("minecraft:goat"));
        assert!(!c.text().contains("\nzombie ="));
        assert!(!c.text().contains("\ncreeper ="));
    }
}
