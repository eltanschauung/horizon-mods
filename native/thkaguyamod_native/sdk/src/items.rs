//! The character weapons and items of Touhou Items Mod (`THKaguyaItems`): what each does when it
//! lands a hit, is let go after being held, is used, is eaten or is carried, with Grimoire of
//! Alice's weapons and point items (by Arekkuusu) beside them. Only the Mochi Hammer has a recipe
//! (see `gen_content.py`); the power and point items drop from the Touhou mobs.
//!
//! A hit that adds damage on top of the weapon's own lands eleven ticks later, once the target's
//! invulnerability from the first has run out, so armour and death are vanilla's.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use mod_api::{Compound, Flow, ItemPlace, ServerHost, Tag};

use crate::sdk::{Caster, EffectSpec, SpellHost};
use crate::shot::{form, shot, shot_full};
use crate::vec::{V3, v3};
use crate::{actor, bodies, fire_now, grow, middle, ray_box};

#[path = "more.rs"]
mod more;

const NS: &str = "thkaguyamod:";

/// The colours, by `THShotLib`'s numbers.
mod color {
    pub const RED: u8 = 0;
    pub const BLUE: u8 = 1;
    pub const GREEN: u8 = 2;
    pub const YELLOW: u8 = 3;
    pub const PURPLE: u8 = 4;
    pub const AQUA: u8 = 5;
    pub const WHITE: u8 = 7;
}

/// One of `item` gone from the player's hand, unless they are in creative.
fn spend(host: &mut dyn ServerHost, p: &Caster, item: &str) {
    if p.game_mode != 1 {
        host.spend_item(p.uuid, item, 1);
    }
}

/// An effect on a mob, through its saved data (players' go through `add_effect`).
fn mob_effect(host: &mut dyn ServerHost, target: u32, id: &str, amplifier: i8, duration: i32) {
    let e = Tag::Compound(
        [
            ("id".into(), Tag::String(id.into())),
            ("amplifier".into(), Tag::Byte(amplifier)),
            ("duration".into(), Tag::Int(duration)),
            ("show_particles".into(), Tag::Byte(1)),
        ]
        .into(),
    );
    merge(
        host,
        target,
        [("active_effects".into(), Tag::List(vec![e]))].into(),
    );
}

/// The living things within `reach` of the player other than them, their dolls and other players:
/// what a bomb or a stopped clock takes in.
fn around(host: &mut dyn ServerHost, p: &Caster, reach: f64) -> Vec<u32> {
    let me = p.entity_id as u32;
    let at = V3::from_array(p.position);
    bodies(host, &p.dimension)
        .into_iter()
        .filter(|(id, kind, b)| {
            *id != me
                && kind != "minecraft:player"
                && !kind.starts_with("alice_dolls:")
                && (middle(*b) - at).length() <= reach
        })
        .map(|(id, _, _)| id)
        .collect()
}

/// How long a held item may charge, as its `consumable` sets it (an hour, in ticks).
const CHARGE_TICKS: i32 = 72_000;

/// Damage still to come: (dimension, target, amount, attacker, ticks to wait).
struct Pending {
    dimension: String,
    target: u32,
    amount: f32,
    attacker: u32,
    wait: i32,
}

#[derive(Default)]
struct State {
    pending: Vec<Pending>,
    /// The Sword of Hisou's memory: the kind of mob each player hit last.
    hisou: HashMap<u128, String>,
    /// Right-click weapons' cooldowns.
    ready: HashMap<(u128, &'static str), Instant>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));

fn short(item: &str) -> Option<&str> {
    item.strip_prefix(NS)
}

fn effect(id: &str, amplifier: u8, duration: i32) -> EffectSpec<'_> {
    EffectSpec {
        id,
        amplifier,
        duration,
        ambient: false,
        particles: true,
    }
}

fn later(dimension: &str, target: u32, amount: f32, attacker: u32) {
    if let Ok(mut s) = STATE.lock() {
        s.pending.push(Pending {
            dimension: dimension.to_owned(),
            target,
            amount,
            attacker,
            wait: 11,
        });
    }
}

/// Whether `player` may use `what` again, and if so, starts its cooldown.
fn cooled(player: u128, what: &'static str, gap: Duration) -> bool {
    let Ok(mut s) = STATE.lock() else {
        return false;
    };
    let now = Instant::now();
    if s.ready.get(&(player, what)).is_some_and(|&t| now < t) {
        return false;
    }
    s.ready.insert((player, what), now + gap);
    true
}

fn float(t: Option<&Tag>) -> Option<f64> {
    match t? {
        Tag::Float(b) => Some(f64::from(f32::from_bits(*b))),
        Tag::Double(b) => Some(f64::from_bits(*b)),
        _ => None,
    }
}

/// A living mob's health and maximum health, from its saved data.
fn health(host: &mut dyn ServerHost, id: u32) -> Option<(f32, f32)> {
    let nbt = Tag::Compound(host.entity(id).and_then(|e| e.nbt(host))?);
    let now = float(nbt.get("Health"))? as f32;
    let mut max = 20.0;
    if let Some(Tag::List(list)) = nbt.get("attributes") {
        for a in list {
            if matches!(a.get("id"), Some(Tag::String(s)) if s.to_string_lossy() == "minecraft:max_health")
                && let Some(b) = float(a.get("base"))
            {
                max = b as f32;
            }
        }
    }
    Some((now, max))
}

fn merge(host: &mut dyn ServerHost, id: u32, nbt: Compound) {
    if let Some(e) = host.entity(id) {
        e.merge_nbt(host, nbt);
    }
}

fn undead(kind: &str) -> bool {
    const UNDEAD: [&str; 15] = [
        "zombie",
        "husk",
        "drowned",
        "zombie_villager",
        "zombified_piglin",
        "zoglin",
        "skeleton",
        "stray",
        "bogged",
        "wither_skeleton",
        "wither",
        "phantom",
        "skeleton_horse",
        "zombie_horse",
        "parched",
    ];
    UNDEAD.contains(&kind.rsplit(':').next().unwrap_or(kind))
}

fn eye(p: &Caster) -> V3 {
    V3::from_array(p.position) + v3(0.0, f64::from(p.eye_height), 0.0)
}

fn sound(host: &mut dyn ServerHost, p: &Caster, id: &str, pitch: f32) {
    if let Some(l) = host.level(&p.dimension) {
        l.play_sound(
            host,
            id,
            p.position,
            mod_api::SoundSource::Players,
            1.0,
            pitch,
        );
    }
}

/// The player's hit landed on `target`, a `kind`, with `item` in hand.
pub fn hurt_enemy(host: &mut dyn ServerHost, item: &str, target: u32, kind: &str) {
    let Some(name) = short(item) else { return };
    let Some(p) = host.acting_caster() else {
        return;
    };
    let me = p.entity_id as u32;
    match name {
        // Sets what it cuts alight for three to five seconds.
        "laevateinn" => {
            let ticks = 60 + crate::roll(41) as i16;
            merge(host, target, [("Fire".into(), Tag::Short(ticks))].into());
        }
        // Encases it in ice: frozen through, and too stiff to move for a while.
        "icicle_sword" => {
            let slow = Tag::Compound(
                [
                    ("id".into(), Tag::String("minecraft:slowness".into())),
                    ("amplifier".into(), Tag::Byte(5)),
                    ("duration".into(), Tag::Int(80)),
                    ("show_particles".into(), Tag::Byte(1)),
                ]
                .into(),
            );
            merge(
                host,
                target,
                [
                    ("TicksFrozen".into(), Tag::Int(300)),
                    ("active_effects".into(), Tag::List(vec![slow])),
                ]
                .into(),
            );
            sound(host, &p, "minecraft:block.glass.break", 1.4);
        }
        // A pillar's weight: five to twenty-two more.
        "onbashira" => {
            let extra = 5.0 + crate::roll(18) as f32;
            later(&p.dimension, target, extra, me);
        }
        // Cuts the undead deeply.
        "roukanken" if undead(kind) => later(&p.dimension, target, 16.0, me),
        // Takes it down to three quarters of its greatest health, at once.
        "kaigo_stick" => {
            if let Some((now, max)) = health(host, target) {
                let to = max * 0.75;
                if now > to {
                    host.set_mob_health(&p.dimension, target, to);
                }
            }
        }
        // Pounds it up and away.
        "mochi_hammer" => {
            let look = actor(&p).look();
            host.push_mob(&p.dimension, target, [look.x * 1.3, 0.55, look.z * 1.3]);
            sound(host, &p, "minecraft:block.anvil.land", 1.6);
        }
        // Learns the kind it last struck, and strikes that kind harder.
        "hisou_sword" => {
            let again = STATE.lock().ok().is_some_and(|mut s| {
                s.hisou.insert(p.uuid, kind.to_owned()).as_deref() == Some(kind)
            });
            if again {
                later(&p.dimension, target, 10.0, me);
            }
        }
        _ => {}
    }
}

/// Right click with `item`: the weapons that fire or reach.
pub fn use_item(host: &mut dyn ServerHost, item: &str) -> Flow {
    let Some(name) = short(item) else {
        return Flow::Continue;
    };
    let Some(p) = host.acting_caster() else {
        return Flow::Continue;
    };
    let look = actor(&p).look();
    let from = eye(&p) + look * 0.8;
    let me = p.entity_id as u32;
    match name {
        // A fan of fire.
        "laevateinn" => {
            if cooled(p.uuid, "laevateinn", Duration::from_millis(800)) {
                fire_now(&p.dimension, me, |f| {
                    f.wide(
                        from,
                        look,
                        0.0,
                        9999,
                        0.9,
                        1.4,
                        0.05,
                        V3::ZERO,
                        shot(form::BIG_LIGHT, color::RED, 0, 40, 0),
                        5,
                        40.0,
                        0.5,
                        0.0,
                    );
                });
                sound(host, &p, "minecraft:item.firecharge.use", 1.0);
            }
            Flow::Cancel
        }
        // The divine spear: a straight line of light, thrown hard.
        "spear_the_gungnir" => {
            if cooled(p.uuid, "gungnir", Duration::from_millis(1200)) {
                fire_now(&p.dimension, me, |f| {
                    for i in 0..8 {
                        let d = shot_full(form::BIG_LIGHT, color::RED, 1.0, 9.0, i, 60, 0);
                        f.shot(
                            from + look * (f64::from(i) * 0.6),
                            look,
                            0.0,
                            1.6,
                            2.4,
                            0.08,
                            V3::ZERO,
                            d,
                        );
                    }
                });
                sound(host, &p, "minecraft:item.trident.throw", 0.8);
            }
            Flow::Cancel
        }
        // Reaps the soul in front: pulls it in, or with sneak sends it away.
        "death_scythe" => {
            if !cooled(p.uuid, "scythe", Duration::from_millis(500)) {
                return Flow::Cancel;
            }
            let start = eye(&p);
            let hit = bodies(host, &p.dimension)
                .into_iter()
                .filter(|(id, _, _)| *id != me)
                .filter_map(|(id, _, b)| Some((ray_box(start, look, 16.0, grow(b, 0.3))?, id, b)))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, id, b)) = hit {
                let to_me = V3::from_array(p.position) - middle(b);
                let len = to_me.length().max(1e-6);
                let dir = to_me * (1.0 / len);
                let dir = if p.sneaking { dir * -1.0 } else { dir };
                let speed = 0.6 * (len / 4.0).clamp(1.0, 3.0);
                host.push_mob(
                    &p.dimension,
                    id,
                    [dir.x * speed, dir.y * speed + 0.25, dir.z * speed],
                );
                sound(host, &p, "minecraft:entity.phantom.flap", 0.6);
            }
            Flow::Cancel
        }
        // Reimu's gohei: a spread of amulets.
        "hakurei_gohei" => {
            if cooled(p.uuid, "gohei", Duration::from_millis(600)) {
                fire_now(&p.dimension, me, |f| {
                    f.wide(
                        from,
                        look,
                        0.0,
                        9999,
                        0.7,
                        1.2,
                        0.04,
                        V3::ZERO,
                        shot(form::AMULET, color::RED, 0, 40, 0),
                        5,
                        30.0,
                        0.3,
                        0.0,
                    );
                });
                sound(host, &p, "minecraft:entity.evoker.cast_spell", 1.4);
            }
            Flow::Cancel
        }
        // Sanae's gohei: a gust that sends what is in front away.
        "miko_stick" => {
            if cooled(p.uuid, "miko_stick", Duration::from_millis(700)) {
                fire_now(&p.dimension, me, |f| {
                    f.wide(
                        from,
                        look,
                        0.0,
                        9999,
                        1.0,
                        1.4,
                        0.0,
                        V3::ZERO,
                        shot(form::WIND, color::GREEN, 0, 30, 0),
                        5,
                        40.0,
                        0.3,
                        0.0,
                    );
                });
                sound(host, &p, "minecraft:entity.breeze.wind_burst", 1.0);
            }
            Flow::Cancel
        }
        // Amulets that go for the nearest thing in sight.
        "homing_amulet" => {
            if cooled(p.uuid, "homing", Duration::from_millis(500)) {
                let aim = crate::find_target(host, &p).map_or(look, |(_, at)| {
                    let d = at - from;
                    let l = d.length().max(1e-6);
                    d * (1.0 / l)
                });
                fire_now(&p.dimension, me, |f| {
                    f.wide(
                        from,
                        aim,
                        0.0,
                        9999,
                        0.8,
                        1.6,
                        0.06,
                        V3::ZERO,
                        shot(form::TALISMAN, color::RED, 0, 50, 0),
                        3,
                        12.0,
                        0.3,
                        0.0,
                    );
                });
                sound(host, &p, "minecraft:entity.evoker.cast_spell", 1.6);
            }
            Flow::Cancel
        }
        // A wide spread of talismans.
        "diffusion_amulet" => {
            if cooled(p.uuid, "diffusion", Duration::from_millis(700)) {
                fire_now(&p.dimension, me, |f| {
                    f.wide(
                        from,
                        look,
                        0.0,
                        9999,
                        0.6,
                        1.0,
                        0.03,
                        V3::ZERO,
                        shot(form::TALISMAN, color::PURPLE, 0, 40, 0),
                        9,
                        80.0,
                        0.3,
                        0.0,
                    );
                });
                sound(host, &p, "minecraft:entity.evoker.cast_spell", 1.2);
            }
            Flow::Cancel
        }
        // One great talisman, hard-hitting.
        "spiritual_strike_talisman" => {
            if cooled(p.uuid, "strike", Duration::from_millis(1500)) {
                fire_now(&p.dimension, me, |f| {
                    f.shot(
                        from,
                        look,
                        0.0,
                        0.9,
                        1.6,
                        0.05,
                        V3::ZERO,
                        shot_full(form::TALISMAN, color::RED, 3.0, 14.0, 0, 60, 0),
                    );
                });
                sound(host, &p, "minecraft:item.trident.throw", 1.2);
            }
            Flow::Cancel
        }
        // A fan of ghostly butterflies.
        "yuyuko_fan" => {
            if cooled(p.uuid, "yuyuko", Duration::from_millis(1000)) {
                fire_now(&p.dimension, me, |f| {
                    f.wide(
                        from,
                        look,
                        0.0,
                        9999,
                        0.5,
                        0.9,
                        0.02,
                        V3::ZERO,
                        shot(form::BUTTERFLY, color::PURPLE, 0, 60, 0),
                        7,
                        70.0,
                        0.3,
                        0.0,
                    );
                });
                sound(host, &p, "minecraft:entity.allay.item_thrown", 0.7);
            }
            Flow::Cancel
        }
        // Squirts of water.
        "kappa_water_pistol" => {
            if cooled(p.uuid, "pistol", Duration::from_millis(250)) {
                fire_now(&p.dimension, me, |f| {
                    f.shot(
                        from,
                        look,
                        0.0,
                        1.2,
                        1.6,
                        0.05,
                        crate::pattern::gravity(-0.03),
                        shot(form::PEARL, color::AQUA, 0, 30, 0),
                    );
                });
                merge(host, me, [("Fire".into(), Tag::Short(0))].into());
                sound(host, &p, "minecraft:entity.generic.splash", 1.6);
            }
            Flow::Cancel
        }
        // Kaguya's mallet: for a while, bigger and stronger.
        "miracle_mallet" => {
            if cooled(p.uuid, "mallet", Duration::from_secs(30)) {
                host.apply_effect(p.uuid, &effect("minecraft:strength", 1, 400));
                host.apply_effect(p.uuid, &effect("minecraft:resistance", 0, 400));
                sound(host, &p, "minecraft:block.amethyst_block.chime", 0.6);
            }
            Flow::Cancel
        }
        // Time stops for everything around but its holder.
        "sakuya_stopwatch" => {
            if cooled(p.uuid, "stopwatch", Duration::from_secs(20)) {
                for id in around(host, &p, 16.0) {
                    mob_effect(host, id, "minecraft:slowness", 9, 100);
                }
                host.apply_effect(p.uuid, &effect("minecraft:speed", 1, 100));
                sound(host, &p, "minecraft:block.bell.use", 0.5);
            }
            Flow::Cancel
        }
        // Suika's gourd never runs dry.
        "ibuki_gourd" => {
            if cooled(p.uuid, "gourd", Duration::from_secs(10)) {
                host.apply_effect(p.uuid, &effect("minecraft:strength", 1, 1200));
                host.apply_effect(p.uuid, &effect("minecraft:nausea", 0, 100));
                sound(host, &p, "minecraft:entity.generic.drink", 0.8);
            }
            Flow::Cancel
        }
        "illness_charm" => {
            if cooled(p.uuid, "charm", Duration::from_secs(30)) {
                host.apply_effect(p.uuid, &effect("minecraft:regeneration", 1, 200));
                sound(host, &p, "minecraft:block.amethyst_block.resonate", 1.2);
            }
            Flow::Cancel
        }
        // Power items: stronger for a while.
        "small_power_item" | "big_power_item" | "full_power_item" => {
            let (amp, ticks) = match name {
                "small_power_item" => (0, 600),
                "big_power_item" => (1, 1200),
                _ => (2, 2400),
            };
            host.apply_effect(p.uuid, &effect("minecraft:strength", amp, ticks));
            if name == "full_power_item" {
                host.apply_effect(p.uuid, &effect("minecraft:speed", 1, ticks));
                host.apply_effect(p.uuid, &effect("minecraft:haste", 1, ticks));
            }
            spend(host, &p, item);
            sound(host, &p, "minecraft:entity.experience_orb.pickup", 0.8);
            Flow::Cancel
        }
        // An extra life: healed, and shielded.
        "extend_item" => {
            host.apply_effect(p.uuid, &effect("minecraft:instant_health", 1, 1));
            host.apply_effect(p.uuid, &effect("minecraft:absorption", 1, 2400));
            host.apply_effect(p.uuid, &effect("minecraft:regeneration", 1, 200));
            spend(host, &p, item);
            sound(host, &p, "minecraft:entity.player.levelup", 1.4);
            Flow::Cancel
        }
        // A bomb: everything around is struck at once.
        "bomb_item" => {
            if cooled(p.uuid, "bomb", Duration::from_secs(2)) {
                for id in around(host, &p, 8.0) {
                    host.hurt_in(&p.dimension, id, 12.0, "minecraft:player_attack", Some(me));
                }
                fire_now(&p.dimension, me, |f| {
                    f.circle_simple(
                        eye(&p),
                        v3(0.0, 1.0, 0.0),
                        0.6,
                        shot(form::BIG_LIGHT, crate::shot::RAINBOW, 0, 14, 0),
                        16,
                    );
                });
                spend(host, &p, item);
                sound(host, &p, "minecraft:entity.generic.explode", 1.2);
            }
            Flow::Cancel
        }
        // Slows the world around, and quickens its holder.
        "time_orb" => {
            for id in around(host, &p, 12.0) {
                mob_effect(host, id, "minecraft:slowness", 3, 200);
            }
            host.apply_effect(p.uuid, &effect("minecraft:speed", 1, 200));
            host.apply_effect(p.uuid, &effect("minecraft:haste", 1, 200));
            spend(host, &p, item);
            sound(host, &p, "minecraft:block.bell.resonate", 1.4);
            Flow::Cancel
        }
        _ => more::use_item(host, item, &p).unwrap_or(Flow::Continue),
    }
}

/// `item` let go after being held `held` ticks, by the player with entity id `entity`. The dashes
/// run where the window's player is; the shots where the world is.
pub fn release(host: &mut dyn ServerHost, item: &str, entity: Option<i32>, time_left: i32) -> bool {
    let Some(name) = short(item) else {
        return false;
    };
    if !matches!(
        name,
        "hakurouken" | "roukanken" | "tengu_fan" | "nuclear_control_rod" | "mini_hakkero"
    ) && !more::charged(name)
    {
        return false;
    }
    let held = (CHARGE_TICKS - time_left).max(0);
    let server = entity
        .and_then(|id| host.casters().into_iter().find(|p| p.entity_id == id))
        .or_else(|| host.acting_caster());
    let look = server
        .as_ref()
        .map_or(v3(0.0, 0.0, 1.0), |p| actor(p).look());
    let flat = {
        let f = v3(look.x, 0.0, look.z);
        let l = f.length();
        if l > 1e-6 { f * (1.0 / l) } else { f }
    };
    let kick = |host: &mut dyn ServerHost, velocity: V3| {
        if let Some(player) = &server
            && let Some(player) = host.player(player.uuid)
        {
            player.entity().push(host, velocity.array());
        }
    };
    match name {
        // Youmu's short sword: a quick step forward.
        "hakurouken" if held >= 3 => {
            kick(host, flat * 1.5 + v3(0.0, 0.15, 0.0));
        }
        // Youmu's long sword: a longer charge, a longer dash.
        "roukanken" if held >= 3 => {
            let far = f64::from(held.min(20)) * 0.15;
            kick(host, flat * far + v3(0.0, 0.2, 0.0));
        }
        // A gust, as strong as it was drawn back.
        "tengu_fan" => {
            if let Some(p) = &server {
                let speed = f64::from(held.min(24)) / 7.0 + 0.3;
                let from = eye(p) + look * 0.8;
                fire_now(&p.dimension, p.entity_id as u32, |f| {
                    f.wide(
                        from,
                        look,
                        0.0,
                        9999,
                        speed,
                        speed,
                        0.0,
                        V3::ZERO,
                        shot(form::WIND, color::WHITE, 0, 40, 0),
                        3,
                        20.0,
                        0.3,
                        0.0,
                    );
                });
                sound(host, p, "minecraft:entity.breeze.wind_burst", 1.2);
            }
        }
        // Utsuho's rod: a sun of a shot, and the kick of firing it.
        "nuclear_control_rod" if held >= 5 => {
            if let Some(p) = &server {
                let from = eye(p) + look * 1.2;
                let size = 1.0 + f64::from(held.min(60)) / 30.0;
                let damage = 6.0 + held.min(60) as f32 / 4.0;
                fire_now(&p.dimension, p.entity_id as u32, |f| {
                    f.shot(
                        from,
                        look,
                        0.0,
                        0.6,
                        0.9,
                        0.02,
                        V3::ZERO,
                        shot_full(form::BIG, color::YELLOW, size as f32, damage, 0, 80, 0),
                    );
                });
                sound(host, p, "minecraft:entity.blaze.shoot", 0.6);
            }
            let back = f64::from(held.min(100)) / 25.0;
            kick(host, look * -back.min(3.0));
        }
        // Marisa's furnace: Master Spark, a beam of rainbow light as long as it was charged.
        "mini_hakkero" if held >= 10 => {
            if let Some(p) = &server {
                let from = eye(p) + look * 1.0;
                let length = 8 + held.min(60) / 3;
                let damage = 8.0 + held.min(60) as f32 / 6.0;
                fire_now(&p.dimension, p.entity_id as u32, |f| {
                    for i in 0..length {
                        let c = [
                            color::RED,
                            color::YELLOW,
                            color::GREEN,
                            color::AQUA,
                            color::BLUE,
                            color::PURPLE,
                        ][i as usize % 6];
                        let d = shot_full(form::BIG_LIGHT, c, 2.2, damage, i / 2, 50, 0);
                        f.shot(
                            from + look * (f64::from(i) * 0.5),
                            look,
                            0.0,
                            1.4,
                            2.0,
                            0.05,
                            V3::ZERO,
                            d,
                        );
                    }
                });
                sound(host, p, "minecraft:entity.warden.sonic_boom", 1.2);
            }
            kick(host, look * -1.2);
        }
        n if more::charged(n) => {
            if let Some(p) = &server {
                more::release(host, n, p, held);
            }
        }
        _ => return false,
    }
    true
}

/// `item` eaten by the player `uuid`.
pub fn eaten(host: &mut dyn ServerHost, item: &str, uuid: u128) {
    if short(item) == Some("heavenly_peach") {
        host.apply_effect(uuid, &effect("minecraft:resistance", 2, 400));
    }
}

/// A tick of a carried stack: the Onbashira and the Mochi Hammer weigh down whoever holds them;
/// Yuuka's Parasol lets them float down.
pub fn carried(host: &mut dyn ServerHost, slot: i32, selected: bool) {
    if !selected {
        return;
    }
    let Some(player) = host.players().acting_player() else {
        return;
    };
    let Some(item) = (ItemPlace::PlayerSlot {
        player: player.uuid(),
        slot,
    })
    .stack(host)
    .map(|s| s.item().id(host)) else {
        return;
    };
    let what = match short(&item) {
        Some("onbashira" | "mochi_hammer") => "minecraft:slowness",
        Some("yuuka_parasol") => "minecraft:slow_falling",
        _ => return,
    };
    if let Some(p) = host.acting_caster() {
        host.apply_effect(
            p.uuid,
            &EffectSpec {
                particles: false,
                ..effect(what, 0, 10)
            },
        );
    }
}

/// A tick of a level: the extra damage whose time has come lands.
pub fn tick(host: &mut dyn ServerHost, dimension: &str) {
    more::worn(host, dimension);
    let due: Vec<Pending> = {
        let Ok(mut s) = STATE.lock() else { return };
        if s.pending.is_empty() {
            return;
        }
        let mut due = Vec::new();
        s.pending.retain_mut(|p| {
            if p.dimension != dimension {
                return true;
            }
            p.wait -= 1;
            if p.wait > 0 {
                return true;
            }
            due.push(Pending {
                dimension: String::new(),
                ..*p
            });
            false
        });
        due
    };
    for p in due {
        host.hurt_in(
            dimension,
            p.target,
            p.amount,
            "minecraft:player_attack",
            Some(p.attacker),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undead_are_known_by_kind() {
        assert!(undead("minecraft:zombie"));
        assert!(undead("minecraft:wither_skeleton"));
        assert!(!undead("minecraft:creeper"));
    }

    #[test]
    fn only_this_mods_items_answer() {
        assert_eq!(short("thkaguyamod:laevateinn"), Some("laevateinn"));
        assert_eq!(short("minecraft:iron_sword"), None);
    }

    #[test]
    fn cooldowns_hold_back_a_second_use() {
        assert!(cooled(7, "test", Duration::from_secs(5)));
        assert!(!cooled(7, "test", Duration::from_secs(5)));
        assert!(cooled(8, "test", Duration::from_secs(5)));
    }
}
