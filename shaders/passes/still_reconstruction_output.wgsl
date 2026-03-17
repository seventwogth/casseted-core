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

struct ConditionedInput {
    noise_coord: vec2<f32>,
};

@group(0) @binding(0) var luma_texture: texture_2d<f32>;
@group(0) @binding(1) var chroma_texture: texture_2d<f32>;
@group(0) @binding(2) var signal_sampler: sampler;
@group(0) @binding(3) var<uniform> effect: EffectUniform;

fn yuv_to_rgb(yuv: vec3<f32>) -> vec3<f32> {
    let y = yuv.x;
    let u = yuv.y;
    let v = yuv.z;
    return vec3<f32>(
        y + 1.13983 * v,
        y - 0.39465 * u - 0.58060 * v,
        y + 2.03211 * u,
    );
}

fn hash12(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn sample_luma(uv: vec2<f32>) -> f32 {
    let clamped = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(luma_texture, signal_sampler, clamped).x;
}

fn sample_chroma(uv: vec2<f32>) -> vec2<f32> {
    let clamped = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(chroma_texture, signal_sampler, clamped).xy;
}

fn apply_input_conditioning(uv: vec2<f32>) -> ConditionedInput {
    let frame_size = effect.frame.xy;
    let inv_size = effect.frame.zw;
    let line_index = floor(uv.y * frame_size.y + effect.input_conditioning.w);
    let line_phase = line_index + effect.reconstruction_output.w * 0.5;
    let line_jitter = sin(line_phase * 0.37) * effect.input_conditioning.z * inv_size.x;
    let sample_uv = vec2<f32>(
        uv.x + line_jitter,
        uv.y + effect.input_conditioning.w * inv_size.y,
    );

    var conditioned: ConditionedInput;
    conditioned.noise_coord = vec2<f32>(
        floor(sample_uv.x * frame_size.x),
        floor(sample_uv.y * frame_size.y),
    );
    return conditioned;
}

fn sample_output_noise(noise_coord: vec2<f32>) -> vec2<f32> {
    let frame_index = effect.reconstruction_output.w;
    let luma_noise = (hash12(noise_coord + vec2<f32>(frame_index, 3.0)) - 0.5)
        * effect.reconstruction_output.x;
    let chroma_noise = (hash12(noise_coord.yx + vec2<f32>(5.0, frame_index)) - 0.5)
        * effect.reconstruction_output.y;
    return vec2<f32>(luma_noise, chroma_noise);
}

fn reconstruct_output(luma_signal: f32, chroma_signal: vec2<f32>, noise: vec2<f32>) -> vec3<f32> {
    let reconstructed_y = clamp(
        luma_signal + dot(chroma_signal, vec2<f32>(0.10, -0.05)) * effect.reconstruction_output.z + noise.x,
        0.0,
        1.0,
    );
    let reconstructed_chroma = chroma_signal + vec2<f32>(noise.y, -noise.y * 0.5);
    return clamp(
        yuv_to_rgb(vec3<f32>(reconstructed_y, reconstructed_chroma.x, reconstructed_chroma.y)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
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
    let conditioned = apply_input_conditioning(in.uv);
    let luma_signal = sample_luma(in.uv);
    let chroma_signal = sample_chroma(in.uv);
    let noise = sample_output_noise(conditioned.noise_coord);
    let rgb = reconstruct_output(luma_signal, chroma_signal, noise);
    return vec4<f32>(rgb, 1.0);
}
