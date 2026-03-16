//! Shared domain types used across the workspace.

use std::fmt;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageDataError {
    UnexpectedByteLen { expected: usize, actual: usize },
}

impl fmt::Display for ImageDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedByteLen { expected, actual } => {
                write!(
                    f,
                    "image byte length does not match descriptor: expected {expected} bytes, got {actual}"
                )
            }
        }
    }
}

impl std::error::Error for ImageDataError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageFrame {
    pub descriptor: FrameDescriptor,
    pub data: Vec<u8>,
}

impl ImageFrame {
    pub fn new(descriptor: FrameDescriptor, data: Vec<u8>) -> Result<Self, ImageDataError> {
        let expected = descriptor.byte_len() as usize;
        let actual = data.len();

        if expected != actual {
            return Err(ImageDataError::UnexpectedByteLen { expected, actual });
        }

        Ok(Self { descriptor, data })
    }

    pub fn rgba8(size: FrameSize, data: Vec<u8>) -> Result<Self, ImageDataError> {
        Self::new(FrameDescriptor::new(size, PixelFormat::Rgba8Unorm, 0), data)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameDescriptor, FrameSize, ImageDataError, ImageFrame, PixelFormat};

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

    #[test]
    fn image_frame_validates_byte_length() {
        let result = ImageFrame::rgba8(FrameSize::new(2, 2), vec![0; 12]);

        assert_eq!(
            result,
            Err(ImageDataError::UnexpectedByteLen {
                expected: 16,
                actual: 12,
            })
        );
    }

    #[test]
    fn solid_rgba8_produces_expected_number_of_bytes() {
        let image = ImageFrame::rgba8(
            FrameSize::new(3, 2),
            [
                [10, 20, 30, 255],
                [10, 20, 30, 255],
                [10, 20, 30, 255],
                [10, 20, 30, 255],
                [10, 20, 30, 255],
                [10, 20, 30, 255],
            ]
            .into_iter()
            .flatten()
            .collect(),
        )
        .expect("raw RGBA bytes should build a valid image");

        assert_eq!(image.data.len(), 3 * 2 * 4);
        assert_eq!(&image.data[0..4], &[10, 20, 30, 255]);
    }
}
