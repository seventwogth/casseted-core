struct EffectUniform {
    frame: vec4<f32>,
    // (horizontal reference scale, vertical reference scale, unused, unused)
    reference: vec4<f32>,
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
    // Output-pixel coordinate, used where horizontal position matters.
    noise_coord: vec2<f32>,
    // Same coordinate with the row replaced by its reference-line index, used
    // by every per-line hash so line identity does not multiply with the
    // output row count.
    reference_coord: vec2<f32>,
};

struct ReconstructionSignal {
    luma: f32,
    chroma: vec2<f32>,
};

struct DropoutApproximation {
    signal: ReconstructionSignal,
    dropout_mix: f32,
};

struct HeadSwitchingApproximation {
    signal: ReconstructionSignal,
    switching_mix: f32,
};

struct ReconstructionContamination {
    luma: f32,
    chroma: vec2<f32>,
};

struct QuietRegionProfile {
    quiet_luma: f32,
    quiet_chroma: f32,
    quiet_mix: f32,
    dark_mix: f32,
};

@group(0) @binding(0) var luma_texture: texture_2d<f32>;
@group(0) @binding(1) var chroma_texture: texture_2d<f32>;
@group(0) @binding(2) var signal_sampler: sampler;
@group(0) @binding(3) var<uniform> effect: EffectUniform;

// Same reference-raster factors the branch passes use. Contamination
// structure, dropout, head switching, and the quiet-region probe are all
// specified on the reference raster, so they are resolved against the real
// frame here instead of being left in absolute output pixels.
//
// The vertical factor comes from the standard's active line count, which is
// why line-oriented terms below index reference lines rather than output rows:
// a dropout probability stated per line must not fire more often just because
// the raster has more rows.
fn reference_scale() -> f32 {
    return max(effect.reference.x, 1.0);
}

fn vertical_reference_scale() -> f32 {
    return max(effect.reference.y, 1.0);
}

// The decode edge uses the same fixed BT.601-like working matrix as the input
// conditioning pass. `output_transfer` remains deferred after this inverse
// matrix + clamp step.
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

// Takes a reference coordinate: `x` in output pixels, `y` already reduced to a
// reference-line index. `cells_per_px` is stated in cells per reference pixel,
// so a band keeps the same number of cycles across the frame at any output
// width, and its line-to-line variation keeps the same rate at any height.
fn smooth_noise_x(noise_coord: vec2<f32>, cells_per_px: f32, seed: vec2<f32>) -> f32 {
    let phase = noise_coord.x * cells_per_px / reference_scale();
    let cell = floor(phase);
    let blend = fract(phase);
    let smooth_blend = blend * blend * (3.0 - 2.0 * blend);
    let line_phase = noise_coord.y * seed.y + effect.frame.w + seed.x * 1.37;
    let left = centered_hash(vec2<f32>(cell + seed.x, line_phase));
    let right = centered_hash(vec2<f32>(cell + seed.x + 1.0, line_phase));
    return mix(left, right, smooth_blend);
}

// Per-pixel hash noise carries no band limit of its own, so above the
// reference raster it would sit finer than the modelled luma bandwidth allows.
// Sampling it on the reference grid keeps its relative frequency fixed.
//
// Horizontally it interpolates; vertically it holds one value per reference
// line, which makes the finest carrier line-correlated on tall rasters rather
// than isotropic. That matches how analog line noise reads and avoids the hard
// vertical blocking a nearest-neighbour grid in both axes would produce.
//
// Takes a reference coordinate. At the reference raster the interpolation
// weight is exactly zero, so this reduces to the plain per-pixel hash.
fn reference_scaled_fine_noise(noise_coord: vec2<f32>, seed: vec2<f32>) -> f32 {
    let phase = noise_coord.x / reference_scale();
    let cell = floor(phase);
    let blend = fract(phase);
    let smooth_blend = blend * blend * (3.0 - 2.0 * blend);
    let left = centered_hash(vec2<f32>(cell, noise_coord.y) + seed);
    let right = centered_hash(vec2<f32>(cell + 1.0, noise_coord.y) + seed);
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

fn rotate_chroma(chroma: vec2<f32>, angle_rad: f32) -> vec2<f32> {
    if (abs(angle_rad) <= 1e-5) {
        return chroma;
    }

    let sin_angle = sin(angle_rad);
    let cos_angle = cos(angle_rad);
    return vec2<f32>(
        chroma.x * cos_angle - chroma.y * sin_angle,
        chroma.x * sin_angle + chroma.y * cos_angle,
    );
}

fn sample_reconstruction_signal(uv: vec2<f32>) -> ReconstructionSignal {
    var signal: ReconstructionSignal;
    signal.luma = sample_luma(uv);
    signal.chroma = sample_chroma(uv);
    return signal;
}

fn quiet_region_profile(
    uv: vec2<f32>,
    signal: ReconstructionSignal,
) -> QuietRegionProfile {
    // The gradient thresholds below are calibrated against reference-raster
    // neighbours, so the probe steps by the reference factor on each axis.
    let probe_step =
        frame_inv_size() * vec2<f32>(reference_scale(), vertical_reference_scale());
    let left = sample_reconstruction_signal(uv - vec2<f32>(probe_step.x, 0.0));
    let right = sample_reconstruction_signal(uv + vec2<f32>(probe_step.x, 0.0));
    let up = sample_reconstruction_signal(uv - vec2<f32>(0.0, probe_step.y));
    let down = sample_reconstruction_signal(uv + vec2<f32>(0.0, probe_step.y));

    let luma_gradient = max(
        max(abs(signal.luma - left.luma), abs(signal.luma - right.luma)),
        max(abs(signal.luma - up.luma), abs(signal.luma - down.luma)),
    );
    let chroma_gradient = max(
        max(length(signal.chroma - left.chroma), length(signal.chroma - right.chroma)),
        max(length(signal.chroma - up.chroma), length(signal.chroma - down.chroma)),
    );

    var profile: QuietRegionProfile;
    profile.dark_mix = 1.0 - smoothstep(0.16, 0.52, clamp(signal.luma, 0.0, 1.0));
    profile.quiet_luma = 1.0 - smoothstep(0.020, 0.110, luma_gradient);
    profile.quiet_chroma = 1.0 - smoothstep(0.015, 0.070, chroma_gradient);
    profile.quiet_mix = profile.quiet_luma
        * mix(0.78, 1.0, profile.quiet_chroma)
        * mix(0.82, 1.0, profile.dark_mix);
    return profile;
}

fn frame_inv_size() -> vec2<f32> {
    return vec2<f32>(
        1.0 / max(effect.frame.x, 1.0),
        1.0 / max(effect.frame.y, 1.0),
    );
}

// The final pass does not resample the signal through transport again.
// It only reuses the same resolved scan-line phase as a procedural seed so
// noise and dropout stay coherently anchored to the conditioned signal.
fn procedural_seed_from_conditioned_phase(uv: vec2<f32>) -> ProceduralSeed {
    let frame_size = effect.frame.xy;
    let inv_size = frame_inv_size();
    let vertical_scale = vertical_reference_scale();
    let vertical_offset_px = effect.input_conditioning.w * vertical_scale;
    let line_index = floor(uv.y * frame_size.y + vertical_offset_px);
    let line_phase = line_index / vertical_scale + effect.frame.w * 0.5;
    let line_jitter = sin(line_phase * 0.37) * effect.input_conditioning.z * inv_size.x;
    let sample_uv = vec2<f32>(
        uv.x + line_jitter,
        uv.y + vertical_offset_px * inv_size.y,
    );

    var seed: ProceduralSeed;
    seed.noise_coord = vec2<f32>(
        floor(sample_uv.x * frame_size.x),
        floor(sample_uv.y * frame_size.y),
    );
    seed.reference_coord = vec2<f32>(
        seed.noise_coord.x,
        floor(seed.noise_coord.y / vertical_scale),
    );
    return seed;
}

fn apply_head_switching_approximation(
    uv: vec2<f32>,
    seed: ProceduralSeed,
    base_signal: ReconstructionSignal,
) -> HeadSwitchingApproximation {
    var switching: HeadSwitchingApproximation;
    switching.signal = base_signal;
    switching.switching_mix = 0.0;

    // The formal band height is stated in lines, so it covers the same share
    // of the picture regardless of how many output rows the raster has.
    let vertical_scale = vertical_reference_scale();
    let band_lines = effect.frame.z * vertical_scale;
    let offset_px = effect.reconstruction_output.w;
    if (band_lines <= 1e-5 || abs(offset_px) <= 1e-5) {
        return switching;
    }

    let band_top = max(0.0, effect.frame.y - band_lines);
    let line_in_band = seed.noise_coord.y - band_top;
    if (line_in_band < 0.0) {
        return switching;
    }

    let band_progress =
        clamp((line_in_band + 0.5) / max(band_lines, 1.0), 0.0, 1.0);
    let line_breakup = mix(
        0.82,
        1.0,
        hash12(vec2<f32>(seed.reference_coord.y + 173.0, effect.frame.w + 19.0)),
    );
    let band_mix = band_progress * band_progress * line_breakup;
    let seam_mix =
        (1.0 - smoothstep(0.0, 1.5 * vertical_scale, line_in_band)) * line_breakup;
    let inv_size = frame_inv_size();
    let shifted_signal = sample_reconstruction_signal(
        uv + vec2<f32>(offset_px * inv_size.x, 0.0),
    );
    let luma_shift_mix = band_mix * 0.18 + seam_mix * 0.22;
    let chroma_shift_mix = band_mix * 0.28 + seam_mix * 0.18;
    let seam_luma_noise = centered_hash(
        vec2<f32>(
            floor(seed.noise_coord.x * 0.25 / reference_scale()) + 191.0,
            seed.reference_coord.y + effect.frame.w + 13.0,
        ),
    ) * seam_mix * 0.025;
    let chroma_support = 1.0 - band_mix * 0.28 - seam_mix * 0.12;

    switching.switching_mix = max(luma_shift_mix, chroma_shift_mix);
    switching.signal.luma = clamp(
        mix(base_signal.luma, shifted_signal.luma, luma_shift_mix) + seam_luma_noise,
        0.0,
        1.0,
    );
    switching.signal.chroma = mix(
        base_signal.chroma,
        shifted_signal.chroma * chroma_support,
        chroma_shift_mix,
    );
    return switching;
}

fn sample_reconstruction_contamination(
    uv: vec2<f32>,
    reference_coord: vec2<f32>,
    signal: ReconstructionSignal,
    disturbance_mix: f32,
) -> ReconstructionContamination {
    let frame_index = effect.frame.w;
    let clamped_luma = clamp(signal.luma, 0.0, 1.0);
    let chroma_disturbance_scale = mix(1.0, 0.45, disturbance_mix);
    let quiet_profile = quiet_region_profile(uv, signal);
    let quiet_luma_surface_mix =
        quiet_profile.quiet_luma * (0.35 + 0.65 * quiet_profile.dark_mix);
    let quiet_surface_mix =
        quiet_profile.quiet_mix * (0.35 + 0.65 * quiet_profile.dark_mix);

    var contamination: ReconstructionContamination;
    contamination.luma = 0.0;
    contamination.chroma = vec2<f32>(0.0, 0.0);

    if (effect.reconstruction_output.x > 1e-5) {
        let luma_visibility = 0.35 + 0.65 * pow(1.0 - clamped_luma, 0.7);
        let luma_fine = reference_scaled_fine_noise(reference_coord, vec2<f32>(frame_index, 3.0));
        let luma_band = smooth_noise_x(
            reference_coord + vec2<f32>(0.0, 17.0),
            0.12,
            vec2<f32>(11.0, 0.31),
        );
        let luma_line = centered_hash(vec2<f32>(reference_coord.y + 29.0, frame_index + 13.0));
        let luma_surface_band = smooth_noise_x(
            reference_coord + vec2<f32>(0.0, 109.0),
            0.045,
            vec2<f32>(107.0, 0.13),
        );
        let luma_surface_drift = smooth_noise_x(
            reference_coord + vec2<f32>(0.0, 149.0),
            0.018,
            vec2<f32>(149.0, 0.09),
        );
        let luma_surface_line = centered_hash(
            vec2<f32>(reference_coord.y * 0.22 + 173.0, frame_index + 59.0),
        );
        let luma_disturbance_scale = mix(1.0, 0.72, disturbance_mix);
        let quiet_luma_carrier = luma_surface_band * 0.52
            + luma_surface_drift * 0.32
            + luma_surface_line * 0.16;
        let luma_texture = luma_fine * mix(0.45, 0.22, quiet_luma_surface_mix)
            + luma_band * mix(0.35, 0.44, quiet_luma_surface_mix)
            + luma_line * mix(0.20, 0.16, quiet_luma_surface_mix);
        let quiet_luma_additive = quiet_luma_carrier
            * quiet_luma_surface_mix
            * (0.26 + 0.24 * quiet_profile.dark_mix);
        contamination.luma = (luma_texture + quiet_luma_additive)
            * effect.reconstruction_output.x
            * luma_visibility
            * luma_disturbance_scale;
    }

    if (effect.reconstruction_output.y > 1e-5) {
        let chroma_band_u = smooth_noise_x(
            reference_coord + vec2<f32>(0.0, 41.0),
            0.08,
            vec2<f32>(47.0, 0.23),
        );
        let chroma_band_v = smooth_noise_x(
            reference_coord + vec2<f32>(0.0, 67.0),
            0.06,
            vec2<f32>(71.0, 0.19),
        );
        let chroma_line_u =
            centered_hash(vec2<f32>(reference_coord.y * 0.5 + 97.0, frame_index + 23.0));
        let chroma_line_v =
            centered_hash(vec2<f32>(reference_coord.y * 0.5 + 131.0, frame_index + 31.0));
        let chroma_surface_u = smooth_noise_x(
            reference_coord + vec2<f32>(0.0, 181.0),
            0.035,
            vec2<f32>(181.0, 0.11),
        );
        let chroma_surface_v = smooth_noise_x(
            reference_coord + vec2<f32>(0.0, 211.0),
            0.028,
            vec2<f32>(211.0, 0.15),
        );
        let chroma_surface_line_u =
            centered_hash(vec2<f32>(reference_coord.y * 0.20 + 227.0, frame_index + 67.0));
        let chroma_surface_line_v =
            centered_hash(vec2<f32>(reference_coord.y * 0.18 + 257.0, frame_index + 79.0));
        let chroma_visibility = 0.55 + 0.25 * pow(1.0 - clamped_luma, 0.5);
        let quiet_chroma_mix =
            quiet_profile.quiet_mix * (0.12 + 0.42 * quiet_profile.dark_mix);
        let chroma_texture = vec2<f32>(
            chroma_band_u * 0.72 + chroma_line_u * 0.28,
            chroma_band_v * 0.72 + chroma_line_v * 0.28,
        );
        let chroma_surface = vec2<f32>(
            chroma_surface_u * 0.74 + chroma_surface_line_u * 0.26,
            chroma_surface_v * 0.74 + chroma_surface_line_v * 0.26,
        );
        let chroma_additive = (chroma_texture + chroma_surface * quiet_chroma_mix * 0.12)
            * effect.reconstruction_output.y
            * chroma_visibility
            * chroma_disturbance_scale;
        contamination.chroma = chroma_additive;
    }

    if (effect.reconstruction_aux.w > 1e-5) {
        // Keep phase noise in Y/C space so the instability reads like chroma
        // decode wobble instead of spatial RGB splitting.
        let phase_band = smooth_noise_x(
            reference_coord + vec2<f32>(0.0, 91.0),
            0.05,
            vec2<f32>(89.0, 0.17),
        );
        let phase_line =
            centered_hash(vec2<f32>(reference_coord.y * 0.35 + 157.0, frame_index + 43.0));
        let phase_perturbation = (phase_band * 0.74 + phase_line * 0.26)
            * effect.reconstruction_aux.w
            * chroma_disturbance_scale
            * (1.0 + quiet_surface_mix * 0.18);
        let rotated_chroma = rotate_chroma(signal.chroma, phase_perturbation);
        contamination.chroma = contamination.chroma + (rotated_chroma - signal.chroma);
    }

    return contamination;
}

fn line_dropout_mask(seed: ProceduralSeed) -> f32 {
    let probability = effect.reconstruction_aux.x;
    let mean_span_px = effect.reconstruction_aux.y;
    if (probability <= 1e-5 || mean_span_px <= 1e-5) {
        return 0.0;
    }

    let frame_index = effect.frame.w;
    // The probability is stated per line, so it is drawn once per reference
    // line. Drawing per output row would fire proportionally more dropouts on
    // a taller raster.
    let noise_coord = seed.noise_coord;
    let line_index = seed.reference_coord.y;
    let line_seed = hash12(vec2<f32>(line_index + 17.0, frame_index + 5.0));
    if (line_seed >= probability) {
        return 0.0;
    }

    let span_scale = mix(
        0.6,
        1.8,
        hash12(vec2<f32>(line_index + 41.0, frame_index + 9.0)),
    );
    let scale = reference_scale();
    let span_px = max(1.0 * scale, mean_span_px * span_scale);
    let center_px = hash12(vec2<f32>(line_index + 59.0, frame_index + 21.0)) * effect.frame.x;
    let edge_softness = max(0.75 * scale, span_px * 0.2);
    let distance_px = abs(noise_coord.x - center_px);
    let segment = 1.0
        - smoothstep(span_px * 0.5, span_px * 0.5 + edge_softness, distance_px);
    let breakup = mix(
        0.82,
        1.0,
        hash12(vec2<f32>(
            floor(noise_coord.x * 0.35 / scale) + line_index,
            frame_index + 37.0,
        )),
    );
    return segment * breakup;
}

fn apply_dropout_approximation(
    uv: vec2<f32>,
    seed: ProceduralSeed,
    base_signal: ReconstructionSignal,
) -> DropoutApproximation {
    var dropout: DropoutApproximation;
    dropout.signal = base_signal;
    dropout.dropout_mix = 0.0;

    let mask = line_dropout_mask(seed);
    if (mask <= 1e-4) {
        return dropout;
    }

    // Concealment pulls from the neighbouring scan lines, so it steps by the
    // reference-line spacing rather than by one output row.
    let inv_size = frame_inv_size();
    let line_step = inv_size.y * vertical_reference_scale();
    let conceal_up_uv = uv - vec2<f32>(0.0, line_step);
    let conceal_down_uv = uv + vec2<f32>(0.0, line_step);
    let concealed_up = sample_reconstruction_signal(conceal_up_uv);
    let concealed_down = sample_reconstruction_signal(conceal_down_uv);
    var concealed_signal: ReconstructionSignal;
    concealed_signal.luma = concealed_up.luma * 0.55 + concealed_down.luma * 0.45;
    concealed_signal.chroma = concealed_up.chroma * 0.55 + concealed_down.chroma * 0.45;
    let line_strength = mix(
        0.35,
        0.72,
        hash12(vec2<f32>(seed.reference_coord.y + 73.0, effect.frame.w + 11.0)),
    );
    dropout.dropout_mix = mask * line_strength;
    let dropout_luma_noise =
        reference_scaled_fine_noise(seed.reference_coord, vec2<f32>(effect.frame.w, 29.0))
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
    let disturbance_scale = mix(1.0, 0.85, dropout_mix);
    return dot(chroma_signal, vec2<f32>(0.10, -0.05))
        * effect.reconstruction_output.z
        * disturbance_scale;
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
    // The active still-image subset ends at clamped decoded RGB.
    // Formal `output_transfer` remains deferred, so there is no additional
    // post-decode display/output shaping step here.
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
    let head_switching = apply_head_switching_approximation(in.uv, seed, base_signal);
    let dropout = apply_dropout_approximation(in.uv, seed, head_switching.signal);
    let disturbance_mix = max(dropout.dropout_mix, head_switching.switching_mix);
    let contamination = sample_reconstruction_contamination(
        in.uv,
        seed.reference_coord,
        dropout.signal,
        disturbance_mix,
    );
    let display_yuv = compose_display_yuv(dropout.signal, contamination, disturbance_mix);
    let rgb = decode_output_rgb(display_yuv);
    return vec4<f32>(rgb, 1.0);
}
