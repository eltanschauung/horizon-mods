// SPDX-License-Identifier: MIT
fn keep_sprint(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if game.on("horizon_old_combat_mod") && game.main_hand_attack_speed() >= 1000.0 {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
