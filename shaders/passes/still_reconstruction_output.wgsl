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

struct ProceduralSeed {
    noise_coord: vec2<f32>,
};

struct ReconstructionSignal {
    luma: f32,
    chroma: vec2<f32>,
};

struct DropoutApproximation {
    signal: ReconstructionSignal,
    dropout_mix: f32,
};

struct ReconstructionContamination {
    luma: f32,
    chroma: vec2<f32>,
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

fn centered_hash(p: vec2<f32>) -> f32 {
    return hash12(p) - 0.5;
}

fn smooth_noise_x(noise_coord: vec2<f32>, cells_per_px: f32, seed: vec2<f32>) -> f32 {
    let phase = noise_coord.x * cells_per_px;
    let cell = floor(phase);
    let blend = fract(phase);
    let smooth_blend = blend * blend * (3.0 - 2.0 * blend);
    let line_phase = noise_coord.y * seed.y + effect.frame.w + seed.x * 1.37;
    let left = centered_hash(vec2<f32>(cell + seed.x, line_phase));
    let right = centered_hash(vec2<f32>(cell + seed.x + 1.0, line_phase));
    return mix(left, right, smooth_blend);
}

fn sample_luma(uv: vec2<f32>) -> f32 {
    let clamped = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(luma_texture, signal_sampler, clamped).x;
}

fn sample_chroma(uv: vec2<f32>) -> vec2<f32> {
    let clamped = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(chroma_texture, signal_sampler, clamped).xy;
}

fn sample_reconstruction_signal(uv: vec2<f32>) -> ReconstructionSignal {
    var signal: ReconstructionSignal;
    signal.luma = sample_luma(uv);
    signal.chroma = sample_chroma(uv);
    return signal;
}

fn frame_inv_size() -> vec2<f32> {
    return vec2<f32>(effect.frame.z, 1.0 / max(effect.frame.y, 1.0));
}

// The final pass does not resample the signal through transport again.
// It only reuses the same resolved scan-line phase as a procedural seed so
// noise and dropout stay coherently anchored to the conditioned signal.
fn procedural_seed_from_conditioned_phase(uv: vec2<f32>) -> ProceduralSeed {
    let frame_size = effect.frame.xy;
    let inv_size = frame_inv_size();
    let line_index = floor(uv.y * frame_size.y + effect.input_conditioning.w);
    let line_phase = line_index + effect.frame.w * 0.5;
    let line_jitter = sin(line_phase * 0.37) * effect.input_conditioning.z * inv_size.x;
    let sample_uv = vec2<f32>(
        uv.x + line_jitter,
        uv.y + effect.input_conditioning.w * inv_size.y,
    );

    var seed: ProceduralSeed;
    seed.noise_coord = vec2<f32>(
        floor(sample_uv.x * frame_size.x),
        floor(sample_uv.y * frame_size.y),
    );
    return seed;
}

fn sample_reconstruction_contamination(
    noise_coord: vec2<f32>,
    signal: ReconstructionSignal,
    dropout_mix: f32,
) -> ReconstructionContamination {
    let frame_index = effect.frame.w;
    let clamped_luma = clamp(signal.luma, 0.0, 1.0);

    var contamination: ReconstructionContamination;
    contamination.luma = 0.0;
    contamination.chroma = vec2<f32>(0.0, 0.0);

    if (effect.reconstruction_output.x > 1e-5) {
        let luma_visibility = 0.35 + 0.65 * pow(1.0 - clamped_luma, 0.7);
        let luma_fine = centered_hash(noise_coord + vec2<f32>(frame_index, 3.0));
        let luma_band = smooth_noise_x(
            noise_coord + vec2<f32>(0.0, 17.0),
            0.12,
            vec2<f32>(11.0, 0.31),
        );
        let luma_line = centered_hash(vec2<f32>(noise_coord.y + 29.0, frame_index + 13.0));
        let luma_dropout_scale = mix(1.0, 0.72, dropout_mix);
        contamination.luma = (luma_fine * 0.45 + luma_band * 0.35 + luma_line * 0.20)
            * effect.reconstruction_output.x
            * luma_visibility
            * luma_dropout_scale;
    }

    if (effect.reconstruction_output.y > 1e-5) {
        let chroma_band_u = smooth_noise_x(
            noise_coord + vec2<f32>(0.0, 41.0),
            0.08,
            vec2<f32>(47.0, 0.23),
        );
        let chroma_band_v = smooth_noise_x(
            noise_coord + vec2<f32>(0.0, 67.0),
            0.06,
            vec2<f32>(71.0, 0.19),
        );
        let chroma_line_u =
            centered_hash(vec2<f32>(noise_coord.y * 0.5 + 97.0, frame_index + 23.0));
        let chroma_line_v =
            centered_hash(vec2<f32>(noise_coord.y * 0.5 + 131.0, frame_index + 31.0));
        let chroma_visibility = 0.55 + 0.25 * pow(1.0 - clamped_luma, 0.5);
        let chroma_dropout_scale = mix(1.0, 0.45, dropout_mix);
        let chroma_additive = vec2<f32>(
            chroma_band_u * 0.72 + chroma_line_u * 0.28,
            chroma_band_v * 0.72 + chroma_line_v * 0.28,
        ) * effect.reconstruction_output.y
            * chroma_visibility
            * chroma_dropout_scale;
        let phase_like = centered_hash(vec2<f32>(
            floor(noise_coord.x * 0.14) + noise_coord.y * 0.12 + 149.0,
            frame_index + 37.0,
        ));
        let chroma_phase = vec2<f32>(-signal.chroma.y, signal.chroma.x)
            * (phase_like * effect.reconstruction_output.y * 0.45 * chroma_dropout_scale);
        contamination.chroma = chroma_additive + chroma_phase;
    }

    return contamination;
}

fn line_dropout_mask(noise_coord: vec2<f32>) -> f32 {
    let probability = effect.reconstruction_aux.x;
    let mean_span_px = effect.reconstruction_aux.y;
    if (probability <= 1e-5 || mean_span_px <= 1e-5) {
        return 0.0;
    }

    let frame_index = effect.frame.w;
    let line_index = noise_coord.y;
    let line_seed = hash12(vec2<f32>(line_index + 17.0, frame_index + 5.0));
    if (line_seed >= probability) {
        return 0.0;
    }

    let span_scale = mix(
        0.6,
        1.8,
        hash12(vec2<f32>(line_index + 41.0, frame_index + 9.0)),
    );
    let span_px = max(1.0, mean_span_px * span_scale);
    let center_px = hash12(vec2<f32>(line_index + 59.0, frame_index + 21.0)) * effect.frame.x;
    let edge_softness = max(0.75, span_px * 0.2);
    let distance_px = abs(noise_coord.x - center_px);
    let segment = 1.0
        - smoothstep(span_px * 0.5, span_px * 0.5 + edge_softness, distance_px);
    let breakup = mix(
        0.82,
        1.0,
        hash12(vec2<f32>(floor(noise_coord.x * 0.35) + line_index, frame_index + 37.0)),
    );
    return segment * breakup;
}

fn apply_dropout_approximation(
    uv: vec2<f32>,
    noise_coord: vec2<f32>,
    base_signal: ReconstructionSignal,
) -> DropoutApproximation {
    var dropout: DropoutApproximation;
    dropout.signal = base_signal;
    dropout.dropout_mix = 0.0;

    let mask = line_dropout_mask(noise_coord);
    if (mask <= 1e-4) {
        return dropout;
    }

    let inv_size = frame_inv_size();
    let conceal_up_uv = uv - vec2<f32>(0.0, inv_size.y);
    let conceal_down_uv = uv + vec2<f32>(0.0, inv_size.y);
    let concealed_up = sample_reconstruction_signal(conceal_up_uv);
    let concealed_down = sample_reconstruction_signal(conceal_down_uv);
    var concealed_signal: ReconstructionSignal;
    concealed_signal.luma = concealed_up.luma * 0.55 + concealed_down.luma * 0.45;
    concealed_signal.chroma = concealed_up.chroma * 0.55 + concealed_down.chroma * 0.45;
    let line_strength = mix(
        0.35,
        0.72,
        hash12(vec2<f32>(noise_coord.y + 73.0, effect.frame.w + 11.0)),
    );
    dropout.dropout_mix = mask * line_strength;
    let dropout_luma_noise =
        (hash12(noise_coord + vec2<f32>(effect.frame.w, 29.0)) - 0.5)
            * dropout.dropout_mix
            * 0.08;
    dropout.signal.luma = clamp(
        mix(base_signal.luma, concealed_signal.luma, dropout.dropout_mix)
            + dropout.dropout_mix * 0.05
            + dropout_luma_noise,
        0.0,
        1.0,
    );
    let concealed_chroma_support = 0.35 * mix(1.0, 0.75, dropout.dropout_mix);
    dropout.signal.chroma = mix(
        base_signal.chroma,
        concealed_signal.chroma * concealed_chroma_support,
        dropout.dropout_mix * 0.85,
    );
    return dropout;
}

fn y_c_leakage_luma(chroma_signal: vec2<f32>, dropout_mix: f32) -> f32 {
    let dropout_scale = mix(1.0, 0.85, dropout_mix);
    return dot(chroma_signal, vec2<f32>(0.10, -0.05))
        * effect.reconstruction_output.z
        * dropout_scale;
}

fn compose_display_yuv(
    reconstructed_signal: ReconstructionSignal,
    contamination: ReconstructionContamination,
    dropout_mix: f32,
) -> vec3<f32> {
    let reconstructed_y = clamp(
        reconstructed_signal.luma
            + y_c_leakage_luma(reconstructed_signal.chroma, dropout_mix)
            + contamination.luma,
        0.0,
        1.0,
    );
    let reconstructed_chroma = reconstructed_signal.chroma + contamination.chroma;
    return vec3<f32>(reconstructed_y, reconstructed_chroma.x, reconstructed_chroma.y);
}

fn decode_output_rgb(display_yuv: vec3<f32>) -> vec3<f32> {
    return clamp(yuv_to_rgb(display_yuv), vec3<f32>(0.0), vec3<f32>(1.0));
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
    let seed = procedural_seed_from_conditioned_phase(in.uv);
    let base_signal = sample_reconstruction_signal(in.uv);
    let dropout = apply_dropout_approximation(
        in.uv,
        seed.noise_coord,
        base_signal,
    );
    let contamination = sample_reconstruction_contamination(
        seed.noise_coord,
        dropout.signal,
        dropout.dropout_mix,
    );
    let display_yuv = compose_display_yuv(dropout.signal, contamination, dropout.dropout_mix);
    let rgb = decode_output_rgb(display_yuv);
    return vec4<f32>(rgb, 1.0);
}
