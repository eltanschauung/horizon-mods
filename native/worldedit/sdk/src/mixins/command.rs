fn command_head(game: &mut crate::game::Game<'_>, command: &str) -> core::ops::ControlFlow<bool> {
    match crate::game::worldedit::WorldEditGame::worldedit_command(game, command) {
        Some(result) => core::ops::ControlFlow::Break(result),
        None => core::ops::ControlFlow::Continue(()),
    }
}
