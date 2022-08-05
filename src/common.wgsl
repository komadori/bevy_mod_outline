#define_import_path bevy_mod_outline::common
#import bevy_pbr::mesh_view_bindings
#import bevy_pbr::mesh_types

@group(1) @binding(0)
var<uniform> mesh: Mesh;

fn model_origin_z() -> f32 {
    var origin = mesh.model[3]; 
    var proj_zw = mat4x2<f32>(
        view.view_proj[0].zw, view.view_proj[1].zw,
        view.view_proj[2].zw, view.view_proj[3].zw);
    var zw = proj_zw * origin;
    return zw.x / zw.y;
}