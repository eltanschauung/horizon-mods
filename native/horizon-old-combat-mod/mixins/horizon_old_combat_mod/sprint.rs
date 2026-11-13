// SPDX-License-Identifier: MIT
fn keep_sprint(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if game.old_combat_keeps_sprint() { core::ops::ControlFlow::Break(()) } else { core::ops::ControlFlow::Continue(()) }
}
