fn handle(
    game: &crate::game::Game<'_>,
    screen: &mod_api::ScreenView<'_>,
    character: char,
    canceled: &mut bool,
) {
    crate::game::minimap_waypoints::typed(game, screen, character, canceled);
}
