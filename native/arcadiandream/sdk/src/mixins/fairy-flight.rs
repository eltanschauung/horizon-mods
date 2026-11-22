fn fairy_flight(
    mob: &mut crate::mobs::Mob,
    world: &crate::mobs::MobWorld<'_>,
    level: &mut crate::health::DamageLevel<'_>,
    events: &mut Vec<crate::mobs::MobEvent>,
) {
    if matches!(
        mob.kind.id.as_str(),
        "arcadiandream:fairy" | "arcadiandream:sunflower_fairy" | "arcadiandream:ice_fairy"
    ) {
        mob.travel(world, level, horizon_core::geometry::Vec3::ZERO, events);
    }
}
