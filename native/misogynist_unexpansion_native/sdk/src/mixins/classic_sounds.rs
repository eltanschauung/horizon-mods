// Use the classic sounds (including for babies, which still have a higher voice).
fn classic_sounds(value: String, species: crate::mobs::Species, kind: &str) -> String {
    use crate::mobs::Species;
    let name = match species {
        Species::Pig => "pig",
        Species::Cow => "cow",
        Species::Chicken => "chicken",
        Species::Wolf => "wolf",
        Species::Cat => "cat",
        _ => return value,
    };
    format!("entity.{name}.{kind}")
}
