use crate::StillImagePipeline;
use crate::projection::REFERENCE_WIDTH_PX;
use casseted_signal::{SignalSettings, VhsModel};
use casseted_types::ImageFrame;

pub(crate) const EFFECT_UNIFORM_FLOATS: usize = 24;
const BT601_SAMPLES_PER_US: f32 = 13.5;

// The still-image path resolves controls into explicit logical stages and then
// packs them into a shared uniform block used across the compact multi-pass run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedStillStages {
    pub(crate) frame: FrameStage,
    pub(crate) input_conditioning: InputConditioningStage,
    pub(crate) luma_degradation: LumaDegradationStage,
    pub(crate) chroma_degradation: ChromaDegradationStage,
    pub(crate) reconstruction_output: ReconstructionOutputStage,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FrameStage {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) inv_width: f32,
    pub(crate) inv_height: f32,
    pub(crate) frame_index: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct InputConditioningStage {
    pub(crate) highlight_soft_knee: f32,
    pub(crate) highlight_compression: f32,
    pub(crate) line_jitter_px: f32,
    pub(crate) vertical_offset_lines: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LumaDegradationStage {
    pub(crate) blur_px: f32,
    pub(crate) detail_mix: f32,
    pub(crate) highlight_bleed_threshold: f32,
    pub(crate) highlight_bleed_amount: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ChromaDegradationStage {
    pub(crate) offset_px: f32,
    // Shared chroma bandwidth-loss proxy. The chroma shader derives its
    // horizontal low-pass span, coarse cell integration / reconstruction, and
    // restrained contamination tail from this one compact term.
    pub(crate) blur_px: f32,
    pub(crate) saturation: f32,
    pub(crate) vertical_blend: f32,
    pub(crate) phase_error_rad: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ReconstructionOutputStage {
    pub(crate) luma_contamination_amount: f32,
    pub(crate) chroma_contamination_amount: f32,
    pub(crate) y_c_leakage: f32,
    pub(crate) dropout_line_probability: f32,
    pub(crate) dropout_span_px: f32,
    pub(crate) chroma_phase_noise_rad: f32,
    pub(crate) head_switching_band_lines: f32,
    pub(crate) head_switching_offset_px: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EffectUniforms {
    pub(crate) frame: [f32; 4],
    pub(crate) input_conditioning: [f32; 4],
    pub(crate) luma_degradation: [f32; 4],
    pub(crate) chroma_degradation: [f32; 4],
    pub(crate) reconstruction_output: [f32; 4],
    pub(crate) reconstruction_aux: [f32; 4],
}

impl From<ResolvedStillStages> for EffectUniforms {
    fn from(stages: ResolvedStillStages) -> Self {
        Self {
            frame: [
                stages.frame.width,
                stages.frame.height,
                // Keep the block compact: derive inverse size in shader and
                // reuse the third lane for the model-only head-switching band.
                stages.reconstruction_output.head_switching_band_lines,
                stages.frame.frame_index,
            ],
            input_conditioning: [
                stages.input_conditioning.highlight_soft_knee,
                stages.input_conditioning.highlight_compression,
                stages.input_conditioning.line_jitter_px,
                stages.input_conditioning.vertical_offset_lines,
            ],
            luma_degradation: [
                stages.luma_degradation.blur_px,
                stages.luma_degradation.detail_mix,
                stages.luma_degradation.highlight_bleed_threshold,
                stages.luma_degradation.highlight_bleed_amount,
            ],
            chroma_degradation: [
                stages.chroma_degradation.offset_px,
                stages.chroma_degradation.blur_px,
                stages.chroma_degradation.saturation,
                stages.chroma_degradation.vertical_blend,
            ],
            reconstruction_output: [
                stages.reconstruction_output.luma_contamination_amount,
                stages.reconstruction_output.chroma_contamination_amount,
                stages.reconstruction_output.y_c_leakage,
                stages.reconstruction_output.head_switching_offset_px,
            ],
            reconstruction_aux: [
                stages.reconstruction_output.dropout_line_probability,
                stages.reconstruction_output.dropout_span_px,
                // Keep the shared block compact: the model-only chroma-phase
                // terms reuse the auxiliary spill lanes instead of widening the
                // preview-facing stage surface. `z` is packed here for the
                // chroma pass, while `w` is consumed later by the final
                // reconstruction/output pass.
                stages.chroma_degradation.phase_error_rad,
                stages.reconstruction_output.chroma_phase_noise_rad,
            ],
        }
    }
}

impl EffectUniforms {
    pub(crate) fn as_bytes(self) -> [u8; EFFECT_UNIFORM_FLOATS * 4] {
        let floats = [
            self.frame[0],
            self.frame[1],
            self.frame[2],
            self.frame[3],
            self.input_conditioning[0],
            self.input_conditioning[1],
            self.input_conditioning[2],
            self.input_conditioning[3],
            self.luma_degradation[0],
            self.luma_degradation[1],
            self.luma_degradation[2],
            self.luma_degradation[3],
            self.chroma_degradation[0],
            self.chroma_degradation[1],
            self.chroma_degradation[2],
            self.chroma_degradation[3],
            self.reconstruction_output[0],
            self.reconstruction_output[1],
            self.reconstruction_output[2],
            self.reconstruction_output[3],
            self.reconstruction_aux[0],
            self.reconstruction_aux[1],
            self.reconstruction_aux[2],
            self.reconstruction_aux[3],
        ];

        let mut bytes = [0_u8; EFFECT_UNIFORM_FLOATS * 4];
        for (index, value) in floats.into_iter().enumerate() {
            let offset = index * 4;
            bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        }

        bytes
    }
}

pub(crate) fn resolve_still_stages(
    input: &ImageFrame,
    pipeline: &StillImagePipeline,
) -> ResolvedStillStages {
    let signal = pipeline.effective_preview_signal();
    let model = pipeline.model();
    let width = input.descriptor.size.width as f32;
    let height = input.descriptor.size.height as f32;
    let reference_scale = (width / REFERENCE_WIDTH_PX).max(0.0);

    ResolvedStillStages {
        frame: FrameStage {
            width,
            height,
            inv_width: width.recip(),
            inv_height: height.recip(),
            frame_index: input.descriptor.frame_index as f32,
        },
        input_conditioning: resolve_input_conditioning_stage(signal, reference_scale),
        luma_degradation: resolve_luma_degradation_stage(signal, reference_scale, model),
        chroma_degradation: resolve_chroma_degradation_stage(signal, reference_scale, model),
        reconstruction_output: resolve_reconstruction_output_stage(input, signal, model),
    }
}

pub(crate) fn effect_uniforms(input: &ImageFrame, pipeline: &StillImagePipeline) -> EffectUniforms {
    resolve_still_stages(input, pipeline).into()
}

pub(crate) fn effect_uniform_bytes(
    input: &ImageFrame,
    pipeline: &StillImagePipeline,
) -> [u8; EFFECT_UNIFORM_FLOATS * 4] {
    effect_uniforms(input, pipeline).as_bytes()
}

fn resolve_input_conditioning_stage(
    signal: SignalSettings,
    reference_scale: f32,
) -> InputConditioningStage {
    InputConditioningStage {
        highlight_soft_knee: signal.tone.highlight_soft_knee.clamp(0.0, 0.999),
        highlight_compression: signal.tone.highlight_compression.max(0.0),
        line_jitter_px: signal.tracking.line_jitter_px * reference_scale,
        vertical_offset_lines: signal.tracking.vertical_offset_lines,
    }
}

fn resolve_luma_degradation_stage(
    signal: SignalSettings,
    reference_scale: f32,
    model: Option<VhsModel>,
) -> LumaDegradationStage {
    // Keep the luma contract compact: the shader now expands this one
    // bandwidth-loss proxy plus the pre-emphasis-derived recovery mix into a
    // broader low-pass / residual attenuation approximation.
    let detail_mix = model
        .map(|vhs| detail_mix_from_preemphasis(vhs.luma.preemphasis_db))
        .unwrap_or(0.0);
    let blur_px = signal.luma.blur_px.max(0.0) * reference_scale;

    LumaDegradationStage {
        blur_px,
        detail_mix,
        highlight_bleed_threshold: highlight_bleed_threshold(signal.tone.highlight_soft_knee),
        highlight_bleed_amount: highlight_bleed_amount(
            signal.luma.blur_px.max(0.0),
            signal.tone.highlight_compression,
        ),
    }
}

fn resolve_chroma_degradation_stage(
    signal: SignalSettings,
    reference_scale: f32,
    model: Option<VhsModel>,
) -> ChromaDegradationStage {
    let vertical_blend = model
        .map(|vhs| vhs.decode.chroma_vertical_blend.clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let phase_error_rad = model
        .map(|vhs| chroma_phase_error_rad(vhs.chroma.phase_error_deg))
        .unwrap_or(0.0);

    ChromaDegradationStage {
        offset_px: signal.chroma.offset_px * reference_scale,
        // Keep the stage contract compact: the pass now expands this one proxy
        // into low-pass, coarse chroma resolution loss, and restrained
        // luma-anchored smear / contamination.
        blur_px: signal.chroma.bleed_px.max(0.0) * reference_scale,
        saturation: signal.chroma.saturation.max(0.0),
        vertical_blend,
        phase_error_rad,
    }
}

fn resolve_reconstruction_output_stage(
    input: &ImageFrame,
    signal: SignalSettings,
    model: Option<VhsModel>,
) -> ReconstructionOutputStage {
    let y_c_leakage = model
        .map(|vhs| vhs.decode.luma_chroma_crosstalk.clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let reference_scale = (input.descriptor.size.width as f32 / REFERENCE_WIDTH_PX).max(0.0);
    let (
        dropout_line_probability,
        dropout_span_px,
        head_switching_band_lines,
        head_switching_offset_px,
    ) = model
        .map(|vhs| {
            let head_switching_band_lines =
                head_switching_band_lines(vhs.transport.head_switching_band_lines);
            (
                dropout_line_probability(vhs.noise.dropout_probability_per_line),
                dropout_span_px_from_time(vhs.noise.dropout_mean_span_us, reference_scale),
                head_switching_band_lines,
                if head_switching_band_lines > 0.0 {
                    head_switching_offset_px_from_time(
                        vhs.transport.head_switching_offset_us,
                        reference_scale,
                    )
                } else {
                    0.0
                },
            )
        })
        .unwrap_or((0.0, 0.0, 0.0, 0.0));
    let chroma_phase_noise_rad = model
        .map(|vhs| chroma_phase_noise_rad(vhs.noise.chroma_phase_noise_deg))
        .unwrap_or(0.0);

    ReconstructionOutputStage {
        luma_contamination_amount: signal.noise.luma_amount.max(0.0),
        chroma_contamination_amount: signal.noise.chroma_amount.max(0.0),
        y_c_leakage,
        dropout_line_probability,
        dropout_span_px,
        chroma_phase_noise_rad,
        head_switching_band_lines,
        head_switching_offset_px,
    }
}

fn detail_mix_from_preemphasis(preemphasis_db: f32) -> f32 {
    (preemphasis_db.max(0.0) * 0.015).min(0.12)
}

fn chroma_phase_error_rad(phase_error_deg: f32) -> f32 {
    phase_error_deg.to_radians()
}

fn chroma_phase_noise_rad(chroma_phase_noise_deg: f32) -> f32 {
    chroma_phase_noise_deg.max(0.0).to_radians()
}

fn highlight_bleed_threshold(highlight_soft_knee: f32) -> f32 {
    (highlight_soft_knee.clamp(0.0, 0.999) + 0.12).clamp(0.72, 0.96)
}

fn highlight_bleed_amount(blur_px: f32, highlight_compression: f32) -> f32 {
    let blur_factor = blur_px.max(0.0) / (blur_px.max(0.0) + 1.25);
    let compression = highlight_compression.max(0.0);
    let compression_factor = compression / (compression + 1.0);

    (blur_factor * (0.06 + compression_factor * 0.14)).min(0.16)
}

fn dropout_line_probability(dropout_probability_per_line: f32) -> f32 {
    dropout_probability_per_line.clamp(0.0, 0.08)
}

fn dropout_span_px_from_time(dropout_mean_span_us: f32, reference_scale: f32) -> f32 {
    (dropout_mean_span_us.max(0.0) * BT601_SAMPLES_PER_US * reference_scale)
        .min(48.0 * reference_scale)
}

fn head_switching_band_lines(head_switching_band_lines: u32) -> f32 {
    (head_switching_band_lines as f32).clamp(0.0, 20.0)
}

fn head_switching_offset_px_from_time(
    head_switching_offset_us: f32,
    reference_scale: f32,
) -> f32 {
    let hard_cap_px = 32.0 * reference_scale;
    (head_switching_offset_us * BT601_SAMPLES_PER_US * reference_scale)
        .clamp(-hard_cap_px, hard_cap_px)
}
