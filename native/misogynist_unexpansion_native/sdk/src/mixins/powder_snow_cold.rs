// Standing on powder snow freezes you slowly: fully frozen after 15 seconds (300 ticks to the 140
// ticks of frost vanilla's freezing counts to), then its damage every two seconds as when inside
// it, and the frost overlay and frozen slowness growing on the way. Run before the tick's block
// effects, single player's and a server's alike; the 2 more each tick make up the thaw that follows
// them when the player isn't in powder snow.
fn powder_snow_cold(world: &mut MotionWorld<'_, '_, '_>) {
    if !world.player.can_freeze() {
        return;
    }
    let Ok(on) = world.support(horizon_world::movement_support::LEGACY_SUPPORT_OFFSET) else { return };
    let snow = world.assets.registry.state(world.block(on)).is_some_and(|s| s.name == "minecraft:powder_snow");
    if !snow {
        return;
    }
    let full = PlayerMotion::TICKS_REQUIRED_TO_FREEZE;
    // 7 ticks of frost in every 15: 140 in 300.
    let step = 2 + i32::from(world.player.health.tick_count.rem_euclid(15) < 7);
    world.player.ticks_frozen = (world.player.ticks_frozen + step).min(full + 2);
}
