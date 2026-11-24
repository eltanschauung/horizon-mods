//! Touhou Mobs for Horizon: the fairies, youkai, bosses and NPCs of Touhou Items Mod (kuro-anco),
//! SPELLCARD PLUS and Journey To Gensokyo (Katrix), in one mod. A port for personal use; the
//! original art is in the content mod `thmobs`.
//!
//! Each mob is a living type of the content mod copying the wandering trader, whose behaviour is all
//! goals: this mod clears them and gives it the danmaku mobs' own (`ai`), which runs its attack
//! (`attack`) from its row in `mobs`. A boss that runs out of health goes on to its next phase
//! (`living_death`). Each is drawn with its Java model, converted part for part (`model`); its danmaku
//! fly in the same engine as the spell cards' (`shot`, `pattern`, `cards`): the server moves them and
//! settles hits, clients are told each shot once.
#![allow(dead_code)] // The danmaku engine is shared with the spell cards port; not every part is used yet.

mod ai;
mod attack;
mod cards;
mod classic;
mod config;
mod mobs;
mod model;
mod models_gen;
mod pattern;
mod shot;
mod spatial;
mod vec;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

use mod_api::{
    ActiveEffect, BossBarColor, ClientHost, ClientLevelUnloadEvent, ClientPacketEvent,
    ClientTickPostEvent, Compound, EntityInteractEvent, Flow, Heightmap, Hooks, Host, HostSide,
    HostTransaction, InteractionResult, ItemPart, ItemPlace, ItemResource, LevelRef,
    LevelTickPostEvent, LightLayer, LivingDeathEvent, LivingIncomingDamageEvent, MobRender,
    ModelPart, ParticleOptions, PlayerRef, PlayerTeleportedEvent, RegisterLayerDefinitionsEvent,
    Registrar, RegistrationContext, RenderLivingPreEvent, ServerHost, SoundSource,
    SubmitCustomGeometryEvent, Tag, Text, UseOnEvent, WorldDrawContext,
};

use cards::{Actor, Card, Ctx, card_position};
use classic::Fx;
use mobs::Look;
use pattern::Fire;
use shot::{Batch, Shot, form, shot_full, special};
use vec::{Rng, V3, angle, angle_to, cross, rotate, v3};

#[derive(Default)]
struct Mod;

static EXPLOSION_PARTICLES: std::sync::OnceLock<[mod_api::ParticleKey; 2]> =
    std::sync::OnceLock::new();

const CHANNEL: &str = "thmobs_native:shots";
const DIMENSIONS: [&str; 3] = [
    "minecraft:overworld",
    "minecraft:the_nether",
    "minecraft:the_end",
];

// ---- The server's side ----

struct Live {
    shot: Shot,
    owner: u32,
}

/// A spell card a mob declared.
struct Cast {
    card: Card,
    mob: u32,
    time: i32,
    /// Toward its next tick, at the mobs' pace.
    pace: f32,
    card_pos: V3,
    card_yaw: f32,
    card_pitch: f32,
    state: [f32; 2],
    target: V3,
    /// Cast by a classic boss: at the original's full rate.
    full: bool,
}

/// Killing Doll's stopped time about a mob, and the ticks it has left.
struct Stop {
    mob: u32,
    left: i32,
}

struct Server {
    config: config::Loaded,
    shots: HashMap<String, Vec<Live>>,
    casts: HashMap<String, Vec<Cast>>,
    stops: HashMap<String, Vec<Stop>>,
    /// The classic bosses about, this tick.
    classic: HashSet<u32>,
    /// The Great Dolls about, this tick: their danmaku does half its damage.
    halved: HashSet<u32>,
    /// The boss bars shown: by player and boss, the dimension, the fill (in thousandths) and the name.
    bars: HashMap<(u128, u32), (String, i32, String)>,
    next_id: u32,
    rng: Rng,
    /// Spawner ticks before the next named character (a boss, or one of the people) may spawn.
    named_wait: u32,
}

#[derive(Default)]
struct Population {
    mobs: BTreeMap<u32, &'static mobs::Mob>,
    hostiles: BTreeMap<u32, bool>,
}

// Separate from SERVER: summoning during a spawn attempt can run lifecycle hooks.
static POPULATIONS: LazyLock<Mutex<HashMap<String, Population>>> =
    LazyLock::new(Mutex::default);

fn tracked_mobs(dimension: &str) -> Vec<(u32, &'static mobs::Mob)> {
    POPULATIONS.lock().ok()
        .and_then(|levels| Some(levels.get(dimension)?.mobs.iter().map(|(&id, &mob)| (id, mob)).collect()))
        .unwrap_or_default()
}

fn tracked_hostiles(dimension: &str, active: bool) -> Vec<(u32, bool)> {
    POPULATIONS.lock().ok()
        .and_then(|levels| Some(levels.get(dimension)?.hostiles.iter().filter(|&(_, &creeper)| active || creeper).map(|(&id, &creeper)| (id, creeper)).collect()))
        .unwrap_or_default()
}

/// Apply the hard 128-block distance rule to wild mobs, including neutral
/// fairies. Named characters, summons and persistent mobs retain their state.
pub fn despawn_wild_mobs(host: &mut dyn ServerHost, level: LevelRef, departing: Option<u128>) {
    let Some(players) = level.online_players(host).and_then(|players| {
        players.into_iter()
            .filter(|p| Some(p.uuid()) != departing && p.game_mode(host) != Some(mod_api::GameMode::Spectator))
            .map(|p| p.entity().position(host))
            .collect::<Option<Vec<_>>>()
    }) else { return };
    for (id, mob) in tracked_mobs(level.dimension_id()) {
        let Some(entity) = host.entity(id) else { continue };
        if mob.spawn.is_none() || named(mob)
            || !entity.position(host).is_some_and(|at| {
                players.iter().all(|p| (0..3).map(|i| (at[i] - p[i]).powi(2)).sum::<f64>() > 128.0 * 128.0)
            })
            || entity.leash_holder(host) != Some(mod_api::LeashHolder::Unleashed)
            || entity.is_passenger(host) != Some(false)
            || entity.is_vehicle(host) != Some(false)
            || !host.mob(entity).is_some_and(|mob| mob.persistence_required(host) == Some(false))
            || !host.living(entity).and_then(|living| living.health(host)).is_some_and(|health| health > 0.0)
            || !entity.nbt(host).is_some_and(|data| !data.contains_key(&"CustomName".into()))
        {
            continue;
        }
        entity.discard(host);
    }
}

/// A named character: a boss with phases, or one of the people. One of each at a time, one at a
/// time, and seldom: about once or twice a night.
fn named(m: &mobs::Mob) -> bool {
    m.npc || !m.phases.is_empty()
}

/// Ticks between named characters: five minutes, half a night.
const NAMED_EVERY: u32 = 6000;
/// After that wait, the chance a named character the spawner picks really comes, in 100.
const NAMED_CHANCE: u32 = 35;
/// Yukari's chance to come through when a Yukari gap is used: 0.05%, 1 in 2,000.
const YUKARI_FROM_GAP: u32 = 2_000;
/// A mob with a classic version is that instead, when it spawns, 1 in this many times (0.005%).
const CLASSIC_IN: u32 = 20_000;

/// Bosses gone on to their next phase, and the health to start it with, still to be given them:
/// the game sets a mob whose death was canceled to 1 health after `living_death`.
static REFILL: Mutex<Vec<(u32, f32, u64)>> = Mutex::new(Vec::new());

/// Level ticks so far (each dimension's), the clock for `HITS`.
static TICKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The tick each of our mobs last took real damage. Horizon's `/kill` sets health to 0 with no blow
/// (and its death hook names the last real one), so a death with no damage just before it is a
/// kill command, or another mod's, and ends the mob for good.
static HITS: LazyLock<Mutex<HashMap<u32, u64>>> = LazyLock::new(Default::default);

/// How many level ticks after a blow a death may still be that blow's: the death is settled in the
/// tick it lands or the next, and with three dimensions ticking that is up to six level ticks.
const BLOW_TICKS: u64 = 8;

fn now() -> u64 {
    TICKS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Whether a death ends the mob for good rather than starting its next phase: the kill command, the
/// void, or no blow just before it.
fn ends_for_good(damage_type: &str, last_hit: Option<u64>, now: u64) -> bool {
    matches!(
        damage_type,
        "minecraft:generic_kill" | "minecraft:out_of_world"
    ) || last_hit.is_none_or(|hit| now.saturating_sub(hit) > BLOW_TICKS)
}

/// The phase a boss felled in `phase` comes back in, if any. Only the classic bosses come back:
/// every other boss has one life, with the health of its strongest phase (`Mob::max_health`).
fn next_life(spec: &mobs::Mob, phase: i32, killed: bool) -> Option<&mobs::Phase> {
    if killed || !spec.classic() {
        return None;
    }
    spec.phases
        .iter()
        .find(|p| p.from == phase)
        .filter(|p| p.hp > 0.0)
}

/// Everything kept about mob `id`: it is gone, or its id is someone else's now.
fn forget(id: u32) {
    if let Ok(mut b) = ai::BOSSES.lock() {
        b.remove(&id);
    }
    if let Ok(mut s) = ai::STATUS.lock() {
        s.remove(&id);
    }
    if let Ok(mut h) = HITS.lock() {
        h.remove(&id);
    }
    if let Ok(mut r) = REFILL.lock() {
        r.retain(|e| e.0 != id);
    }
}

static SERVER: LazyLock<Mutex<Server>> = LazyLock::new(|| {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64);
    Mutex::new(Server {
        config: config::Loaded::new(),
        shots: HashMap::new(),
        casts: HashMap::new(),
        stops: HashMap::new(),
        classic: HashSet::new(),
        halved: HashSet::new(),
        bars: HashMap::new(),
        next_id: 1,
        rng: Rng::new(seed),
        named_wait: NAMED_EVERY / 2,
    })
});

fn ray_box(a: V3, d: V3, reach: f64, b: [[f64; 3]; 2]) -> Option<f64> {
    let (mut near, mut far) = (0.0f64, reach);
    for (o, v, lo, hi) in [
        (a.x, d.x, b[0][0], b[1][0]),
        (a.y, d.y, b[0][1], b[1][1]),
        (a.z, d.z, b[0][2], b[1][2]),
    ] {
        if v.abs() < 1e-12 {
            if o < lo || o > hi {
                return None;
            }
        } else {
            let (t0, t1) = ((lo - o) / v, (hi - o) / v);
            near = near.max(t0.min(t1));
            far = far.min(t0.max(t1));
            if near > far {
                return None;
            }
        }
    }
    Some(near)
}

fn grow(b: [[f64; 3]; 2], by: f64) -> [[f64; 3]; 2] {
    [
        [b[0][0] - by, b[0][1] - by, b[0][2] - by],
        [b[1][0] + by, b[1][1] + by, b[1][2] + by],
    ]
}

fn middle(b: [[f64; 3]; 2]) -> V3 {
    v3(
        (b[0][0] + b[1][0]) / 2.0,
        (b[0][1] + b[1][1]) / 2.0,
        (b[0][2] + b[1][2]) / 2.0,
    )
}

fn passable(block: &str) -> bool {
    const SOFT: [&str; 30] = [
        "air",
        "water",
        "lava",
        "grass",
        "fern",
        "torch",
        "vine",
        "vines",
        "sapling",
        "rail",
        "button",
        "carpet",
        "sign",
        "kelp",
        "kelp_plant",
        "snow",
        "bush",
        "flower",
        "tulip",
        "poppy",
        "dandelion",
        "orchid",
        "allium",
        "bluet",
        "daisy",
        "cornflower",
        "lily_of_the_valley",
        "mushroom",
        "roots",
        "sprouts",
    ];
    let name = block.split('[').next().unwrap_or(block);
    SOFT.iter().any(|s| name.ends_with(s))
}

fn is_ours(kind: &str) -> bool {
    kind.starts_with("thmobs:")
}

/// Whether it is day, as the sky's light goes.
fn daytime(time: i64) -> bool {
    let t = time.rem_euclid(24000);
    !(12_542..=23_459).contains(&t)
}

/// Whether the mob kind is one of the classic bosses.
fn is_classic(kind: &str) -> bool {
    kind.starts_with("thmobs:classic_") || kind == GREAT_DOLL
}

/// Yukari Great Doll: classic Yukari with a quarter of her health and half her danmaku's damage,
/// who comes through a gap made global with a Yukari umbrella in place of Yukari.
const GREAT_DOLL: &str = "thmobs:yukari_great_doll";

struct PlayerView {
    handle: PlayerRef,
    uuid: u128,
    entity_id: i32,
    dimension: String,
    position: [f64; 3],
    health: f32,
    game_mode: i32,
}

fn players(host: &dyn ServerHost, dimension: &str) -> Vec<PlayerView> {
    host.level(dimension)
        .and_then(|level| level.online_players(host))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|handle| {
            let entity = handle.entity();
            Some(PlayerView {
                handle,
                uuid: handle.uuid(),
                entity_id: entity.id() as i32,
                dimension: dimension.to_owned(),
                position: entity.position(host)?,
                health: host.living(entity)?.health(host)?,
                game_mode: if handle.game_mode(host)? == mod_api::GameMode::Spectator {
                    3
                } else {
                    0
                },
            })
        })
        .collect()
}

fn resident_entities(host: &dyn Host, level: LevelRef) -> Option<Vec<mod_api::EntityRef>> {
    // A discard can leave a stale ID pending in the SDK's level index until
    // the tick ends. The bounding-box query filters these handles and only
    // visits entities already resident in memory.
    level.all_entities(host)
        .or_else(|| level.entities_in(host, [f64::MIN; 3], [f64::MAX; 3]))
}

fn block_id(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<String> {
    level.block(host, pos).map(|block| block.id(host))
}

fn bounds(host: &dyn Host, entity: mod_api::EntityRef) -> Option<[[f64; 3]; 2]> {
    entity.bounding_box(host)
}

fn set_mob_health(host: &mut dyn ServerHost, dimension: &str, id: u32, hp: f32) -> bool {
    let Some(entity) = host.entity(id) else {
        return false;
    };
    if !entity
        .level(host)
        .is_some_and(|level| level.dimension_id() == dimension)
    {
        return false;
    }
    host.living(entity)
        .is_some_and(|living| living.set_health(host, hp))
}

fn hurt(host: &mut dyn ServerHost, id: u32, damage: f32, owner: u32) {
    if let Some(victim) = host.entity(id) {
        let causing = host.entity(owner);
        victim.hurt(host, "minecraft:mob_projectile", damage, None, causing);
    }
}

fn apply_effect(
    host: &mut dyn ServerHost,
    entity: mod_api::EntityRef,
    id: &str,
    ticks: i32,
    level: i32,
) {
    if let (Some(living), Some(effect)) = (host.living(entity), host.effect_key(id)) {
        living.add_effect(
            host,
            &ActiveEffect::new(effect, ticks, level, false, true, true),
        );
    }
}

/// How tall the mob stands.
fn height_of(host: &dyn Host, id: u32) -> f64 {
    host.entity(id)
        .and_then(|e| bounds(host, e))
        .map_or(1.8, |b| b[1][1] - b[0][1])
}

/// Whether nothing solid stands on the way from `a` to `b`.
fn clear(host: &dyn Host, level: mod_api::LevelRef, a: V3, b: V3) -> bool {
    let d = b - a;
    let n = (d.length() * 2.0).ceil().max(1.0) as i32;
    (1..=n).all(|i| {
        let p = a + d * (f64::from(i) / f64::from(n));
        block_id(
            host,
            level,
            [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32],
        )
        .is_none_or(|b| passable(&b))
    })
}

fn dot(a: V3, b: V3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

/// A mob as a caster: where it stands and looks.
fn actor_of(host: &dyn Host, id: u32) -> Option<Actor> {
    let e = host.entity(id)?;
    let pos = V3::from_array(e.position(host)?);
    let [yaw, pitch] = e.rotation(host).unwrap_or([0.0, 0.0]);
    let eye_height = e.eye_height(host).unwrap_or(1.62);
    Some(Actor {
        pos,
        eye_height,
        yaw,
        pitch,
    })
}

impl Server {
    fn give_id(&mut self, mut s: Shot) -> Shot {
        s.id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        s
    }

    fn add(&mut self, dimension: &str, mut s: Shot, owner: u32, batch: &mut Batch) {
        s.fancy = s.fancy || self.classic.contains(&owner);
        if self.halved.contains(&owner) {
            slow(&mut s);
        }
        let s = self.give_id(s);
        batch.fired.push(s.clone());
        self.shots
            .entry(dimension.to_owned())
            .or_default()
            .push(Live { shot: s, owner });
    }

    fn tick(&mut self, host: &mut dyn ServerHost, dimension: &str, mobs: &[(u32, &mobs::Mob)]) -> Batch {
        let mut batch = Batch::default();
        let level = host.level(dimension);
        let here = |id| mobs.binary_search_by_key(&id, |(id, _)| *id).is_ok();
        let queued = ai::FIRED.lock().is_ok_and(|q| q.iter().any(|(id, _, _)| here(*id)))
            || ai::CASTS.lock().is_ok_and(|q| q.iter().any(|(id, _)| here(*id)));
        if !queued && !self.casts.contains_key(dimension) && !self.stops.contains_key(dimension)
            && self.shots.get(dimension).is_none_or(Vec::is_empty)
        {
            return batch;
        }
        self.classic.clear();
        self.halved.clear();
        for &(id, spec) in mobs {
            if spec.classic() {
                self.classic.insert(id);
            }
            if spec.id == "yukari_great_doll" {
                self.halved.insert(id);
            }
        }
        let dimension_players = players(host, dimension);
        let player_ids: HashMap<u32, u128> = dimension_players.iter()
            .filter(|p| p.entity_id > 0)
            .map(|p| (p.entity_id as u32, p.uuid))
            .collect();
        let player = dimension_players.iter()
            .filter(|p| p.health > 0.0 && p.game_mode != 3)
            .map(|p| V3::from_array(p.position) + v3(0.0, 0.9, 0.0))
            .collect::<Vec<_>>();

        // What the mobs of this level fired, and the cards they declared.
        let fired: Vec<(u32, Shot, bool)> = ai::FIRED
            .lock()
            .map(|mut q| {
                let (fired, keep) = std::mem::take(&mut *q)
                    .into_iter()
                    .partition(|(id, _, _)| here(*id));
                *q = keep;
                fired
            })
            .unwrap_or_default();
        for (owner, s, sound) in fired {
            if sound && let Some(l) = level {
                l.play_sound(
                    host,
                    "minecraft:entity.arrow.shoot",
                    s.pos.array(),
                    SoundSource::Hostile,
                    0.6,
                    1.2,
                );
            }
            self.add(dimension, s, owner, &mut batch);
        }
        let declared: Vec<(u32, u32)> = ai::CASTS
            .lock()
            .map(|mut q| {
                let (declared, keep) = std::mem::take(&mut *q)
                    .into_iter()
                    .partition(|(id, _)| here(*id));
                *q = keep;
                declared
            })
            .unwrap_or_default();
        for (mob, number) in declared {
            let full = self.classic.contains(&mob);
            let card = if full {
                Card::classic_by_number(number)
            } else {
                Card::by_number(number)
            };
            let (Some(card), Some(user)) = (card, actor_of(host, mob)) else {
                continue;
            };
            let casts = self.casts.entry(dimension.to_owned()).or_default();
            casts.retain(|c| c.mob != mob);
            let target = player
                .first()
                .copied()
                .unwrap_or(user.pos + user.look() * 16.0);
            casts.push(Cast {
                card,
                mob,
                time: 0,
                pace: 0.0,
                card_pos: card_position(&user),
                card_yaw: user.yaw,
                card_pitch: user.pitch,
                state: [0.0; 2],
                target,
                full,
            });
        }

        // The spell cards being cast.
        let mut casts = self.casts.remove(dimension).unwrap_or_default();
        casts.retain_mut(|cast| {
            let Some(user) = actor_of(host, cast.mob) else {
                return false;
            };
            cast.pace += if cast.full { 1.0 } else { ai::PACE };
            if cast.pace < 1.0 {
                return true;
            }
            cast.pace -= 1.0;
            let near = player.iter().copied().min_by(|a, b| {
                (*a - user.pos)
                    .length()
                    .total_cmp(&(*b - user.pos).length())
            });
            if let Some(t) = near {
                cast.target = t;
            }
            let ctx = Ctx {
                time: cast.time,
                level: ai::level(),
                user,
                target: cast.target,
                target_alive: near.is_some(),
                card: cast.card_pos,
                card_yaw: cast.card_yaw,
                card_pitch: cast.card_pitch,
                user_height: height_of(host, cast.mob),
            };
            let mut out = Vec::new();
            let mut fx = Vec::new();
            cast.card.tick(
                &ctx,
                &mut cast.state,
                &mut Fire {
                    out: &mut out,
                    rng: &mut self.rng,
                },
                &mut fx,
            );
            if cast.card == Card::PerfectFreeze && cast.time == 64 {
                self.freeze(dimension, ctx.pos_user(), &mut out, &mut batch.gone);
            }
            for f in fx {
                match f {
                    Fx::Sound(sound, volume, pitch) => {
                        if let Some(l) = level {
                            l.play_sound(
                                host,
                                sound,
                                ctx.pos_user().array(),
                                SoundSource::Hostile,
                                volume,
                                pitch,
                            );
                        }
                    }
                    Fx::TimeStop(ticks) => {
                        let stops = self.stops.entry(dimension.to_owned()).or_default();
                        stops.retain(|s| s.mob != cast.mob);
                        stops.push(Stop {
                            mob: cast.mob,
                            left: ticks,
                        });
                    }
                    Fx::Redirect => {
                        let target = near.filter(|_| ctx.target_alive);
                        self.redirect(
                            dimension,
                            cast.mob,
                            cast.card_pos,
                            target,
                            user.eye_height,
                            &mut out,
                            &mut batch.gone,
                        );
                    }
                    Fx::Gap { tries, yaw } => {
                        let Some(l) = level else { continue };
                        if let Some(at) = tries.into_iter().find(|p| clear(host, l, user.pos, *p)) {
                            let s = shot_full(form::GAP, 0, 2.0, 0.0, 0, 10, special::GAP);
                            out.push(Shot::new(
                                at,
                                angle(yaw, 0.0),
                                0.0,
                                pattern::rotate_default(),
                                0.0,
                                9999,
                                0.0,
                                0.0,
                                0.0,
                                V3::ZERO,
                                s,
                            ));
                        }
                    }
                }
            }
            for s in out {
                self.add(dimension, s, cast.mob, &mut batch);
            }
            cast.time += 1;
            cast.time < cast.card.info().end
        });
        if !casts.is_empty() {
            self.casts.insert(dimension.to_owned(), casts);
        }

        // Stopped time: what is left of it, and where.
        let mut stops = self.stops.remove(dimension).unwrap_or_default();
        stops.retain_mut(|s| {
            s.left -= 1;
            s.left > 0 && here(s.mob)
        });
        let stopped: Vec<(V3, i32)> = stops
            .iter()
            .filter_map(|s| actor_of(host, s.mob).map(|a| (a.pos, s.left)))
            .collect();
        if !stops.is_empty() {
            self.stops.insert(dimension.to_owned(), stops);
        }

        if self.shots.get(dimension).is_some_and(|v| !v.is_empty()) {
            let difficulty = level.and_then(|l| l.difficulty(host)).unwrap_or("normal");
            let mut owners: HashMap<u32, Option<Actor>> = HashMap::new();
            let nearest = |at: V3| {
                player
                    .iter()
                    .copied()
                    .min_by(|a, b| (*a - at).length().total_cmp(&(*b - at).length()))
            };
            // One snapshot per active projectile tick. The SDK box API scans
            // the level even for a small box, so index the reusable snapshot.
            let mut index = spatial::Grid::default();
            let bodies: Vec<_> = level.and_then(|l| resident_entities(host, l))
                .unwrap_or_default()
                .into_iter()
                .filter_map(|e| {
                    host.living(e)?;
                    if is_ours(&e.kind(host)?.id(host)) {
                        return None;
                    }
                    let b = bounds(host, e)?;
                    index.insert(b);
                    Some((e.id(), b))
                })
                .collect();
            let mut candidates = Vec::new();

            let mut shots = self.shots.remove(dimension).unwrap_or_default();
            let mut kept = Vec::with_capacity(shots.len());
            let mut fresh = Vec::new();
            let drained: Vec<Live> = shots.drain(..).collect();
            for mut live in drained {
                let scale = if self.halved.contains(&live.owner) { 0.5 } else { 1.0 };
                // In stopped time a shot that has flown two ticks hangs until the time runs again.
                if live.shot.hold == 0 && live.shot.ticks >= 2 && live.shot.anchor.is_none() {
                    let at = live.shot.pos;
                    if let Some((_, left)) = stopped.iter().find(|(c, _)| {
                        let d = at - *c;
                        d.x.abs() <= 40.0 && d.y.abs() <= 40.0 && d.z.abs() <= 40.0
                    }) {
                        live.shot.hold = *left;
                        batch.fired.push(live.shot.clone());
                    }
                }
                if !live.shot.step() {
                    continue;
                }
                let owner = *owners
                    .entry(live.owner)
                    .or_insert_with(|| actor_of(host, live.owner));
                let mut resend = false;
                let mut gone = false;
                // A held laser goes where its owner goes.
                if let Some((out, up)) = live.shot.anchor {
                    let Some(o) = owner else {
                        batch.gone.push(live.shot.id);
                        continue;
                    };
                    let s = &mut live.shot;
                    s.pos = o.pos + s.angle * f64::from(out) + v3(0.0, f64::from(up), 0.0);
                    s.prev = s.pos;
                    resend = true;
                }
                if live.shot.hold == 0 {
                    let s = &mut live.shot;
                    match s.special {
                        // Fantasy Seal's orb homes on the nearest player, twelve degrees a tick at most.
                        special::FANTASY_ORB if s.ticks >= 15 => {
                            s.limit = 0.6;
                            s.accel = 0.03;
                            if let Some(t) = nearest(s.pos) {
                                let to = (t + v3(0.0, 0.72, 0.0) - s.pos).normalize();
                                let axis = cross(s.angle, to);
                                if axis.length() > 1e-6 {
                                    let deg = dot(s.angle.normalize(), to)
                                        .clamp(-1.0, 1.0)
                                        .acos()
                                        .to_degrees()
                                        .min(12.0);
                                    s.angle =
                                        rotate(axis.normalize(), s.angle, deg as f32).normalize();
                                    s.motion = s.angle * s.speed();
                                }
                            }
                            resend = true;
                        }
                        // Bewitching Power Spoiler's scales fly back to their shooter.
                        special::SPOILER | special::SPOILER_BACK if s.ticks >= 20 => {
                            if let Some(o) = owner {
                                s.limit = 1.0;
                                s.accel = 0.05;
                                s.angle = angle_to(s.pos, o.pos + v3(0.0, 1.0, 0.0));
                                s.motion = s.angle * s.speed();
                                resend = true;
                                if (s.pos - o.pos).length() < 1.5 {
                                    gone = true;
                                    if s.special == special::SPOILER_BACK
                                        && let Ok(mut b) = ai::BUFFS.lock()
                                    {
                                        b.push(live.owner);
                                    }
                                }
                            }
                        }
                        // Meteor on Earth's familiar sheds its lights about its shooter's look.
                        special::METEOR if s.ticks == 1 => {
                            if let Some(o) = owner {
                                s.rotate = o.look();
                                resend = true;
                            }
                        }
                        _ => {}
                    }
                }
                // Gagouji Cyclone's shots turn at their shooter's target at their end.
                if live.shot.special == special::SEEK_TARGET && live.shot.ticks >= live.shot.end {
                    if let (Some(t), Some(from)) =
                        (player.first(), host.entity(live.owner).and_then(|e| bounds(host, e)))
                    {
                        let s = &mut live.shot;
                        s.angle = angle_to(middle(from), *t - v3(0.0, 0.9, 0.0));
                        s.motion = s.angle * s.speed();
                    }
                    let s = &mut live.shot;
                    s.end = 90;
                    s.accel = 0.03;
                    s.limit = 0.2 + f64::from(ai::level()) * 0.1;
                    s.special = 0;
                    batch.fired.push(s.clone());
                }
                // Bursts: shots some specials fire from where they are.
                let full = self.classic.contains(&live.owner);
                let target = nearest(live.shot.pos);
                let burst = if live.shot.hold > 0 {
                    Vec::new()
                } else {
                    bursts(
                        &live.shot,
                        owner.map(|u| u.look()),
                        target,
                        full,
                        &mut self.rng,
                    )
                };
                let doll = self.halved.contains(&live.owner);
                for mut child in burst {
                    child.fancy = live.shot.fancy;
                    if doll {
                        slow(&mut child);
                    }
                    let child = self.give_id(child);
                    batch.fired.push(child.clone());
                    fresh.push(Live {
                        shot: child,
                        owner: live.owner,
                    });
                }
                // Fantasy Seal's amulets: out, still, a change of colour, then at the target, twice.
                if (11100..=11105).contains(&live.shot.special) && live.shot.ticks == live.shot.end
                {
                    let from = host.entity(live.owner).and_then(|e| bounds(host, e)).map(middle);
                    fantasy_seal(&mut live.shot, from.zip(player.first().copied()));
                    batch.fired.push(live.shot.clone());
                }
                let mut hit = gone;
                // A laser: cut short by a wall, it hits all along itself every tick and stays.
                let s = &live.shot;
                let passes = matches!(s.form, form::FAMILIAR | form::HIDDEN | form::GAP)
                    || (21..=25).contains(&s.form)
                    || s.special == special::SPOILER_BACK;
                if !hit && s.solid() && s.is_laser() {
                    let dir = s.angle.normalize();
                    let (from, length, size, owner_id, damage) = (
                        s.pos,
                        f64::from(s.laser),
                        f64::from(s.size),
                        live.owner,
                        s.damage_for(false, difficulty) * scale,
                    );
                    let moving = s.speed() > 1e-6;
                    if let Some(l) = level {
                        let mut d = 0.25;
                        while d < length {
                            let p = from + dir * d;
                            if block_id(
                                host,
                                l,
                                [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32],
                            )
                            .is_some_and(|b| !passable(&b))
                            {
                                if d <= 0.3 && moving {
                                    hit = true;
                                } else {
                                    live.shot.laser = d as f32;
                                    resend = true;
                                }
                                break;
                            }
                            d += 0.5;
                        }
                    }
                    let length = f64::from(live.shot.laser);
                    let end = from + dir * length.max(1e-6);
                    for &candidate in index.query(spatial::swept(from.array(), end.array(), size), &mut candidates) {
                        let (id, b) = bodies[candidate];
                        if id == owner_id {
                            continue;
                        }
                        if ray_box(from, dir, length.max(1e-6), grow(b, size)).is_some() {
                            hurt(host, id, damage, owner_id);
                        }
                    }
                } else if !hit && s.solid() && s.special == special::SPOILER {
                    // The scale marks what it touches (wither and hunger) and turns back, unharmed.
                    let travel = s.pos - s.prev;
                    let reach = travel.length();
                    let dir = if reach > 1e-9 {
                        travel * (1.0 / reach)
                    } else {
                        v3(0.0, 1.0, 0.0)
                    };
                    let (prev, size) = (s.prev, f64::from(s.size));
                    let reach = reach.max(1e-6);
                    let end = prev + dir * reach;
                    let touched = index.query(spatial::swept(prev.array(), end.array(), size * 0.5), &mut candidates)
                        .iter().map(|&i| bodies[i]).find(|(id, b)| {
                            *id != live.owner
                                && ray_box(prev, dir, reach, grow(*b, size * 0.5)).is_some()
                        });
                    if let Some((id, _)) = touched {
                        if let Some(uuid) = player_ids.get(&id) {
                            for effect in ["minecraft:wither", "minecraft:hunger"] {
                                if let Some(player) = host.player(*uuid) {
                                    apply_effect(host, player.entity(), effect, 400, 1);
                                }
                            }
                        }
                        live.shot.special = special::SPOILER_BACK;
                        resend = true;
                    }
                }
                let s = &live.shot;
                // A familiar's shot carries on; it doesn't hit; nor do the unseen ones, nor lasers here.
                if !hit && s.solid() && !passes && !s.is_laser() && s.special != special::SPOILER {
                    let travel = s.pos - s.prev;
                    let reach = travel.length();
                    let dir = if reach > 1e-9 {
                        travel * (1.0 / reach)
                    } else {
                        v3(0.0, 1.0, 0.0)
                    };
                    let end = s.prev + dir * reach.max(1e-6);
                    for &candidate in index.query(spatial::swept(s.prev.array(), end.array(), f64::from(s.size) * 0.5), &mut candidates) {
                        let (id, b) = bodies[candidate];
                        if id == live.owner {
                            continue;
                        }
                        if ray_box(
                            s.prev,
                            dir,
                            reach.max(1e-6),
                            grow(b, f64::from(s.size) * 0.5),
                        )
                        .is_some()
                        {
                            hurt(
                                host,
                                id,
                                s.damage_for(false, difficulty) * scale,
                                live.owner,
                            );
                            hit = true;
                            break;
                        }
                    }
                    if !hit && let Some(l) = level {
                        let at = [
                            s.pos.x.floor() as i32,
                            s.pos.y.floor() as i32,
                            s.pos.z.floor() as i32,
                        ];
                        hit = block_id(host, l, at).is_some_and(|b| !passable(&b));
                    }
                }
                if hit {
                    batch.gone.push(live.shot.id);
                } else {
                    if resend {
                        batch.fired.push(live.shot.clone());
                    }
                    kept.push(live);
                }
            }
            kept.append(&mut fresh);
            *self.shots.entry(dimension.to_owned()).or_default() = kept;
        }
        batch
    }

    /// Perfect Freeze at its sixty-fourth tick: every shot within twenty blocks stops, turns white and
    /// faces a random way, to set off again twenty ticks on.
    fn freeze(&mut self, dimension: &str, at: V3, fired: &mut Vec<Shot>, gone: &mut Vec<u32>) {
        let Some(shots) = self.shots.get_mut(dimension) else {
            return;
        };
        let mut frozen = Vec::new();
        shots.retain(|live| {
            let d = live.shot.pos - at;
            let near = d.x.abs() <= 20.0 && d.y.abs() <= 20.0 && d.z.abs() <= 20.0;
            if near {
                frozen.push(live.shot.clone());
                gone.push(live.shot.id);
            }
            !near
        });
        for old in frozen {
            let a = angle(self.rng.float() * 360.0, self.rng.float() * 180.0 - 90.0);
            let data = shot_full(
                old.form,
                7,
                old.size,
                old.damage,
                0,
                old.end - old.ticks + 50,
                special::FREEZE,
            );
            fired.push(Shot::new(
                old.pos,
                a,
                0.0,
                pattern::rotate_default(),
                0.0,
                9999,
                1e-4,
                1e-4,
                0.0,
                V3::ZERO,
                data,
            ));
        }
    }

    /// Killing Doll in stopped time: a quarter of the caster's knives (form 28, colour 0 or 1) within
    /// twenty blocks of the card are taken up and thrown again at the target, red.
    #[allow(clippy::too_many_arguments)]
    fn redirect(
        &mut self,
        dimension: &str,
        mob: u32,
        card: V3,
        target: Option<V3>,
        eye: f64,
        fired: &mut Vec<Shot>,
        gone: &mut Vec<u32>,
    ) {
        let Server { shots, rng, .. } = self;
        let Some(shots) = shots.get_mut(dimension) else {
            return;
        };
        let mut taken = Vec::new();
        shots.retain(|live| {
            let s = &live.shot;
            let d = s.pos - card;
            let near = d.x.abs() <= 20.0 && d.y.abs() <= 20.0 && d.z.abs() <= 20.0;
            if live.owner == mob
                && near
                && s.form == form::KNIFE
                && s.color <= 1
                && rng.int(100) < 25
            {
                taken.push(s.pos);
                gone.push(s.id);
                false
            } else {
                true
            }
        });
        let mut fire = Fire {
            out: fired,
            rng: &mut self.rng,
        };
        for at in taken {
            let at = v3(at.x, at.y - (eye - 0.25), at.z);
            let way = match target {
                Some(t) => angle_to(at, t),
                None => vec::angle_random(fire.rng),
            };
            let first = 0.2 + fire.rng.double() * 0.5;
            fire.shot(
                at,
                way,
                0.0,
                first,
                0.7,
                0.02,
                V3::ZERO,
                shot_full(form::KNIFE, 2, shot::SIZE[28], 12.0, 0, 80, 0),
            );
        }
    }

    /// The classic bosses' bars: each player within 64 blocks sees the boss's health in all its cards,
    /// its name and the card it is casting. A bar goes when its boss is gone or out of reach.
    fn boss_bars(&mut self, host: &mut dyn ServerHost, dimension: &str, bosses: &[(u32, &mobs::Mob)]) {
        if bosses.is_empty() && !self.bars.values().any(|(level, _, _)| level == dimension) {
            return;
        }
        let players: Vec<_> = players(host, dimension)
            .into_iter()
            .filter(|p| p.dimension == dimension)
            .collect();
        let mut want: HashMap<(u128, u32), (String, i32, String)> = HashMap::new();
        for &(id, spec) in bosses {
            let Some(at) = host.entity(id).and_then(|e| e.position(host)) else {
                continue;
            };
            let health = ai::status(id).map_or(spec.max_health(), |s| s.0);
            if health <= 0.0 {
                continue;
            }
            let max = spec.max_health();
            let fill = ((health / max).clamp(0.0, 1.0) * 1000.0) as i32;
            let card = self
                .casts
                .get(dimension)
                .and_then(|c| c.iter().find(|c| c.mob == id))
                .map(|c| c.card.info().name);
            let name = match card {
                Some(card) => format!("{}: {card}", spec.name),
                None => spec.name.to_owned(),
            };
            for p in &players {
                let d = V3::from_array(p.position) - V3::from_array(at);
                if d.length() <= 64.0 {
                    want.insert((p.uuid, id), (dimension.to_owned(), fill, name.clone()));
                }
            }
        }
        let bar_id = |&(player, mob): &(u128, u32)| format!("thmobs_native:{player:032x}_{mob}");
        for (key, value) in &want {
            if self.bars.get(key) == Some(value) {
                continue;
            }
            let id = bar_id(key);
            let Some(bar) = host.boss_bar(&id).or_else(|| {
                host.create_boss_bar(&id, Text::literal(&value.2))
                    .ok()
                    .flatten()
            }) else {
                continue;
            };
            let _ = bar.set_name(host, Text::literal(&value.2));
            bar.set_color(host, BossBarColor::Purple);
            bar.set_max(host, 1000);
            bar.set_value(host, value.1);
            bar.set_visible(host, true);
            if let Some(player) = host.player(key.0) {
                bar.add_player(host, player);
            }
        }
        let stale: Vec<_> = self
            .bars
            .iter()
            .filter(|(k, v)| v.0 == dimension && !want.contains_key(k))
            .map(|(k, _)| *k)
            .collect();
        for key in stale {
            if let Some(bar) = host.boss_bar(&bar_id(&key)) {
                bar.remove(host);
            }
            self.bars.remove(&key);
        }
        self.bars.extend(want);
    }

    /// Where something may stand in column `x`, `z` near height `near`: on the surface in the
    /// overworld, else the first floor found going down from a little above.
    fn floor(
        host: &dyn ServerHost,
        level: LevelRef,
        dimension: &str,
        x: i32,
        z: i32,
        near: i32,
    ) -> Option<i32> {
        if dimension == "minecraft:overworld" {
            let heights = host
                .surface_heights(dimension, &[(x, z)], Heightmap::MotionBlocking)
                .ok()?;
            return heights.first().copied().flatten();
        }
        let open = |y: i32| block_id(host, level, [x, y, z]).is_some_and(|b| b.ends_with("air"));
        (near - 16..near + 16)
            .rev()
            .find(|&y| open(y) && open(y + 1) && !open(y - 1))
    }

    /// About once a second for each player, a try at a group of one of our mobs 24 to 48 blocks off,
    /// by the original's weights, biomes and rules.
    fn spawn(&mut self, host: &mut dyn ServerHost, dimension: &str, mobs: &[(u32, &mobs::Mob)]) {
        let Some(level) = host.level(dimension) else {
            return;
        };
        let players: Vec<_> = players(host, dimension)
            .into_iter()
            .filter(|p| p.dimension == dimension && p.game_mode != 3)
            .collect();
        if players.is_empty()
            || level.difficulty(host) == Some("peaceful")
            || matches!(level.game_rule(host, "minecraft:spawn_mobs"), Ok(Some(mod_api::GameRuleValue::Bool(false))))
        {
            return;
        }
        self.named_wait = self.named_wait.saturating_sub(1);
        let mut population = None;
        let mut spawned_named = HashSet::new();
        self.config
            .refresh(|| Some(host.instance_dir().join("config")));
        let cfg = self.config.config.clone();
        let mut weight = None;
        for p in players {
            // `rate` tries a second, about, at twenty ticks a second.
            if f64::from(self.rng.float()) >= cfg.rate / 20.0 {
                continue;
            }
            let total = *weight.get_or_insert_with(|| {
                mobs::MOBS.iter().filter_map(|m| cfg.rule(m)).map(|r| r.weight).sum()
            });
            if total == 0 {
                continue;
            }
            let ours = population.get_or_insert_with(|| mobs.iter()
                .filter_map(|(id, _)| host.entity(*id)?.position(host))
                .collect::<Vec<_>>());
            let near = ours
                .iter()
                .filter(|q| (q[0] - p.position[0]).hypot(q[2] - p.position[2]) < 64.0)
                .count();
            if near >= cfg.cap {
                continue;
            }
            let mut pick = self.rng.int(total);
            let Some(mob) = mobs::MOBS.iter().find(|m| match cfg.rule(m) {
                Some(r) if pick < r.weight => true,
                Some(r) => {
                    pick -= r.weight;
                    false
                }
                None => false,
            }) else {
                continue;
            };
            let (Some(rule), Some(chosen)) = (mob.spawn, cfg.rule(mob)) else {
                continue;
            };
            // A named character: only after the wait, now and then, and never two of the same.
            let is_named = named(mob);
            if is_named
                && (self.named_wait > 0
                    || self.rng.int(100) >= NAMED_CHANCE
                    || (spawned_named.contains(mob.id) || mobs.iter().any(|(_, resident)| resident.id == mob.id)))
            {
                continue;
            }
            let a = f64::from(self.rng.float()) * std::f64::consts::TAU;
            let r = 24.0 + f64::from(self.rng.float()) * 24.0;
            let (x, z) = (
                (p.position[0] + a.cos() * r).floor() as i32,
                (p.position[2] + a.sin() * r).floor() as i32,
            );
            let Some(y) = Self::floor(host, level, dimension, x, z, p.position[1] as i32) else {
                continue;
            };
            let Some(biome) = level.biome_at(host, [x, y, z]) else {
                continue;
            };
            let biome = biome
                .strip_prefix("minecraft:")
                .unwrap_or(&biome)
                .to_owned();
            if !rule.biomes.contains(&biome.as_str()) {
                continue;
            }
            let block_light = level
                .brightness(host, LightLayer::Block, [x, y, z])
                .unwrap_or_default();
            let sky = level
                .brightness(host, LightLayer::Sky, [x, y, z])
                .unwrap_or_default();
            let day = daytime(level.day_time(host).unwrap_or(0));
            match rule.light {
                mobs::Light::Fairy => {
                    if self.rng.int(100) < 50
                        || (y < 64 && y < self.rng.int(64) as i32)
                        || sky <= 7
                        || !day
                    {
                        continue;
                    }
                    let below = block_id(host, level, [x, y - 1, z]).unwrap_or_default();
                    if !["minecraft:grass_block", "minecraft:dirt", "minecraft:sand"]
                        .iter()
                        .any(|b| below.starts_with(b))
                    {
                        continue;
                    }
                }
                mobs::Light::Dark => {
                    let lit = if day {
                        sky.max(block_light) as u32
                    } else {
                        block_light as u32
                    };
                    if lit > self.rng.int(8) {
                        continue;
                    }
                }
                mobs::Light::Any => {}
                mobs::Light::Rare => {
                    if self.rng.int(100) < 98 {
                        continue;
                    }
                }
            }
            let count = if is_named {
                1
            } else {
                (chosen.min + self.rng.int(chosen.max - chosen.min + 1)).min((cfg.cap - near) as u32)
            };
            if is_named {
                self.named_wait = NAMED_EVERY;
            }
            for _ in 0..count {
                let (dx, dz) = (self.rng.int(5) as i32 - 2, self.rng.int(5) as i32 - 2);
                let at = [
                    f64::from(x + dx) + 0.5,
                    f64::from(y) + 0.1,
                    f64::from(z + dz) + 0.5,
                ];
                // Now and then the classic version comes instead.
                let this = mobs::classic_of(mob)
                    .filter(|_| self.rng.int(CLASSIC_IN) == 0)
                    .unwrap_or(mob);
                if summon(host, dimension, this, at).is_some() {
                    ours.push(at);
                    if is_named {
                        spawned_named.insert(mob.id);
                    }
                }
            }
        }
    }
}

/// The shots a special fires from where its shot is, this tick: `target` is the nearest player's
/// middle, `full` whether its shooter fires at the original's full rate.
fn bursts(
    s: &Shot,
    owner_look: Option<V3>,
    target: Option<V3>,
    full: bool,
    rng: &mut Rng,
) -> Vec<Shot> {
    let mut out = Vec::new();
    // A spray that runs every tick runs only on the mobs' share of them.
    let paced = |t: i32| ((t as f32) * ai::PACE).floor() != ((t - 1) as f32 * ai::PACE).floor();
    if !full && (11600..=11602).contains(&s.special) && !paced(s.ticks) {
        return out;
    }
    let level = ai::level();
    let mut fire = Fire { out: &mut out, rng };
    match s.special {
        // Marisa's familiars spray stars as they circle.
        11600 => fire.sphere(
            s.pos,
            s.angle,
            0.0,
            pattern::rotate_default(),
            0.0,
            9999,
            0.3,
            0.5,
            0.05,
            V3::ZERO,
            shot::shot(8, s.color, 0, 60, 0),
            3,
            0.0,
            0.0,
        ),
        11601 if s.ticks % 2 == 0 => fire.sphere(
            s.pos,
            s.angle,
            0.0,
            pattern::rotate_default(),
            0.0,
            9999,
            0.3,
            0.5,
            0.05,
            V3::ZERO,
            shot::shot(8, s.color, 0, 60, 0),
            2,
            0.0,
            0.0,
        ),
        11602 if s.ticks % 2 == 0 => fire.sphere(
            s.pos,
            s.angle,
            0.0,
            pattern::rotate_default(),
            0.0,
            9999,
            0.3,
            0.5,
            0.05,
            V3::ZERO,
            shot::shot(8, shot::RANDOM, 0, 60, 0),
            3,
            0.0,
            0.0,
        ),
        special::MISFORTUNE_FAN if s.ticks == 10 => {
            let d = shot_full(0, s.color, shot::SIZE[0], s.damage, 0, 60, 0);
            fire.wide(
                s.pos,
                s.angle,
                0.0,
                0,
                0.5,
                0.0,
                0.0,
                V3::ZERO,
                d,
                5,
                60.0,
                0.0,
                0.0,
            );
            for limit in [0.7, 0.8, 0.9, 1.0] {
                let d = shot_full(10, s.color, 0.1, s.damage, 0, 60, 0);
                fire.shot(s.pos, s.angle, 0.0, 0.5, limit, 0.1, V3::ZERO, d);
            }
        }
        special::MISFORTUNE_RING if s.ticks == 10 => {
            let d = shot_full(0, s.color, shot::SIZE[0], s.damage, 0, 60, 0);
            fire.circle(
                s.pos,
                s.angle,
                0.0,
                0,
                0.5,
                0.0,
                0.0,
                V3::ZERO,
                d,
                36,
                0.0,
                0.0,
            );
        }
        // Stardust Reverie's big star sheds a small one each tick, round its axis.
        special::STARDUST => {
            let way = rotate(s.rotate, s.angle, s.ticks as f32 * 13.0);
            let d = shot_full(s.form, s.color, 0.3, 5.0, 10, 50, 0);
            fire.shot_turning(
                s.pos,
                way,
                s.slope,
                s.rotate,
                0.0,
                9999,
                0.1,
                0.3,
                0.03,
                V3::ZERO,
                d,
            );
        }
        // Meteor on Earth's familiar: a light every other tick, still until its thirtieth.
        special::METEOR if s.ticks % 2 == 0 => {
            let way = rotate(s.angle, s.rotate, s.ticks as f32 * 13.0);
            fire.shot(
                s.pos,
                way,
                0.0,
                0.0,
                0.5,
                0.0,
                V3::ZERO,
                shot::shot(0, 5, 5, 40, special::METEOR_LIGHT),
            );
        }
        // Miracle Fruit's oval bursts at its end into eight lines of six.
        special::MIRACLE_FRUIT if s.ticks == s.end => {
            let mut a = fire.rng.float() * 360.0;
            for _ in 0..8 {
                let look = rotate(s.rotate, s.angle, a);
                for i in 0..6 {
                    fire.shot_turning(
                        s.pos,
                        look,
                        0.0,
                        s.rotate,
                        0.0,
                        9999,
                        0.5,
                        0.2,
                        0.8,
                        V3::ZERO,
                        shot_full(0, 0, 0.3, 6.0, 3 + i * 2, 90, 0),
                    );
                }
                a += 45.0;
            }
        }
        // Flying-glowworm Nest's gap fires its laser.
        special::GAP if s.ticks == 5 => {
            fire.laser_a(
                s.pos,
                s.angle,
                0.1,
                1.4,
                0.1,
                V3::ZERO,
                pattern::laser(4, 0.1, 4.0, 8.0, 0, 40, 0),
            );
        }
        // Curse of Dreams and Reality.
        special::CURSE_ORB if s.ticks == s.end => {
            fire.shot(
                s.pos,
                s.angle,
                0.0,
                0.0,
                0.0,
                0.0,
                V3::ZERO,
                shot_full(form::HIDDEN, 0, 0.0, 0.0, 0, 10, special::CURSE_EMIT),
            );
        }
        special::CURSE_EMIT if s.ticks % 2 == 0 => {
            let yaw = -vec::yaw_of(s.angle.x, s.angle.z) + (s.ticks * 5) as f32;
            let pitch = -(s.angle.y.clamp(-1.0, 1.0).asin().to_degrees() as f32);
            let d = shot_full(10, 5, shot::SIZE[10], 3.0, 0, 5, special::CURSE_SEEK);
            fire.sphere(
                s.pos,
                angle(yaw, pitch),
                0.0,
                pattern::rotate_default(),
                0.0,
                9999,
                0.35,
                0.35,
                0.0,
                V3::ZERO,
                d,
                (4 * level) as usize,
                0.1,
                0.0,
            );
        }
        special::CURSE_SEEK if s.ticks == s.end => {
            let way = target.map_or(s.angle, |t| angle_to(s.pos, t));
            fire.shot(
                s.pos,
                way,
                0.0,
                0.45,
                0.45,
                0.0,
                V3::ZERO,
                shot_full(10, 5, shot::SIZE[10], 4.0, 0, 80, 0),
            );
        }
        special::CURSE_SPLIT if s.ticks == s.end => {
            let mut corner = |dx: i32, dy: i32, dz: i32, color: u8| {
                let at = s.pos + v3(f64::from(dx), f64::from(dy), f64::from(dz));
                let d = shot_full(6, color, shot::SIZE[6] * 1.5, 4.0, 0, 80, 0);
                fire.sphere(
                    at,
                    s.angle,
                    0.0,
                    pattern::rotate_default(),
                    0.0,
                    9999,
                    0.2,
                    0.2,
                    0.0,
                    V3::ZERO,
                    d,
                    (4 * level) as usize,
                    0.1,
                    0.0,
                );
            };
            for i in [-1, 1] {
                for j in [-1, 1] {
                    for k in [-1, 1] {
                        corner(i, j, k, 5);
                    }
                }
            }
            for i in [-2, 0, 2] {
                for j in [-2, 0, 2] {
                    for k in [-2, 0, 2] {
                        corner(i, j, k, 2);
                    }
                }
            }
        }
        // Mesh of Light and Darkness: each hanging shot sets a laser along its way.
        special::MESH if s.ticks == 1 => {
            let l = pattern::laser(i32::from(s.color), 0.5, 20.0, s.damage, 20, s.end, 0);
            fire.laser_b(
                s.pos + s.angle,
                s.angle,
                pattern::rotate_default(),
                0.0,
                9999,
                l,
                None,
            );
        }
        special::MISFORTUNE_MARK if s.ticks == s.end => {
            let d = shot_full(14, s.color, shot::SIZE[14], s.damage, 0, 60, 0);
            let look = owner_look.unwrap_or(s.angle);
            fire.circle(
                s.pos,
                look,
                0.0,
                0,
                0.0,
                0.5,
                0.05,
                V3::ZERO,
                d,
                ai::level(),
                0.0,
                (s.id % 36) as f32 * 10.0,
            );
        }
        _ => {}
    }
    out
}

/// One step of Fantasy Seal's amulets (`ShotFantasySeal`), at the end of the step before.
fn fantasy_seal(s: &mut Shot, aim: Option<(V3, V3)>) {
    let speed = s.speed();
    let aim_at = |s: &mut Shot| {
        if let Some((from, to)) = aim {
            s.angle = angle_to(from, to);
        }
    };
    match s.special {
        11100 => {
            (s.form, s.special, s.end, s.limit, s.accel) = (4, 11101, 10, 0.0, 0.0);
            s.restart(0.0);
        }
        11101 => {
            (s.color, s.special, s.end) = (0, 11102, 10);
            s.restart(speed);
        }
        11102 => {
            (s.form, s.special, s.end, s.limit, s.accel) = (27, 11103, 30, 0.0, -0.04);
            aim_at(s);
            s.restart(0.75);
        }
        11103 => {
            (s.form, s.special, s.end) = (4, 11104, 10);
            s.restart(speed);
        }
        11104 => {
            (s.color, s.special, s.end) = (4, 11105, 10);
            s.restart(speed);
        }
        _ => {
            (s.form, s.special, s.end, s.limit, s.accel) = (27, 0, 50, 0.3, 0.0);
            aim_at(s);
            s.restart(0.3);
        }
    }
}

/// How fast the Great Doll's danmaku goes, of classic Yukari's.
const DOLL_SPEED: f64 = 0.5;

/// A shot of the Great Doll's: half as fast, speeding up half as much, to half the top speed.
fn slow(s: &mut Shot) {
    s.motion = s.motion * DOLL_SPEED;
    s.accel *= DOLL_SPEED;
    s.limit *= DOLL_SPEED;
}

/// Whether `block` is one of Yukari Gaps' gaps, and if so whether it is a Giga Gap (a gap made
/// global with a Yukari umbrella, since Yukari Gaps 0.7.0).
fn gap_kind(block: &str) -> Option<bool> {
    let id = block.split('[').next().unwrap_or(block);
    match id {
        "yukari_gap:yukari_gap" => Some(false),
        "yukari_gap:giga_gap" => Some(true),
        _ => None,
    }
}

/// The gap a player arrived by, from the gap blocks found round `to`: the one nearest across, at
/// its lower half (its lowest block in that column). Its position, and whether it is a Giga Gap.
fn arrival_gap(found: &[([i32; 3], bool)], to: [i32; 3]) -> Option<([i32; 3], bool)> {
    let across = |p: &[i32; 3]| (p[0] - to[0]).pow(2) + (p[2] - to[2]).pow(2);
    let nearest = found.iter().min_by_key(|(p, _)| (across(p), (p[1] - to[1]).abs()))?;
    found
        .iter()
        .filter(|(p, _)| p[0] == nearest.0[0] && p[2] == nearest.0[2])
        .min_by_key(|(p, _)| p[1])
        .copied()
}

/// Whether a gap made global with a Yukari umbrella is by `to`: Yukari Gaps keeps its gaps in
/// `yukari_gap/gaps.tsv` in the world, a line each (`H`, dimension, x, y, z of its lower half,
/// facing, owner, locked, global, ...).
fn global_gap_near(host: &dyn ServerHost, dimension: &str, to: [i32; 3]) -> bool {
    let Some(dir) = host.world_dir() else {
        return false;
    };
    let text = std::fs::read_to_string(dir.join("yukari_gap").join("gaps.tsv")).unwrap_or_default();
    global_gap_in(&text, dimension, to)
}

fn global_gap_in(text: &str, dimension: &str, to: [i32; 3]) -> bool {
    text.lines().any(|line| {
        let f: Vec<&str> = line.split('\t').collect();
        let at = |i: usize| f.get(i).and_then(|v| v.parse::<i32>().ok());
        let (Some(x), Some(y), Some(z)) = (at(2), at(3), at(4)) else {
            return false;
        };
        f.first() == Some(&"H")
            && f.get(1) == Some(&dimension)
            && f.get(8) == Some(&"1")
            && (x - to[0]).abs() <= 2
            && (-3..=2).contains(&(y - to[1]))
            && (z - to[2]).abs() <= 2
    })
}

/// The boss `id` on to the phase `next`: a moment unhurt, its attack started over, its danmaku gone.
fn begin_phase(id: u32, next: &mobs::Phase) {
    if let Ok(mut bosses) = ai::BOSSES.lock() {
        bosses.insert(
            id,
            ai::Boss {
                phase: next.to,
                counter: Some(-next.interval),
                invincible: next.invincible,
                ai: next.ai,
            },
        );
    }
    if let Ok(mut server) = SERVER.lock() {
        for (_, shots) in server.shots.iter_mut() {
            shots.retain(|l| l.owner != id);
        }
        for (_, casts) in server.casts.iter_mut() {
            casts.retain(|c| c.mob != id);
        }
        for (_, stops) in server.stops.iter_mut() {
            stops.retain(|s| s.mob != id);
        }
    }
}

/// The classic bosses' cards, as the Wither's phases: each has all its cards' health in one bar, and
/// each card's worth it loses breaks the card it is on and starts the next. One blow can break only
/// one card: past that, it starts the next card with that card's full share.
fn classic_cards(host: &mut dyn ServerHost, dimension: &str, bosses: &[(u32, &mobs::Mob)]) {
    for &(id, spec) in bosses {
        let Some((health, _)) = ai::status(id) else {
            continue;
        };
        if health <= 0.0 {
            continue;
        }
        let phase = ai::phase_of(id, spec);
        let card = spec.card_of(phase);
        let due = spec.card_at(health);
        if due <= card {
            continue;
        }
        let Some(next) = spec.phases.iter().find(|p| p.from == phase) else {
            continue;
        };
        explode(host, id, false);
        begin_phase(id, next);
        if due > card + 1 {
            let top = spec.top_of(card + 1);
            set_mob_health(host, dimension, id, top);
            if let Ok(mut s) = ai::STATUS.lock() {
                s.insert(id, (top, next.to));
            }
        }
    }
}

/// A classic boss's spell broken (a blast) or the boss beaten (a great one).
fn explode(host: &mut dyn ServerHost, id: u32, last: bool) {
    let Some(entity) = host.entity(id) else {
        return;
    };
    let (Some(at), Some(level)) = (entity.position(host), entity.level(host)) else {
        return;
    };
    let at = [at[0], at[1] + 1.0, at[2]];
    let (count, volume) = if last { (2, 4.0) } else { (6, 2.0) };
    if let Some(kind) = EXPLOSION_PARTICLES
        .get()
        .map(|keys| keys[usize::from(last)])
    {
        level.send_particles(
            host,
            &ParticleOptions::simple(kind),
            None,
            at,
            [0.6, 0.6, 0.6],
            0.0,
            count,
        );
    }
    level.play_sound(
        host,
        "minecraft:entity.generic.explode",
        at,
        SoundSource::Hostile,
        volume,
        if last { 0.8 } else { 1.2 },
    );
}

/// A trade for a wandering trader's `Offers`: `count` of `buy` for one `sell`.
fn offer(buy: &str, count: i32, sell: &str) -> Tag {
    let stack = |id: &str, count: i32| -> Tag {
        Tag::Compound(
            [
                ("id".into(), Tag::String(id.into())),
                ("count".into(), Tag::Int(count)),
            ]
            .into(),
        )
    };
    Tag::Compound(
        [
            ("buy".into(), stack(buy, count)),
            ("sell".into(), stack(sell, 1)),
            ("maxUses".into(), Tag::Int(9999)),
            ("rewardExp".into(), Tag::Byte(0)),
        ]
        .into(),
    )
}

fn summon(
    host: &mut dyn ServerHost,
    dimension: &str,
    mob: &mobs::Mob,
    at: [f64; 3],
) -> Option<u32> {
    // A classic boss comes with all its cards' health; any other boss, with one life, with the
    // health of its strongest phase.
    let health = mob.max_health();
    let mut nbt: Compound = [
        ("Silent".into(), Tag::Byte(1)),
        ("Health".into(), Tag::Float(health.to_bits())),
    ]
    .into();
    if mob.id == "rinnosuke" {
        // The one trade of his made of vanilla things; the rest are Touhou Items Mod's items, to come.
        let offers: Compound = [(
            "Recipes".into(),
            Tag::List(vec![offer(
                "minecraft:gold_ingot",
                1,
                "minecraft:ender_pearl",
            )]),
        )]
        .into();
        nbt = [
            ("Silent".into(), Tag::Byte(1)),
            ("Health".into(), Tag::Float(mob.health.to_bits())),
            ("Offers".into(), Tag::Compound(offers)),
        ]
        .into();
    }
    let id = host.summon(dimension, &format!("thmobs:{}", mob.id), at, nbt)?;
    // A sunflower fairy comes with a familiar.
    if mob.id == "sunflower_fairy"
        && let Some(familiar) = mobs::by_kind("thmobs:familiar")
    {
        let up = [at[0], at[1] + 1.8, at[2]];
        let nbt: Compound = [
            ("Silent".into(), Tag::Byte(1)),
            ("Health".into(), Tag::Float(familiar.health.to_bits())),
        ]
        .into();
        host.summon(dimension, "thmobs:familiar", up, nbt);
    }
    Some(id)
}

// ---- The client's side ----

static CLIENT: LazyLock<Mutex<HashMap<u32, Shot>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// Each model, baked once.
static BAKED: LazyLock<Mutex<HashMap<&'static str, ModelPart>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const TINT: [[u8; 3]; 8] = [
    [224, 0, 0],
    [0, 0, 224],
    [0, 224, 0],
    [224, 224, 0],
    [224, 0, 224],
    [0, 224, 224],
    [255, 128, 0],
    [255, 255, 255],
];

fn strip(form: u8) -> Option<&'static str> {
    Some(match form {
        form::SMALL | form::TINY | form::MEDIUM => "thmobs:entity/shot/small",
        form::PEARL => "thmobs:entity/shot/pearl",
        form::CIRCLE => "thmobs:entity/shot/circle",
        form::HEART => "thmobs:entity/shot/heart",
        form::TALISMAN => "thmobs:entity/shot/talisman",
        form::BIG => "thmobs:entity/shot/big",
        _ => return None,
    })
}

const LIGHT: &str = "thmobs:entity/shot/light";
/// Plain white: the classic shapes and the lasers are drawn on it, tinted.
const WHITE: &str = "thmobs:entity/shot/laser";
const GAP: &str = "thmobs:entity/shot/gap";
const SPARK: &str = "thmobs:entity/shot/spark";
/// The rainbow's order, for Fantasy Seal's orbs.
const RAINBOW: [usize; 7] = [0, 6, 3, 2, 5, 1, 4];

const DANMAKU_SPRITES: [&str; 11] = [
    LIGHT,
    WHITE,
    GAP,
    SPARK,
    "thmobs:entity/shot/small",
    "thmobs:entity/shot/pearl",
    "thmobs:entity/shot/circle",
    "thmobs:entity/shot/heart",
    "thmobs:entity/shot/talisman",
    "thmobs:entity/shot/big",
    "thmobs:entity/familiar",
];

fn pipeline_name(sprite: &str) -> mod_api::Identifier {
    mod_api::Identifier::parse(&format!(
        "thmobs_native:{}",
        sprite.rsplit('/').next().unwrap()
    ))
    .unwrap()
}

/// Submit a two-sided sprite at its world position. The old sheets use UVs in sixteenths.
fn quad(
    host: &dyn ClientHost,
    draw: &mut dyn WorldDrawContext,
    sprite: &str,
    at: V3,
    corners: [V3; 4],
    uv: [f32; 4],
    rgba: [u8; 4],
) {
    let Ok(pipeline) = host.render_assets().render_pipeline(&pipeline_name(sprite)) else {
        return;
    };
    let [u0, v0, u1, v1] = uv.map(|component| component / 16.0);
    let color = u32::from_be_bytes([rgba[3], rgba[0], rgba[1], rgba[2]]);
    let quad = mod_api::PipelineQuad::new(
        corners.map(|offset| (at + offset).array()),
        [[u0, v0], [u0, v1], [u1, v1], [u1, v0]],
        color,
    );
    let _ = draw.pipeline_quads(pipeline, &[quad]);
}

/// A kite `l` long each way along `along` and `w` wide each way along `side`.
fn kite(along: V3, side: V3, l: f64, w: f64) -> [V3; 4] {
    [along * l, side * w, -(along * l), -(side * w)]
}

/// A laser (`RenderTHLaser`): a beam from where it is along its way, thin while it waits out its
/// delay; Master Spark's wide and bright.
fn draw_laser(
    host: &dyn ClientHost,
    draw: &mut dyn WorldDrawContext,
    partial: f32,
    s: &Shot,
    right: V3,
    forward: V3,
) {
    let partial = f64::from(partial);
    let base = s.prev.lerp(s.pos, partial);
    let dir = s.angle.normalize();
    let length = f64::from(s.laser);
    if length <= 0.0 {
        return;
    }
    let mut side = cross(dir, forward);
    if side.length() < 1e-6 {
        side = right;
    }
    let side = side.normalize();
    let tint = TINT[usize::from(s.color % 8)];
    let ribbon = |h: f64| {
        [
            side * h,
            side * h + dir * length,
            -(side * h) + dir * length,
            -(side * h),
        ]
    };
    let full = [0.0, 0.0, 16.0, 16.0];
    let width = f64::from(s.size);
    if s.count < 0 {
        quad(
            host,
            draw,
            WHITE,
            base,
            ribbon(0.04),
            full,
            [tint[0], tint[1], tint[2], 150],
        );
        return;
    }
    if s.form == shot::LASER_SPARK {
        let hue = TINT[RAINBOW[(s.count / 2).rem_euclid(7) as usize]];
        quad(
            host,
            draw,
            WHITE,
            base,
            ribbon(width * 0.6),
            full,
            [hue[0], hue[1], hue[2], 110],
        );
        quad(
            host,
            draw,
            SPARK,
            base,
            ribbon(width * 0.45),
            [0.0, 8.0, 16.0, 16.0],
            [255, 255, 255, 210],
        );
        quad(
            host,
            draw,
            WHITE,
            base,
            ribbon(width * 0.22),
            full,
            [255, 255, 255, 255],
        );
    } else {
        quad(
            host,
            draw,
            WHITE,
            base,
            ribbon((width * 0.6).max(0.06)),
            full,
            [tint[0], tint[1], tint[2], 110],
        );
        quad(
            host,
            draw,
            WHITE,
            base,
            ribbon((width * 0.4).max(0.04)),
            full,
            [tint[0], tint[1], tint[2], 210],
        );
        quad(
            host,
            draw,
            WHITE,
            base,
            ribbon((width * 0.18).max(0.02)),
            full,
            [255, 255, 255, 255],
        );
    }
}

/// The classic bosses' shapes, flat and turned to the camera, pointed along their course: stars that
/// spin, butterflies, rice, scales, icicles, kunai, arrows, knives, ovals and amulets. False for a
/// form drawn the old way.
fn draw_fancy(
    host: &dyn ClientHost,
    draw: &mut dyn WorldDrawContext,
    partial: f32,
    s: &Shot,
    at: V3,
    right: V3,
    up: V3,
    forward: V3,
) -> bool {
    let tint = TINT[usize::from(s.color % 8)];
    let size = f64::from(s.size);
    let full = [0.0, 0.0, 16.0, 16.0];
    let way = if s.motion.length() > 1e-6 {
        s.motion
    } else {
        s.angle
    };
    let mut along = way - forward * dot(way, forward);
    if along.length() < 1e-6 {
        along = up;
    }
    let along = along.normalize();
    let side = cross(along, forward).normalize();
    let colour = [tint[0], tint[1], tint[2], 230];
    let white = [255, 255, 255, 255];
    match s.form {
        form::STAR | form::SMALL_STAR => {
            let spin = (f64::from(s.count) + f64::from(partial)) * 0.15;
            let big = size * 0.75;
            for (scale, rgba) in [(1.0, colour), (0.5, white)] {
                let (outer, inner) = (big * scale, big * scale * 0.42);
                for k in 0..5 {
                    let t = spin + f64::from(k) * std::f64::consts::TAU / 5.0;
                    let point = |r: f64, a: f64| (right * a.cos() + up * a.sin()) * r;
                    let half = std::f64::consts::PI / 5.0;
                    quad(
                        host,
                        draw,
                        WHITE,
                        at,
                        [
                            point(outer, t),
                            point(inner, t + half),
                            V3::ZERO,
                            point(inner, t - half),
                        ],
                        full,
                        rgba,
                    );
                }
            }
        }
        form::BUTTERFLY => {
            for turn in [55.0f64, -55.0] {
                let wing =
                    (along * turn.to_radians().cos() + side * turn.to_radians().sin()).normalize();
                let wing_side = cross(wing, forward).normalize();
                quad(
                    host,
                    draw,
                    WHITE,
                    at + wing * (size * 0.3),
                    kite(wing, wing_side, size * 0.4, size * 0.22),
                    full,
                    colour,
                );
            }
            quad(
                host,
                draw,
                WHITE,
                at,
                kite(along, side, size * 0.3, size * 0.08),
                full,
                white,
            );
        }
        form::AMULET => {
            let (l, w) = (size * 0.8, size * 0.45);
            let rect = |k: f64| {
                [
                    along * l * k + side * w * k,
                    along * l * k - side * w * k,
                    -(along * l * k) - side * w * k,
                    -(along * l * k) + side * w * k,
                ]
            };
            quad(host, draw, WHITE, at, rect(1.0), full, colour);
            quad(host, draw, WHITE, at, rect(0.55), full, white);
        }
        _ => {
            let (l, w) = match s.form {
                form::RICE => (1.0, 0.45),
                form::SCALE => (0.75, 0.55),
                form::CRYSTAL => (1.2, 0.38),
                form::KUNAI => (1.3, 0.32),
                form::ARROW => (1.5, 0.22),
                form::KNIFE => (1.5, 0.28),
                form::OVAL => (0.9, 0.6),
                _ => return false,
            };
            quad(
                host,
                draw,
                WHITE,
                at,
                kite(along, side, size * l, size * w),
                full,
                colour,
            );
            quad(
                host,
                draw,
                WHITE,
                at,
                kite(along, side, size * l * 0.55, size * w * 0.5),
                full,
                white,
            );
        }
    }
    true
}

fn draw_shot(
    host: &dyn ClientHost,
    draw: &mut dyn WorldDrawContext,
    partial: f32,
    s: &Shot,
    right: V3,
    up: V3,
    forward: V3,
) {
    if s.is_laser() {
        draw_laser(host, draw, partial, s, right, forward);
        return;
    }
    if s.form == form::HIDDEN || (21..=25).contains(&s.form) {
        return;
    }
    let at = s.prev.lerp(s.pos, f64::from(partial));
    if s.form == form::GAP {
        let h = f64::from(s.size) * 0.5;
        quad(
            host,
            draw,
            GAP,
            at,
            [
                -right * h + up * (h * 0.66),
                -right * h - up * (h * 0.66),
                right * h - up * (h * 0.66),
                right * h + up * (h * 0.66),
            ],
            [0.0, 0.0, 12.0, 16.0],
            [255, 255, 255, 255],
        );
        return;
    }
    if s.special == special::FANTASY_ORB {
        let tint = TINT[RAINBOW[(s.id % 7) as usize]];
        let h = f64::from(s.size) * 0.5;
        for (half, rgba) in [
            (h * 1.3, [tint[0], tint[1], tint[2], 170]),
            (h * 0.8, [tint[0], tint[1], tint[2], 230]),
            (h * 0.4, [255, 255, 255, 255]),
        ] {
            quad(
                host,
                draw,
                LIGHT,
                at,
                [
                    -right * half + up * half,
                    -right * half - up * half,
                    right * half - up * half,
                    right * half + up * half,
                ],
                [0.0, 0.0, 8.0, 16.0],
                rgba,
            );
        }
        return;
    }
    if s.fancy && s.count >= 0 && draw_fancy(host, draw, partial, s, at, right, up, forward) {
        return;
    }
    let color = usize::from(s.color % 8);
    let tint = TINT[color];
    let frame = f32::from(u8::from(s.count.rem_euclid(2) == 1)) * 8.0;
    let light_uv = [frame, 0.0, frame + 8.0, 16.0];
    let mut quads: Vec<(&str, [f32; 4], f64, [u8; 4])> = Vec::with_capacity(2);
    if s.count < 0 {
        let grown = (f64::from((-s.count).min(10)) * 0.3 * f64::from(s.size)).min(1.0);
        quads.push((LIGHT, light_uv, grown, [tint[0], tint[1], tint[2], 128]));
    } else if let Some(sheet) = strip(s.form) {
        let u = color as f32 * 2.0;
        quads.push((
            sheet,
            [u, 0.0, u + 2.0, 16.0],
            f64::from(s.size),
            [255, 255, 255, 255],
        ));
    } else {
        let (outer, inner) = if matches!(s.form, form::LIGHT | form::BIG_LIGHT) {
            (2.0, 1.0)
        } else {
            (1.4, 0.7)
        };
        let size = f64::from(s.size);
        quads.push((
            LIGHT,
            light_uv,
            size * outer,
            [tint[0], tint[1], tint[2], 204],
        ));
        quads.push((LIGHT, light_uv, size * inner, [255, 255, 255, 255]));
    }
    for (sprite, uv, half, rgba) in quads {
        let (r, u) = (right * half, up * half);
        quad(
            host,
            draw,
            sprite,
            at,
            [-r + u, -r - u, r - u, r + u],
            uv,
            rgba,
        );
    }
}

/// The phantoms and familiars: a phantom a column of glows rising and shrinking (`RenderTHPhantom`),
/// a familiar its picture, each turned to the camera.
fn draw_glows(
    host: &dyn ClientHost,
    draw: &mut dyn WorldDrawContext,
    view: &mod_api::WorldRenderView,
    right: V3,
    up: V3,
) {
    let partial = f64::from(view.partial_tick);
    let Some(level) = host.level(host.dimension()) else {
        return;
    };
    let radius = 128.0;
    let min = view.camera.map(|value| value - radius);
    let max = view.camera.map(|value| value + radius);
    for e in level.entities_in(host, min, max).unwrap_or_default() {
        let Some(kind) = e.kind(host).map(|kind| kind.id(host)) else {
            continue;
        };
        let Some(spec) = mobs::by_kind(&kind) else {
            continue;
        };
        let Look::Glow(sprite, size) = spec.look else {
            continue;
        };
        if host
            .living(e)
            .and_then(|mob| mob.health(host))
            .is_some_and(|health| health <= 0.0)
            || e.is_invisible(host) == Some(true)
        {
            continue;
        }
        let Some(position) = e.position(host) else {
            continue;
        };
        let at = V3::from_array(position) + v3(0.0, 0.5, 0.0);
        let tint = TINT[(e.id() % 8) as usize];
        let mut quads: Vec<(String, [f32; 4], V3, f64, [u8; 4])> = Vec::new();
        if sprite == "light" {
            let t = (view.game_time as f64 + partial) % 20.0;
            let lift = v3(0.0, 0.1 + t * 0.06, 0.0);
            let shrink = (20.0 - t) / 20.0;
            for (k, scale) in [
                (0.0, 1.0),
                (1.0, 0.9 * shrink),
                (2.0, 0.8 * shrink * 0.9 * shrink),
            ] {
                let centre = at + lift * k;
                quads.push((
                    LIGHT.to_owned(),
                    [0.0, 0.0, 8.0, 16.0],
                    centre,
                    f64::from(size) * scale * 1.4,
                    [tint[0], tint[1], tint[2], 200],
                ));
                quads.push((
                    LIGHT.to_owned(),
                    [0.0, 0.0, 8.0, 16.0],
                    centre,
                    f64::from(size) * scale * 0.7,
                    [255, 255, 255, 255],
                ));
            }
        } else {
            quads.push((
                format!("thmobs:entity/{sprite}"),
                [0.0, 0.0, 16.0, 16.0],
                at,
                f64::from(size),
                [255, 255, 255, 255],
            ));
        }
        for (name, uv, centre, half, rgba) in quads {
            let (r, u) = (right * half, up * half);
            quad(
                host,
                draw,
                &name,
                centre,
                [-r + u, -r - u, r - u, r + u],
                uv,
                rgba,
            );
        }
    }
}

fn set(root: &mut ModelPart, name: &str, f: impl Fn(&mut ModelPart)) {
    if let Some(p) = model::part_mut(root, name) {
        f(p);
    }
}

/// `ModelTHFairy.setRotationAngles` and its kin, for a fairy in the air or on the ground; for the
/// others, a head that turns, limbs that swing and wings that beat, by their parts' names.
fn pose(root: &mut ModelPart, model: &str, mob: &MobRender<'_>) {
    let tick = mob.age;
    let (yaw, pitch) = (mob.head_yaw.to_radians(), mob.pitch.to_radians());
    // The crow tengu and the hell raven: `ModelTenguCrow` and `ModelHellRaven.setRotationAngles`. The
    // head turns, the body bobs, and the two-piece wings beat, the outer piece half as far.
    if let Some(bird) = [
        ("jtg/ModelTenguCrow", "tengu"),
        ("jtg/ModelHellRaven", "raven"),
    ]
    .iter()
    .find(|(m, _)| *m == model)
    .map(|(_, b)| *b)
    {
        set(root, &format!("{bird}Head"), |p| {
            p.y_rot = yaw;
            p.x_rot = pitch;
        });
        set(root, &format!("{bird}Body"), |p| {
            p.x_rot = std::f32::consts::FRAC_PI_3 + (tick * 0.1).cos() * 0.15
        });
        let wing = (tick * 0.8).cos() * std::f32::consts::PI * 0.25;
        set(root, &format!("{bird}WingRight"), |p| p.y_rot = wing);
        set(root, &format!("{bird}WingLeft"), |p| p.y_rot = -wing);
        set(root, &format!("{bird}WingRight2"), |p| p.y_rot = wing * 0.5);
        // So the raven's was named: `ravenWingleft2`.
        for left2 in ["tenguWingLeft2", "ravenWingleft2"] {
            set(root, left2, |p| p.y_rot = -wing * 0.5);
        }
        return;
    }
    for head in ["head", "bipedHead", "Head"] {
        set(root, head, |p| {
            p.y_rot = yaw;
            p.x_rot = pitch;
        });
    }
    let flying = !mob.on_ground;
    let fairy = model.contains("Fairy");
    if fairy && flying {
        let lz = ((tick / 10.0).sin() * 0.1).abs();
        let lx = ((tick / 10.0).sin() * 0.2).abs();
        set(root, "rightLeg", |p| {
            p.z_rot = lz;
            p.x_rot = lx;
        });
        // So it was written: the left leg takes the right one's sideways swing forwards.
        set(root, "leftLeg", |p| {
            p.z_rot = -lz;
            p.x_rot = lz;
        });
        let arm = -0.7 - (tick / 10.0).sin() * 0.1;
        set(root, "rightArm", |p| {
            p.x_rot = arm;
            p.y_rot = 0.0;
            p.z_rot = -0.6457718;
        });
        set(root, "leftArm", |p| {
            p.x_rot = arm;
            p.y_rot = 0.0;
            p.z_rot = 0.6457718;
        });
    } else {
        let (m, far) = (mob.walk_position * 0.6662, mob.walk_speed);
        let swing = m.cos() * far;
        let back = (m + std::f32::consts::PI).cos() * far;
        // Swings on top of the pose the model rests in, so arms held out stay out.
        for (name, v) in [
            ("rightLeg", swing * 0.7),
            ("bipedRightLeg", swing * 1.4),
            ("leftLeg", back * 0.7),
            ("bipedLeftLeg", back * 1.4),
        ] {
            set(root, name, |p| p.x_rot += v);
        }
        for (name, v) in [
            ("rightArm", back * 0.5),
            ("bipedRightArm", back * 0.5),
            ("leftArm", swing * 0.5),
            ("bipedLeftArm", swing * 0.5),
        ] {
            set(root, name, |p| p.x_rot += v);
        }
    }
    let wing = if flying {
        (tick * 1.3).cos() * std::f32::consts::PI * 0.25
    } else {
        (tick * 0.5).cos() * std::f32::consts::PI * 0.1 + 0.47123894
    };
    for (name, sign) in [
        ("rightWing", 1.0),
        ("rightWing2", 1.0),
        ("leftWing", -1.0),
        ("leftWing2", -1.0),
        ("wingRight", 1.0),
        ("wingLeft", -1.0),
        ("RightWing", 1.0),
        ("LeftWing", -1.0),
    ] {
        set(root, name, |p| p.y_rot = wing * sign);
    }
}

/// The Yukari umbrella (Yukari Gaps): Yukari drops it 70 times in 100, the Great Doll every time.
const UMBRELLA: &str = "yukari_gap:yukari_umbrella";

/// The fairies and crows, which drop what Arcadian Dream's fairies do (and sound as they do:
/// `fairy_voice.rs`).
const FAIRIES: [&str; 9] = [
    "thmobs:fairy",
    "thmobs:sunflower_fairy",
    "thmobs:ice_fairy",
    "thmobs:nether_fairy",
    "thmobs:moon_fairy",
    "thmobs:moon_sunflower_fairy",
    "thmobs:crow_tengu",
    "thmobs:old_crow_tengu",
    "thmobs:hell_raven",
];

/// The chance, in percent, that the mob `kind` drops a Yukari umbrella when beaten.
fn umbrella_chance(kind: &str) -> u32 {
    match kind {
        GREAT_DOLL => 100,
        "thmobs:yukari" | "thmobs:classic_yukari" => 70,
        _ => 0,
    }
}

fn drop_points(host: &mut dyn ServerHost, id: u32, kind: &str) {
    let Some(entity) = host.entity(id) else {
        return;
    };
    let (Some(at), Some(level)) = (entity.position(host), entity.level(host)) else {
        return;
    };
    let dimension = level.dimension_id().to_owned();
    static LOOT_RANDOM: LazyLock<Mutex<Rng>> = LazyLock::new(|| {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |time| time.as_nanos() as u64);
        Mutex::new(Rng::new(seed))
    });
    let roll = |n: u32| LOOT_RANDOM.lock().map_or(0, |mut rng| rng.int(n));
    // Every mob, boss or not, drops what the fairies drop: Arcadian Dream's fairy loot table, 1 to 4
    // power and point items; a bomb and an extend 10% and 2.5% respectively, a star 15%, the Fairy
    // Playground disc 1%. No boss has drops of its own (only Yukari's umbrella, below).
    let mut drops = vec![
        ("arcadiandream:power_item", 1 + roll(4)),
        ("arcadiandream:point_item", 1 + roll(4)),
    ];
    if roll(10) == 0 {
        drops.push(("arcadiandream:bomb_item", 1));
    }
    if roll(40) == 0 {
        drops.push(("arcadiandream:extend_item", 1));
    }
    if roll(20) < 3 {
        drops.push(("arcadiandream:star_item", 1));
    }
    if roll(100) == 0 {
        drops.push(("arcadiandream:music_disc_fairy_playground", 1));
    }
    // The others also drop Touhou Items' Doll Points one time in 25, as before.
    if !FAIRIES.contains(&kind) && roll(25) == 0 {
        drops.push(("thkaguyamod:point_item", 1 + roll(3)));
    }
    if roll(100) < umbrella_chance(kind) {
        drops.push((UMBRELLA, 1));
    }
    for (id, count) in drops {
        let item: Compound = [
            ("id".into(), Tag::String(id.into())),
            ("count".into(), Tag::Int(count as i32)),
        ]
        .into();
        let nbt: Compound = [
            ("Item".into(), Tag::Compound(item)),
            ("PickupDelay".into(), Tag::Short(10)),
        ]
        .into();
        host.summon(
            &dimension,
            "minecraft:item",
            [at[0], at[1] + 0.5, at[2]],
            nbt,
        );
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let _ = EXPLOSION_PARTICLES.set([
            r.particle_key("minecraft:explosion"),
            r.particle_key("minecraft:explosion_emitter"),
        ]);
        for spec in mobs::MOBS {
            let egg = format!("thmobs:{}_spawn_egg", spec.id);
            let item = r.item_key(&egg);
            r.item_part(item, Egg);
        }
        for id in COPIED_FAIRIES {
            let item = r.item_key(&format!("thmobs:{id}_spawn_egg"));
            r.item_part(item, Egg);
        }
    }

    fn register_render_pipelines(
        &self,
        _: &RegistrationContext,
        event: &mut mod_api::RegisterRenderPipelinesEvent<'_>,
    ) {
        for sprite in DANMAKU_SPRITES {
            let texture = mod_api::Identifier::parse(sprite).unwrap();
            event.pipelines.push(
                mod_api::RenderPipeline::new(
                    pipeline_name(sprite),
                    mod_api::Identifier::parse("thmobs_native:danmaku").unwrap(),
                    mod_api::PipelineBlend::Translucent,
                    mod_api::WorldDepth::Tested,
                )
                .textured(mod_api::PipelineTexture::Sprite(texture)),
            );
        }
    }

    fn register_layer_definitions(
        &self,
        _: &RegistrationContext,
        event: &mut RegisterLayerDefinitionsEvent<'_>,
    ) {
        event.layers.extend(model::layers());
    }

    fn server_stopped(&self, _: &mut dyn ServerHost, _: &mut mod_api::ServerStoppedEvent) {
        if let Ok(mut levels) = POPULATIONS.lock() {
            for id in levels.values().flat_map(|level| level.mobs.keys()) {
                ai::pending(*id);
                forget(*id);
            }
            levels.clear();
        }
        if let Ok(mut server) = SERVER.lock() {
            server.shots.clear();
            server.casts.clear();
            server.stops.clear();
            server.classic.clear();
            server.halved.clear();
            server.bars.clear();
        }
    }

    fn entity_join_level(
        &self,
        _host: &mut dyn ServerHost,
        event: &mut mod_api::EntityJoinLevelEvent<'_>,
    ) -> Flow {
        let mob = mobs::by_kind(event.kind);
        let creeper = event.kind == "minecraft:creeper";
        if mob.is_some() || creeper || (ai::monster(event.kind) && !matches!(event.kind, "minecraft:enderman" | "minecraft:zombified_piglin" | "minecraft:piglin")) {
            if let Ok(mut levels) = POPULATIONS.lock() {
                let level = levels.entry(event.dimension.to_owned()).or_default();
                if let Some(mob) = mob {
                    level.mobs.insert(event.entity, mob);
                } else {
                    level.hostiles.insert(event.entity, creeper);
                }
            }
        }
        if is_ours(event.kind) {
            forget(event.entity);
            ai::pending(event.entity);
        }
        Flow::Continue
    }

    fn entity_leave_level(
        &self,
        _: &mut dyn ServerHost,
        event: &mut mod_api::EntityLeaveLevelEvent<'_>,
    ) {
        if let Ok(mut levels) = POPULATIONS.lock() {
            if let Some(level) = levels.get_mut(event.dimension) {
                level.mobs.remove(&event.entity);
                level.hostiles.remove(&event.entity);
            }
        }
        if is_ours(event.kind) {
            forget(event.entity);
            ai::pending(event.entity);
        }
    }

    /// A boss out of health goes on to its next phase, as `moveDanmakuAttack` had it: back to full
    /// health of the next, a moment unhurt, its danmaku gone. A classic boss goes on to its next
    /// card before then (`classic_cards`); this is for one felled from a card that wasn't its last
    /// in one blow, which leaves it at the start of its next.
    fn living_death(&self, host: &mut dyn ServerHost, event: &mut LivingDeathEvent<'_>) -> Flow {
        let kind = mobs::bare(event.kind);
        let Some(spec) = mobs::by_kind(kind) else {
            return Flow::Continue;
        };
        let phase = ai::phase_of(event.entity, spec);
        // `/kill` and the void end it for good, drops and all, whatever phase it is in.
        let last_hit = HITS.lock().ok().and_then(|h| h.get(&event.entity).copied());
        let killed = ends_for_good(event.source.damage_type, last_hit, now());
        let next = next_life(spec, phase, killed);
        let classic = is_classic(kind);
        if classic {
            explode(host, event.entity, next.is_none());
        }
        let Some(next) = next else {
            // Gone for good: it scatters Point items, a boss many more than a fairy.
            drop_points(host, event.entity, kind);
            forget(event.entity);
            return Flow::Continue;
        };
        begin_phase(event.entity, next);
        if let Ok(mut h) = HITS.lock() {
            h.remove(&event.entity);
        }
        // The game leaves a mob whose death was canceled at 1 health once this returns: the next
        // phase's health goes on at the next tick (`level_tick_post`).
        let hp = if classic {
            spec.top_of(spec.card_of(next.to))
        } else {
            next.hp
        };
        if let Ok(mut r) = REFILL.lock() {
            r.retain(|e| e.0 != event.entity);
            r.push((event.entity, hp, now()));
        }
        Flow::Cancel
    }

    /// A boss can't be hurt in a new phase's first moments. Each blow that lands is noted, for the
    /// death hook to tell a blow's death from a kill command's.
    fn living_incoming_damage(
        &self,
        _: &mut dyn mod_api::LevelRandomHost,
        event: &mut LivingIncomingDamageEvent<'_>,
    ) -> Flow {
        if !is_ours(mobs::bare(event.kind)) {
            return Flow::Continue;
        }
        if ai::invincible(event.entity as u32) {
            return Flow::Cancel;
        }
        ai::hurt_by(event.entity as u32, event.source.causing);
        if let Ok(mut h) = HITS.lock() {
            h.insert(event.entity as u32, now());
        }
        Flow::Continue
    }

    /// A right click: Rinnosuke trades; Miko sets the danmaku level, from easy to lunatic and round
    /// again; the others do nothing.
    fn entity_interact(
        &self,
        host: HostSide<'_>,
        event: &mut EntityInteractEvent<'_>,
    ) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let kind = mobs::bare(event.kind);
        if !is_ours(kind) {
            return None;
        }
        if kind == "thmobs:rinnosuke" {
            return None;
        }
        if kind == "thmobs:miko"
            && let Some(player) = host.players().acting_player()
        {
            static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
            let fresh = LAST.lock().map(|mut last| {
                let now = std::time::Instant::now();
                let fresh = last.is_none_or(|t| now.duration_since(t).as_millis() > 300);
                *last = Some(now);
                fresh
            });
            if fresh.unwrap_or(false) {
                let next = ai::level() % 4 + 1;
                ai::LEVEL.store(next, std::sync::atomic::Ordering::Relaxed);
                let name = ["EASY", "NORMAL", "HARD", "LUNATIC"][(next - 1) as usize];
                let _ = player.action_bar(host, Text::literal(name));
            }
        }
        Some(InteractionResult::Consume)
    }

    /// A Yukari gap used: once in 2,000, Yukari comes out of the gap after the player (a Giga Gap
    /// brings Yukari Great Doll).
    fn player_teleported(&self, host: &mut dyn ServerHost, event: &mut PlayerTeleportedEvent<'_>) {
        let Some(level) = host.level(event.dimension) else {
            return;
        };
        let to = event.to.map(|c| c.floor() as i32);
        // Yukari Gaps sets the player down one block in front of the gap's lower half.
        let mut found = Vec::new();
        for dx in -2..=2 {
            for dy in -1..=3 {
                for dz in -2..=2 {
                    let pos = [to[0] + dx, to[1] + dy, to[2] + dz];
                    if let Some(id) = block_id(host, level, pos).and_then(|b| gap_kind(&b)) {
                        found.push((pos, id));
                    }
                }
            }
        }
        let Some((gap, giga)) = arrival_gap(&found, to) else {
            return;
        };
        let roll = SERVER.lock().map_or(1, |mut s| s.rng.int(YUKARI_FROM_GAP));
        if roll != 0 {
            return;
        }
        let Some(yukari) = mobs::MOBS.iter().find(|m| m.id == "yukari") else {
            return;
        };
        let yukari = if giga || global_gap_near(host, event.dimension, gap) {
            // A Giga Gap (a gap made global with a Yukari umbrella) brings the Great Doll instead.
            let Some(doll) = mobs::by_kind(GREAT_DOLL) else {
                return;
            };
            doll
        } else {
            mobs::classic_of(yukari)
                .filter(|_| SERVER.lock().is_ok_and(|mut s| s.rng.int(CLASSIC_IN) == 0))
                .unwrap_or(yukari)
        };
        // Out of the gap itself: it has no collision, so she steps out of it behind the player.
        let at = [f64::from(gap[0]) + 0.5, f64::from(gap[1]), f64::from(gap[2]) + 0.5];
        let dimension = event.dimension.to_owned();
        summon(host, &dimension, yukari, at);
    }

    fn level_tick_post(&self, host: HostSide<'_>, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else { return };
        let dimension = event.level.dimension_id();
        if event.level.game_time(host).is_some_and(|tick| tick.rem_euclid(100) == 0) {
            despawn_wild_mobs(host, event.level, None);
        }
        let mobs: Vec<_> = tracked_mobs(dimension).into_iter()
            .filter(|(id, _)| host.entity(*id).and_then(|e| e.level(host)) == Some(event.level))
            .collect();
        ai::tick_level(host, event.level, &mobs);
        TICKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // A refill that finds no living mob in any dimension within a few ticks is dropped: its
        // mob is gone, and the id may come to another.
        if let Ok(mut r) = REFILL.lock() {
            let t = now();
            r.retain(|&(id, hp, at)| {
                !set_mob_health(host, dimension, id, hp) && t.saturating_sub(at) <= BLOW_TICKS
            });
        }
        let bosses: Vec<_> = mobs.iter().copied().filter(|(_, spec)| spec.classic()).collect();
        classic_cards(host, dimension, &bosses);
        let batch = match SERVER.lock() {
            Ok(mut server) => {
                server.spawn(host, dimension, &mobs);
                let batch = server.tick(host, dimension, &mobs);
                server.boss_bars(host, dimension, &bosses);
                batch
            }
            Err(_) => return,
        };
        if batch.fired.is_empty() && batch.gone.is_empty() {
            return;
        }
        let messages = batch.encode(30_000);
        for p in players(host, dimension) {
            if p.dimension == dimension {
                for m in &messages {
                    send_shots(host, p.handle, m);
                }
            }
        }
    }

    fn client_packet(&self, _host: &mut dyn ClientHost, event: &mut ClientPacketEvent<'_>) {
        let Some(payload) = &event.packet.custom_payload else {
            return;
        };
        if payload.channel != CHANNEL {
            return;
        }
        let batch = Batch::decode(&payload.data);
        if let Ok(mut shots) = CLIENT.lock() {
            for id in batch.gone {
                shots.remove(&id);
            }
            for s in batch.fired {
                shots.insert(s.id, s);
            }
        }
    }

    fn client_tick_post(&self, _host: &mut dyn ClientHost, _event: &mut ClientTickPostEvent) {
        if let Ok(mut shots) = CLIENT.lock() {
            shots.retain(|_, s| s.step());
        }
    }

    fn client_level_unload(
        &self,
        _host: &mut dyn ClientHost,
        _event: &mut ClientLevelUnloadEvent<'_>,
    ) {
        if let Ok(mut shots) = CLIENT.lock() {
            shots.clear();
        }
    }

    fn submit_custom_geometry(
        &self,
        host: &dyn ClientHost,
        event: &mut SubmitCustomGeometryEvent<'_>,
    ) {
        let Ok(shots) = CLIENT.lock() else { return };
        let Some(camera) = host.camera() else { return };
        let yaw = f64::from(camera.yaw).to_radians();
        let forward = angle(camera.yaw, camera.pitch);
        let right = v3(-yaw.cos(), 0.0, -yaw.sin());
        let up = vec::cross(right, forward);
        for s in shots.values() {
            draw_shot(
                host,
                event.draw,
                event.view.partial_tick,
                s,
                right,
                up,
                forward,
            );
        }
        draw_glows(host, event.draw, event.view, right, up);
    }

    /// Ours are drawn with their own models and textures, posed as the original posed them; the
    /// phantoms and familiars as a glow facing the camera.
    fn render_living_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut RenderLivingPreEvent<'_>,
    ) -> Flow {
        let Some(spec) = mobs::by_kind(event.entity.kind) else {
            return Flow::Continue;
        };
        match spec.look {
            Look::Model(model, _) => {
                let Ok(mut baked) = BAKED.lock() else {
                    return Flow::Continue;
                };
                if !baked.contains_key(model) {
                    let Some(part) = host.bake_layer(&model::layer_id(model)) else {
                        return Flow::Continue;
                    };
                    baked.insert(model, part);
                }
                let Some(root) = baked.get_mut(model) else {
                    return Flow::Continue;
                };
                root.reset_all_poses();
                pose(root, model, event.entity);
                let texture = mobs::texture_of(spec, event.entity.id);
                // The Great Doll is half her size.
                let size = if spec.id == "yukari_great_doll" {
                    0.5
                } else {
                    1.0
                };
                let draw = &mut *event.draw;
                draw.push();
                draw.scale([size, -size, -size]);
                draw.translate([0.0, -1.501, 0.0]);
                draw.model_part(root, &format!("thmobs:entity/{texture}"), 0xFFFF_FFFF);
                draw.pop();
            }
            Look::Vanilla(layer, sprite) => {
                let Ok(mut baked) = BAKED.lock() else {
                    return Flow::Continue;
                };
                if !baked.contains_key(layer) {
                    let Some(part) = host.bake_layer(layer) else {
                        return Flow::Continue;
                    };
                    baked.insert(layer, part);
                }
                let Some(root) = baked.get_mut(layer) else {
                    return Flow::Continue;
                };
                root.reset_all_poses();
                set(root, "head", |p| {
                    p.y_rot = event.entity.head_yaw.to_radians();
                    p.x_rot = event.entity.pitch.to_radians();
                });
                let swing =
                    (event.entity.walk_position * 0.6662).cos() * 1.4 * event.entity.walk_speed;
                for (name, sign) in [
                    ("right_hind_leg", 1.0),
                    ("left_front_leg", 1.0),
                    ("left_hind_leg", -1.0),
                    ("right_front_leg", -1.0),
                ] {
                    set(root, name, |p| p.x_rot = swing * sign);
                }
                let draw = &mut *event.draw;
                draw.push();
                draw.scale([1.0, -1.0, -1.0]);
                draw.translate([0.0, -1.501, 0.0]);
                draw.model_part(root, sprite, 0xFFFF_FFFF);
                draw.pop();
            }
            // Drawn with the danmaku in `world_submit`, facing the camera; nothing of the trader here.
            Look::Glow(..) => {}
        }
        Flow::Cancel
    }
}

fn send_shots(host: &mut dyn ServerHost, player: PlayerRef, message: &[u8]) {
    let mut body = Vec::with_capacity(1 + CHANNEL.len() + message.len());
    body.push(CHANNEL.len() as u8);
    body.extend(CHANNEL.as_bytes());
    body.extend(message);
    let packet = mod_api::Identifier::parse("minecraft:custom_payload").unwrap();
    let _ = host.player_send_packet(player, mod_api::ClientboundPlayPacket::new(packet, body));
}

// These use Arcadian Dream's fairy AI, animation and danmaku.
const COPIED_FAIRIES: [&str; 2] = ["woodland_fairy", "frost_fairy"];

struct Egg;

impl ItemPart for Egg {
    fn item_subscribed(&self) -> &'static [Option<mod_api::HookId>] {
        &[Some(mod_api::HookId::UseOn)]
    }

    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let item = event.stack.item().id(host);
        let id = item.strip_prefix("thmobs:")?.strip_suffix("_spawn_egg")?;
        let player = host.player(event.player)?;
        let level = player.level(host)?;
        let at = [
            f64::from(event.pos[0] + event.face[0]) + 0.5,
            f64::from(event.pos[1] + event.face[1]),
            f64::from(event.pos[2] + event.face[2]) + 0.5,
        ];
        let spawned = if COPIED_FAIRIES.contains(&id) {
            host.summon(
                level.dimension_id(),
                &format!("thmobs:{id}"),
                at,
                [("NoAI".into(), Tag::Byte(1)), ("NoGravity".into(), Tag::Byte(1))].into(),
            ).is_some()
        } else {
            let mob = mobs::MOBS.iter().find(|spec| spec.id == id)?;
            summon(host, level.dimension_id(), mob, at).is_some()
        };
        if !spawned {
            return Some(InteractionResult::Consume);
        }
        if player.has_infinite_materials(host) != Some(true) {
            let place = ItemPlace::PlayerHand {
                player: event.player,
                hand: event.hand,
            };
            let resource = ItemResource::of(event.stack);
            let _ = host.transaction(|host, tx| Ok::<_, ()>(place.extract(host, tx, &resource, 1)));
        }
        Some(InteractionResult::SuccessServer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_is_day_and_night_is_night() {
        assert!(daytime(1000));
        assert!(daytime(23_800));
        assert!(!daytime(18_000));
    }

    #[test]
    fn yukari_comes_out_of_the_lower_half_of_the_gap_arrived_by() {
        assert_eq!(gap_kind("yukari_gap:yukari_gap"), Some(false));
        assert_eq!(gap_kind("yukari_gap:giga_gap[half=upper]"), Some(true));
        assert_eq!(gap_kind("yukari_gap:yukari_umbrella"), None);
        // The player stands at (11, 64, 10), one block east of a Giga Gap at (10, 64, 10); another
        // gap is farther off.
        let found = [([10, 65, 10], true), ([10, 64, 10], true), ([13, 64, 12], false), ([13, 65, 12], false)];
        assert_eq!(arrival_gap(&found, [11, 64, 10]), Some(([10, 64, 10], true)));
        assert_eq!(arrival_gap(&[], [11, 64, 10]), None);
    }

    #[test]
    fn only_global_gaps_by_the_arrival_bring_the_great_doll() {
        let o = "0".repeat(32);
        let text = format!(
            "H\tminecraft:overworld\t10\t64\t10\teast\t{o}\t0\t1\t\t\tHome\nH\tminecraft:overworld\t50\t64\t50\teast\t{o}\t0\t0\t\t\tAway\n"
        );
        assert!(global_gap_in(&text, "minecraft:overworld", [11, 64, 10]));
        assert!(!global_gap_in(&text, "minecraft:the_nether", [11, 64, 10]));
        assert!(!global_gap_in(&text, "minecraft:overworld", [51, 64, 50]));
        assert!(!global_gap_in(&text, "minecraft:overworld", [30, 64, 30]));
        assert!(is_classic(GREAT_DOLL));
        assert_eq!(umbrella_chance(GREAT_DOLL), 100);
        assert_eq!(umbrella_chance("thmobs:yukari"), 70);
        assert_eq!(umbrella_chance("thmobs:reimu"), 0);
        for f in FAIRIES {
            assert!(mobs::by_kind(f).is_some(), "{f}");
        }
    }

    #[test]
    fn only_our_kinds_are_ours() {
        assert!(is_ours("thmobs:fairy"));
        assert!(!is_ours("minecraft:wandering_trader"));
    }

    /// `mobs.json` beside the crate: what `gen_content.py` needs of each mob.
    #[test]
    fn write_the_manifest() {
        let mut rows = Vec::new();
        for m in mobs::MOBS {
            let (template, look) = match m.look {
                Look::Model(model, textures) => (
                    "minecraft:wandering_trader",
                    format!("{{\"model\": \"{model}\", \"textures\": {textures:?}}}"),
                ),
                Look::Glow(sprite, _) => (
                    "minecraft:wandering_trader",
                    format!("{{\"glow\": \"{sprite}\"}}"),
                ),
                Look::Vanilla(..) => ("minecraft:wandering_trader", "{}".to_owned()),
            };
            rows.push(format!(
                "{{\"id\": \"{}\", \"name\": \"{}\", \"template\": \"{template}\", \"max_health\": {}, \"speed\": {}, \"look\": {look}, \"egg\": [{}, {}]}}",
                m.id, m.name, m.max_health(), m.ai.speed.min(1.0), m.egg.0, m.egg.1
            ));
        }
        let json = format!("[\n  {}\n]\n", rows.join(",\n  "));
        std::fs::write(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../mobs.json"),
            json,
        )
        .unwrap();
    }

    /// Each classic card with what its shots fire in turn (the server's bursts), for 300 ticks: no
    /// pattern runs away, and every shot stays a real place, way and laser.
    #[test]
    fn classic_cards_and_their_bursts_stay_in_bounds() {
        for card in cards::CLASSIC {
            let user = Actor {
                pos: v3(0.0, 64.0, 0.0),
                eye_height: 1.62,
                yaw: 10.0,
                pitch: 0.0,
            };
            let target = v3(4.0, 65.0, 14.0);
            let mut rng = Rng::new(11);
            let mut live: Vec<Shot> = Vec::new();
            let mut state = [0.0; 2];
            let mut most = 0;
            for time in 0..300 {
                if time < card.info().end {
                    let ctx = Ctx {
                        time,
                        level: 2,
                        user,
                        target,
                        target_alive: true,
                        card: card_position(&user),
                        card_yaw: user.yaw,
                        card_pitch: user.pitch,
                        user_height: 1.8,
                    };
                    card.tick(
                        &ctx,
                        &mut state,
                        &mut Fire {
                            out: &mut live,
                            rng: &mut rng,
                        },
                        &mut Vec::new(),
                    );
                }
                let mut next = Vec::with_capacity(live.len());
                for mut s in live.drain(..) {
                    if !s.step() {
                        continue;
                    }
                    next.extend(bursts(&s, Some(user.look()), Some(target), true, &mut rng));
                    next.push(s);
                }
                live = next;
                most = most.max(live.len());
                assert!(
                    live.iter().all(|s| s.pos.length().is_finite()
                        && s.angle.length().is_finite()
                        && s.laser.is_finite()),
                    "{card:?} at {time}"
                );
            }
            assert!(most < 8000, "{card:?}: {most} shots at once");
        }
    }

    /// A blow just before a death ends a phase; `/kill`, which deals none (and whose death names the
    /// last real blow), the void, and a death long after any blow end the mob.
    #[test]
    fn only_a_blow_just_before_a_death_ends_a_phase() {
        assert!(!ends_for_good("minecraft:player_attack", Some(100), 100));
        assert!(!ends_for_good(
            "minecraft:player_attack",
            Some(100),
            100 + BLOW_TICKS
        ));
        assert!(ends_for_good(
            "minecraft:player_attack",
            Some(100),
            101 + BLOW_TICKS
        ));
        assert!(ends_for_good("minecraft:player_attack", None, 100));
        assert!(ends_for_good("minecraft:generic_kill", Some(100), 100));
        assert!(ends_for_good("minecraft:out_of_world", Some(100), 100));
    }

    /// The basic bosses die on their first death, with their strongest phase's health (Yukari
    /// 300); the classic ones go on through their cards.
    #[test]
    fn only_classic_bosses_come_back() {
        for m in mobs::MOBS.iter().filter(|m| !m.phases.is_empty()) {
            let first = mobs::start_of(m, 1);
            if m.classic() {
                assert!(next_life(m, first, false).is_some(), "{}", m.id);
                assert!(next_life(m, first, true).is_none(), "{}", m.id);
            } else {
                assert!(next_life(m, first, false).is_none(), "{}", m.id);
                assert!(m.max_health() >= m.health, "{}", m.id);
            }
        }
        let yukari = mobs::MOBS.iter().find(|m| m.id == "yukari").unwrap();
        assert_eq!(yukari.max_health(), 300.0);
    }

    /// A mob gone (or a new one with its id) leaves no phase, time unhurt or refill behind.
    #[test]
    fn a_forgotten_id_is_no_boss_and_not_invincible() {
        let id = 987_654;
        ai::BOSSES.lock().unwrap().insert(
            id,
            ai::Boss {
                phase: 3,
                counter: None,
                invincible: 160,
                ai: None,
            },
        );
        HITS.lock().unwrap().insert(id, 5);
        REFILL.lock().unwrap().push((id, 100.0, 5));
        assert!(ai::invincible(id));
        forget(id);
        assert!(!ai::invincible(id));
        assert!(!HITS.lock().unwrap().contains_key(&id));
        assert!(REFILL.lock().unwrap().iter().all(|e| e.0 != id));
    }

    #[test]
    fn every_mob_attacks_without_trouble() {
        for spec in mobs::MOBS {
            for &phase in spec.start.iter().chain(spec.phases.iter().map(|p| &p.to)) {
                let mut out = Vec::new();
                let mut rng = Rng::new(5);
                let mut mem = [0; 4];
                for c in 0..300 {
                    let mut a = attack::Atk {
                        c,
                        level: ai::level(),
                        phase,
                        pos: v3(0.0, 65.0, 0.0),
                        feet: v3(0.0, 64.0, 0.0),
                        yaw: 30.0,
                        pitch: 5.0,
                        ticks: c,
                        target: Some(v3(0.0, 65.0, 10.0)),
                        fire: Fire {
                            out: &mut out,
                            rng: &mut rng,
                        },
                        span: 30,
                        pattern: mobs::FAIRY_PATTERNS[1][c as usize % 9],
                        glide: None,
                        stop: None,
                        card: None,
                        sound: false,
                        wet: c % 2 == 0,
                        mem: &mut mem,
                    };
                    (spec.attack)(&mut a);
                }
                assert!(
                    out.iter()
                        .all(|s| s.pos.length().is_finite() && s.motion.length().is_finite()),
                    "{} {phase}",
                    spec.id
                );
            }
        }
    }
}
