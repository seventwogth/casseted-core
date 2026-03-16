//! Built-in WGSL shader source registry for the workspace.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderSource {
    pub name: &'static str,
    pub source: &'static str,
}

pub const SIGNAL_PREVIEW_SHADER: ShaderSource = ShaderSource {
    name: "signal_preview",
    source: include_str!("../../../shaders/signal_preview.wgsl"),
};

pub fn builtin_shaders() -> &'static [ShaderSource] {
    &[SIGNAL_PREVIEW_SHADER]
}

pub fn find_shader(name: &str) -> Option<ShaderSource> {
    builtin_shaders()
        .iter()
        .copied()
        .find(|shader| shader.name == name)
}

#[cfg(test)]
mod tests {
    use super::SIGNAL_PREVIEW_SHADER;

    #[test]
    fn embedded_shader_is_not_empty() {
        assert!(SIGNAL_PREVIEW_SHADER.source.contains("@vertex"));
    }
}
