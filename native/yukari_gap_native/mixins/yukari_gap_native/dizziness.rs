// A gentle camera roll, subject to Horizon's screen effects accessibility setting.
fn gap_dizziness(game: &crate::game::Game<'_>, shake: &mut crate::game::native_hooks::NativeShake) {
    if game.potions.effects.amplifier("yukari_gap:dizziness").is_some() {
        let time = horizon_client::hud_overlays::now_ms() as f64 / 1000.0;
        shake.roll += (time * 2.4).sin() as f32 * 2.5;
    }
}
