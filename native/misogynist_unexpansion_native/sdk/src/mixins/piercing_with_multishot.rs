// Piercing and Multishot go on one crossbow together: vanilla's crossbow exclusive set keeps them
// apart, at the enchanting table, the anvil and `/enchant` alike.
fn piercing_with_multishot(value: bool, a: &str, b: &str) -> bool {
    let pair = |x: &str, y: &str| x == "minecraft:piercing" && y == "minecraft:multishot";
    value || pair(a, b) || pair(b, a)
}
