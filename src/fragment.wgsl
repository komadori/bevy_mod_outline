#import bevy_mod_outline::common::VertexOutput

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
    return out;
}