// Misogynistic Lighting: no torch flicker in the frame about to be drawn (`old_lighting.cfg`).
fn old_lightmap(game: &mut crate::game::Game<'_>) {
    crate::game::misogynistic_lighting::flicker(&mut game.fog.block_light_flicker);
}
