// SPDX-License-Identifier: MIT
fn block_input(screen: &crate::game::native_hooks::NativeScreenView<'_>, canceled: &mut bool) {
    crate::game::invis_armor::block_input(screen, canceled);
}
