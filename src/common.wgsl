#define_import_path bevy_mod_outline::common
#import bevy_pbr::mesh_view_bindings
#import bevy_pbr::mesh_types

@group(1) @binding(0)
var<uniform> mesh: Mesh;

#ifdef SKINNED
@group(1) @binding(1)
var<uniform> joint_matrices: SkinnedMesh;
#import bevy_pbr::skinning
#endif

fn model_origin_z(model: mat4x4<f32>, view_proj: mat4x4<f32>) -> f32 {
    var origin = model[3]; 
    var proj_zw = mat4x2<f32>(
        view_proj[0].zw, view_proj[1].zw,
        view_proj[2].zw, view_proj[3].zw);
    var zw = proj_zw * origin;
    return zw.x / zw.y;
}