//! The old water: beta's look, after Nostalgic Tweaks' old water by Adrenix (LGPL-3.0; the fog
//! colours and distance are its numbers, the textures are this mod's own).
//!
//! - The textures: the content mod's block atlas puts its beta-style still and flowing water
//!   (`misogynist_unexpansion:block/old_water_still`, `_flow`, drawn by `gen_content.py`) in place
//!   of vanilla's water sprites. Nothing here: they come and go with the content mod.
//! - `colour`: one water colour everywhere, no biome tint: white, so the old textures' own blue
//!   shows (`biome_colors`).
//! - `fog`: water fog as in beta: dark blue, darker the less sky reaches you, and thick, gone
//!   16 blocks out (`compute_fog_color`, `render_fog`).
//!
//! `old_water.cfg` in the mod's config folder switches the two, both on by default. Written with
//! the defaults when missing, and read again when it changes, within ten seconds.

pub const FILE: &str = "old_water.cfg";

/// Old water fog: from 16 blocks behind the eye to 16 in front (Nostalgic Tweaks' old density).
pub const FOG_START: f32 = -16.0;
pub const FOG_END: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub colour: bool,
    pub fog: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { colour: true, fog: true }
    }
}

impl Settings {
    pub fn text(&self) -> String {
        let on = |b: bool| if b { "on" } else { "off" };
        format!(
            "# Minecraft Misogynist Unexpansion: the old water, as in beta. Each on or off; saved\n\
             # changes take hold within ten seconds. (The old textures come with the content mod.)\n\
             # colour: one water colour everywhere, no biome tint.\n\
             # fog: dark, thick water fog, darker the less sky reaches you.\n\
             # Delete this file to get the defaults back.\n\n\
             colour = {}\nfog = {}\n",
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
            let slot = match key.trim() {
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

    /// Every biome's water colour, or None to keep the biome's own.
    pub fn water_tint(&self) -> Option<u32> {
        self.colour.then_some(0xFF_FFFF)
    }
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
        Loaded { settings: Settings { colour: true, fog: true }, loaded: false, ticks: 0 }
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
        assert!(s.colour && s.fog);
        assert_eq!(Settings::parse(&s.text()), s);
        let off = Settings { colour: false, fog: true };
        assert_eq!(Settings::parse(&off.text()), off);
    }

    #[test]
    fn unknown_lines_keep_the_defaults() {
        let s = Settings::parse("fog = maybe\nwaves = off\ncolor = off # spelt the other way\n");
        assert_eq!(s, Settings { colour: false, fog: true });
    }

    #[test]
    fn the_tint_is_white_or_the_biomes() {
        assert_eq!(Settings::default().water_tint(), Some(0xFF_FFFF));
        assert_eq!(Settings { colour: false, fog: true }.water_tint(), None);
    }

    #[test]
    fn the_fog_darkens_away_from_the_sky() {
        let blue = |l| fog_rgb(l)[2];
        assert!(blue(15) > blue(10) && blue(10) > blue(7) && blue(7) > blue(4) && blue(4) > blue(0));
        assert!(fog_rgb(15).iter().chain(fog_rgb(0).iter()).all(|c| (0.0..=1.0).contains(c)));
    }
}
