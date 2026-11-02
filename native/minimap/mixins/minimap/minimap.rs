// The minimap Horizon draws for the mod, unless another mod's came first.
fn minimap_view(game: &crate::game::Game<'_>, minimap: &mut Option<crate::game::native_hooks::NativeMinimap>) {
    use crate::game::official_minimap as m;
    use horizon_client::extras::Corner;
    use horizon_client::extras_hud::minimap_coordinates::COORDINATES;
    COORDINATES.with(|c| *c.borrow_mut() = None);
    // Turned off in Options > Mods: nothing until the next build leaves it out.
    if minimap.is_some() || !game.native_on(m::ID) {
        return;
    }
    let settings = m::settings(|key| game.native_setting(m::ID, key));
    COORDINATES.with(|c| *c.borrow_mut() = if settings.show_coordinates {
        game.native_player().map(|p| format!("XYZ: {:.2} / {:.2} / {:.2}", p.x, p.y, p.z))
    } else { None });
    let (size, blocks_per_pixel) = settings.view();
    *minimap = Some(crate::game::native_hooks::NativeMinimap {
        corner: [Corner::TopLeft, Corner::TopRight, Corner::BottomLeft, Corner::BottomRight][settings.corner],
        size,
        blocks_per_pixel,
        rotate: settings.rotate,
        round: settings.round,
    });
}
