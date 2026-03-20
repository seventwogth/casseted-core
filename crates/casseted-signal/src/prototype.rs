//! Compact prototype settings used by the current still-image shader pipeline.
//!
//! The grouping stays intentionally small and preview-oriented, but it now maps
//! more explicitly onto the implementation stages used by the fused still pass:
//! - `tone` + `tracking`: input conditioning / tone shaping
//! - `luma`: luma degradation
//! - `chroma`: chroma degradation
//! - `noise`: reconstruction / output noise contamination
//!
//! Important scope note:
//! these are preview-facing authoring controls, not the canonical formal model.
//! The pipeline may softly normalize extreme manual values before they are packed
//! into the still-pass uniform block so preview overrides stay closer to the
//! intended analog-like visual regime.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignalSettings {
    pub tone: ToneSettings,
    pub luma: LumaSettings,
    pub chroma: ChromaSettings,
    pub noise: NoiseSettings,
    pub tracking: TrackingSettings,
}

impl SignalSettings {
    pub const fn neutral() -> Self {
        Self {
            tone: ToneSettings::neutral(),
            luma: LumaSettings::neutral(),
            chroma: ChromaSettings::neutral(),
            noise: NoiseSettings::neutral(),
            tracking: TrackingSettings::neutral(),
        }
    }

    pub fn is_neutral(&self) -> bool {
        self.tone.is_neutral()
            && self.luma.is_neutral()
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
pub struct ToneSettings {
    /// Normalized luma level where soft-knee highlight compression begins.
    /// `1.0` disables the tone stage in the preview path.
    pub highlight_soft_knee: f32,
    /// Compression strength applied above the soft-knee threshold.
    /// `0.0` disables highlight compression.
    pub highlight_compression: f32,
}

impl ToneSettings {
    pub const fn neutral() -> Self {
        Self {
            highlight_soft_knee: 1.0,
            highlight_compression: 0.0,
        }
    }

    pub fn is_neutral(&self) -> bool {
        self.highlight_soft_knee == 1.0 && self.highlight_compression == 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LumaSettings {
    /// Legacy preview control name for the shared luma bandwidth-loss proxy in
    /// reference-width pixels. The current luma pass expands this into a
    /// horizontal low-pass span plus multi-band detail attenuation rather than
    /// treating it as a plain post-process blur radius.
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
    /// Horizontal chroma delay proxy in reference-width pixels.
    pub offset_px: f32,
    /// Legacy preview control name for the shared chroma bandwidth-loss proxy.
    /// The current chroma pass derives both horizontal low-pass span and
    /// effective coarse chroma resolution from this value.
    pub bleed_px: f32,
    /// Post-blur chroma gain. `1.0` keeps chroma magnitude neutral.
    pub saturation: f32,
}

impl ChromaSettings {
    pub const fn neutral() -> Self {
        Self {
            offset_px: 0.0,
            bleed_px: 0.0,
            saturation: 1.0,
        }
    }

    pub fn is_neutral(&self) -> bool {
        self.offset_px == 0.0 && self.bleed_px == 0.0 && self.saturation == 1.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoiseSettings {
    /// Preview luma-contamination amplitude after formal sigma projection.
    /// The reconstruction pass reshapes it into brightness-dependent,
    /// partially line-correlated luma noise.
    pub luma_amount: f32,
    /// Preview chroma-contamination amplitude after formal sigma projection.
    /// The reconstruction pass reshapes it into softer, lower-bandwidth
    /// chroma contamination.
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
    /// Horizontal line-jitter amplitude in reference-width pixels.
    pub line_jitter_px: f32,
    /// Still-frame vertical offset snapshot expressed in scan lines.
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
    use super::{ChromaSettings, NoiseSettings, SignalSettings, ToneSettings, TrackingSettings};

    #[test]
    fn default_signal_settings_are_neutral() {
        assert!(SignalSettings::default().is_neutral());
    }

    #[test]
    fn non_zero_chroma_settings_are_not_neutral() {
        let settings = SignalSettings {
            tone: ToneSettings {
                highlight_soft_knee: 0.72,
                highlight_compression: 0.35,
            },
            chroma: ChromaSettings {
                offset_px: 1.5,
                bleed_px: 2.0,
                saturation: 0.95,
            },
            ..SignalSettings::default()
        };

        assert!(!settings.is_neutral());
    }

    #[test]
    fn tone_settings_are_part_of_neutrality_check() {
        let settings = SignalSettings {
            tone: ToneSettings {
                highlight_soft_knee: 0.75,
                highlight_compression: 0.30,
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
