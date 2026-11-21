// Use the same full-range search as the vanilla command. The public synchronous
// sampling API is bounded below the command's 6400-block radius at its resolution.
fn locate_compass_biome(game: &mut crate::game::Game<'_>) {
    let Some(search) = naturescompass_native::take_search() else {
        return;
    };
    let (ran, lines) = game.run_console_command(&search.command(), true);
    let coords = ran
        .then(|| {
            lines.iter().find_map(|line| {
                naturescompass_native::coords_in_component(&line.component().to_string())
                    .or_else(|| naturescompass_native::parse_coords(&line.plain()))
            })
        })
        .flatten();
    naturescompass_native::finish_search(search, coords);
}
