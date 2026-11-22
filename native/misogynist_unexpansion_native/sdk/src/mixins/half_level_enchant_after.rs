// After the click: when it enchanted here (the level fell by the slot's 1 to 3), the rest of the
// level asked is taken too.
fn half_level_enchant_after(value: (), game: &mut crate::game::Game<'_>) {
    let () = value;
    let click = crate::game::stations::unexpansion_enchant::CLICK.lock().ok().and_then(|mut c| c.take());
    let Some((before, cost, charged)) = click else { return };
    let owed = cost - charged;
    if game.remote.is_none() && owed > 0 && game.experience.level == before - charged {
        game.give_levels(-owed);
    }
}
