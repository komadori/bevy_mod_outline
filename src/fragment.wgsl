struct FragmentOutput {
    @location(0) colour: vec4<f32>,
#ifdef FLAT_DEPTH
    @builtin(frag_depth) frag_depth: f32,
#endif
};

struct OutlineFragmentUniform {
    @align(16)
    colour: vec4<f32>,
};

#ifdef VOLUME
@group(2) @binding(1)
var<uniform> fstage: OutlineFragmentUniform;
#endif

@fragment
#ifdef FLAT_DEPTH
fn fragment(@location(0) @interpolate(flat) flat_depth: f32) -> FragmentOutput {
#else
fn fragment() -> FragmentOutput {
#endif
    var out: FragmentOutput;
#ifdef VOLUME
    out.colour = fstage.colour;
#endif
#ifdef FLAT_DEPTH
    out.frag_depth = flat_depth; 
#endif
    return out;
}