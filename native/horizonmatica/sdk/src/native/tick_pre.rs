// Once a client tick, before the window's tick: the overlay is built with the game's block
// models (see game_support.rs).
fn horizonmatica_tick_pre(game: &mut crate::game::Game<'_>) {
    crate::game::horizonmatica_game::tick(game);
}
