// SPDX-License-Identifier: MIT
fn hands(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<(Vec<Vertex>,Vec<(u32,i32)>)> {
    let angle_hides_items = game.native_on(crate::game::firstpersonmodel::ID)
        && game.camera_type.first_person() && game.firstperson_auto_hide()
        && game.visible_hand_items().iter().flatten().next().is_some()
        && !game.firstperson_vanilla_hands();
    if angle_hides_items || (game.firstperson_body_active() && !game.firstperson_vanilla_hands()) {
        core::ops::ControlFlow::Break((Vec::new(),Vec::new()))
    } else { core::ops::ControlFlow::Continue(()) }
}
