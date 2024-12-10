#import bevy_mod_outline::common::VertexOutput

struct Instance {
    world_from_local: mat3x4<f32>,
    world_plane_origin: vec3<f32>,
    world_plane_offset: vec3<f32>,
    volume_offset: f32,
    volume_colour: vec4<f32>,
    stencil_offset: f32,
    first_vertex_index: u32,
};

struct FragmentOutput {
    @location(0) colour: vec4<f32>,
#ifdef FLAT_DEPTH
    @builtin(frag_depth) frag_depth: f32,
#endif
};

@fragment
fn fragment(vertex: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
#ifdef FLAT_DEPTH
    out.frag_depth = vertex.flat_depth; 
#endif
#ifdef VOLUME
    out.colour = vertex.volume_colour;
#endif
#ifdef FLOOD_INIT
    out.colour = vec4<f32>(vertex.position.xy, out.frag_depth, 0.0);
#endif
    return out;
}