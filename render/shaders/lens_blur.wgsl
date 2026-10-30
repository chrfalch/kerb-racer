// The tilt-shift blur, at half resolution: each pixel here covers a 2×2 block
// of the frame and gathers a golden-angle spiral of full-resolution taps. The
// lens pass (lens.wgsl) blends it back over the sharp frame by each full pixel's
// own blur, so the cost of the wide blur is a quarter of the screen's.
@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var depth: texture_2d<f32>;
@group(0) @binding(2) var<uniform> lens: Blur;

// The head of lens.wgsl's `Lens`: the same buffer binds both passes.
struct Blur {
    focus: f32,
    sharp: f32,
    ramp: f32,
    radius: f32,
    size: vec2<f32>,
    vignette: f32,
    warmth: f32,
    band: f32,
    band_half: f32,
    band_ramp: f32,
    saturation: f32,
}

@vertex fn blur_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    return vec4(p, 0.0, 1.0);
}

fn texel(p: vec2<f32>) -> vec2<i32> {
    return vec2<i32>(clamp(p, vec2(0.0), lens.size - 1.0));
}

fn coc(y: f32, d: f32) -> f32 {
    let off = abs(y / lens.size.y - lens.band) - lens.band_half;
    let band = smoothstep(0.0, lens.band_ramp, off);
    var depth = 0.0;
    if (d > 0.0) {
        depth = smoothstep(lens.sharp, lens.sharp + lens.ramp, abs(d - lens.focus));
    }
    return max(band * band, depth) * lens.radius;
}

@fragment fn blur_fs(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    // The centre of this pixel's 2×2 block, in full-resolution pixels.
    let p = floor(at.xy) * 2.0 + 1.0;
    let d = textureLoad(depth, texel(p), 0).r;
    let r = coc(p.y, d);
    var sum = textureLoad(scene, texel(p), 0).rgb;
    var weight = 1.0;
    if (r > 0.5) {
        // A tap only counts as far as its own blur reaches, so a sharp road
        // never smears into a blurred crown.
        let taps = 16;
        for (var k = 1; k <= taps; k++) {
            let f = f32(k) / f32(taps);
            let a = f32(k) * 2.39996;
            let o = vec2(cos(a), sin(a)) * sqrt(f) * r;
            let q = texel(p + o);
            let dq = textureLoad(depth, q, 0).r;
            let w = smoothstep(0.0, 1.0, coc(p.y + o.y, dq) - length(o) + 1.0);
            sum += textureLoad(scene, q, 0).rgb * w;
            weight += w;
        }
    }
    return vec4(sum / weight, 1.0);
}
