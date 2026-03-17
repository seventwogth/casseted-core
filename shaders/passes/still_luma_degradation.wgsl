struct EffectUniform {
    frame: vec4<f32>,
    input_conditioning: vec4<f32>,
    luma_degradation: vec4<f32>,
    chroma_degradation: vec4<f32>,
    reconstruction_output: vec4<f32>,
};

struct VsOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var working_texture: texture_2d<f32>;
@group(0) @binding(1) var working_sampler: sampler;
@group(0) @binding(2) var<uniform> effect: EffectUniform;

fn sample_working_signal(uv: vec2<f32>) -> vec3<f32> {
    let clamped = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(working_texture, working_sampler, clamped).rgb;
}

fn degrade_luma(uv: vec2<f32>) -> f32 {
    let inv_size = effect.frame.zw;
    let center = sample_working_signal(uv);
    let luma_offset = effect.luma_degradation.x * inv_size.x;
    let inner_step = vec2<f32>(luma_offset, 0.0);
    let outer_step = vec2<f32>(luma_offset * 2.0, 0.0);
    let left_outer = sample_working_signal(uv - outer_step);
    let left_inner = sample_working_signal(uv - inner_step);
    let right_inner = sample_working_signal(uv + inner_step);
    let right_outer = sample_working_signal(uv + outer_step);
    let blurred_luma = left_outer.x * 0.15
        + left_inner.x * 0.22
        + center.x * 0.26
        + right_inner.x * 0.22
        + right_outer.x * 0.15;
    let edge_band = left_inner.x * 0.25 + center.x * 0.5 + right_inner.x * 0.25;
    return clamp(
        blurred_luma + (center.x - edge_band) * effect.luma_degradation.y,
        0.0,
        1.0,
    );
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );

    var output: VsOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.uv = uvs[vertex_index];
    return output;
}

@fragment
fn fs_main(in: VsOutput) -> @location(0) vec4<f32> {
    let luma_signal = degrade_luma(in.uv);
    return vec4<f32>(luma_signal, 0.0, 0.0, 1.0);
}
