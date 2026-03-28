struct EffectUniform {
    frame: vec4<f32>,
    input_conditioning: vec4<f32>,
    luma_degradation: vec4<f32>,
    chroma_degradation: vec4<f32>,
    reconstruction_output: vec4<f32>,
    reconstruction_aux: vec4<f32>,
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

fn frame_inv_size() -> vec2<f32> {
    return vec2<f32>(
        1.0 / max(effect.frame.x, 1.0),
        1.0 / max(effect.frame.y, 1.0),
    );
}

fn highlight_mask(value: f32, threshold: f32) -> f32 {
    let headroom = max(1e-5, 1.0 - threshold);
    return clamp((value - threshold) / headroom, 0.0, 1.0);
}

fn bandwidth_mix(blur_px: f32) -> f32 {
    return blur_px / (blur_px + 1.35);
}

fn detail_recovery_mix() -> f32 {
    return clamp(effect.luma_degradation.y / 0.12, 0.0, 1.0);
}

fn highlight_bleed(
    prev_near: f32,
    prev_mid: f32,
    prev_far: f32,
    center_luma: f32,
    base_luma: f32,
) -> f32 {
    let threshold = effect.luma_degradation.z;
    let amount = effect.luma_degradation.w;
    if (amount <= 1e-4 || threshold >= 0.999) {
        return 0.0;
    }

    let highlight_energy = highlight_mask(prev_near, threshold) * 0.56
        + highlight_mask(prev_mid, threshold) * 0.28
        + highlight_mask(prev_far, threshold) * 0.10
        + highlight_mask(center_luma, threshold) * 0.06;
    let contour_energy = max(prev_near - base_luma, 0.0) * 0.60
        + max(prev_mid - base_luma, 0.0) * 0.28
        + max(prev_far - base_luma, 0.0) * 0.12;
    return highlight_energy * contour_energy * amount * (1.0 - base_luma * 0.82);
}

fn degrade_luma(uv: vec2<f32>) -> f32 {
    let blur_px = max(effect.luma_degradation.x, 0.0);
    let center = sample_working_signal(uv).x;
    if (blur_px <= 1e-4) {
        return center;
    }

    let inv_size = frame_inv_size();
    let sample_step_px = max(0.5, blur_px * 0.55 + 0.45);
    let sample_step = vec2<f32>(sample_step_px * inv_size.x, 0.0);
    let left_far = sample_working_signal(uv - sample_step * 3.0).x;
    let left_outer = sample_working_signal(uv - sample_step * 2.0).x;
    let left_inner = sample_working_signal(uv - sample_step).x;
    let right_inner = sample_working_signal(uv + sample_step).x;
    let right_outer = sample_working_signal(uv + sample_step * 2.0).x;
    let right_far = sample_working_signal(uv + sample_step * 3.0).x;
    let low_luma = left_far * 0.06
        + left_outer * 0.12
        + left_inner * 0.18
        + center * 0.28
        + right_inner * 0.18
        + right_outer * 0.12
        + right_far * 0.06;
    let mid_luma = left_outer * 0.10
        + left_inner * 0.22
        + center * 0.36
        + right_inner * 0.22
        + right_outer * 0.10;
    let threshold = effect.luma_degradation.z;
    let bright_gate = highlight_mask(max(left_inner, center), threshold);
    let lag_mix = bright_gate * bandwidth_mix(blur_px) * 0.18;
    let lagged_low_luma = left_far * 0.09
        + left_outer * 0.16
        + left_inner * 0.22
        + center * 0.26
        + right_inner * 0.16
        + right_outer * 0.08
        + right_far * 0.03;
    let band_limited_luma = mix(low_luma, lagged_low_luma, lag_mix);
    let detail_mix = detail_recovery_mix();
    let micro_gain = clamp(1.0 - bandwidth_mix(blur_px) * (0.88 - detail_mix * 0.34), 0.10, 1.0)
        * (1.0 - bright_gate * (0.10 + bandwidth_mix(blur_px) * 0.08));
    let edge_gain = clamp(1.0 - bandwidth_mix(blur_px) * (0.46 - detail_mix * 0.20), 0.30, 1.0);
    let edge_band = mid_luma - band_limited_luma;
    let micro_band = center - mid_luma;
    let base_luma = clamp(
        band_limited_luma + edge_band * edge_gain + micro_band * micro_gain,
        0.0,
        1.0,
    );
    return clamp(
        base_luma + highlight_bleed(left_inner, left_outer, left_far, center, base_luma),
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
