// The light sources: once a tick, every entity near the camera that gives light (a glowing item
// in a hand or on the ground, fire, a creature, projectile or minecart that glows), with where it
// was the tick before; once a frame (`visuals`), the nearest of them where the frame draws them,
// handed to the game as its lights. Also the toggle key and the line it shows over the hotbar.
//
// The facts come from the server when it has the mod (`net`); else from what the client sees:
// its own hands and the kinds of entities that glow by themselves.
use crate::net;
use crate::runtime::{self as dl, Fact, Glow, Kind, Who};
use crate::store;
use mod_api::*;
use std::collections::HashMap;
use std::sync::Mutex;

/// One source: its light and where it was at the last two ticks (its lift included).
#[derive(Clone, Copy)]
struct Source {
    level: u8,
    before: [f64; 3],
    now: [f64; 3],
}

/// The key of the local player's own source.
const ME: u64 = u64::MAX;

#[derive(Default)]
struct State {
    /// This tick's sources: by client entity id, or `1 << 32 | id` for the server's.
    sources: HashMap<u64, Source>,
    /// The toggle key's presses since the last tick.
    toggles: u32,
    /// The line over the hotbar after a toggle, and the ticks it has left.
    line: Option<(bool, u32)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(s.get_or_insert_with(State::default))
}

fn russian(host: &dyn ClientHost) -> bool {
    // The game's language from the mod's own texts: `client_language` is None on the title.
    host.translate("dynamiclights.language").as_deref() == Some("ru")
}

// ---- the sources --------------------------------------------------------------------------

/// What an item gives off held: its light and whether it is a flame.
fn item(host: &dyn ClientHost, stack: Option<ItemStack>) -> (u8, bool) {
    stack.map_or((0, false), |s| {
        let id = s.item().id(host);
        (dl::item_light(&id), dl::fire_item(&id))
    })
}

/// Inside water, as `Entity.isEyeInFluid` measures it at `at`.
fn underwater(host: &dyn ClientHost, level: LevelRef, at: [f64; 3]) -> bool {
    let pos = at.map(|v| v.floor() as i32);
    let water = level
        .fluid_state(host, pos)
        .is_some_and(|f| !f.is_empty() && f.fluid.id(host).ends_with("water"));
    water && level.fluid_height(host, pos).is_some_and(|h| at[1] <= f64::from(pos[1]) + f64::from(h))
}

/// The local player's own source: its hands, at hand height (spectators give none).
fn me(host: &dyn ClientHost) -> Option<(u32, [f64; 3], Fact)> {
    let player = host.local_player()?;
    let e = player.entity();
    if player.game_mode(host) == Some(GameMode::Spectator) {
        return None;
    }
    let feet = e.position(host)?;
    let living = host.living(e)?;
    let hands = [Hand::Main, Hand::Off].map(|h| item(host, living.hand_item(host, h)));
    Some((e.id(), [feet[0], feet[1] + 1.2, feet[2]], Fact::new(Who::Me).hands(hands)))
}

/// What the client sees by itself of the entities near `camera`: the kinds that glow.
fn seen(host: &dyn ClientHost, level: LevelRef, camera: [f64; 3], range: f64) -> Vec<(u32, [f64; 3], Fact)> {
    let r = range + 4.0;
    let min = [camera[0] - r, camera[1] - r, camera[2] - r];
    let max = [camera[0] + r, camera[1] + r, camera[2] + r];
    let mut out = Vec::new();
    for e in level.entities_in(host, min, max).unwrap_or_default() {
        let (Some(feet), Some(kind)) = (e.position(host), e.kind(host).map(|k| k.id(host))) else { continue };
        let mut f = Fact::new(Who::Other);
        let lift = if let Some(g) = Glow::of(&kind) {
            f.glow = dl::flame(g.level(), g.fire());
            f.glow_kind = g.kind();
            g.lift()
        } else {
            let size = e.bounding_box(host).map_or([0.6, 0.6], |b| [b[1][0] - b[0][0], b[1][1] - b[0][1]]);
            let own = dl::creature_light(&kind, (size[0] / 0.52).round() as i32, 0);
            if own == 0 {
                continue;
            }
            f.glow = dl::flame(own, dl::fire_creature(&kind));
            f.glow_kind = Kind::Creatures;
            size[1] * 0.6
        };
        out.push((e.id(), [feet[0], feet[1] + lift, feet[2]], f));
    }
    out
}

/// Every source near `camera` and its light with settings `s`, by key.
fn gather(host: &dyn ClientHost, s: &dl::Settings, camera: [f64; 3]) -> HashMap<u64, (u8, [f64; 3])> {
    let mut out = HashMap::new();
    let Some(level) = host.level(host.dimension()) else { return out };
    let mine = me(host);
    let mut add = |key: u64, at: [f64; 3], f: Fact| {
        let light = f.level(s, || underwater(host, level, at));
        if light > 0 {
            out.insert(key, (light, at));
        }
    };
    match net::server_facts() {
        Some(facts) => {
            // The server's facts, with the player's own hands and place as the client has them
            // (at once and smooth); its burning is the server's.
            let mut burning = false;
            for (id, at, f) in facts {
                if f.who == Who::Me {
                    burning = f.burning;
                } else {
                    add(1 << 32 | u64::from(id), at, f);
                }
            }
            if let Some((_, at, mut f)) = mine {
                f.burning = burning;
                add(ME, at, f);
            }
        }
        None => {
            if let Some((_, at, f)) = mine {
                add(ME, at, f);
            }
            for (id, at, f) in seen(host, level, camera, f64::from(s.range)) {
                add(u64::from(id), at, f);
            }
        }
    }
    out
}

/// Each tick: the sources, and the toggle key's presses.
pub(crate) fn tick(host: &mut dyn ClientHost) {
    if with(|st| std::mem::take(&mut st.toggles)) % 2 == 1 {
        let mut s = store::settings(host);
        s.flip(dl::Opt::Enabled);
        store::set(s);
        with(|st| st.line = Some((s.enabled, 40)));
    }
    with(|st| {
        if let Some((_, left)) = &mut st.line {
            *left = left.saturating_sub(1);
            if *left == 0 {
                st.line = None;
            }
        }
    });
    let s = store::settings(host);
    let camera = host.local_player().and_then(|p| p.entity().eye_position(host));
    let found = match camera {
        Some(camera) if s.enabled && host.on(dl::ID) => gather(host, &s, camera),
        _ => HashMap::new(),
    };
    with(|st| {
        let old = std::mem::take(&mut st.sources);
        for (key, (level, now)) in found {
            let before = old.get(&key).map_or(now, |o| o.now);
            st.sources.insert(key, Source { level, before, now });
        }
    });
}

/// Each frame: the `max_sources` nearest sources within `range`, where the frame draws them.
pub(crate) fn visuals(host: &dyn ClientHost, event: &mut VisualsEvent<'_>) {
    if !host.on(dl::ID) {
        return;
    }
    let s = store::settings(host);
    let Some(camera) = host.camera() else { return };
    if !s.enabled {
        return;
    }
    let t = f64::from(camera.partial_tick);
    let reach = f64::from(s.range) * f64::from(s.range);
    let mut near: Vec<(f64, [f64; 3], u8)> = with(|st| {
        st.sources
            .values()
            .map(|src| {
                let at: [f64; 3] = std::array::from_fn(|i| src.before[i] + (src.now[i] - src.before[i]) * t);
                let d: f64 = (0..3).map(|i| (at[i] - camera.position[i]).powi(2)).sum();
                (d, at, src.level)
            })
            .filter(|(d, _, _)| *d <= reach)
            .collect()
    });
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut lights: Vec<([f64; 3], f32, f32)> = near
        .into_iter()
        .take(usize::from(s.max_sources))
        .map(|(_, at, level)| {
            let (radius, intensity) = s.light(level);
            (at, radius, intensity)
        })
        .collect();
    dl::blend(&mut lights);
    for (at, radius, intensity) in lights {
        if intensity > 0.0 {
            event.lights.push(ColoredLight::new(at, dl::LIGHT_RGB, radius, intensity));
        }
    }
}

// ---- the toggle key -----------------------------------------------------------------------

/// A key went down while playing (no screen open): the toggle key flips the main switch.
pub(crate) fn key(host: &dyn ClientHost, event: &InputKeyEvent<'_>) {
    if !host.on(dl::ID) || event.action != KeyAction::Press || host.observed_screen_id().is_some() {
        return;
    }
    let bound = store::toggle_key(host);
    if bound != dl::UNBOUND && bound == event.key {
        with(|st| st.toggles += 1);
    }
}

/// The line over the hotbar after a toggle ("Dynamic Lights: ON"), where and as the action bar
/// shows its message (`Gui.renderOverlayMessage`): 68 above the bottom, fading out over its
/// last second.
pub(crate) fn draw_line(host: &dyn ClientHost, draw: &mut dyn DrawContext) {
    let Some((on, left)) = with(|st| st.line) else { return };
    let state = Text::translate(if on { "options.on" } else { "options.off" }, Vec::new());
    let line = Text::translate(
        "options.generic_value",
        vec![Text::literal(dl::Opt::Enabled.caption(russian(host))), state],
    );
    let [w, h] = draw.size();
    let width = host.text_width(&line).map_or(0.0, |w| w as f32);
    let alpha = ((left as f32 * 255.0 / 20.0).min(255.0) as u32).max(8);
    let _ = draw.gui_paint(&[GuiDraw::Text {
        text: line,
        x: ((w - width) / 2.0).trunc() as i32,
        y: (h - 68.0 - 4.0) as i32,
        color: alpha << 24 | 0x00FF_FFFF,
        shadow: true,
    }]);
}
