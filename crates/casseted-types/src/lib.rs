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

    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
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

impl PixelFormat {
    pub const fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::Rgba8Unorm => 4,
            Self::Rgba16Float => 8,
        }
    }
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

    pub const fn byte_len(&self) -> u64 {
        self.size.pixels() * self.format.bytes_per_pixel() as u64
    }
}

impl Default for FrameDescriptor {
    fn default() -> Self {
        Self::new(FrameSize::new(640, 480), PixelFormat::Rgba8Unorm, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameDescriptor, FrameSize, PixelFormat};

    #[test]
    fn default_frame_descriptor_is_vga_like() {
        let frame = FrameDescriptor::default();

        assert_eq!(frame.size, FrameSize::new(640, 480));
    }

    #[test]
    fn empty_size_is_reported() {
        assert!(FrameSize::new(0, 480).is_empty());
        assert!(!FrameSize::new(640, 480).is_empty());
    }

    #[test]
    fn frame_descriptor_byte_len_matches_format() {
        let frame = FrameDescriptor::new(FrameSize::new(320, 240), PixelFormat::Rgba16Float, 3);

        assert_eq!(frame.byte_len(), 320 * 240 * 8);
    }
}
