// An enchantment costs the full level it asks for (a level 30 one takes 30 levels), not the 1 to 3
// of vanilla. Before the click: the player's level and the offer's cost.
fn half_level_enchant_before(game: &crate::game::Game<'_>, button: usize) {
    let cost = game.station.as_ref().and_then(|s| s.offers.costs.get(button).copied()).unwrap_or(0);
    if let Ok(mut c) = crate::game::stations::unexpansion_enchant::CLICK.lock() {
        *c = Some((game.experience.level, cost, button as i32 + 1));
    }
}
