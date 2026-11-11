// SPDX-License-Identifier: MIT
fn visible(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<bool> { if game.firstperson_body_active() { core::ops::ControlFlow::Break(true) } else { core::ops::ControlFlow::Continue(()) } }
