fn pipeline_fragment(input: PipelineInput) -> vec4<f32> {
    let uv = clamp(input.uv, vec2<f32>(0.0), vec2<f32>(0.999999));
    return pipeline_texture(uv) * input.color;
}
