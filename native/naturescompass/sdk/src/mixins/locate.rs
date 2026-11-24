// Search the selected dimension using the same typed biome lookup as /locate.
fn locate_compass_biome(game: &mut crate::game::Game<'_>) {
    let Some(search) = naturescompass_native::take_search() else {
        return;
    };
    let (dimension, biome, origin) = search.target();
    let target = crate::game::Dim::from_id(dimension);
    let back = game.enter_command_level(target);
    let coords = if game.dim == target {
        let exact = game.level(dimension).and_then(|level| {
            naturescompass_native::nearby_saved_biome(game, level, biome, origin)
        });
        exact
            .or_else(|| game.nearest_biome(biome, origin).map(|(_, at)| at))
            .map(|[x, y, z]| (x, Some(y), z))
    } else {
        None
    };
    if let Some(back) = back {
        game.enter_command_level(back);
    }
    naturescompass_native::finish_search(search, coords);
}
