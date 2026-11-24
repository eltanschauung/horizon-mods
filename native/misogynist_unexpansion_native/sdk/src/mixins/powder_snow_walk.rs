// What powder snow asks of the player, as if leather boots were always on and it had never fallen:
// it holds them up, as it did every block of snow before, even landing on it from high up.
fn powder_snow_walk(value: crate::collisions::PowderSnowSubject<'static>) -> crate::collisions::PowderSnowSubject<'static> {
    crate::collisions::PowderSnowSubject { leather_boots: true, fall_distance: 0.0, ..value }
}
