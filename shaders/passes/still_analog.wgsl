struct EffectUniform {
    frame: vec4<f32>,
    luma_chroma: vec4<f32>,
    noise_tracking: vec4<f32>,
};

struct VsOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> effect: EffectUniform;

fn rgb_to_yuv(rgb: vec3<f32>) -> vec3<f32> {
    let y = dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
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
    let line_index = in.uv.y * frame_size.y + effect.noise_tracking.z + effect.noise_tracking.w;
    let line_jitter = sin(line_index * 0.35) * effect.luma_chroma.w * inv_size.x;
    let base_uv = vec2<f32>(in.uv.x + line_jitter, in.uv.y + effect.noise_tracking.z * inv_size.y);

    let blur_offset = effect.luma_chroma.x * inv_size.x;
    let left = rgb_to_yuv(sample_rgb(base_uv - vec2<f32>(blur_offset, 0.0)));
    let center = rgb_to_yuv(sample_rgb(base_uv));
    let right = rgb_to_yuv(sample_rgb(base_uv + vec2<f32>(blur_offset, 0.0)));
    let blurred_luma = left.x * 0.25 + center.x * 0.5 + right.x * 0.25;

    let chroma_offset = effect.luma_chroma.y * inv_size.x;
    let bleed_offset = effect.luma_chroma.z * inv_size.x;
    let chroma_a = rgb_to_yuv(sample_rgb(base_uv + vec2<f32>(chroma_offset, 0.0)));
    let chroma_b = rgb_to_yuv(sample_rgb(base_uv + vec2<f32>(chroma_offset + bleed_offset, 0.0)));
    let chroma = mix(chroma_a.yz, chroma_b.yz, 0.35);

    let noise_coord = vec2<f32>(floor(base_uv.x * frame_size.x), floor(base_uv.y * frame_size.y));
    let luma_noise = (hash12(noise_coord + vec2<f32>(effect.noise_tracking.w, 3.0)) - 0.5)
        * effect.noise_tracking.x;
    let chroma_noise = (hash12(noise_coord.yx + vec2<f32>(5.0, effect.noise_tracking.w)) - 0.5)
        * effect.noise_tracking.y;

    let yuv = vec3<f32>(
        blurred_luma + luma_noise,
        chroma.x + chroma_noise,
        chroma.y - chroma_noise * 0.5,
    );
    let rgb = clamp(yuv_to_rgb(yuv), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, 1.0);
}
