#import bevy_render::view View
#import bevy_render::maths
#import bevy_pbr::mesh_types Mesh
#import bevy_pbr::mesh_types SkinnedMesh
#import bevy_pbr::mesh_functions

struct Vertex {
#ifdef VERTEX_POSITIONS
    @location(0) position: vec3<f32>,
    @builtin(instance_index) instance_index: u32,
#endif
#ifndef OFFSET_ZERO
    @location(1) outline_normal: vec3<f32>,
#endif
#ifdef SKINNED
    @location(5) joint_indices: vec4<u32>,
    @location(6) joint_weights: vec4<f32>,
#endif
#ifdef MORPH_TARGETS
    @builtin(vertex_index) index: u32,
#endif
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
#ifdef FLAT_DEPTH
    @location(0) @interpolate(flat) flat_depth: f32,
#endif
};

struct OutlineViewUniform {
    @align(16)
    scale: vec2<f32>,
};

struct OutlineVertexUniform {
    @align(16)
    origin: vec3<f32>,
    offset: f32,
};

@group(0) @binding(0)
var<uniform> view: View;

#import bevy_pbr::mesh_bindings
#import bevy_pbr::skinning
#import bevy_pbr::morph

@group(2) @binding(0)
var<uniform> view_uniform: OutlineViewUniform;

@group(3) @binding(0)
var<uniform> vstage: OutlineVertexUniform;

#ifdef MORPH_TARGETS
fn morph_vertex(vertex_in: Vertex) -> Vertex {
    var vertex = vertex_in;
    let weight_count = bevy_pbr::morph::layer_count();
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = bevy_pbr::morph::weight_at(i);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * bevy_pbr::morph::morph(vertex.index, bevy_pbr::morph::position_offset, i);
    }
    return vertex;
}
#endif

fn mat4to3(m: mat4x4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(
        m[0].xyz, m[1].xyz, m[2].xyz
    );
}

fn model_origin_z(plane: vec3<f32>, view_proj: mat4x4<f32>) -> f32 {
    var proj_zw = mat4x2<f32>(
        view_proj[0].zw, view_proj[1].zw,
        view_proj[2].zw, view_proj[3].zw);
    var zw = proj_zw * vec4<f32>(plane, 1.0);
    return zw.x / zw.y;
}

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
#ifdef MORPH_TARGETS
    var vertex = morph_vertex(vertex_no_morph);
#else
    var vertex = vertex_no_morph;
#endif
#ifdef SKINNED
    let model = bevy_pbr::skinning::skin_model(vertex.joint_indices, vertex.joint_weights);
#else
    let model = bevy_pbr::mesh_functions::get_model_matrix(vertex_no_morph.instance_index);
#endif
    let clip_pos = view.view_proj * (model * vec4<f32>(vertex.position, 1.0));
#ifdef OFFSET_ZERO
    let out_xy = clip_pos.xy;
#else
    let clip_norm = mat4to3(view.view_proj) * (mat4to3(model) * vertex.outline_normal);
    let ndc_delta = vstage.offset * normalize(clip_norm.xy) * view_uniform.scale * clip_pos.w;
    let out_xy = clip_pos.xy + ndc_delta;
#endif
    var out: VertexOutput;
    out.position = vec4<f32>(out_xy, clip_pos.zw);
#ifdef FLAT_DEPTH
    out.flat_depth = model_origin_z(vstage.origin, view.view_proj);
#endif
    return out;
}
