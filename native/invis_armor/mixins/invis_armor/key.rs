// SPDX-License-Identifier: MIT
fn handle_key(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    crate::game::invis_armor::key(game, screen, key, modifiers, canceled);
}
