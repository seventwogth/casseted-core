//! Minimal composition layer for frame metadata, signal settings, and shader selection.

use casseted_gpu::GpuRequirements;
use casseted_shaderlib::{ShaderSource, find_shader};
use casseted_signal::{SignalPlan, SignalProfile};
use casseted_types::FrameDescriptor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelinePreset {
    SignalPreview,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PipelineDefinition {
    pub preset: PipelinePreset,
    pub frame: FrameDescriptor,
    pub signal: SignalProfile,
    pub shader_name: &'static str,
}

impl PipelineDefinition {
    pub fn signal_preview(frame: FrameDescriptor) -> Self {
        Self {
            preset: PipelinePreset::SignalPreview,
            frame,
            signal: SignalProfile::default(),
            shader_name: "signal_preview",
        }
    }

    pub fn signal_plan(&self) -> SignalPlan {
        SignalPlan::preview(self.frame.clone(), self.signal)
    }

    pub fn shader(&self) -> ShaderSource {
        find_shader(self.shader_name).expect("pipeline preset references a built-in shader")
    }

    pub fn gpu_requirements(&self) -> GpuRequirements {
        GpuRequirements::default()
    }
}

#[cfg(test)]
mod tests {
    use super::PipelineDefinition;
    use casseted_types::FrameDescriptor;

    #[test]
    fn signal_preview_uses_embedded_shader() {
        let pipeline = PipelineDefinition::signal_preview(FrameDescriptor::default());

        assert_eq!(pipeline.shader().name, "signal_preview");
    }
}
