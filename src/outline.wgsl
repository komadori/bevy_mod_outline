#import bevy_render::view::View
#import bevy_render::maths
#import bevy_pbr::mesh_types::{SkinnedMesh, MorphAttributes, MorphDescriptor, MorphWeights}
#import bevy_pbr::skinning::joint_matrices
#import bevy_mod_outline::common::{OutlineViewUniform, VertexOutput}

struct Instance {
    world_from_local: mat3x4<f32>,
    world_plane_origin: vec3<f32>,
    world_plane_offset: vec3<f32>,
    volume_colour: vec4<f32>,
    volume_offset: f32,
    stencil_offset: f32,
    alpha_mask_threshold: f32,
    first_vertex_index: u32,
    current_skin_index: u32,
    current_morph_index: u32,
};

struct Vertex {
    @location(0) position: vec3<f32>,
    @builtin(instance_index) instance_index: u32,
#ifndef VERTEX_OFFSET_ZERO
    @location(1) outline_normal: vec3<f32>,
#endif
#ifdef ALPHA_MASK_TEXTURE
    @location(2) uv: vec2<f32>,
#endif
#ifdef SKINNED
    @location(5) joint_indices: vec4<u32>,
    @location(6) joint_weights: vec4<f32>,
#endif
#ifdef MORPH_TARGETS
    @builtin(vertex_index) index: u32,
#endif
};

@group(0) @binding(0)
var<uniform> view_uniform: OutlineViewUniform;

#import bevy_pbr::skinning

#ifdef INSTANCE_BATCH_SIZE
@group(1) @binding(0) var<uniform> mesh: array<Instance, #{INSTANCE_BATCH_SIZE}u>;
#else
@group(1) @binding(0) var<storage> mesh: array<Instance>;
#endif

#ifdef MORPH_TARGETS
#ifdef SKINS_USE_UNIFORM_BUFFERS
@group(2) @binding(2) var<uniform> morph_weights: MorphWeights;
@group(2) @binding(3) var morph_targets: texture_3d<f32>;
#else
@group(2) @binding(2) var<storage> morph_weights: array<f32>;
@group(2) @binding(3) var<storage> morph_targets: array<MorphAttributes>;
@group(2) @binding(8) var<storage> morph_descriptors: array<MorphDescriptor>;
#endif

fn morph_layer_count(descriptor_index: u32) -> u32 {
#ifdef SKINS_USE_UNIFORM_BUFFERS
    let dimensions = textureDimensions(morph_targets);
    return u32(dimensions.z);
#else
    return morph_descriptors[descriptor_index].weight_count;
#endif
}

fn morph_weight_at(weight_index: u32, descriptor_index: u32) -> f32 {
#ifdef SKINS_USE_UNIFORM_BUFFERS
    let i = weight_index;
    return morph_weights.weights[i / 4u][i % 4u];
#else
    let weights_offset = morph_descriptors[descriptor_index].current_weights_offset;
    return morph_weights[weights_offset + weight_index];
#endif
}

#ifdef SKINS_USE_UNIFORM_BUFFERS
const morph_position_offset: u32 = 0u;
const morph_total_component_count: u32 = 9u;

fn morph_component_texture_coord(vertex_index: u32, component_offset: u32) -> vec2<u32> {
    let width = u32(textureDimensions(morph_targets).x);
    let component_index = morph_total_component_count * vertex_index + component_offset;
    return vec2<u32>(component_index % width, component_index / width);
}

fn morph_pixel(vertex: u32, component: u32, weight: u32) -> f32 {
    let coord = morph_component_texture_coord(vertex, component);
    return textureLoad(morph_targets, vec3(coord, weight), 0).r;
}

fn morph(vertex_index: u32, component_offset: u32, weight_index: u32) -> vec3<f32> {
    return vec3<f32>(
        morph_pixel(vertex_index, component_offset, weight_index),
        morph_pixel(vertex_index, component_offset + 1u, weight_index),
        morph_pixel(vertex_index, component_offset + 2u, weight_index),
    );
}

fn morph_position(vertex_index: u32, weight_index: u32, instance_index: u32) -> vec3<f32> {
    return morph(vertex_index, morph_position_offset, weight_index);
}
#else
fn get_morph_target(vertex_index: u32, weight_index: u32, descriptor_index: u32) -> MorphAttributes {
    let targets_offset = morph_descriptors[descriptor_index].targets_offset;
    let vertex_count = morph_descriptors[descriptor_index].vertex_count;
    return morph_targets[targets_offset + weight_index * vertex_count + vertex_index];
}

fn morph_position(vertex_index: u32, weight_index: u32, descriptor_index: u32) -> vec3<f32> {
    return get_morph_target(vertex_index, weight_index, descriptor_index).position;
}
#endif

fn morph_vertex(vertex_in: Vertex, instance_index: u32) -> Vertex {
    var vertex = vertex_in;
    let first_vertex = mesh[instance_index].first_vertex_index;
    var morph_index = mesh[instance_index].current_morph_index;
    let vertex_index = vertex.index - first_vertex;

    let weight_count = morph_layer_count(morph_index);
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = morph_weight_at(i, morph_index);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * morph_position(vertex_index, i, morph_index);
    }
    return vertex;
}
#endif

#ifdef SKINNED
fn skin_model(
    indexes: vec4<u32>,
    weights: vec4<f32>,
    instance_index: u32,
) -> mat4x4<f32> {
#ifdef SKINS_USE_UNIFORM_BUFFERS
    return weights.x * joint_matrices.data[indexes.x]
        + weights.y * joint_matrices.data[indexes.y]
        + weights.z * joint_matrices.data[indexes.z]
        + weights.w * joint_matrices.data[indexes.w];
#else
    var skin_index = mesh[instance_index].current_skin_index;
    return weights.x * joint_matrices[skin_index + indexes.x]
        + weights.y * joint_matrices[skin_index + indexes.y]
        + weights.z * joint_matrices[skin_index + indexes.z]
        + weights.w * joint_matrices[skin_index + indexes.w];
#endif
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
    let iid = vertex_no_morph.instance_index;
#ifdef MORPH_TARGETS
    var vertex = morph_vertex(vertex_no_morph, iid);
#else
    var vertex = vertex_no_morph;
#endif
#ifdef SKINNED
    let model = skin_model(vertex.joint_indices, vertex.joint_weights, iid);
#else
    let model = bevy_render::maths::affine3_to_square(mesh[iid].world_from_local);
#endif
    let clip_pos = view_uniform.clip_from_world * (model * vec4<f32>(vertex.position, 1.0));
#ifdef VOLUME
    let offset = mesh[iid].volume_offset;
#else
    let offset = mesh[iid].stencil_offset;
#endif
#ifdef VERTEX_OFFSET_ZERO
    let out_xy = clip_pos.xy;
#else
    let clip_norm = mat4to3(view_uniform.clip_from_world) * (mat4to3(model) * vertex.outline_normal);
    let corrected_norm = normalize(clip_norm.xy * vec2<f32>(view_uniform.aspect, 1.0));
    let ndc_delta = offset * corrected_norm * view_uniform.scale_clip_from_logical * clip_pos.w;
    let out_xy = clip_pos.xy + ndc_delta;
#endif
    var out: VertexOutput;
    out.position = vec4<f32>(out_xy, clip_pos.zw);
#ifdef FLAT_DEPTH
#ifdef PLANE_OFFSET_ZERO
    let world_plane = mesh[iid].world_plane_origin;
#else
    let world_from_view = bevy_render::maths::mat2x4_f32_to_mat3x3_unpack(view_uniform.world_from_view_a, view_uniform.world_from_view_b);
    let model_eye = normalize(world_from_view * vec3<f32>(0.0, 0.0, -1.0));
    let world_plane = mesh[iid].world_plane_origin + model_eye * mesh[iid].world_plane_offset;
#endif
    out.flat_depth = model_origin_z(world_plane, view_uniform.clip_from_world);
#endif
#ifdef VOLUME
    out.volume_colour = mesh[iid].volume_colour;
#endif
#ifdef ALPHA_MASK_TEXTURE
    out.alpha_mask_threshold = mesh[iid].alpha_mask_threshold;
    out.uv = vertex.uv;
#endif
    return out;
}
