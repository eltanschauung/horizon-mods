fn radar(game: &mut crate::game::Game<'_>, level: mod_api::LevelRef) {
    if game.dim.id() == level.dimension_id() {
        crate::game::gravestone_glue::publish_waypoints(game, game.dim.id());
    }
}
