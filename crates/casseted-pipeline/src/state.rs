use crate::projection::{
    SignalOverrides, apply_preview_overrides, effective_preview_signal,
    project_vhs_model_to_preview_signal,
};
use crate::runtime::{PipelineError, StillPipelineRuntime, process_with_gpu};
use casseted_gpu::{GpuContext, GpuContextDescriptor};
use casseted_shaderlib::ShaderId;
use casseted_signal::{SignalSettings, VhsModel};
use casseted_types::ImageFrame;

const STILL_PIPELINE_SHADER_IDS: [ShaderId; 4] = [
    ShaderId::StillInputConditioning,
    ShaderId::StillLumaDegradation,
    ShaderId::StillChromaDegradation,
    ShaderId::StillReconstructionOutput,
];

#[derive(Debug, Clone, Copy, PartialEq)]
struct PipelineState {
    model: Option<VhsModel>,
    preview_base: SignalSettings,
    preview_overrides: SignalOverrides,
}

impl PipelineState {
    fn manual(signal: SignalSettings) -> Self {
        Self {
            model: None,
            preview_base: signal,
            preview_overrides: SignalOverrides::default(),
        }
    }

    fn from_model(model: VhsModel) -> Self {
        Self {
            model: Some(model),
            preview_base: project_vhs_model_to_preview_signal(model),
            preview_overrides: SignalOverrides::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StillImagePipeline {
    state: PipelineState,
}

impl StillImagePipeline {
    pub fn new(signal: SignalSettings) -> Self {
        Self {
            state: PipelineState::manual(signal),
        }
    }

    pub fn from_vhs_model(model: VhsModel) -> Self {
        Self {
            state: PipelineState::from_model(model),
        }
    }

    pub fn model(&self) -> Option<VhsModel> {
        self.state.model
    }

    pub fn preview_base_signal(&self) -> SignalSettings {
        self.state.preview_base
    }

    pub fn preview_overrides(&self) -> SignalOverrides {
        self.state.preview_overrides
    }

    pub fn preview_signal(&self) -> SignalSettings {
        apply_preview_overrides(self.state.preview_base, self.state.preview_overrides)
    }

    pub fn effective_preview_signal(&self) -> SignalSettings {
        effective_preview_signal(
            self.state.preview_base,
            self.state.preview_overrides,
            self.state.model.is_some(),
        )
    }

    pub fn set_model(&mut self, model: VhsModel) {
        self.state.model = Some(model);
        self.state.preview_base = project_vhs_model_to_preview_signal(model);
    }

    pub fn clear_model(&mut self) {
        self.state.preview_base = self.preview_signal();
        self.state.model = None;
        self.state.preview_overrides = SignalOverrides::default();
    }

    pub fn set_preview_signal(&mut self, signal: SignalSettings) {
        self.state = PipelineState::manual(signal);
    }

    pub fn set_preview_overrides(&mut self, overrides: SignalOverrides) {
        self.state.preview_overrides = overrides;
    }

    pub fn clear_preview_overrides(&mut self) {
        self.state.preview_overrides = SignalOverrides::default();
    }

    pub fn shader_ids(&self) -> &'static [ShaderId] {
        &STILL_PIPELINE_SHADER_IDS
    }

    pub fn process_blocking(&self, input: &ImageFrame) -> Result<ImageFrame, PipelineError> {
        let context = pollster::block_on(GpuContext::request(&GpuContextDescriptor::default()))
            .map_err(PipelineError::GpuInit)?;

        self.process_with_gpu(&context, input)
    }

    pub fn process_with_gpu(
        &self,
        context: &GpuContext,
        input: &ImageFrame,
    ) -> Result<ImageFrame, PipelineError> {
        process_with_gpu(self, context, input)
    }

    pub fn process_with_runtime(
        &self,
        runtime: &StillPipelineRuntime<'_>,
        input: &ImageFrame,
    ) -> Result<ImageFrame, PipelineError> {
        runtime.process(self, input)
    }
}

impl Default for StillImagePipeline {
    fn default() -> Self {
        Self::from_vhs_model(VhsModel::default())
    }
}
