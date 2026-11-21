fn placement_decode(
    json: &str,
) -> core::ops::ControlFlow<Option<Result<Arc<dyn NativePlacement>, DecodeError>>> {
    let value = match serde_json::from_str::<serde_json::Value>(json) {
        Ok(v) => v,
        Err(_) => return core::ops::ControlFlow::Continue(()),
    };
    if !value["type"]
        .as_str()
        .is_some_and(|s| s.starts_with("horizon_gensokyo_legacy:"))
    {
        return core::ops::ControlFlow::Continue(());
    }
    core::ops::ControlFlow::Break(Some(
        gensokyo_placements::Modifier::decode(value)
            .map(|v| Arc::new(v) as Arc<dyn NativePlacement>),
    ))
}
