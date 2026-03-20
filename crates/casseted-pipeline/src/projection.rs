use casseted_signal::{
    ChromaSettings, LumaSettings, NoiseSettings, SignalSettings, ToneSettings, TrackingSettings,
    VhsModel,
};

pub(crate) const REFERENCE_WIDTH_PX: f32 = 720.0;
const BT601_SAMPLES_PER_US: f32 = 13.5;
const STILL_JITTER_ATTENUATION: f32 = 0.22;
const STILL_CHROMA_DELAY_ATTENUATION: f32 = 0.4;
const REFERENCE_LUMA_BANDWIDTH_MHZ: f32 = 4.2;
const REFERENCE_CHROMA_BANDWIDTH_KHZ: f32 = 1000.0;
const PREVIEW_LUMA_BLUR_RECOMMENDED_CAP: f32 = 3.25;
const PREVIEW_LUMA_BLUR_HARD_CAP: f32 = 4.75;
const PREVIEW_CHROMA_OFFSET_RECOMMENDED_CAP: f32 = 0.35;
const PREVIEW_CHROMA_OFFSET_HARD_CAP: f32 = 0.60;
const PREVIEW_CHROMA_BLEED_RECOMMENDED_CAP: f32 = 3.0;
const PREVIEW_CHROMA_BLEED_HARD_CAP: f32 = 4.25;
const PREVIEW_CHROMA_BLEED_OFFSET_RATIO: f32 = 2.5;
const PREVIEW_LUMA_NOISE_RECOMMENDED_CAP: f32 = 0.02;
const PREVIEW_LUMA_NOISE_HARD_CAP: f32 = 0.04;
const PREVIEW_CHROMA_NOISE_RECOMMENDED_CAP: f32 = 0.012;
const PREVIEW_CHROMA_NOISE_HARD_CAP: f32 = 0.025;
const PREVIEW_LINE_JITTER_RECOMMENDED_CAP: f32 = 0.35;
const PREVIEW_LINE_JITTER_HARD_CAP: f32 = 0.55;
const PREVIEW_VERTICAL_OFFSET_RECOMMENDED_CAP: f32 = 0.35;
const PREVIEW_VERTICAL_OFFSET_HARD_CAP: f32 = 0.75;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SignalOverrides {
    pub tone: ToneOverrides,
    pub luma: LumaOverrides,
    pub chroma: ChromaOverrides,
    pub noise: NoiseOverrides,
    pub tracking: TrackingOverrides,
}

impl SignalOverrides {
    pub fn is_empty(&self) -> bool {
        self.tone.is_empty()
            && self.luma.is_empty()
            && self.chroma.is_empty()
            && self.noise.is_empty()
            && self.tracking.is_empty()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ToneOverrides {
    pub highlight_soft_knee: Option<f32>,
    pub highlight_compression: Option<f32>,
}

impl ToneOverrides {
    pub fn is_empty(&self) -> bool {
        self.highlight_soft_knee.is_none() && self.highlight_compression.is_none()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LumaOverrides {
    pub blur_px: Option<f32>,
}

impl LumaOverrides {
    pub fn is_empty(&self) -> bool {
        self.blur_px.is_none()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ChromaOverrides {
    pub offset_px: Option<f32>,
    pub bleed_px: Option<f32>,
    pub saturation: Option<f32>,
}

impl ChromaOverrides {
    pub fn is_empty(&self) -> bool {
        self.offset_px.is_none() && self.bleed_px.is_none() && self.saturation.is_none()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NoiseOverrides {
    pub luma_amount: Option<f32>,
    pub chroma_amount: Option<f32>,
}

impl NoiseOverrides {
    pub fn is_empty(&self) -> bool {
        self.luma_amount.is_none() && self.chroma_amount.is_none()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TrackingOverrides {
    pub line_jitter_px: Option<f32>,
    pub vertical_offset_lines: Option<f32>,
}

impl TrackingOverrides {
    pub fn is_empty(&self) -> bool {
        self.line_jitter_px.is_none() && self.vertical_offset_lines.is_none()
    }
}

pub(crate) fn project_vhs_model_to_preview_signal(model: VhsModel) -> SignalSettings {
    // Only the currently active still-image runtime subset is projected here.
    // Formal input selectors, chroma phase terms, head switching, and output
    // transfer remain documented-only until a later runtime milestone.
    SignalSettings {
        tone: ToneSettings {
            highlight_soft_knee: model.tone.highlight_soft_knee,
            highlight_compression: model.tone.highlight_compression,
        },
        luma: LumaSettings {
            blur_px: luma_blur_from_bandwidth(model.luma.bandwidth_mhz),
        },
        chroma: ChromaSettings {
            offset_px: chroma_offset_from_delay(model.chroma.delay_us),
            bleed_px: chroma_bleed_from_bandwidth(model.chroma.bandwidth_khz),
            saturation: model.chroma.saturation_gain.max(0.0),
        },
        noise: NoiseSettings {
            luma_amount: luma_noise_amount_from_sigma(model.noise.luma_sigma),
            chroma_amount: chroma_noise_amount_from_sigma(model.noise.chroma_sigma),
        },
        tracking: TrackingSettings {
            line_jitter_px: line_jitter_px_from_timebase(model.transport.line_jitter_us),
            vertical_offset_lines: model.transport.vertical_wander_lines,
        },
    }
}

pub(crate) fn apply_preview_overrides(
    preview_base: SignalSettings,
    preview_overrides: SignalOverrides,
) -> SignalSettings {
    SignalSettings {
        tone: ToneSettings {
            highlight_soft_knee: preview_overrides
                .tone
                .highlight_soft_knee
                .unwrap_or(preview_base.tone.highlight_soft_knee),
            highlight_compression: preview_overrides
                .tone
                .highlight_compression
                .unwrap_or(preview_base.tone.highlight_compression),
        },
        luma: LumaSettings {
            blur_px: preview_overrides
                .luma
                .blur_px
                .unwrap_or(preview_base.luma.blur_px),
        },
        chroma: ChromaSettings {
            offset_px: preview_overrides
                .chroma
                .offset_px
                .unwrap_or(preview_base.chroma.offset_px),
            bleed_px: preview_overrides
                .chroma
                .bleed_px
                .unwrap_or(preview_base.chroma.bleed_px),
            saturation: preview_overrides
                .chroma
                .saturation
                .unwrap_or(preview_base.chroma.saturation),
        },
        noise: NoiseSettings {
            luma_amount: preview_overrides
                .noise
                .luma_amount
                .unwrap_or(preview_base.noise.luma_amount),
            chroma_amount: preview_overrides
                .noise
                .chroma_amount
                .unwrap_or(preview_base.noise.chroma_amount),
        },
        tracking: TrackingSettings {
            line_jitter_px: preview_overrides
                .tracking
                .line_jitter_px
                .unwrap_or(preview_base.tracking.line_jitter_px),
            vertical_offset_lines: preview_overrides
                .tracking
                .vertical_offset_lines
                .unwrap_or(preview_base.tracking.vertical_offset_lines),
        },
    }
}

pub(crate) fn effective_preview_signal(
    preview_base: SignalSettings,
    preview_overrides: SignalOverrides,
    has_formal_model: bool,
) -> SignalSettings {
    if has_formal_model {
        normalize_model_preview_signal(preview_base, preview_overrides)
    } else {
        normalize_manual_preview_signal(apply_preview_overrides(preview_base, preview_overrides))
    }
}

fn normalize_model_preview_signal(
    preview_base: SignalSettings,
    preview_overrides: SignalOverrides,
) -> SignalSettings {
    if preview_overrides.is_empty() {
        return preview_base;
    }

    let tone = ToneSettings {
        highlight_soft_knee: preview_overrides
            .tone
            .highlight_soft_knee
            .map(|value| {
                normalize_preview_tone(ToneSettings {
                    highlight_soft_knee: value,
                    highlight_compression: preview_base.tone.highlight_compression,
                })
                .highlight_soft_knee
            })
            .unwrap_or(preview_base.tone.highlight_soft_knee),
        highlight_compression: preview_overrides
            .tone
            .highlight_compression
            .map(|value| {
                normalize_preview_tone(ToneSettings {
                    highlight_soft_knee: preview_base.tone.highlight_soft_knee,
                    highlight_compression: value,
                })
                .highlight_compression
            })
            .unwrap_or(preview_base.tone.highlight_compression),
    };

    let luma = LumaSettings {
        blur_px: preview_overrides
            .luma
            .blur_px
            .map(|value| normalize_preview_luma(LumaSettings { blur_px: value }).blur_px)
            .unwrap_or(preview_base.luma.blur_px),
    };

    let chroma = normalize_model_chroma(preview_base.chroma, preview_overrides.chroma);

    let noise = NoiseSettings {
        luma_amount: preview_overrides
            .noise
            .luma_amount
            .map(|value| {
                normalize_preview_noise(NoiseSettings {
                    luma_amount: value,
                    chroma_amount: preview_base.noise.chroma_amount,
                })
                .luma_amount
            })
            .unwrap_or(preview_base.noise.luma_amount),
        chroma_amount: preview_overrides
            .noise
            .chroma_amount
            .map(|value| {
                normalize_preview_noise(NoiseSettings {
                    luma_amount: preview_base.noise.luma_amount,
                    chroma_amount: value,
                })
                .chroma_amount
            })
            .unwrap_or(preview_base.noise.chroma_amount),
    };

    let tracking = TrackingSettings {
        line_jitter_px: preview_overrides
            .tracking
            .line_jitter_px
            .map(|value| {
                normalize_preview_tracking(TrackingSettings {
                    line_jitter_px: value,
                    vertical_offset_lines: preview_base.tracking.vertical_offset_lines,
                })
                .line_jitter_px
            })
            .unwrap_or(preview_base.tracking.line_jitter_px),
        vertical_offset_lines: preview_overrides
            .tracking
            .vertical_offset_lines
            .map(|value| {
                normalize_preview_tracking(TrackingSettings {
                    line_jitter_px: preview_base.tracking.line_jitter_px,
                    vertical_offset_lines: value,
                })
                .vertical_offset_lines
            })
            .unwrap_or(preview_base.tracking.vertical_offset_lines),
    };

    SignalSettings {
        tone,
        luma,
        chroma,
        noise,
        tracking,
    }
}

fn normalize_model_chroma(
    preview_base: ChromaSettings,
    preview_overrides: ChromaOverrides,
) -> ChromaSettings {
    let override_offset_or_bleed =
        preview_overrides.offset_px.is_some() || preview_overrides.bleed_px.is_some();

    let mut chroma = if override_offset_or_bleed {
        normalize_preview_chroma(ChromaSettings {
            offset_px: preview_overrides
                .offset_px
                .unwrap_or(preview_base.offset_px),
            bleed_px: preview_overrides.bleed_px.unwrap_or(preview_base.bleed_px),
            saturation: preview_base.saturation,
        })
    } else {
        preview_base
    };

    chroma.saturation = preview_overrides
        .saturation
        .map(|value| {
            normalize_preview_chroma(ChromaSettings {
                offset_px: preview_base.offset_px,
                bleed_px: preview_base.bleed_px,
                saturation: value,
            })
            .saturation
        })
        .unwrap_or(preview_base.saturation);

    chroma
}

fn normalize_manual_preview_signal(signal: SignalSettings) -> SignalSettings {
    SignalSettings {
        tone: normalize_preview_tone(signal.tone),
        luma: normalize_preview_luma(signal.luma),
        chroma: normalize_preview_chroma(signal.chroma),
        noise: normalize_preview_noise(signal.noise),
        tracking: normalize_preview_tracking(signal.tracking),
    }
}

fn normalize_preview_tone(tone: ToneSettings) -> ToneSettings {
    ToneSettings {
        highlight_soft_knee: tone.highlight_soft_knee.clamp(0.0, 0.999),
        highlight_compression: tone.highlight_compression.max(0.0),
    }
}

fn normalize_preview_luma(luma: LumaSettings) -> LumaSettings {
    LumaSettings {
        blur_px: soft_cap_magnitude(
            luma.blur_px,
            PREVIEW_LUMA_BLUR_RECOMMENDED_CAP,
            PREVIEW_LUMA_BLUR_HARD_CAP,
        ),
    }
}

fn normalize_preview_chroma(chroma: ChromaSettings) -> ChromaSettings {
    let offset_px = soft_cap_signed(
        chroma.offset_px,
        PREVIEW_CHROMA_OFFSET_RECOMMENDED_CAP,
        PREVIEW_CHROMA_OFFSET_HARD_CAP,
    );
    let bleed_px = soft_cap_magnitude(
        chroma.bleed_px,
        PREVIEW_CHROMA_BLEED_RECOMMENDED_CAP,
        PREVIEW_CHROMA_BLEED_HARD_CAP,
    )
    .max(offset_px.abs() * PREVIEW_CHROMA_BLEED_OFFSET_RATIO);

    ChromaSettings {
        offset_px,
        bleed_px,
        saturation: chroma.saturation.max(0.0),
    }
}

fn normalize_preview_noise(noise: NoiseSettings) -> NoiseSettings {
    NoiseSettings {
        luma_amount: soft_cap_magnitude(
            noise.luma_amount,
            PREVIEW_LUMA_NOISE_RECOMMENDED_CAP,
            PREVIEW_LUMA_NOISE_HARD_CAP,
        ),
        chroma_amount: soft_cap_magnitude(
            noise.chroma_amount,
            PREVIEW_CHROMA_NOISE_RECOMMENDED_CAP,
            PREVIEW_CHROMA_NOISE_HARD_CAP,
        ),
    }
}

fn normalize_preview_tracking(tracking: TrackingSettings) -> TrackingSettings {
    TrackingSettings {
        line_jitter_px: soft_cap_magnitude(
            tracking.line_jitter_px.abs(),
            PREVIEW_LINE_JITTER_RECOMMENDED_CAP,
            PREVIEW_LINE_JITTER_HARD_CAP,
        ),
        vertical_offset_lines: soft_cap_signed(
            tracking.vertical_offset_lines,
            PREVIEW_VERTICAL_OFFSET_RECOMMENDED_CAP,
            PREVIEW_VERTICAL_OFFSET_HARD_CAP,
        ),
    }
}

fn soft_cap_magnitude(value: f32, recommended_cap: f32, hard_cap: f32) -> f32 {
    let magnitude = value.max(0.0);
    if magnitude <= recommended_cap {
        return magnitude;
    }

    let span = (hard_cap - recommended_cap).max(f32::EPSILON);
    let excess = magnitude - recommended_cap;
    recommended_cap + (excess * span) / (excess + span)
}

fn soft_cap_signed(value: f32, recommended_cap: f32, hard_cap: f32) -> f32 {
    value.signum() * soft_cap_magnitude(value.abs(), recommended_cap, hard_cap)
}

fn line_jitter_px_from_timebase(line_jitter_us: f32) -> f32 {
    line_jitter_us.max(0.0) * BT601_SAMPLES_PER_US * STILL_JITTER_ATTENUATION
}

fn chroma_offset_from_delay(delay_us: f32) -> f32 {
    delay_us * BT601_SAMPLES_PER_US * STILL_CHROMA_DELAY_ATTENUATION
}

fn luma_blur_from_bandwidth(bandwidth_mhz: f32) -> f32 {
    (((REFERENCE_LUMA_BANDWIDTH_MHZ - bandwidth_mhz).max(0.0)) / 1.0 * 1.6).min(4.5)
}

fn chroma_bleed_from_bandwidth(bandwidth_khz: f32) -> f32 {
    (((REFERENCE_CHROMA_BANDWIDTH_KHZ - bandwidth_khz).max(0.0)) / 300.0).min(4.5)
}

fn luma_noise_amount_from_sigma(luma_sigma: f32) -> f32 {
    luma_sigma.clamp(0.0, 1.0)
}

fn chroma_noise_amount_from_sigma(chroma_sigma: f32) -> f32 {
    (chroma_sigma.max(0.0) * 0.35).min(1.0)
}
