// Every wolf the original (pale) one, and every pig, cow and chicken the temperate one, whatever
// variant it spawned with, was saved with or a server reports: the index the game keeps for it.
// Frogs, which share the farm animals' variant names, keep theirs.
fn original_variants(value: u8, s: crate::mobs::Species) -> u8 {
    use crate::mobs::Species;
    match s {
        // `WOLF_VARIANTS[0]` is pale; the farm animals' list starts with temperate.
        Species::Wolf | Species::Pig | Species::Cow | Species::Chicken => 0,
        _ => value,
    }
}
