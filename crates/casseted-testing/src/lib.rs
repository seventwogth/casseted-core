//! Shared helpers for workspace tests.

use casseted_types::{FrameDescriptor, FrameSize, ImageDataError, ImageFrame, PixelFormat};
use image::{ImageFormat, ImageReader, RgbaImage};
use std::fmt;
use std::path::Path;

pub fn assert_frame_size_eq(expected: FrameSize, actual: FrameSize) {
    assert_eq!(
        expected, actual,
        "expected frame size {}x{}, got {}x{}",
        expected.width, expected.height, actual.width, actual.height
    );
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageDiffStats {
    pub changed_bytes: usize,
    pub total_bytes: usize,
    pub mean_absolute_difference: f32,
    pub max_absolute_difference: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageDiffTolerance {
    pub max_changed_bytes: usize,
    pub max_mean_absolute_difference: f32,
    pub max_absolute_difference: u8,
}

#[derive(Debug)]
pub enum PngIoError {
    Io(std::io::Error),
    Image(image::ImageError),
    ImageData(ImageDataError),
    InvalidImageBuffer,
}

impl fmt::Display for PngIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Image(error) => write!(f, "{error}"),
            Self::ImageData(error) => write!(f, "{error}"),
            Self::InvalidImageBuffer => {
                f.write_str("failed to rebuild RGBA image buffer from frame data")
            }
        }
    }
}

impl std::error::Error for PngIoError {}

impl From<std::io::Error> for PngIoError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<image::ImageError> for PngIoError {
    fn from(value: image::ImageError) -> Self {
        Self::Image(value)
    }
}

impl From<ImageDataError> for PngIoError {
    fn from(value: ImageDataError) -> Self {
        Self::ImageData(value)
    }
}

pub fn gradient_rgba8_image(size: FrameSize) -> ImageFrame {
    let mut data = Vec::with_capacity(size.pixels() as usize * 4);
    for y in 0..size.height {
        for x in 0..size.width {
            data.extend_from_slice(&[
                (x.saturating_mul(17)) as u8,
                (y.saturating_mul(17)) as u8,
                ((x + y).saturating_mul(9)) as u8,
                255,
            ]);
        }
    }

    ImageFrame::rgba8(size, data).expect("generated gradient image must be valid")
}

pub fn reference_card_rgba8_image(size: FrameSize) -> ImageFrame {
    let mut data = Vec::with_capacity(size.pixels() as usize * 4);
    let width_max = size.width.saturating_sub(1).max(1);
    let height_max = size.height.saturating_sub(1).max(1);

    for y in 0..size.height {
        for x in 0..size.width {
            let fx = x as f32 / width_max as f32;
            let fy = y as f32 / height_max as f32;
            let tile_x = x / 8;
            let tile_y = y / 8;
            let checker = if (tile_x + tile_y) % 2 == 0 { 0.0 } else { 1.0 };
            let diagonal = ((x + y) % 32) as f32 / 31.0;

            let bar_color = match x.saturating_mul(6) / size.width.max(1) {
                0 => [0.12, 0.12, 0.12],
                1 => [0.86, 0.22, 0.20],
                2 => [0.24, 0.78, 0.24],
                3 => [0.22, 0.38, 0.86],
                4 => [0.88, 0.82, 0.24],
                _ => [0.92, 0.92, 0.92],
            };

            let (r, g, b) = if y < size.height / 3 {
                (
                    bar_color[0] * 0.75 + fx * 0.25,
                    bar_color[1] * 0.75 + fy * 0.20 + diagonal * 0.05,
                    bar_color[2] * 0.75 + (1.0 - fx) * 0.25,
                )
            } else if y < (size.height * 2) / 3 {
                (
                    fx * 0.70 + checker * 0.25 + 0.05,
                    fy * 0.45 + (1.0 - checker) * 0.35 + 0.10,
                    (1.0 - fx) * 0.35 + diagonal * 0.55 + 0.10,
                )
            } else {
                let edge = if x % 16 < 8 { 0.18 } else { 0.82 };
                (
                    edge * 0.55 + fx * 0.35 + checker * 0.10,
                    checker * 0.65 + fy * 0.30 + 0.05,
                    diagonal * 0.65 + (1.0 - fy) * 0.25 + 0.10,
                )
            };

            data.extend_from_slice(&[
                (r.clamp(0.0, 1.0) * 255.0).round() as u8,
                (g.clamp(0.0, 1.0) * 255.0).round() as u8,
                (b.clamp(0.0, 1.0) * 255.0).round() as u8,
                255,
            ]);
        }
    }

    ImageFrame::rgba8(size, data).expect("generated reference card must be valid")
}

pub fn load_png(path: &Path, frame_index: u64) -> Result<ImageFrame, PngIoError> {
    let image = ImageReader::open(path)?
        .with_guessed_format()?
        .decode()?
        .to_rgba8();
    let (width, height) = image.dimensions();

    ImageFrame::new(
        FrameDescriptor::new(
            FrameSize::new(width, height),
            PixelFormat::Rgba8Unorm,
            frame_index,
        ),
        image.into_raw(),
    )
    .map_err(PngIoError::ImageData)
}

pub fn save_png(path: &Path, image: &ImageFrame) -> Result<(), PngIoError> {
    let rgba = RgbaImage::from_raw(
        image.descriptor.size.width,
        image.descriptor.size.height,
        image.data.clone(),
    )
    .ok_or(PngIoError::InvalidImageBuffer)?;

    rgba.save_with_format(path, ImageFormat::Png)?;
    Ok(())
}

pub fn image_diff_stats(expected: &ImageFrame, actual: &ImageFrame) -> ImageDiffStats {
    assert_eq!(
        expected.descriptor.size, actual.descriptor.size,
        "image diff requires equal frame sizes"
    );
    assert_eq!(
        expected.descriptor.format, actual.descriptor.format,
        "image diff requires equal pixel formats"
    );

    let total_bytes = expected.data.len();
    let mut changed_bytes = 0;
    let mut total_absolute_difference = 0_u64;
    let mut max_absolute_difference = 0_u8;

    for (left, right) in expected.data.iter().zip(actual.data.iter()) {
        let delta = left.abs_diff(*right);
        if delta != 0 {
            changed_bytes += 1;
        }
        max_absolute_difference = max_absolute_difference.max(delta);
        total_absolute_difference += delta as u64;
    }

    ImageDiffStats {
        changed_bytes,
        total_bytes,
        mean_absolute_difference: if total_bytes == 0 {
            0.0
        } else {
            total_absolute_difference as f32 / total_bytes as f32
        },
        max_absolute_difference,
    }
}

pub fn assert_images_match_with_tolerance(
    expected: &ImageFrame,
    actual: &ImageFrame,
    tolerance: ImageDiffTolerance,
) {
    let stats = image_diff_stats(expected, actual);

    assert!(
        stats.changed_bytes <= tolerance.max_changed_bytes
            && stats.mean_absolute_difference <= tolerance.max_mean_absolute_difference
            && stats.max_absolute_difference <= tolerance.max_absolute_difference,
        "expected image diff to stay within tolerance, got changed_bytes={} mean_abs_diff={} max_abs_diff={} with tolerance changed_bytes<={} mean_abs_diff<={} max_abs_diff<={}",
        stats.changed_bytes,
        stats.mean_absolute_difference,
        stats.max_absolute_difference,
        tolerance.max_changed_bytes,
        tolerance.max_mean_absolute_difference,
        tolerance.max_absolute_difference,
    );
}

pub fn assert_images_not_identical(expected: &ImageFrame, actual: &ImageFrame) {
    let stats = image_diff_stats(expected, actual);

    assert!(
        stats.changed_bytes > 0,
        "expected processed image to differ from input, but all {} bytes matched",
        stats.total_bytes
    );
}

pub fn snapshot_frame_descriptor(frame: &FrameDescriptor) -> String {
    format!(
        "frame={}x{} format={:?} index={}",
        frame.size.width, frame.size.height, frame.format, frame.frame_index
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ImageDiffTolerance, assert_images_match_with_tolerance, gradient_rgba8_image,
        image_diff_stats, reference_card_rgba8_image, snapshot_frame_descriptor,
    };
    use casseted_types::{FrameDescriptor, FrameSize, ImageFrame};

    #[test]
    fn snapshot_contains_dimensions() {
        let snapshot = snapshot_frame_descriptor(&FrameDescriptor::default());

        assert!(snapshot.contains("640x480"));
    }

    #[test]
    fn generated_gradient_matches_requested_size() {
        let image = gradient_rgba8_image(FrameSize::new(4, 3));

        assert_eq!(image.descriptor.size, FrameSize::new(4, 3));
        assert_eq!(image.data.len(), 4 * 3 * 4);
    }

    #[test]
    fn image_diff_reports_changed_bytes() {
        let left = gradient_rgba8_image(FrameSize::new(2, 2));
        let mut right = left.clone();
        right.data[0] = right.data[0].saturating_add(10);
        let right =
            ImageFrame::new(right.descriptor.clone(), right.data).expect("image data stays valid");

        let stats = image_diff_stats(&left, &right);

        assert_eq!(stats.changed_bytes, 1);
        assert!(stats.mean_absolute_difference > 0.0);
        assert_eq!(stats.max_absolute_difference, 10);
    }

    #[test]
    fn reference_card_matches_requested_size() {
        let image = reference_card_rgba8_image(FrameSize::new(8, 6));

        assert_eq!(image.descriptor.size, FrameSize::new(8, 6));
        assert_eq!(image.data.len(), 8 * 6 * 4);
    }

    #[test]
    fn tolerance_assertion_accepts_small_diffs() {
        let left = gradient_rgba8_image(FrameSize::new(2, 2));
        let mut right = left.clone();
        right.data[0] = right.data[0].saturating_add(1);
        let right =
            ImageFrame::new(right.descriptor.clone(), right.data).expect("image data stays valid");

        assert_images_match_with_tolerance(
            &left,
            &right,
            ImageDiffTolerance {
                max_changed_bytes: 1,
                max_mean_absolute_difference: 1.0,
                max_absolute_difference: 1,
            },
        );
    }
}
