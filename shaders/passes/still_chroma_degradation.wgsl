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

fn degrade_chroma(uv: vec2<f32>) -> vec2<f32> {
    let inv_size = effect.frame.zw;
    let chroma_offset = effect.chroma_degradation.x * inv_size.x;
    let chroma_blur = effect.chroma_degradation.y * inv_size.x;
    let chroma_center_uv = uv + vec2<f32>(chroma_offset, 0.0);
    let inner_step = vec2<f32>(chroma_blur, 0.0);
    let outer_step = vec2<f32>(chroma_blur * 2.0, 0.0);
    let chroma_left_outer = sample_working_signal(chroma_center_uv - outer_step).yz;
    let chroma_left_inner = sample_working_signal(chroma_center_uv - inner_step).yz;
    let chroma_center = sample_working_signal(chroma_center_uv).yz;
    let chroma_right_inner = sample_working_signal(chroma_center_uv + inner_step).yz;
    let chroma_right_outer = sample_working_signal(chroma_center_uv + outer_step).yz;
    let chroma_horizontal = chroma_left_outer * 0.14
        + chroma_left_inner * 0.22
        + chroma_center * 0.28
        + chroma_right_inner * 0.22
        + chroma_right_outer * 0.14;
    let chroma_up = sample_working_signal(chroma_center_uv - vec2<f32>(0.0, inv_size.y)).yz;
    let chroma_down = sample_working_signal(chroma_center_uv + vec2<f32>(0.0, inv_size.y)).yz;
    let chroma_vertical = (chroma_up + chroma_horizontal * 3.0 + chroma_down) * 0.2;
    return mix(
        chroma_horizontal,
        chroma_vertical,
        effect.chroma_degradation.w,
    ) * effect.chroma_degradation.z;
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
    let chroma_signal = degrade_chroma(in.uv);
    return vec4<f32>(chroma_signal, 0.0, 1.0);
}
