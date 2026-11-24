// SPDX-License-Identifier: MIT
use super::Game;
use eating_drinking_animation::state as anim;
use horizon_client::mobs::{Mob, anim::BodyPose, render};
use horizon_core::use_animation::UseAnimation;
use std::sync::Arc;
fn eating_drinking_kind(game: &Game<'_>, stack: &horizon_client::items::ItemStack) -> Option<bool> {
    let components = game
        .pack
        .item_renderer
        .models
        .components(&horizon_client::item_render::stack_view(stack));
    let mut a = components.get("minecraft:consumable").map_or(
        UseAnimation::None,
        horizon_client::first_person::consumable_animation,
    );
    game.native_use_animation(stack, &mut a);
    match a {
        UseAnimation::Eat => Some(false),
        UseAnimation::Drink => Some(true),
        _ => None,
    }
}
pub(super) fn eating_drinking_tick(game: &mut Game<'_>) {
    let mut inputs = Vec::new();
    if game.on("eating_drinking_animation") && game.player.is_some() {
        for m in &game.mobs {
            if &*m.kind.id != "minecraft:player" {
                continue;
            }
            let active = (m.extra.using_item > 0)
                .then(|| {
                    let stack = render::held_item_stack(&m.extra, m.extra.using_off_hand)?;
                    let drink = eating_drinking_kind(game, &stack)?;
                    Some((
                        m.main_arm_left() != m.extra.using_off_hand,
                        drink,
                        m.extra.using_item as f32,
                    ))
                })
                .flatten();
            inputs.push((m.join.get(), active));
        }
        if let Some(m) = &game.player_model {
            let active = game.using.and_then(|(slot, ticks)| {
                let drink = eating_drinking_kind(game, game.stack_in(slot)?)?;
                Some((
                    game.settings.main_hand_left != (slot == super::shield::OFFHAND_SLOT),
                    drink,
                    ticks as f32,
                ))
            });
            inputs.push((m.join.get(), active));
        }
    }
    anim::update(&inputs);
    for model in game.mobs.iter_mut().chain(game.player_model.iter_mut()) {
        if model.kind.id.as_str() == "minecraft:player" {
            update_model_pose(model);
        }
    }
}
pub(super) fn eating_drinking_particles(
    game: &mut Game<'_>,
    item: &str,
    ticks: u32,
    at: [f64; 3],
) -> bool {
    if !game.on("eating_drinking_animation") {
        return false;
    }
    let Some(m) = game.mobs.iter().find(|m| {
        &*m.kind.id == "minecraft:player"
            && (m.position.x - at[0]).abs() < 0.001
            && (m.position.y - at[1]).abs() < 0.001
            && (m.position.z - at[2]).abs() < 0.001
    }) else {
        return false;
    };
    let Some(c) = game.items().get(item).and_then(|i| i.consumable.as_ref()) else {
        return false;
    };
    if !c.particles || !c.should_emit((c.ticks().max(0) as u32).saturating_sub(ticks) as i32) {
        return true;
    }
    // Camera-relative transform avoids losing precision far from world origin.
    let camera = glam::DVec3::from_array(at);
    let Some(t) = render::held_item_transform(
        m,
        1.0,
        camera,
        game.res.mob_data(),
        m.main_arm_left() != m.extra.using_off_hand,
    ) else {
        return false;
    };
    let origin = t.transform_point3(glam::Vec3::ZERO).as_dvec3() + camera;
    eating_drinking_crumbs(
        game,
        &horizon_client::items::ItemStack::new(item, 1),
        5,
        origin,
    );
    true
}
pub(super) fn eating_drinking_local_particles(
    game: &mut Game<'_>,
    stack: &horizon_client::items::ItemStack,
    count: u32,
) -> bool {
    if !game.on("eating_drinking_animation")
        || game.camera_type.is_first_person()
        || game.using.is_none()
    {
        return false;
    }
    let Some(m) = &game.player_model else {
        return false;
    };
    let at = glam::DVec3::new(m.position.x, m.position.y, m.position.z);
    let left = game.settings.main_hand_left
        != game
            .using
            .is_some_and(|(s, _)| s == super::shield::OFFHAND_SLOT);
    let Some(t) = render::held_item_transform(m, 1.0, at, game.res.mob_data(), left) else {
        return false;
    };
    let origin = t.transform_point3(glam::Vec3::ZERO).as_dvec3() + at;
    eating_drinking_crumbs(game, stack, count, origin);
    true
}
fn eating_drinking_crumbs(
    game: &mut Game<'_>,
    stack: &horizon_client::items::ItemStack,
    count: u32,
    origin: glam::DVec3,
) {
    let options = game.item_particle(stack);
    use horizon_core::random::RandomSource;
    for _ in 0..count {
        let jitter =
            std::array::from_fn::<_, 3, _>(|_| f64::from(game.world.random.next_float()) - 0.5);
        let p = [
            origin.x + jitter[0] * 0.12,
            origin.y + jitter[1] * 0.12,
            origin.z + jitter[2] * 0.12,
        ];
        game.particles
            .add(&options, p, [jitter[0] * 0.04, 0.02, jitter[2] * 0.04]);
    }
}

fn update_model_pose(model: &mut Mob) {
    let arms = anim::arms(model.join.get());
    let pose = model.emote.as_ref().and_then(|existing| {
        let any: &dyn std::any::Any = &**existing;
        any.downcast_ref::<UseItemPose>()
    });
    let active = arms.iter().any(|arm| arm.old > 0.0 || arm.weight > 0.0)
        && !model.extra.spectator
        && model.alive();
    if !active && pose.is_none() {
        return;
    }
    let inner = if let Some(pose) = pose {
        pose.inner.clone()
    } else {
        model.emote.take()
    };
    model.emote = if active {
        Some(Arc::new(UseItemPose { inner, arms }))
    } else {
        inner
    };
}

struct UseItemPose {
    inner: Option<Arc<dyn BodyPose>>,
    arms: [anim::Arm; 2],
}

impl std::fmt::Debug for UseItemPose {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("UseItemPose").finish_non_exhaustive()
    }
}

impl BodyPose for UseItemPose {
    fn pose(&self, age: f32, at_rest: &[[f32; 9]; 6], parts: &mut [[f32; 9]; 6]) -> bool {
        if let Some(inner) = &self.inner {
            inner.pose(age, at_rest, parts);
        }
        let partial = age.fract().clamp(0.0, 1.0);
        let head = parts[0];
        for (side, arm) in self.arms.iter().enumerate() {
            let weight = arm.old + (arm.weight - arm.old) * partial;
            if weight <= 0.0 {
                continue;
            }
            let eased = weight * weight * (3.0 - 2.0 * weight);
            let target = anim::rotations(
                [head[3], head[4]],
                arm.drink,
                arm.ticks + partial,
                side == 1,
            );
            // The sleeves remain children of these arms.
            for (angle, wanted) in parts[2 + side][3..6].iter_mut().zip(target) {
                *angle += (wanted - *angle) * eased;
            }
        }
        true
    }
}
