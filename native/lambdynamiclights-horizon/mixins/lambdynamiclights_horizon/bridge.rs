// SPDX-License-Identifier: MIT
// Renderer-only integration. No entity simulation, block writes or network messages.
use super::{Game, lambdynamiclights as policy, shield};
use glam::DVec3;
use horizon_client::dynamic_light::Source;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    time::Instant,
};

type Light = (Source, Option<String>);
struct State {
    root: PathBuf,
    assets: usize,
    settings: policy::Settings,
    rules: policy::Rules,
    world: String,
    updated: Option<Instant>,
    frame: Option<u64>,
    sources: Vec<Light>,
}
thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

pub fn invalidate() {
    STATE.with_borrow_mut(|s| *s = None);
}

fn documents(dir: &Path) -> Vec<PathBuf> {
    // Resource roots have already been safely overlaid by Horizon. Read only direct JSON
    // files; skip symlinks and keep file order deterministic within each namespace.
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|t| t.is_file())
                && entry.path().extension().is_some_and(|e| e == "json")
            {
                out.push(entry.path());
            }
        }
    }
    out.sort();
    out
}
fn load_rules(root: &Path) -> policy::Rules {
    let mut rules = policy::Rules::default();
    for bytes in policy::ITEMS {
        rules.item_document(bytes);
    }
    // Overlaid resource packs replace a file of the same path. Namespaces are then sorted,
    // with last matching rule winning, as documented for this port.
    let mut namespaces = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root.join("assets")) {
        namespaces.extend(
            entries
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.path()),
        );
    }
    namespaces.sort();
    for namespace in namespaces {
        for (folder, entity) in [("item", false), ("entity", true)] {
            for path in documents(&namespace.join("dynamiclights").join(folder)) {
                // Bound pack input; no I/O in subsequent render frames.
                let Ok(meta) = std::fs::metadata(&path) else {
                    continue;
                };
                if meta.len() > 65_536 {
                    continue;
                }
                if let Ok(bytes) = std::fs::read(path) {
                    if entity {
                        rules.entity_document(&bytes);
                    } else {
                        rules.item_document(&bytes);
                    }
                }
            }
        }
    }
    rules
}

pub fn sources(game: &Game<'_>, alpha: f64) -> Vec<Light> {
    STATE.with_borrow_mut(|slot| {
        let assets = std::sync::Arc::as_ptr(&game.pack.assets) as usize;
        let needs_load = slot
            .as_ref()
            .is_none_or(|s| s.root != game.options.resources || s.assets != assets);
        if needs_load {
            *slot = Some(State {
                root: game.options.resources.clone(),
                assets,
                settings: policy::Settings::read(|k| game.native_setting(policy::ID, k)),
                rules: load_rules(&game.options.resources),
                world: String::new(),
                updated: None,
                frame: None,
                sources: Vec::new(),
            });
        }
        let s = slot.as_mut().unwrap();
        if s.settings.mode == policy::Mode::Off {
            s.sources.clear();
            s.updated = None;
            return Vec::new();
        }
        let world = format!("{}:{}", game.world_root.display(), game.dim.id());
        let now = Instant::now();
        if s.world != world
            || (s.frame != Some(game.frames)
                && s.updated
                    .is_none_or(|t| now.duration_since(t) >= s.settings.mode.delay()))
        {
            s.sources = gather(game, alpha.clamp(0.0, 1.0), &s.settings, &s.rules);
            s.world = world;
            s.updated = Some(now);
            s.frame = Some(game.frames);
        }
        s.sources.clone()
    })
}

fn gather(
    game: &Game<'_>,
    alpha: f64,
    settings: &policy::Settings,
    rules: &policy::Rules,
) -> Vec<Light> {
    let mut out = Vec::new();
    let block = |id: &str| {
        game.pack
            .assets
            .registry
            .default_state(id)
            .and_then(|state| game.res.facts().states.get(state.0 as usize))
            .map_or(0, |f| f.emission)
    };
    let item = |id: &str, wet: bool| rules.item(id, wet, settings.water, &block);
    let equipment = |ids: &[Option<&str>], wet: bool| -> (u8, Option<String>) {
        let (level, id) = ids
            .iter()
            .flatten()
            .map(|id| (item(id, wet), *id))
            .max_by_key(|p| p.0)
            .unwrap_or((0, ""));
        (level, (level > 0).then(|| id.to_owned()))
    };
    let wet_at = |p: DVec3| {
        let pos = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
        let state = game.world.block(pos[0], pos[1], pos[2]);
        game.res
            .facts()
            .states
            .get(state.0 as usize)
            .is_some_and(|f| f.name.as_ref() == "minecraft:water" || f.waterlogged)
    };
    let mut push = |at: DVec3, natural: u8, equip: (u8, Option<String>), burning: bool| {
        let level = policy::combine(natural, equip.0, burning);
        if level > 0 && at.is_finite() {
            out.push((
                Source { at, level },
                if equip.0 == level { equip.1 } else { None },
            ));
        }
    };
    if settings.own
        && let Some(p) = &game.player
        && !p.spectator
    {
        let feet = game
            .previous_position
            .lerp(p.position, game.clock.camera_fraction());
        let at = DVec3::new(
            feet.x,
            feet.y + f64::from(p.dimensions.eye_height) * 0.75,
            feet.z,
        );
        push(
            at,
            0,
            equipment(
                &[
                    game.stack_in(game.selected).map(|s| &*s.item),
                    game.stack_in(shield::OFFHAND_SLOT).map(|s| &*s.item),
                    game.stack_in(39).map(|s| &*s.item),
                ],
                p.is_under_water(),
            ),
            p.fire_ticks > 0,
        );
    }
    if settings.entities {
        for m in game.mobs.iter().filter(|m| !m.removed && m.health > 0.0) {
            let pos = m.previous.lerp(m.position, alpha);
            let at = DVec3::new(pos.x, pos.y + f64::from(m.height()) * 0.6, pos.z);
            let wet = m.fluids.in_water;
            let natural = match m.kind.id.as_str() {
                "minecraft:blaze" => {
                    if wet {
                        4
                    } else {
                        10
                    }
                }
                "minecraft:allay" => 8,
                "minecraft:magma_cube" => {
                    if m.anim.lerp(0, alpha as f32) > 0.6 {
                        11
                    } else {
                        8
                    }
                }
                "minecraft:glow_squid" => {
                    policy::glow_squid(horizon_client::mobs::aquatic::dark_ticks(m))
                }
                "minecraft:creeper" => {
                    let swell = m.previous_swell as f32
                        + (m.swell - m.previous_swell) as f32 * alpha as f32;
                    let progress = swell / m.max_swell.max(1) as f32;
                    settings
                        .creeper
                        .level(progress, (progress * 10.0) as i32 % 2 != 0)
                }
                "minecraft:enderman" => m
                    .extra
                    .carried
                    .and_then(|s| game.res.facts().states.get(s.0 as usize))
                    .map_or(0, |f| f.emission),
                _ => 0,
            };
            push(
                at,
                rules.entity(&m.kind.id, natural),
                equipment(
                    &[
                        m.extra.main_hand.as_deref(),
                        m.extra.off_hand.as_deref(),
                        m.extra.head.as_deref(),
                    ],
                    wet,
                ),
                m.fire_ticks > 0,
            );
        }
        // On remote servers, player models live in `mobs`. Hosted LAN guests are kept by
        // the hub instead. Never traverse unplaced, dead or other-dimension guests.
        if let Some(host) = &game.lan {
            for g in
                host.hub.guests.iter().filter(|g| {
                    g.placed && !g.dead && g.game_mode != 3 && g.dimension == game.dim.id()
                })
            {
                let p = DVec3::from_array(std::array::from_fn(|i| {
                    g.previous_position[i] + (g.position[i] - g.previous_position[i]) * alpha
                }));
                let stack = |slot: usize| {
                    g.inventory
                        .get(slot)
                        .and_then(Option::as_ref)
                        .map(|s| &*s.item)
                };
                push(
                    p + DVec3::Y * 1.2,
                    0,
                    equipment(
                        &[stack(g.selected), stack(40), stack(39)],
                        wet_at(p + DVec3::Y * 1.6),
                    ),
                    g.motion.as_ref().is_some_and(|m| m.fire_ticks > 0),
                );
            }
        }
        for d in &game.dropped {
            let p = d.previous.lerp(d.position, alpha);
            let at = DVec3::new(p.x, p.y + 0.25, p.z);
            push(
                at,
                0,
                equipment(&[Some(&d.stack.item)], wet_at(at)),
                d.fire_ticks > 0,
            );
        }
        for p in &game.creatures.projectiles {
            let pos = p.previous.lerp(p.position, alpha);
            let at = DVec3::new(pos.x, pos.y, pos.z);
            let natural = match p.kind.id() {
                "minecraft:fireball" if !p.touching_water => 14,
                "minecraft:small_fireball" if !p.touching_water => 12,
                "minecraft:wither_skull" => 12,
                _ => 0,
            };
            let stack = p.arrow.as_ref().map(|a| &*a.pickup_item.item);
            let drawn = p.drawn_item();
            push(
                at,
                rules.entity(p.kind.id(), natural),
                equipment(
                    &[stack.or_else(|| drawn.as_ref().map(|s| &*s.item))],
                    p.touching_water,
                ),
                p.fire_ticks > 0,
            );
        }
        for p in &game.dragon_fireballs {
            let p = p.previous.lerp(p.position, alpha);
            push(
                DVec3::new(p.x, p.y, p.z),
                rules.entity("minecraft:dragon_fireball", 14),
                (0, None),
                false,
            );
        }
        for t in &game.tnts {
            let p = t.previous.lerp(t.position, alpha);
            let remaining = t.fuse as f32 - alpha as f32;
            push(
                DVec3::new(p.x, p.y + 0.49, p.z),
                settings
                    .tnt
                    .level(1.0 - remaining / 80.0, t.fuse / 5 % 2 == 0),
                (0, None),
                false,
            );
        }
        for stand in &game.armor_stands {
            let p = stand.render_position(alpha);
            let ids = stand
                .equipment
                .each_ref()
                .map(|s| s.as_ref().map(|s| &*s.item));
            push(
                DVec3::new(p.x, p.y + 1.2, p.z),
                0,
                equipment(&ids, wet_at(DVec3::new(p.x, p.y + 1.6, p.z))),
                stand.fire_ticks > 0,
            );
        }
    }
    if settings.frames {
        for frame in &game.item_frames {
            let p = DVec3::from_array(frame.pos.map(|v| v as f64 + 0.5));
            push(
                p,
                rules.entity(frame.kind(), if frame.glow { 14 } else { 0 }),
                equipment(&[frame.item.as_ref().map(|s| &*s.item)], wet_at(p)),
                false,
            );
        }
        for other in &game.others {
            if game.text_displays.contains_key(&(other.id as i32)) {
                continue;
            }
            let p = DVec3::new(other.position.x, other.position.y, other.position.z);
            let Some(tag) = game.persist_raw(other.id) else {
                continue;
            };
            let natural = if other.kind == "minecraft:item_display" {
                horizon_client::save::text(tag.get("item").and_then(|v| v.get("id")))
                    .map_or(0, |id| item(&id, wet_at(p)))
            } else if other.kind == "minecraft:block_display" {
                horizon_client::save::text(tag.get("block_state").and_then(|v| v.get("Name")))
                    .map_or(0, |id| block(&id))
            } else {
                0
            };
            push(p, rules.entity(&other.kind, natural), (0, None), false);
        }
    }
    if settings.frames {
        for (at, level) in
            super::text_displays::lambdynamiclights_displays::sources(game, alpha, |id| {
                item(id, false)
            })
        {
            push(at, level, (0, None), false);
        }
    }
    if settings.beams {
        for at in super::beacon::lambdynamiclights_beacons::sources(game) {
            push(at, 15, (0, None), false);
        }
        if settings.entities {
            for m in game.mobs.iter().filter(|m| !m.removed) {
                if let Some((_, scale)) = horizon_client::mobs::guardian::beam(m, alpha as f32) {
                    let target = match m.ai.target {
                        Some(horizon_client::mobs::Who::Player) => game.player.as_ref().map(|p| {
                            DVec3::new(
                                p.position.x,
                                p.position.y + f64::from(p.dimensions.eye_height) * 0.5,
                                p.position.z,
                            )
                        }),
                        Some(horizon_client::mobs::Who::Mob(id)) => {
                            game.mobs.iter().find(|t| t.id == id).map(|t| {
                                DVec3::new(
                                    t.position.x,
                                    t.position.y + f64::from(t.height()) * 0.5,
                                    t.position.z,
                                )
                            })
                        }
                        _ => None,
                    };
                    if let Some(end) = target {
                        let pos = m.previous.lerp(m.position, alpha);
                        let start = DVec3::new(pos.x, pos.y + m.eye_height(), pos.z);
                        let steps = ((end - start).length() / 4.0).ceil().clamp(1.0, 32.0) as usize;
                        for step in 0..=steps {
                            push(
                                start.lerp(end, step as f64 / steps as f64),
                                (scale.clamp(0.0, 1.0) * 15.0) as u8,
                                (0, None),
                                false,
                            );
                        }
                    }
                }
            }
        }
    }
    if settings.particles {
        let entries = [
            horizon_shader_api::manifest::ParticleLight {
                particles: vec!["minecraft:firefly".into()],
                color: [1.0; 3],
                reach: 7.0,
            },
            horizon_shader_api::manifest::ParticleLight {
                particles: vec!["minecraft:sonic_boom".into()],
                color: [1.0; 3],
                reach: 15.0,
            },
        ];
        for (p, _, level, _) in game.particles.lights(&entries, alpha as f32) {
            push(DVec3::from_array(p), level as u8, (0, None), false);
        }
    }
    out
}
