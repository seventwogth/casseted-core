//! Small signal-domain building blocks for analog-style transforms.

use casseted_types::FrameDescriptor;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignalProfile {
    pub luma_lowpass_hz: f32,
    pub chroma_lowpass_hz: f32,
    pub head_switching_noise: f32,
}

impl SignalProfile {
    pub const VHS_NTSC: Self = Self {
        luma_lowpass_hz: 3_000_000.0,
        chroma_lowpass_hz: 400_000.0,
        head_switching_noise: 0.18,
    };

    pub fn estimated_bandwidth_ratio(self) -> f32 {
        self.chroma_lowpass_hz / self.luma_lowpass_hz
    }
}

impl Default for SignalProfile {
    fn default() -> Self {
        Self::VHS_NTSC
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SignalPlan {
    pub input: FrameDescriptor,
    pub output: FrameDescriptor,
    pub profile: SignalProfile,
}

impl SignalPlan {
    pub fn preview(input: FrameDescriptor, profile: SignalProfile) -> Self {
        Self {
            output: input.clone(),
            input,
            profile,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SignalProfile;

    #[test]
    fn ntsc_profile_reduces_chroma_bandwidth() {
        let ratio = SignalProfile::VHS_NTSC.estimated_bandwidth_ratio();

        assert!(ratio < 1.0);
    }
}
