// Gravestone Core Game Tick Hook
fn gravestone_tick(game: &mut crate::game::Game<'_>) {
    crate::game::gravestone_glue::tick(game);
}
