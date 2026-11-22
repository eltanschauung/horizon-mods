// SPDX-License-Identifier: EUPL-1.2-or-later
// Native adaptation of Arcadian Dream by Maxmani and contributors.

use mod_api::serde_json::{Value, json};
use mod_api::{
    GameMode, HostTransaction, Identifier, InteractionResult, ItemPlace, ServerHost, UseItemEvent,
};

use crate::{config, items};

pub const FAMILIES: [&str; 6] = ["amulet", "bubble", "circle", "kunai", "pellet", "star"];

pub fn fire(
    host: &mut dyn ServerHost,
    event: &UseItemEvent<'_>,
    family: &str,
) -> Option<InteractionResult> {
    fire_hand(host, event.player, event.hand, event.stack, family)
}

pub fn fire_hand(
    host: &mut dyn ServerHost,
    uuid: u128,
    hand: mod_api::Hand,
    stack: &mod_api::ItemStack,
    family: &str,
) -> Option<InteractionResult> {
    let player = host.player(uuid)?;
    let owner = host.living(player.entity())?;
    let level = player.level(host)?;
    let group = host.item_cooldown_group(stack);
    if player.has_item_cooldown(host, &group) == Some(true) {
        return Some(InteractionResult::Consume);
    }
    let data = items::shot_settings(stack, family);
    let durability = host.max_damage(stack.item()).unwrap_or(100).max(1);
    if stack.damage() >= durability.saturating_sub(1) {
        return Some(InteractionResult::Consume);
    }
    let [yaw, pitch] = player.entity().rotation(host)?;
    let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    let view = [
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    ];
    let day_time = level.day_time(host).unwrap_or_default() as f64;
    let density = number(&data, "density", 1.0).clamp(1.0, 40.0) as usize;
    let speed = number(&data, "speed", 0.8).clamp(0.01, 4.0);
    let mut projectile_data = data.clone();
    projectile_data["owner"] = json!(format!("{uuid:032x}"));
    projectile_data["power"] = json!(
        (number(&data, "power", 2.0)
            * number(config::config(), "danmakuDamageMultiplier", 1.0).clamp(1.0, 4.0))
        .clamp(0.1, 60.0)
    );
    let projectile_type = Identifier::parse(&format!("arcadiandream:{family}_bullet")).ok()?;
    let pattern = data["pattern"].as_str().unwrap_or("spread");
    let spread = number(&data, "divergence", 0.0).clamp(0.0, 90.0)
        + if pattern == "spread" {
            density.saturating_sub(1) as f64
        } else {
            0.0
        };
    let mut picture = stack.clone();
    items::set_custom(&mut picture, &projectile_data)?;
    let color = data["color"]
        .as_i64()
        .unwrap_or(0xff0000)
        .clamp(0, 0xff_ffff);
    let scale = ((number(&data, "scale", 1.0) - 1.0) / 0.75)
        .round()
        .clamp(0.0, 4.0) as u8;
    picture
        .apply_components(
            &[
                (
                    "minecraft:dyed_color".into(),
                    mod_api::Tag::Int(color as i32),
                ),
                (
                    "minecraft:item_model".into(),
                    mod_api::Tag::String(
                        format!("arcadiandream:arcadian_projectile/{family}_{scale}_0")
                            .as_str()
                            .into(),
                    ),
                ),
            ]
            .into(),
        )
        .ok()?;
    let mut fired = false;
    for (index, (angle, layer)) in directions(pattern, density).into_iter().enumerate() {
        let offset = ((day_time + index as f64 * 13.1).sin() * spread).to_radians();
        let (sin, cos) = (angle + offset).sin_cos();
        let pace = speed * (1.0 - layer as f64 / density as f64).max(0.05);
        let velocity = [
            (view[0] * cos - view[2] * sin) * pace,
            view[1] * pace,
            (view[0] * sin + view[2] * cos) * pace,
        ];
        if let Some(projectile) =
            host.spawn_projectile_from_rotation(owner, &projectile_type, &picture, 0.0, 1.0, 0.0)
        {
            let entity = projectile.entity();
            entity.set_velocity(host, velocity);
            entity.set_no_gravity(host, true);
            fired = true;
        }
    }
    if !fired {
        return None;
    }
    let cooldown = number(&data, "cooldown", 5.0).clamp(1.0, 600.0)
        * number(config::config(), "danmakuCooldownMultiplier", 1.0).clamp(1.0, 4.0);
    let cooldown = if data["timeOrbUsed"] == true {
        cooldown / 2.0
    } else {
        cooldown
    } as i32;
    if config::config()["cooldownPerBulletType"]
        .as_bool()
        .unwrap_or(true)
    {
        player.set_item_cooldown(host, &group, cooldown);
    } else {
        for family in FAMILIES {
            player.set_item_cooldown(host, &format!("arcadiandream:{family}_shot"), cooldown);
        }
    }
    if player.game_mode(host) != Some(GameMode::Creative) {
        let place = ItemPlace::PlayerHand { player: uuid, hand };
        let used_stack = stack.clone();
        let _ = host.transaction(|host, tx| {
            if place.stack(host).as_ref() != Some(&used_stack) {
                return Err(());
            }
            place
                .edit(host, tx, |current| {
                    current.set_damage(
                        (current.damage() + 1 + density as u32 / 4).min(durability - 1),
                    );
                })
                .then_some(())
                .ok_or(())
        });
    }
    Some(InteractionResult::SuccessServer)
}

pub fn number(data: &Value, key: &str, default: f64) -> f64 {
    data[key]
        .as_f64()
        .filter(|n| n.is_finite())
        .unwrap_or(default)
}

pub fn directions(pattern: &str, density: usize) -> Vec<(f64, usize)> {
    let n = density.max(1);
    match pattern {
        "ring" => (0..n)
            .map(|i| (i as f64 * std::f64::consts::TAU / n as f64, 0))
            .collect(),
        "arc" => (0..n)
            .map(|i| {
                let offset = if n == 1 {
                    0.0
                } else {
                    i as f64 / (n - 1) as f64 - 0.5
                };
                (offset * std::f64::consts::FRAC_PI_4, 0)
            })
            .collect(),
        "double" | "triple" => {
            let rays = n.min(if pattern == "double" { 2 } else { 3 });
            (0..rays)
                .flat_map(|ray| {
                    let angle = ((ray as f64 - (rays - 1) as f64 / 2.0) * 15.0).to_radians();
                    (0..(n / rays).max(1)).map(move |i| (angle, i))
                })
                .collect()
        }
        "ray" => (0..n).map(|i| (0.0, i)).collect(),
        _ => vec![(0.0, 0); n],
    }
}

pub fn hit_target(
    host: &mut dyn ServerHost,
    victim: mod_api::EntityRef,
    owner: Option<mod_api::EntityRef>,
    direct: Option<mod_api::EntityRef>,
    kind: &str,
    data: &Value,
) {
    let ethereal = |entity| {
        let (Some(living), Some(effect)) = (
            host.living(entity),
            host.effect_key("arcadiandream:ethereal"),
        ) else {
            return false;
        };
        living.potion_affected(host, effect)
    };
    if ethereal(victim) || owner.is_some_and(ethereal) {
        return;
    }
    let victim_kind = victim.kind(host).map(|id| id.id(host)).unwrap_or_default();
    let is_fairy = |id: &str| id.starts_with("arcadiandream:") && id.ends_with("fairy");
    if is_fairy(&victim_kind)
        && owner
            .and_then(|owner| owner.kind(host))
            .is_some_and(|id| is_fairy(&id.id(host)))
    {
        return;
    }
    let mut damage = number(data, "power", 2.0).clamp(0.1, 60.0) as f32;
    if kind == "arcadiandream:amulet_bullet"
        && host.entity_type_in_tag(&victim_kind, "minecraft:undead")
    {
        damage *= 2.0;
    }
    let damage_type = if kind == "arcadiandream:star_bullet" {
        "arcadiandream:danmaku_sharp"
    } else {
        "arcadiandream:danmaku"
    };
    if victim.hurt(host, damage_type, damage, direct, owner) == Some(true) && data["icy"] == true {
        let frozen = victim.ticks_frozen(host).unwrap_or(0);
        victim.set_ticks_frozen(host, (frozen + (10.0 * damage) as i32).min(560));
    }
}

fn shots_near(
    host: &dyn ServerHost,
    player: mod_api::PlayerRef,
    eye: [f64; 3],
    range: f64,
) -> Option<Vec<mod_api::ProjectileRef>> {
    let level = player.level(host)?;
    Some(
        level
            .entities_in(host, eye.map(|x| x - range), eye.map(|x| x + range))?
            .into_iter()
            .filter(|entity| {
                entity.kind(host).is_some_and(|kind| {
                    let kind = kind.id(host);
                    kind.starts_with("arcadiandream:") && kind.ends_with("_bullet")
                })
            })
            .filter_map(|entity| host.projectile(entity))
            .collect(),
    )
}

pub fn enlarge_aimed(
    host: &mut dyn ServerHost,
    player: mod_api::PlayerRef,
    place: ItemPlace,
    held: &mod_api::ItemStack,
) -> Option<InteractionResult> {
    let eye = player.entity().eye_position(host)?;
    let [yaw, pitch] = player
        .entity()
        .rotation(host)?
        .map(|angle| f64::from(angle).to_radians());
    let look = [
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    ];
    let nearest = shots_near(host, player, eye, 16.0)?
        .into_iter()
        .filter(|shot| shot.owner(host) == Some(player.entity()))
        .filter_map(|shot| {
            let pos = shot.entity().position(host)?;
            let delta = [0, 1, 2].map(|axis| pos[axis] - eye[axis]);
            let along: f64 = (0..3).map(|axis| delta[axis] * look[axis]).sum();
            let across: f64 = (0..3)
                .map(|axis| (delta[axis] - along * look[axis]).powi(2))
                .sum();
            (along > 0.0 && along < 16.0 && across < 1.0).then_some((shot, along))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((shot, _)) = nearest {
        let mut item = shot.carried_item(host)?;
        let mut data = crate::relics::custom(&item);
        data["scale"] = json!((number(&data, "scale", 1.0) + 0.75).min(4.0));
        items::set_custom(&mut item, &data)?;
        let creative = player.game_mode(host) == Some(GameMode::Creative);
        host.transaction(|host, tx| {
            if place.stack(host).as_ref() != Some(held) {
                return Err(());
            }
            if !creative
                && !place.edit(host, tx, |stack| {
                    stack.set_damage(stack.damage().saturating_add(1))
                })
            {
                return Err(());
            }
            shot.set_carried_item(host, item)
                .ok()
                .filter(|changed| *changed)
                .ok_or(())
        })
        .ok()?;
        player.set_item_cooldown(host, "arcadiandream:miracle_mallet", 60);
    }
    Some(InteractionResult::SuccessServer)
}

pub fn cancel_near_player(
    host: &mut dyn ServerHost,
    player: mod_api::PlayerRef,
    from_extend: bool,
) -> usize {
    let Some(eye) = player.entity().eye_position(host) else {
        return 0;
    };
    let Some(shots) = shots_near(host, player, eye, 24.0) else {
        return 0;
    };
    let targets: Vec<_> = shots
        .into_iter()
        .filter(|shot| shot.owner(host) != Some(player.entity()))
        .map(|shot| shot.entity())
        .filter(|shot| {
            shot.position(host).is_some_and(|at| {
                (0..3)
                    .map(|axis| (at[axis] - eye[axis]).powi(2))
                    .sum::<f64>()
                    < 24.0_f64.powi(2)
            })
        })
        .collect();
    let count = targets
        .into_iter()
        .filter(|shot| shot.remove(host, mod_api::RemovalReason::Discarded))
        .count();
    cancelled(host, player, count, from_extend);
    count
}

pub fn cancel_bullet(host: &mut dyn ServerHost, id: u32) -> bool {
    let Some(player) = host.players().acting_player() else {
        return false;
    };
    let Some(shot) = host.entity(id).and_then(|entity| host.projectile(entity)) else {
        return false;
    };
    if shot.owner(host) == Some(player.entity())
        || player.has_item_cooldown(host, "arcadiandream:magatama_necklace") == Some(true)
        || crate::relics::worn(host, player, "necklace").is_none()
        || !shot
            .entity()
            .remove(host, mod_api::RemovalReason::Discarded)
    {
        return false;
    }
    player.set_item_cooldown(host, "arcadiandream:magatama_necklace", 10);
    cancelled(host, player, 1, false);
    true
}

fn cancelled(
    host: &mut dyn ServerHost,
    player: mod_api::PlayerRef,
    count: usize,
    from_extend: bool,
) {
    if count != 0 {
        let trigger = Identifier::parse("arcadiandream:bullets_cancelled").unwrap();
        player.award_stat(
            host,
            mod_api::StatType::Custom,
            &trigger.to_string(),
            count.min(i32::MAX as usize) as i32,
        );
        let _ = player.trigger_criterion(
            host,
            &trigger,
            std::sync::Arc::new(CancelCriterion { count, from_extend }),
        );
    }
}

struct CancelCriterion {
    count: usize,
    from_extend: bool,
}

impl mod_api::CriterionTest for CancelCriterion {
    fn test(
        &self,
        context: &dyn mod_api::CriterionContext,
    ) -> Result<bool, mod_api::CriterionTestError> {
        let value: Value =
            mod_api::serde_json::from_str(&context.conditions_json()).unwrap_or_default();
        let amount = self.count as u64;
        Ok(amount >= value["amount"]["min"].as_u64().unwrap_or(0)
            && amount <= value["amount"]["max"].as_u64().unwrap_or(u64::MAX)
            && value["from_extend"]
                .as_bool()
                .is_none_or(|from_extend| from_extend == self.from_extend))
    }
}

pub fn projectile_item(
    host: &dyn ServerHost,
    family: &str,
    spell: &Value,
) -> Option<mod_api::ItemStack> {
    let item = host.item_key(&Identifier::parse(&format!("arcadiandream:{family}_shot")).ok()?)?;
    let mut stack = mod_api::ItemStack::new(item, 1)?;
    let mut data = spell.clone();
    data["power"] = json!(
        (number(spell, "power", 3.0)
            * number(config::config(), "danmakuDamageMultiplier", 1.0).clamp(1.0, 4.0))
        .clamp(0.1, 60.0)
    );
    items::set_custom(&mut stack, &data)?;
    Some(stack)
}

pub fn dispense(
    host: &mut dyn ServerHost,
    event: &mod_api::DispenseItemEvent<'_>,
) -> Option<(Option<mod_api::ItemStack>, i32)> {
    let id = event.stack.item().id(host);
    let family = id.strip_prefix("arcadiandream:")?.strip_suffix("_shot")?;
    let data = items::shot_settings(event.stack, family);
    let mut remainder = event.stack.clone();
    let durability = number(&data, "durability", 100.0) as u32;
    if remainder.damage() >= durability.saturating_sub(1) {
        return Some((Some(remainder), 1001));
    }
    let step = match event.facing {
        mod_api::Direction::Down => [0.0, -1.0, 0.0],
        mod_api::Direction::Up => [0.0, 1.0, 0.0],
        mod_api::Direction::North => [0.0, 0.0, -1.0],
        mod_api::Direction::South => [0.0, 0.0, 1.0],
        mod_api::Direction::West => [-1.0, 0.0, 0.0],
        mod_api::Direction::East => [1.0, 0.0, 0.0],
    };
    let at = [0, 1, 2].map(|i| f64::from(event.pos[i]) + 0.5 + step[i] * 0.7);
    let speed = number(&data, "speed", 1.0).clamp(0.01, 4.0);
    let stack = projectile_item(host, family, &data)?.to_nbt(host)?;
    let id = host.summon(
        host.dimension(),
        &format!("arcadiandream:{family}_bullet"),
        at,
        [
            ("Item".into(), mod_api::Tag::Compound(stack)),
            (
                "Motion".into(),
                mod_api::Tag::List(
                    step.map(|v| mod_api::Tag::Double((v * speed).to_bits()))
                        .into(),
                ),
            ),
            ("NoGravity".into(), mod_api::Tag::Byte(1)),
        ]
        .into(),
    );
    if id.is_none() {
        return Some((Some(remainder), 1001));
    }
    remainder.set_damage(remainder.damage() + 1);
    Some((Some(remainder), 1000))
}
