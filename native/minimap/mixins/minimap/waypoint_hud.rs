fn waypoint_hud(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::minimap_waypoints::draw_world(game, batch);
}
