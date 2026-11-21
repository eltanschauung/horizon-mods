// SPDX-License-Identifier: MIT
use core::ops::ControlFlow;

fn keep_sprint(game: &crate::game::Game<'_>) -> ControlFlow<()> {
    if game.main_hand_attack_speed() >= 1000.0 {
        ControlFlow::Break(())
    } else {
        ControlFlow::Continue(())
    }
}
