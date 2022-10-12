#import bevy_mod_outline::common

struct VertexInput {
    @location(0) position: vec3<f32>,
#ifdef SKINNED
    @location(2) joint_indexes: vec4<u32>,
    @location(3) joint_weights: vec4<f32>,
#endif
};

@vertex
fn vertex(vertex: VertexInput) -> @builtin(position) vec4<f32> {
#ifdef SKINNED
    let model = skin_model(vertex.joint_indexes, vertex.joint_weights);
#else
    let model = mesh.model;
#endif
    var clip_pos = view.view_proj * (model * vec4<f32>(vertex.position, 1.0));
    var ndc_pos = clip_pos.xy / clip_pos.w;
    return vec4<f32>(ndc_pos, model_origin_z(mesh.model, view.view_proj), 1.0);
}

@fragment
fn fragment() {
    return;
}
