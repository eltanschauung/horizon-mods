fn jade_screen_pressed(game: &crate::game::Game<'_>, screen: &mod_api::ScreenView<'_>, button: i32, modifiers: u32, canceled: &mut bool) {
    crate::game::jade_config::pressed(game, screen, button, modifiers, canceled);
}
