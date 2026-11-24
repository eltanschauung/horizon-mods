fn wand_head(
    game: &mut crate::game::Game<'_>,
    button: winit::event::MouseButton,
    pressed: bool,
) -> core::ops::ControlFlow<Result<()>> {
    if crate::game::worldedit::WorldEditGame::worldedit_wand(game, button, pressed) {
        core::ops::ControlFlow::Break(Ok(()))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
