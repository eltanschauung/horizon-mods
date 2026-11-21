fn tick(game: &mut crate::game::Game<'_>) {
    crate::game::fork_eating_drinking::eating_drinking_tick(game);
}
