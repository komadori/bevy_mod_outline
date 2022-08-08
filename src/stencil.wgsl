#import bevy_mod_outline::common

struct VertexInput {
    @location(0) position: vec3<f32>,
};

@vertex
fn vertex(vertex: VertexInput) -> @builtin(position) vec4<f32> {
    var clip_pos = view.view_proj * (mesh.model * vec4<f32>(vertex.position, 1.0));
    var ndc_pos = clip_pos.xy / clip_pos.w;
    return vec4<f32>(ndc_pos, model_origin_z(mesh.model, view.view_proj), 1.0);
}

@fragment
fn fragment() {
    return;
}
