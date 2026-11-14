fn threshold(game: &crate::game::Game<'_>, active: i64) -> core::ops::ControlFlow<i64> {
 if game.native_on("horizon_multiplayer_sleep") { core::ops::ControlFlow::Break((active.max(0).saturating_mul(33).saturating_add(99)/100).max(1)) } else { core::ops::ControlFlow::Continue(()) }
}
