fn handle(
    game: &crate::game::Game<'_>,
    screen: &mod_api::ScreenView<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) {
    crate::game::minimap_waypoints::draw(game, screen, batch);
}
