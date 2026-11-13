// SPDX-License-Identifier: MIT
fn draw(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    crate::game::invis_armor::draw(game, batch, screen);
}
