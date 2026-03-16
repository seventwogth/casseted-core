//! Minimal `wgpu` configuration helpers used by the core workspace.

use casseted_types::FrameSize;

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
    use super::TextureContext;
    use casseted_types::FrameSize;

    #[test]
    fn texture_context_maps_to_2d_extent() {
        let extent =
            TextureContext::new(FrameSize::new(720, 576), wgpu::TextureFormat::Rgba8Unorm).extent();

        assert_eq!(extent.width, 720);
        assert_eq!(extent.height, 576);
        assert_eq!(extent.depth_or_array_layers, 1);
    }
}
