#import bevy_pbr::mesh_view_bind_group
#import bevy_pbr::mesh_struct

struct Vertex {
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] normal: vec3<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
};

[[group(1), binding(0)]]
var<uniform> mesh: Mesh;

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
    var n_pos = clip_pos + vec4<f32>(50.0 * normalize(clip_norm.xy) * clip_pos.w / vec2<f32>(view.width, view.height), 0.0, 0.0);
    out.clip_position = n_pos + vec4<f32>(0.0,0.0,0.0,0.0);
    return out;
}

struct FragmentInput {
    [[builtin(front_facing)]] is_front: bool;
};

[[stage(fragment)]]
fn fragment(in: FragmentInput) -> [[location(0)]] vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
