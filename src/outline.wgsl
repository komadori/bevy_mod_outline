#import bevy_render::view  View
#import bevy_pbr::mesh_types Mesh
#import bevy_pbr::mesh_types SkinnedMesh

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
#ifdef VERTEX_NORMALS
        vertex.normal += weight * bevy_pbr::morph::morph(vertex.index, bevy_pbr::morph::normal_offset, i);
#endif
#ifdef VERTEX_TANGENTS
        vertex.tangent += vec4(weight * bevy_pbr::morph::morph(vertex.index, bevy_pbr::morph::tangent_offset, i), 0.0);
#endif
    }
    return vertex;
}
#endif

struct Vertex {
#ifdef VERTEX_POSITIONS
    @location(0) position: vec3<f32>,
#endif
#ifndef OFFSET_ZERO
    @location(1) outline_normal: vec3<f32>,
#endif
#ifdef VERTEX_NORMALS
    @location(2) normal: vec3<f32>,
#endif
#ifdef VERTEX_TANGENTS
    @location(3) tangent: vec4<f32>,
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
#ifdef OPENGL_WORKAROUND
    @location(0) normalised_depth: f32,
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

@group(1) @binding(0)
var<uniform> mesh: Mesh;

#import bevy_pbr::skinning
#import bevy_pbr::morph

@group(2) @binding(0)
var<uniform> view_uniform: OutlineViewUniform;

@group(3) @binding(0)
var<uniform> vstage: OutlineVertexUniform;

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
    let model = mesh.model;
#endif
    let clip_pos = view.view_proj * (model * vec4<f32>(vertex.position, 1.0));
#ifdef FLAT_DEPTH
    let out_z = model_origin_z(vstage.origin, view.view_proj) * clip_pos.w;
#else
    let out_z = clip_pos.z;
#endif
#ifdef OFFSET_ZERO
    let out_xy = clip_pos.xy;
#else
    let clip_norm = mat4to3(view.view_proj) * (mat4to3(model) * vertex.outline_normal);
    let ndc_delta = vstage.offset * normalize(clip_norm.xy) * view_uniform.scale * clip_pos.w;
    let out_xy = clip_pos.xy + ndc_delta;
#endif
    var out: VertexOutput;
    out.position = vec4<f32>(out_xy, out_z, clip_pos.w);
#ifdef OPENGL_WORKAROUND
    out.normalised_depth = 0.5 + 0.5 * (out_z / clip_pos.w);
#endif
    return out;
}
