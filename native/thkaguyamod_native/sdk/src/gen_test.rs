//! Every translated card cast from start to end against a stand-in world, its shots moved and made
//! to hit, to find any that panic.

use crate::gen_cards;
use crate::gen_info::{GEN, GenInfo};
use crate::rt::{self, Cx, EH, Ent, Hit, SH, World};
use crate::shot::{Phase, Shot, laser};
use crate::vec::v3;
use mod_api::prelude::EntitiesHost;
use mod_api::*;

const DIM: &str = "minecraft:overworld";

/// A flat world of stone below y 60 holding a player (1) and a zombie (2).
struct Fake {
    registries: testing::FakeHost,
    commands: testing::FakeCommands,
    players: testing::FakePlayers,
    messages: testing::FakeMessages,
}

impl Fake {
    fn new() -> Self {
        let mut registries = testing::FakeHost::new();
        registries.level.entity(1, "minecraft:player");
        registries.level.entity(2, "minecraft:zombie");
        registries.resolve_entity_types().unwrap();
        Self {
            registries,
            commands: Default::default(),
            players: Default::default(),
            messages: Default::default(),
        }
    }
}

impl Host for Fake {
    fn backend(&self) -> Option<BackendRef<'_>> {
        Some(mod_api::__private::backend_ref(self))
    }
}
impl ServerHost for Fake {
    fn backend_mut(&mut self) -> Option<BackendMutRef<'_>> {
        Some(<BackendMutRef<'_> as mod_api::__private::game::BackendMutRefOf>::of(self))
    }
}
impl Backend for Fake {}
impl BackendMut for Fake {}
mod_api::impl_host_areas!(Fake, except(entities, living));
mod_api::impl_server_host_areas!(Fake, except(services));
mod_api::impl_backend_areas!(Fake, except(entities, levels, keys, living));
mod_api::impl_backend_mut_areas!(Fake, except(entities, levels));

impl mod_api::prelude::EntitiesHost for Fake {
    fn level(&self, id: &str) -> Option<LevelRef> {
        self.registries.level(id)
    }
    fn entity(&self, id: u32) -> Option<EntityRef> {
        self.registries.entity(id)
    }
}
impl mod_api::prelude::LivingHost for Fake {
    fn living(&self, entity: EntityRef) -> Option<LivingRef> {
        Some(LivingRef::new(entity))
    }
}
impl mod_api::prelude::ServicesServerHost for Fake {
    fn commands(&mut self) -> &mut dyn Commands {
        &mut self.commands
    }
    fn players(&self) -> &dyn Players {
        &self.players
    }
    fn messages(&self) -> &dyn Messages {
        &self.messages
    }
}
impl mod_api::prelude::EntitiesBackend for Fake {
    fn entity_kind(&self, e: EntityRef) -> Option<EntityTypeKey> {
        e.kind(&self.registries)
    }
    fn entity_level(&self, e: EntityRef) -> Option<LevelRef> {
        e.level(&self.registries)
    }
    fn entity_position(&self, e: EntityRef) -> Option<[f64; 3]> {
        Some(if e.id() == 1 {
            [0.0, 60.0, 0.0]
        } else {
            [0.0, 60.0, 8.0]
        })
    }
    fn entity_rotation(&self, _: EntityRef) -> Option<[f32; 2]> {
        Some([0.0; 2])
    }
    fn entity_bounding_box(&self, e: EntityRef) -> Option<[[f64; 3]; 2]> {
        let p = self.entity_position(e)?;
        Some([
            [p[0] - 0.3, p[1], p[2] - 0.3],
            [p[0] + 0.3, p[1] + 1.8, p[2] + 0.3],
        ])
    }
    fn entity_velocity(&self, _: EntityRef) -> Option<[f64; 3]> {
        Some([0.0; 3])
    }
    fn entity_eye_height(&self, _: EntityRef) -> Option<f64> {
        Some(1.62)
    }
    fn entity_is_in_water(&self, _: EntityRef) -> Option<bool> {
        Some(false)
    }
    fn entity_is_crouching(&self, _: EntityRef) -> Option<bool> {
        Some(false)
    }
    fn entity_on_ground(&self, _: EntityRef) -> Option<bool> {
        Some(true)
    }
}
impl mod_api::prelude::LivingClientBackend for Fake {}
impl mod_api::prelude::LivingBackend for Fake {
    fn living_health(&self, _: LivingRef) -> Option<f32> {
        Some(20.0)
    }
    fn living_max_health(&self, _: LivingRef) -> Option<f32> {
        Some(20.0)
    }
}
impl mod_api::prelude::KeysBackend for Fake {
    fn entity_type_key_id(&self, key: EntityTypeKey) -> String {
        key.id(&self.registries)
    }
}
impl mod_api::prelude::LevelsBackend for Fake {
    fn level_all_entities(&self, _: LevelRef) -> Option<Vec<EntityRef>> {
        Some(
            [1, 2]
                .into_iter()
                .filter_map(|id| self.entity(id))
                .collect(),
        )
    }
    fn level_block(&self, level: LevelRef, pos: [i32; 3]) -> Option<BlockKey> {
        level.block(&self.registries, pos)
    }
}
impl mod_api::prelude::EntitiesBackendMut for Fake {
    fn entity_uuid(&mut self, e: EntityRef) -> Option<u128> {
        Some(u128::from(e.id()))
    }
    fn entity_hurt(
        &mut self,
        _: EntityRef,
        _: &str,
        _: f32,
        _: Option<EntityRef>,
        _: Option<EntityRef>,
    ) -> Option<bool> {
        Some(true)
    }
    fn entity_push(&mut self, _: EntityRef, _: [f64; 3]) -> bool {
        true
    }
    fn entity_set_velocity(&mut self, _: EntityRef, _: [f64; 3]) -> bool {
        true
    }
    fn entity_teleport_to(&mut self, _: EntityRef, _: [f64; 3], _: Option<[f32; 2]>) -> bool {
        true
    }
}
impl mod_api::prelude::LevelsBackendMut for Fake {
    fn level_play_sound(
        &mut self,
        _: LevelRef,
        _: &str,
        _: [f64; 3],
        _: SoundSource,
        _: f32,
        _: f32,
    ) -> bool {
        true
    }
}

/// Casts `g` at level `level` and runs on until its shots are gone (or a while after). Returns how
/// many shots it had at most.
fn cast(g: &'static GenInfo, level: i32, seed: u64) -> usize {
    let mut host = Fake::new();
    let user = Ent {
        id: 1,
        present: true,
        x: 0.0,
        y: 60.0,
        z: 0.0,
        eye: 1.62,
        height: 1.8,
        width: 0.6,
        health: 20.0,
        max_health: 20.0,
        living: true,
        player: true,
        on_ground: true,
        ..Ent::NONE
    };
    let mut shots: Vec<Shot> = Vec::new();
    let mut next_id = 1;
    let mut state = None;
    let mut end = g.end;
    let mut most = 0;
    let mut t = 0;
    while t < end.min(3000) + 400 {
        let mut w = World::new(&mut host, DIM, shots, next_id);
        let tick_seed = seed ^ (t as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        for i in 0..w.shots.len() {
            if w.shots[i].frozen > 0 {
                w.shots[i].frozen -= 1;
                continue;
            }
            match w.shots[i].begin() {
                Phase::Gone => w.shots[i].dead = true,
                Phase::Waiting => {}
                Phase::Moving => {
                    let (sp, owner) = (w.shots[i].special, w.shots[i].owner);
                    if gen_cards::owns_special(sp) || sp == rt::special::MUSOU {
                        let owner = w.ent(owner);
                        let mut cx = Cx::new(
                            &mut w,
                            owner,
                            Ent::NONE,
                            v3(0.0, 0.0, 0.0),
                            tick_seed ^ i as u64,
                        );
                        if sp == rt::special::MUSOU {
                            rt::musou(&mut cx, SH::I(i));
                        } else {
                            gen_cards::special_move(&mut cx, sp, SH::I(i));
                        }
                    } else {
                        w.shots[i].special_motion();
                    }
                    if !w.shots[i].dead {
                        w.shots[i].finish();
                    }
                }
            }
            if w.shots[i].laser == laser::SET
                && !w.shots[i].dead
                && rt::anchor_pos(&w, w.shots[i].anchor).is_none()
            {
                w.shots[i].dead = true;
            }
        }
        // Every few ticks each scripted shot strikes the zombie and the ground.
        if t % 5 == 0 {
            for i in 0..w.shots.len() {
                let s = w.shots[i].clone();
                if s.dead || !gen_cards::owns_special(s.special) {
                    continue;
                }
                let owner = w.ent(s.owner);
                let zombie = w.ent(2);
                let mut cx = Cx::new(
                    &mut w,
                    owner,
                    Ent::NONE,
                    v3(0.0, 0.0, 0.0),
                    tick_seed ^ i as u64,
                );
                let keep = if (t / 5 + i as i32) % 2 == 0 {
                    gen_cards::special_hit_entity(&mut cx, s.special, SH::I(i), EH::Ent(zombie))
                } else {
                    let h = Hit {
                        present: true,
                        entity: EH::None,
                        side: 1,
                        at: s.pos,
                        bx: s.pos.x.floor() as i32,
                        by: 59,
                        bz: s.pos.z.floor() as i32,
                    };
                    gen_cards::special_hit_block(&mut cx, s.special, SH::I(i), h)
                };
                if keep == Some(false) {
                    w.shots[i].dead = true;
                }
            }
        }
        if t < end {
            let target = w.ent(2);
            let mut cx = Cx::new(&mut w, user, target, v3(0.0, 61.0, 1.5), tick_seed);
            cx.time = t;
            cx.level = level;
            cx.end = end;
            cx.need = g.need;
            cx.number = g.number;
            cx.by_player = true;
            if state.is_none() {
                state = gen_cards::start(&mut cx, g.class);
                assert!(state.is_some(), "{} has no script", g.class);
            }
            gen_cards::tick(&mut cx, state.as_mut().unwrap());
            if let Some(e) = cx.new_end {
                end = e;
            }
        }
        rt::tick_helpers(&mut w, tick_seed);
        let (s, n) = w.finish();
        shots = s.into_iter().filter(|s| !s.dead).collect();
        next_id = n;
        rt::sweep_helpers();
        most = most.max(shots.len());
        assert!(
            shots.len() < 20_000,
            "{} made {} shots",
            g.class,
            shots.len()
        );
        for s in &shots {
            assert!(
                s.pos.x.is_finite() && s.pos.y.is_finite() && s.pos.z.is_finite(),
                "{} put a shot at {:?}",
                g.class,
                s.pos
            );
        }
        if t >= end && shots.is_empty() {
            break;
        }
        t += 1;
    }
    most
}

#[test]
fn every_translated_card_runs() {
    let mut failed = Vec::new();
    for g in GEN {
        for (level, seed) in [(1, 7), (2, 99), (4, 12345)] {
            let r = std::panic::catch_unwind(|| cast(g, level, seed));
            match r {
                Ok(n) if level == 2 => println!("{:<28} {:>5} shots at most", g.class, n),
                Ok(_) => {}
                Err(e) => {
                    let why = e
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_default();
                    failed.push(format!("{} (level {level}): {why}", g.class));
                    break;
                }
            }
        }
    }
    assert!(
        failed.is_empty(),
        "{} cards failed:\n{}",
        failed.len(),
        failed.join("\n")
    );
}
