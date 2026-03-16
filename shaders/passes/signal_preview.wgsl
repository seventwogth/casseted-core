struct VsOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

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
    let base = vec3<f32>(0.92, 0.85, 0.72);
    let band = 0.05 * sin(in.uv.y * 720.0);
    let edge = smoothstep(0.0, 0.15, in.uv.x) * smoothstep(1.0, 0.85, in.uv.x);
    let color = base - band + vec3<f32>(0.0, 0.02, 0.03) * edge;
    return vec4<f32>(color, 1.0);
}
