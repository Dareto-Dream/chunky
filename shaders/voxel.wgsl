struct Uniforms {
    view_proj: mat4x4<f32>,
    light_dir: vec3<f32>,
    _pad: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // Instance attributes
    @location(2) inst_pos: vec3<f32>,
    @location(3) inst_color: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world = in.position + in.inst_pos;
    out.clip_pos = uniforms.view_proj * vec4<f32>(world, 1.0);
    out.color = in.inst_color;
    out.normal = in.normal;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light = normalize(uniforms.light_dir);
    let diffuse = max(dot(normalize(in.normal), light), 0.0);
    let lighting = 0.35 + diffuse * 0.65;
    return vec4<f32>(in.color * lighting, 1.0);
}

// ─── Grid lines ──────────────────────────────────────────────────────────────

struct GridUniforms {
    view_proj: mat4x4<f32>,
    color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> grid_uniforms: GridUniforms;

struct GridVertex {
    @location(0) position: vec3<f32>,
}

struct GridFragment {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_grid(in: GridVertex) -> GridFragment {
    var out: GridFragment;
    out.pos = grid_uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.color = grid_uniforms.color;
    return out;
}

@fragment
fn fs_grid(in: GridFragment) -> @location(0) vec4<f32> {
    return in.color;
}
