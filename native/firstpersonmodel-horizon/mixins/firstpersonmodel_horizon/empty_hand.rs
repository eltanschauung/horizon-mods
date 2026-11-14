// SPDX-License-Identifier: MIT
fn empty_hand(game: &crate::game::Game<'_>, stack: Option<&ItemStack>, canceled: &mut bool) {
    if stack.is_none() && game.native_on(crate::game::firstpersonmodel::ID) && game.firstperson_auto_hide()
        && !game.firstperson_vanilla_hands() && game.firstperson_body_active() {
        *canceled = true;
    }
}
