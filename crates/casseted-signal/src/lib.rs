//! Domain-level signal models for analog-style transforms.
//!
//! The crate currently exposes two layers on purpose:
//! - `SignalSettings`: a compact prototype-oriented parameter set used by the
//!   first still-image shader pipeline
//! - `VhsModel`: the formal VHS/analog v1 model that describes the intended
//!   signal flow and parameter taxonomy for future implementations

mod prototype;
mod vhs;

pub use prototype::{
    ChromaSettings, LumaSettings, NoiseSettings, SignalSettings, TrackingSettings,
};
pub use vhs::{
    InputTransfer, OutputTransfer, TemporalSampling, VHS_SIGNAL_FLOW_V1, VhsChromaSettings,
    VhsDecodeSettings, VhsInputSettings, VhsLumaSettings, VhsModel, VhsNoiseSettings,
    VhsSignalStage, VhsTransportSettings, VideoMatrix, VideoStandard,
};
