fn compass_north(north: Option<(f32, f32)>) -> Option<(f32, f32)> {
    ::minimap::state::COMPASS_NORTH.with(|direction| {
        if direction.get().is_some() {
            direction.set(north);
            None
        } else {
            north
        }
    })
}
