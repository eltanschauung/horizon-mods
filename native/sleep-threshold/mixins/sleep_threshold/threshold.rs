fn threshold(game: &crate::game::Game<'_>, active: i64) -> std::ops::ControlFlow<i64> {
    use crate::game::sleep_threshold as m;
    if !game.native_on(m::ID) || game.ui.rules.int("players_sleeping_percentage") > 100 {
        return std::ops::ControlFlow::Continue(());
    }
    match m::needed(m::choice(&game.native_data_dir(m::ID)), active) {
        Some(needed) => std::ops::ControlFlow::Break(needed),
        None => std::ops::ControlFlow::Continue(()),
    }
}
