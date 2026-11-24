fn obituary_tick(game: &mut crate::game::Game<'_>) {
    crate::game::gravestone_glue::check_pending_obituaries(game);
}
