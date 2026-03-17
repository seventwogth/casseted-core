struct EffectUniform {
    frame: vec4<f32>,
    input_conditioning: vec4<f32>,
    luma_degradation: vec4<f32>,
    chroma_degradation: vec4<f32>,
    reconstruction_output: vec4<f32>,
    reconstruction_aux: vec4<f32>,
};

struct VsOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> effect: EffectUniform;

const BT601_LUMA: vec3<f32> = vec3<f32>(0.299, 0.587, 0.114);

fn rgb_to_yuv(rgb: vec3<f32>) -> vec3<f32> {
    let y = dot(rgb, BT601_LUMA);
    let u = (rgb.b - y) * 0.492111;
    let v = (rgb.r - y) * 0.877283;
    return vec3<f32>(y, u, v);
}

fn sample_rgb(uv: vec2<f32>) -> vec3<f32> {
    let clamped = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(input_texture, input_sampler, clamped).rgb;
}

fn frame_inv_size() -> vec2<f32> {
    return vec2<f32>(effect.frame.z, 1.0 / max(effect.frame.y, 1.0));
}

fn soft_highlight_knee(value: f32, knee: f32, compression: f32) -> f32 {
    if (compression <= 0.0 || value <= knee || knee >= 0.999) {
        return value;
    }

    let headroom = max(1e-5, 1.0 - knee);
    let t = clamp((value - knee) / headroom, 0.0, 1.0);
    let compressed = log2(1.0 + compression * t) / log2(1.0 + compression);
    return knee + compressed * headroom;
}

fn apply_tone_shaping(rgb: vec3<f32>) -> vec3<f32> {
    let y = dot(rgb, BT601_LUMA);
    if (y <= 1e-5) {
        return rgb;
    }

    let toned_y = soft_highlight_knee(
        y,
        effect.input_conditioning.x,
        effect.input_conditioning.y,
    );
    let scale = toned_y / max(y, 1e-5);
    return clamp(rgb * scale, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn conditioned_sample_uv(uv: vec2<f32>) -> vec2<f32> {
    let frame_size = effect.frame.xy;
    let inv_size = frame_inv_size();
    let line_index = floor(uv.y * frame_size.y + effect.input_conditioning.w);
    let line_phase = line_index + effect.frame.w * 0.5;
    let line_jitter = sin(line_phase * 0.37) * effect.input_conditioning.z * inv_size.x;
    return vec2<f32>(
        uv.x + line_jitter,
        uv.y + effect.input_conditioning.w * inv_size.y,
    );
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );

    var output: VsOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.uv = uvs[vertex_index];
    return output;
}

@fragment
fn fs_main(in: VsOutput) -> @location(0) vec4<f32> {
    let sample_uv = conditioned_sample_uv(in.uv);
    let working_signal = rgb_to_yuv(apply_tone_shaping(sample_rgb(sample_uv)));
    return vec4<f32>(working_signal, 1.0);
}
