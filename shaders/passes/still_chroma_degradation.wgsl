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
    return vec2<f32>(effect.frame.z, 1.0 / max(effect.frame.y, 1.0));
}

fn sample_chroma_px(pixel_pos: vec2<f32>) -> vec2<f32> {
    let frame_size = effect.frame.xy;
    let clamped = clamp(pixel_pos, vec2<f32>(0.0), frame_size - vec2<f32>(1.0, 1.0));
    let uv = (clamped + vec2<f32>(0.5, 0.5)) * frame_inv_size();
    return sample_working_signal(uv).yz;
}

fn sample_luma_px(pixel_pos: vec2<f32>) -> f32 {
    let frame_size = effect.frame.xy;
    let clamped = clamp(pixel_pos, vec2<f32>(0.0), frame_size - vec2<f32>(1.0, 1.0));
    let uv = (clamped + vec2<f32>(0.5, 0.5)) * frame_inv_size();
    return sample_working_signal(uv).x;
}

fn bandwidth_mix(blur_px: f32) -> f32 {
    return blur_px / (blur_px + 1.0);
}

fn local_luma_edge(pixel_pos: vec2<f32>) -> f32 {
    let left = sample_luma_px(pixel_pos - vec2<f32>(1.0, 0.0));
    let center = sample_luma_px(pixel_pos);
    let right = sample_luma_px(pixel_pos + vec2<f32>(1.0, 0.0));
    let edge_energy = max(abs(center - left), abs(center - right));
    return clamp(edge_energy * 2.8, 0.0, 1.0);
}

fn lowpass_chroma_line(pixel_pos: vec2<f32>, span_px: f32) -> vec2<f32> {
    let near_step_px = max(0.45, span_px * 0.42 + 0.30);
    let mid_step_px = max(near_step_px + 0.55, span_px * 0.95 + 0.55);
    let far_step_px = max(mid_step_px + 0.65, span_px * 1.55 + 0.85);
    let left_far = sample_chroma_px(pixel_pos - vec2<f32>(far_step_px, 0.0));
    let left_mid = sample_chroma_px(pixel_pos - vec2<f32>(mid_step_px, 0.0));
    let left_near = sample_chroma_px(pixel_pos - vec2<f32>(near_step_px, 0.0));
    let center = sample_chroma_px(pixel_pos);
    let right_near = sample_chroma_px(pixel_pos + vec2<f32>(near_step_px, 0.0));
    let right_mid = sample_chroma_px(pixel_pos + vec2<f32>(mid_step_px, 0.0));
    let right_far = sample_chroma_px(pixel_pos + vec2<f32>(far_step_px, 0.0));
    return left_far * 0.07
        + left_mid * 0.12
        + left_near * 0.18
        + center * 0.26
        + right_near * 0.18
        + right_mid * 0.12
        + right_far * 0.07;
}

fn sample_chroma_cell(pixel_pos: vec2<f32>, span_px: f32, cell_size_px: f32) -> vec2<f32> {
    let integration_step_px = max(0.35, cell_size_px * 0.24);
    let left = lowpass_chroma_line(
        pixel_pos - vec2<f32>(integration_step_px, 0.0),
        span_px * 1.02,
    );
    let center = lowpass_chroma_line(pixel_pos, span_px);
    let right = lowpass_chroma_line(
        pixel_pos + vec2<f32>(integration_step_px, 0.0),
        span_px * 1.02,
    );
    return left * 0.22 + center * 0.56 + right * 0.22;
}

fn quadratic_b_spline_weights(phase: f32) -> vec3<f32> {
    let clamped_phase = clamp(phase, 0.0, 1.0);
    let centered_phase = clamped_phase - 0.5;
    return vec3<f32>(
        0.5 * (1.0 - clamped_phase) * (1.0 - clamped_phase),
        0.75 - centered_phase * centered_phase,
        0.5 * clamped_phase * clamped_phase,
    );
}

fn reconstruct_chroma_line(
    pixel_pos: vec2<f32>,
    span_px: f32,
    cell_size_px: f32,
    chroma_offset_px: f32,
    smear_amount: f32,
    edge_guard: f32,
    chroma_bandwidth_mix: f32,
) -> vec2<f32> {
    // Reconstruct chroma from cell-integrated coarse samples so bandwidth loss
    // reads as reduced chroma resolution, then add a restrained trailing tail.
    let phase = pixel_pos.x / cell_size_px;
    let center_cell_index = floor(phase);
    let center_x = (center_cell_index + 0.5) * cell_size_px;
    let prev = sample_chroma_cell(
        vec2<f32>(center_x - cell_size_px, pixel_pos.y),
        span_px,
        cell_size_px,
    );
    let center = sample_chroma_cell(vec2<f32>(center_x, pixel_pos.y), span_px, cell_size_px);
    let next = sample_chroma_cell(
        vec2<f32>(center_x + cell_size_px, pixel_pos.y),
        span_px,
        cell_size_px,
    );
    let weights = quadratic_b_spline_weights(fract(phase));
    let reconstructed = prev * weights.x + center * weights.y + next * weights.z;

    var smear_direction = 1.0;
    if (abs(chroma_offset_px) > 1e-4) {
        smear_direction = sign(chroma_offset_px);
    }
    let smear_near = sample_chroma_cell(
        vec2<f32>(center_x - smear_direction * cell_size_px, pixel_pos.y),
        span_px * 1.10,
        cell_size_px,
    );
    let smear_far = sample_chroma_cell(
        vec2<f32>(center_x - smear_direction * cell_size_px * 2.0, pixel_pos.y),
        span_px * 1.25,
        cell_size_px,
    );
    let contamination = center * 0.60 + smear_near * 0.28 + smear_far * 0.12;
    let smeared = reconstructed * 0.76 + contamination * 0.24;
    let restrained_smear = smear_amount * (1.0 - edge_guard * (0.22 + chroma_bandwidth_mix * 0.18));
    return mix(reconstructed, smeared, restrained_smear);
}

fn degrade_chroma(uv: vec2<f32>) -> vec2<f32> {
    let chroma_offset_px = effect.chroma_degradation.x;
    let chroma_blur_px = max(effect.chroma_degradation.y, 0.0);
    let pixel_pos = uv * effect.frame.xy - vec2<f32>(0.5, 0.5);
    let chroma_center = pixel_pos + vec2<f32>(chroma_offset_px, 0.0);
    let edge_guard = local_luma_edge(pixel_pos);
    if (chroma_blur_px <= 1e-4) {
        let chroma_base = sample_chroma_px(chroma_center);
        let chroma_up = sample_chroma_px(chroma_center - vec2<f32>(0.0, 1.0));
        let chroma_down = sample_chroma_px(chroma_center + vec2<f32>(0.0, 1.0));
        let chroma_vertical = chroma_up * 0.18 + chroma_base * 0.64 + chroma_down * 0.18;
        return mix(
            chroma_base,
            chroma_vertical,
            effect.chroma_degradation.w,
        ) * effect.chroma_degradation.z;
    }

    let chroma_bandwidth_mix = bandwidth_mix(chroma_blur_px);
    let delay_mix = abs(chroma_offset_px) / (abs(chroma_offset_px) + chroma_blur_px * 0.5 + 0.35);
    let lowpass_span_px = 0.40 + chroma_blur_px * 0.72 + chroma_bandwidth_mix * 0.28;
    let cell_size_px = 1.0 + chroma_blur_px * 0.52 + chroma_bandwidth_mix * 0.38;
    let smear_amount = clamp(0.08 + chroma_bandwidth_mix * 0.14 + delay_mix * 0.05, 0.0, 0.27);
    let vertical_neighbor_weight = mix(0.18, 0.24, chroma_bandwidth_mix);
    let vertical_center_weight = 1.0 - vertical_neighbor_weight * 2.0;
    let chroma_line = reconstruct_chroma_line(
        chroma_center,
        lowpass_span_px,
        cell_size_px,
        chroma_offset_px,
        smear_amount,
        edge_guard,
        chroma_bandwidth_mix,
    );
    let chroma_up = reconstruct_chroma_line(
        chroma_center - vec2<f32>(0.0, 1.0),
        lowpass_span_px,
        cell_size_px,
        chroma_offset_px,
        smear_amount,
        edge_guard,
        chroma_bandwidth_mix,
    );
    let chroma_down = reconstruct_chroma_line(
        chroma_center + vec2<f32>(0.0, 1.0),
        lowpass_span_px,
        cell_size_px,
        chroma_offset_px,
        smear_amount,
        edge_guard,
        chroma_bandwidth_mix,
    );
    let chroma_vertical = chroma_up * vertical_neighbor_weight
        + chroma_line * vertical_center_weight
        + chroma_down * vertical_neighbor_weight;
    return mix(
        chroma_line,
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
