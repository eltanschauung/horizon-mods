//! The rest of the four mods' items that do something (`gen_more.py` lists them): Touhou Items
//! Mod's shots, lasers, knives and artefacts, SPELLCARD PLUS's Hina doll, and what Grimoire of
//! Alice's masks and Touhou Items Mod's hats and robe give their wearer. A submodule of `items`.

use crate::sdk::SpellHost;
use std::time::Duration;

use crate::sdk::{Caster, EffectSpec};
use mod_api::{Compound, EquipmentSlot, Flow, ServerHost};

use super::{color, cooled, effect, eye, short, sound, spend};
use crate::actor;
use crate::fire_now;
use crate::shot::{RAINBOW, RANDOM, form, shot, shot_full};
use crate::vec::{V3, angle, v3};

/// Touhou Items Mod's shot items, in the order of its `shotNames`: the item, its form
/// (`shotTypeTrans`) and its speed.
const SHOTS: [(&str, u8, f64); 19] = [
    ("small_shot", form::SMALL, 0.5),
    ("tiny_shot", form::TINY, 0.5),
    ("medium_shot", form::MEDIUM, 0.4),
    ("big_shot", form::BIG, 0.25),
    ("star_shot", form::STAR, 0.5),
    ("small_star_shot", form::SMALL_STAR, 0.5),
    ("circle_shot", form::CIRCLE, 0.5),
    ("scale_shot", form::SCALE, 0.55),
    ("butterfly_shot", form::BUTTERFLY, 0.3),
    ("light_shot", form::LIGHT, 0.5),
    ("knife_shot", form::KNIFE, 0.65),
    ("heart_shot", form::HEART, 0.4),
    ("kunai_shot", form::KUNAI, 0.6),
    ("talisman_shot", form::TALISMAN, 0.5),
    ("big_light_shot", form::BIG_LIGHT, 0.4),
    ("rice_shot", form::RICE, 0.5),
    ("oval_shot", form::OVAL, 0.4),
    ("crystal_shot", form::CRYSTAL, 0.45),
    ("arrow_shot", form::ARROW, 0.5),
];

/// A right click with one of these; `None` when the item is none of them.
pub(super) fn use_item(host: &mut dyn ServerHost, item: &str, p: &Caster) -> Option<Flow> {
    let name = short(item)?;
    let look = actor(p).look();
    let from = eye(p) + look * 0.8;
    let me = p.entity_id as u32;
    // A shot item: one shot of its own form, in a colour of its own, used up.
    if let Some(&(_, shape, speed)) = SHOTS.iter().find(|s| s.0 == name) {
        if cooled(p.uuid, "shot_item", Duration::from_millis(150)) {
            fire_now(&p.dimension, me, |f| {
                f.shot(
                    from,
                    look,
                    0.0,
                    speed,
                    speed,
                    0.0,
                    V3::ZERO,
                    shot(shape, RANDOM, 0, 60, 0),
                )
            });
            spend(host, p, item);
            sound(host, p, "minecraft:entity.arrow.shoot", 1.6);
        }
        return Some(Flow::Cancel);
    }
    match name {
        // A laser item: a line of light as long as its kind.
        "middle_laser" | "short_laser" | "long_laser" => {
            if cooled(p.uuid, "laser_item", Duration::from_millis(400)) {
                let length = match name {
                    "short_laser" => 4,
                    "long_laser" => 10,
                    _ => 6,
                };
                fire_now(&p.dimension, me, |f| {
                    for i in 0..length {
                        let d = shot_full(form::LIGHT, color::WHITE, 0.4, 3.0, i, 40, 0);
                        f.shot(
                            from + look * (f64::from(i) * 0.4),
                            look,
                            0.0,
                            1.2,
                            1.2,
                            0.0,
                            V3::ZERO,
                            d,
                        );
                    }
                });
                spend(host, p, item);
                sound(host, p, "minecraft:entity.evoker.cast_spell", 1.8);
            }
        }
        // Shot material: a tiny white shot straight ahead (`ShotData.shot(1, 7)`).
        "shot_material" => {
            if cooled(p.uuid, "shot_item", Duration::from_millis(150)) {
                fire_now(&p.dimension, me, |f| {
                    f.shot(
                        from,
                        look,
                        0.0,
                        1.0,
                        1.0,
                        0.0,
                        V3::ZERO,
                        shot(form::TINY, color::WHITE, 0, 40, 0),
                    )
                });
                spend(host, p, item);
                sound(host, p, "minecraft:entity.arrow.shoot", 1.8);
            }
        }
        // Sakuya's knives, thrown; the white one strikes hardest.
        "silver_knife_blue" | "silver_knife_red" | "silver_knife_green" | "silver_knife_white" => {
            if cooled(p.uuid, "knife", Duration::from_millis(200)) {
                let (c, damage) = match name {
                    "silver_knife_red" => (color::RED, 4.0),
                    "silver_knife_green" => (color::GREEN, 4.0),
                    "silver_knife_white" => (color::WHITE, 6.0),
                    _ => (color::BLUE, 3.0),
                };
                fire_now(&p.dimension, me, |f| {
                    f.shot(
                        from,
                        look,
                        0.0,
                        0.9,
                        0.9,
                        0.0,
                        v3(0.0, -0.01, 0.0),
                        shot_full(form::KNIFE, c, 0.3, damage, 0, 60, 0),
                    )
                });
                spend(host, p, item);
                sound(host, p, "minecraft:entity.arrow.shoot", 1.2);
            }
        }
        // A ring of purple will-o'-wisps upward about its holder (`ItemSoulTorch`).
        "soul_torch" => {
            if cooled(p.uuid, "soul_torch", Duration::from_millis(600)) {
                let up = angle(p.yaw, -90.0);
                fire_now(&p.dimension, me, |f| {
                    f.ring(
                        eye(p),
                        up,
                        0.0,
                        9999,
                        0.01,
                        0.4,
                        0.03,
                        V3::ZERO,
                        shot_full(26, color::PURPLE, 0.4, 7.0, 0, 480, 0),
                        3,
                        0.3,
                        30.0,
                        0.0,
                    )
                });
                spend(host, p, item);
                sound(host, p, "minecraft:entity.blaze.shoot", 1.4);
            }
        }
        // The dragon's jewel: a great rainbow bullet.
        "dragon_neck_jewel" => {
            if cooled(p.uuid, "dragon_jewel", Duration::from_secs(1)) {
                fire_now(&p.dimension, me, |f| {
                    f.shot(
                        from,
                        look,
                        0.0,
                        0.4,
                        0.9,
                        0.03,
                        V3::ZERO,
                        shot_full(form::BIG_LIGHT, RAINBOW, 2.0, 12.0, 0, 100, 0),
                    )
                });
                sound(host, p, "minecraft:entity.ender_dragon.shoot", 1.0);
            }
        }
        // Koishi's closed eye: a moment unseen.
        "closed_third_eye" => {
            host.apply_effect(p.uuid, &effect("minecraft:invisibility", 0, 20));
        }
        // Hina's doll takes a little hurt away, and floats off again as a Floating Hina Doll.
        "instant_hina_doll" => {
            if p.health < 20.0 && p.health > 0.0 {
                host.apply_effect(p.uuid, &effect("minecraft:regeneration", 3, 7));
                spend(host, p, item);
                let id = mod_api::Identifier::parse("thkaguyamod:floating_hina_doll").unwrap();
                if let (Some(player), Some(item)) = (host.player(p.uuid), host.item_key(&id)) {
                    let _ = player.give_stack(host, mod_api::ItemStack::of(item));
                }
            }
        }
        // The decoy doll stands where it is set down, for monsters to strike at.
        "cursed_decoy_doll" => {
            let at = V3::from_array(p.position) + v3(look.x, 0.0, look.z) * 1.5;
            if host
                .summon(
                    &p.dimension,
                    "thmobs:cursed_decoy_doll",
                    at.array(),
                    Compound::default(),
                )
                .is_some()
            {
                spend(host, p, item);
            }
        }
        _ => return None,
    }
    Some(Flow::Cancel)
}

/// The items held back and let go: whether `name` is one.
pub(super) fn charged(name: &str) -> bool {
    matches!(name, "hourai_jeweled_branch" | "bloodthirsty_yin_yang_orb")
}

/// One let go after `held` ticks, by `p`.
pub(super) fn release(host: &mut dyn ServerHost, name: &str, p: &Caster, held: i32) {
    let look = actor(p).look();
    let from = eye(p) + look * 0.8;
    match name {
        // The jeweled branch: a circle of pearls in five colours, wider for a longer draw.
        "hourai_jeweled_branch" if held >= 5 => {
            let way = 5 + held.min(40) / 4;
            fire_now(&p.dimension, p.entity_id as u32, |f| {
                for (i, c) in [
                    color::RED,
                    color::BLUE,
                    color::GREEN,
                    color::YELLOW,
                    color::PURPLE,
                ]
                .into_iter()
                .enumerate()
                {
                    let d = shot(form::PEARL, c, i as i32 * 2, 80, 0);
                    f.circle(
                        from,
                        look,
                        0.0,
                        9999,
                        0.3,
                        0.6,
                        0.02,
                        V3::ZERO,
                        d,
                        way,
                        0.5,
                        i as f32 * 7.0,
                    );
                }
            });
            sound(host, p, "minecraft:entity.evoker.cast_spell", 1.2);
        }
        // The bloodthirsty orb: thrown as hard as it was drawn.
        "bloodthirsty_yin_yang_orb" if held >= 3 => {
            let speed = 0.4 + f64::from(held.min(30)) / 30.0;
            fire_now(&p.dimension, p.entity_id as u32, |f| {
                f.shot(
                    from,
                    look,
                    0.0,
                    speed,
                    speed,
                    0.0,
                    v3(0.0, -0.02, 0.0),
                    shot_full(form::BIG, color::RED, 1.0, 8.0, 0, 80, 0),
                );
            });
            sound(host, p, "minecraft:entity.snowball.throw", 0.6);
        }
        _ => {}
    }
}

/// What a worn item gives: below the wearer's level `at` the first effects, above it the second, as
/// Grimoire of Alice's masks (one with no level gives the first always). Each is (id, amplifier).
type Worn = (
    &'static str,
    EquipmentSlot,
    i32,
    &'static [(&'static str, u8)],
    &'static [(&'static str, u8)],
);
const WORN: [Worn; 12] = [
    (
        "fox_mask",
        EquipmentSlot::Head,
        40,
        &[("speed", 0)],
        &[("resistance", 4), ("speed", 0)],
    ),
    (
        "raiden_mask",
        EquipmentSlot::Head,
        50,
        &[("weakness", 0)],
        &[("weakness", 0), ("haste", 2)],
    ),
    (
        "monkey_mask",
        EquipmentSlot::Head,
        60,
        &[("slowness", 2)],
        &[("invisibility", 0)],
    ),
    (
        "hyottoko_mask",
        EquipmentSlot::Head,
        70,
        &[("jump_boost", 2)],
        &[("regeneration", 4), ("jump_boost", 4)],
    ),
    (
        "fuku_no_kami_mask",
        EquipmentSlot::Head,
        80,
        &[("nausea", 0)],
        &[("fire_resistance", 0), ("night_vision", 0)],
    ),
    (
        "uba_mask",
        EquipmentSlot::Head,
        90,
        &[("mining_fatigue", 4), ("blindness", 2)],
        &[("weakness", 2)],
    ),
    (
        "hannya_mask",
        EquipmentSlot::Head,
        100,
        &[("strength", 0)],
        &[("strength", 4)],
    ),
    (
        "koomote_mask",
        EquipmentSlot::Head,
        110,
        &[("hunger", 2)],
        &[("absorption", 4)],
    ),
    (
        "mask_of_hope",
        EquipmentSlot::Head,
        120,
        &[("wither", 4)],
        &[("poison", 4)],
    ),
    (
        "kokoros_masks",
        EquipmentSlot::Head,
        i32::MAX,
        &[
            ("strength", 3),
            ("speed", 4),
            ("fire_resistance", 4),
            ("water_breathing", 4),
            ("resistance", 3),
            ("jump_boost", 4),
            ("night_vision", 4),
        ],
        &[],
    ),
    // Touhou Items Mod: the fire rat's robe does not burn, the kappa's cap breathes water.
    (
        "fire_rat_robe",
        EquipmentSlot::Chest,
        i32::MAX,
        &[("fire_resistance", 0)],
        &[],
    ),
    (
        "kappa_cap",
        EquipmentSlot::Head,
        i32::MAX,
        &[("water_breathing", 0)],
        &[],
    ),
];

/// Once a second, what each player in `dimension` wears works on them.
pub(super) fn worn(host: &mut dyn ServerHost, dimension: &str) {
    for p in host.casters() {
        if p.dimension != dimension
            || p.health <= 0.0
            || !cooled(p.uuid, "worn", Duration::from_millis(1000))
        {
            continue;
        }
        let Some(living) = host
            .player(p.uuid)
            .and_then(|player| host.living(player.entity()))
        else {
            continue;
        };
        let head = living
            .equipment(host, EquipmentSlot::Head)
            .map(|stack| stack.item().id(host));
        let chest = living
            .equipment(host, EquipmentSlot::Chest)
            .map(|stack| stack.item().id(host));
        for (item, slot, at, low, high) in WORN {
            let on = if slot == EquipmentSlot::Head {
                &head
            } else {
                &chest
            };
            if on.as_deref().and_then(short) != Some(item) {
                continue;
            }
            for (id, amplifier) in if p.xp_level <= at { low } else { high } {
                // Night vision flickers out under ten seconds left; the rest last past the next second.
                let ticks = if *id == "night_vision" { 400 } else { 60 };
                let id = format!("minecraft:{id}");
                let e = EffectSpec {
                    particles: false,
                    ..effect(&id, *amplifier, ticks)
                };
                host.apply_effect(p.uuid, &e);
            }
        }
    }
}
