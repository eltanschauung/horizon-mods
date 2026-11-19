// Gravestone Core BlockEntity Hook
fn gravestone_block_entity(
    block: &str,
    entity: &mut Option<(String, horizon_world::nbt::Compound)>,
) {
    crate::game::gravestone_glue::block_entity(block, entity);
}
