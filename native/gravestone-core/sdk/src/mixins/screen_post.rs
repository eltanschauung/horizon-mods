fn gravestone_screen_post(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &mod_api::ScreenView<'_>,
) {
    crate::game::gravestone_glue::screen_render_post(game, batch, screen);
}
