struct VertexInput {
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] tex_coords: vec2<f32>;
};
struct InstanceInput {
    [[location(5)]] model_matrix_0: vec4<f32>;
    [[location(6)]] model_matrix_1: vec4<f32>;
    [[location(7)]] model_matrix_2: vec4<f32>;
    [[location(8)]] model_matrix_3: vec4<f32>;
};

struct VertexOutput {
    [[location(0)]] tex_coords: vec2<f32>;
    [[builtin(position)]] position: vec4<f32>;
};


[[block]]
struct PushConstants {
    mvp: mat4x4<f32>;
    model: mat4x4<f32>;
};
var<push_constant> push: PushConstants;


[[stage(vertex)]]
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    out.position = push.mvp * vec4<f32>(model.position, 1.0);
    return out;
}

[[block]]
struct MaterialOptions {
    diffuse_enabled: u32;
};
[[group(0), binding(0)]]
var<uniform> options: MaterialOptions;
[[group(0), binding(1)]]
var t_diffuse: texture_2d<f32>;
[[group(0), binding(2)]]
var s_diffuse: sampler;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    if (options.diffuse_enabled == 1u) {
        return textureSample(t_diffuse, s_diffuse, vec2<f32>(in.tex_coords.x, 1.0 - in.tex_coords.y));
    } else {
        return vec4<f32>(1.);
    }
}
