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
    sample_uv: vec2<f32>,
    noise_coord: vec2<f32>,
};

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> effect: EffectUniform;

const BT601_LUMA: vec3<f32> = vec3<f32>(0.299, 0.587, 0.114);

fn rgb_to_yuv(rgb: vec3<f32>) -> vec3<f32> {
    let y = dot(rgb, BT601_LUMA);
    let u = (rgb.b - y) * 0.492111;
    let v = (rgb.r - y) * 0.877283;
    return vec3<f32>(y, u, v);
}

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

fn sample_rgb(uv: vec2<f32>) -> vec3<f32> {
    let clamped = clamp(uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
    return textureSample(input_texture, input_sampler, clamped).rgb;
}

fn soft_highlight_knee(value: f32, knee: f32, compression: f32) -> f32 {
    if (compression <= 0.0 || value <= knee || knee >= 0.999) {
        return value;
    }

    let headroom = max(1e-5, 1.0 - knee);
    let t = clamp((value - knee) / headroom, 0.0, 1.0);
    let compressed = log2(1.0 + compression * t) / log2(1.0 + compression);
    return knee + compressed * headroom;
}

fn apply_tone_shaping(rgb: vec3<f32>) -> vec3<f32> {
    let y = dot(rgb, BT601_LUMA);
    if (y <= 1e-5) {
        return rgb;
    }

    let toned_y = soft_highlight_knee(
        y,
        effect.input_conditioning.x,
        effect.input_conditioning.y,
    );
    let scale = toned_y / max(y, 1e-5);
    return clamp(rgb * scale, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn sample_working_signal(uv: vec2<f32>) -> vec3<f32> {
    return rgb_to_yuv(apply_tone_shaping(sample_rgb(uv)));
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
    conditioned.sample_uv = sample_uv;
    conditioned.noise_coord = vec2<f32>(
        floor(sample_uv.x * frame_size.x),
        floor(sample_uv.y * frame_size.y),
    );
    return conditioned;
}

fn degrade_luma(sample_uv: vec2<f32>) -> f32 {
    let inv_size = effect.frame.zw;
    let center = sample_working_signal(sample_uv);
    let luma_offset = effect.luma_degradation.x * inv_size.x;
    let inner_step = vec2<f32>(luma_offset, 0.0);
    let outer_step = vec2<f32>(luma_offset * 2.0, 0.0);
    let left_outer = sample_working_signal(sample_uv - outer_step);
    let left_inner = sample_working_signal(sample_uv - inner_step);
    let right_inner = sample_working_signal(sample_uv + inner_step);
    let right_outer = sample_working_signal(sample_uv + outer_step);
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

fn degrade_chroma(sample_uv: vec2<f32>) -> vec2<f32> {
    let inv_size = effect.frame.zw;
    let chroma_offset = effect.chroma_degradation.x * inv_size.x;
    let chroma_blur = effect.chroma_degradation.y * inv_size.x;
    let chroma_center_uv = sample_uv + vec2<f32>(chroma_offset, 0.0);
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
    let luma = degrade_luma(conditioned.sample_uv);
    let chroma = degrade_chroma(conditioned.sample_uv);
    let noise = sample_output_noise(conditioned.noise_coord);
    let rgb = reconstruct_output(luma, chroma, noise);
    return vec4<f32>(rgb, 1.0);
}
