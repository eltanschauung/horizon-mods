fn local_particles(
    game: &mut crate::game::Game<'_>,
    stack: &ItemStack,
    count: u32,
) -> core::ops::ControlFlow<()> {
    if crate::game::fork_eating_drinking::eating_drinking_local_particles(game, stack, count) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
