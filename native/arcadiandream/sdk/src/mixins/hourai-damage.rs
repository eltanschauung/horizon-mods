fn ethereal_damage(
    game: &mut crate::game::Game<'_>,
    damage_type: &str,
) -> core::ops::ControlFlow<bool> {
    if !matches!(
        damage_type,
        "minecraft:out_of_world" | "minecraft:generic_kill"
    ) && mod_arcadiandream::block_ethereal_attack(game) == mod_api::Flow::Cancel
    {
        return core::ops::ControlFlow::Break(false);
    }
    core::ops::ControlFlow::Continue(())
}
