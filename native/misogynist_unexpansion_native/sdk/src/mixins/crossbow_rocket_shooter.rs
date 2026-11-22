// A rocket goes off: if a player shot it from a crossbow, that player, else no one.
fn crossbow_rocket_shooter(r: &crate::game::fireworks::Rocket) {
    let shooter = (r.angled && r.owner == Some(horizon_client::mobs::Who::Player)).then_some(r.guest);
    if let Ok(mut s) = crate::game::fireworks::unexpansion_rocket::SHOOTER.lock() {
        *s = shooter;
    }
}
