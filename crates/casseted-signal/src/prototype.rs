//! Compact prototype settings used by the current still-image shader pipeline.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignalSettings {
    pub luma: LumaSettings,
    pub chroma: ChromaSettings,
    pub noise: NoiseSettings,
    pub tracking: TrackingSettings,
}

impl SignalSettings {
    pub const fn neutral() -> Self {
        Self {
            luma: LumaSettings::neutral(),
            chroma: ChromaSettings::neutral(),
            noise: NoiseSettings::neutral(),
            tracking: TrackingSettings::neutral(),
        }
    }

    pub fn is_neutral(&self) -> bool {
        self.luma.is_neutral()
            && self.chroma.is_neutral()
            && self.noise.is_neutral()
            && self.tracking.is_neutral()
    }
}

impl Default for SignalSettings {
    fn default() -> Self {
        Self::neutral()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LumaSettings {
    pub blur_px: f32,
}

impl LumaSettings {
    pub const fn neutral() -> Self {
        Self { blur_px: 0.0 }
    }

    pub fn is_neutral(&self) -> bool {
        self.blur_px == 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromaSettings {
    pub offset_px: f32,
    pub bleed_px: f32,
}

impl ChromaSettings {
    pub const fn neutral() -> Self {
        Self {
            offset_px: 0.0,
            bleed_px: 0.0,
        }
    }

    pub fn is_neutral(&self) -> bool {
        self.offset_px == 0.0 && self.bleed_px == 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoiseSettings {
    pub luma_amount: f32,
    pub chroma_amount: f32,
}

impl NoiseSettings {
    pub const fn neutral() -> Self {
        Self {
            luma_amount: 0.0,
            chroma_amount: 0.0,
        }
    }

    pub fn is_neutral(&self) -> bool {
        self.luma_amount == 0.0 && self.chroma_amount == 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackingSettings {
    pub line_jitter_px: f32,
    pub vertical_offset_lines: f32,
}

impl TrackingSettings {
    pub const fn neutral() -> Self {
        Self {
            line_jitter_px: 0.0,
            vertical_offset_lines: 0.0,
        }
    }

    pub fn is_neutral(&self) -> bool {
        self.line_jitter_px == 0.0 && self.vertical_offset_lines == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{ChromaSettings, NoiseSettings, SignalSettings, TrackingSettings};

    #[test]
    fn default_signal_settings_are_neutral() {
        assert!(SignalSettings::default().is_neutral());
    }

    #[test]
    fn non_zero_chroma_settings_are_not_neutral() {
        let settings = SignalSettings {
            chroma: ChromaSettings {
                offset_px: 1.5,
                bleed_px: 2.0,
            },
            ..SignalSettings::default()
        };

        assert!(!settings.is_neutral());
    }

    #[test]
    fn non_zero_noise_or_tracking_settings_are_not_neutral() {
        let noisy = SignalSettings {
            noise: NoiseSettings {
                luma_amount: 0.02,
                chroma_amount: 0.04,
            },
            ..SignalSettings::default()
        };
        let unstable = SignalSettings {
            tracking: TrackingSettings {
                line_jitter_px: 0.75,
                vertical_offset_lines: 1.0,
            },
            ..SignalSettings::default()
        };

        assert!(!noisy.is_neutral());
        assert!(!unstable.is_neutral());
    }

    #[test]
    fn neutral_settings_remain_copyable_and_comparable() {
        let left = SignalSettings::default();
        let right = left;

        assert_eq!(left, right);
    }
}
