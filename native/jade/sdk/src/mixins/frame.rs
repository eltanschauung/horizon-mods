fn jade_frame(game: &mut crate::game::Game<'_>) {
    if ::jade::open_config() { crate::game::jade::request_open(); }
    crate::game::jade_config::frame(game);
}
