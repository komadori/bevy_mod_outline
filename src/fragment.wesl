import package::common::VertexOutput;
@if(ALPHA_MASK_TEXTURE)
import constants::ALPHA_MASK_CHANNEL;

struct FragmentOutput {
    @location(0) colour: vec4<f32>,
    @if(FLAT_DEPTH)
    @builtin(frag_depth) frag_depth: f32,
};

@if(ALPHA_MASK_TEXTURE)
@group(3) @binding(0) var alpha_mask_texture: texture_2d<f32>;
@if(ALPHA_MASK_TEXTURE)
@group(3) @binding(1) var alpha_mask_sampler: sampler;

@fragment
fn fragment(vertex: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    @if(ALPHA_MASK_TEXTURE) {
        let alpha_mask = textureSample(alpha_mask_texture, alpha_mask_sampler, vertex.uv)[ALPHA_MASK_CHANNEL];
        if (alpha_mask < vertex.alpha_mask_threshold) {
            discard;
        }
    }

    @if(FLAT_DEPTH)
    out.frag_depth = vertex.flat_depth;
    @if(VOLUME)
    out.colour = vertex.volume_colour;
    @if(FLOOD_INIT && MSAA)
    out.colour = vec4<f32>(1.0, 0.0, 0.0, 0.0);
    @if(FLOOD_INIT && !MSAA)
    out.colour = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    return out;
}
