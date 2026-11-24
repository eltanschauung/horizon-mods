//! The mod's settings: registered with the game (they show on the mod's page in Options > Mods,
//! labelled from `assets/horizonmatica/lang/`), read and written through the host. A few values
//! the menu's sliders can't hold (any render radius, any face limit, the tool's item) are kept in
//! the mod's data files instead (`store`).

use crate::store;
use crate::world::World;
use crate::ID;
use horizonmatica_core::printer::PrinterSettings;
use mod_api::{Registrar, SettingSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Missing blocks as translucent block models, mismatches as coloured boxes.
    Ghost,
    /// Everything as coloured boxes.
    Boxes,
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub render: bool,
    /// Ghosts and boxes show through terrain.
    pub through: bool,
    pub mode: Mode,
    pub alpha: f32,
    /// How far around the player the overlay is built, in blocks (no upper bound: the player
    /// decides what the memory is worth), or the game's view distance with `radius_view`.
    pub radius: i32,
    pub radius_view: bool,
    /// Most faces the overlay keeps (0: no limit); the nearest are built first.
    pub max_faces: usize,
    /// Water and lava (whole cubes as ghosts) aren't drawn.
    pub hide_fluids: bool,
    pub show_extra: bool,
    pub show_wrong: bool,
    /// Missing blocks are marked through everything, however far.
    pub highlight_missing: bool,
    /// Wrong blocks and wrong states too.
    pub highlight_wrong: bool,
    pub strict: bool,
    pub hud: bool,
    pub materials_hud: bool,
    pub printer_on: bool,
    pub printer: PrinterSettings,
    /// Redstone's on/off (powered, lit...) counts as a wrong state.
    pub power: bool,
    /// The selection tool: `tool_item` in the main hand sets the area's corners.
    pub tool: bool,
    pub tool_item: String,
    /// How far the tool reaches, in blocks.
    pub tool_range: f64,
    /// A click sets the corner on the block in front of the face looked at, not on the block.
    pub tool_front: bool,
    /// Water and lava stop the tool's ray (else it goes through them to the ground).
    pub tool_fluids: bool,
    /// The tool's controls show in the HUD while it is in hand.
    pub tool_hud: bool,
    /// The tool works on placements (select, move, turn) instead of the area's corners.
    pub tool_placement: bool,
    /// Frames around the placements (the selected one bright, cyan or, locked, orange).
    pub placement_boxes: bool,
    /// Easy place: holding the right button places the block the schematic wants where the
    /// crosshair points, when it is the item in hand.
    pub easy_place: bool,
    /// Ticks between two easy placements while the button is held.
    pub easy_interval: u32,
    /// Easy place may click a ghost block with no real neighbour (the printer has its own).
    pub easy_air: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            render: true,
            through: false,
            mode: Mode::Ghost,
            alpha: 0.55,
            radius: 32,
            radius_view: false,
            max_faces: 300_000,
            hide_fluids: false,
            show_extra: true,
            show_wrong: true,
            highlight_missing: false,
            highlight_wrong: false,
            strict: false,
            hud: true,
            materials_hud: false,
            printer_on: false,
            printer: PrinterSettings { interval: 3, ..PrinterSettings::default() },
            power: true,
            tool: true,
            tool_item: "minecraft:stick".into(),
            tool_range: 128.0,
            tool_front: false,
            tool_fluids: false,
            tool_hud: true,
            tool_placement: false,
            placement_boxes: true,
            easy_place: false,
            easy_interval: 2,
            easy_air: false,
        }
    }
}

/// A registered setting's kind and default.
#[derive(Clone, Copy)]
pub enum Kind {
    Toggle(bool),
    /// Option names (stored by index) and the default's index.
    Choice(&'static [&'static str], usize),
    /// min, max, step, default.
    Range(f64, f64, f64, f64),
}

/// Every registered setting: key, English label (also in the lang file), kind.
pub const SPECS: &[(&str, &str, Kind)] = &[
    ("render", "Rendering", Kind::Toggle(true)),
    ("through", "See through blocks", Kind::Toggle(false)),
    ("mode", "Mode", Kind::Choice(&["ghost", "boxes"], 0)),
    ("alpha", "Ghost opacity", Kind::Range(0.1, 1.0, 0.05, 0.55)),
    ("radius_view", "Radius = view distance", Kind::Toggle(false)),
    ("hide_fluids", "Hide water and lava", Kind::Toggle(false)),
    ("placement_boxes", "Boxes around placements", Kind::Toggle(true)),
    ("show_extra", "Show extra blocks", Kind::Toggle(true)),
    ("show_wrong", "Show wrong blocks", Kind::Toggle(true)),
    ("highlight_missing", "Highlight missing blocks", Kind::Toggle(false)),
    ("highlight_wrong", "Highlight wrong blocks", Kind::Toggle(false)),
    ("strict", "Strict states", Kind::Toggle(false)),
    ("power", "Redstone on/off counts as wrong", Kind::Toggle(true)),
    ("hud", "Info HUD", Kind::Toggle(true)),
    ("materials_hud", "Material list HUD", Kind::Toggle(false)),
    ("printer_interval", "Printer: one block every (ticks)", Kind::Range(1.0, 20.0, 1.0, 3.0)),
    ("printer_reach", "Printer reach", Kind::Range(1.0, 6.0, 0.5, 4.5)),
    ("air_place", "Printer air place", Kind::Toggle(false)),
    ("tool", "Selection tool", Kind::Toggle(true)),
    ("tool_mode", "Tool works on", Kind::Choice(&["area", "placement"], 0)),
    ("tool_range", "Tool reach", Kind::Range(16.0, 256.0, 16.0, 128.0)),
    ("tool_target", "Tool corner on", Kind::Choice(&["block", "front"], 0)),
    ("tool_fluids", "Tool stops at water/lava", Kind::Toggle(false)),
    ("tool_hud", "Tool controls in the HUD", Kind::Toggle(true)),
    ("easy_place", "Easy place", Kind::Toggle(false)),
    ("easy_interval", "Easy place every (ticks)", Kind::Range(1.0, 10.0, 1.0, 2.0)),
    ("easy_air", "Easy air place", Kind::Toggle(false)),
];

fn spec(key: &str) -> Option<Kind> {
    SPECS.iter().find(|(k, ..)| *k == key).map(|(_, _, kind)| *kind)
}

/// Registers every setting (from `Hooks::register`).
pub fn register(r: &mut dyn Registrar) {
    for &(key, _, kind) in SPECS {
        let label = format!("{ID}.setting.{key}");
        let spec = match kind {
            Kind::Toggle(default) => SettingSpec::toggle(key, label, default),
            Kind::Choice(options, default) => {
                SettingSpec::choice(key, label, options.iter().map(|o| format!("{ID}.setting.{key}.{o}")), default)
            }
            Kind::Range(min, max, step, default) => SettingSpec::range(key, label, min, max, step, default),
        };
        if let Err(e) = r.register_setting(spec) {
            eprintln!("[Horizonmatica] setting {key} refused: {e:?}");
        }
    }
}

/// A setting as stored (raw text), or None.
fn raw(w: &World<'_>, key: &str) -> Option<String> {
    w.host.setting(ID, key).map(|v| v.trim().to_string())
}

pub fn get_bool(w: &World<'_>, key: &str) -> bool {
    let default = match spec(key) {
        Some(Kind::Toggle(d)) => d,
        _ => false,
    };
    match raw(w, key).as_deref() {
        Some("true") => true,
        Some("false") => false,
        _ => default,
    }
}

/// A choice's option name.
pub fn get_choice(w: &World<'_>, key: &str) -> &'static str {
    let Some(Kind::Choice(options, default)) = spec(key) else { return "" };
    let i = raw(w, key).and_then(|v| v.parse::<usize>().ok()).filter(|i| *i < options.len()).unwrap_or(default);
    options[i]
}

pub fn get_number(w: &World<'_>, key: &str) -> f64 {
    let Some(Kind::Range(min, max, _, default)) = spec(key) else { return 0.0 };
    raw(w, key).and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite() && (min..=max).contains(v)).unwrap_or(default)
}

/// Stores a registered setting from its text: `true`/`false`, a choice's option name, a number.
pub fn set(w: &World<'_>, key: &str, value: &str) {
    let stored = match spec(key) {
        Some(Kind::Choice(options, _)) => match options.iter().position(|o| *o == value) {
            Some(i) => i.to_string(),
            None => return,
        },
        Some(Kind::Range(min, max, ..)) => match value.parse::<f64>() {
            Ok(v) if v.is_finite() => v.clamp(min, max).to_string(),
            _ => return,
        },
        Some(Kind::Toggle(_)) => value.to_string(),
        None => return,
    };
    w.host.set_setting(ID, key, &stored);
}

/// Before 0.4.0 a choice was kept by its option's name (`ghost`); the game's settings keep the
/// index. Converts what an older version left, once a session.
pub fn migrate(w: &World<'_>) {
    for (key, _, kind) in SPECS {
        if let Kind::Choice(options, _) = kind
            && let Some(v) = raw(w, key)
            && v.parse::<usize>().is_err()
            && let Some(i) = options.iter().position(|o| *o == v)
        {
            w.host.set_setting(ID, key, &i.to_string());
        }
    }
}

/// Reads every setting.
pub fn load(w: &World<'_>, printer_on: bool) -> Settings {
    let d = Settings::default();
    Settings {
        render: get_bool(w, "render"),
        through: get_bool(w, "through"),
        mode: if get_choice(w, "mode") == "boxes" { Mode::Boxes } else { Mode::Ghost },
        alpha: get_number(w, "alpha") as f32,
        radius: store::read_value(w, "radius").and_then(|v| v.trim().parse::<f64>().ok()).map_or(d.radius, |v| (v as i32).max(16)),
        radius_view: get_bool(w, "radius_view"),
        max_faces: store::read_value(w, "max_faces").and_then(|v| parse_faces(&v)).unwrap_or(d.max_faces),
        hide_fluids: get_bool(w, "hide_fluids"),
        show_extra: get_bool(w, "show_extra"),
        show_wrong: get_bool(w, "show_wrong"),
        highlight_missing: get_bool(w, "highlight_missing"),
        highlight_wrong: get_bool(w, "highlight_wrong"),
        strict: get_bool(w, "strict"),
        hud: get_bool(w, "hud"),
        materials_hud: get_bool(w, "materials_hud"),
        // A session's switch.
        printer_on,
        printer: PrinterSettings {
            interval: get_number(w, "printer_interval") as u32,
            reach: get_number(w, "printer_reach"),
            air_place: get_bool(w, "air_place"),
            ..d.printer
        },
        power: get_bool(w, "power"),
        tool: get_bool(w, "tool"),
        tool_item: store::read_value(w, "tool_item").map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).unwrap_or(d.tool_item),
        tool_range: get_number(w, "tool_range"),
        tool_front: get_choice(w, "tool_target") == "front",
        tool_fluids: get_bool(w, "tool_fluids"),
        tool_hud: get_bool(w, "tool_hud"),
        tool_placement: get_choice(w, "tool_mode") == "placement",
        placement_boxes: get_bool(w, "placement_boxes"),
        easy_place: get_bool(w, "easy_place"),
        easy_interval: (get_number(w, "easy_interval") as u32).max(1),
        easy_air: get_bool(w, "easy_air"),
    }
}

/// Writes every setting (commands change them too).
pub fn save(w: &World<'_>, s: &Settings) {
    set(w, "render", &s.render.to_string());
    set(w, "through", &s.through.to_string());
    set(w, "mode", if s.mode == Mode::Boxes { "boxes" } else { "ghost" });
    set(w, "alpha", &s.alpha.to_string());
    store::write_value(w, "radius", &s.radius.to_string());
    set(w, "radius_view", &s.radius_view.to_string());
    store::write_value(w, "max_faces", &faces_key(s.max_faces));
    set(w, "hide_fluids", &s.hide_fluids.to_string());
    set(w, "show_extra", &s.show_extra.to_string());
    set(w, "show_wrong", &s.show_wrong.to_string());
    set(w, "highlight_missing", &s.highlight_missing.to_string());
    set(w, "highlight_wrong", &s.highlight_wrong.to_string());
    set(w, "strict", &s.strict.to_string());
    set(w, "hud", &s.hud.to_string());
    set(w, "materials_hud", &s.materials_hud.to_string());
    set(w, "printer_interval", &s.printer.interval.to_string());
    set(w, "printer_reach", &s.printer.reach.to_string());
    set(w, "air_place", &s.printer.air_place.to_string());
    set(w, "power", &s.power.to_string());
    set(w, "tool", &s.tool.to_string());
    store::write_value(w, "tool_item", &s.tool_item);
    set(w, "tool_range", &s.tool_range.to_string());
    set(w, "tool_target", if s.tool_front { "front" } else { "block" });
    set(w, "tool_fluids", &s.tool_fluids.to_string());
    set(w, "tool_hud", &s.tool_hud.to_string());
    set(w, "tool_mode", if s.tool_placement { "placement" } else { "area" });
    set(w, "placement_boxes", &s.placement_boxes.to_string());
    set(w, "easy_place", &s.easy_place.to_string());
    set(w, "easy_interval", &s.easy_interval.to_string());
    set(w, "easy_air", &s.easy_air.to_string());
}

/// A face limit as written: `none` (0), a number, or one with k or m (`300k`, `2.5m`).
pub fn parse_faces(v: &str) -> Option<usize> {
    let v = v.trim().to_ascii_lowercase();
    if matches!(v.as_str(), "none" | "off" | "unlimited" | "0") {
        return Some(0);
    }
    let (num, times) = match v.strip_suffix('m').or_else(|| v.strip_suffix("mio")) {
        Some(n) => (n, 1_000_000.0),
        None => match v.strip_suffix('k') {
            Some(n) => (n, 1_000.0),
            None => (v.as_str(), 1.0),
        },
    };
    let n = num.trim().parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0)? * times;
    Some((n as usize).max(1_000))
}

/// How a face limit is stored.
pub fn faces_key(n: usize) -> String {
    match n {
        0 => "none".into(),
        n if n % 1_000_000 == 0 => format!("{}m", n / 1_000_000),
        n if n % 1_000 == 0 => format!("{}k", n / 1_000),
        n => n.to_string(),
    }
}

pub fn faces_text(n: usize) -> String {
    match n {
        0 => "no limit".into(),
        n if n >= 1_000_000 => format!("{} M faces", (n as f64 / 1e6 * 10.0).round() / 10.0),
        n => format!("{}k faces", n / 1_000),
    }
}
