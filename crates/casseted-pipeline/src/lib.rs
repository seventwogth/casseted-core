//! Minimal still-image GPU pipeline for analog-inspired processing.

mod projection;
mod runtime;
mod stages;
mod state;

pub use projection::{
    ChromaOverrides, LumaOverrides, NoiseOverrides, SignalOverrides, ToneOverrides,
    TrackingOverrides,
};
pub use runtime::PipelineError;
pub use state::StillImagePipeline;

#[cfg(test)]
mod stage_regression;

#[cfg(test)]
mod tests;
