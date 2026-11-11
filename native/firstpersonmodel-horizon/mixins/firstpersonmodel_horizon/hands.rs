// SPDX-License-Identifier: MIT
fn hands(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<(Vec<Vertex>,Vec<(u32,i32)>)> { if game.firstperson_body_active() && !game.firstperson_vanilla_hands() { core::ops::ControlFlow::Break((Vec::new(),Vec::new())) } else { core::ops::ControlFlow::Continue(()) } }
