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
const REFERENCE_WIDTH_PX: f32 = 720.0;
const BT601_SAMPLES_PER_US: f32 = 13.5;
const STILL_JITTER_ATTENUATION: f32 = 0.5;
const REFERENCE_LUMA_BANDWIDTH_MHZ: f32 = 4.2;
const REFERENCE_CHROMA_BANDWIDTH_KHZ: f32 = 1000.0;

#[derive(Debug, Clone, PartialEq)]
pub struct StillImagePipeline {
    pub model: Option<VhsModel>,
    pub signal: SignalSettings,
    pub shader_id: ShaderId,
}

impl StillImagePipeline {
    pub fn new(signal: SignalSettings) -> Self {
        Self {
            model: None,
            signal,
            shader_id: ShaderId::StillAnalog,
        }
    }

    pub fn from_vhs_model(model: VhsModel) -> Self {
        Self {
            model: Some(model),
            signal: prototype_signal_from_model(model),
            shader_id: ShaderId::StillAnalog,
        }
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
        let input_texture = create_input_texture(context, input);
        let input_view = input_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let output_texture = create_output_texture(context, texture_size);
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("casseted-still-image-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..wgpu::SamplerDescriptor::default()
        });

        let uniform_bytes = effect_uniform_bytes(input, self.signal, self.model);
        let uniform_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("casseted-still-image-uniforms"),
            size: uniform_bytes.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        context
            .queue
            .write_buffer(&uniform_buffer, 0, &uniform_bytes);

        let bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("casseted-still-image-bind-group-layout"),
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
                });

        let bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("casseted-still-image-bind-group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&input_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            });

        let shader = shader_source(self.shader_id);
        let shader = context.create_shader_module(Some(shader.label), shader.source);
        let pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("casseted-still-image-pipeline-layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });
        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("casseted-still-image-pipeline"),
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
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview: None,
                });

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

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("casseted-still-image-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
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
            render_pass.set_pipeline(&render_pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

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

fn prototype_signal_from_model(model: VhsModel) -> SignalSettings {
    SignalSettings {
        tone: ToneSettings {
            highlight_soft_knee: model.tone.highlight_soft_knee,
            highlight_compression: model.tone.highlight_compression,
        },
        luma: LumaSettings {
            blur_px: luma_blur_from_bandwidth(model.luma.bandwidth_mhz),
        },
        chroma: ChromaSettings {
            offset_px: (model.chroma.delay_us.max(0.0) * BT601_SAMPLES_PER_US).max(0.0),
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

fn luma_blur_from_bandwidth(bandwidth_mhz: f32) -> f32 {
    (((REFERENCE_LUMA_BANDWIDTH_MHZ - bandwidth_mhz).max(0.0)) / 1.2 * 1.25).min(4.0)
}

fn chroma_bleed_from_bandwidth(bandwidth_khz: f32) -> f32 {
    (((REFERENCE_CHROMA_BANDWIDTH_KHZ - bandwidth_khz).max(0.0)) / 400.0).min(4.0)
}

fn luma_noise_amount_from_sigma(luma_sigma: f32) -> f32 {
    (luma_sigma.max(0.0) * 1.25).min(1.0)
}

fn chroma_noise_amount_from_sigma(chroma_sigma: f32) -> f32 {
    (chroma_sigma.max(0.0) * 0.5).min(1.0)
}

fn detail_mix_from_preemphasis(preemphasis_db: f32) -> f32 {
    (preemphasis_db.max(0.0) * 0.025).min(0.20)
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
    let texture = context.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("casseted-still-image-input"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

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

fn create_output_texture(context: &GpuContext, size: FrameSize) -> wgpu::Texture {
    context.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("casseted-still-image-output"),
        size: texture_extent(size),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct EffectUniforms {
    frame: [f32; 4],
    tone_luma: [f32; 4],
    chroma: [f32; 4],
    transport: [f32; 4],
    noise_decode: [f32; 4],
}

impl EffectUniforms {
    fn as_bytes(self) -> [u8; EFFECT_UNIFORM_FLOATS * 4] {
        let floats = [
            self.frame[0],
            self.frame[1],
            self.frame[2],
            self.frame[3],
            self.tone_luma[0],
            self.tone_luma[1],
            self.tone_luma[2],
            self.tone_luma[3],
            self.chroma[0],
            self.chroma[1],
            self.chroma[2],
            self.chroma[3],
            self.transport[0],
            self.transport[1],
            self.transport[2],
            self.transport[3],
            self.noise_decode[0],
            self.noise_decode[1],
            self.noise_decode[2],
            self.noise_decode[3],
        ];

        let mut bytes = [0_u8; EFFECT_UNIFORM_FLOATS * 4];
        for (index, value) in floats.into_iter().enumerate() {
            let offset = index * 4;
            bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
        }

        bytes
    }
}

fn effect_uniforms(
    input: &ImageFrame,
    signal: SignalSettings,
    model: Option<VhsModel>,
) -> EffectUniforms {
    let width = input.descriptor.size.width as f32;
    let height = input.descriptor.size.height as f32;
    let frame_index = input.descriptor.frame_index as f32;
    let reference_scale = (width / REFERENCE_WIDTH_PX).max(0.0);
    // `StillImagePipeline::new(signal)` uses the narrower manual preview path,
    // so model-only terms stay neutral when no formal model is attached.
    let detail_mix = model
        .map(|vhs| detail_mix_from_preemphasis(vhs.luma.preemphasis_db))
        .unwrap_or(0.0);
    let chroma_vertical_blend = model
        .map(|vhs| vhs.decode.chroma_vertical_blend.clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let luma_chroma_crosstalk = model
        .map(|vhs| vhs.decode.luma_chroma_crosstalk.clamp(0.0, 1.0))
        .unwrap_or(0.0);

    EffectUniforms {
        frame: [width, height, width.recip(), height.recip()],
        tone_luma: [
            signal.tone.highlight_soft_knee.clamp(0.0, 0.999),
            signal.tone.highlight_compression.max(0.0),
            signal.luma.blur_px.max(0.0) * reference_scale,
            detail_mix,
        ],
        chroma: [
            signal.chroma.offset_px * reference_scale,
            signal.chroma.bleed_px.max(0.0) * reference_scale,
            signal.chroma.saturation.max(0.0),
            chroma_vertical_blend,
        ],
        transport: [
            signal.tracking.line_jitter_px * reference_scale,
            signal.tracking.vertical_offset_lines,
            frame_index,
            0.0,
        ],
        noise_decode: [
            signal.noise.luma_amount.max(0.0),
            signal.noise.chroma_amount.max(0.0),
            luma_chroma_crosstalk,
            0.0,
        ],
    }
}

fn effect_uniform_bytes(
    input: &ImageFrame,
    signal: SignalSettings,
    model: Option<VhsModel>,
) -> [u8; EFFECT_UNIFORM_FLOATS * 4] {
    effect_uniforms(input, signal, model).as_bytes()
}

#[cfg(test)]
mod tests {
    use super::{StillImagePipeline, effect_uniform_bytes, effect_uniforms, padded_bytes_per_row};
    use casseted_gpu::{GpuContext, GpuContextDescriptor, GpuInitError};
    use casseted_signal::{SignalSettings, VhsModel};
    use casseted_testing::{assert_images_not_identical, gradient_rgba8_image};
    use casseted_types::FrameSize;

    #[test]
    fn pipeline_uses_still_analog_shader() {
        let pipeline = StillImagePipeline::default();

        assert_eq!(pipeline.shader_id.label(), "still_analog");
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
        assert_eq!(pipeline.signal.tone.highlight_soft_knee, 0.72);
        assert!(pipeline.signal.chroma.offset_px > 1.0);
        assert!(pipeline.signal.luma.blur_px > 0.0);
    }

    #[test]
    fn manual_pipeline_keeps_model_dependent_decode_terms_neutral() {
        let input = gradient_rgba8_image(FrameSize::new(720, 480));
        let uniforms = effect_uniforms(&input, SignalSettings::default(), None);

        assert_eq!(uniforms.tone_luma[3], 0.0);
        assert_eq!(uniforms.chroma[3], 0.0);
        assert_eq!(uniforms.noise_decode[2], 0.0);
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
