#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_mod_outline::common::OutlineViewUniform

struct ComposeOutputUniform {
    flat_depth: f32,
    volume_offset: f32,
    volume_colour: vec4<f32>,
}

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> view: OutlineViewUniform;
@group(0) @binding(3) var<uniform> instance: ComposeOutputUniform;

struct FragmentOutput {
    @location(0) colour: vec4<f32>,
    @builtin(frag_depth) frag_depth: f32,
};

@fragment
fn fragment(in: FullscreenVertexOutput) -> FragmentOutput {
    let tex = textureSample(screen_texture, texture_sampler, in.uv);
    let threshold = view.scale_physical_from_logical * instance.volume_offset;
    let dist = distance(in.position.xy, tex.xy);
    var out: FragmentOutput;
#ifdef MSAA
    let inner = max(threshold - 1.0, 0.0);
    let coverage = 1.0 - smoothstep(inner, threshold, dist);
    if coverage <= 0.0 {
        discard;
    }
    out.colour = vec4<f32>(instance.volume_colour.rgb, instance.volume_colour.a * coverage);
    out.frag_depth = instance.flat_depth;
#else
    if dist <= threshold {
        out.colour = instance.volume_colour;
        out.frag_depth = instance.flat_depth;
    } else {
        discard;
    }
#endif
    return out;
}
