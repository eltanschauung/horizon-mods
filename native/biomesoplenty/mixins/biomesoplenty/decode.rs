fn decode_bop(
    kind: &str,
    bindings: &FeatureBlockBindings<'_>,
) -> std::ops::ControlFlow<Option<Result<NativeFeature, DecodeError>>> {
    match bop_runtime::decode(kind, bindings) {
        Some(feature) => std::ops::ControlFlow::Break(Some(feature)),
        None => std::ops::ControlFlow::Continue(()),
    }
}
