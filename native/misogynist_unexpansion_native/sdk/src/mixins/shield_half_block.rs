// A player's shield (any `blocks_attacks` item) as it blocks: every damage reduction at half, so a
// raised shield takes 50% of a hit and the other half still hurts.
fn shield_half_block(
    value: Option<std::borrow::Cow<'_, horizon_client::items::BlocksAttacks>>,
) -> Option<std::borrow::Cow<'_, horizon_client::items::BlocksAttacks>> {
    let mut component = value?.into_owned();
    for r in &mut component.reductions {
        r.base *= 0.5;
        r.factor *= 0.5;
    }
    Some(std::borrow::Cow::Owned(component))
}
