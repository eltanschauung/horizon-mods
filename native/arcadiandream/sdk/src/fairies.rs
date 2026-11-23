// SPDX-License-Identifier: EUPL-1.2-or-later

use crate::danmaku::{directions, number};
use mod_api::serde_json::{self, Value, json};
use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, EntityRef, Holder, LeashHolder, LevelRef,
    PlayerRef, ServerHost, Tag,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock, Mutex},
};

const FAIRIES: [&str; 3] = [
    "arcadiandream:fairy",
    "arcadiandream:sunflower_fairy",
    "arcadiandream:ice_fairy",
];
const FAIRY_CAP: u64 = 24;
const DESPAWN_DISTANCE: f64 = 128.0;

#[derive(Clone, Default)]
struct Fairy {
    initialized: bool,
    variant: u8,
    attack: u8,
    bullet: u8,
    color: u32,
    offset: i32,
    random: i32,
    anger: u32,
    cooldown: i32,
}

static ATTACKS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../assets/arcadiandream/embedded/fairy_attacks.json"
    ))
    .expect("embedded fairy attacks")
});

static FAIRY: AttachmentType<Fairy> = AttachmentType {
    namespace: "arcadiandream",
    name: "fairy",
    default: Fairy::default,
    copy_on_death: false,
    sync: false,
    codec: Some(AttachmentCodec {
        encode: |data| {
            let json = json!({
                "initialized": data.initialized,
                "variant": data.variant,
                "attack": data.attack,
                "bullet": data.bullet,
                "color": data.color,
                "offset": data.offset,
                "random": data.random,
                "anger": data.anger,
                "cooldown": data.cooldown,
            })
            .to_string();
            Ok([("json".into(), Tag::String(json.as_str().into()))].into())
        },
        decode: |value| {
            let Some(Tag::String(json)) = value.get(&"json".into()) else {
                return Err(AttachmentError::Decode);
            };
            let data: Value = serde_json::from_str(&json.to_string_lossy())
                .map_err(|_| AttachmentError::Decode)?;
            let n = |key: &str| data[key].as_i64().unwrap_or_default();
            Ok(Fairy {
                initialized: data["initialized"] == true,
                variant: n("variant").clamp(0, 3) as u8,
                attack: n("attack").clamp(0, 7) as u8,
                bullet: n("bullet").clamp(0, 2) as u8,
                color: n("color").clamp(0, 0xff_ffff) as u32,
                offset: n("offset").clamp(-5, 5) as i32,
                random: n("random").clamp(1, 11) as i32,
                anger: n("anger").clamp(0, 800) as u32,
                cooldown: n("cooldown").clamp(0, 600) as i32,
            })
        },
    }),
};

const VARIANTS: &str = "arcadiandream:fairy_variants";
static CLIENT_VARIANTS: LazyLock<Mutex<HashMap<u32, u8>>> = LazyLock::new(Mutex::default);
static HURT: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(Mutex::default);

pub fn register(registrar: &mut dyn mod_api::Registrar) {
    registrar.payload_handlers().registrar("1").play_to_client(
        mod_api::PayloadId(VARIANTS.into()),
        Some(Arc::new(VariantPacket)),
    );
}

struct VariantPacket;

impl mod_api::IPayloadHandler for VariantPacket {
    fn handle(
        &self,
        bytes: mod_api::PayloadBytes,
        context: &mut dyn mod_api::IPayloadContext,
    ) -> Result<(), mod_api::PayloadHandleError> {
        if bytes.0.len() <= 5120
            && bytes.0.len().is_multiple_of(5)
            && context.client_host().is_ok()
            && let Ok(mut variants) = CLIENT_VARIANTS.lock()
        {
            if variants.len() > 4096 {
                variants.clear()
            }
            for record in bytes.0.chunks_exact(5) {
                let id = u32::from_le_bytes(record[..4].try_into().unwrap());
                variants.insert(id, record[4].min(3));
            }
        }
        Ok(())
    }
}

pub fn is_fairy(kind: &str) -> bool {
    FAIRIES.contains(&kind)
}

pub fn variant(id: u32) -> usize {
    CLIENT_VARIANTS
        .lock()
        .ok()
        .and_then(|variants| variants.get(&id).copied())
        .unwrap_or(0) as usize
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn nearby_players(host: &dyn ServerHost, level: LevelRef) -> Vec<PlayerRef> {
    level
        .online_players(host)
        .unwrap_or_default()
        .into_iter()
        .filter(|player| {
            !matches!(
                player.game_mode(host),
                Some(mod_api::GameMode::Creative | mod_api::GameMode::Spectator)
            )
        })
        .collect()
}

pub fn aggro(id: u32) {
    if let Ok(mut hurt) = HURT.lock()
        && hurt.len() < 4096
    {
        hurt.insert(id);
    }
}

pub fn sync_variants(host: &mut dyn ServerHost, level: LevelRef) {
    if level.game_time(host).unwrap_or_default().rem_euclid(20) != 0 {
        return;
    }
    for player in level.online_players(host).unwrap_or_default() {
        let Some(at) = player.entity().position(host) else {
            continue;
        };
        let entities = level
            .entities_in(host, at.map(|p| p - 128.0), at.map(|p| p + 128.0))
            .unwrap_or_default();
        let mut bytes = Vec::new();
        for entity in entities {
            if bytes.len() >= 5120 {
                break;
            }
            if entity
                .kind(host)
                .is_some_and(|kind| is_fairy(&kind.id(host)))
                && let Ok(Some(data)) = FAIRY.get_existing(host, Holder::Entity(entity.id()))
            {
                bytes.extend(entity.id().to_le_bytes());
                bytes.push(data.variant);
            }
        }
        if !bytes.is_empty() {
            let _ = player.send_payload(
                host,
                &mod_api::PayloadId(VARIANTS.into()),
                mod_api::PayloadBytes(bytes),
            );
        }
    }
}

pub fn tick(host: &mut dyn ServerHost, entity: EntityRef, kind: &str) -> Option<()> {
    let mob = host.mob(entity)?;
    let living = host.living(entity)?;
    if living.health(host)? <= 0.0 {
        return None;
    }
    let level = entity.level(host)?;
    let pos = entity.position(host)?;
    let holder = Holder::Entity(entity.id());
    let mut data = FAIRY.get(host, holder).unwrap_or_default();
    if !data.initialized {
        // The original chooses these independently once per mob; persist the
        // choice so a reload does not change a fairy's texture or attack.
        let mut seed = (level.game_time(host).unwrap_or(0) as u64)
            .wrapping_add(u64::from(entity.id()))
            .wrapping_mul(0x9e3779b97f4a7c15);
        let mut pick = |size| {
            seed ^= seed >> 12;
            seed ^= seed << 25;
            seed ^= seed >> 27;
            seed.wrapping_mul(0x2545f4914f6cdd1d) % size
        };
        let colors = [
            0xffffff, 0x808080, 0x404040, 0x0a0a0a, 0x964b00, 0xff0000, 0xffa500, 0xffe400,
            0xbfff00, 0x00ff00, 0x00ffff, 0x3ab3da, 0x1d1dff, 0xa023f0, 0xff00ff, 0xf38baa,
        ];
        data.initialized = true;
        data.variant = pick(4) as u8;
        data.attack = pick(if kind == FAIRIES[0] { 6 } else { 8 }) as u8;
        data.bullet = pick(if kind == FAIRIES[0] { 2 } else { 3 }) as u8;
        data.offset = pick(11) as i32 - 5;
        data.random = pick(11) as i32 + 1;
        data.color = colors[pick(16) as usize];
    }
    if HURT.lock().is_ok_and(|mut hurt| hurt.remove(&entity.id())) {
        data.anger = 400 + entity.id() % 400;
    }
    if mob.no_ai(host) == Some(false) {
        mob.set_no_ai(host, true);
        entity.set_no_gravity(host, true);
    }

    data.anger = data.anger.saturating_sub(1);
    data.cooldown = data.cooldown.saturating_sub(1);
    let mut target = None;
    if data.anger > 0 || kind == FAIRIES[1] {
        for player in nearby_players(host, level) {
            let Some(at) = player.entity().position(host) else {
                continue;
            };
            let dist = distance(pos, at);
            if dist >= 24.0 || target.is_some_and(|(_, best)| dist >= best) {
                continue;
            }
            if data.anger == 0 && charm_mode(host, player).is_some_and(|mode| mode != 1) {
                continue;
            }
            target = Some((at, dist));
        }
    }
    if let Some((at, dist)) = target {
        let d = dist.max(0.01);
        let facing = (pos[0] - at[0]).atan2(at[2] - pos[2]).to_degrees() as f32;
        entity.set_rotation(host, facing, 0.0);
        let factor = if d > 10.0 {
            0.08
        } else if d < 5.0 {
            -0.04
        } else {
            0.0
        };
        entity.set_velocity(
            host,
            [
                (at[0] - pos[0]) / d * factor,
                (at[1] + 1.0 - pos[1]) / d * 0.06,
                (at[2] - pos[2]) / d * factor,
            ],
        );
        if data.cooldown <= 0 {
            let mut spell = attack(kind, &data);
            spell["icy"] = json!(kind == FAIRIES[2]);
            let family = if kind == FAIRIES[0] {
                ["circle", "pellet"][usize::from(data.bullet) % 2]
            } else {
                ["circle", "kunai", "pellet"][usize::from(data.bullet) % 3]
            };
            let time = level.day_time(host).unwrap_or_default() as f64;
            fire(host, living, family, pos, at, &spell, time);
            data.cooldown = number(&spell, "cooldown", 30.0).max(1.0) as i32;
        }
    } else {
        let tick = (f64::from(entity.tick_count(host).unwrap_or_default())
            + f64::from(entity.id()))
            * 0.02;
        entity.set_velocity(
            host,
            [tick.cos() * 0.03, tick.sin() * 0.01, tick.sin() * 0.03],
        );
    }
    let _ = FAIRY.set(host, holder, data);
    Some(())
}

fn fire(
    host: &mut dyn ServerHost,
    fairy: mod_api::LivingRef,
    family: &str,
    from: [f64; 3],
    to: [f64; 3],
    spell: &Value,
    time: f64,
) -> Option<()> {
    let stack = crate::danmaku::projectile_item(host, family, spell)?;
    let kind = mod_api::Identifier::parse(&format!("arcadiandream:{family}_bullet")).ok()?;
    let d = distance(from, to).max(0.01);
    let dir = [
        (to[0] - from[0]) / d,
        (to[1] - from[1] - 0.2) / d,
        (to[2] - from[2]) / d,
    ];
    let density = number(spell, "density", 1.0).clamp(1.0, 40.0) as usize;
    let pattern = spell["pattern"].as_str().unwrap_or("spread");
    let speed = number(spell, "speed", 0.5).clamp(0.0, 4.0);
    for (n, (angle, layer)) in directions(pattern, density).into_iter().enumerate() {
        let spread = number(spell, "divergence", 0.0)
            + if matches!(pattern, "spread" | "rain") {
                (density - 1) as f64
            } else {
                0.0
            };
        let a = angle + ((time + n as f64 * 13.1).sin() * spread).to_radians();
        let pace = speed
            * if matches!(pattern, "ray" | "double" | "triple") {
                (1.0 - layer as f64 / density as f64).max(0.1)
            } else {
                1.0
            };
        let velocity = if pattern == "rain" {
            [a.sin() * 0.15, speed, a.cos() * 0.15]
        } else {
            [
                (dir[0] * a.cos() - dir[2] * a.sin()) * pace,
                dir[1] * pace,
                (dir[0] * a.sin() + dir[2] * a.cos()) * pace,
            ]
        };
        if let Some(shot) = host.spawn_projectile_from_rotation(fairy, &kind, &stack, 0.0, 1.0, 0.0)
        {
            shot.entity().set_velocity(host, velocity);
            shot.entity().set_no_gravity(host, true);
        }
    }
    Some(())
}

fn charm_mode(host: &mut dyn ServerHost, player: PlayerRef) -> Option<u64> {
    crate::relics::worn(host, player, "charm").map(|stack| {
        crate::relics::custom(&stack)["mode"]
            .as_u64()
            .unwrap_or(0)
            .min(2)
    })
}

fn uses_spawn_budget(kind: &str) -> bool {
    kind == FAIRIES[0] || kind == FAIRIES[2]
}

pub fn despawn_level(
    host: &mut dyn ServerHost,
    level: LevelRef,
    departing: Option<u128>,
) -> Option<u64> {
    // Match the vanilla hard distance check, using players in this dimension.
    // Creative players still keep mobs nearby. A missing lookup defers cleanup.
    let players = level
        .online_players(host)?
        .into_iter()
        .filter(|player| Some(player.uuid()) != departing)
        .filter(|player| player.game_mode(host) != Some(mod_api::GameMode::Spectator))
        .map(|player| player.entity().position(host))
        .collect::<Option<Vec<_>>>()?;

    let mut population = 0;
    let entities = level
        .all_entities(host)
        .or_else(|| level.entities_in(host, [f64::MIN; 3], [f64::MAX; 3]))?;
    for entity in entities {
        let Some(kind) = entity.kind(host).map(|kind| kind.id(host)) else {
            continue;
        };
        if !is_fairy(&kind)
            || !host
                .living(entity)
                .and_then(|living| living.health(host))
                .is_some_and(|hp| hp > 0.0)
        {
            continue;
        }
        let removed = entity.position(host).is_some_and(|at| {
            players.iter().all(|pos| distance(at, *pos) > DESPAWN_DISTANCE)
        }) && entity.leash_holder(host) == Some(LeashHolder::Unleashed)
            && entity.vehicle(host).is_none()
            && entity.passengers(host).is_some_and(|passengers| passengers.is_empty())
            && entity.nbt(host).is_some_and(|data| {
                !data.contains_key(&"CustomName".into())
                    && !matches!(data.get(&"PersistenceRequired".into()), Some(Tag::Byte(n)) if *n != 0)
            })
            && entity.discard(host);
        if !removed && uses_spawn_budget(&kind) {
            population += 1;
        }
    }
    Some(population)
}

pub fn spawn(host: &mut dyn ServerHost, level: LevelRef) {
    let Some(ticks) = level
        .game_time(host)
        .filter(|ticks| ticks.rem_euclid(100) == 0)
    else {
        return;
    };
    // Despawning also runs at night and with natural spawning disabled.
    let mut remaining =
        despawn_level(host, level, None).map_or(0, |count| FAIRY_CAP.saturating_sub(count));
    let Some(time) = level.day_time(host) else {
        return;
    };
    if time.rem_euclid(24000) > 12000
        || level.difficulty(host) == Some("peaceful")
        || matches!(
            level.game_rule(host, "minecraft:spawn_mobs"),
            Ok(Some(mod_api::GameRuleValue::Bool(false)))
        )
    {
        return;
    }
    let players: Vec<_> = nearby_players(host, level)
        .into_iter()
        .filter_map(|player| {
            let position = player.entity().position(host)?;
            Some((player, position, charm_mode(host, player)))
        })
        .collect();
    let dimension = level.dimension_id();
    let settings = crate::config::config();
    for (player, pos, _) in &players {
        let angle =
            (time as f64 * 1.618 + player.uuid() as u64 as f64).sin() * std::f64::consts::TAU;
        let x = pos[0] as i32 + (angle.cos() * 32.0) as i32;
        let z = pos[2] as i32 + (angle.sin() * 32.0) as i32;
        let Some(y) = host
            .surface_heights(
                dimension,
                &[(x, z)],
                mod_api::Heightmap::MotionBlockingNoLeaves,
            )
            .ok()
            .and_then(|heights| heights.first().copied().flatten())
        else {
            continue;
        };
        let at = [f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5];
        let charm_dist = number(&settings["fairyCharmOptions"], "distance", 128.0);
        if settings["fairyCharmOptions"]["canPreventSpawning"]
            .as_bool()
            .unwrap_or(true)
            && players.iter().any(|(_, pos, charm)| {
                charm.is_some_and(|c| c >= 1) && distance(at, *pos) < charm_dist
            })
        {
            continue;
        }
        if level
            .brightness(host, mod_api::LightLayer::Sky, [x, y, z])
            .unwrap_or(0)
            .max(
                level
                    .brightness(host, mod_api::LightLayer::Block, [x, y, z])
                    .unwrap_or(0),
            )
            <= 8
        {
            continue;
        }
        let Some(ground) = level.block(host, [x, y - 1, z]).map(|key| key.id(host)) else {
            continue;
        };
        if !["grass_block", "podzol", "snow_block", "snow", "ice", "mud"]
            .iter()
            .any(|suffix| ground.ends_with(suffix))
        {
            continue;
        }
        let Some(biome) = level.biome_at(host, [x, y, z]) else {
            continue;
        };
        let in_tag = |tag| level.biome_in_tag(host, &biome, tag) == Some(true);
        let snow = in_tag("minecraft:spawns_snow_foxes");
        let temperate = in_tag("c:climate_temperate")
            || in_tag("c:is_temperate/overworld")
            || level.biome_properties(host, &biome).is_some_and(|props| {
                (0.25..=1.0).contains(&props.temperature)
                    && props.downfall >= 0.3
                    && !["ocean", "cave", "river"]
                        .iter()
                        .any(|word| biome.contains(word))
            });
        let sunflower = biome == "minecraft:sunflower_plains";
        let roll = (ticks as u64 ^ (player.uuid() as u64)).wrapping_mul(0x9e3779b97f4a7c15) >> 32;
        let (name, enabled, number) = if snow {
            (FAIRIES[2], "spawnIceFairies", 1 + roll % 2)
        } else if temperate || sunflower {
            if roll % if sunflower { 200 } else { 100 } < 80 {
                (FAIRIES[0], "spawnFairies", 1 + roll % 3)
            } else {
                (FAIRIES[1], "spawnSunflowerFairies", 1)
            }
        } else {
            continue;
        };
        if !settings["fairyOptions"][enabled].as_bool().unwrap_or(true) {
            continue;
        }
        let capped = uses_spawn_budget(name);
        let number = if capped {
            number.min(remaining)
        } else {
            // Sunflower fairies keep their existing per-site spawn check.
            let nearby = level
                .entities_in(host, at.map(|c| c - 64.0), at.map(|c| c + 64.0))
                .unwrap_or_default();
            if nearby
                .iter()
                .filter(|e| e.kind(host).is_some_and(|kind| is_fairy(&kind.id(host))))
                .take(25)
                .count()
                > 24
            {
                continue;
            }
            number
        };
        for member in 0..number {
            let spawned = host.summon(
                dimension,
                name,
                [at[0] + member as f64 * 0.7, at[1], at[2]],
                [
                    ("NoAI".into(), Tag::Byte(1)),
                    ("NoGravity".into(), Tag::Byte(1)),
                ]
                .into(),
            );
            if capped && spawned.is_some() {
                remaining -= 1;
            }
        }
    }
}
fn attack(kind: &str, data: &Fairy) -> Value {
    let name = kind.trim_start_matches("arcadiandream:");
    let row = &ATTACKS[name][data.attack as usize];
    let mut out = serde_json::json!({"pattern":row["pattern"],"cooldown":row["cooldown"].as_i64().unwrap_or(30)+data.offset as i64,"color":if name=="ice_fairy"{0x00ffff}else{data.color}});
    if let Some(args) = row["args"].as_array() {
        for (key, v) in [
            "density",
            "speed",
            "divergence",
            "power",
            "duration",
            "color",
            "gravity",
        ]
        .into_iter()
        .zip(args)
        {
            if key == "color" {
                continue;
            }
            let s = v.as_str().unwrap_or("0");
            let n = if s.starts_with("getRandomNumberProvider()") {
                data.random as f64
                    + s.split('+')
                        .nth(1)
                        .and_then(|s| s.trim().parse::<f64>().ok())
                        .unwrap_or(0.0)
            } else {
                s.parse().unwrap_or(0.0)
            };
            out[key] = serde_json::json!(n);
        }
    }
    out
}
