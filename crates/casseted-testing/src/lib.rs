//! Shared helpers for workspace tests.

use casseted_types::{FrameDescriptor, FrameSize};

pub fn assert_frame_size_eq(expected: FrameSize, actual: FrameSize) {
    assert_eq!(
        expected, actual,
        "expected frame size {}x{}, got {}x{}",
        expected.width, expected.height, actual.width, actual.height
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
    use super::snapshot_frame_descriptor;
    use casseted_types::FrameDescriptor;

    #[test]
    fn snapshot_contains_dimensions() {
        let snapshot = snapshot_frame_descriptor(&FrameDescriptor::default());

        assert!(snapshot.contains("640x480"));
    }
}
