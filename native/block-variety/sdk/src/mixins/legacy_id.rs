fn upgrade_saved_frame(id: Option<String>) -> Option<String> {
    id.map(|id| match id.as_str() {
        "block_variety:frame" => "block_variety_native:frame".into(),
        "block_variety:item_trash_can" => "block_variety_native:item_trash_can".into(),
        _ => id,
    })
}
