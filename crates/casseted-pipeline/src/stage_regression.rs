use super::{ChromaOverrides, LumaOverrides, SignalOverrides, StillImagePipeline, ToneOverrides};
use crate::stages::{ResolvedStillStages, effect_uniforms, resolve_still_stages};
use casseted_gpu::{GpuContext, GpuContextDescriptor, GpuInitError};
use casseted_shaderlib::ShaderId;
use casseted_signal::{SignalSettings, ToneSettings, TrackingSettings, VhsModel};
use casseted_testing::{
    ImageDiffTolerance, assert_images_match_with_tolerance, image_diff_stats, load_png,
    reference_card_rgba8_image, save_png,
};
use casseted_types::{FrameSize, ImageFrame};
use std::fs;
use std::path::PathBuf;

const REFERENCE_WIDTH: u32 = 96;
const REFERENCE_HEIGHT: u32 = 64;
const REFERENCE_SCALE: f32 = REFERENCE_WIDTH as f32 / 720.0;
const OUTPUT_TOLERANCE: ImageDiffTolerance = ImageDiffTolerance {
    max_changed_bytes: 1024,
    max_mean_absolute_difference: 0.35,
    max_absolute_difference: 3,
};

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

    fn reference_image_path(self) -> PathBuf {
        reference_image_dir().join(format!("{}.png", self.key()))
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

fn reference_size() -> FrameSize {
    FrameSize::new(REFERENCE_WIDTH, REFERENCE_HEIGHT)
}

fn reference_image_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
        .join("reference-images")
        .join("still-pipeline-v1")
}

fn source_image_path() -> PathBuf {
    reference_image_dir().join("reference-card-96x64.png")
}

fn generated_reference_input() -> ImageFrame {
    reference_card_rgba8_image(reference_size())
}

fn load_reference_input_fixture() -> ImageFrame {
    load_png(&source_image_path(), 0).expect("reference input PNG should be readable")
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
fn reference_input_fixture_matches_generator() {
    let generated = generated_reference_input();
    let fixture = load_reference_input_fixture();

    assert_images_match_with_tolerance(
        &generated,
        &fixture,
        ImageDiffTolerance {
            max_changed_bytes: 0,
            max_mean_absolute_difference: 0.0,
            max_absolute_difference: 0,
        },
    );
}

#[test]
fn stage_uniforms_match_reference_defaults() {
    let input = load_reference_input_fixture();

    for case in STAGE_REFERENCE_CASES {
        let pipeline = case.build_pipeline();
        let stages = resolve_still_stages(&input, &pipeline);
        let _uniforms = effect_uniforms(&input, &pipeline);
        case.assert_resolved_stage_defaults(&stages);
    }
}

#[test]
fn stage_reference_images_match_fixtures_when_gpu_is_available() {
    let gpu = match try_gpu_context() {
        Ok(context) => context,
        Err(GpuInitError::AdapterNotFound) => return,
        Err(error) => panic!("failed to initialize gpu context: {error}"),
    };
    let input = load_reference_input_fixture();

    for case in STAGE_REFERENCE_CASES {
        let expected = load_png(&case.reference_image_path(), 0)
            .unwrap_or_else(|error| panic!("{} reference PNG should load: {error}", case.key()));
        let actual = render_reference_case(&gpu, case, &input);
        assert_images_match_with_tolerance(&expected, &actual, OUTPUT_TOLERANCE);
    }
}

#[test]
fn stage_parameter_perturbations_produce_bounded_output_differences() {
    let gpu = match try_gpu_context() {
        Ok(context) => context,
        Err(GpuInitError::AdapterNotFound) => return,
        Err(error) => panic!("failed to initialize gpu context: {error}"),
    };
    let input = load_reference_input_fixture();

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
#[ignore = "updates committed stage reference PNGs"]
fn bless_stage_reference_images() {
    let gpu = try_gpu_context()
        .unwrap_or_else(|error| panic!("failed to initialize gpu context: {error}"));
    let input = generated_reference_input();
    let reference_dir = reference_image_dir();

    fs::create_dir_all(&reference_dir).expect("reference directory should be created");
    save_png(&source_image_path(), &input).expect("reference input PNG should be written");

    for case in STAGE_REFERENCE_CASES {
        let output = render_reference_case(&gpu, case, &input);
        save_png(&case.reference_image_path(), &output).unwrap_or_else(|error| {
            panic!("{} reference PNG should be written: {error}", case.key())
        });
    }
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
