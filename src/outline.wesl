import bevy_render::maths::affine3_to_square;
import package::common::{OutlineViewUniform, VertexOutput, model_origin_z, outline_flat_depth};
@if(MORPH_TARGETS)
import bevy_pbr::render::mesh_types::{MorphAttributes, MorphDescriptor, MorphWeights};
@if(SKINNED)
import bevy_pbr::render::skinning::joint_matrices;
@if(INSTANCE_BATCH_SIZE)
import constants::INSTANCE_BATCH_SIZE;

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
    @if(!VERTEX_OFFSET_ZERO)
    @location(1) outline_normal: vec3<f32>,
    @if(ALPHA_MASK_TEXTURE)
    @location(2) uv: vec2<f32>,
    @if(SKINNED)
    @location(5) joint_indices: vec4<u32>,
    @if(SKINNED)
    @location(6) joint_weights: vec4<f32>,
    @if(MORPH_TARGETS)
    @builtin(vertex_index) index: u32,
};

@group(0) @binding(0)
var<uniform> view_uniform: OutlineViewUniform;

@if(INSTANCE_BATCH_SIZE)
@group(1) @binding(0) var<uniform> mesh: array<Instance, INSTANCE_BATCH_SIZE>;
@if(!INSTANCE_BATCH_SIZE)
@group(1) @binding(0) var<storage> mesh: array<Instance>;

@if(MORPH_TARGETS && SKINS_USE_UNIFORM_BUFFERS)
@group(2) @binding(2) var<uniform> morph_weights: MorphWeights;
@if(MORPH_TARGETS && SKINS_USE_UNIFORM_BUFFERS)
@group(2) @binding(3) var morph_targets: texture_3d<f32>;
@if(MORPH_TARGETS && !SKINS_USE_UNIFORM_BUFFERS)
@group(2) @binding(2) var<storage> morph_weights: array<f32>;
@if(MORPH_TARGETS && !SKINS_USE_UNIFORM_BUFFERS)
@group(2) @binding(3) var<storage> morph_targets: array<MorphAttributes>;
@if(MORPH_TARGETS && !SKINS_USE_UNIFORM_BUFFERS)
@group(2) @binding(8) var<storage> morph_descriptors: array<MorphDescriptor>;

@if(MORPH_TARGETS)
fn morph_layer_count(descriptor_index: u32) -> u32 {
    @if(SKINS_USE_UNIFORM_BUFFERS) {
        let dimensions = textureDimensions(morph_targets);
        return u32(dimensions.z);
    }
    @if(!SKINS_USE_UNIFORM_BUFFERS)
    return morph_descriptors[descriptor_index].weight_count;
}

@if(MORPH_TARGETS)
fn morph_weight_at(weight_index: u32, descriptor_index: u32) -> f32 {
    @if(SKINS_USE_UNIFORM_BUFFERS) {
        let i = weight_index;
        return morph_weights.weights[i / 4u][i % 4u];
    }
    @if(!SKINS_USE_UNIFORM_BUFFERS) {
        let weights_offset = morph_descriptors[descriptor_index].current_weights_offset;
        return morph_weights[weights_offset + weight_index];
    }
}

@if(MORPH_TARGETS && SKINS_USE_UNIFORM_BUFFERS)
const morph_position_offset: u32 = 0u;
@if(MORPH_TARGETS && SKINS_USE_UNIFORM_BUFFERS)
const morph_total_component_count: u32 = 9u;

@if(MORPH_TARGETS && SKINS_USE_UNIFORM_BUFFERS)
fn morph_component_texture_coord(vertex_index: u32, component_offset: u32) -> vec2<u32> {
    let width = u32(textureDimensions(morph_targets).x);
    let component_index = morph_total_component_count * vertex_index + component_offset;
    return vec2<u32>(component_index % width, component_index / width);
}

@if(MORPH_TARGETS && SKINS_USE_UNIFORM_BUFFERS)
fn morph_pixel(vertex: u32, component: u32, weight: u32) -> f32 {
    let coord = morph_component_texture_coord(vertex, component);
    return textureLoad(morph_targets, vec3(coord, weight), 0).r;
}

@if(MORPH_TARGETS && SKINS_USE_UNIFORM_BUFFERS)
fn morph(vertex_index: u32, component_offset: u32, weight_index: u32) -> vec3<f32> {
    return vec3<f32>(
        morph_pixel(vertex_index, component_offset, weight_index),
        morph_pixel(vertex_index, component_offset + 1u, weight_index),
        morph_pixel(vertex_index, component_offset + 2u, weight_index),
    );
}

@if(MORPH_TARGETS && SKINS_USE_UNIFORM_BUFFERS)
fn morph_position(vertex_index: u32, weight_index: u32, instance_index: u32) -> vec3<f32> {
    return morph(vertex_index, morph_position_offset, weight_index);
}

@if(MORPH_TARGETS && !SKINS_USE_UNIFORM_BUFFERS)
fn get_morph_target(vertex_index: u32, weight_index: u32, descriptor_index: u32) -> MorphAttributes {
    let targets_offset = morph_descriptors[descriptor_index].targets_offset;
    let vertex_count = morph_descriptors[descriptor_index].vertex_count;
    return morph_targets[targets_offset + weight_index * vertex_count + vertex_index];
}

@if(MORPH_TARGETS && !SKINS_USE_UNIFORM_BUFFERS)
fn morph_position(vertex_index: u32, weight_index: u32, descriptor_index: u32) -> vec3<f32> {
    return get_morph_target(vertex_index, weight_index, descriptor_index).position;
}

@if(MORPH_TARGETS)
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

@if(SKINNED)
fn skin_model(
    indexes: vec4<u32>,
    weights: vec4<f32>,
    instance_index: u32,
) -> mat4x4<f32> {
    @if(SKINS_USE_UNIFORM_BUFFERS)
    return weights.x * joint_matrices.data[indexes.x]
        + weights.y * joint_matrices.data[indexes.y]
        + weights.z * joint_matrices.data[indexes.z]
        + weights.w * joint_matrices.data[indexes.w];
    @if(!SKINS_USE_UNIFORM_BUFFERS) {
        var skin_index = mesh[instance_index].current_skin_index;
        return weights.x * joint_matrices[skin_index + indexes.x]
            + weights.y * joint_matrices[skin_index + indexes.y]
            + weights.z * joint_matrices[skin_index + indexes.z]
            + weights.w * joint_matrices[skin_index + indexes.w];
    }
}

fn mat4to3(m: mat4x4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(
        m[0].xyz, m[1].xyz, m[2].xyz
    );
}

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    let iid = vertex_no_morph.instance_index;
    @if(MORPH_TARGETS)
    var vertex = morph_vertex(vertex_no_morph, iid);
    @if(!MORPH_TARGETS)
    var vertex = vertex_no_morph;
    @if(SKINNED)
    let model = skin_model(vertex.joint_indices, vertex.joint_weights, iid);
    @if(!SKINNED)
    let model = affine3_to_square(mesh[iid].world_from_local);
    let clip_pos = view_uniform.clip_from_world * (model * vec4<f32>(vertex.position, 1.0));
    @if(VOLUME)
    let offset = mesh[iid].volume_offset;
    @if(!VOLUME)
    let offset = mesh[iid].stencil_offset;
    @if(VERTEX_OFFSET_ZERO)
    let out_xy = clip_pos.xy;
    @if(!VERTEX_OFFSET_ZERO)
    let clip_norm = mat4to3(view_uniform.clip_from_world) * (mat4to3(model) * vertex.outline_normal);
    @if(!VERTEX_OFFSET_ZERO)
    let corrected_norm = normalize(clip_norm.xy * vec2<f32>(view_uniform.aspect, 1.0));
    @if(!VERTEX_OFFSET_ZERO)
    let ndc_delta = offset * corrected_norm * view_uniform.scale_clip_from_logical * clip_pos.w;
    @if(!VERTEX_OFFSET_ZERO)
    let out_xy = clip_pos.xy + ndc_delta;
    var out: VertexOutput;
    out.position = vec4<f32>(out_xy, clip_pos.zw);
    @if(FLAT_DEPTH && PLANE_OFFSET_ZERO)
    out.flat_depth = model_origin_z(mesh[iid].world_plane_origin, view_uniform.clip_from_world);
    @if(FLAT_DEPTH && !PLANE_OFFSET_ZERO)
    out.flat_depth = outline_flat_depth(view_uniform, mesh[iid].world_plane_origin, mesh[iid].world_plane_offset);
    @if(VOLUME)
    out.volume_colour = mesh[iid].volume_colour;
    @if(ALPHA_MASK_TEXTURE)
    out.alpha_mask_threshold = mesh[iid].alpha_mask_threshold;
    @if(ALPHA_MASK_TEXTURE)
    out.uv = vertex.uv;
    return out;
}
