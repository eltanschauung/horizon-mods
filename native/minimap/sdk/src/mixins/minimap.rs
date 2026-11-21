// The minimap Horizon draws for the mod, unless another mod's came first.
fn minimap_view(game: &crate::game::Game<'_>, minimap: &mut Option<mod_api::Minimap>) {
    use ::minimap::runtime as m;
    use ::minimap::state::{COMPASS_NORTH, COORDINATES};
    use horizon_client::extras::Corner;
    COMPASS_NORTH.with(|direction| direction.set(None));
    COORDINATES.with(|c| *c.borrow_mut() = None);
    ::minimap::state::MARKERS.with_borrow_mut(|v| v.clear());
    ::minimap::state::OWN_HEAD.with(|v| v.set(false));
    ::minimap::state::OUTLINE.with(|s| s.set((0, false)));
    // Turned off in Options > Mods: nothing until the next build leaves it out.
    if minimap.is_some() || !game.on(m::ID) {
        return;
    }
    let settings = m::settings(|key| game.setting(m::ID, key));
    COORDINATES.with(|c| {
        *c.borrow_mut() = if settings.show_coordinates {
            game.native_player().map(|p| {
                format!(
                    "{} {} {}",
                    p.x.floor() as i64,
                    p.y.floor() as i64,
                    p.z.floor() as i64
                )
            })
        } else {
            None
        }
    });
    ::minimap::state::OUTLINE.with(|s| s.set((settings.outline, settings.round)));
    COMPASS_NORTH.with(|direction| direction.set(Some((0.0, -1.0))));
    let (size, blocks_per_pixel) = settings.view();
    crate::game::minimap_waypoints::markers(game, settings);
    *minimap = Some(mod_api::Minimap {
        corner: [
            Corner::TopLeft,
            Corner::TopRight,
            Corner::BottomLeft,
            Corner::BottomRight,
        ][settings.corner],
        size,
        blocks_per_pixel,
        rotate: settings.rotate,
        round: settings.round,
    });
}
