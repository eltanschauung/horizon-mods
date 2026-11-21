fn inventory(game: &mut crate::game::Game<'_>, dying: &crate::game::creatures::DyingDrops<'_>) {
    if let Some(uuid) = dying.player {
        crate::game::gravestone_glue::drop_inventory(game, uuid);
    }
}
