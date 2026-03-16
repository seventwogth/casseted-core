//! Shared domain types used across the workspace.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSize {
    pub width: u32,
    pub height: u32,
}

impl FrameSize {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub const fn pixels(self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba8Unorm,
    Rgba16Float,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameDescriptor {
    pub size: FrameSize,
    pub format: PixelFormat,
    pub frame_index: u64,
}

impl FrameDescriptor {
    pub const fn new(size: FrameSize, format: PixelFormat, frame_index: u64) -> Self {
        Self {
            size,
            format,
            frame_index,
        }
    }
}

impl Default for FrameDescriptor {
    fn default() -> Self {
        Self::new(FrameSize::new(640, 480), PixelFormat::Rgba8Unorm, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameDescriptor, FrameSize};

    #[test]
    fn default_frame_descriptor_is_vga_like() {
        let frame = FrameDescriptor::default();

        assert_eq!(frame.size, FrameSize::new(640, 480));
    }
}
