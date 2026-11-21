fn particles(
    game: &mut crate::game::Game<'_>,
    item: &str,
    ticks: u32,
    at: [f64; 3],
) -> core::ops::ControlFlow<()> {
    if crate::game::fork_eating_drinking::eating_drinking_particles(game, item, ticks, at) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
