struct EffectUniform {
    frame: vec4<f32>,
    tone_luma: vec4<f32>,
    chroma: vec4<f32>,
    transport: vec4<f32>,
    noise_decode: vec4<f32>,
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

fn yuv_to_rgb(yuv: vec3<f32>) -> vec3<f32> {
    let y = yuv.x;
    let u = yuv.y;
    let v = yuv.z;
    return vec3<f32>(
        y + 1.13983 * v,
        y - 0.39465 * u - 0.58060 * v,
        y + 2.03211 * u,
    );
}

fn hash12(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn sample_rgb(uv: vec2<f32>) -> vec3<f32> {
    let clamped = clamp(uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
    return textureSample(input_texture, input_sampler, clamped).rgb;
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

fn apply_tone_curve(rgb: vec3<f32>) -> vec3<f32> {
    let y = dot(rgb, BT601_LUMA);
    if (y <= 1e-5) {
        return rgb;
    }

    let toned_y = soft_highlight_knee(y, effect.tone_luma.x, effect.tone_luma.y);
    let scale = toned_y / max(y, 1e-5);
    return clamp(rgb * scale, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn sample_working_yuv(uv: vec2<f32>) -> vec3<f32> {
    return rgb_to_yuv(apply_tone_curve(sample_rgb(uv)));
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(3.0, 1.0),
    );

    var output: VsOutput;
    let xy = positions[vertex_index];
    output.position = vec4<f32>(xy, 0.0, 1.0);
    output.uv = xy * 0.5 + vec2<f32>(0.5, 0.5);
    return output;
}

@fragment
fn fs_main(in: VsOutput) -> @location(0) vec4<f32> {
    let frame_size = effect.frame.xy;
    let inv_size = effect.frame.zw;
    let line_index = floor(in.uv.y * frame_size.y + effect.transport.y);
    let line_phase = line_index + effect.transport.z * 0.5;
    let line_jitter = sin(line_phase * 0.37) * effect.transport.x * inv_size.x;
    let base_uv = vec2<f32>(in.uv.x + line_jitter, in.uv.y + effect.transport.y * inv_size.y);

    let center = sample_working_yuv(base_uv);
    let luma_offset = effect.tone_luma.z * inv_size.x;
    let inner_step = vec2<f32>(luma_offset, 0.0);
    let outer_step = vec2<f32>(luma_offset * 2.0, 0.0);
    let left_outer = sample_working_yuv(base_uv - outer_step);
    let left_inner = sample_working_yuv(base_uv - inner_step);
    let right_inner = sample_working_yuv(base_uv + inner_step);
    let right_outer = sample_working_yuv(base_uv + outer_step);
    let blurred_luma = left_outer.x * 0.12
        + left_inner.x * 0.23
        + center.x * 0.30
        + right_inner.x * 0.23
        + right_outer.x * 0.12;
    let edge_band = left_inner.x * 0.2 + center.x * 0.6 + right_inner.x * 0.2;
    let luma = clamp(blurred_luma + (center.x - edge_band) * effect.tone_luma.w, 0.0, 1.0);

    let chroma_offset = effect.chroma.x * inv_size.x;
    let chroma_blur = effect.chroma.y * inv_size.x;
    let chroma_center = sample_working_yuv(base_uv + vec2<f32>(chroma_offset, 0.0));
    let chroma_left = sample_working_yuv(base_uv + vec2<f32>(chroma_offset - chroma_blur, 0.0));
    let chroma_right = sample_working_yuv(base_uv + vec2<f32>(chroma_offset + chroma_blur, 0.0));
    let chroma_horizontal = chroma_left.yz * 0.25 + chroma_center.yz * 0.5 + chroma_right.yz * 0.25;
    let chroma_up = sample_working_yuv(base_uv + vec2<f32>(chroma_offset, -inv_size.y)).yz;
    let chroma_down = sample_working_yuv(base_uv + vec2<f32>(chroma_offset, inv_size.y)).yz;
    let chroma_vertical = (chroma_up + chroma_horizontal * 2.0 + chroma_down) * 0.25;
    let chroma = mix(chroma_horizontal, chroma_vertical, effect.chroma.w) * effect.chroma.z;

    let noise_coord = vec2<f32>(floor(base_uv.x * frame_size.x), floor(base_uv.y * frame_size.y));
    let luma_noise = (hash12(noise_coord + vec2<f32>(effect.transport.z, 3.0)) - 0.5)
        * effect.noise_decode.x;
    let chroma_noise = (hash12(noise_coord.yx + vec2<f32>(5.0, effect.transport.z)) - 0.5)
        * effect.noise_decode.y;

    let reconstructed_y = clamp(
        luma + dot(chroma, vec2<f32>(0.10, -0.05)) * effect.noise_decode.z + luma_noise,
        0.0,
        1.0,
    );
    let reconstructed_chroma = chroma + vec2<f32>(chroma_noise, -chroma_noise * 0.5);
    let yuv = vec3<f32>(reconstructed_y, reconstructed_chroma.x, reconstructed_chroma.y);
    let rgb = clamp(yuv_to_rgb(yuv), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, 1.0);
}
