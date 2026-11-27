// Misogynistic Lighting: the frame's light made the old light's, and its sky, fog, sunrise and
// cloud colours 1.7.10's, after the End's flash and before the lightning flash (`old_lighting.cfg`).
fn old_sky(sky: &mut horizon_client::environment::Sky, game: &crate::game::Game<'_>, partial: f64) {
    crate::game::misogynistic_lighting::sky(game, sky, partial);
}
