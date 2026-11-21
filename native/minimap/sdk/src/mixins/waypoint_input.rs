fn handle(
    game: &crate::game::Game<'_>,
    screen: &mod_api::ScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    crate::game::minimap_waypoints::input(game, screen, key, modifiers, canceled);
}
