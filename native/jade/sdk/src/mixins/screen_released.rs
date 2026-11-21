fn jade_screen_released(game: &crate::game::Game<'_>, screen: &mod_api::ScreenView<'_>, canceled: &mut bool) {
    crate::game::jade_config::released(game, screen, canceled);
}
