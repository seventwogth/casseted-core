//! Built-in WGSL shader source registry for the workspace.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderId {
    StillAnalog,
}

impl ShaderId {
    pub const fn label(self) -> &'static str {
        match self {
            Self::StillAnalog => "still_analog",
        }
    }

    pub const fn relative_path(self) -> &'static str {
        match self {
            Self::StillAnalog => "shaders/passes/still_analog.wgsl",
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

pub const STILL_ANALOG_SHADER: ShaderSource = ShaderSource {
    id: ShaderId::StillAnalog,
    label: "still_analog",
    relative_path: "shaders/passes/still_analog.wgsl",
    source: include_str!("../../../shaders/passes/still_analog.wgsl"),
};

pub const BUILTIN_SHADERS: [ShaderSource; 1] = [STILL_ANALOG_SHADER];

pub fn builtin_shaders() -> &'static [ShaderSource] {
    &BUILTIN_SHADERS
}

pub fn shader_source(id: ShaderId) -> ShaderSource {
    match id {
        ShaderId::StillAnalog => STILL_ANALOG_SHADER,
    }
}

#[cfg(test)]
mod tests {
    use super::{BUILTIN_SHADERS, STILL_ANALOG_SHADER, ShaderId, shader_source};

    #[test]
    fn embedded_shader_is_not_empty() {
        assert!(STILL_ANALOG_SHADER.source.contains("@vertex"));
    }

    #[test]
    fn shader_lookup_by_id_returns_expected_asset() {
        let shader = shader_source(ShaderId::StillAnalog);

        assert_eq!(shader.label, "still_analog");
        assert_eq!(shader.relative_path, "shaders/passes/still_analog.wgsl");
    }

    #[test]
    fn builtin_registry_contains_still_analog() {
        assert!(
            BUILTIN_SHADERS
                .iter()
                .any(|shader| shader.id == ShaderId::StillAnalog)
        );
    }

    #[test]
    fn embedded_shader_contains_fragment_entrypoint() {
        assert!(STILL_ANALOG_SHADER.source.contains("textureSample"));
    }
}
