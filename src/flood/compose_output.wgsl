#import bevy_mod_outline::common::{OutlineViewUniform, outline_flat_depth}

struct ComposeOutputUniform {
    world_plane_origin: vec3<f32>,
    world_plane_offset: vec3<f32>,
    volume_offset: f32,
    volume_colour: vec4<f32>,
}

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var<uniform> view: OutlineViewUniform;
@group(0) @binding(2) var<uniform> instance: ComposeOutputUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) flat_depth: f32,
};

struct FragmentOutput {
    @location(0) colour: vec4<f32>,
    @builtin(frag_depth) frag_depth: f32,
};

@vertex
fn vertex(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle (per bevy_core_pipeline::fullscreen_vertex_shader).
    let uv = vec2<f32>(f32(vertex_index >> 1u), f32(vertex_index & 1u)) * 2.0;
    var out: VertexOutput;
    out.position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    // Compute the flat depth from the same inputs and code as the stencil
    // pass so the two depths agree exactly.
    out.flat_depth = outline_flat_depth(view, instance.world_plane_origin, instance.world_plane_offset);
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let tex = textureLoad(screen_texture, vec2<i32>(in.position.xy), 0);
    let threshold = view.scale_physical_from_logical * instance.volume_offset;
    // The flood texture stores the delta from each pixel to its nearest seed,
    // so the distance to that seed is the length of the stored delta.
    let dist = length(tex.xy);
    var out: FragmentOutput;
#ifdef MSAA
    let inner = max(threshold - 1.0, 0.0);
    let coverage = 1.0 - smoothstep(inner, threshold, dist);
    if coverage <= 0.0 {
        discard;
    }
    out.colour = vec4<f32>(instance.volume_colour.rgb, instance.volume_colour.a * coverage);
#else
    if dist <= threshold {
        out.colour = instance.volume_colour;
    } else {
        discard;
    }
#endif
    out.frag_depth = in.flat_depth;
    return out;
}
