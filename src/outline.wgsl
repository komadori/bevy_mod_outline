#import bevy_pbr::mesh_view_bind_group
#import bevy_pbr::mesh_struct

struct Vertex {
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] normal: vec3<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
};

struct VertexStageData {
    width: f32;
};

struct FragmentStageData {
    colour: vec4<f32>;
};

[[group(1), binding(0)]]
var<uniform> mesh: Mesh;

[[group(2), binding(0)]]
var<uniform> vstage: VertexStageData;

[[group(2), binding(1)]]
var<uniform> fstage: FragmentStageData;

fn mat4to3(m: mat4x4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(
        m[0].xyz, m[1].xyz, m[2].xyz
    );
}

[[stage(vertex)]]
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    var clip_pos = view.view_proj * (mesh.model * vec4<f32>(vertex.position, 1.0));
    var clip_norm = mat4to3(view.view_proj) * (mat4to3(mesh.model) * normalize(vertex.normal));
    var clip_delta = vec4<f32>(vstage.width * normalize(clip_norm.xy) * clip_pos.w / vec2<f32>(view.width, view.height), 0.0, 0.0);
    out.clip_position = clip_pos + clip_delta;
    return out;
}

struct FragmentInput {
    [[builtin(front_facing)]] is_front: bool;
};

[[stage(fragment)]]
fn fragment(in: FragmentInput) -> [[location(0)]] vec4<f32> {
    return fstage.colour;
}
