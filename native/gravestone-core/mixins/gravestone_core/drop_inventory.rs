fn gravestone_drop_inventory(
    game: &mut crate::game::Game<'_>,
    at: [f64; 3],
    who: (u128, i32),
) {
    crate::game::gravestone_glue::drop_inventory(game, at, who);
}
