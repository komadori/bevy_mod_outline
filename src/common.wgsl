#define_import_path bevy_mod_outline::common

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
#ifdef FLAT_DEPTH
    @location(0) @interpolate(flat) flat_depth: f32,
#endif
#ifdef VOLUME
    @location(1) @interpolate(flat) volume_colour: vec4<f32>,
#endif
};