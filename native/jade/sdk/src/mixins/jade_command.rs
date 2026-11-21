fn jade_dispatch(game: &mut crate::game::Game<'_>, command: &str) -> std::ops::ControlFlow<bool> {
    use crate::game::jade;
    if !game.on("jade") {
        return std::ops::ControlFlow::Continue(());
    }
    let command = command.strip_prefix('/').unwrap_or(command);
    let mut ran = false;
    if game.is_logical_server() {
        if let Some(acting) = game.native_acting_player() {
            if let Some(player) = jade::server_players(game).into_iter().find(|p| p.uuid == acting.uuid()) {
                jade::perform_command(game, &player, command, &mut ran);
            }
        } else {
            jade::perform_console_command(game, command, &mut ran);
        }
    } else {
        jade::client_command(game, command, &mut ran);
    }
    if ran {
        std::ops::ControlFlow::Break(true)
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
