fn life_ready(game: &mut horizon_client::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if game.is_logical_server()
        && mod_arcadiandream::life_ready(game)
        && let Some(player) = game.player.as_mut()
    {
        player.health.totem_ready = true;
        player.health.totem_off_hand = false;
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
