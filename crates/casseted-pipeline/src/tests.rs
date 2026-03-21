use crate::stages::{effect_uniform_bytes, effect_uniforms, resolve_still_stages};
use crate::{
    ChromaOverrides, NoiseOverrides, SignalOverrides, StillImagePipeline, StillPipelineRuntime,
    TrackingOverrides,
};
use casseted_gpu::{GpuContext, GpuContextDescriptor, GpuInitError};
use casseted_shaderlib::ShaderId;
use casseted_signal::{
    ChromaSettings, InputTransfer, NoiseSettings, OutputTransfer, SignalSettings, TemporalSampling,
    TrackingSettings, VhsModel, VideoMatrix, VideoStandard,
};
use casseted_testing::{
    assert_images_not_identical, gradient_rgba8_image, reference_card_rgba8_image,
};
use casseted_types::FrameSize;

#[test]
fn pipeline_uses_expected_multi_pass_shaders() {
    let pipeline = StillImagePipeline::default();

    assert_eq!(
        pipeline.shader_ids(),
        &[
            ShaderId::StillInputConditioning,
            ShaderId::StillLumaDegradation,
            ShaderId::StillChromaDegradation,
            ShaderId::StillReconstructionOutput,
        ]
    );
}

#[test]
fn padded_bytes_per_row_aligns_to_copy_requirement() {
    let padded = crate::runtime::padded_bytes_per_row(17);

    assert_eq!(padded % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
    assert!(padded >= 17 * 4);
}

#[test]
fn uniform_bytes_include_frame_dimensions() {
    let input = gradient_rgba8_image(FrameSize::new(8, 4));
    let pipeline = StillImagePipeline::default();
    let bytes = effect_uniform_bytes(&input, &pipeline);

    assert_eq!(&bytes[0..4], &(8.0_f32).to_ne_bytes());
    assert_eq!(&bytes[4..8], &(4.0_f32).to_ne_bytes());
}

#[test]
fn default_pipeline_projects_vhs_model_into_current_signal_settings() {
    let pipeline = StillImagePipeline::default();
    let projected = pipeline.preview_base_signal();

    assert_eq!(pipeline.model(), Some(VhsModel::default()));
    assert_eq!(projected.tone.highlight_soft_knee, 0.64);
    assert!((projected.chroma.offset_px - 0.324).abs() < 1e-6);
    assert!((projected.luma.blur_px - 1.92).abs() < 1e-6);
}

#[test]
fn manual_pipeline_keeps_model_dependent_final_reconstruction_terms_neutral() {
    let input = gradient_rgba8_image(FrameSize::new(720, 480));
    let pipeline = StillImagePipeline::new(SignalSettings::default());
    let stages = resolve_still_stages(&input, &pipeline);

    assert_eq!(stages.luma_degradation.detail_mix, 0.0);
    assert_eq!(stages.luma_degradation.highlight_bleed_amount, 0.0);
    assert_eq!(stages.chroma_degradation.vertical_blend, 0.0);
    assert_eq!(stages.chroma_degradation.phase_error_rad, 0.0);
    assert_eq!(stages.reconstruction_output.luma_contamination_amount, 0.0);
    assert_eq!(
        stages.reconstruction_output.chroma_contamination_amount,
        0.0
    );
    assert_eq!(stages.reconstruction_output.y_c_leakage, 0.0);
    assert_eq!(stages.reconstruction_output.dropout_line_probability, 0.0);
    assert_eq!(stages.reconstruction_output.dropout_span_px, 0.0);
    assert_eq!(stages.reconstruction_output.chroma_phase_noise_rad, 0.0);
    assert_eq!(stages.reconstruction_output.head_switching_band_lines, 0.0);
    assert_eq!(stages.reconstruction_output.head_switching_offset_px, 0.0);
}

#[test]
fn model_path_resolves_secondary_artifact_terms() {
    let input = gradient_rgba8_image(FrameSize::new(720, 480));
    let pipeline = StillImagePipeline::default();
    let stages = resolve_still_stages(&input, &pipeline);

    assert!((stages.luma_degradation.highlight_bleed_threshold - 0.76).abs() < 1e-6);
    assert!((stages.luma_degradation.highlight_bleed_amount - 0.06642922).abs() < 1e-6);
    assert_eq!(stages.chroma_degradation.phase_error_rad, 0.0);
    assert!((stages.reconstruction_output.dropout_line_probability - 0.002).abs() < 1e-6);
    assert!((stages.reconstruction_output.dropout_span_px - 20.25).abs() < 1e-6);
    assert!(
        (stages.reconstruction_output.chroma_phase_noise_rad - 1.5_f32.to_radians()).abs() < 1e-6
    );
    assert_eq!(stages.reconstruction_output.head_switching_band_lines, 6.0);
    assert!((stages.reconstruction_output.head_switching_offset_px - 20.25).abs() < 1e-6);
}

#[test]
fn effect_uniforms_are_grouped_by_logical_stage() {
    let input = gradient_rgba8_image(FrameSize::new(720, 480));
    let pipeline = StillImagePipeline::default();
    let uniforms = effect_uniforms(&input, &pipeline);

    assert_eq!(uniforms.frame[2], 6.0);
    assert_eq!(uniforms.frame[3], 0.0);
    assert!((uniforms.input_conditioning[0] - 0.64).abs() < 1e-6);
    assert!((uniforms.luma_degradation[1] - 0.045).abs() < 1e-6);
    assert!((uniforms.luma_degradation[2] - 0.76).abs() < 1e-6);
    assert!((uniforms.luma_degradation[3] - 0.06642922).abs() < 1e-6);
    assert!((uniforms.chroma_degradation[3] - 0.35).abs() < 1e-6);
    assert!((uniforms.reconstruction_output[2] - 0.02).abs() < 1e-6);
    assert!((uniforms.reconstruction_output[3] - 20.25).abs() < 1e-6);
    assert!((uniforms.reconstruction_aux[0] - 0.002).abs() < 1e-6);
    assert!((uniforms.reconstruction_aux[1] - 20.25).abs() < 1e-6);
    assert_eq!(uniforms.reconstruction_aux[2], 0.0);
    assert!((uniforms.reconstruction_aux[3] - 1.5_f32.to_radians()).abs() < 1e-6);
}

#[test]
fn manual_preview_guardrails_soft_limit_glitch_prone_controls() {
    let input = gradient_rgba8_image(FrameSize::new(720, 480));
    let pipeline = StillImagePipeline::new(SignalSettings {
        luma: casseted_signal::LumaSettings { blur_px: 9.0 },
        chroma: ChromaSettings {
            offset_px: -3.0,
            bleed_px: 0.1,
            saturation: 1.0,
        },
        noise: NoiseSettings {
            luma_amount: 0.25,
            chroma_amount: 0.20,
        },
        tracking: TrackingSettings {
            line_jitter_px: -4.0,
            vertical_offset_lines: 2.0,
        },
        ..SignalSettings::neutral()
    });

    let effective = pipeline.effective_preview_signal();
    let stages = resolve_still_stages(&input, &pipeline);

    assert!(effective.luma.blur_px > 3.25);
    assert!(effective.luma.blur_px < 4.75);
    assert!(effective.chroma.offset_px < 0.0);
    assert!(effective.chroma.offset_px.abs() < 0.60);
    assert!(effective.chroma.bleed_px >= effective.chroma.offset_px.abs() * 2.5);
    assert!(effective.noise.luma_amount < 0.04);
    assert!(effective.noise.chroma_amount < 0.025);
    assert!(effective.tracking.line_jitter_px < 0.55);
    assert!(effective.tracking.vertical_offset_lines.abs() < 0.75);
    assert!((stages.chroma_degradation.offset_px - effective.chroma.offset_px).abs() < 1e-6);
    assert!(
        (stages.input_conditioning.line_jitter_px - effective.tracking.line_jitter_px).abs() < 1e-6
    );
}

#[test]
fn model_path_applies_guardrails_when_preview_overrides_diverge_from_projection() {
    let input = gradient_rgba8_image(FrameSize::new(720, 480));
    let mut pipeline = StillImagePipeline::default();
    pipeline.set_preview_overrides(SignalOverrides {
        chroma: ChromaOverrides {
            offset_px: Some(2.0),
            bleed_px: Some(0.0),
            ..ChromaOverrides::default()
        },
        noise: NoiseOverrides {
            luma_amount: Some(0.2),
            chroma_amount: Some(0.2),
        },
        tracking: TrackingOverrides {
            line_jitter_px: Some(3.0),
            ..TrackingOverrides::default()
        },
        ..SignalOverrides::default()
    });

    let effective = pipeline.effective_preview_signal();
    let stages = resolve_still_stages(&input, &pipeline);

    assert!(effective.chroma.offset_px < 0.60);
    assert!(effective.chroma.bleed_px >= effective.chroma.offset_px.abs() * 2.5);
    assert!(effective.noise.luma_amount < 0.04);
    assert!(effective.noise.chroma_amount < 0.025);
    assert!(effective.tracking.line_jitter_px < 0.55);
    assert!((stages.chroma_degradation.offset_px - effective.chroma.offset_px).abs() < 1e-6);
    assert!(
        (stages.reconstruction_output.luma_contamination_amount - effective.noise.luma_amount)
            .abs()
            < 1e-6
    );
    assert!(
        (stages.reconstruction_output.chroma_contamination_amount - effective.noise.chroma_amount)
            .abs()
            < 1e-6
    );
}

#[test]
fn model_override_guardrails_do_not_rewrite_untouched_projected_terms() {
    let mut model = VhsModel::default();
    model.tone.highlight_soft_knee = 1.0;
    model.tone.highlight_compression = 0.0;
    model.transport.vertical_wander_lines = 0.05;
    let mut pipeline = StillImagePipeline::from_vhs_model(model);
    pipeline.set_preview_overrides(SignalOverrides {
        chroma: ChromaOverrides {
            offset_px: Some(2.0),
            bleed_px: Some(0.0),
            ..ChromaOverrides::default()
        },
        ..SignalOverrides::default()
    });

    let effective = pipeline.effective_preview_signal();

    assert_eq!(effective.tone.highlight_soft_knee, 1.0);
    assert_eq!(effective.tone.highlight_compression, 0.0);
    assert_eq!(effective.tracking.vertical_offset_lines, 0.05);
    assert!(effective.chroma.offset_px < 0.60);
    assert!(effective.chroma.bleed_px >= effective.chroma.offset_px.abs() * 2.5);
}

#[test]
fn explicit_override_intent_survives_model_reprojection() {
    let mut pipeline = StillImagePipeline::default();
    let preserved_offset = pipeline.preview_base_signal().chroma.offset_px;
    pipeline.set_preview_overrides(SignalOverrides {
        chroma: ChromaOverrides {
            offset_px: Some(preserved_offset),
            ..ChromaOverrides::default()
        },
        ..SignalOverrides::default()
    });

    let mut updated_model = VhsModel::default();
    updated_model.chroma.delay_us = -0.08;
    pipeline.set_model(updated_model);

    assert_eq!(
        pipeline.preview_overrides().chroma.offset_px,
        Some(preserved_offset)
    );
    assert!((pipeline.preview_signal().chroma.offset_px - preserved_offset).abs() < 1e-6);
    assert!(pipeline.preview_base_signal().chroma.offset_px < 0.0);
    assert_ne!(
        pipeline.preview_signal().chroma.offset_px,
        pipeline.preview_base_signal().chroma.offset_px
    );
}

#[test]
fn model_projection_preserves_signed_chroma_delay() {
    let mut model = VhsModel::default();
    model.chroma.delay_us = -0.08;

    let pipeline = StillImagePipeline::from_vhs_model(model);

    assert!(pipeline.preview_base_signal().chroma.offset_px < 0.0);
}

#[test]
fn chroma_phase_terms_bypass_preview_projection_but_change_runtime_stage_state() {
    let input = gradient_rgba8_image(FrameSize::new(720, 480));
    let mut base_model = VhsModel::default();
    base_model.noise.chroma_phase_noise_deg = 0.0;
    let mut phase_model = base_model;
    phase_model.chroma.phase_error_deg = 18.0;
    phase_model.noise.chroma_phase_noise_deg = 9.0;

    let base_pipeline = StillImagePipeline::from_vhs_model(base_model);
    let phase_pipeline = StillImagePipeline::from_vhs_model(phase_model);
    let base_stages = resolve_still_stages(&input, &base_pipeline);
    let phase_stages = resolve_still_stages(&input, &phase_pipeline);

    assert_eq!(
        base_pipeline.preview_base_signal(),
        phase_pipeline.preview_base_signal()
    );
    assert_eq!(base_stages.chroma_degradation.phase_error_rad, 0.0);
    assert_eq!(
        base_stages.reconstruction_output.chroma_phase_noise_rad,
        0.0
    );
    assert!((phase_stages.chroma_degradation.phase_error_rad - 18.0_f32.to_radians()).abs() < 1e-6);
    assert!(
        (phase_stages.reconstruction_output.chroma_phase_noise_rad - 9.0_f32.to_radians()).abs()
            < 1e-6
    );
    assert_ne!(
        effect_uniforms(&input, &base_pipeline),
        effect_uniforms(&input, &phase_pipeline)
    );
}

#[test]
fn head_switching_terms_bypass_preview_projection_but_change_runtime_stage_state() {
    let input = gradient_rgba8_image(FrameSize::new(720, 480));
    let mut base_model = VhsModel::default();
    base_model.transport.head_switching_band_lines = 0;
    base_model.transport.head_switching_offset_us = 0.0;
    let mut switching_model = base_model;
    switching_model.transport.head_switching_band_lines = 12;
    switching_model.transport.head_switching_offset_us = 2.0;

    let base_pipeline = StillImagePipeline::from_vhs_model(base_model);
    let switching_pipeline = StillImagePipeline::from_vhs_model(switching_model);
    let base_stages = resolve_still_stages(&input, &base_pipeline);
    let switching_stages = resolve_still_stages(&input, &switching_pipeline);

    assert_eq!(
        base_pipeline.preview_base_signal(),
        switching_pipeline.preview_base_signal()
    );
    assert_eq!(base_stages.reconstruction_output.head_switching_band_lines, 0.0);
    assert_eq!(base_stages.reconstruction_output.head_switching_offset_px, 0.0);
    assert_eq!(
        switching_stages.reconstruction_output.head_switching_band_lines,
        12.0
    );
    assert!((switching_stages.reconstruction_output.head_switching_offset_px - 27.0).abs() < 1e-6);
    assert_ne!(
        effect_uniforms(&input, &base_pipeline),
        effect_uniforms(&input, &switching_pipeline)
    );
}

#[test]
fn head_switching_terms_pack_into_documented_runtime_uniform_lanes() {
    let input = gradient_rgba8_image(FrameSize::new(720, 480));
    let mut base_model = VhsModel::default();
    base_model.transport.head_switching_band_lines = 0;
    base_model.transport.head_switching_offset_us = 0.0;
    let mut switching_model = base_model;
    switching_model.transport.head_switching_band_lines = 12;
    switching_model.transport.head_switching_offset_us = 2.0;

    let base_pipeline = StillImagePipeline::from_vhs_model(base_model);
    let switching_pipeline = StillImagePipeline::from_vhs_model(switching_model);
    let base_uniforms = effect_uniforms(&input, &base_pipeline);
    let switching_uniforms = effect_uniforms(&input, &switching_pipeline);

    assert_eq!(
        base_pipeline.preview_base_signal(),
        switching_pipeline.preview_base_signal()
    );
    assert_eq!(base_uniforms.frame[2], 0.0);
    assert_eq!(base_uniforms.reconstruction_output[3], 0.0);
    assert_eq!(switching_uniforms.frame[2], 12.0);
    assert!((switching_uniforms.reconstruction_output[3] - 27.0).abs() < 1e-6);
    assert_ne!(base_uniforms, switching_uniforms);
}

#[test]
fn input_decode_selectors_remain_documented_only_in_runtime_subset() {
    let input = gradient_rgba8_image(FrameSize::new(720, 480));
    let base_model = VhsModel::default();
    let mut deferred_only = base_model;
    deferred_only.standard = VideoStandard::Pal;
    deferred_only.input.matrix = VideoMatrix::Bt601;
    deferred_only.input.transfer = InputTransfer::Bt601;
    deferred_only.input.temporal_sampling = TemporalSampling::InterlacedFields;

    let base_pipeline = StillImagePipeline::from_vhs_model(base_model);
    let deferred_pipeline = StillImagePipeline::from_vhs_model(deferred_only);

    assert_eq!(
        base_pipeline.preview_base_signal(),
        deferred_pipeline.preview_base_signal()
    );
    assert_eq!(
        effect_uniforms(&input, &base_pipeline),
        effect_uniforms(&input, &deferred_pipeline)
    );
}

#[test]
fn output_transfer_selector_is_currently_deferred_in_runtime_subset() {
    let input = gradient_rgba8_image(FrameSize::new(720, 480));
    let base_model = VhsModel::default();
    let mut deferred_only = base_model;
    deferred_only.decode.output_transfer = OutputTransfer::Bt1886Like;

    let base_pipeline = StillImagePipeline::from_vhs_model(base_model);
    let deferred_pipeline = StillImagePipeline::from_vhs_model(deferred_only);
    let base_stages = resolve_still_stages(&input, &base_pipeline);
    let deferred_stages = resolve_still_stages(&input, &deferred_pipeline);

    assert_eq!(
        base_pipeline.preview_base_signal(),
        deferred_pipeline.preview_base_signal()
    );
    assert_eq!(base_stages, deferred_stages);
    assert_eq!(
        effect_uniforms(&input, &base_pipeline),
        effect_uniforms(&input, &deferred_pipeline)
    );
}

#[test]
fn still_image_pipeline_modifies_pixels_when_gpu_is_available() {
    let gpu = match pollster::block_on(GpuContext::request(&GpuContextDescriptor::default())) {
        Ok(context) => context,
        Err(GpuInitError::AdapterNotFound) => return,
        Err(error) => panic!("failed to initialize gpu context: {error}"),
    };

    let input = gradient_rgba8_image(FrameSize::new(8, 8));

    let output = StillImagePipeline::default()
        .process_with_gpu(&gpu, &input)
        .expect("pipeline should process the image");

    assert_images_not_identical(&input, &output);
}

#[test]
fn chroma_phase_terms_modify_gpu_output_when_gpu_is_available() {
    let gpu = match pollster::block_on(GpuContext::request(&GpuContextDescriptor::default())) {
        Ok(context) => context,
        Err(GpuInitError::AdapterNotFound) => return,
        Err(error) => panic!("failed to initialize gpu context: {error}"),
    };

    let input = reference_card_rgba8_image(FrameSize::new(64, 48));
    let mut base_model = VhsModel::default();
    base_model.noise.chroma_phase_noise_deg = 0.0;
    let mut phase_model = base_model;
    phase_model.chroma.phase_error_deg = 14.0;
    phase_model.noise.chroma_phase_noise_deg = 5.0;

    let base_output = StillImagePipeline::from_vhs_model(base_model)
        .process_with_gpu(&gpu, &input)
        .expect("base pipeline should process the image");
    let phase_output = StillImagePipeline::from_vhs_model(phase_model)
        .process_with_gpu(&gpu, &input)
        .expect("phase-aware pipeline should process the image");

    assert_images_not_identical(&base_output, &phase_output);
}

#[test]
fn head_switching_terms_modify_gpu_output_when_gpu_is_available() {
    let gpu = match pollster::block_on(GpuContext::request(&GpuContextDescriptor::default())) {
        Ok(context) => context,
        Err(GpuInitError::AdapterNotFound) => return,
        Err(error) => panic!("failed to initialize gpu context: {error}"),
    };

    let input = reference_card_rgba8_image(FrameSize::new(64, 48));
    let mut base_model = VhsModel::default();
    base_model.transport.head_switching_band_lines = 0;
    base_model.transport.head_switching_offset_us = 0.0;
    let mut switching_model = base_model;
    switching_model.transport.head_switching_band_lines = 10;
    switching_model.transport.head_switching_offset_us = 2.0;

    let base_output = StillImagePipeline::from_vhs_model(base_model)
        .process_with_gpu(&gpu, &input)
        .expect("base pipeline should process the image");
    let switching_output = StillImagePipeline::from_vhs_model(switching_model)
        .process_with_gpu(&gpu, &input)
        .expect("head-switching-aware pipeline should process the image");

    assert_images_not_identical(&base_output, &switching_output);
}

#[test]
fn compiled_runtime_can_be_reused_across_repeated_runs() {
    let gpu = match pollster::block_on(GpuContext::request(&GpuContextDescriptor::default())) {
        Ok(context) => context,
        Err(GpuInitError::AdapterNotFound) => return,
        Err(error) => panic!("failed to initialize gpu context: {error}"),
    };

    let runtime = StillPipelineRuntime::new(&gpu);
    let input_a = gradient_rgba8_image(FrameSize::new(8, 8));
    let input_b = gradient_rgba8_image(FrameSize::new(12, 10));
    let pipeline = StillImagePipeline::default();

    let output_a = pipeline
        .process_with_runtime(&runtime, &input_a)
        .expect("runtime should process the first image");
    let output_b = pipeline
        .process_with_runtime(&runtime, &input_b)
        .expect("runtime should process the second image");
    let legacy_output_a = pipeline
        .process_with_gpu(&gpu, &input_a)
        .expect("legacy gpu entry point should remain functional");

    assert_images_not_identical(&input_a, &output_a);
    assert_images_not_identical(&input_b, &output_b);
    assert_eq!(output_a, legacy_output_a);
}
