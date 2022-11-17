#import bevy_mod_outline::common

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
#ifdef SKINNED
    @location(2) joint_indexes: vec4<u32>,
    @location(3) joint_weights: vec4<f32>,
#endif
};

struct OutlineViewUniform {
#ifdef ALIGN_16
    @align(16)
#endif
    scale: vec2<f32>,
};

struct OutlineVertexUniform {
    @align(16)
    plane: vec3<f32>,
    width: f32,
};

struct OutlineFragmentUniform {
    @align(16)
    colour: vec4<f32>,
};

@group(2) @binding(0)
var<uniform> view_uniform: OutlineViewUniform;

@group(3) @binding(0)
var<uniform> vstage: OutlineVertexUniform;

@group(3) @binding(1)
var<uniform> fstage: OutlineFragmentUniform;

fn mat4to3(m: mat4x4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(
        m[0].xyz, m[1].xyz, m[2].xyz
    );
}

@vertex
fn vertex(vertex: VertexInput) -> @builtin(position) vec4<f32> {
#ifdef SKINNED
    let model = skin_model(vertex.joint_indexes, vertex.joint_weights);
#else
    let model = mesh.model;
#endif
    var clip_pos = view.view_proj * (model * vec4<f32>(vertex.position, 1.0));
    var clip_norm = mat4to3(view.view_proj) * (mat4to3(model) * vertex.normal);
    var ndc_pos = clip_pos.xy / clip_pos.w;
    var ndc_delta = vstage.width * normalize(clip_norm.xy) * view_uniform.scale;
    return vec4<f32>(ndc_pos + ndc_delta, model_origin_z(vstage.plane, view.view_proj), 1.0);
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return fstage.colour;
}
