//! The old water: beta's look, after Nostalgic Tweaks' old water by Adrenix (LGPL-3.0; the fog
//! colours and distance are its numbers, the textures are this mod's own).
//!
//! - The textures: the content mod's block atlas puts its beta-style still and flowing water
//!   (`misogynist_unexpansion:block/old_water_still`, `_flow`, drawn by `gen_content.py`) in place
//!   of vanilla's water sprites. Nothing here: they come and go with the content mod. They are
//!   grey, as vanilla's are, so the colour comes from the tint whichever texture is drawn (a white
//!   tint on vanilla's grey water, where the atlas swap didn't take, made it milky).
//! - `colour`: one water colour everywhere, no biome tint: beta's blue, [`OLD_WATER`]
//!   (`biome_colors`); only warm and lukewarm oceans lean a little ([`WARM_LEAN`]) towards their
//!   own lighter blue.
//! - `fog`: water fog as in beta: dark blue, darker the less sky reaches you, and thick, gone
//!   16 blocks out (`compute_fog_color`, `render_fog`).
//!
//! - `textures`: the textures above, as a resource pack the content mod carries
//!   (`assets/misogynist_unexpansion/resourcepacks/old_water`), turned on as the game starts while they're wanted.
//!
//! `old_water.cfg` in the mod's config folder switches them on this client: `water = old` or
//! `new` (the game's own water, none of the three), and each part on its own. All on by default.
//! Written with the defaults when missing, and read again when it changes, within ten seconds;
//! the textures take a restart.

pub const FILE: &str = "old_water.cfg";

/// The content mod's pack with the old textures in place of vanilla's water sprites.
pub const PACK: &str = "misogynist_unexpansion:assets/misogynist_unexpansion/resourcepacks/old_water";

/// Beta's water blue, the one water colour everywhere.
pub const OLD_WATER: u32 = 0x40_64F0;

/// Old water fog: from 16 blocks behind the eye to 16 in front (Nostalgic Tweaks' old density).
pub const FOG_START: f32 = -16.0;
pub const FOG_END: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `water = old`: false is the game's own water, whatever the parts say.
    pub old: bool,
    pub textures: bool,
    pub colour: bool,
    pub fog: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings::ALL_OLD
    }
}

impl Settings {
    pub const ALL_OLD: Settings = Settings { old: true, textures: true, colour: true, fog: true };

    /// Whether each part is drawn: on, with the old water.
    pub fn textures_on(&self) -> bool {
        self.old && self.textures
    }
    pub fn colour_on(&self) -> bool {
        self.old && self.colour
    }
    pub fn fog_on(&self) -> bool {
        self.old && self.fog
    }

    pub fn text(&self) -> String {
        let on = |b: bool| if b { "on" } else { "off" };
        format!(
            "# Minecraft Misogynist Unexpansion: the water on this client. Saved changes take hold\n\
             # within ten seconds; the textures take a restart.\n\
             # water: old, as in beta, or new, the game's own water (then the three below are off).\n\
             # With old water, each part on or off:\n\
             # textures: beta-style still and flowing water.\n\
             # colour: one water colour everywhere, no biome tint.\n\
             # fog: dark, thick water fog, darker the less sky reaches you.\n\
             # Delete this file to get the defaults back.\n\n\
             water = {}\ntextures = {}\ncolour = {}\nfog = {}\n",
            if self.old { "old" } else { "new" },
            on(self.textures),
            on(self.colour),
            on(self.fog),
        )
    }

    /// Reads a file in `text`'s form: a line it doesn't understand keeps its default.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else { continue };
            if key.trim() == "water" {
                match value.trim().to_ascii_lowercase().as_str() {
                    "old" | "beta" | "on" => s.old = true,
                    "new" | "modern" | "off" => s.old = false,
                    _ => {}
                }
                continue;
            }
            let slot = match key.trim() {
                "textures" | "texture" => &mut s.textures,
                "colour" | "color" | "tint" => &mut s.colour,
                "fog" => &mut s.fog,
                _ => continue,
            };
            match value.trim().to_ascii_lowercase().as_str() {
                "on" | "true" | "yes" => *slot = true,
                "off" | "false" | "no" => *slot = false,
                _ => {}
            }
        }
        s
    }

    /// `biome`'s water colour, given its own (`own`), or None to keep the biome's own. Beta's blue
    /// everywhere, but warm and lukewarm oceans lean a little towards their own lighter blue.
    pub fn water_tint(&self, biome: &str, own: u32) -> Option<u32> {
        if !self.colour_on() {
            return None;
        }
        let name = biome.rsplit(':').next().unwrap_or(biome);
        Some(match name {
            "warm_ocean" | "deep_warm_ocean" | "lukewarm_ocean" | "deep_lukewarm_ocean" => {
                blend(OLD_WATER, own, WARM_LEAN)
            }
            _ => OLD_WATER,
        })
    }
}

/// How far warm and lukewarm oceans' water leans from beta's blue towards their own colour.
pub const WARM_LEAN: f32 = 0.3;

/// `a` moved `t` (0 to 1) of the way to `b`, channel by channel (0xRRGGBB).
fn blend(a: u32, b: u32, t: f32) -> u32 {
    (0..3).fold(0, |out, i| {
        let shift = 16 - 8 * i;
        let (x, y) = (((a >> shift) & 0xFF) as f32, ((b >> shift) & 0xFF) as f32);
        out | (((x + (y - x) * t).round() as u32) << shift)
    })
}

/// Old water fog's colour at `sky_light` (0 to 15) where the eye is: Nostalgic Tweaks' table.
pub fn fog_rgb(sky_light: i32) -> [f32; 3] {
    let [r, g, b]: [u8; 3] = match sky_light {
        12.. => [9, 16, 73],
        9..=11 => [6, 11, 58],
        6..=8 => [5, 8, 50],
        3..=5 => [5, 7, 45],
        _ => [4, 5, 41],
    };
    [r, g, b].map(|c| f32::from(c) / 255.0)
}

/// The settings, read from the mod's config files.
pub struct Loaded {
    pub settings: Settings,
    loaded: bool,
    ticks: u32,
}

impl Loaded {
    pub const fn new() -> Loaded {
        Loaded { settings: Settings::ALL_OLD, loaded: false, ticks: 0 }
    }

    /// Every 200 calls: writes the defaults if the file is missing, and reads it again. True when
    /// the settings changed.
    pub fn refresh(&mut self, files: &dyn mod_api::ConfigFiles) -> bool {
        self.ticks = self.ticks.wrapping_add(1);
        if self.loaded && self.ticks % 200 != 0 {
            return false;
        }
        let path = mod_api::ConfigPath::new(&format!("misogynist_unexpansion_native/{FILE}")).unwrap();
        let before = self.settings;
        match files.read(&path) {
            Ok(bytes) => {
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    self.settings = Settings::parse(text);
                }
            }
            Err(_) if matches!(files.stat(&path), Ok(None)) => {
                let _ = files.write(&path, Settings::default().text().as_bytes());
            }
            Err(_) => {}
        }
        self.loaded = true;
        self.settings != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_all_on_and_read_back() {
        let s = Settings::default();
        assert!(s.textures_on() && s.colour_on() && s.fog_on());
        assert_eq!(Settings::parse(&s.text()), s);
        let off = Settings { colour: false, ..Settings::ALL_OLD };
        assert_eq!(Settings::parse(&off.text()), off);
        let new = Settings { old: false, ..Settings::ALL_OLD };
        assert_eq!(Settings::parse(&new.text()), new);
    }

    #[test]
    fn unknown_lines_keep_the_defaults() {
        let s = Settings::parse("fog = maybe\nwaves = off\ncolor = off # spelt the other way\n");
        assert_eq!(s, Settings { colour: false, ..Settings::ALL_OLD });
    }

    #[test]
    fn the_tint_is_beta_blue_or_the_biomes() {
        let on = Settings::default();
        assert_eq!(on.water_tint("minecraft:plains", 0x3F76E4), Some(OLD_WATER));
        assert_eq!(on.water_tint("minecraft:swamp", 0x617B64), Some(OLD_WATER));
        assert_ne!(OLD_WATER, 0xFF_FFFF);
        assert_eq!(Settings { colour: false, ..Settings::ALL_OLD }.water_tint("minecraft:plains", 0x3F76E4), None);
    }

    #[test]
    fn new_water_turns_every_part_off() {
        // A file from before `water` keeps the old water; `water = new` wins over the parts.
        assert_eq!(Settings::parse("colour = on\nfog = on\n"), Settings::ALL_OLD);
        let new = Settings::parse("textures = on\ncolour = on\nwater = new\nfog = on\n");
        assert!(!new.textures_on() && !new.colour_on() && !new.fog_on());
        assert_eq!(new.water_tint("minecraft:plains", 0x3F76E4), None);
        let old_flat = Settings::parse("water = old\ntextures = off\n");
        assert!(!old_flat.textures_on() && old_flat.colour_on() && old_flat.fog_on());
    }

    #[test]
    fn warm_oceans_lean_a_little_to_their_own_blue() {
        let warm = 0x43D5EE;
        let tint = Settings::default().water_tint("minecraft:warm_ocean", warm).unwrap();
        let g = |c: u32| (c >> 8) & 0xFF;
        assert!(g(OLD_WATER) < g(tint) && g(tint) < g(warm));
        // Still mostly beta's blue.
        assert!(g(tint) - g(OLD_WATER) < g(warm) - g(tint));
        assert_ne!(Settings::default().water_tint("minecraft:deep_lukewarm_ocean", 0x45ADF2), Some(OLD_WATER));
        assert_eq!(blend(0x000000, 0xFFFFFF, 0.0), 0);
        assert_eq!(blend(0x000000, 0xFFFFFF, 1.0), 0xFFFFFF);
    }

    #[test]
    fn the_fog_darkens_away_from_the_sky() {
        let blue = |l| fog_rgb(l)[2];
        assert!(blue(15) > blue(10) && blue(10) > blue(7) && blue(7) > blue(4) && blue(4) > blue(0));
        assert!(fog_rgb(15).iter().chain(fog_rgb(0).iter()).all(|c| (0.0..=1.0).contains(c)));
    }
}
