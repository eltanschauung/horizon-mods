fn settings(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    use crate::game::sleep_threshold as m;
    if mod_id == m::ID {
        buttons.push(m::label(m::choice(&game.native_data_dir(m::ID))));
    }
}
