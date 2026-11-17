fn feature_decode(
    kind: &str,
    json: &str,
    bindings: &FeatureBlockBindings<'_>,
) -> core::ops::ControlFlow<Option<Result<NativeFeature, DecodeError>>> {
    if !kind.starts_with("horizon_gensokyo_legacy:") {
        return core::ops::ControlFlow::Continue(());
    }
    let result = serde_json::from_str(json)
        .map_err(DecodeError::from)
        .and_then(|v| {
            crate::worldgen::feature_graph::gensokyo::decode(
                v,
                bindings,
                0,
                &mut 65536,
                &std::collections::BTreeSet::new(),
            )
        })
        .map(|v| NativeFeature(std::sync::Arc::new(v)));
    core::ops::ControlFlow::Break(Some(result))
}
