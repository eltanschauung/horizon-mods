//! The other old looks, after Nostalgic Tweaks (Adrenix, LGPL-3.0; written anew, beta's numbers):
//!
//! - `fog`: the old distance fog. The world fades from a quarter of the render distance out to
//!   its end, as in alpha and beta, not only at the far edge.
//! - `void_fog`: beta's void fog in the Overworld. Near the bottom of the world, where no sky
//!   light reaches, the fog closes in (down to five blocks) and turns black.
//! - `version`: the version text in the top left of the HUD, `version_text` (Minecraft Beta 1.7.3
//!   by default), as the old game showed it. F3 and F1 hide it.
//! - `item_name`: the item's name over the hotbar when you pick another slot, which the old game
//!   never showed. Off by default.
//! - `xp_bar`: the experience bar. The old game had none; it stays by default (enchanting needs
//!   it). A horse's jump bar stays either way.
//!
//! `nostalgic.cfg`, beside `old_lighting.cfg`, switches them. Written with the defaults when
//! missing, and read again every two seconds.

pub const FILE: &str = "misogynistic_lighting/nostalgic.cfg";

pub const DEFAULT_VERSION_TEXT: &str = "Minecraft Beta 1.7.3";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tweaks {
    pub fog: bool,
    pub void_fog: bool,
    pub version: bool,
    pub version_text: String,
    pub item_name: bool,
    pub xp_bar: bool,
}

impl Default for Tweaks {
    fn default() -> Tweaks {
        Tweaks {
            fog: true,
            void_fog: true,
            version: true,
            version_text: DEFAULT_VERSION_TEXT.into(),
            item_name: false,
            xp_bar: true,
        }
    }
}

impl Tweaks {
    pub fn text(&self) -> String {
        let on = |b: bool| if b { "on" } else { "off" };
        format!(
            "# Misogynistic Lighting: the other old looks, after Nostalgic Tweaks. Each on or off;\n\
             # saved changes take hold within two seconds. Delete this file to get the defaults back.\n\
             # fog: the old distance fog, from a quarter of the render distance out.\n\
             # void_fog: beta's black fog near the bottom of the Overworld, where no sky light reaches.\n\
             # version: the version text in the top left; version_text is what it says.\n\
             # item_name: the item's name over the hotbar (the old game had none).\n\
             # xp_bar: the experience bar (the old game had none; enchanting needs it).\n\n\
             fog = {}\nvoid_fog = {}\nversion = {}\nversion_text = {}\nitem_name = {}\nxp_bar = {}\n",
            on(self.fog),
            on(self.void_fog),
            on(self.version),
            self.version_text,
            on(self.item_name),
            on(self.xp_bar),
        )
    }

    /// Reads a file in `text`'s form: a line it doesn't understand keeps its default.
    pub fn parse(text: &str) -> Tweaks {
        let mut t = Tweaks::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let key = key.trim();
            if key == "version_text" {
                // The whole rest of the line: a `#` may be part of it.
                let value = value.trim();
                t.version_text = value.chars().take(64).collect();
                continue;
            }
            let value = value.split('#').next().unwrap_or("").trim().to_ascii_lowercase();
            let slot = match key {
                "fog" | "old_fog" => &mut t.fog,
                "void_fog" => &mut t.void_fog,
                "version" | "version_overlay" => &mut t.version,
                "item_name" | "selected_item_name" => &mut t.item_name,
                "xp_bar" | "experience_bar" => &mut t.xp_bar,
                _ => continue,
            };
            match value.as_str() {
                "on" | "true" | "yes" => *slot = true,
                "off" | "false" | "no" => *slot = false,
                _ => {}
            }
        }
        t
    }
}

/// Where the old distance fog starts: a quarter of the way to its end.
pub fn old_fog_start(end: f32) -> f32 {
    end * 0.25
}

/// Beta's void fog: how far the fog reaches with `sky` light (0 to 15) at the eye, `above` blocks
/// over the bottom of the world; None where there is none.
pub fn void_fog_end(sky: i32, above: f64) -> Option<f32> {
    let d = f64::from(sky.clamp(0, 15)) / 16.0 + (above + 4.0) / 32.0;
    if d >= 1.0 {
        return None;
    }
    let d = d.max(0.0);
    Some(((d * d) as f32 * 100.0).max(5.0))
}

/// Beta's void fog colour: the fog darkens towards black in the last 32 blocks over the bottom.
pub fn void_fog_shade(above: f64) -> f32 {
    let d = (above / 32.0).clamp(0.0, 1.0);
    (d * d) as f32
}

/// The bottom of a dimension's world, for the void fog (only the Overworld has it).
pub fn void_bottom(dimension: &str) -> Option<f64> {
    (dimension == "minecraft:overworld").then_some(-64.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_read_back() {
        let t = Tweaks::default();
        assert_eq!(Tweaks::parse(&t.text()), t);
        let off = Tweaks { fog: false, version_text: "Minecraft Alpha v1.2.6 # old".into(), xp_bar: false, ..t };
        assert_eq!(Tweaks::parse(&off.text()), off);
        assert_eq!(Tweaks::parse("fog = maybe\nwaves = off\n"), Tweaks::default());
    }

    #[test]
    fn old_fog_from_a_quarter_out() {
        assert_eq!(old_fog_start(128.0), 32.0);
    }

    #[test]
    fn void_fog_closes_in_at_the_bottom() {
        // In the open or high up: none.
        assert_eq!(void_fog_end(15, 0.0), None);
        assert_eq!(void_fog_end(0, 40.0), None);
        // In the dark at the bottom: five blocks.
        assert_eq!(void_fog_end(0, 0.0), Some(5.0));
        // Thinner going up.
        let (low, high) = (void_fog_end(0, 10.0).unwrap(), void_fog_end(0, 20.0).unwrap());
        assert!(low < high && high < 100.0);
        assert_eq!(void_fog_shade(0.0), 0.0);
        assert_eq!(void_fog_shade(40.0), 1.0);
        assert!(void_fog_shade(16.0) > 0.2 && void_fog_shade(16.0) < 0.3);
        assert_eq!(void_bottom("minecraft:the_nether"), None);
    }
}
