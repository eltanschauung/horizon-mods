// A mouse button while playing: easy place takes the right button's press when it places (or when
// the game would place a block the schematic doesn't want), and a middle click on a block the
// schematic wants takes that block in hand (see game_support.rs). The mod's own clicks pass.
fn horizonmatica_mouse(game: &mut crate::game::Game<'_>, button: winit::event::MouseButton, pressed: bool) -> ::core::ops::ControlFlow<Result<()>> {
    if crate::game::horizonmatica_game::mouse(game, button, pressed) {
        ::core::ops::ControlFlow::Break(Ok(()))
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
