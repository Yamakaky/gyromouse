struct VertexInput {
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] uv: vec2<f32>;
    [[location(2)]] normal: vec3<f32>;
};

struct VertexOutput {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] uv: vec2<f32>;
    [[location(1)]] normal: vec4<f32>;
};


[[block]]
struct PushConstants {
    mvp: mat4x4<f32>;
    model: mat4x4<f32>;
};
var<push_constant> push: PushConstants;


[[stage(vertex)]]
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = push.mvp * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    out.normal = push.model * vec4<f32>(in.normal, 1.0);
    return out;
}

[[block]]
struct Material {
    base_color: vec4<f32>;
    use_diffuse_texture: u32;
};
[[group(0), binding(0)]]
var<uniform> material: Material;
[[group(0), binding(1)]]
var t_diffuse: texture_2d<f32>;
[[group(0), binding(2)]]
var s_diffuse: sampler;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    var color = material.base_color;
    if (material.use_diffuse_texture == 1u) {
        color = color * textureSample(t_diffuse, s_diffuse, in.uv);
    }
    return color;
}
