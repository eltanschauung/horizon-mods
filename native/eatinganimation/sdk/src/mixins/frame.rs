use core::ops::ControlFlow;

fn frame(item: &str, model: &str, elapsed_ticks: i32, has_food: bool) -> ControlFlow<Option<String>> {
    if !has_food && !matches!(item, "minecraft:potion" | "minecraft:milk_bucket") {
        return ControlFlow::Continue(());
    }
    let stage = match elapsed_ticks {
        27.. => 2,
        21..=26 => 1,
        11..=20 => 0,
        _ => return ControlFlow::Continue(()),
    };
    let frame = match item {
        "minecraft:potion" => format!("minecraft:item/potion_overlay_drinking_{stage}"),
        "minecraft:enchanted_golden_apple" => format!("minecraft:item/golden_apple_eating_{stage}"),
        "minecraft:milk_bucket" | "minecraft:honey_bottle" => format!("{model}_drinking_{stage}"),
        _ => format!("{model}_eating_{stage}"),
    };
    ControlFlow::Break(Some(frame))
}
