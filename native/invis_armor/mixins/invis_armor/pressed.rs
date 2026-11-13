// SPDX-License-Identifier: MIT
fn pressed(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::invis_armor::pressed(game, screen, button, canceled);
}
