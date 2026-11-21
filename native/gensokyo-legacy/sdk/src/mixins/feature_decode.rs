fn feature_decode(
    kind: &str,
    json: &str,
    bindings: &FeatureBlockBindings<'_>,
) -> core::ops::ControlFlow<Option<Result<NativeFeature, DecodeError>>> {
    if !kind.starts_with("horizon_gensokyo_legacy:") {
        return core::ops::ControlFlow::Continue(());
    }
    if kind == "horizon_gensokyo_legacy:bamboo_grove" {
        let result = serde_json::from_str(json)
            .map_err(DecodeError::from)
            .and_then(|value| {
                crate::worldgen::native_feature::bamboo_grove::Grove::decode(value, bindings)
            })
            .map(|grove| NativeFeature(std::sync::Arc::new(grove)));
        return core::ops::ControlFlow::Break(Some(result));
    }
    let result = serde_json::from_str(json)
        .map_err(DecodeError::from)
        .and_then(|v| {
            crate::worldgen::native_feature::gensokyo::decode(
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
