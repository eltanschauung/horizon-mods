//! `spawns.cfg` in the mod's config folder: whether each mob spawns on its own, how often and in what
//! groups, and the spawner's rate and cap. Written with the defaults when missing; read again when
//! it changes, so an edit takes hold in game within ten seconds.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::mobs::{self, Light, MOBS};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    pub enabled: bool,
    pub weight: u32,
    pub min: u32,
    pub max: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// How many tries a second for each player, about; 1.25 by default.
    pub rate: f64,
    /// How many of our mobs may be within 64 blocks of a player.
    pub cap: usize,
    pub rules: HashMap<String, Rule>,
}

pub const FILE: &str = "spawns.cfg";

impl Default for Config {
    fn default() -> Config {
        let rules = MOBS
            .iter()
            .filter_map(|m| m.spawn.map(|s| (m.id.to_owned(), Rule { enabled: true, weight: s.weight, min: s.min, max: s.max })))
            .collect();
        Config { rate: 1.25, cap: 12, rules }
    }
}

impl Config {
    /// The rule for a mob that spawns on its own, as this file has it.
    pub fn rule(&self, mob: &mobs::Mob) -> Option<Rule> {
        let s = mob.spawn?;
        let r = self.rules.get(mob.id).copied().unwrap_or(Rule { enabled: true, weight: s.weight, min: s.min, max: s.max });
        (r.enabled && r.weight > 0).then_some(Rule { max: r.max.max(r.min), ..r })
    }

    pub fn text(&self) -> String {
        let mut out = String::from(
            "# Touhou Mobs: how its mobs spawn on their own. Saved changes take hold within ten seconds.\n\
             # Delete this file to get the defaults back.\n\
             #\n\
             # rate: tries a second for each player, about (each try picks one mob by weight, and fails if\n\
             #   the spot isn't in that mob's biomes or its light). cap: the most of these mobs within 64\n\
             #   blocks of a player.\n",
        );
        out += &format!("rate = {}\ncap = {}\n\n", self.rate, self.cap);
        out += "# mob = on/off weight smallest-group largest-group    (where and in what light, for reference)\n";
        for m in MOBS {
            let Some(s) = m.spawn else { continue };
            let r = self.rules.get(m.id).copied().unwrap_or(Rule { enabled: true, weight: s.weight, min: s.min, max: s.max });
            let light = match s.light {
                Light::Fairy => "daylight, open sky, on grass, dirt or sand",
                Light::Dark => "in the dark",
                Light::Any => "any light",
                Light::Rare => "any light, 2 tries in 100",
            };
            let where_ = s.biomes.iter().take(6).map(|b| b.replace("biomesoplenty:", "BoP ")).collect::<Vec<_>>().join(", ");
            let more = if s.biomes.len() > 6 { format!(" and {} more", s.biomes.len() - 6) } else { String::new() };
            out += &format!(
                "{} = {} {} {} {}    # {}; {}{}\n",
                m.id,
                if r.enabled { "on" } else { "off" },
                r.weight,
                r.min,
                r.max,
                light,
                where_,
                more
            );
        }
        out
    }

    /// Reads a file in `text`'s form: what it doesn't say, or says wrong, keeps its default.
    pub fn parse(text: &str) -> Config {
        let mut c = Config::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else { continue };
            let (key, value) = (key.trim(), value.trim());
            match key {
                "rate" => {
                    if let Ok(v) = value.parse::<f64>()
                        && v.is_finite()
                        && v >= 0.0
                    {
                        c.rate = v.min(20.0);
                    }
                }
                "cap" => {
                    if let Ok(v) = value.parse() {
                        c.cap = v;
                    }
                }
                _ => {
                    let Some(rule) = c.rules.get_mut(key) else { continue };
                    let words: Vec<&str> = value.split_whitespace().collect();
                    if let Some(w) = words.first() {
                        rule.enabled = !matches!(*w, "off" | "false" | "no" | "0");
                    }
                    let nums: Vec<u32> = words.iter().skip(1).filter_map(|w| w.parse().ok()).collect();
                    if let Some(&w) = nums.first() {
                        rule.weight = w;
                    }
                    if let Some(&lo) = nums.get(1) {
                        rule.min = lo;
                    }
                    if let Some(&hi) = nums.get(2) {
                        rule.max = hi;
                    }
                }
            }
        }
        c
    }
}

/// The config and the file it came from, with the time it was last read.
pub struct Loaded {
    pub config: Config,
    pub file: Option<(PathBuf, Option<SystemTime>)>,
    pub ticks: u32,
}

impl Loaded {
    pub fn new() -> Loaded {
        Loaded { config: Config::default(), file: None, ticks: 0 }
    }

    /// Every 200 ticks: writes the defaults if the file is missing, and reads it again if it changed.
    pub fn refresh(&mut self, dir: impl FnOnce() -> Option<PathBuf>) {
        self.ticks += 1;
        if self.file.is_some() && self.ticks % 200 != 0 {
            return;
        }
        let path = match &self.file {
            Some((p, _)) => p.clone(),
            None => match dir() {
                Some(d) => d.join(FILE),
                None => return,
            },
        };
        if !path.exists() {
            let _ = std::fs::write(&path, Config::default().text());
        }
        let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if self.file.as_ref().is_some_and(|(_, t)| *t == modified) {
            return;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            self.config = Config::parse(&text);
        }
        self.file = Some((path, modified));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_read_back_as_written() {
        let c = Config::default();
        assert_eq!(Config::parse(&c.text()), c);
    }

    #[test]
    fn a_line_changes_its_mob_alone() {
        let c = Config::parse("rate = 0.5\ncap = 4\nfairy = off 30 1 6\ncirno = on 9 2 3\nnobody = on 1 1 1\n");
        assert_eq!(c.rate, 0.5);
        assert_eq!(c.cap, 4);
        let fairy = MOBS.iter().find(|m| m.id == "fairy").unwrap();
        let cirno = MOBS.iter().find(|m| m.id == "cirno").unwrap();
        assert_eq!(c.rule(fairy), None);
        assert_eq!(c.rule(cirno), Some(Rule { enabled: true, weight: 9, min: 2, max: 3 }));
        let rumia = MOBS.iter().find(|m| m.id == "rumia").unwrap();
        assert_eq!(c.rule(rumia).map(|r| r.weight), Some(4));
    }

    #[test]
    fn eggs_only_mobs_have_no_line() {
        let text = Config::default().text();
        assert!(!text.contains("\nyukari ="));
        assert!(text.contains("\nfairy = on 30 1 6"));
    }
}
