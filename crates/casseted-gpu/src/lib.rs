//! Minimal `wgpu` foundation used by the core workspace.

use casseted_shaderlib::ShaderSource;
use casseted_types::FrameSize;
use std::fmt;

#[derive(Debug, Clone)]
pub struct GpuRequirements {
    pub label: &'static str,
    pub required_features: wgpu::Features,
    pub required_limits: wgpu::Limits,
}

impl Default for GpuRequirements {
    fn default() -> Self {
        Self {
            label: "casseted-core",
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GpuContextDescriptor {
    pub label: &'static str,
    pub backends: wgpu::Backends,
    pub power_preference: wgpu::PowerPreference,
    pub force_fallback_adapter: bool,
    pub required_features: wgpu::Features,
    pub required_limits: wgpu::Limits,
}

impl GpuContextDescriptor {
    pub fn adapter_options(&self) -> wgpu::RequestAdapterOptions<'static, 'static> {
        wgpu::RequestAdapterOptions {
            power_preference: self.power_preference,
            force_fallback_adapter: self.force_fallback_adapter,
            compatible_surface: None,
        }
    }

    pub fn device_descriptor(&self) -> wgpu::DeviceDescriptor<'static> {
        wgpu::DeviceDescriptor {
            label: Some(self.label),
            required_features: self.required_features,
            required_limits: self.required_limits.clone(),
        }
    }
}

impl Default for GpuContextDescriptor {
    fn default() -> Self {
        Self {
            label: "casseted-core",
            backends: wgpu::Backends::all(),
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
        }
    }
}

impl From<GpuRequirements> for GpuContextDescriptor {
    fn from(value: GpuRequirements) -> Self {
        Self {
            label: value.label,
            required_features: value.required_features,
            required_limits: value.required_limits,
            ..Self::default()
        }
    }
}

#[derive(Debug)]
pub enum GpuInitError {
    AdapterNotFound,
    RequestDevice(wgpu::RequestDeviceError),
}

impl fmt::Display for GpuInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdapterNotFound => f.write_str("no compatible GPU adapter was found"),
            Self::RequestDevice(error) => write!(f, "failed to request device: {error}"),
        }
    }
}

impl std::error::Error for GpuInitError {}

#[derive(Debug)]
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
}

impl GpuContext {
    pub async fn request(descriptor: &GpuContextDescriptor) -> Result<Self, GpuInitError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: descriptor.backends,
            ..wgpu::InstanceDescriptor::default()
        });

        let adapter = instance
            .request_adapter(&descriptor.adapter_options())
            .await
            .ok_or(GpuInitError::AdapterNotFound)?;
        let adapter_info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&descriptor.device_descriptor(), None)
            .await
            .map_err(GpuInitError::RequestDevice)?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            adapter_info,
        })
    }

    pub fn create_shader_module(&self, shader: ShaderSource) -> wgpu::ShaderModule {
        self.device
            .create_shader_module(shader_module_descriptor(Some(shader.label), shader.source))
    }

    pub fn create_shader_module_from_wgsl(
        &self,
        label: Option<&str>,
        source: &str,
    ) -> wgpu::ShaderModule {
        self.device
            .create_shader_module(shader_module_descriptor(label, source))
    }
}

pub fn shader_module_descriptor<'a>(
    label: Option<&'a str>,
    source: &'a str,
) -> wgpu::ShaderModuleDescriptor<'a> {
    wgpu::ShaderModuleDescriptor {
        label,
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(source)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureContext {
    pub size: FrameSize,
    pub format: wgpu::TextureFormat,
}

impl TextureContext {
    pub const fn new(size: FrameSize, format: wgpu::TextureFormat) -> Self {
        Self { size, format }
    }

    pub fn extent(self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.size.width,
            height: self.size.height,
            depth_or_array_layers: 1,
        }
    }

    pub fn descriptor(self) -> wgpu::TextureDescriptor<'static> {
        wgpu::TextureDescriptor {
            label: Some("casseted-frame-texture"),
            size: self.extent(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GpuContextDescriptor, TextureContext, shader_module_descriptor};
    use casseted_shaderlib::{ShaderId, shader_source};
    use casseted_types::FrameSize;

    #[test]
    fn texture_context_maps_to_2d_extent() {
        let extent =
            TextureContext::new(FrameSize::new(720, 576), wgpu::TextureFormat::Rgba8Unorm).extent();

        assert_eq!(extent.width, 720);
        assert_eq!(extent.height, 576);
        assert_eq!(extent.depth_or_array_layers, 1);
    }

    #[test]
    fn gpu_context_descriptor_defaults_are_headless_friendly() {
        let descriptor = GpuContextDescriptor::default();

        assert_eq!(descriptor.label, "casseted-core");
        assert_eq!(descriptor.backends, wgpu::Backends::all());
        assert_eq!(descriptor.required_features, wgpu::Features::empty());
    }

    #[test]
    fn shader_module_descriptor_wraps_wgsl_source() {
        let shader = shader_source(ShaderId::StillAnalog);
        let descriptor = shader_module_descriptor(Some(shader.label), shader.source);

        assert_eq!(descriptor.label, Some("still_analog"));
        match descriptor.source {
            wgpu::ShaderSource::Wgsl(source) => {
                assert!(source.contains("@vertex"));
                assert!(source.contains("fs_main"));
            }
            _ => panic!("expected WGSL shader source"),
        }
    }
}
