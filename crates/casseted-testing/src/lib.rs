//! Shared helpers for workspace tests.

use casseted_types::{FrameDescriptor, FrameSize, ImageFrame};

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

    for (left, right) in expected.data.iter().zip(actual.data.iter()) {
        let delta = left.abs_diff(*right) as u64;
        if delta != 0 {
            changed_bytes += 1;
        }
        total_absolute_difference += delta;
    }

    ImageDiffStats {
        changed_bytes,
        total_bytes,
        mean_absolute_difference: if total_bytes == 0 {
            0.0
        } else {
            total_absolute_difference as f32 / total_bytes as f32
        },
    }
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
    use super::{gradient_rgba8_image, image_diff_stats, snapshot_frame_descriptor};
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
    }
}
