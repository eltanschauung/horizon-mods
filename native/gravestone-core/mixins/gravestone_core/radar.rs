fn publish(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::gravestone_glue::publish_waypoints(game, dimension);
}
