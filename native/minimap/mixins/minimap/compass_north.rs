fn compass_north(north: Option<(f32, f32)>) -> Option<(f32, f32)> {
    horizon_client::extras_hud::minimap_coordinates::COMPASS_NORTH.with(|direction| {
        if direction.get().is_some() {
            direction.set(north);
            None
        } else {
            north
        }
    })
}
