// SPDX-License-Identifier: MIT
fn frame(game: &crate::game::Game<'_>) {
    horizon_client::mobs::render::set_chestplate_hides_skin(game.equipment_render_opacity("chestplate") >= 1.0);
    crate::game::invis_armor::frame(game);
}
