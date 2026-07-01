#define_import_path bevy_mod_outline::common

#import bevy_render::maths

struct OutlineViewUniform {
    clip_from_world: mat4x4<f32>,
    world_from_view_a: mat2x4<f32>,
    world_from_view_b: f32,
    aspect: f32,
    scale_clip_from_logical: vec2<f32>,
    scale_physical_from_logical: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
#ifdef FLAT_DEPTH
    @location(0) @interpolate(flat) flat_depth: f32,
#endif
#ifdef VOLUME
    @location(1) @interpolate(flat) volume_colour: vec4<f32>,
#endif
#ifdef ALPHA_MASK_TEXTURE
    @location(2) @interpolate(flat) alpha_mask_threshold: f32,
    @location(3) uv: vec2<f32>,
#endif
};

fn model_origin_z(plane: vec3<f32>, view_proj: mat4x4<f32>) -> f32 {
    var proj_zw = mat4x2<f32>(
        view_proj[0].zw, view_proj[1].zw,
        view_proj[2].zw, view_proj[3].zw);
    var zw = proj_zw * vec4<f32>(plane, 1.0);
    return zw.x / zw.y;
}

fn outline_flat_depth(
    view: OutlineViewUniform,
    world_plane_origin: vec3<f32>,
    world_plane_offset: vec3<f32>,
) -> f32 {
    let world_from_view = bevy_render::maths::mat2x4_f32_to_mat3x3_unpack(
        view.world_from_view_a,
        view.world_from_view_b,
    );
    let model_eye = normalize(world_from_view * vec3<f32>(0.0, 0.0, -1.0));
    let world_pos = world_plane_origin + model_eye * world_plane_offset;
    return model_origin_z(world_pos, view.clip_from_world);
}
