fn handle(
    game: &crate::game::Game<'_>,
    screen: &mod_api::ScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::minimap_waypoints::release(game, screen, button, canceled);
}
