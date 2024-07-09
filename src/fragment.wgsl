struct VertexOutput {
#ifdef FLAT_DEPTH
    @location(0) @interpolate(flat) flat_depth: f32,
#endif
#ifdef VOLUME
    @location(1) @interpolate(flat) volume_colour: vec4<f32>,
#endif
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
    return out;
}