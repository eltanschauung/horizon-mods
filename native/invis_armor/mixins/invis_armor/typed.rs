// SPDX-License-Identifier: MIT
fn handle_typed(screen: &crate::game::native_hooks::NativeScreenView<'_>, canceled: &mut bool) {
    crate::game::invis_armor::block_input(screen, canceled);
}
