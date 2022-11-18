#ifdef VOLUME

struct OutlineFragmentUniform {
    @align(16)
    colour: vec4<f32>,
};

@group(3) @binding(1)
var<uniform> fstage: OutlineFragmentUniform;

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return fstage.colour;
}

#else
// Stencil

@fragment
fn fragment() {
    return;
}

#endif