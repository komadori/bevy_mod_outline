#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct JumpFloodUniform {
    step_length: u32,
    _padding: vec3<f32>
}

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var<uniform> instance: JumpFloodUniform;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<i32>(textureDimensions(screen_texture));
    let step = i32(instance.step_length);
    let pos = vec2<i32>(in.position.xy);

    var result = textureLoad(screen_texture, pos, 0).xy;
    var closest_dist = length(result);

    // Check all 8 neighbouring pixels at the current step distance.
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            if (dx == 0 && dy == 0) {
                continue;
            }

            let offset = vec2<i32>(dx * step, dy * step);
            let neighbour_coord = pos + offset;
            if (any(neighbour_coord < vec2<i32>(0)) || any(neighbour_coord >= dims)) {
                continue;
            }

            let delta = textureLoad(screen_texture, neighbour_coord, 0).xy + vec2<f32>(offset);
            let dist = length(delta);

            if (dist < closest_dist) {
                closest_dist = dist;
                result = delta;
            }
        }
    }
    return vec4<f32>(result, 0.0, 0.0);
}
