use crate::StillImagePipeline;
use crate::stages::effect_uniform_bytes;
use casseted_gpu::{GpuContext, GpuInitError};
use casseted_shaderlib::{ShaderId, shader_source};
use casseted_types::{FrameSize, ImageDataError, ImageFrame, PixelFormat};
use std::fmt;
use std::sync::mpsc;

const BYTES_PER_PIXEL_RGBA8: u32 = 4;
const INTERMEDIATE_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const OUTPUT_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

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

#[derive(Debug)]
pub struct StillPipelineRuntime<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    sampler: wgpu::Sampler,
    single_texture_layout: wgpu::BindGroupLayout,
    dual_texture_layout: wgpu::BindGroupLayout,
    pass_chain: CompiledStillPassChain,
}

#[derive(Debug)]
struct CompiledStillPassChain {
    conditioning: wgpu::RenderPipeline,
    luma: wgpu::RenderPipeline,
    chroma: wgpu::RenderPipeline,
    reconstruction: wgpu::RenderPipeline,
}

#[derive(Debug)]
struct StillRunResources {
    input_texture: wgpu::Texture,
    working_texture: wgpu::Texture,
    luma_texture: wgpu::Texture,
    chroma_texture: wgpu::Texture,
    output_texture: wgpu::Texture,
    uniform_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    padded_bytes_per_row: u32,
}

impl<'a> StillPipelineRuntime<'a> {
    pub fn new(context: &'a GpuContext) -> Self {
        let sampler = create_linear_sampler(&context.device);
        let single_texture_layout = create_single_texture_bind_group_layout(
            &context.device,
            "casseted-still-image-single-input",
        );
        let dual_texture_layout = create_dual_texture_bind_group_layout(
            &context.device,
            "casseted-still-image-dual-input",
        );
        let pass_chain = CompiledStillPassChain::new(
            &context.device,
            &single_texture_layout,
            &dual_texture_layout,
        );

        Self {
            device: &context.device,
            queue: &context.queue,
            sampler,
            single_texture_layout,
            dual_texture_layout,
            pass_chain,
        }
    }

    pub fn process(
        &self,
        pipeline: &StillImagePipeline,
        input: &ImageFrame,
    ) -> Result<ImageFrame, PipelineError> {
        validate_input_image(input)?;

        let texture_size = input.descriptor.size;
        let uniform_bytes = effect_uniform_bytes(input, pipeline);
        let resources = self.create_run_resources(input, &uniform_bytes);

        let input_view = resources
            .input_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let working_view = resources
            .working_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let luma_view = resources
            .luma_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let chroma_view = resources
            .chroma_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let output_view = resources
            .output_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let conditioning_bind_group = create_single_texture_bind_group(
            self.device,
            &self.single_texture_layout,
            &input_view,
            &self.sampler,
            &resources.uniform_buffer,
            "casseted-still-image-conditioning-bind-group",
        );
        let luma_bind_group = create_single_texture_bind_group(
            self.device,
            &self.single_texture_layout,
            &working_view,
            &self.sampler,
            &resources.uniform_buffer,
            "casseted-still-image-luma-bind-group",
        );
        let chroma_bind_group = create_single_texture_bind_group(
            self.device,
            &self.single_texture_layout,
            &working_view,
            &self.sampler,
            &resources.uniform_buffer,
            "casseted-still-image-chroma-bind-group",
        );
        let reconstruction_bind_group = create_dual_texture_bind_group(
            self.device,
            &self.dual_texture_layout,
            &luma_view,
            &chroma_view,
            &self.sampler,
            &resources.uniform_buffer,
            "casseted-still-image-reconstruction-bind-group",
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("casseted-still-image-encoder"),
            });

        encode_fullscreen_pass(
            &mut encoder,
            &self.pass_chain.conditioning,
            &conditioning_bind_group,
            &working_view,
            "casseted-still-image-input-conditioning-pass",
        );
        encode_fullscreen_pass(
            &mut encoder,
            &self.pass_chain.luma,
            &luma_bind_group,
            &luma_view,
            "casseted-still-image-luma-degradation-pass",
        );
        encode_fullscreen_pass(
            &mut encoder,
            &self.pass_chain.chroma,
            &chroma_bind_group,
            &chroma_view,
            "casseted-still-image-chroma-degradation-pass",
        );
        encode_fullscreen_pass(
            &mut encoder,
            &self.pass_chain.reconstruction,
            &reconstruction_bind_group,
            &output_view,
            "casseted-still-image-reconstruction-pass",
        );

        encoder.copy_texture_to_buffer(
            resources.output_texture.as_image_copy(),
            wgpu::ImageCopyBuffer {
                buffer: &resources.readback_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(resources.padded_bytes_per_row),
                    rows_per_image: Some(texture_size.height),
                },
            },
            texture_extent(texture_size),
        );

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = resources.readback_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);

        let map_result = receiver
            .recv()
            .map_err(|_| PipelineError::MapChannelClosed)?;
        map_result.map_err(PipelineError::BufferMap)?;

        let mapped = buffer_slice.get_mapped_range();
        let output_bytes = strip_padding(
            &mapped,
            texture_size.width,
            texture_size.height,
            resources.padded_bytes_per_row,
        );
        drop(mapped);
        resources.readback_buffer.unmap();

        ImageFrame::new(input.descriptor.clone(), output_bytes).map_err(PipelineError::ImageData)
    }

    fn create_run_resources(&self, input: &ImageFrame, uniform_bytes: &[u8]) -> StillRunResources {
        let texture_size = input.descriptor.size;
        let input_texture = create_input_texture(self.device, self.queue, input);
        let working_texture = create_pipeline_texture(
            self.device,
            texture_size,
            INTERMEDIATE_TEXTURE_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            "casseted-still-image-working-signal",
        );
        let luma_texture = create_pipeline_texture(
            self.device,
            texture_size,
            INTERMEDIATE_TEXTURE_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            "casseted-still-image-luma-signal",
        );
        let chroma_texture = create_pipeline_texture(
            self.device,
            texture_size,
            INTERMEDIATE_TEXTURE_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            "casseted-still-image-chroma-signal",
        );
        let output_texture = create_pipeline_texture(
            self.device,
            texture_size,
            OUTPUT_TEXTURE_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            "casseted-still-image-output",
        );

        let uniform_buffer = create_uniform_buffer(self.device, uniform_bytes);
        self.queue.write_buffer(&uniform_buffer, 0, uniform_bytes);

        let padded_bytes_per_row = padded_bytes_per_row(texture_size.width);
        let output_buffer_size = padded_bytes_per_row as u64 * texture_size.height as u64;
        let readback_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("casseted-still-image-readback"),
            size: output_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        StillRunResources {
            input_texture,
            working_texture,
            luma_texture,
            chroma_texture,
            output_texture,
            uniform_buffer,
            readback_buffer,
            padded_bytes_per_row,
        }
    }
}

impl CompiledStillPassChain {
    fn new(
        device: &wgpu::Device,
        single_texture_layout: &wgpu::BindGroupLayout,
        dual_texture_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            conditioning: create_render_pipeline(
                device,
                ShaderId::StillInputConditioning,
                &[single_texture_layout],
                INTERMEDIATE_TEXTURE_FORMAT,
                "casseted-still-image-conditioning-pipeline",
            ),
            luma: create_render_pipeline(
                device,
                ShaderId::StillLumaDegradation,
                &[single_texture_layout],
                INTERMEDIATE_TEXTURE_FORMAT,
                "casseted-still-image-luma-pipeline",
            ),
            chroma: create_render_pipeline(
                device,
                ShaderId::StillChromaDegradation,
                &[single_texture_layout],
                INTERMEDIATE_TEXTURE_FORMAT,
                "casseted-still-image-chroma-pipeline",
            ),
            reconstruction: create_render_pipeline(
                device,
                ShaderId::StillReconstructionOutput,
                &[dual_texture_layout],
                OUTPUT_TEXTURE_FORMAT,
                "casseted-still-image-reconstruction-pipeline",
            ),
        }
    }
}

pub(crate) fn process_with_gpu(
    pipeline: &StillImagePipeline,
    context: &GpuContext,
    input: &ImageFrame,
) -> Result<ImageFrame, PipelineError> {
    StillPipelineRuntime::new(context).process(pipeline, input)
}

pub(crate) fn padded_bytes_per_row(width: u32) -> u32 {
    let unpadded = width * BYTES_PER_PIXEL_RGBA8;
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let remainder = unpadded % alignment;

    if remainder == 0 {
        unpadded
    } else {
        unpadded + (alignment - remainder)
    }
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

fn create_input_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    input: &ImageFrame,
) -> wgpu::Texture {
    let size = texture_extent(input.descriptor.size);
    let texture = create_pipeline_texture(
        device,
        input.descriptor.size,
        OUTPUT_TEXTURE_FORMAT,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        "casseted-still-image-input",
    );

    queue.write_texture(
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
    device: &wgpu::Device,
    size: FrameSize,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    label: &'static str,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
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

fn create_linear_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
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

fn create_uniform_buffer(device: &wgpu::Device, uniform_bytes: &[u8]) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("casseted-still-image-uniforms"),
        size: uniform_bytes.len() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_single_texture_bind_group_layout(
    device: &wgpu::Device,
    label: &'static str,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
    device: &wgpu::Device,
    label: &'static str,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    texture_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform_buffer: &wgpu::Buffer,
    label: &'static str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
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
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    first_texture_view: &wgpu::TextureView,
    second_texture_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform_buffer: &wgpu::Buffer,
    label: &'static str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
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
    device: &wgpu::Device,
    shader_id: ShaderId,
    bind_group_layouts: &[&wgpu::BindGroupLayout],
    target_format: wgpu::TextureFormat,
    label: &'static str,
) -> wgpu::RenderPipeline {
    let shader_source = shader_source(shader_id);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(shader_source.label),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(shader_source.source)),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts,
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
