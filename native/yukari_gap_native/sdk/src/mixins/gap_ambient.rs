fn gap_ambient(game: &mut crate::game::Game<'_>) {
    use horizon_core::random::RandomSource;
    use mod_yukari_gap_native::{AMBIENT_SOUND, GAP_ENTITY};

    if !game.on("yukari_gap_native") {
        return;
    }
    let Some(eye) = game.eye_position() else {
        return;
    };
    let cx = (eye[0].floor() as i32) >> 4;
    let cz = (eye[2].floor() as i32) >> 4;
    let world = &game.world;
    let registry = game.pack.assets.registry;

    // Use the client ambience pass, just like Nether portals. Only the
    // nearby loaded chunks can contain gaps within the sound's 16-block range.
    for x in cx - 1..=cx + 1 {
        for z in cz - 1..=cz + 1 {
            for &(bx, by, bz) in world.block_entities.mods_in((x, z)) {
                let at = [bx, by, bz].map(|axis| f64::from(axis) + 0.5);
                let distance = at
                    .iter()
                    .zip(eye)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>();
                if distance >= 16.0 * 16.0
                    || world
                        .block_entities
                        .get(&(bx, by, bz))
                        .is_none_or(|e| e.id() != GAP_ENTITY)
                {
                    continue;
                }
                let Some(state) = registry.state(world.block(bx, by, bz)) else {
                    continue;
                };
                if state
                    .properties
                    .get("half")
                    .is_some_and(|half| half == "upper")
                {
                    continue;
                }
                let random = game.particles.random();
                if random.next_int_bound(100).ok() == Some(0) {
                    let pitch = 0.8 + 0.4 * random.next_float();
                    game.particles
                        .sounds
                        .push((AMBIENT_SOUND, at, 0.5, pitch, false));
                }
            }
        }
    }
}
