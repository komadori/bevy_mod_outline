struct FragmentOutput {
    @location(0) colour: vec4<f32>,
#ifdef OPENGL_WORKAROUND
    @builtin(frag_depth) frag_depth: f32,
#endif
};

struct OutlineFragmentUniform {
    @align(16)
    colour: vec4<f32>,
};

#ifdef VOLUME
@group(3) @binding(1)
var<uniform> fstage: OutlineFragmentUniform;
#endif

@fragment
#ifdef OPENGL_WORKAROUND
fn fragment(@location(0) normalised_depth: f32) -> FragmentOutput {
#else
fn fragment() -> FragmentOutput {
#endif
    var out: FragmentOutput;
#ifdef VOLUME
    out.colour = fstage.colour;
#endif
#ifdef OPENGL_WORKAROUND
    out.frag_depth = normalised_depth; 
#endif
    return out;
}