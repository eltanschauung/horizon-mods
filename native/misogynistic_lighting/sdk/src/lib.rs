//! Misogynistic Lighting: the old lighting, after Nostalgic Tweaks' classic light texture (Adrenix,
//! LGPL-3.0; written anew). Client side only, so it has no say in whether a server lets you in.
//!
//! - The mixins `old-sky` and `old-lightmap` set the light each frame is drawn with
//!   (`src/mixins/classic_light.rs`, a module of the game): white sky and block light, the old
//!   light's grey floor, no torch flicker, and the open sky as bright as the old sky light,
//!   eleven levels down at night. The game's lightmap then comes out close to the old one.
//!   The same mixin makes the sky, fog, sunrise and cloud colours 1.7.10's.
//! - `old_lighting.cfg`, in `config/misogynistic_lighting/`, switches both (`lighting` and
//!   `colours`, each `on` or `off`). This half writes it with the defaults when it's missing; the
//!   renderer reads it.
//! - Options > Mods > Misogynistic Lighting switches everything in game: a change there is
//!   written to the files.
//! - The other old looks, from this half (`tweaks.rs`, switched in `nostalgic.cfg`): the old
//!   distance fog, beta's void fog, the version text, no item name over the hotbar and, if
//!   wanted, no experience bar.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use mod_api::client::fog::FogType;
use mod_api::{ClientHost, ConfigPath, Flow, Hooks, Registrar, RegistrationContext, SettingSpec};

mod tweaks;
use tweaks::Tweaks;

/// `nostalgic.cfg`, as last read; the dimension this client is in; ticks since the last read.
static TWEAKS: Mutex<Option<Tweaks>> = Mutex::new(None);
static DIMENSION: Mutex<String> = Mutex::new(String::new());
static TICKS: AtomicU32 = AtomicU32::new(0);
/// Whether this frame's HUD drew a mount's health: the bar under the hotbar is then its jump bar.
static RIDING: AtomicBool = AtomicBool::new(false);

fn tweaks() -> Tweaks {
    TWEAKS.lock().ok().and_then(|t| t.clone()).unwrap_or_default()
}

/// Reads `nostalgic.cfg` again, writing the defaults if it's missing.
fn read_tweaks(files: &dyn mod_api::ConfigFiles) {
    let Ok(path) = ConfigPath::new(tweaks::FILE) else { return };
    let read = match files.read(&path) {
        Ok(bytes) => std::str::from_utf8(&bytes).ok().map(Tweaks::parse),
        Err(_) => {
            if matches!(files.stat(&path), Ok(None)) {
                let _ = files.write(&path, Tweaks::default().text().as_bytes());
            }
            None
        }
    };
    if let (Some(t), Ok(mut slot)) = (read, TWEAKS.lock()) {
        *slot = Some(t);
    }
}

/// The sky light at the eye and how far it is over the bottom of the world, where there is void fog.
fn void_at(host: &dyn ClientHost, camera: [f64; 3]) -> Option<(i32, f64)> {
    let dimension = DIMENSION.lock().ok()?.clone();
    let bottom = tweaks::void_bottom(&dimension)?;
    let eye = camera.map(|c| c.floor() as i32);
    let sky = host.level(&dimension).and_then(|level| level.brightness(host, mod_api::LightLayer::Sky, eye)).unwrap_or(15);
    Some((sky, camera[1] - bottom))
}

#[cfg(test)]
#[path = "mixins/classic_light.rs"]
mod classic_light;

const FILE: &str = "misogynistic_lighting/old_lighting.cfg";

#[derive(Default)]
struct Mod;

/// Writes `old_lighting.cfg` with both switches on, if it isn't there.
fn write_default(files: &dyn mod_api::ConfigFiles) {
    let Ok(path) = ConfigPath::new(FILE) else { return };
    if matches!(files.stat(&path), Ok(None)) {
        let _ = files.write(&path, switches_text(true, true).as_bytes());
    }
}

/// `old_lighting.cfg`'s text, as the renderer's half writes it (`classic_light::text`).
fn switches_text(lighting: bool, colours: bool) -> String {
    let on = |b: bool| if b { "on" } else { "off" };
    format!(
        "# Misogynistic Lighting, after Nostalgic Tweaks. Each on or off; a saved change takes hold\n\
         # within two seconds, and Options > Mods > Misogynistic Lighting switches them too.\n\
         # lighting: the old light (one grey light, no coloured or flickering light, darker nights).\n\
         # colours: 1.7.10's sky, fog, sunrise and cloud colours, and the Nether's old red fog.\n\
         # Delete this file to get the defaults back.\n\n\
         lighting = {}\ncolours = {}\n",
        on(lighting),
        on(colours)
    )
}

/// A switch in `old_lighting.cfg`'s text by any of its names (missing: on).
fn switch(text: &str, names: &[&str]) -> bool {
    let mut on = true;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else { continue };
        if names.contains(&key.trim()) {
            match value.trim().to_ascii_lowercase().as_str() {
                "on" | "true" | "yes" => on = true,
                "off" | "false" | "no" => on = false,
                _ => {}
            }
        }
    }
    on
}

const LIGHTING: &[&str] = &["lighting", "old_lighting"];
const COLOURS: &[&str] = &["colours", "colors", "old_colours"];

/// The toggles on the mod's page in Options > Mods, with their defaults: the first two are
/// `old_lighting.cfg`'s, the rest `nostalgic.cfg`'s.
const SETTINGS: [(&str, bool); 7] = [
    ("lighting", true),
    ("colours", true),
    ("fog", true),
    ("void_fog", true),
    ("version", true),
    ("item_name", false),
    ("xp_bar", true),
];

/// The game's `config/` folder (the renderer's half reads its file the same way).
fn config_dir() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_default().join("config")
}

/// A toggle changed on the page: written to its file, which then switches it as a saved edit does.
fn setting_to_file(key: &str, on: bool) {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(dir.join("misogynistic_lighting"));
    match key {
        "lighting" | "colours" => {
            let path = dir.join(FILE);
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let (mut lighting, mut colours) = (switch(&text, LIGHTING), switch(&text, COLOURS));
            if key == "lighting" {
                lighting = on;
            } else {
                colours = on;
            }
            let _ = std::fs::write(&path, switches_text(lighting, colours));
        }
        _ => {
            let path = dir.join(tweaks::FILE);
            let mut t = std::fs::read_to_string(&path).map(|s| Tweaks::parse(&s)).unwrap_or_default();
            let slot = match key {
                "fog" => &mut t.fog,
                "void_fog" => &mut t.void_fog,
                "version" => &mut t.version,
                "item_name" => &mut t.item_name,
                "xp_bar" => &mut t.xp_bar,
                _ => return,
            };
            *slot = on;
            let _ = std::fs::write(&path, t.text());
            if let Ok(mut slot) = TWEAKS.lock() {
                *slot = Some(t);
            }
        }
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// The page in Options > Mods: each old look on or off.
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        for (key, default) in SETTINGS {
            let _ = registrar.register_setting(
                SettingSpec::toggle(key, format!("misogynistic_lighting.{key}"), default)
                    .tooltip(format!("misogynistic_lighting.{key}.tooltip")),
            );
        }
    }

    fn setting_changed(&self, host: &mut dyn ClientHost, event: &mut mod_api::SettingChangedEvent<'_>) {
        if let Some(on) = host.setting_bool(event.key) {
            setting_to_file(event.key, on);
        }
    }

    fn client_level_load(&self, host: &mut dyn ClientHost, event: &mut mod_api::ClientLevelLoadEvent<'_>) {
        write_default(host.config_files());
        read_tweaks(host.config_files());
        if let Ok(mut d) = DIMENSION.lock() {
            *d = event.dimension.to_owned();
        }
    }

    /// `nostalgic.cfg` read again every two seconds.
    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut mod_api::ClientTickPostEvent) {
        if TICKS.fetch_add(1, Ordering::Relaxed).is_multiple_of(40) {
            read_tweaks(host.config_files());
        }
    }

    /// The old distance fog, from a quarter of the render distance out; beta's void fog closing in
    /// near the bottom of the Overworld. Closer fog (blindness, water) is left alone.
    fn render_fog(&self, host: &dyn ClientHost, event: &mut mod_api::RenderFogEvent<'_>) {
        if event.fog_type != FogType::Atmospheric {
            return;
        }
        let t = tweaks();
        let fog = &mut *event.fog;
        if t.void_fog
            && let Some((sky, above)) = void_at(host, event.camera)
            && let Some(end) = tweaks::void_fog_end(sky, above)
            && end < fog.render_distance_end
        {
            fog.render_distance_end = end;
            fog.render_distance_start = fog.render_distance_start.min(tweaks::old_fog_start(end));
            fog.sky_end = fog.sky_end.min(end);
            fog.cloud_end = fog.cloud_end.min(end);
        }
        if t.fog {
            fog.render_distance_start = fog.render_distance_start.min(tweaks::old_fog_start(fog.render_distance_end));
        }
    }

    /// Beta's void fog turns black in the last 32 blocks over the bottom of the Overworld.
    fn compute_fog_color(&self, host: &dyn ClientHost, event: &mut mod_api::ComputeFogColorEvent<'_>) {
        if event.fog_type != FogType::Atmospheric || !tweaks().void_fog {
            return;
        }
        if let Some((_, above)) = void_at(host, event.camera) {
            let shade = tweaks::void_fog_shade(above);
            event.rgb.iter_mut().for_each(|c| *c *= shade);
        }
    }

    /// No item name over the hotbar; no experience bar, if so set (a horse's jump bar stays).
    fn gui_layer_pre(&self, _: &dyn ClientHost, event: &mut mod_api::GuiLayerPreEvent<'_>) -> Flow {
        match event.layer {
            "minecraft:hotbar" => RIDING.store(false, Ordering::Relaxed),
            "minecraft:vehicle_health" => RIDING.store(true, Ordering::Relaxed),
            "minecraft:selected_item_name" if !tweaks().item_name => return Flow::Cancel,
            "minecraft:contextual_info_bar" if !tweaks().xp_bar && !RIDING.load(Ordering::Relaxed) => {
                return Flow::Cancel;
            }
            _ => {}
        }
        Flow::Continue
    }

    /// The version text in the top left, as the old game drew it (F3 takes that corner).
    fn render_gui_post(&self, host: &dyn ClientHost, event: &mut mod_api::RenderGuiPostEvent<'_>) {
        let t = tweaks();
        if !t.version || t.version_text.is_empty() || host.native_corner_shown(mod_api::Corner::TopLeft) {
            return;
        }
        let width = event.draw.text_width(&t.version_text);
        let (x, y) = event.corners.place(mod_api::Corner::TopLeft, width, 9.0);
        event.draw.text(&t.version_text, [x, y], 0xFFFF_FFFF, true);
    }
}

#[cfg(test)]
mod tests {
    use super::classic_light::*;

    #[test]
    fn the_default_file_is_the_renderers_own() {
        for (lighting, colours) in [(true, true), (false, true), (true, false), (false, false)] {
            let written = super::switches_text(lighting, colours);
            assert_eq!(written, text(Switches { lighting, colours }));
            assert_eq!(parse(&written), Switches { lighting, colours });
            assert_eq!(super::switch(&written, super::LIGHTING), lighting);
            assert_eq!(super::switch(&written, super::COLOURS), colours);
        }
        assert_eq!(parse("nonsense\nlighting = maybe\n"), Switches::default());
        assert!(!parse("old_lighting = OFF # spelt the long way\n").lighting);
        // A 0.2 file, with no colours line: the colours are on.
        assert_eq!(parse("lighting = off\n"), Switches { lighting: false, colours: true });
        assert_eq!(format!("misogynistic_lighting/{FILE}"), super::FILE);
    }

    #[test]
    fn every_toggle_has_its_words() {
        let lang: std::collections::HashMap<String, String> =
            serde_json_lite(include_str!("../assets/misogynistic_lighting/lang/en_us.json"));
        for (key, _) in super::SETTINGS {
            for k in [format!("misogynistic_lighting.{key}"), format!("misogynistic_lighting.{key}.tooltip")] {
                assert!(lang.get(&k).is_some_and(|v| !v.is_empty()), "{k}");
            }
        }
    }

    /// A flat JSON object of strings, as the lang file is.
    fn serde_json_lite(text: &str) -> std::collections::HashMap<String, String> {
        text.lines()
            .filter_map(|line| {
                let (k, v) = line.trim().trim_end_matches(',').split_once("\": \"")?;
                Some((k.trim_start_matches('"').to_owned(), v.trim_end_matches('"').to_owned()))
            })
            .collect()
    }

    #[test]
    fn the_old_sky_by_day_and_night() {
        let noon = celestial_angle(6000, 0.0);
        let midnight = celestial_angle(18000, 0.0);
        assert!(noon.abs() < 1e-6 && (midnight - 0.5).abs() < 1e-6);
        assert_eq!(daylight(noon), 1.0);
        assert_eq!(daylight(midnight), 0.0);
        // Plains (0.8) by day: 1.7.10's light blue sky, 0x79A7FF.
        let plains = old_sky_color(0.8, noon, 0.0, 0.0).map(|c| (c * 255.0).round() as u8);
        assert_eq!(plains, [0x79, 0xA7, 0xFF]);
        // A desert's sky more cyan, a snowy one greyer (Java's HSB rounding, a step off today's).
        let byte = |c: [f32; 3]| c.map(|v| (v * 255.0).round() as u8);
        assert_eq!(byte(sky_by_temperature(2.0)), [0x6F, 0xB2, 0xFF]);
        assert_eq!(byte(sky_by_temperature(0.0)), [0x80, 0xA1, 0xFF]);
        assert_eq!(old_sky_color(0.8, midnight, 0.0, 0.0), [0.0; 3]);
        // Rain greys it.
        let wet = old_sky_color(0.8, noon, 1.0, 0.0);
        assert!(wet[2] - wet[0] < 0.2);
        // The fog: 0xC0D8FF by day, near black at night.
        let day = old_fog_color(Dimension::Overworld, noon);
        assert!((day[0] - 0.752_941_2).abs() < 1e-6 && day[2] == 1.0);
        let night = old_fog_color(Dimension::Overworld, midnight);
        assert!(night[0] < 0.05 && night[2] < 0.1);
        assert_eq!(old_fog_color(Dimension::Nether, noon), [0.2, 0.03, 0.03]);
        // Higher up is cooler.
        assert!(temperature_at(0.8, 160.0) < 0.8 && temperature_at(0.8, 40.0) == 0.8);
    }

    #[test]
    fn sunrise_only_at_dawn_and_dusk() {
        assert_eq!(old_sunrise(celestial_angle(6000, 0.0))[3], 0.0);
        assert_eq!(old_sunrise(celestial_angle(18000, 0.0))[3], 0.0);
        let dawn = old_sunrise(celestial_angle(23500, 0.0));
        let dusk = old_sunrise(celestial_angle(12500, 0.0));
        assert!(dawn[3] > 0.0 && dusk[3] > 0.0);
        assert!(dawn[0] > dawn[1] && dawn[1] >= dawn[2]);
        // Clouds white by day, dim at night.
        assert_eq!(old_cloud_color(celestial_angle(6000, 0.0), 0.0, 0.0), [1.0; 3]);
        assert!(old_cloud_color(celestial_angle(18000, 0.0), 0.0, 0.0)[0] < 0.2);
    }

    #[test]
    fn brighter_with_each_level_from_dim_to_full() {
        for sky in [false, true] {
            assert!((brightness(0, sky) - 0.05).abs() < 1e-6);
            assert!((brightness(15, sky) - 1.0).abs() < 1e-6);
            for l in 1..16 {
                assert!(brightness(l, sky) > brightness(l - 1, sky), "level {l} sky {sky}");
            }
        }
    }

    /// The game's lightmap at (sky level, block level) given the restyled light, before its
    /// Brightness curve: white light, so one grey (`Renderer::update_lightmap`'s sums).
    fn game_light(dimension: Dimension, sub: i32, sky: i32, block: i32) -> f32 {
        let (ambient, factor) = restyle(dimension, sub);
        let curve = |level: f32| level / (4.0 - 3.0 * level);
        let sky = curve(sky as f32 / 15.0) * factor;
        let block = curve(block as f32 / 15.0) * (1.4 + NO_FLICKER);
        (ambient + sky + block).clamp(0.0, 1.0)
    }

    /// The old light texture at (sky level, block level), Overworld.
    fn old_light(sub: i32, sky: i32, block: i32) -> f32 {
        brightness(block, false).max(brightness((sky - sub).max(0), true))
    }

    #[test]
    fn nights_are_eleven_levels_darker() {
        assert_eq!(sky_subtracted(1.0), 0);
        assert_eq!(sky_subtracted(4.0 / 15.0), 11);
        let (_, day) = restyle(Dimension::Overworld, 0);
        let (_, night) = restyle(Dimension::Overworld, 11);
        assert!(night < day / 8.0);
        // The open sky as bright as the old sky light, by day and night.
        for sub in [0, 5, 11] {
            assert!((game_light(Dimension::Overworld, sub, 15, 0) - old_light(sub, 15, 0)).abs() < 1e-5);
        }
        // A torch's light is the same by night.
        assert_eq!(game_light(Dimension::Overworld, 11, 0, 14), game_light(Dimension::Overworld, 0, 0, 14));
    }

    #[test]
    fn close_to_the_old_light_texture() {
        // Where only one light reaches (the other at 0), the game's light is the old one's, near
        // enough; where sky and block light meet halfway, the game adds them where the old light
        // took the brighter, so it comes out brighter there.
        let (mut alone, mut mixed, mut total) = (0f32, 0f32, 0f32);
        for sub in [0, 11] {
            for sky in 0..16 {
                for block in 0..16 {
                    let error = (game_light(Dimension::Overworld, sub, sky, block) - old_light(sub, sky, block)).abs();
                    if (sky - sub).max(0) == 0 || block == 0 {
                        alone = alone.max(error);
                    } else {
                        mixed = mixed.max(error);
                    }
                    total += error;
                }
            }
        }
        let mean = total / 512.0;
        eprintln!("one light: {alone:.3}, both: {mixed:.3}, mean: {mean:.3}");
        assert!(alone < 0.16 && mixed < 0.45 && mean < 0.06);
        // Darkness as dark as the old light's.
        assert!((game_light(Dimension::Overworld, 11, 0, 0) - 0.05).abs() < 1e-6);
    }

    #[test]
    fn dimensions_by_their_ambient_light() {
        assert_eq!(Dimension::from_ambient([0.04, 0.04, 0.04]), Dimension::Overworld);
        assert_eq!(Dimension::from_ambient([0.19, 0.16, 0.13]), Dimension::Nether);
        assert_eq!(Dimension::from_ambient([0.25, 0.28, 0.25]), Dimension::End);
        assert!(floor(Dimension::Nether) > floor(Dimension::Overworld));
        assert!(floor(Dimension::End) > floor(Dimension::Nether));
        assert_eq!(restyle(Dimension::Nether, 0).1, 0.0);
    }
}
