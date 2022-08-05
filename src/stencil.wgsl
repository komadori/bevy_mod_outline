#import bevy_mod_outline::common

struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vertex(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    var clip_pos = view.view_proj * (mesh.model * vec4<f32>(vertex.position, 1.0));
    out.clip_position = vec4<f32>(clip_pos.xy / clip_pos.w, model_origin_z(), 1.0);
    return out;
}

@fragment
fn fragment() {
    return;
}
