// The blast reaching the player in place (the host, or a guest swapped in): not the one who shot it.
fn crossbow_rocket_spares_shooter(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    let shooter = crate::game::fireworks::unexpansion_rocket::SHOOTER.lock().ok().and_then(|s| *s);
    if shooter.is_some_and(|conn| conn == game.lan_acting) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
