use super::{ChromaOverrides, LumaOverrides, SignalOverrides, StillImagePipeline, ToneOverrides};
use crate::stages::{ResolvedStillStages, effect_uniforms, resolve_still_stages};
use casseted_gpu::{GpuContext, GpuContextDescriptor, GpuInitError};
use casseted_shaderlib::ShaderId;
use casseted_signal::{SignalSettings, ToneSettings, TrackingSettings, VhsModel, VideoStandard};
use casseted_testing::{image_diff_stats, load_png, reference_card_rgba8_image};
use casseted_types::{FrameSize, ImageFrame};
use std::fs;
use std::path::PathBuf;

const REFERENCE_WIDTH: u32 = 96;
const REFERENCE_HEIGHT: u32 = 64;
const REFERENCE_SCALE: f32 = REFERENCE_WIDTH as f32 / 720.0;
// Horizontal bandwidth loss is specified in reference pixels at 720 px wide and
// is scaled to the real frame through `s_ref`. These constants drive the
// resolution-invariance check: the same relative grating must be attenuated by
// the same amount at every output width, so the calibrated look does not drift
// once the pipeline runs above the reference width.
const GRATING_WIDTHS: [u32; 3] = [720, 2160, 3600];
const GRATING_HEIGHT: u32 = 48;
const GRATING_CYCLES_PER_FRAME: f32 = 80.0;
const GRATING_ROW_MARGIN: u32 = 8;
const GRATING_LUMA_AMPLITUDE: f32 = 0.235;
const GRATING_CHROMA_RED_AMPLITUDE: f32 = 0.20;
// Keeps the chroma grating at constant luma: 0.299 * dR + 0.587 * dG = 0.
const GRATING_CHROMA_GREEN_AMPLITUDE: f32 = 0.1018;
const RESOLUTION_INVARIANCE_TOLERANCE: f32 = 0.10;
// Reconstruction contamination is also specified at the reference width. On a
// flat field its energy at a fixed set of relative frequencies must not fall
// away as the raster grows, otherwise a high-resolution render reads cleaner
// than the same content at the reference width.
const NOISE_PROBE_HEIGHT: u32 = 180;
const NOISE_PROBE_LEVEL: u8 = 128;
const NOISE_PROBE_CYCLES_PER_FRAME: [f32; 6] = [10.0, 20.0, 40.0, 80.0, 160.0, 240.0];
const NOISE_PROBE_TOLERANCE: f32 = 0.45;
// Line-oriented artifacts are specified per line of the reference raster, so a
// taller frame must not multiply how many of them fire. Heights are exact
// multiples of the NTSC active line count so reference lines map cleanly.
const DROPOUT_PROBE_WIDTH: u32 = 720;
const DROPOUT_PROBE_HEIGHTS: [u32; 3] = [480, 1440, 2400];
const DROPOUT_PROBE_DEVIATION: f32 = 0.03;
const CURRENT_REFERENCE_BUCKETS: [&str; 8] = [
    "01_target-look",
    "02_highlights-specular",
    "03_color-edges-chroma",
    "04_portrait-skin",
    "05_ui-text-detail",
    "06_neutral-interior",
    "07_silhouette-low-detail",
    "08_dark-screen-noise",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StageReferenceCase {
    InputConditioningTone,
    LumaChromaTransform,
    LumaDegradation,
    ChromaDegradation,
    ReconstructionOutput,
}

const STAGE_REFERENCE_CASES: [StageReferenceCase; 5] = [
    StageReferenceCase::InputConditioningTone,
    StageReferenceCase::LumaChromaTransform,
    StageReferenceCase::LumaDegradation,
    StageReferenceCase::ChromaDegradation,
    StageReferenceCase::ReconstructionOutput,
];

impl StageReferenceCase {
    fn key(self) -> &'static str {
        match self {
            Self::InputConditioningTone => "input-conditioning-tone",
            Self::LumaChromaTransform => "luma-chroma-transform",
            Self::LumaDegradation => "luma-degradation",
            Self::ChromaDegradation => "chroma-degradation",
            Self::ReconstructionOutput => "reconstruction-output",
        }
    }

    fn formulas_section(self) -> &'static str {
        match self {
            Self::InputConditioningTone => "4.1",
            Self::LumaChromaTransform => "4.2",
            Self::LumaDegradation => "4.3",
            Self::ChromaDegradation => "4.4",
            Self::ReconstructionOutput => "4.5 / 5.2 / 5.3 / 5.4",
        }
    }

    fn build_pipeline(self) -> StillImagePipeline {
        match self {
            Self::InputConditioningTone => StillImagePipeline::new(SignalSettings {
                tone: ToneSettings {
                    highlight_soft_knee: 0.64,
                    highlight_compression: 0.62,
                },
                tracking: TrackingSettings {
                    line_jitter_px: 0.35,
                    vertical_offset_lines: 0.25,
                },
                ..SignalSettings::neutral()
            }),
            Self::LumaChromaTransform => StillImagePipeline::new(SignalSettings::neutral()),
            Self::LumaDegradation => {
                let mut model = neutral_reference_model();
                model.luma.bandwidth_mhz = 3.0;
                model.luma.preemphasis_db = 3.0;
                StillImagePipeline::from_vhs_model(model)
            }
            Self::ChromaDegradation => {
                let mut model = neutral_reference_model();
                model.chroma.bandwidth_khz = 300.0;
                model.chroma.saturation_gain = 0.94;
                model.chroma.delay_us = 0.08;
                model.decode.chroma_vertical_blend = 0.35;
                StillImagePipeline::from_vhs_model(model)
            }
            Self::ReconstructionOutput => {
                let mut model = neutral_reference_model();
                model.noise.luma_sigma = 0.018;
                model.noise.chroma_sigma = 0.022;
                model.noise.dropout_probability_per_line = 0.06;
                model.noise.dropout_mean_span_us = 1.8;
                model.decode.luma_chroma_crosstalk = 0.04;
                StillImagePipeline::from_vhs_model(model)
            }
        }
    }

    fn perturb(self, pipeline: &mut StillImagePipeline) -> bool {
        match self {
            Self::InputConditioningTone => {
                pipeline.set_preview_overrides(SignalOverrides {
                    tone: ToneOverrides {
                        highlight_soft_knee: Some(0.60),
                        highlight_compression: Some(0.68),
                    },
                    ..SignalOverrides::default()
                });
                true
            }
            Self::LumaChromaTransform => false,
            Self::LumaDegradation => {
                pipeline.set_preview_overrides(SignalOverrides {
                    luma: LumaOverrides {
                        blur_px: Some(pipeline.preview_base_signal().luma.blur_px + 0.35),
                    },
                    ..SignalOverrides::default()
                });
                true
            }
            Self::ChromaDegradation => {
                pipeline.set_preview_overrides(SignalOverrides {
                    chroma: ChromaOverrides {
                        bleed_px: Some(pipeline.preview_base_signal().chroma.bleed_px + 0.40),
                        offset_px: Some(pipeline.preview_base_signal().chroma.offset_px + 0.20),
                        ..ChromaOverrides::default()
                    },
                    ..SignalOverrides::default()
                });
                true
            }
            Self::ReconstructionOutput => {
                if let Some(mut model) = pipeline.model() {
                    model.noise.dropout_probability_per_line += 0.02;
                    model.noise.dropout_mean_span_us += 0.45;
                    pipeline.set_model(model);
                }
                true
            }
        }
    }

    fn assert_resolved_stage_defaults(self, stages: &ResolvedStillStages) {
        assert_approx_eq(stages.frame.width, REFERENCE_WIDTH as f32, "frame.width");
        assert_approx_eq(stages.frame.height, REFERENCE_HEIGHT as f32, "frame.height");
        assert_approx_eq(
            stages.frame.inv_width,
            1.0 / REFERENCE_WIDTH as f32,
            "frame.inv_width",
        );
        assert_approx_eq(
            stages.frame.inv_height,
            1.0 / REFERENCE_HEIGHT as f32,
            "frame.inv_height",
        );
        assert_approx_eq(stages.frame.frame_index, 0.0, "frame.frame_index");

        match self {
            Self::InputConditioningTone => {
                assert_approx_eq(
                    stages.input_conditioning.highlight_soft_knee,
                    0.64,
                    "input_conditioning.highlight_soft_knee",
                );
                assert_approx_eq(
                    stages.input_conditioning.highlight_compression,
                    0.62,
                    "input_conditioning.highlight_compression",
                );
                assert_approx_eq(
                    stages.input_conditioning.line_jitter_px,
                    0.35 * REFERENCE_SCALE,
                    "input_conditioning.line_jitter_px",
                );
                assert_approx_eq(
                    stages.input_conditioning.vertical_offset_lines,
                    0.25,
                    "input_conditioning.vertical_offset_lines",
                );
                assert_approx_eq(
                    stages.luma_degradation.blur_px,
                    0.0,
                    "luma_degradation.blur_px",
                );
                assert_approx_eq(
                    stages.luma_degradation.detail_mix,
                    0.0,
                    "luma_degradation.detail_mix",
                );
                assert_approx_eq(
                    stages.luma_degradation.highlight_bleed_threshold,
                    0.76,
                    "luma_degradation.highlight_bleed_threshold",
                );
                assert_approx_eq(
                    stages.luma_degradation.highlight_bleed_amount,
                    0.0,
                    "luma_degradation.highlight_bleed_amount",
                );
                assert_approx_eq(
                    stages.chroma_degradation.offset_px,
                    0.0,
                    "chroma_degradation.offset_px",
                );
                assert_approx_eq(
                    stages.chroma_degradation.blur_px,
                    0.0,
                    "chroma_degradation.blur_px",
                );
                assert_approx_eq(
                    stages.chroma_degradation.saturation,
                    1.0,
                    "chroma_degradation.saturation",
                );
                assert_approx_eq(
                    stages.chroma_degradation.vertical_blend,
                    0.0,
                    "chroma_degradation.vertical_blend",
                );
                assert_approx_eq(
                    stages.chroma_degradation.phase_error_rad,
                    0.0,
                    "chroma_degradation.phase_error_rad",
                );
                assert_approx_eq(
                    stages.reconstruction_output.luma_contamination_amount,
                    0.0,
                    "reconstruction_output.luma_contamination_amount",
                );
                assert_approx_eq(
                    stages.reconstruction_output.chroma_contamination_amount,
                    0.0,
                    "reconstruction_output.chroma_contamination_amount",
                );
                assert_approx_eq(
                    stages.reconstruction_output.y_c_leakage,
                    0.0,
                    "reconstruction_output.y_c_leakage",
                );
                assert_approx_eq(
                    stages.reconstruction_output.dropout_line_probability,
                    0.0,
                    "reconstruction_output.dropout_line_probability",
                );
                assert_approx_eq(
                    stages.reconstruction_output.dropout_span_px,
                    0.0,
                    "reconstruction_output.dropout_span_px",
                );
                assert_approx_eq(
                    stages.reconstruction_output.chroma_phase_noise_rad,
                    0.0,
                    "reconstruction_output.chroma_phase_noise_rad",
                );
                assert_approx_eq(
                    stages.reconstruction_output.head_switching_band_lines,
                    0.0,
                    "reconstruction_output.head_switching_band_lines",
                );
                assert_approx_eq(
                    stages.reconstruction_output.head_switching_offset_px,
                    0.0,
                    "reconstruction_output.head_switching_offset_px",
                );
            }
            Self::LumaChromaTransform => {
                assert_approx_eq(
                    stages.input_conditioning.highlight_soft_knee,
                    0.999,
                    "input_conditioning.highlight_soft_knee",
                );
                assert_approx_eq(
                    stages.input_conditioning.highlight_compression,
                    0.0,
                    "input_conditioning.highlight_compression",
                );
                assert_approx_eq(
                    stages.input_conditioning.line_jitter_px,
                    0.0,
                    "input_conditioning.line_jitter_px",
                );
                assert_approx_eq(
                    stages.luma_degradation.blur_px,
                    0.0,
                    "luma_degradation.blur_px",
                );
                assert_approx_eq(
                    stages.luma_degradation.detail_mix,
                    0.0,
                    "luma_degradation.detail_mix",
                );
                assert_approx_eq(
                    stages.luma_degradation.highlight_bleed_threshold,
                    0.96,
                    "luma_degradation.highlight_bleed_threshold",
                );
                assert_approx_eq(
                    stages.luma_degradation.highlight_bleed_amount,
                    0.0,
                    "luma_degradation.highlight_bleed_amount",
                );
                assert_approx_eq(
                    stages.chroma_degradation.offset_px,
                    0.0,
                    "chroma_degradation.offset_px",
                );
                assert_approx_eq(
                    stages.chroma_degradation.blur_px,
                    0.0,
                    "chroma_degradation.blur_px",
                );
                assert_approx_eq(
                    stages.chroma_degradation.saturation,
                    1.0,
                    "chroma_degradation.saturation",
                );
                assert_approx_eq(
                    stages.reconstruction_output.luma_contamination_amount,
                    0.0,
                    "reconstruction_output.luma_contamination_amount",
                );
                assert_approx_eq(
                    stages.reconstruction_output.chroma_contamination_amount,
                    0.0,
                    "reconstruction_output.chroma_contamination_amount",
                );
                assert_approx_eq(
                    stages.reconstruction_output.y_c_leakage,
                    0.0,
                    "reconstruction_output.y_c_leakage",
                );
                assert_approx_eq(
                    stages.reconstruction_output.dropout_line_probability,
                    0.0,
                    "reconstruction_output.dropout_line_probability",
                );
                assert_approx_eq(
                    stages.reconstruction_output.dropout_span_px,
                    0.0,
                    "reconstruction_output.dropout_span_px",
                );
                assert_approx_eq(
                    stages.reconstruction_output.chroma_phase_noise_rad,
                    0.0,
                    "reconstruction_output.chroma_phase_noise_rad",
                );
                assert_approx_eq(
                    stages.reconstruction_output.head_switching_band_lines,
                    0.0,
                    "reconstruction_output.head_switching_band_lines",
                );
                assert_approx_eq(
                    stages.reconstruction_output.head_switching_offset_px,
                    0.0,
                    "reconstruction_output.head_switching_offset_px",
                );
            }
            Self::LumaDegradation => {
                assert_approx_eq(
                    stages.input_conditioning.highlight_soft_knee,
                    0.999,
                    "input_conditioning.highlight_soft_knee",
                );
                assert_approx_eq(
                    stages.luma_degradation.blur_px,
                    1.92 * REFERENCE_SCALE,
                    "luma_degradation.blur_px",
                );
                assert_approx_eq(
                    stages.luma_degradation.detail_mix,
                    0.045,
                    "luma_degradation.detail_mix",
                );
                assert_approx_eq(
                    stages.luma_degradation.highlight_bleed_threshold,
                    0.96,
                    "luma_degradation.highlight_bleed_threshold",
                );
                assert_approx_eq(
                    stages.luma_degradation.highlight_bleed_amount,
                    0.03634069,
                    "luma_degradation.highlight_bleed_amount",
                );
                assert_approx_eq(
                    stages.chroma_degradation.saturation,
                    1.0,
                    "chroma_degradation.saturation",
                );
                assert_approx_eq(
                    stages.reconstruction_output.y_c_leakage,
                    0.0,
                    "reconstruction_output.y_c_leakage",
                );
                assert_approx_eq(
                    stages.reconstruction_output.dropout_line_probability,
                    0.0,
                    "reconstruction_output.dropout_line_probability",
                );
                assert_approx_eq(
                    stages.reconstruction_output.dropout_span_px,
                    0.0,
                    "reconstruction_output.dropout_span_px",
                );
                assert_approx_eq(
                    stages.reconstruction_output.chroma_phase_noise_rad,
                    0.0,
                    "reconstruction_output.chroma_phase_noise_rad",
                );
                assert_approx_eq(
                    stages.reconstruction_output.head_switching_band_lines,
                    0.0,
                    "reconstruction_output.head_switching_band_lines",
                );
                assert_approx_eq(
                    stages.reconstruction_output.head_switching_offset_px,
                    0.0,
                    "reconstruction_output.head_switching_offset_px",
                );
            }
            Self::ChromaDegradation => {
                assert_approx_eq(
                    stages.chroma_degradation.offset_px,
                    0.432 * REFERENCE_SCALE,
                    "chroma_degradation.offset_px",
                );
                assert_approx_eq(
                    stages.chroma_degradation.blur_px,
                    (7.0 / 3.0) * REFERENCE_SCALE,
                    "chroma_degradation.blur_px",
                );
                assert_approx_eq(
                    stages.chroma_degradation.saturation,
                    0.94,
                    "chroma_degradation.saturation",
                );
                assert_approx_eq(
                    stages.chroma_degradation.vertical_blend,
                    0.35,
                    "chroma_degradation.vertical_blend",
                );
                assert_approx_eq(
                    stages.chroma_degradation.phase_error_rad,
                    0.0,
                    "chroma_degradation.phase_error_rad",
                );
                assert_approx_eq(
                    stages.reconstruction_output.y_c_leakage,
                    0.0,
                    "reconstruction_output.y_c_leakage",
                );
                assert_approx_eq(
                    stages.reconstruction_output.dropout_line_probability,
                    0.0,
                    "reconstruction_output.dropout_line_probability",
                );
                assert_approx_eq(
                    stages.reconstruction_output.dropout_span_px,
                    0.0,
                    "reconstruction_output.dropout_span_px",
                );
                assert_approx_eq(
                    stages.reconstruction_output.chroma_phase_noise_rad,
                    0.0,
                    "reconstruction_output.chroma_phase_noise_rad",
                );
                assert_approx_eq(
                    stages.reconstruction_output.head_switching_band_lines,
                    0.0,
                    "reconstruction_output.head_switching_band_lines",
                );
                assert_approx_eq(
                    stages.reconstruction_output.head_switching_offset_px,
                    0.0,
                    "reconstruction_output.head_switching_offset_px",
                );
            }
            Self::ReconstructionOutput => {
                assert_approx_eq(
                    stages.chroma_degradation.saturation,
                    1.0,
                    "chroma_degradation.saturation",
                );
                assert_approx_eq(
                    stages.reconstruction_output.luma_contamination_amount,
                    0.018,
                    "reconstruction_output.luma_contamination_amount",
                );
                assert_approx_eq(
                    stages.reconstruction_output.chroma_contamination_amount,
                    0.0077,
                    "reconstruction_output.chroma_contamination_amount",
                );
                assert_approx_eq(
                    stages.reconstruction_output.y_c_leakage,
                    0.04,
                    "reconstruction_output.y_c_leakage",
                );
                assert_approx_eq(
                    stages.reconstruction_output.dropout_line_probability,
                    0.06,
                    "reconstruction_output.dropout_line_probability",
                );
                assert_approx_eq(
                    stages.reconstruction_output.dropout_span_px,
                    3.24,
                    "reconstruction_output.dropout_span_px",
                );
                assert_approx_eq(
                    stages.reconstruction_output.chroma_phase_noise_rad,
                    0.0,
                    "reconstruction_output.chroma_phase_noise_rad",
                );
                assert_approx_eq(
                    stages.reconstruction_output.head_switching_band_lines,
                    0.0,
                    "reconstruction_output.head_switching_band_lines",
                );
                assert_approx_eq(
                    stages.reconstruction_output.head_switching_offset_px,
                    0.0,
                    "reconstruction_output.head_switching_offset_px",
                );
            }
        }
    }

    fn assert_perturbation_bounds(self, diff: casseted_testing::ImageDiffStats) {
        assert!(
            diff.changed_bytes > 0,
            "{} perturbation should change the output",
            self.key()
        );

        let (max_mean, max_abs) = match self {
            Self::InputConditioningTone => (6.0, 96),
            Self::LumaDegradation => (4.0, 80),
            Self::ChromaDegradation => (5.0, 96),
            Self::ReconstructionOutput => (2.5, 48),
            Self::LumaChromaTransform => (0.0, 0),
        };

        assert!(
            diff.mean_absolute_difference <= max_mean,
            "{} perturbation exceeded mean diff bound: {} > {}",
            self.key(),
            diff.mean_absolute_difference,
            max_mean,
        );
        assert!(
            diff.max_absolute_difference <= max_abs,
            "{} perturbation exceeded max diff bound: {} > {}",
            self.key(),
            diff.max_absolute_difference,
            max_abs,
        );
    }
}

fn neutral_reference_model() -> VhsModel {
    let mut model = VhsModel::default();
    model.tone.highlight_soft_knee = 1.0;
    model.tone.highlight_compression = 0.0;
    model.luma.bandwidth_mhz = 4.2;
    model.luma.preemphasis_db = 0.0;
    model.chroma.bandwidth_khz = 1000.0;
    model.chroma.saturation_gain = 1.0;
    model.chroma.delay_us = 0.0;
    model.chroma.phase_error_deg = 0.0;
    model.transport.line_jitter_us = 0.0;
    model.transport.vertical_wander_lines = 0.0;
    model.transport.head_switching_band_lines = 0;
    model.transport.head_switching_offset_us = 0.0;
    model.noise.luma_sigma = 0.0;
    model.noise.chroma_sigma = 0.0;
    model.noise.chroma_phase_noise_deg = 0.0;
    model.noise.dropout_probability_per_line = 0.0;
    model.noise.dropout_mean_span_us = 0.0;
    model.decode.chroma_vertical_blend = 0.0;
    model.decode.luma_chroma_crosstalk = 0.0;
    model
}

fn bandwidth_reference_model() -> VhsModel {
    // Isolate horizontal bandwidth loss: tone, transport, noise, dropout, and
    // decode terms stay neutral so only the luma/chroma band limiting shapes
    // the measured grating response.
    let mut model = neutral_reference_model();
    model.luma.bandwidth_mhz = 2.325;
    model.chroma.bandwidth_khz = 100.0;
    model
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GratingBand {
    Luma,
    Chroma,
}

impl GratingBand {
    fn label(self) -> &'static str {
        match self {
            Self::Luma => "luma",
            Self::Chroma => "chroma",
        }
    }

    fn column_range(self, width: u32) -> (u32, u32) {
        let split = width / 2;
        match self {
            Self::Luma => (0, split),
            Self::Chroma => (split, width),
        }
    }
}

fn quantize_unorm8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

// One frame carrying both probes: a luma-only grating on the left half and a
// constant-luma chroma grating on the right half, each at the same relative
// frequency regardless of frame width.
fn horizontal_grating_image(width: u32) -> ImageFrame {
    let size = FrameSize::new(width, GRATING_HEIGHT);
    let split = width / 2;
    let mut data = Vec::with_capacity(size.pixels() as usize * 4);

    for _ in 0..size.height {
        for x in 0..width {
            let phase = std::f32::consts::TAU * GRATING_CYCLES_PER_FRAME * x as f32 / width as f32;
            let wave = phase.sin();
            let pixel = if x < split {
                let level = quantize_unorm8(0.5 + GRATING_LUMA_AMPLITUDE * wave);
                [level, level, level, 255]
            } else {
                [
                    quantize_unorm8(0.5 + GRATING_CHROMA_RED_AMPLITUDE * wave),
                    quantize_unorm8(0.5 - GRATING_CHROMA_GREEN_AMPLITUDE * wave),
                    quantize_unorm8(0.5),
                    255,
                ]
            };
            data.extend_from_slice(&pixel);
        }
    }

    ImageFrame::rgba8(size, data).expect("generated grating image should be valid")
}

fn sample_luma_and_chroma_v(frame: &ImageFrame, x: u32, y: u32) -> (f32, f32) {
    let width = frame.descriptor.size.width as usize;
    let index = (y as usize * width + x as usize) * 4;
    let red = frame.data[index] as f32 / 255.0;
    let green = frame.data[index + 1] as f32 / 255.0;
    let blue = frame.data[index + 2] as f32 / 255.0;
    let luma = 0.299 * red + 0.587 * green + 0.114 * blue;
    (luma, (red - luma) * 0.877_283)
}

// Single-bin DFT at the grating frequency: robust to sub-pixel sampling phase
// in a way that a direct edge-width measurement is not.
fn modulation_transfer(frame: &ImageFrame, band: GratingBand) -> f32 {
    let width = frame.descriptor.size.width;
    let height = frame.descriptor.size.height;
    let (start, end) = band.column_range(width);
    let sample_count = (end - start) as f32;
    let mut total = 0.0;
    let mut rows = 0;

    for y in GRATING_ROW_MARGIN..height - GRATING_ROW_MARGIN {
        let mut cosine_sum = 0.0;
        let mut sine_sum = 0.0;
        for x in start..end {
            let (luma, chroma_v) = sample_luma_and_chroma_v(frame, x, y);
            let value = match band {
                GratingBand::Luma => luma,
                GratingBand::Chroma => chroma_v,
            };
            let phase = std::f32::consts::TAU * GRATING_CYCLES_PER_FRAME * x as f32 / width as f32;
            cosine_sum += value * phase.cos();
            sine_sum += value * phase.sin();
        }
        total += 2.0 * (cosine_sum / sample_count).hypot(sine_sum / sample_count);
        rows += 1;
    }

    total / rows as f32
}

fn luma_contamination_reference_model() -> VhsModel {
    // Isolate reconstruction contamination: no bandwidth loss, tone shaping,
    // transport, dropout, or decode terms, so a flat input leaves only the
    // luma contamination the final pass injects.
    let mut model = neutral_reference_model();
    model.noise.luma_sigma = 0.018;
    model
}

fn flat_field_image(width: u32, height: u32) -> ImageFrame {
    let size = FrameSize::new(width, height);
    let data = vec![NOISE_PROBE_LEVEL; size.pixels() as usize * 4];
    ImageFrame::rgba8(size, data).expect("generated flat field should be valid")
}

fn dropout_reference_model() -> VhsModel {
    // Isolate line-oriented dropout: no bandwidth loss, tone shaping, or
    // contamination, so every deviation from the flat field is a dropout.
    let mut model = neutral_reference_model();
    model.noise.dropout_probability_per_line = 0.06;
    model.noise.dropout_mean_span_us = 1.8;
    model
}

// Number of contiguous row runs carrying a dropout. Counting runs rather than
// rows is the point: a taller raster should spread the same events over more
// rows, not produce more events.
fn dropout_band_count(frame: &ImageFrame) -> u32 {
    let width = frame.descriptor.size.width;
    let height = frame.descriptor.size.height;
    let mut bands = 0;
    let mut inside = false;

    for y in 0..height {
        let peak = (0..width)
            .map(|x| (sample_luma_and_chroma_v(frame, x, y).0 - 0.5).abs())
            .fold(0.0_f32, f32::max);
        let hit = peak > DROPOUT_PROBE_DEVIATION;
        if hit && !inside {
            bands += 1;
        }
        inside = hit;
    }

    bands
}

// Sparse spectral probe: total contamination power carried at a fixed set of
// cycles-per-frame, normalized so the value is comparable across widths. The
// probe frequencies stay well below the 360 cycles/frame Nyquist limit of the
// narrowest tested raster.
fn contamination_band_power(frame: &ImageFrame) -> f32 {
    let width = frame.descriptor.size.width;
    let height = frame.descriptor.size.height;
    let sample_count = width as f32;
    let mut total = 0.0;

    for y in 0..height {
        let mean = (0..width)
            .map(|x| sample_luma_and_chroma_v(frame, x, y).0)
            .sum::<f32>()
            / sample_count;
        for cycles in NOISE_PROBE_CYCLES_PER_FRAME {
            let mut cosine_sum = 0.0;
            let mut sine_sum = 0.0;
            for x in 0..width {
                let value = sample_luma_and_chroma_v(frame, x, y).0 - mean;
                let phase = std::f32::consts::TAU * cycles * x as f32 / width as f32;
                cosine_sum += value * phase.cos();
                sine_sum += value * phase.sin();
            }
            let cosine_mean = cosine_sum / sample_count;
            let sine_mean = sine_sum / sample_count;
            total += cosine_mean * cosine_mean + sine_mean * sine_mean;
        }
    }

    total / height as f32
}

fn reference_size() -> FrameSize {
    FrameSize::new(REFERENCE_WIDTH, REFERENCE_HEIGHT)
}

fn reference_corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
        .join("reference-images")
}

fn reference_bucket_dir(bucket: &str) -> PathBuf {
    reference_corpus_dir().join(bucket)
}

fn reference_bucket_pngs(bucket: &str) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(reference_bucket_dir(bucket))
        .unwrap_or_else(|error| panic!("{bucket} bucket should be readable: {error}"))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let is_png = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
            is_png.then_some(path)
        })
        .collect();
    paths.sort();
    paths
}

fn generated_reference_input() -> ImageFrame {
    reference_card_rgba8_image(reference_size())
}

fn try_gpu_context() -> Result<GpuContext, GpuInitError> {
    pollster::block_on(GpuContext::request(&GpuContextDescriptor::default()))
}

fn render_reference_case(
    gpu: &GpuContext,
    case: StageReferenceCase,
    input: &ImageFrame,
) -> ImageFrame {
    case.build_pipeline()
        .process_with_gpu(gpu, input)
        .unwrap_or_else(|error| panic!("{} should render: {error}", case.key()))
}

fn assert_approx_eq(actual: f32, expected: f32, label: &str) {
    let delta = (actual - expected).abs();
    assert!(
        delta < 1e-5,
        "{label} expected {expected}, got {actual} (delta={delta})"
    );
}

#[test]
fn stage_uniforms_match_reference_defaults() {
    let input = generated_reference_input();

    for case in STAGE_REFERENCE_CASES {
        let pipeline = case.build_pipeline();
        let stages = resolve_still_stages(&input, &pipeline);
        let _uniforms = effect_uniforms(&input, &pipeline);
        case.assert_resolved_stage_defaults(&stages);
    }
}

#[test]
fn reference_bucket_structure_matches_current_baseline_corpus() {
    let mut actual_buckets: Vec<_> = fs::read_dir(reference_corpus_dir())
        .expect("reference corpus directory should be readable")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry.file_type().ok()?.is_dir().then(|| entry.file_name())
        })
        .filter_map(|name| name.to_str().map(str::to_owned))
        .collect();
    actual_buckets.sort();

    let expected_buckets = CURRENT_REFERENCE_BUCKETS
        .iter()
        .map(|bucket| (*bucket).to_owned())
        .collect::<Vec<_>>();

    assert_eq!(actual_buckets, expected_buckets);

    for bucket in CURRENT_REFERENCE_BUCKETS {
        let images = reference_bucket_pngs(bucket);
        assert!(
            !images.is_empty(),
            "{bucket} should contain at least one PNG reference"
        );
    }
}

#[test]
fn stage_parameter_perturbations_produce_bounded_output_differences() {
    let gpu = match try_gpu_context() {
        Ok(context) => context,
        Err(GpuInitError::AdapterNotFound) => return,
        Err(error) => panic!("failed to initialize gpu context: {error}"),
    };
    let input = generated_reference_input();

    for case in STAGE_REFERENCE_CASES {
        let mut perturbed = case.build_pipeline();
        if !case.perturb(&mut perturbed) {
            continue;
        }

        let base = render_reference_case(&gpu, case, &input);
        let varied = perturbed
            .process_with_gpu(&gpu, &input)
            .unwrap_or_else(|error| panic!("{} perturbation should render: {error}", case.key()));
        let diff = image_diff_stats(&base, &varied);
        case.assert_perturbation_bounds(diff);
    }
}

#[test]
fn default_pipeline_matches_compiled_runtime_on_reference_bucket_corpus_when_gpu_is_available() {
    let gpu = match try_gpu_context() {
        Ok(context) => context,
        Err(GpuInitError::AdapterNotFound) => return,
        Err(error) => panic!("failed to initialize gpu context: {error}"),
    };
    let runtime = crate::StillPipelineRuntime::new(&gpu);
    let pipeline = StillImagePipeline::default();

    for bucket in CURRENT_REFERENCE_BUCKETS {
        for image_path in reference_bucket_pngs(bucket) {
            let input = load_png(&image_path, 0)
                .unwrap_or_else(|error| panic!("{} should load: {error}", image_path.display()));
            let direct = pipeline
                .process_with_gpu(&gpu, &input)
                .unwrap_or_else(|error| {
                    panic!(
                        "{} should render on the direct GPU path: {error}",
                        image_path.display()
                    )
                });
            let reused = pipeline
                .process_with_runtime(&runtime, &input)
                .unwrap_or_else(|error| {
                    panic!(
                        "{} should render on the compiled runtime path: {error}",
                        image_path.display()
                    )
                });
            let diff = image_diff_stats(&input, &direct);

            assert_eq!(
                direct,
                reused,
                "{} runtime output drifted",
                image_path.display()
            );
            assert!(
                diff.changed_bytes > 0,
                "{} should produce a non-identical processed image",
                image_path.display()
            );
        }
    }
}

#[test]
fn horizontal_bandwidth_response_stays_resolution_invariant_when_gpu_is_available() {
    let gpu = match try_gpu_context() {
        Ok(context) => context,
        Err(GpuInitError::AdapterNotFound) => return,
        Err(error) => panic!("failed to initialize gpu context: {error}"),
    };
    let pipeline = StillImagePipeline::from_vhs_model(bandwidth_reference_model());

    for band in [GratingBand::Luma, GratingBand::Chroma] {
        let responses: Vec<f32> = GRATING_WIDTHS
            .iter()
            .map(|&width| {
                let input = horizontal_grating_image(width);
                let output = pipeline
                    .process_with_gpu(&gpu, &input)
                    .unwrap_or_else(|error| {
                        panic!("{width}px {} grating should render: {error}", band.label())
                    });
                modulation_transfer(&output, band)
            })
            .collect();

        let lowest = responses.iter().copied().fold(f32::INFINITY, f32::min);
        let highest = responses.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mean = responses.iter().sum::<f32>() / responses.len() as f32;
        assert!(
            mean > 1e-4,
            "{} grating response collapsed to zero across {GRATING_WIDTHS:?}",
            band.label(),
        );

        let spread = (highest - lowest) / mean;
        assert!(
            spread <= RESOLUTION_INVARIANCE_TOLERANCE,
            "{} bandwidth response drifted {spread} across widths {GRATING_WIDTHS:?} \
             (tolerance {RESOLUTION_INVARIANCE_TOLERANCE}, measured {responses:?}); horizontal \
             spatial constants must be expressed in reference pixels and scaled by s_ref",
            band.label(),
        );
    }
}

#[test]
fn reconstruction_contamination_stays_resolution_invariant_when_gpu_is_available() {
    let gpu = match try_gpu_context() {
        Ok(context) => context,
        Err(GpuInitError::AdapterNotFound) => return,
        Err(error) => panic!("failed to initialize gpu context: {error}"),
    };
    let pipeline = StillImagePipeline::from_vhs_model(luma_contamination_reference_model());

    let powers: Vec<f32> = GRATING_WIDTHS
        .iter()
        .map(|&width| {
            let input = flat_field_image(width, NOISE_PROBE_HEIGHT);
            let output = pipeline
                .process_with_gpu(&gpu, &input)
                .unwrap_or_else(|error| panic!("{width}px flat field should render: {error}"));
            contamination_band_power(&output)
        })
        .collect();

    let baseline = powers[0];
    assert!(
        baseline > 0.0,
        "reference-width flat field carried no measurable contamination",
    );

    for (width, power) in GRATING_WIDTHS.iter().zip(&powers).skip(1) {
        let retained = power / baseline;
        assert!(
            (retained - 1.0).abs() <= NOISE_PROBE_TOLERANCE,
            "contamination power at {width}px is {retained} of the reference-width level \
             (tolerance {NOISE_PROBE_TOLERANCE}, measured {powers:?}); contamination structure \
             must be specified in reference pixels so it does not thin out as the raster grows",
        );
    }
}

#[test]
fn line_oriented_artifacts_stay_invariant_across_frame_height_when_gpu_is_available() {
    let gpu = match try_gpu_context() {
        Ok(context) => context,
        Err(GpuInitError::AdapterNotFound) => return,
        Err(error) => panic!("failed to initialize gpu context: {error}"),
    };
    let pipeline = StillImagePipeline::from_vhs_model(dropout_reference_model());

    let counts: Vec<u32> = DROPOUT_PROBE_HEIGHTS
        .iter()
        .map(|&height| {
            let input = flat_field_image(DROPOUT_PROBE_WIDTH, height);
            let output = pipeline
                .process_with_gpu(&gpu, &input)
                .unwrap_or_else(|error| panic!("{height}px flat field should render: {error}"));
            dropout_band_count(&output)
        })
        .collect();

    let baseline = counts[0];
    assert!(
        baseline > 0,
        "reference-height flat field carried no dropout bands to compare against",
    );

    for (height, count) in DROPOUT_PROBE_HEIGHTS.iter().zip(&counts).skip(1) {
        assert_eq!(
            *count, baseline,
            "dropout fired {count} times at {height}px against {baseline} at the reference \
             height (all counts {counts:?}); a per-line probability must be drawn once per \
             reference line, not once per output row",
        );
    }
}

#[test]
fn video_standard_line_count_drives_the_vertical_reference_scale() {
    // `VhsModel.standard` is active in exactly one role: it decides how many
    // lines the reference raster has, which is what every line-oriented term
    // is measured against. The two standards must therefore resolve different
    // vertical factors for the same frame.
    let input = flat_field_image(DROPOUT_PROBE_WIDTH, 1152);
    let ntsc = StillImagePipeline::from_vhs_model(VhsModel::for_standard(VideoStandard::NtscM));
    let pal = StillImagePipeline::from_vhs_model(VhsModel::for_standard(VideoStandard::Pal));

    let ntsc_scale = resolve_still_stages(&input, &ntsc)
        .frame
        .vertical_reference_scale;
    let pal_scale = resolve_still_stages(&input, &pal)
        .frame
        .vertical_reference_scale;

    assert_approx_eq(ntsc_scale, 1152.0 / 480.0, "ntsc vertical_reference_scale");
    assert_approx_eq(pal_scale, 1152.0 / 576.0, "pal vertical_reference_scale");
}

#[test]
fn reference_scales_never_shrink_the_calibration_below_the_reference_raster() {
    // Both factors are clamped at 1.0, which is what keeps output at or below
    // the reference raster unchanged.
    let input = flat_field_image(320, 240);
    let pipeline = StillImagePipeline::default();
    let frame = resolve_still_stages(&input, &pipeline).frame;

    assert_approx_eq(frame.horizontal_reference_scale, 1.0, "horizontal clamp");
    assert_approx_eq(frame.vertical_reference_scale, 1.0, "vertical clamp");
}

#[test]
fn case_metadata_covers_formulas_sections() {
    for case in STAGE_REFERENCE_CASES {
        assert!(
            !case.formulas_section().is_empty(),
            "{} should map to a formulas section",
            case.key()
        );
        assert_eq!(
            case.build_pipeline().shader_ids(),
            &[
                ShaderId::StillInputConditioning,
                ShaderId::StillLumaDegradation,
                ShaderId::StillChromaDegradation,
                ShaderId::StillReconstructionOutput,
            ]
        );
    }
}
