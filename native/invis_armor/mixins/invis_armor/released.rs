// SPDX-License-Identifier: MIT
fn released(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    crate::game::invis_armor::released(game, screen, canceled);
}
