// Powder snow slows what walks on it as soul sand does (its speed factor 0.4), now that it holds
// you up: the movement properties each block state is bound to, powder snow's made slow.
fn powder_snow_slow(
    value: io::Result<BoundMovementProperties>,
    props: &MovementProperties<'_>,
) -> io::Result<BoundMovementProperties> {
    let mut bound = value?;
    for (id, block) in bound.states.iter_mut().enumerate() {
        if props.registry.state(BlockStateId(id as u32)).is_some_and(|s| s.name == "minecraft:powder_snow") {
            block.coefficients.speed_factor = 0.4;
        }
    }
    Ok(bound)
}
