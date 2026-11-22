fn hook(
    level: &crate::game::GameLevel<'_>,
    pos: BlockPos,
    state: BlockStateId,
    survives: &mut Option<bool>,
) {
    let Some(s) = level
        .registry
        .state(state)
        .filter(|s| s.name == "arcadiandream:mysterious_seal")
    else {
        return;
    };
    let d = match s.properties.get("facing").map(String::as_str) {
        Some("up") => [0, -1, 0],
        Some("down") => [0, 1, 0],
        Some("east") => [-1, 0, 0],
        Some("west") => [1, 0, 0],
        Some("south") => [0, 0, -1],
        _ => [0, 0, 1],
    };
    *survives = Some(
        level
            .facts
            .get(level.world.block(pos.x + d[0], pos.y + d[1], pos.z + d[2]))
            .full_collision,
    );
}
