fn handle(game: &crate::game::Game<'_>, screen: &mod_api::ScreenView<'_>, canceled: &mut bool) {
    crate::game::minimap_waypoints::swallow(game, screen, canceled);
}
