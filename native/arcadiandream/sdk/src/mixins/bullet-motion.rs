fn tick_shot(
    value: Option<(bool, Hit)>,
    shot: &mut crate::projectiles::Projectile,
    world: &dyn crate::projectiles::ProjectileWorld,
    impact: bool,
) -> Option<(bool, Hit)> {
    crate::arcadian_motion::tick(shot, world, impact).or(value)
}
