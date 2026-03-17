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

fn lowpass_chroma_line(pixel_pos: vec2<f32>, span_px: f32) -> vec2<f32> {
    let inner_step_px = max(0.65, span_px * 0.55 + 0.35);
    let outer_step_px = max(inner_step_px + 0.75, span_px * 1.35 + 0.75);
    let left_outer = sample_chroma_px(pixel_pos - vec2<f32>(outer_step_px, 0.0));
    let left_inner = sample_chroma_px(pixel_pos - vec2<f32>(inner_step_px, 0.0));
    let center = sample_chroma_px(pixel_pos);
    let right_inner = sample_chroma_px(pixel_pos + vec2<f32>(inner_step_px, 0.0));
    let right_outer = sample_chroma_px(pixel_pos + vec2<f32>(outer_step_px, 0.0));
    return left_outer * 0.12
        + left_inner * 0.23
        + center * 0.30
        + right_inner * 0.23
        + right_outer * 0.12;
}

fn reconstruct_chroma_line(
    pixel_pos: vec2<f32>,
    span_px: f32,
    cell_size_px: f32,
    chroma_offset_px: f32,
    smear_amount: f32,
) -> vec2<f32> {
    // Reconstruct chroma from a coarser horizontal grid so bandwidth loss reads
    // as reduced color resolution and smear, not as an RGB-split effect.
    let phase = pixel_pos.x / cell_size_px;
    let left_cell_index = floor(phase - 0.5);
    let left_center_x = (left_cell_index + 0.5) * cell_size_px;
    let right_center_x = left_center_x + cell_size_px;
    let cell_mix = smoothstep(
        0.15,
        0.85,
        clamp((pixel_pos.x - left_center_x) / cell_size_px, 0.0, 1.0),
    );
    let left = lowpass_chroma_line(vec2<f32>(left_center_x, pixel_pos.y), span_px);
    let right = lowpass_chroma_line(vec2<f32>(right_center_x, pixel_pos.y), span_px);
    let reconstructed = mix(left, right, cell_mix);

    var smear_direction = 0.0;
    if (abs(chroma_offset_px) > 1e-4) {
        smear_direction = sign(chroma_offset_px);
    }
    let smear_shift_px = smear_direction
        * min(cell_size_px * 0.5, abs(chroma_offset_px) + span_px * 0.25);
    let smear_source = lowpass_chroma_line(
        vec2<f32>(pixel_pos.x - smear_shift_px, pixel_pos.y),
        span_px * 1.15,
    );
    let smeared = reconstructed * 0.78 + smear_source * 0.22;
    return mix(reconstructed, smeared, smear_amount);
}

fn degrade_chroma(uv: vec2<f32>) -> vec2<f32> {
    let chroma_offset_px = effect.chroma_degradation.x;
    let chroma_blur_px = max(effect.chroma_degradation.y, 0.0);
    let pixel_pos = uv * effect.frame.xy - vec2<f32>(0.5, 0.5);
    let chroma_center = pixel_pos + vec2<f32>(chroma_offset_px, 0.0);
    if (chroma_blur_px <= 1e-4) {
        let chroma_base = sample_chroma_px(chroma_center);
        let chroma_up = sample_chroma_px(chroma_center - vec2<f32>(0.0, 1.0));
        let chroma_down = sample_chroma_px(chroma_center + vec2<f32>(0.0, 1.0));
        let chroma_vertical = chroma_up * 0.25 + chroma_base * 0.5 + chroma_down * 0.25;
        return mix(
            chroma_base,
            chroma_vertical,
            effect.chroma_degradation.w,
        ) * effect.chroma_degradation.z;
    }

    let bandwidth_mix = chroma_blur_px / (chroma_blur_px + 1.0);
    let lowpass_span_px = chroma_blur_px * 0.85 + 0.35;
    let cell_size_px = 1.0 + chroma_blur_px * 0.65;
    let smear_amount = clamp(0.10 + bandwidth_mix * 0.16, 0.0, 0.26);
    let chroma_line = reconstruct_chroma_line(
        chroma_center,
        lowpass_span_px,
        cell_size_px,
        chroma_offset_px,
        smear_amount,
    );
    let chroma_up = reconstruct_chroma_line(
        chroma_center - vec2<f32>(0.0, 1.0),
        lowpass_span_px,
        cell_size_px,
        chroma_offset_px,
        smear_amount,
    );
    let chroma_down = reconstruct_chroma_line(
        chroma_center + vec2<f32>(0.0, 1.0),
        lowpass_span_px,
        cell_size_px,
        chroma_offset_px,
        smear_amount,
    );
    let chroma_vertical = chroma_up * 0.25 + chroma_line * 0.5 + chroma_down * 0.25;
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
