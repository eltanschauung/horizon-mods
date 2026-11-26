// SPDX-License-Identifier: MIT
// BodyPose and recursive geometry integration follow the MIT SDK port by DeportedToHell.
use super::Game;
use core::ops::ControlFlow;
use firstpersonmodel_horizon::{self as fpm, view_policy};
use horizon_client::mobs::anim::BodyPose;
use horizon_client_render::render_geometry::Vertex;
use std::{cell::Cell, sync::Arc};

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static DRAWING: Cell<bool> = const { Cell::new(false) };
    static VANILLA_HANDS: Cell<bool> = const { Cell::new(true) };
}

#[derive(Debug)]
struct HeadlessBody(Option<Arc<dyn BodyPose>>);

impl BodyPose for HeadlessBody {
    fn pose(&self, age: f32, rest: &[[f32; 9]; 6], parts: &mut [[f32; 9]; 6]) -> bool {
        let changed = self
            .0
            .as_ref()
            .is_some_and(|pose| pose.pose(age, rest, parts));
        if DRAWING.with(Cell::get) {
            parts[0][6..9].fill(0.0);
            true
        } else {
            changed
        }
    }
}

pub(in crate::game) fn frame_pre(game: &mut Game<'_>) {
    let (configured_enabled, auto_hide, configured_hands) = fpm::settings();
    let enabled = configured_enabled && game.on(fpm::ID)
        && game.camera_type.is_first_person()
        && !game.spectator()
        && game.sleeping.is_none()
        && game.replay.driver.is_none();

    let hidden = game.player.as_ref().is_none_or(|player| {
        auto_hide && view_policy::hide_body(game.pitch, player.swimming, player.fall_flying)
    });
    let vanilla = if auto_hide { hidden } else { configured_hands };
    let active = enabled
        && !hidden
        && game
            .player
            .as_ref()
            .is_some_and(|player| !player.health.dead_or_dying())
        && game.player_model.is_some()
        && (vanilla
            || !game.visible_hand_items().iter().flatten().any(|stack| {
                matches!(
                    stack.item.as_ref(),
                    "minecraft:filled_map" | "minecraft:spyglass"
                )
            }));

    ACTIVE.with(|flag| flag.set(active));
    VANILLA_HANDS.with(|flag| flag.set(vanilla));
    fpm::cancel_hands(active && !vanilla);
    if active {
        if let Some(model) = game.player_model.as_mut() {
            model.emote = Some(Arc::new(HeadlessBody(model.emote.take())));
        }
    }
}

pub(in crate::game) fn frame_post(game: &mut Game<'_>) {
    if ACTIVE.with(|flag| flag.replace(false))
        && let Some(model) = game.player_model.as_mut()
        && let Some(pose) = model.emote.as_ref()
    {
        let pose: &dyn std::any::Any = &**pose;
        if let Some(headless) = pose.downcast_ref::<HeadlessBody>() {
            model.emote = headless.0.clone();
        }
    }
    fpm::cancel_hands(false);
}

pub(in crate::game) fn body_in_view(game: &Game<'_>) -> bool {
    ACTIVE.with(Cell::get) && game.camera_type.is_first_person()
}

pub(in crate::game) fn draw_body(
    game: &Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<Vertex>,
) -> ControlFlow<()> {
    if !body_in_view(game) || DRAWING.with(Cell::get) {
        return ControlFlow::Continue(());
    }
    let Some(model) = game.player_model.as_ref() else {
        return ControlFlow::Continue(());
    };
    let yaw = model.previous_body_yaw
        + ((model.body_yaw - model.previous_body_yaw + 180.0).rem_euclid(360.0) - 180.0)
            * alpha as f32;
    let yaw = f64::from(yaw).to_radians();
    let scale = game
        .player
        .as_ref()
        .map_or(1.0, |player| f64::from(player.scale));
    let distance = if model.extra.crouching { 0.35 } else { 0.25 } * scale;
    let offset = glam::DVec3::new(distance * yaw.sin(), 0.0, -distance * yaw.cos());
    DRAWING.with(|flag| flag.set(true));
    game.player_model_geometry(camera - offset, alpha, out);
    DRAWING.with(|flag| flag.set(false));
    ControlFlow::Break(())
}

pub(in crate::game) fn hide_held_items() -> ControlFlow<()> {
    if DRAWING.with(Cell::get) && VANILLA_HANDS.with(Cell::get) {
        ControlFlow::Break(())
    } else {
        ControlFlow::Continue(())
    }
}

pub(in crate::game) fn hide_head_items() -> ControlFlow<()> {
    if DRAWING.with(Cell::get) {
        ControlFlow::Break(())
    } else {
        ControlFlow::Continue(())
    }
}
