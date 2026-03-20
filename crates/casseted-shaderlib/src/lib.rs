//! Built-in WGSL shader source registry for the workspace.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderId {
    StillInputConditioning,
    StillLumaDegradation,
    StillChromaDegradation,
    StillReconstructionOutput,
}

impl ShaderId {
    pub const fn label(self) -> &'static str {
        match self {
            Self::StillInputConditioning => "still_input_conditioning",
            Self::StillLumaDegradation => "still_luma_degradation",
            Self::StillChromaDegradation => "still_chroma_degradation",
            Self::StillReconstructionOutput => "still_reconstruction_output",
        }
    }

    pub const fn relative_path(self) -> &'static str {
        match self {
            Self::StillInputConditioning => "shaders/passes/still_input_conditioning.wgsl",
            Self::StillLumaDegradation => "shaders/passes/still_luma_degradation.wgsl",
            Self::StillChromaDegradation => "shaders/passes/still_chroma_degradation.wgsl",
            Self::StillReconstructionOutput => "shaders/passes/still_reconstruction_output.wgsl",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderSource {
    pub id: ShaderId,
    pub label: &'static str,
    pub relative_path: &'static str,
    pub source: &'static str,
}

pub const STILL_INPUT_CONDITIONING_SHADER: ShaderSource = ShaderSource {
    id: ShaderId::StillInputConditioning,
    label: "still_input_conditioning",
    relative_path: "shaders/passes/still_input_conditioning.wgsl",
    source: include_str!("../../../shaders/passes/still_input_conditioning.wgsl"),
};

pub const STILL_LUMA_DEGRADATION_SHADER: ShaderSource = ShaderSource {
    id: ShaderId::StillLumaDegradation,
    label: "still_luma_degradation",
    relative_path: "shaders/passes/still_luma_degradation.wgsl",
    source: include_str!("../../../shaders/passes/still_luma_degradation.wgsl"),
};

pub const STILL_CHROMA_DEGRADATION_SHADER: ShaderSource = ShaderSource {
    id: ShaderId::StillChromaDegradation,
    label: "still_chroma_degradation",
    relative_path: "shaders/passes/still_chroma_degradation.wgsl",
    source: include_str!("../../../shaders/passes/still_chroma_degradation.wgsl"),
};

pub const STILL_RECONSTRUCTION_OUTPUT_SHADER: ShaderSource = ShaderSource {
    id: ShaderId::StillReconstructionOutput,
    label: "still_reconstruction_output",
    relative_path: "shaders/passes/still_reconstruction_output.wgsl",
    source: include_str!("../../../shaders/passes/still_reconstruction_output.wgsl"),
};

pub const BUILTIN_SHADERS: [ShaderSource; 4] = [
    STILL_INPUT_CONDITIONING_SHADER,
    STILL_LUMA_DEGRADATION_SHADER,
    STILL_CHROMA_DEGRADATION_SHADER,
    STILL_RECONSTRUCTION_OUTPUT_SHADER,
];

pub fn builtin_shaders() -> &'static [ShaderSource] {
    &BUILTIN_SHADERS
}

pub fn shader_source(id: ShaderId) -> ShaderSource {
    match id {
        ShaderId::StillInputConditioning => STILL_INPUT_CONDITIONING_SHADER,
        ShaderId::StillLumaDegradation => STILL_LUMA_DEGRADATION_SHADER,
        ShaderId::StillChromaDegradation => STILL_CHROMA_DEGRADATION_SHADER,
        ShaderId::StillReconstructionOutput => STILL_RECONSTRUCTION_OUTPUT_SHADER,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BUILTIN_SHADERS, STILL_CHROMA_DEGRADATION_SHADER, STILL_INPUT_CONDITIONING_SHADER,
        STILL_LUMA_DEGRADATION_SHADER, STILL_RECONSTRUCTION_OUTPUT_SHADER, ShaderId, shader_source,
    };

    #[test]
    fn embedded_shaders_are_not_empty() {
        for shader in BUILTIN_SHADERS {
            assert!(shader.source.contains("@vertex"));
            assert!(shader.source.contains("@fragment"));
        }
    }

    #[test]
    fn shader_lookup_by_id_returns_expected_asset() {
        let cases = [
            (
                ShaderId::StillInputConditioning,
                "still_input_conditioning",
                "shaders/passes/still_input_conditioning.wgsl",
            ),
            (
                ShaderId::StillLumaDegradation,
                "still_luma_degradation",
                "shaders/passes/still_luma_degradation.wgsl",
            ),
            (
                ShaderId::StillChromaDegradation,
                "still_chroma_degradation",
                "shaders/passes/still_chroma_degradation.wgsl",
            ),
            (
                ShaderId::StillReconstructionOutput,
                "still_reconstruction_output",
                "shaders/passes/still_reconstruction_output.wgsl",
            ),
        ];

        for (id, label, relative_path) in cases {
            let shader = shader_source(id);
            assert_eq!(shader.label, label);
            assert_eq!(shader.relative_path, relative_path);
        }
    }

    #[test]
    fn builtin_registry_contains_expected_passes() {
        assert!(BUILTIN_SHADERS.contains(&STILL_INPUT_CONDITIONING_SHADER));
        assert!(BUILTIN_SHADERS.contains(&STILL_LUMA_DEGRADATION_SHADER));
        assert!(BUILTIN_SHADERS.contains(&STILL_CHROMA_DEGRADATION_SHADER));
        assert!(BUILTIN_SHADERS.contains(&STILL_RECONSTRUCTION_OUTPUT_SHADER));
    }

    #[test]
    fn embedded_shaders_contain_expected_ops() {
        assert!(
            STILL_INPUT_CONDITIONING_SHADER
                .source
                .contains("conditioned_sample_uv")
        );
        assert!(
            STILL_LUMA_DEGRADATION_SHADER
                .source
                .contains("degrade_luma")
        );
        assert!(
            STILL_CHROMA_DEGRADATION_SHADER
                .source
                .contains("degrade_chroma")
        );
        assert!(
            STILL_RECONSTRUCTION_OUTPUT_SHADER
                .source
                .contains("sample_reconstruction_contamination")
        );
        assert!(
            STILL_RECONSTRUCTION_OUTPUT_SHADER
                .source
                .contains("compose_display_yuv")
        );
    }
}
