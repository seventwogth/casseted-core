//! Minimal still-image GPU pipeline for analog-inspired processing.

use casseted_gpu::{GpuContext, GpuContextDescriptor, GpuInitError};
use casseted_shaderlib::{ShaderId, shader_source};
use casseted_signal::{
    ChromaSettings, LumaSettings, NoiseSettings, SignalSettings, ToneSettings, TrackingSettings,
    VhsModel,
};
use casseted_types::{FrameSize, ImageDataError, ImageFrame, PixelFormat};
use std::fmt;
use std::sync::mpsc;

const BYTES_PER_PIXEL_RGBA8: u32 = 4;
const EFFECT_UNIFORM_FLOATS: usize = 20;
const INTERMEDIATE_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const REFERENCE_WIDTH_PX: f32 = 720.0;
const BT601_SAMPLES_PER_US: f32 = 13.5;
const STILL_JITTER_ATTENUATION: f32 = 0.22;
const STILL_CHROMA_DELAY_ATTENUATION: f32 = 0.4;
const REFERENCE_LUMA_BANDWIDTH_MHZ: f32 = 4.2;
const REFERENCE_CHROMA_BANDWIDTH_KHZ: f32 = 1000.0;
const PREVIEW_LUMA_BLUR_RECOMMENDED_CAP: f32 = 3.25;
const PREVIEW_LUMA_BLUR_HARD_CAP: f32 = 4.75;
const PREVIEW_CHROMA_OFFSET_RECOMMENDED_CAP: f32 = 0.35;
const PREVIEW_CHROMA_OFFSET_HARD_CAP: f32 = 0.60;
const PREVIEW_CHROMA_BLEED_RECOMMENDED_CAP: f32 = 3.0;
const PREVIEW_CHROMA_BLEED_HARD_CAP: f32 = 4.25;
const PREVIEW_CHROMA_BLEED_OFFSET_RATIO: f32 = 2.5;
const PREVIEW_LUMA_NOISE_RECOMMENDED_CAP: f32 = 0.02;
const PREVIEW_LUMA_NOISE_HARD_CAP: f32 = 0.04;
const PREVIEW_CHROMA_NOISE_RECOMMENDED_CAP: f32 = 0.012;
const PREVIEW_CHROMA_NOISE_HARD_CAP: f32 = 0.025;
const PREVIEW_LINE_JITTER_RECOMMENDED_CAP: f32 = 0.35;
const PREVIEW_LINE_JITTER_HARD_CAP: f32 = 0.55;
const PREVIEW_VERTICAL_OFFSET_RECOMMENDED_CAP: f32 = 0.35;
const PREVIEW_VERTICAL_OFFSET_HARD_CAP: f32 = 0.75;
const STILL_PIPELINE_SHADER_IDS: [ShaderId; 4] = [
    ShaderId::StillInputConditioning,
    ShaderId::StillLumaDegradation,
    ShaderId::StillChromaDegradation,
    ShaderId::StillReconstructionOutput,
];

#[derive(Debug, Clone, PartialEq)]
pub struct StillImagePipeline {
    pub model: Option<VhsModel>,
    pub signal: SignalSettings,
}

impl StillImagePipeline {
    pub fn new(signal: SignalSettings) -> Self {
        Self {
            model: None,
            signal,
        }
    }

    pub fn from_vhs_model(model: VhsModel) -> Self {
        Self {
            model: Some(model),
            signal: project_vhs_model_to_preview_signal(model),
        }
    }

    pub fn effective_preview_signal(&self) -> SignalSettings {
        effective_preview_signal(self.signal, self.model)
    }

    pub fn shader_ids(&self) -> &'static [ShaderId] {
        &STILL_PIPELINE_SHADER_IDS
    }

    pub fn process_blocking(&self, input: &ImageFrame) -> Result<ImageFrame, PipelineError> {
        let context = pollster::block_on(GpuContext::request(&GpuContextDescriptor::default()))
            .map_err(PipelineError::GpuInit)?;

        self.process_with_gpu(&context, input)
    }

    pub fn process_with_gpu(
        &self,
        context: &GpuContext,
        input: &ImageFrame,
    ) -> Result<ImageFrame, PipelineError> {
        validate_input_image(input)?;

        let texture_size = input.descriptor.size;
        let uniform_bytes = effect_uniform_bytes(input, self.signal, self.model);
        let input_texture = create_input_texture(context, input);
        let input_view = input_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let working_texture = create_pipeline_texture(
            context,
            texture_size,
            INTERMEDIATE_TEXTURE_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            "casseted-still-image-working-signal",
        );
        let working_view = working_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let luma_texture = create_pipeline_texture(
            context,
            texture_size,
            INTERMEDIATE_TEXTURE_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            "casseted-still-image-luma-signal",
        );
        let luma_view = luma_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let chroma_texture = create_pipeline_texture(
            context,
            texture_size,
            INTERMEDIATE_TEXTURE_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            "casseted-still-image-chroma-signal",
        );
        let chroma_view = chroma_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let output_texture = create_pipeline_texture(
            context,
            texture_size,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            "casseted-still-image-output",
        );
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = create_linear_sampler(context);
        let uniform_buffer = create_uniform_buffer(context, &uniform_bytes);
        context
            .queue
            .write_buffer(&uniform_buffer, 0, &uniform_bytes);

        let single_texture_layout =
            create_single_texture_bind_group_layout(context, "casseted-still-image-single-input");
        let dual_texture_layout =
            create_dual_texture_bind_group_layout(context, "casseted-still-image-dual-input");

        let conditioning_bind_group = create_single_texture_bind_group(
            context,
            &single_texture_layout,
            &input_view,
            &sampler,
            &uniform_buffer,
            "casseted-still-image-conditioning-bind-group",
        );
        let luma_bind_group = create_single_texture_bind_group(
            context,
            &single_texture_layout,
            &working_view,
            &sampler,
            &uniform_buffer,
            "casseted-still-image-luma-bind-group",
        );
        let chroma_bind_group = create_single_texture_bind_group(
            context,
            &single_texture_layout,
            &working_view,
            &sampler,
            &uniform_buffer,
            "casseted-still-image-chroma-bind-group",
        );
        let reconstruction_bind_group = create_dual_texture_bind_group(
            context,
            &dual_texture_layout,
            &luma_view,
            &chroma_view,
            &sampler,
            &uniform_buffer,
            "casseted-still-image-reconstruction-bind-group",
        );

        let conditioning_pipeline = create_render_pipeline(
            context,
            ShaderId::StillInputConditioning,
            &[&single_texture_layout],
            INTERMEDIATE_TEXTURE_FORMAT,
            "casseted-still-image-conditioning-pipeline",
        );
        let luma_pipeline = create_render_pipeline(
            context,
            ShaderId::StillLumaDegradation,
            &[&single_texture_layout],
            INTERMEDIATE_TEXTURE_FORMAT,
            "casseted-still-image-luma-pipeline",
        );
        let chroma_pipeline = create_render_pipeline(
            context,
            ShaderId::StillChromaDegradation,
            &[&single_texture_layout],
            INTERMEDIATE_TEXTURE_FORMAT,
            "casseted-still-image-chroma-pipeline",
        );
        let reconstruction_pipeline = create_render_pipeline(
            context,
            ShaderId::StillReconstructionOutput,
            &[&dual_texture_layout],
            wgpu::TextureFormat::Rgba8Unorm,
            "casseted-still-image-reconstruction-pipeline",
        );

        let padded_bytes_per_row = padded_bytes_per_row(texture_size.width);
        let output_buffer_size = padded_bytes_per_row as u64 * texture_size.height as u64;
        let readback_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("casseted-still-image-readback"),
            size: output_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("casseted-still-image-encoder"),
            });

        encode_fullscreen_pass(
            &mut encoder,
            &conditioning_pipeline,
            &conditioning_bind_group,
            &working_view,
            "casseted-still-image-input-conditioning-pass",
        );
        encode_fullscreen_pass(
            &mut encoder,
            &luma_pipeline,
            &luma_bind_group,
            &luma_view,
            "casseted-still-image-luma-degradation-pass",
        );
        encode_fullscreen_pass(
            &mut encoder,
            &chroma_pipeline,
            &chroma_bind_group,
            &chroma_view,
            "casseted-still-image-chroma-degradation-pass",
        );
        encode_fullscreen_pass(
            &mut encoder,
            &reconstruction_pipeline,
            &reconstruction_bind_group,
            &output_view,
            "casseted-still-image-reconstruction-pass",
        );

        encoder.copy_texture_to_buffer(
            output_texture.as_image_copy(),
            wgpu::ImageCopyBuffer {
                buffer: &readback_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(texture_size.height),
                },
            },
            wgpu::Extent3d {
                width: texture_size.width,
                height: texture_size.height,
                depth_or_array_layers: 1,
            },
        );

        context.queue.submit(Some(encoder.finish()));

        let buffer_slice = readback_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        context.device.poll(wgpu::Maintain::Wait);

        let map_result = receiver
            .recv()
            .map_err(|_| PipelineError::MapChannelClosed)?;
        map_result.map_err(PipelineError::BufferMap)?;

        let mapped = buffer_slice.get_mapped_range();
        let output_bytes = strip_padding(
            &mapped,
            texture_size.width,
            texture_size.height,
            padded_bytes_per_row,
        );
        drop(mapped);
        readback_buffer.unmap();

        ImageFrame::new(input.descriptor.clone(), output_bytes).map_err(PipelineError::ImageData)
    }
}

impl Default for StillImagePipeline {
    fn default() -> Self {
        Self::from_vhs_model(VhsModel::default())
    }
}

#[derive(Debug)]
pub enum PipelineError {
    EmptyFrame,
    UnsupportedPixelFormat(PixelFormat),
    GpuInit(GpuInitError),
    BufferMap(wgpu::BufferAsyncError),
    MapChannelClosed,
    ImageData(ImageDataError),
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFrame => f.write_str("still-image pipeline received an empty frame"),
            Self::UnsupportedPixelFormat(format) => {
                write!(
                    f,
                    "still-image pipeline currently supports only RGBA8 input, got {format:?}"
                )
            }
            Self::GpuInit(error) => write!(f, "{error}"),
            Self::BufferMap(error) => write!(f, "failed to map GPU readback buffer: {error}"),
            Self::MapChannelClosed => f.write_str("GPU readback channel closed before completion"),
            Self::ImageData(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PipelineError {}

fn project_vhs_model_to_preview_signal(model: VhsModel) -> SignalSettings {
    SignalSettings {
        tone: ToneSettings {
            highlight_soft_knee: model.tone.highlight_soft_knee,
            highlight_compression: model.tone.highlight_compression,
        },
        luma: LumaSettings {
            blur_px: luma_blur_from_bandwidth(model.luma.bandwidth_mhz),
        },
        chroma: ChromaSettings {
            offset_px: chroma_offset_from_delay(model.chroma.delay_us),
            bleed_px: chroma_bleed_from_bandwidth(model.chroma.bandwidth_khz),
            saturation: model.chroma.saturation_gain.max(0.0),
        },
        noise: NoiseSettings {
            luma_amount: luma_noise_amount_from_sigma(model.noise.luma_sigma),
            chroma_amount: chroma_noise_amount_from_sigma(model.noise.chroma_sigma),
        },
        tracking: TrackingSettings {
            line_jitter_px: line_jitter_px_from_timebase(model.transport.line_jitter_us),
            vertical_offset_lines: model.transport.vertical_wander_lines,
        },
    }
}

fn line_jitter_px_from_timebase(line_jitter_us: f32) -> f32 {
    (line_jitter_us.max(0.0) * BT601_SAMPLES_PER_US * STILL_JITTER_ATTENUATION).max(0.0)
}

fn chroma_offset_from_delay(delay_us: f32) -> f32 {
    (delay_us.max(0.0) * BT601_SAMPLES_PER_US * STILL_CHROMA_DELAY_ATTENUATION).max(0.0)
}

fn luma_blur_from_bandwidth(bandwidth_mhz: f32) -> f32 {
    (((REFERENCE_LUMA_BANDWIDTH_MHZ - bandwidth_mhz).max(0.0)) / 1.0 * 1.6).min(4.5)
}

fn chroma_bleed_from_bandwidth(bandwidth_khz: f32) -> f32 {
    (((REFERENCE_CHROMA_BANDWIDTH_KHZ - bandwidth_khz).max(0.0)) / 300.0).min(4.5)
}

fn luma_noise_amount_from_sigma(luma_sigma: f32) -> f32 {
    luma_sigma.clamp(0.0, 1.0)
}

fn chroma_noise_amount_from_sigma(chroma_sigma: f32) -> f32 {
    (chroma_sigma.max(0.0) * 0.35).min(1.0)
}

fn detail_mix_from_preemphasis(preemphasis_db: f32) -> f32 {
    (preemphasis_db.max(0.0) * 0.015).min(0.12)
}

fn effective_preview_signal(signal: SignalSettings, model: Option<VhsModel>) -> SignalSettings {
    if uses_model_projected_preview_signal(signal, model) {
        signal
    } else {
        normalize_manual_preview_signal(signal)
    }
}

fn uses_model_projected_preview_signal(signal: SignalSettings, model: Option<VhsModel>) -> bool {
    model
        .map(project_vhs_model_to_preview_signal)
        .is_some_and(|projected| signal == projected)
}

fn normalize_manual_preview_signal(signal: SignalSettings) -> SignalSettings {
    let chroma_offset_px = soft_cap_signed(
        signal.chroma.offset_px,
        PREVIEW_CHROMA_OFFSET_RECOMMENDED_CAP,
        PREVIEW_CHROMA_OFFSET_HARD_CAP,
    );
    let chroma_bleed_px = soft_cap_magnitude(
        signal.chroma.bleed_px,
        PREVIEW_CHROMA_BLEED_RECOMMENDED_CAP,
        PREVIEW_CHROMA_BLEED_HARD_CAP,
    )
    .max(chroma_offset_px.abs() * PREVIEW_CHROMA_BLEED_OFFSET_RATIO);

    SignalSettings {
        tone: ToneSettings {
            highlight_soft_knee: signal.tone.highlight_soft_knee.clamp(0.0, 0.999),
            highlight_compression: signal.tone.highlight_compression.max(0.0),
        },
        luma: LumaSettings {
            blur_px: soft_cap_magnitude(
                signal.luma.blur_px,
                PREVIEW_LUMA_BLUR_RECOMMENDED_CAP,
                PREVIEW_LUMA_BLUR_HARD_CAP,
            ),
        },
        chroma: ChromaSettings {
            offset_px: chroma_offset_px,
            bleed_px: chroma_bleed_px,
            saturation: signal.chroma.saturation.max(0.0),
        },
        noise: NoiseSettings {
            luma_amount: soft_cap_magnitude(
                signal.noise.luma_amount,
                PREVIEW_LUMA_NOISE_RECOMMENDED_CAP,
                PREVIEW_LUMA_NOISE_HARD_CAP,
            ),
            chroma_amount: soft_cap_magnitude(
                signal.noise.chroma_amount,
                PREVIEW_CHROMA_NOISE_RECOMMENDED_CAP,
                PREVIEW_CHROMA_NOISE_HARD_CAP,
            ),
        },
        tracking: TrackingSettings {
            line_jitter_px: soft_cap_magnitude(
                signal.tracking.line_jitter_px.abs(),
                PREVIEW_LINE_JITTER_RECOMMENDED_CAP,
                PREVIEW_LINE_JITTER_HARD_CAP,
            ),
            vertical_offset_lines: soft_cap_signed(
                signal.tracking.vertical_offset_lines,
                PREVIEW_VERTICAL_OFFSET_RECOMMENDED_CAP,
                PREVIEW_VERTICAL_OFFSET_HARD_CAP,
            ),
        },
    }
}

fn soft_cap_magnitude(value: f32, recommended_cap: f32, hard_cap: f32) -> f32 {
    let magnitude = value.max(0.0);
    if magnitude <= recommended_cap {
        return magnitude;
    }

    let span = (hard_cap - recommended_cap).max(f32::EPSILON);
    let excess = magnitude - recommended_cap;
    recommended_cap + (excess * span) / (excess + span)
}

fn soft_cap_signed(value: f32, recommended_cap: f32, hard_cap: f32) -> f32 {
    value.signum() * soft_cap_magnitude(value.abs(), recommended_cap, hard_cap)
}

fn validate_input_image(input: &ImageFrame) -> Result<(), PipelineError> {
    if input.descriptor.size.is_empty() {
        return Err(PipelineError::EmptyFrame);
    }

    if input.descriptor.format != PixelFormat::Rgba8Unorm {
        return Err(PipelineError::UnsupportedPixelFormat(
            input.descriptor.format,
        ));
    }

    Ok(())
}

fn create_input_texture(context: &GpuContext, input: &ImageFrame) -> wgpu::Texture {
    let size = texture_extent(input.descriptor.size);
    let texture = create_pipeline_texture(
        context,
        input.descriptor.size,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        "casseted-still-image-input",
    );

    context.queue.write_texture(
        texture.as_image_copy(),
        input.as_bytes(),
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(input.descriptor.size.width * BYTES_PER_PIXEL_RGBA8),
            rows_per_image: Some(input.descriptor.size.height),
        },
        size,
    );

    texture
}

fn create_pipeline_texture(
    context: &GpuContext,
    size: FrameSize,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    label: &'static str,
) -> wgpu::Texture {
    context.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: texture_extent(size),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn create_linear_sampler(context: &GpuContext) -> wgpu::Sampler {
    context.device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("casseted-still-image-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..wgpu::SamplerDescriptor::default()
    })
}

fn create_uniform_buffer(context: &GpuContext, uniform_bytes: &[u8]) -> wgpu::Buffer {
    context.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("casseted-still-image-uniforms"),
        size: uniform_bytes.len() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_single_texture_bind_group_layout(
    context: &GpuContext,
    label: &'static str,
) -> wgpu::BindGroupLayout {
    context
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
}

fn create_dual_texture_bind_group_layout(
    context: &GpuContext,
    label: &'static str,
) -> wgpu::BindGroupLayout {
    context
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
}

fn create_single_texture_bind_group(
    context: &GpuContext,
    layout: &wgpu::BindGroupLayout,
    texture_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform_buffer: &wgpu::Buffer,
    label: &'static str,
) -> wgpu::BindGroup {
    context
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        })
}

fn create_dual_texture_bind_group(
    context: &GpuContext,
    layout: &wgpu::BindGroupLayout,
    first_texture_view: &wgpu::TextureView,
    second_texture_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform_buffer: &wgpu::Buffer,
    label: &'static str,
) -> wgpu::BindGroup {
    context
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(first_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(second_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        })
}

fn create_render_pipeline(
    context: &GpuContext,
    shader_id: ShaderId,
    bind_group_layouts: &[&wgpu::BindGroupLayout],
    target_format: wgpu::TextureFormat,
    label: &'static str,
) -> wgpu::RenderPipeline {
    let shader_source = shader_source(shader_id);
    let shader = context.create_shader_module(Some(shader_source.label), shader_source.source);
    let pipeline_layout = context
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts,
            push_constant_ranges: &[],
        });

    context
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        })
}

fn encode_fullscreen_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    bind_group: &wgpu::BindGroup,
    target_view: &wgpu::TextureView,
    label: &'static str,
) {
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, bind_group, &[]);
    render_pass.draw(0..3, 0..1);
}

fn texture_extent(size: FrameSize) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: size.width,
        height: size.height,
        depth_or_array_layers: 1,
    }
}

fn padded_bytes_per_row(width: u32) -> u32 {
    let unpadded = width * BYTES_PER_PIXEL_RGBA8;
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let remainder = unpadded % alignment;

    if remainder == 0 {
        unpadded
    } else {
        unpadded + (alignment - remainder)
    }
}

fn strip_padding(data: &[u8], width: u32, height: u32, padded_bytes_per_row: u32) -> Vec<u8> {
    let unpadded_bytes_per_row = (width * BYTES_PER_PIXEL_RGBA8) as usize;
    let padded_bytes_per_row = padded_bytes_per_row as usize;
    let mut output = Vec::with_capacity(unpadded_bytes_per_row * height as usize);

    for row in 0..height as usize {
        let start = row * padded_bytes_per_row;
        let end = start + unpadded_bytes_per_row;
        output.extend_from_slice(&data[start..end]);
    }

    output
}

// The still-image path resolves controls into explicit logical stages and then
// packs them into a shared uniform block used across the compact multi-pass run.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedStillStages {
    frame: FrameStage,
    input_conditioning: InputConditioningStage,
    luma_degradation: LumaDegradationStage,
    chroma_degradation: ChromaDegradationStage,
    reconstruction_output: ReconstructionOutputStage,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FrameStage {
    width: f32,
    height: f32,
    inv_width: f32,
    inv_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct InputConditioningStage {
    highlight_soft_knee: f32,
    highlight_compression: f32,
    line_jitter_px: f32,
    vertical_offset_lines: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LumaDegradationStage {
    blur_px: f32,
    detail_mix: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ChromaDegradationStage {
    offset_px: f32,
    // Shared chroma bandwidth-loss proxy. The chroma shader derives its
    // horizontal low-pass span and coarse reconstruction cell size from this.
    blur_px: f32,
    saturation: f32,
    vertical_blend: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ReconstructionOutputStage {
    luma_noise_amount: f32,
    chroma_noise_amount: f32,
    luma_chroma_crosstalk: f32,
    frame_index: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct EffectUniforms {
    frame: [f32; 4],
    input_conditioning: [f32; 4],
    luma_degradation: [f32; 4],
    chroma_degradation: [f32; 4],
    reconstruction_output: [f32; 4],
}

impl From<ResolvedStillStages> for EffectUniforms {
    fn from(stages: ResolvedStillStages) -> Self {
        Self {
            frame: [
                stages.frame.width,
                stages.frame.height,
                stages.frame.inv_width,
                stages.frame.inv_height,
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
                0.0,
                0.0,
            ],
            chroma_degradation: [
                stages.chroma_degradation.offset_px,
                stages.chroma_degradation.blur_px,
                stages.chroma_degradation.saturation,
                stages.chroma_degradation.vertical_blend,
            ],
            reconstruction_output: [
                stages.reconstruction_output.luma_noise_amount,
                stages.reconstruction_output.chroma_noise_amount,
                stages.reconstruction_output.luma_chroma_crosstalk,
                stages.reconstruction_output.frame_index,
            ],
        }
    }
}

impl EffectUniforms {
    fn as_bytes(self) -> [u8; EFFECT_UNIFORM_FLOATS * 4] {
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
        ];

        let mut bytes = [0_u8; EFFECT_UNIFORM_FLOATS * 4];
        for (index, value) in floats.into_iter().enumerate() {
            let offset = index * 4;
            bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        }

        bytes
    }
}

fn resolve_still_stages(
    input: &ImageFrame,
    signal: SignalSettings,
    model: Option<VhsModel>,
) -> ResolvedStillStages {
    let signal = effective_preview_signal(signal, model);
    let width = input.descriptor.size.width as f32;
    let height = input.descriptor.size.height as f32;
    let reference_scale = (width / REFERENCE_WIDTH_PX).max(0.0);

    ResolvedStillStages {
        frame: FrameStage {
            width,
            height,
            inv_width: width.recip(),
            inv_height: height.recip(),
        },
        input_conditioning: resolve_input_conditioning_stage(signal, reference_scale),
        luma_degradation: resolve_luma_degradation_stage(signal, reference_scale, model),
        chroma_degradation: resolve_chroma_degradation_stage(signal, reference_scale, model),
        reconstruction_output: resolve_reconstruction_output_stage(input, signal, model),
    }
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
    let detail_mix = model
        .map(|vhs| detail_mix_from_preemphasis(vhs.luma.preemphasis_db))
        .unwrap_or(0.0);

    LumaDegradationStage {
        blur_px: signal.luma.blur_px.max(0.0) * reference_scale,
        detail_mix,
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

    ChromaDegradationStage {
        offset_px: signal.chroma.offset_px * reference_scale,
        // Keep the stage contract compact: the pass now expands this one proxy
        // into low-pass, coarse chroma resolution loss, and restrained smear.
        blur_px: signal.chroma.bleed_px.max(0.0) * reference_scale,
        saturation: signal.chroma.saturation.max(0.0),
        vertical_blend,
    }
}

fn resolve_reconstruction_output_stage(
    input: &ImageFrame,
    signal: SignalSettings,
    model: Option<VhsModel>,
) -> ReconstructionOutputStage {
    let luma_chroma_crosstalk = model
        .map(|vhs| vhs.decode.luma_chroma_crosstalk.clamp(0.0, 1.0))
        .unwrap_or(0.0);

    ReconstructionOutputStage {
        luma_noise_amount: signal.noise.luma_amount.max(0.0),
        chroma_noise_amount: signal.noise.chroma_amount.max(0.0),
        luma_chroma_crosstalk,
        frame_index: input.descriptor.frame_index as f32,
    }
}

fn effect_uniforms(
    input: &ImageFrame,
    signal: SignalSettings,
    model: Option<VhsModel>,
) -> EffectUniforms {
    resolve_still_stages(input, signal, model).into()
}

fn effect_uniform_bytes(
    input: &ImageFrame,
    signal: SignalSettings,
    model: Option<VhsModel>,
) -> [u8; EFFECT_UNIFORM_FLOATS * 4] {
    effect_uniforms(input, signal, model).as_bytes()
}

#[cfg(test)]
mod stage_regression;

#[cfg(test)]
mod tests {
    use super::{
        StillImagePipeline, effect_uniform_bytes, effect_uniforms, padded_bytes_per_row,
        resolve_still_stages,
    };
    use casseted_gpu::{GpuContext, GpuContextDescriptor, GpuInitError};
    use casseted_shaderlib::ShaderId;
    use casseted_signal::{
        ChromaSettings, NoiseSettings, SignalSettings, TrackingSettings, VhsModel,
    };
    use casseted_testing::{assert_images_not_identical, gradient_rgba8_image};
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
        let padded = padded_bytes_per_row(17);

        assert_eq!(padded % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
        assert!(padded >= 17 * 4);
    }

    #[test]
    fn uniform_bytes_include_frame_dimensions() {
        let input = gradient_rgba8_image(FrameSize::new(8, 4));
        let pipeline = StillImagePipeline::default();
        let bytes = effect_uniform_bytes(&input, pipeline.signal, pipeline.model);

        assert_eq!(&bytes[0..4], &(8.0_f32).to_ne_bytes());
        assert_eq!(&bytes[4..8], &(4.0_f32).to_ne_bytes());
    }

    #[test]
    fn default_pipeline_projects_vhs_model_into_current_signal_settings() {
        let pipeline = StillImagePipeline::default();

        assert_eq!(pipeline.model, Some(VhsModel::default()));
        assert_eq!(pipeline.signal.tone.highlight_soft_knee, 0.64);
        assert!((pipeline.signal.chroma.offset_px - 0.324).abs() < 1e-6);
        assert!((pipeline.signal.luma.blur_px - 1.92).abs() < 1e-6);
    }

    #[test]
    fn manual_pipeline_keeps_model_dependent_decode_terms_neutral() {
        let input = gradient_rgba8_image(FrameSize::new(720, 480));
        let stages = resolve_still_stages(&input, SignalSettings::default(), None);

        assert_eq!(stages.luma_degradation.detail_mix, 0.0);
        assert_eq!(stages.chroma_degradation.vertical_blend, 0.0);
        assert_eq!(stages.reconstruction_output.luma_chroma_crosstalk, 0.0);
    }

    #[test]
    fn effect_uniforms_are_grouped_by_logical_stage() {
        let input = gradient_rgba8_image(FrameSize::new(720, 480));
        let pipeline = StillImagePipeline::default();
        let uniforms = effect_uniforms(&input, pipeline.signal, pipeline.model);

        assert!((uniforms.input_conditioning[0] - 0.64).abs() < 1e-6);
        assert!((uniforms.luma_degradation[1] - 0.045).abs() < 1e-6);
        assert!((uniforms.chroma_degradation[3] - 0.35).abs() < 1e-6);
        assert!((uniforms.reconstruction_output[2] - 0.02).abs() < 1e-6);
    }

    #[test]
    fn manual_preview_guardrails_soft_limit_glitch_prone_controls() {
        let input = gradient_rgba8_image(FrameSize::new(720, 480));
        let pipeline = StillImagePipeline::new(SignalSettings {
            luma: super::LumaSettings { blur_px: 9.0 },
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
        let stages = resolve_still_stages(&input, pipeline.signal, pipeline.model);

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
            (stages.input_conditioning.line_jitter_px - effective.tracking.line_jitter_px).abs()
                < 1e-6
        );
    }

    #[test]
    fn model_path_applies_guardrails_when_preview_overrides_diverge_from_projection() {
        let input = gradient_rgba8_image(FrameSize::new(720, 480));
        let mut pipeline = StillImagePipeline::default();
        pipeline.signal.chroma.offset_px = 2.0;
        pipeline.signal.chroma.bleed_px = 0.0;
        pipeline.signal.noise.luma_amount = 0.2;
        pipeline.signal.noise.chroma_amount = 0.2;
        pipeline.signal.tracking.line_jitter_px = 3.0;

        let effective = pipeline.effective_preview_signal();
        let stages = resolve_still_stages(&input, pipeline.signal, pipeline.model);

        assert!(effective.chroma.offset_px < 0.60);
        assert!(effective.chroma.bleed_px >= effective.chroma.offset_px.abs() * 2.5);
        assert!(effective.noise.luma_amount < 0.04);
        assert!(effective.noise.chroma_amount < 0.025);
        assert!(effective.tracking.line_jitter_px < 0.55);
        assert!((stages.chroma_degradation.offset_px - effective.chroma.offset_px).abs() < 1e-6);
        assert!(
            (stages.reconstruction_output.luma_noise_amount - effective.noise.luma_amount).abs()
                < 1e-6
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
}
