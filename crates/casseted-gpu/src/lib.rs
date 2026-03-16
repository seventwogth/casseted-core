//! Minimal `wgpu` foundation used by the core workspace.

use casseted_shaderlib::ShaderSource;
use std::fmt;

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

#[cfg(test)]
mod tests {
    use super::{GpuContextDescriptor, shader_module_descriptor};
    use casseted_shaderlib::{ShaderId, shader_source};

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
