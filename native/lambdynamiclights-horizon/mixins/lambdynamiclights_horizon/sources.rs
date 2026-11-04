// SPDX-License-Identifier: MIT
fn dynamic_sources(
    game: &crate::game::Game<'_>,
    alpha: f64,
) -> core::ops::ControlFlow<Vec<(Source, Option<String>)>> {
    if game.native_on(crate::game::lambdynamiclights::ID) {
        core::ops::ControlFlow::Break(crate::game::lambdynamiclights_bridge::sources(game, alpha))
    } else {
        crate::game::lambdynamiclights_bridge::invalidate();
        core::ops::ControlFlow::Continue(())
    }
}
