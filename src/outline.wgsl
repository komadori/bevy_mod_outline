#import bevy_mod_outline::common

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

struct OutlineViewUniform {
    scale: vec2<f32>,
};

struct OutlineVertexUniform {
    width: f32,
};

struct OutlineFragmentUniform {
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
fn vertex(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    var clip_pos = view.view_proj * (mesh.model * vec4<f32>(vertex.position, 1.0));
    var clip_norm = mat4to3(view.view_proj) * (mat4to3(mesh.model) * vertex.normal);
    var clip_delta = vec2<f32>(vstage.width * normalize(clip_norm.xy) * clip_pos.w * view_uniform.scale);
    out.clip_position = vec4<f32>((clip_pos.xy + clip_delta) / clip_pos.w, model_origin_z(), 1.0);
    return out;
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return fstage.colour;
}
