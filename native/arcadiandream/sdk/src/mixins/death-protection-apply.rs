fn life_apply(game: &mut horizon_client::game::Game<'_>) -> core::ops::ControlFlow<Option<Body>> {
    if game.is_logical_server()
        && game
            .player
            .as_ref()
            .is_some_and(|player| player.health.totem_used)
        && mod_arcadiandream::revive_acting(game)
        && let Some(player) = game.player.as_mut()
    {
        player.health.totem_used = false;
        return core::ops::ControlFlow::Break(Some(horizon_client::game::totem::Body {
            bounds: horizon_client::game::render_tick::player_tracking_body(player),
            protector: None,
        }));
    }
    core::ops::ControlFlow::Continue(())
}
