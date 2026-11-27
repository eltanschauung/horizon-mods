// Misogynistic Lighting: the old lighting, after Nostalgic Tweaks' classic light texture by Adrenix
// (LGPL-3.0; the brightness curve and its numbers are its, written anew here): one grey light, as
// before the coloured, flickering light; no torch orange, no blue night, and nights darker.
//
// Horizon's prebuilt renderer takes no replacement lightmap, so this sets the light it is given
// each frame instead: white sky and block light, a grey floor of light as dark as the old one,
// no flicker, and the sky as bright as the old sky light at its level (eleven levels down at
// night). The game's lightmap sums already follow beta's light curve, so what it draws comes out
// close to the old light texture.
//
// A module of the game crate (`crate::game::misogynistic_lighting`), also compiled into the mod's
// own tests, where the game half is left out.

/// The settings file, in the game's `config/misogynistic_lighting/` folder.
pub const FILE: &str = "old_lighting.cfg";

/// The torch flicker that leaves block light at its plain level: the game adds it to 1.4.
pub const NO_FLICKER: f32 = -0.4;

/// The dimension, as far as the light goes: Nostalgic Tweaks treats the Nether and the End apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dimension {
    Overworld,
    Nether,
    End,
}

impl Dimension {
    /// From the dimension's ambient light colour (`visual/ambient_light_color`): the Overworld's is
    /// near black, the Nether's a warm brown, the End's a greenish grey.
    pub fn from_ambient(ambient: [f32; 3]) -> Dimension {
        let [r, g, b] = ambient;
        if r.max(g).max(b) < 0.1 {
            Dimension::Overworld
        } else if g > r && g >= b {
            Dimension::End
        } else {
            Dimension::Nether
        }
    }
}

/// How bright a light `level` (0 to 15) is, block or sky light: Nostalgic Tweaks' classic curve.
pub fn brightness(level: i32, sky: bool) -> f32 {
    let level = level.clamp(0, 15);
    let f = 1.0 - level as f32 / 15.0;
    let k = if sky {
        if level >= 14 { 0.0 } else { 2.6 }
    } else {
        match level {
            4 => 2.9,
            5 | 9 => 2.5,
            11 | 12 => 2.4,
            13 => 1.8,
            14 => 1.2,
            15 => 0.6,
            _ => 2.6,
        }
    };
    (1.0 - f) / (f * k + 1.0) * 0.95 + 0.05
}

/// How many levels the sky light loses: 0 by day, 11 at night (`sky_light_level` 0 to 1, the
/// dimension's `gameplay/sky_light_level` over 15).
pub fn sky_subtracted(sky_light_level: f32) -> i32 {
    (15.0 - 15.0 * sky_light_level).round().clamp(0.0, 15.0) as i32
}

/// The darkest the light gets, grey: the old light at level 0 in each dimension (the Nether's
/// ambient light and its sky light formula, the End's glow).
pub fn floor(dimension: Dimension) -> f32 {
    match dimension {
        Dimension::Overworld => brightness(0, true),
        Dimension::Nether => 0.05 + 0.75 * brightness(15, true) * 0.1,
        Dimension::End => 0.22 + 0.75 * brightness(0, true),
    }
}

/// What the game's lightmap is given: (the grey ambient light, the sky light factor). The full sky
/// then comes out as bright as the old sky light `sub` levels down.
pub fn restyle(dimension: Dimension, sub: i32) -> (f32, f32) {
    let ambient = floor(dimension);
    let top = match dimension {
        Dimension::Overworld => brightness(15 - sub, true),
        Dimension::Nether => ambient,
        Dimension::End => 0.22 + 0.75 * brightness(15 - sub, true),
    };
    (ambient, (top - ambient).max(0.0))
}

/// `old_lighting.cfg`'s two switches, each `on` or `off`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Switches {
    /// `lighting`: the old light (one grey light, no flicker, darker nights).
    pub lighting: bool,
    /// `colours`: 1.7.10's sky, fog, sunrise and cloud colours.
    pub colours: bool,
}

impl Default for Switches {
    fn default() -> Switches {
        Switches { lighting: true, colours: true }
    }
}

/// `old_lighting.cfg`: `lighting` and `colours`, each `on` or `off` (missing: on).
pub fn parse(text: &str) -> Switches {
    let mut s = Switches::default();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else { continue };
        let slot = match key.trim() {
            "lighting" | "old_lighting" => &mut s.lighting,
            "colours" | "colors" | "old_colours" => &mut s.colours,
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

/// The file's text with these switches.
pub fn text(s: Switches) -> String {
    let on = |b: bool| if b { "on" } else { "off" };
    format!(
        "# Misogynistic Lighting, after Nostalgic Tweaks. Each on or off; a saved change takes hold\n\
         # within two seconds, and Options > Mods > Misogynistic Lighting switches them too.\n\
         # lighting: the old light (one grey light, no coloured or flickering light, darker nights).\n\
         # colours: 1.7.10's sky, fog, sunrise and cloud colours, and the Nether's old red fog.\n\
         # Delete this file to get the defaults back.\n\n\
         lighting = {}\ncolours = {}\n",
        on(s.lighting),
        on(s.colours)
    )
}

/// Where the file is: the game's `config/misogynistic_lighting/`.
#[cfg(not(test))]
pub fn path() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_default().join("config").join("misogynistic_lighting").join(FILE)
}

/// The switches: the file read again every two seconds (missing: both on).
#[cfg(not(test))]
pub(crate) fn switches() -> Switches {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static READ: Mutex<Option<(Instant, Switches)>> = Mutex::new(None);
    let Ok(mut read) = READ.lock() else { return Switches::default() };
    if let Some((at, s)) = *read
        && at.elapsed() < Duration::from_secs(2)
    {
        return s;
    }
    let s = std::fs::read_to_string(path()).map(|t| parse(&t)).unwrap_or_default();
    *read = Some((Instant::now(), s));
    s
}

/// Whether the old lighting is on.
#[cfg(not(test))]
pub(crate) fn enabled() -> bool {
    switches().lighting
}

// 1.7.10's sky and fog colours (`WorldProvider`, `World.getSkyColor`, `getCloudColour`,
// `BiomeGenBase.getSkyColorByTemp`), written anew from its numbers.

/// `WorldProvider.calculateCelestialAngle`: 0 at noon, 0.5 at midnight.
pub fn celestial_angle(day_time: i64, partial: f32) -> f32 {
    let mut f = (day_time.rem_euclid(24000) as f32 + partial) / 24000.0 - 0.25;
    if f < 0.0 {
        f += 1.0;
    }
    if f > 1.0 {
        f -= 1.0;
    }
    let eased = 1.0 - ((f * std::f32::consts::PI).cos() + 1.0) / 2.0;
    f + (eased - f) / 3.0
}

/// How much of the day's colour the sky has at `angle`: 1 by day, 0 at night.
pub fn daylight(angle: f32) -> f32 {
    ((angle * std::f32::consts::TAU).cos() * 2.0 + 0.5).clamp(0.0, 1.0)
}

/// Java's `Color.HSBtoRGB`, as bytes.
fn hsb(hue: f32, saturation: f32, brightness: f32) -> [u8; 3] {
    let byte = |v: f32| (v * 255.0 + 0.5) as u8;
    if saturation == 0.0 {
        return [byte(brightness); 3];
    }
    let h = (hue - hue.floor()) * 6.0;
    let f = h - h.floor();
    let p = brightness * (1.0 - saturation);
    let q = brightness * (1.0 - saturation * f);
    let t = brightness * (1.0 - saturation * (1.0 - f));
    let (r, g, b) = match h as u32 {
        0 => (brightness, t, p),
        1 => (q, brightness, p),
        2 => (p, brightness, t),
        3 => (p, q, brightness),
        4 => (t, p, brightness),
        _ => (brightness, p, q),
    };
    [byte(r), byte(g), byte(b)]
}

/// `BiomeGenBase.getSkyColorByTemp`: the sky's day colour from the temperature.
pub fn sky_by_temperature(temperature: f32) -> [f32; 3] {
    let t = (temperature / 3.0).clamp(-1.0, 1.0);
    hsb(0.622_222_24 - t * 0.05, 0.5 + t * 0.1, 1.0).map(|c| f32::from(c) / 255.0)
}

/// `BiomeGenBase.getFloatTemperature` without its noise: cooler above y 64.
pub fn temperature_at(base: f32, y: f64) -> f32 {
    if y > 64.0 { base - (y as f32 - 64.0) * 0.05 / 30.0 } else { base }
}

/// Greyed towards `share` of its own brightness by `amount` (`World.getSkyColor`'s weather).
fn grey(c: [f32; 3], share: f32, amount: f32) -> [f32; 3] {
    let grey = (c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11) * share;
    c.map(|v| v * (1.0 - amount) + grey * amount)
}

/// `World.getSkyColor`: the sky by the temperature, darkened by night, greyed by rain and thunder.
pub fn old_sky_color(temperature: f32, angle: f32, rain: f32, thunder: f32) -> [f32; 3] {
    let day = daylight(angle);
    let mut c = sky_by_temperature(temperature).map(|v| v * day);
    if rain > 0.0 {
        c = grey(c, 0.6, rain * 0.75);
    }
    if thunder > 0.0 {
        c = grey(c, 0.2, thunder * 0.75);
    }
    c
}

/// `WorldProvider.getFogColor` and the Nether's and the End's: the fog before the renderer
/// blends in the sky, the sunrise and the weather.
pub fn old_fog_color(dimension: Dimension, angle: f32) -> [f32; 3] {
    match dimension {
        Dimension::Overworld => {
            let day = daylight(angle);
            let k = day * 0.94 + 0.06;
            [0.752_941_2 * k, 0.847_058_83 * k, day * 0.91 + 0.09]
        }
        Dimension::Nether => [0.2, 0.03, 0.03],
        Dimension::End => [160.0 / 255.0 * 0.15, 128.0 / 255.0 * 0.15, 160.0 / 255.0 * 0.15],
    }
}

/// `WorldProvider.calcSunriseSunsetColors`: the sunrise and sunset glow, alpha 0 when none.
pub fn old_sunrise(angle: f32) -> [f32; 4] {
    let c = (angle * std::f32::consts::TAU).cos();
    if !(-0.4..=0.4).contains(&c) {
        return [0.0; 4];
    }
    let f = c / 0.4 * 0.5 + 0.5;
    let mut alpha = 1.0 - (1.0 - (f * std::f32::consts::PI).sin()) * 0.99;
    alpha *= alpha;
    [f * 0.3 + 0.7, f * f * 0.7 + 0.2, 0.2, alpha]
}

/// `World.getCloudColour`: white clouds, greyed by rain and thunder, darker at night.
pub fn old_cloud_color(angle: f32, rain: f32, thunder: f32) -> [f32; 3] {
    let day = daylight(angle);
    let mut c = [1.0f32; 3];
    if rain > 0.0 {
        c = grey(c, 0.6, rain * 0.95);
    }
    c = [c[0] * (day * 0.9 + 0.1), c[1] * (day * 0.9 + 0.1), c[2] * (day * 0.85 + 0.15)];
    if thunder > 0.0 {
        c = grey(c, 0.2, thunder * 0.95);
    }
    c
}

/// The frame's sky, fog, sunrise and cloud colours made 1.7.10's.
#[cfg(not(test))]
fn old_colours(game: &super::Game<'_>, sky: &mut horizon_client::environment::Sky, dimension: Dimension, partial: f32) {
    let angle = celestial_angle(game.view_day_time(), partial);
    sky.fog_color = old_fog_color(dimension, angle);
    if dimension != Dimension::Overworld
        || game.dimension_type().skybox != horizon_world::dimension_type::Skybox::Overworld
    {
        return;
    }
    let Some(player) = game.player.as_ref() else { return };
    let p = player.position;
    let (x, y, z) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
    let Some(chunk) = game.world.chunks.get(&(x >> 4, z >> 4)) else { return };
    let biomes = match &game.world_biomes {
        Some(b) => b,
        None => game.res.biomes(),
    };
    let temperature = temperature_at(biomes.get(chunk.biome(x, y, z)).temperature, p.y);
    let (rain, thunder) = game.view_weather(partial);
    sky.sky_color = old_sky_color(temperature, angle, rain, thunder);
    sky.sunrise_color = old_sunrise(angle);
    let [r, g, b] = old_cloud_color(angle, rain, thunder);
    sky.cloud_color = [r, g, b, sky.cloud_color[3]];
}

/// The frame's light, made the old light's (white, grey floor, the old sky level), and its
/// colours 1.7.10's.
#[cfg(not(test))]
pub(crate) fn sky(game: &super::Game<'_>, sky: &mut horizon_client::environment::Sky, partial: f64) {
    let switches = switches();
    let dimension = Dimension::from_ambient(sky.ambient_light_color);
    if switches.colours {
        old_colours(game, sky, dimension, partial as f32);
    }
    if !switches.lighting {
        return;
    }
    let (ambient, factor) = restyle(dimension, sky_subtracted(sky.sky_light_level));
    sky.ambient_light_color = [ambient; 3];
    sky.sky_light_color = [1.0; 3];
    sky.block_light_tint = [1.0; 3];
    sky.sky_light_factor = factor;
}

/// No torch flicker: block light at its plain level.
#[cfg(not(test))]
pub(crate) fn flicker(flicker: &mut f32) {
    if enabled() {
        *flicker = NO_FLICKER;
    }
}
