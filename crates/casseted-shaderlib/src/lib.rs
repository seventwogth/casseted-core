//! Built-in WGSL shader source registry for the workspace.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderId {
    SignalPreview,
}

impl ShaderId {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SignalPreview => "signal_preview",
        }
    }

    pub const fn relative_path(self) -> &'static str {
        match self {
            Self::SignalPreview => "shaders/passes/signal_preview.wgsl",
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

pub const SIGNAL_PREVIEW_SHADER: ShaderSource = ShaderSource {
    id: ShaderId::SignalPreview,
    label: "signal_preview",
    relative_path: "shaders/passes/signal_preview.wgsl",
    source: include_str!("../../../shaders/passes/signal_preview.wgsl"),
};

pub const BUILTIN_SHADERS: [ShaderSource; 1] = [SIGNAL_PREVIEW_SHADER];

pub fn builtin_shaders() -> &'static [ShaderSource] {
    &BUILTIN_SHADERS
}

pub fn shader_source(id: ShaderId) -> ShaderSource {
    match id {
        ShaderId::SignalPreview => SIGNAL_PREVIEW_SHADER,
    }
}

pub fn find_shader(label: &str) -> Option<ShaderSource> {
    builtin_shaders()
        .iter()
        .copied()
        .find(|shader| shader.label == label)
}

#[cfg(test)]
mod tests {
    use super::{SIGNAL_PREVIEW_SHADER, ShaderId, builtin_shaders, shader_source};

    #[test]
    fn embedded_shader_is_not_empty() {
        assert!(SIGNAL_PREVIEW_SHADER.source.contains("@vertex"));
    }

    #[test]
    fn shader_lookup_by_id_returns_expected_asset() {
        let shader = shader_source(ShaderId::SignalPreview);

        assert_eq!(shader.label, "signal_preview");
        assert_eq!(shader.relative_path, "shaders/passes/signal_preview.wgsl");
    }

    #[test]
    fn builtin_registry_contains_signal_preview() {
        assert!(
            builtin_shaders()
                .iter()
                .any(|shader| shader.id == ShaderId::SignalPreview)
        );
    }

    #[test]
    fn embedded_shader_contains_fragment_entrypoint() {
        assert!(SIGNAL_PREVIEW_SHADER.source.contains("fs_main"));
    }
}
