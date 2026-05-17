#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var coverage_texture: texture_2d<f32>;

fn sample(p: vec2<i32>) -> f32 {
    return textureLoad(coverage_texture, p, 0).r;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let pos = vec2<i32>(in.position.xy);
    let coverage = sample(pos);

    if coverage <= 0.0 {
        discard;
    }

    // 3x3 neighbourhood coverage values for a Sobel gradient.
    let tl = sample(pos + vec2<i32>(-1, -1));
    let tc = sample(pos + vec2<i32>( 0, -1));
    let tr = sample(pos + vec2<i32>( 1, -1));
    let ml = sample(pos + vec2<i32>(-1,  0));
    let mr = sample(pos + vec2<i32>( 1,  0));
    let bl = sample(pos + vec2<i32>(-1,  1));
    let bc = sample(pos + vec2<i32>( 0,  1));
    let br = sample(pos + vec2<i32>( 1,  1));

    let gx = (tr + 2.0 * mr + br) - (tl + 2.0 * ml + bl);
    let gy = (bl + 2.0 * bc + br) - (tl + 2.0 * tc + tr);

    let g = vec2<f32>(gx, gy);
    let len = length(g);

    // Sobel `g` points from low coverage to high coverage (into the silhouette),
    // so `-g/len` is the outward edge normal.
    var seed = in.position.xy;
    if len > 1e-4 {
        let outward = -g / len;
        seed = seed + outward * (coverage - 0.5);
    }

    return vec4<f32>(seed, 0.0, 0.0);
}
