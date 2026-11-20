fn threshold(active: i64) -> ::core::ops::ControlFlow<i64> {
    ::core::ops::ControlFlow::Break(mod_horizon_multiplayer_sleep::players_needed(active))
}
