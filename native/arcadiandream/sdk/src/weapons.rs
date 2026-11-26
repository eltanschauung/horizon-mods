// SPDX-License-Identifier: EUPL-1.2-or-later

use crate::{config, danmaku::number, relics};
use mod_api::serde_json::json;
use mod_api::{
    ClipBlock, ClipFluid, GameMode, Hand, HostSide, HostTransaction, InteractionResult, ItemPlace,
    PlayerRef, ServerHost, SoundSource, UseOnEvent,
};

static SMOKE: std::sync::Mutex<Option<mod_api::ParticleKey>> = std::sync::Mutex::new(None);

pub fn register(registrar: &mut dyn mod_api::Registrar) {
    if let Ok(mut smoke) = SMOKE.lock() {
        *smoke = Some(registrar.particle_key("minecraft:white_smoke"));
    }
    for item in [
        "fairy_spawn_egg",
        "sunflower_fairy_spawn_egg",
        "ice_fairy_spawn_egg",
        "wall_passing_chisel",
        "folding_chair",
        "icicle_sword",
        "nue_trident",
        "mochi_mallet",
    ] {
        let key = registrar.item_key(&format!("arcadiandream:{item}"));
        registrar.item_part(key, Weapon);
    }
}

struct Weapon;

impl mod_api::ItemPart for Weapon {
    fn item_subscribed(&self) -> &'static [Option<mod_api::HookId>] {
        &[
            Some(mod_api::HookId::UseOn),
            Some(mod_api::HookId::HurtEnemy),
        ]
    }

    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        use_on(host, event)
    }

    fn hurt_enemy(&self, host: &mut dyn ServerHost, event: &mut mod_api::HurtEnemyEvent<'_>) {
        swing(host, event);
    }
}

pub fn use_on(host: &mut dyn ServerHost, event: &UseOnEvent<'_>) -> Option<InteractionResult> {
    let id = event.stack.item().id(host);
    let player = host.player(event.player)?;
    let level = player.level(host)?;
    let held = ItemPlace::PlayerHand {
        player: event.player,
        hand: event.hand,
    };
    if let Some(kind) = id
        .strip_suffix("_spawn_egg")
        .filter(|kind| crate::fairies::is_fairy(kind))
    {
        let pos = [0, 1, 2].map(|axis| {
            f64::from(event.pos[axis] + event.face[axis]) + if axis == 1 { 0.0 } else { 0.5 }
        });
        host.summon(
            level.dimension_id(),
            kind,
            pos,
            [
                ("NoAI".into(), mod_api::Tag::Byte(1)),
                ("NoGravity".into(), mod_api::Tag::Byte(1)),
            ]
            .into(),
        )?;
        if player.game_mode(host) != Some(GameMode::Creative) {
            let _ = host.transaction(|host, tx| {
                let resource = mod_api::ItemResource::of(event.stack);
                (held.extract(host, tx, &resource, 1) == 1)
                    .then_some(())
                    .ok_or(())
            });
        }
        return Some(InteractionResult::SuccessServer);
    }
    match id.as_str() {
        "arcadiandream:ibuki_gourd"
            if level
                .block(host, event.pos)
                .is_some_and(|b| b.id(host) == "minecraft:water") =>
        {
            relics::edit_custom(host, &held, |data| data["filled"] = json!(true))?;
        }
        "arcadiandream:wall_passing_chisel" => {
            let chisel = &config::config()["chiselOptions"];
            if !chisel["canUse"].as_bool().unwrap_or(true) {
                return Some(InteractionResult::Consume);
            }
            let distance = number(chisel, "maxDistance", 16.0).clamp(1.0, 64.0) as i32;
            for n in 1..=distance {
                let pos = [0, 1, 2].map(|axis| event.pos[axis] - event.face[axis] * n);
                let Some(block) = level.block(host, pos).map(|key| key.id(host)) else {
                    break;
                };
                if matches!(block.as_str(), "minecraft:obsidian" | "minecraft:bedrock") {
                    break;
                }
                let top = [pos[0], pos[1] + 1, pos[2]];
                if matches!(block.as_str(), "minecraft:air" | "minecraft:cave_air")
                    && level
                        .block(host, top)
                        .is_some_and(|key| key.id(host) == "minecraft:air")
                {
                    let to = [
                        f64::from(pos[0]) + 0.5,
                        f64::from(pos[1]),
                        f64::from(pos[2]) + 0.5,
                    ];
                    if player.entity().teleport_to(host, to, None) {
                        wear(host, player, held);
                        level.play_sound(
                            host,
                            "minecraft:entity.item_frame.remove_item",
                            to,
                            SoundSource::Players,
                            1.0,
                            1.0,
                        );
                        if let Some(particle) = SMOKE.lock().ok().and_then(|smoke| *smoke) {
                            level.send_particles(
                                host,
                                &mod_api::ParticleOptions::simple(particle),
                                None,
                                [to[0], to[1] + 1.0, to[2]],
                                [0.3, 0.5, 0.3],
                                0.03,
                                20,
                            );
                        }
                    }
                    break;
                }
            }
        }
        _ => return None,
    }
    Some(InteractionResult::SuccessServer)
}

pub fn swing(host: &mut dyn ServerHost, event: &mut mod_api::HurtEnemyEvent<'_>) {
    let id = event.stack.item().id(host);
    let Some(target) = host.entity(event.entity) else {
        return;
    };
    match id.as_str() {
        "arcadiandream:folding_chair" => {
            let Some(player) = host.players().acting_player() else {
                return;
            };
            let (Some(from), Some(to)) = (player.entity().position(host), target.position(host))
            else {
                return;
            };
            let (dx, dz) = (to[0] - from[0], to[2] - from[2]);
            let distance = dx.hypot(dz).max(0.01);
            let strength = number(
                &config::config()["foldingChairOptions"],
                "knockbackStrength",
                1.5,
            )
            .clamp(0.0, 10.0);
            target.set_velocity(
                host,
                [dx / distance * strength, 0.4, dz / distance * strength],
            );
        }
        "arcadiandream:icicle_sword" => {
            let frozen = target.ticks_frozen(host).unwrap_or(0);
            target.set_ticks_frozen(host, (frozen + 40).min(560));
        }
        "arcadiandream:nue_trident" => {
            if let (Some(target), Some(effect)) =
                (host.living(target), host.effect_key("minecraft:blindness"))
            {
                target.add_effect(
                    host,
                    &mod_api::ActiveEffect::new(effect, 100, 0, false, true, true),
                );
            }
        }
        "arcadiandream:mochi_mallet" => {
            if host
                .living(target)
                .and_then(|living| living.health(host))
                .is_some_and(|health| health <= 0.0)
                && let Some(player) = host.players().acting_player()
            {
                let place = ItemPlace::PlayerHand {
                    player: player.uuid(),
                    hand: Hand::Main,
                };
                let _ = relics::edit_custom(host, &place, |data| {
                    data["kill_count"] =
                        json!(data["kill_count"].as_u64().unwrap_or(0).saturating_add(1));
                });
                update_mallet(host, place);
            }
        }
        _ => {}
    }
}

fn update_mallet(host: &mut dyn ServerHost, place: ItemPlace) {
    let Some(item) = place.stack(host) else {
        return;
    };
    let kills = if config::config()["mochiMalletOptions"]["lowViolence"] == true {
        0
    } else {
        relics::custom(&item)["kill_count"].as_u64().unwrap_or(0)
    };
    let stage = [0, 10, 25, 50, 75, 100, 250, 500, 750, 900, 1000]
        .iter()
        .rposition(|&n| kills >= n)
        .unwrap_or(0);
    let model = format!("arcadiandream:mochi_mallet_{stage}");
    let mut patch = mod_api::Compound::from([(
        "minecraft:item_model".into(),
        mod_api::Tag::String(model.as_str().into()),
    )]);
    if kills > 0 {
        let bonus = kills.min(500) as f64;
        let modifier = |id: &str, attribute: &str, amount: f64| {
            mod_api::Tag::compound([
                ("id", mod_api::Tag::String(id.into())),
                ("type", mod_api::Tag::String(attribute.into())),
                ("amount", mod_api::Tag::Double(amount.to_bits())),
                ("operation", mod_api::Tag::String("add_value".into())),
                ("slot", mod_api::Tag::String("mainhand".into())),
            ])
        };
        patch.insert(
            "minecraft:attribute_modifiers".into(),
            mod_api::Tag::List(vec![
                modifier(
                    "arcadiandream:mochi_damage",
                    "minecraft:attack_damage",
                    3.0 + bonus / 125.0,
                ),
                modifier(
                    "arcadiandream:mochi_speed",
                    "minecraft:attack_speed",
                    -3.2 + bonus / 416.666,
                ),
            ]),
        );
    }
    if item.components().is_some_and(|old| {
        patch
            .iter()
            .all(|(key, value)| old.as_nbt().get(key) == Some(value))
    }) {
        return;
    }
    let _ = host.transaction(|host, tx| {
        let mut applied = false;
        let changed = place.edit(host, tx, |stack| {
            applied = stack.apply_components(&patch).is_ok()
        });
        (applied && changed).then_some(()).ok_or(())
    });
}

pub fn carried(host: &mut dyn ServerHost, player: PlayerRef) {
    let Some(level) = player.level(host) else {
        return;
    };
    let Some(pos) = player.entity().position(host) else {
        return;
    };
    let pos = pos.map(|v| v.floor() as i32);
    let tick = level.game_time(host).unwrap_or_default();
    let nether = level.dimension_id() == "minecraft:the_nether";
    for hand in [Hand::Main, Hand::Off] {
        let place = ItemPlace::PlayerHand {
            player: player.uuid(),
            hand,
        };
        let Some(stack) = place.stack(host) else {
            continue;
        };
        match stack.item().id(host).as_str() {
            "arcadiandream:mochi_mallet" if tick.rem_euclid(20) == 0 => update_mallet(host, place),
            "arcadiandream:icicle_sword" => {
                let temp = level
                    .biome_at(host, pos)
                    .and_then(|biome| level.biome_properties(host, &biome))
                    .map(|b| b.temperature)
                    .unwrap_or_default();
                let daylight = level.day_time(host).unwrap_or_default().rem_euclid(24000) < 12000
                    && level
                        .brightness(host, mod_api::LightLayer::Sky, pos)
                        .unwrap_or_default()
                        >= 14;
                let roll =
                    (tick as u64 ^ player.uuid() as u64).wrapping_mul(0x9e3779b97f4a7c15) >> 32;
                if nether || (temp > 0.5 && daylight && roll % 25 == 0) {
                    let instant = nether
                        && config::config()["icicleSwordOptions"]["meltsInstantly"]
                            .as_bool()
                            .unwrap_or(true);
                    if instant {
                        let _ = host.transaction(|host, tx| {
                            (place.extract(host, tx, &mod_api::ItemResource::of(&stack), 1) == 1)
                                .then_some(())
                                .ok_or(())
                        });
                    } else {
                        wear_by(
                            host,
                            player,
                            place,
                            ((roll % 3) as f32 * temp).max(0.0) as u32,
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn scythe_release(
    host: &mut dyn ServerHost,
    event: &mut mod_api::ReleaseUsingEvent<'_>,
) -> Option<()> {
    if event.stack.item().id(host) != "arcadiandream:death_scythe" || 72_000 - event.time_left <= 20
    {
        return None;
    }
    let player = host.players().acting_player()?;
    let entity = player.entity();
    let level = player.level(host)?;
    let eye = entity.eye_position(host)?;
    let [yaw, pitch] = entity
        .rotation(host)?
        .map(|angle| f64::from(angle).to_radians());
    let look = [
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    ];
    let end = [0, 1, 2].map(|i| eye[i] + 16.0 * look[i]);
    let end = level
        .clip(host, eye, end, ClipBlock::Collider, ClipFluid::None)
        .filter(|hit| !hit.miss)
        .map_or(end, |hit| hit.location);
    let target = level
        .ray_trace_entities(host, eye, end, 0.0, Some(entity))?
        .into_iter()
        .find(|hit| {
            host.living(hit.entity)
                .and_then(|living| living.health(host))
                .is_some_and(|hp| hp > 0.0)
        })?
        .entity;
    if let Some(other) = level
        .online_players(host)?
        .into_iter()
        .find(|other| other.entity() == target)
        && (other.game_mode(host) == Some(GameMode::Creative)
            || matches!(
                level.game_rule(host, "minecraft:pvp"),
                Ok(Some(mod_api::GameRuleValue::Bool(false)))
            ))
    {
        return None;
    }
    let from = entity.position(host)?;
    let to = target.position(host)?;
    if !target.teleport_to(host, from, None) {
        return None;
    }
    if !entity.teleport_to(host, to, None) {
        target.teleport_to(host, to, None);
        return None;
    }
    let main = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand: Hand::Main,
    };
    let hand = if main.stack(host).as_ref() == Some(event.stack) {
        Hand::Main
    } else {
        Hand::Off
    };
    wear(
        host,
        player,
        ItemPlace::PlayerHand {
            player: player.uuid(),
            hand,
        },
    );
    player.set_item_cooldown(host, "arcadiandream:death_scythe", 100);
    player.award_stat(
        host,
        mod_api::StatType::Used,
        "arcadiandream:death_scythe",
        1,
    );
    level.play_sound(
        host,
        "arcadiandream:item.death_scythe.teleport",
        to,
        SoundSource::Players,
        1.0,
        1.1,
    );
    event.used = true;
    Some(())
}

fn wear(host: &mut dyn ServerHost, player: PlayerRef, place: ItemPlace) {
    wear_by(host, player, place, 1);
}

fn wear_by(host: &mut dyn ServerHost, player: PlayerRef, place: ItemPlace, amount: u32) {
    if amount == 0 || player.game_mode(host) == Some(GameMode::Creative) {
        return;
    }
    let broke = host.transaction(|host, tx| {
        let stack = place.stack(host).ok_or(())?;
        let damage = stack.damage().saturating_add(amount);
        let broke = damage >= host.max_damage(stack.item()).unwrap_or(u32::MAX);
        let applied = if broke {
            place.extract(host, tx, &mod_api::ItemResource::of(&stack), 1) == 1
        } else {
            place.edit(host, tx, |stack| stack.set_damage(damage))
        };
        applied.then_some(broke).ok_or(())
    });
    if matches!(broke, Ok(true))
        && let (Some(level), Some(at)) = (player.level(host), player.entity().position(host))
    {
        level.play_sound(host, "minecraft:entity.item.break", at, SoundSource::Players, 0.8, 1.0);
    }
}
