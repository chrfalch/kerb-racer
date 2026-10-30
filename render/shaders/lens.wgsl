// The miniature look: a tilt-shift lens. A sharp horizontal band follows the
// player's car across the screen and everything above and below it melts away,
// as a real tilted lens does over a model railway; depth adds to it, so the
// canopy rising toward the camera blurs even inside the band. Then a toy-like
// grade and a soft vignette. Linear HDR in, linear HDR out, before
// the engine's bloom and tone curve.
@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var depth: texture_2d<f32>;
@group(0) @binding(2) var<uniform> lens: Lens;

struct Lens {
    // View-space distance in focus, in metres.
    focus: f32,
    // Half the depth range that stays sharp, then the range over which blur grows.
    sharp: f32,
    ramp: f32,
    // Largest blur radius, in pixels.
    radius: f32,
    // Active pixel rectangle.
    size: vec2<f32>,
    // Vignette strength and warmth of the grade.
    vignette: f32,
    warmth: f32,
    // The sharp band: its centre (0 top, 1 bottom), half height, and the
    // distance beyond it over which the blur reaches full strength.
    band: f32,
    band_half: f32,
    band_ramp: f32,
    // Saturation of the grade: above 1 the colours read as painted toys.
    saturation: f32,
}

@vertex fn lens_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    return vec4(p, 0.0, 1.0);
}

fn texel(p: vec2<f32>) -> vec2<i32> {
    return vec2<i32>(clamp(p, vec2(0.0), lens.size - 1.0));
}

// Blur radius in pixels at a screen row for a view depth: the larger of the
// tilt-shift band's and the depth of field's. Sky (zero depth) takes the band's.
fn coc(y: f32, d: f32) -> f32 {
    let off = abs(y / lens.size.y - lens.band) - lens.band_half;
    let band = smoothstep(0.0, lens.band_ramp, off);
    var depth = 0.0;
    if (d > 0.0) {
        depth = smoothstep(lens.sharp, lens.sharp + lens.ramp, abs(d - lens.focus));
    }
    return max(band * band, depth) * lens.radius;
}

@fragment fn lens_fs(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let p = at.xy;
    let d = textureLoad(depth, texel(p), 0).r;
    let r = coc(p.y, d);
    var color = textureLoad(scene, texel(p), 0).rgb;
    if (r > 0.5) {
        // A golden-angle spiral of taps. A tap only counts as far as its own
        // blur reaches, so a sharp road never smears into a blurred crown.
        var sum = color;
        var weight = 1.0;
        let taps = 20;
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
        color = sum / weight;
    }
    // A softer palette: ease saturation and contrast toward a mid grey, so the
    // greens and paints sit together instead of shouting.
    let grey = dot(color, vec3(0.2126, 0.7152, 0.0722));
    color = mix(vec3(grey), color, lens.saturation);
    let pivot = 0.18;
    color = pivot * pow(max(color, vec3(0.0)) / pivot, vec3(0.86));
    // Warm the light and cool the shade a touch, then darken the corners.
    let luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
    let lift = smoothstep(0.0, 0.6, luma);
    color *= mix(vec3(0.96, 0.99, 1.04), vec3(1.0 + lens.warmth, 1.0 + lens.warmth * 0.4, 1.0 - lens.warmth), lift);
    let uv = p / lens.size * 2.0 - 1.0;
    let edge = dot(uv * vec2(0.8, 1.0), uv * vec2(0.8, 1.0));
    color *= 1.0 - lens.vignette * smoothstep(0.35, 1.6, edge);
    return vec4(color, 1.0);
}
