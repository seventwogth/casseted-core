//! Formal VHS/analog signal model v1.
//!
//! This model is intentionally implementation-agnostic. It describes the signal
//! stages and parameter groups that later CPU/GPU code should consume, without
//! prescribing a pass graph or shader layout.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoStandard {
    NtscM,
    Pal,
}

impl VideoStandard {
    pub const fn frame_rate_hz(self) -> f32 {
        match self {
            Self::NtscM => 29.97,
            Self::Pal => 25.0,
        }
    }

    pub const fn field_rate_hz(self) -> f32 {
        match self {
            Self::NtscM => 59.94,
            Self::Pal => 50.0,
        }
    }

    pub const fn line_period_us(self) -> f32 {
        match self {
            Self::NtscM => 63.556,
            Self::Pal => 64.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoMatrix {
    Bt601,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputTransfer {
    Srgb,
    Bt601,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalSampling {
    ProgressiveFrame,
    InterlacedFields,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputTransfer {
    Srgb,
    Bt1886Like,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VhsSignalStage {
    /// Normalize the input frame into the working assumptions for still-image v1.
    InputDecode,
    /// Convert gamma-coded RGB into a luma/chroma representation for later loss modeling.
    RgbToLumaChroma,
    /// Apply luma-oriented bandwidth loss and detail shaping.
    LumaRecordPath,
    /// Apply chroma-specific degradation, delay, and phase errors.
    ChromaRecordPath,
    /// Apply spatial displacement that approximates tape transport instability.
    TransportInstability,
    /// Add stochastic corruption such as grain-like noise and dropouts.
    NoiseAndDropouts,
    /// Reconstruct a display-space image from the degraded signal representation.
    DecodeOutput,
}

impl VhsSignalStage {
    pub const fn label(self) -> &'static str {
        match self {
            Self::InputDecode => "input_decode",
            Self::RgbToLumaChroma => "rgb_to_luma_chroma",
            Self::LumaRecordPath => "luma_record_path",
            Self::ChromaRecordPath => "chroma_record_path",
            Self::TransportInstability => "transport_instability",
            Self::NoiseAndDropouts => "noise_and_dropouts",
            Self::DecodeOutput => "decode_output",
        }
    }
}

pub const VHS_SIGNAL_FLOW_V1: [VhsSignalStage; 7] = [
    VhsSignalStage::InputDecode,
    VhsSignalStage::RgbToLumaChroma,
    VhsSignalStage::LumaRecordPath,
    VhsSignalStage::ChromaRecordPath,
    VhsSignalStage::TransportInstability,
    VhsSignalStage::NoiseAndDropouts,
    VhsSignalStage::DecodeOutput,
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VhsModel {
    pub standard: VideoStandard,
    pub input: VhsInputSettings,
    pub luma: VhsLumaSettings,
    pub chroma: VhsChromaSettings,
    pub transport: VhsTransportSettings,
    pub noise: VhsNoiseSettings,
    pub decode: VhsDecodeSettings,
}

impl VhsModel {
    pub const fn for_standard(standard: VideoStandard) -> Self {
        match standard {
            VideoStandard::NtscM => Self::ntsc_v1(),
            VideoStandard::Pal => Self::pal_v1(),
        }
    }

    pub const fn ntsc_v1() -> Self {
        Self {
            standard: VideoStandard::NtscM,
            input: VhsInputSettings {
                matrix: VideoMatrix::Bt601,
                transfer: InputTransfer::Srgb,
                temporal_sampling: TemporalSampling::ProgressiveFrame,
            },
            luma: VhsLumaSettings {
                bandwidth_mhz: 3.0,
                preemphasis_db: 3.0,
            },
            chroma: VhsChromaSettings {
                bandwidth_khz: 300.0,
                saturation_gain: 1.0,
                delay_us: 0.12,
                phase_error_deg: 0.0,
            },
            transport: VhsTransportSettings {
                line_jitter_us: 0.10,
                vertical_wander_lines: 0.15,
                head_switching_band_lines: 6,
                head_switching_offset_us: 1.5,
            },
            noise: VhsNoiseSettings {
                luma_sigma: 0.015,
                chroma_sigma: 0.020,
                chroma_phase_noise_deg: 1.5,
                dropout_probability_per_line: 0.002,
                dropout_mean_span_us: 1.5,
            },
            decode: VhsDecodeSettings {
                chroma_vertical_blend: 0.25,
                luma_chroma_crosstalk: 0.05,
                output_transfer: OutputTransfer::Srgb,
            },
        }
    }

    pub const fn pal_v1() -> Self {
        Self {
            standard: VideoStandard::Pal,
            input: VhsInputSettings {
                matrix: VideoMatrix::Bt601,
                transfer: InputTransfer::Srgb,
                temporal_sampling: TemporalSampling::ProgressiveFrame,
            },
            luma: VhsLumaSettings {
                bandwidth_mhz: 3.0,
                preemphasis_db: 3.0,
            },
            chroma: VhsChromaSettings {
                bandwidth_khz: 400.0,
                saturation_gain: 1.0,
                delay_us: 0.10,
                phase_error_deg: 0.0,
            },
            transport: VhsTransportSettings {
                line_jitter_us: 0.10,
                vertical_wander_lines: 0.15,
                head_switching_band_lines: 8,
                head_switching_offset_us: 1.2,
            },
            noise: VhsNoiseSettings {
                luma_sigma: 0.015,
                chroma_sigma: 0.018,
                chroma_phase_noise_deg: 1.0,
                dropout_probability_per_line: 0.002,
                dropout_mean_span_us: 1.5,
            },
            decode: VhsDecodeSettings {
                chroma_vertical_blend: 0.30,
                luma_chroma_crosstalk: 0.04,
                output_transfer: OutputTransfer::Srgb,
            },
        }
    }

    pub const fn signal_flow(&self) -> &'static [VhsSignalStage] {
        &VHS_SIGNAL_FLOW_V1
    }
}

impl Default for VhsModel {
    fn default() -> Self {
        Self::for_standard(VideoStandard::NtscM)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VhsInputSettings {
    /// Working luma/chroma matrix used after input normalization.
    pub matrix: VideoMatrix,
    /// Transfer curve assumed for the incoming RGB image.
    pub transfer: InputTransfer,
    /// Temporal interpretation of the source frame semantics.
    pub temporal_sampling: TemporalSampling,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VhsLumaSettings {
    /// Approximate luma cutoff in MHz after VHS record/playback losses.
    pub bandwidth_mhz: f32,
    /// Broadband pre-emphasis/de-emphasis amount used to restore some edge detail.
    pub preemphasis_db: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VhsChromaSettings {
    /// Approximate chroma bandwidth in kHz in the reconstructed playback signal.
    pub bandwidth_khz: f32,
    /// Saturation multiplier applied in the chroma path.
    pub saturation_gain: f32,
    /// Relative chroma delay against luma, measured in microseconds.
    pub delay_us: f32,
    /// Additional phase error applied in degrees.
    pub phase_error_deg: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VhsTransportSettings {
    /// Per-line horizontal time-base instability in microseconds.
    pub line_jitter_us: f32,
    /// Slow vertical displacement measured in scan lines.
    pub vertical_wander_lines: f32,
    /// Bottom-band line count affected by head switching.
    pub head_switching_band_lines: u32,
    /// Horizontal displacement of the head-switching region in microseconds.
    pub head_switching_offset_us: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VhsNoiseSettings {
    /// Standard deviation of additive luma noise in normalized signal units.
    pub luma_sigma: f32,
    /// Standard deviation of additive chroma noise in normalized signal units.
    pub chroma_sigma: f32,
    /// Standard deviation of chroma phase perturbation in degrees.
    pub chroma_phase_noise_deg: f32,
    /// Probability that a given scan line contains a dropout segment.
    pub dropout_probability_per_line: f32,
    /// Mean horizontal dropout span expressed in microseconds.
    pub dropout_mean_span_us: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VhsDecodeSettings {
    /// Vertical blend factor used when reconstructing low-bandwidth chroma.
    pub chroma_vertical_blend: f32,
    /// Amount of residual Y/C leakage kept in the output.
    pub luma_chroma_crosstalk: f32,
    pub output_transfer: OutputTransfer,
}

#[cfg(test)]
mod tests {
    use super::{
        InputTransfer, TemporalSampling, VHS_SIGNAL_FLOW_V1, VhsModel, VhsSignalStage, VideoMatrix,
        VideoStandard,
    };

    #[test]
    fn default_vhs_model_is_ntsc_v1() {
        let model = VhsModel::default();

        assert_eq!(model.standard, VideoStandard::NtscM);
        assert_eq!(model.input.matrix, VideoMatrix::Bt601);
        assert_eq!(model.input.transfer, InputTransfer::Srgb);
        assert_eq!(
            model.input.temporal_sampling,
            TemporalSampling::ProgressiveFrame
        );
    }

    #[test]
    fn pal_preset_adjusts_standard_specific_defaults() {
        let model = VhsModel::pal_v1();

        assert_eq!(model.standard, VideoStandard::Pal);
        assert_eq!(model.chroma.bandwidth_khz, 400.0);
        assert_eq!(model.transport.head_switching_band_lines, 8);
    }

    #[test]
    fn for_standard_returns_matching_preset() {
        let ntsc = VhsModel::for_standard(VideoStandard::NtscM);
        let pal = VhsModel::for_standard(VideoStandard::Pal);

        assert_eq!(ntsc, VhsModel::ntsc_v1());
        assert_eq!(pal, VhsModel::pal_v1());
    }

    #[test]
    fn signal_flow_order_is_stable() {
        assert_eq!(
            VHS_SIGNAL_FLOW_V1,
            [
                VhsSignalStage::InputDecode,
                VhsSignalStage::RgbToLumaChroma,
                VhsSignalStage::LumaRecordPath,
                VhsSignalStage::ChromaRecordPath,
                VhsSignalStage::TransportInstability,
                VhsSignalStage::NoiseAndDropouts,
                VhsSignalStage::DecodeOutput,
            ]
        );
    }

    #[test]
    fn video_standard_timings_are_exposed_for_future_mappings() {
        assert_eq!(VideoStandard::NtscM.field_rate_hz(), 59.94);
        assert_eq!(VideoStandard::Pal.line_period_us(), 64.0);
    }
}
