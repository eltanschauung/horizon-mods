fn click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::sleep_threshold as m;
    if mod_id != m::ID || button != 0 {
        return;
    }
    let path = game.native_data_dir(m::ID);
    let next = (m::choice(&path) + 1) % m::LABELS.len();
    game.native_set_setting(m::ID, "threshold", m::LABELS[next]);
    m::invalidate(&path);
}
