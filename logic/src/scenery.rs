//! Generated geometry: the textured ground and road with its kerbs and lines,
//! flowers, the player's marker, and where the Blender models stand.
use crate::track::{Track, CELL, CELLS, HALF, RUNOFF, SINK, WALL};
use exact_game::asset::{AlphaMode, MaterialData, MeshData, Model, Node};
use exact_game::*;

/// sRGB hex to linear RGBA.
pub fn hex(rgb: u32) -> [f32; 4] {
    let c = |shift: u32| {
        let v = ((rgb >> shift) & 0xff) as f32 / 255.0;
        math::powf(v, 2.2)
    };
    [c(16), c(8), c(0), 1.0]
}

fn shade(c: [f32; 4], k: f32) -> [f32; 4] {
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}

/// A cheap integer hash for colour variation; the same on every host.
fn noise(a: i32, b: i32, salt: u32) -> f32 {
    let mut h = (a as u32).wrapping_mul(0x9e37_79b1)
        ^ (b as u32).wrapping_mul(0x85eb_ca77)
        ^ salt.wrapping_mul(0xc2b2_ae3d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65535.0
}

#[derive(Default)]
pub struct Builder {
    pub m: MeshData,
    lo: Vec3,
    hi: Vec3,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            m: MeshData::default(),
            lo: Vec3::splat(f32::MAX),
            hi: Vec3::splat(f32::MIN),
        }
    }
    pub fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, color: [f32; 4]) {
        let mut n = (b - a).cross(c - a);
        // Every face here is seen from above or outside: keep normals up/out.
        let (a, c) = if n.y < -1e-6 {
            n = -n;
            (c, a)
        } else {
            (a, c)
        };
        let n = n.normalize_or_zero();
        let base = (self.m.positions.len() / 3) as u32;
        for p in [a, b, c] {
            self.m.positions.extend([p.x, p.y, p.z]);
            self.m.normals.extend([n.x, n.y, n.z]);
            self.m.uvs.extend([0.0, 0.0]);
            self.m.colors.extend(color);
            self.lo = self.lo.min(p);
            self.hi = self.hi.max(p);
        }
        self.m.indices.extend([base, base + 1, base + 2]);
    }
    /// A face with an explicit outward direction (for vertical walls).
    pub fn face(&mut self, q: [Vec3; 4], out: Vec3, color: [f32; 4]) {
        let n = (q[1] - q[0]).cross(q[2] - q[0]);
        let q = if n.dot(out) < 0.0 {
            [q[0], q[3], q[2], q[1]]
        } else {
            q
        };
        let n = out.normalize_or_zero();
        let base = (self.m.positions.len() / 3) as u32;
        for p in q {
            self.m.positions.extend([p.x, p.y, p.z]);
            self.m.normals.extend([n.x, n.y, n.z]);
            self.m.uvs.extend([0.0, 0.0]);
            self.m.colors.extend(color);
            self.lo = self.lo.min(p);
            self.hi = self.hi.max(p);
        }
        self.m
            .indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    /// An upward quad with texture coordinates at its corners.
    pub fn quad_uv(&mut self, q: [Vec3; 4], uv: [Vec2; 4], color: [f32; 4]) {
        self.quad_uvc(q, uv, [color; 4]);
    }
    /// An upward quad with texture coordinates and a colour at each corner.
    pub fn quad_uvc(&mut self, q: [Vec3; 4], uv: [Vec2; 4], colors: [[f32; 4]; 4]) {
        // The face's own normal, so slopes catch (or turn from) the low sun.
        let mut n = (q[2] - q[0]).cross(q[3] - q[1]);
        if n.y < 0.0 {
            n = -n;
        }
        let n = n.try_normalize().unwrap_or(Vec3::Y);
        self.quad_uvcn(q, uv, colors, [n; 4]);
    }
    /// An upward quad with texture coordinates, and a colour and a normal at each corner.
    pub fn quad_uvcn(&mut self, q: [Vec3; 4], uv: [Vec2; 4], colors: [[f32; 4]; 4], normals: [Vec3; 4]) {
        let up = (q[1] - q[0]).cross(q[2] - q[0]).y >= 0.0;
        let order = if up {
            [0, 1, 2, 0, 2, 3]
        } else {
            [0, 2, 1, 0, 3, 2]
        };
        let base = (self.m.positions.len() / 3) as u32;
        for k in 0..4 {
            let p = q[k];
            let n = normals[k];
            self.m.positions.extend([p.x, p.y, p.z]);
            self.m.normals.extend([n.x, n.y, n.z]);
            self.m.uvs.extend([uv[k].x, uv[k].y]);
            self.m.colors.extend(colors[k]);
            self.lo = self.lo.min(p);
            self.hi = self.hi.max(p);
        }
        self.m.indices.extend(order.map(|k| base + k));
    }
    /// An upward strip of quads between two rows of points that share their
    /// corners: each row is (one side, the other side, the normal there).
    pub fn strip(&mut self, rows: &[(Vec3, Vec3, Vec3)], color: [f32; 4]) {
        let base = (self.m.positions.len() / 3) as u32;
        for &(a, b, n) in rows {
            for p in [a, b] {
                self.m.positions.extend([p.x, p.y, p.z]);
                self.m.normals.extend([n.x, n.y, n.z]);
                self.m.uvs.extend([0.0, 0.0]);
                self.m.colors.extend(color);
                self.lo = self.lo.min(p);
                self.hi = self.hi.max(p);
            }
        }
        for (k, w) in rows.windows(2).enumerate() {
            let [a0, b0, a1, b1] = [0, 1, 2, 3].map(|c| base + 2 * k as u32 + c);
            let up = (w[0].1 - w[0].0).cross(w[1].1 - w[0].0).y >= 0.0;
            let order = if up { [a0, b0, b1, a0, b1, a1] } else { [a0, b1, b0, a0, a1, b1] };
            self.m.indices.extend(order);
        }
    }
    pub fn quad_up(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, color: [f32; 4]) {
        self.tri(a, b, c, color);
        self.tri(a, c, d, color);
    }
    /// A triangle facing away from `inside`.
    fn face_any(&mut self, a: Vec3, b: Vec3, c: Vec3, inside: Vec3, color: [f32; 4]) {
        let n = (b - a).cross(c - a);
        let (b, c) = if n.dot(a - inside) < 0.0 {
            (c, b)
        } else {
            (b, c)
        };
        let n = (b - a).cross(c - a).normalize_or_zero();
        let base = (self.m.positions.len() / 3) as u32;
        for p in [a, b, c] {
            self.m.positions.extend([p.x, p.y, p.z]);
            self.m.normals.extend([n.x, n.y, n.z]);
            self.m.uvs.extend([0.0, 0.0]);
            self.m.colors.extend(color);
            self.lo = self.lo.min(p);
            self.hi = self.hi.max(p);
        }
        self.m.indices.extend([base, base + 1, base + 2]);
    }
    /// A square frustum around Y, turned to face the diagonals.
    pub fn pyramid(&mut self, base: Vec3, r0: f32, h: f32, r1: f32, color: [f32; 4]) {
        let (sides, twist) = (4, std::f32::consts::FRAC_PI_4);
        let ring = |k: u32, r: f32, y: f32| {
            let (s, c) = math::sin_cos(twist + k as f32 / sides as f32 * std::f32::consts::TAU);
            base + Vec3::new(c * r, y, s * r)
        };
        let axis = base + Vec3::Y * (h * 0.4);
        for k in 0..sides {
            let col = shade(color, 0.85 + 0.2 * (k % 2) as f32);
            let (a, b) = (ring(k, r0, 0.0), ring(k + 1, r0, 0.0));
            let (c, d) = (ring(k, r1, h), ring(k + 1, r1, h));
            self.face_any(a, b, c, axis, col);
            if r1 > 0.0 {
                self.face_any(b, d, c, axis, col);
            }
        }
    }
    pub fn finish(mut self) -> MeshData {
        self.m.bounds = [
            self.lo.x, self.lo.y, self.lo.z, self.hi.x, self.hi.y, self.hi.z,
        ];
        self.m
    }
}

fn v3(p: Vec2, y: f32) -> Vec3 {
    Vec3::new(p.x, y, p.y)
}

/// Give a ground-hugging mesh the ground's own normals. Its quads' faces
/// would do on open road, but on the inside of a hairpin the strips fold over
/// each other and their faces point anywhere; the ground under them doesn't.
fn drape(m: &mut MeshData, t: &Track) {
    let mut hint = None;
    for (k, p) in m.positions.chunks(3).enumerate() {
        let at = Vec2::new(p[0], p[2]);
        let (i, _) = match hint {
            Some(h) => t.locate(at, h, 30),
            None => t.locate_all(at),
        };
        hint = Some(i);
        let g = t.gradient(at, i);
        let n = Vec3::new(-g.x, 1.0, -g.y).normalize();
        m.normals[k * 3..k * 3 + 3].copy_from_slice(&[n.x, n.y, n.z]);
        let shade = hillshade(n);
        for c in &mut m.colors[k * 4..k * 4 + 3] {
            *c *= shade;
        }
    }
}

/// Toward the low western sun (the "sun" light in `light`).
const SUN: Vec3 = Vec3::new(-0.888, 0.304, -0.355);

/// From straight above a hill hardly shows, so paint the light on: ground
/// tilted toward the sun a touch brighter, away from it darker, on top of
/// what the lighting already does.
fn hillshade(n: Vec3) -> f32 {
    (1.0 + 1.6 * (n.dot(SUN) - SUN.y)).clamp(0.55, 1.35)
}

/// The point `lat` metres left of sample `i`, `y` above the ground there.
fn on(t: &Track, i: usize, lat: f32, y: f32) -> Vec3 {
    v3(t.at(i, lat), t.surface_at(i, lat) + y)
}

/// A ribbon between two lateral offsets along the whole loop, coloured per segment.
fn ribbon(
    b: &mut Builder,
    t: &Track,
    from: f32,
    to: f32,
    y: f32,
    color: impl Fn(usize) -> Option<[f32; 4]>,
) {
    let n = t.len();
    for i in 0..n {
        let j = (i + 1) % n;
        if let Some(c) = color(i) {
            b.quad_up(
                on(t, i, from, y),
                on(t, i, to, y),
                on(t, j, to, y),
                on(t, j, from, y),
                c,
            );
        }
    }
}

/// A textured ribbon: `u` across the road, `v` along it, one tile per `tile` metres.
fn textured(
    b: &mut Builder,
    t: &Track,
    from: f32,
    to: f32,
    y: f32,
    tile: f32,
    tint: impl Fn(usize) -> [f32; 4],
) {
    // Cut across every couple of metres, so a wide one follows a banked or
    // crowned road and the land on the inside of a hairpin.
    let strips = (((to - from).abs() / 2.0).ceil() as usize).max(1);
    textured_strips(b, t, from, to, strips, y, tile, |i, _| tint(i));
}

/// A textured ribbon cut into `strips` across, tinted at every corner by
/// `tint(sample, lateral offset)`: shading that varies across the road.
#[allow(clippy::too_many_arguments)]
fn textured_strips(
    b: &mut Builder,
    t: &Track,
    from: f32,
    to: f32,
    strips: usize,
    y: f32,
    tile: f32,
    tint: impl Fn(usize, f32) -> [f32; 4],
) {
    let n = t.len();
    // The loop's length in whole tiles, so the texture closes without a seam.
    let tiles = (t.length() / tile).round().max(1.0);
    let along = |i: usize| i as f32 / n as f32 * tiles;
    for i in 0..n {
        let j = (i + 1) % n;
        let (v0, v1) = (along(i), along(i) + tiles / n as f32);
        for k in 0..strips {
            let a = from + (to - from) * k as f32 / strips as f32;
            let c = from + (to - from) * (k + 1) as f32 / strips as f32;
            b.quad_uvc(
                [
                    on(t, i, a, y),
                    on(t, i, c, y),
                    on(t, j, c, y),
                    on(t, j, a, y),
                ],
                [
                    Vec2::new(a / tile, v0),
                    Vec2::new(c / tile, v0),
                    Vec2::new(c / tile, v1),
                    Vec2::new(a / tile, v1),
                ],
                [tint(i, a), tint(i, c), tint(j, c), tint(j, a)],
            );
        }
    }
}

fn textured_material(texture: u32, roughness: f32) -> MaterialData {
    MaterialData {
        metallic: 0.0,
        roughness,
        base_color_texture: Some(texture),
        ..MaterialData::default()
    }
}

/// Several meshes, one node each, as one model.
fn model(meshes: Vec<MeshData>, materials: Vec<MaterialData>, textures: &[&str]) -> Model {
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for m in &meshes {
        lo = lo.min(Vec3::new(m.bounds[0], m.bounds[1], m.bounds[2]));
        hi = hi.max(Vec3::new(m.bounds[3], m.bounds[4], m.bounds[5]));
    }
    let nodes = (0..meshes.len())
        .map(|k| Node {
            name: format!("part-{k}"),
            mesh: Some(k as u32),
            ..Node::default()
        })
        .collect();
    Model {
        meshes,
        materials,
        textures: textures.iter().map(|s| s.to_string()).collect(),
        nodes,
        bounds: [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z],
        ..Model::default()
    }
}

/// A textured ribbon whose edges move per sample: `edge(i)` gives the
/// lateral offsets and heights of its two sides. `u` runs across, `v` along.
fn textured_edges(
    b: &mut Builder,
    t: &Track,
    tile: f32,
    edge: impl Fn(usize) -> ((f32, f32), (f32, f32)),
    tint: impl Fn(usize) -> [f32; 4],
) {
    let n = t.len();
    let tiles = (t.length() / tile).round().max(1.0);
    let along = |i: usize| i as f32 / n as f32 * tiles;
    for i in 0..n {
        let j = (i + 1) % n;
        let (v0, v1) = (along(i), along(i) + tiles / n as f32);
        let ((a0, ha0), (c0, hc0)) = edge(i);
        let ((a1, ha1), (c1, hc1)) = edge(j);
        // Cut across every couple of metres, so it follows the ground.
        let strips = (((c0 - a0).abs().max((c1 - a1).abs()) / 2.0).ceil() as usize).max(1);
        let mix = |a: f32, c: f32, k: usize| a + (c - a) * k as f32 / strips as f32;
        for k in 0..strips {
            let (l0, r0) = (mix(a0, c0, k), mix(a0, c0, k + 1));
            let (l1, r1) = (mix(a1, c1, k), mix(a1, c1, k + 1));
            let (hl0, hr0) = (mix(ha0, hc0, k), mix(ha0, hc0, k + 1));
            let (hl1, hr1) = (mix(ha1, hc1, k), mix(ha1, hc1, k + 1));
            b.quad_uvc(
                [
                    on(t, i, l0, hl0),
                    on(t, i, r0, hr0),
                    on(t, j, r1, hr1),
                    on(t, j, l1, hl1),
                ],
                [
                    Vec2::new(l0 / tile, v0),
                    Vec2::new(r0 / tile, v0),
                    Vec2::new(r1 / tile, v1),
                    Vec2::new(l1 / tile, v1),
                ],
                [tint(i), tint(i), tint(j), tint(j)],
            );
        }
    }
}

/// A smooth wobble along the loop, -1..1, closing without a seam.
fn wobble(t: &Track, i: usize, every: f32, salt: u32) -> f32 {
    let n = t.len();
    let cells = ((t.length() / every).round() as usize).max(1);
    let x = i as f32 / n as f32 * cells as f32;
    let k = x as usize;
    let f = smooth(0.0, 1.0, x - k as f32);
    let a = noise((k % cells) as i32, 0, salt);
    let b = noise(((k + 1) % cells) as i32, 0, salt);
    (a + (b - a) * f) * 2.0 - 1.0
}

/// The dirt road: loose gravel either side, packed dirt with ragged edges,
/// darker wheel ruts worn along the racing line, gravel thrown up in banks round
/// the outside of the corners, and a chequered line at the start. No paint, no kerbs.
fn dirt_road(t: &Track) -> Model {
    let n = t.len();
    let mut gravel = Builder::new();
    // Warm, sandy gravel, its outer edge scattered unevenly into the grass.
    let warm = |k: f32| [1.12 * k, 1.0 * k, 0.8 * k, 1.0];
    textured_edges(
        &mut gravel,
        t,
        2.5,
        |i| {
            let out = |s: f32| HALF + RUNOFF + 0.45 * wobble(t, i, 2.5, if s > 0.0 { 32 } else { 31 });
            ((-out(-1.0), 0.02), (out(1.0), 0.02))
        },
        |i| warm(0.95 + 0.12 * noise(i as i32 / 3, 0, 31)),
    );
    // Gravel banks round the outside of every real corner, swept up by the cars.
    let corner = |i: usize| -> f32 {
        let b = (0..9).map(|k| t.bend[(i + n + k - 4) % n]).sum::<f32>() / 9.0;
        smooth(1.0 / 40.0, 1.0 / 18.0, b.abs()).copysign(b)
    };
    for s in [-1.0f32, 1.0] {
        textured_edges(
            &mut gravel,
            t,
            2.5,
            |i| {
                // Outside of a left-hander is the right (negative) side.
                let k = (-corner(i) * s).max(0.0);
                let inner = HALF - 0.6 * k;
                let outer = HALF + RUNOFF * (0.4 + 0.6 * k);
                ((s * inner, 0.035), (s * outer, 0.035 + 0.14 * k))
            },
            |i| warm(1.05 + 0.08 * noise(i as i32 / 2, 1, 33)),
        );
    }
    let mut dirt = Builder::new();
    // Ragged edges: the dirt wanders a little into the gravel and back.
    let edge = |i: usize, s: f32| s * (HALF - 0.1 + 0.35 * wobble(t, i, 3.0, if s > 0.0 { 42 } else { 41 }));
    let strips = 12;
    let tiles = (t.length() / 5.0).round().max(1.0);
    for i in 0..n {
        let j = (i + 1) % n;
        let (v0, v1) = (
            i as f32 / n as f32 * tiles,
            (i as f32 + 1.0) / n as f32 * tiles,
        );
        let tint = |i: usize, lat: f32| {
            // Two dark ruts where the wheels run, packed and damp; loose and
            // pale out at the edges where only the gravel goes.
            let d = lat - t.line[i];
            let rut = |c: f32| {
                let x = (d - c) / 0.45;
                math::exp(-x * x)
            };
            let worn = 0.2 * (rut(-0.8) + rut(0.8)) + 0.08 * math::exp(-(d / 2.2) * (d / 2.2));
            let loose = 0.1 * ((lat.abs() - (HALF - 1.4)) / 1.2).clamp(0.0, 1.0);
            let patch = 0.06 * wobble(t, i, 7.0, 43);
            shade([1.0; 4], (1.0 - worn) * (1.0 + patch) + loose)
        };
        for k in 0..strips {
            let lat = |i: usize, k: usize| {
                let (l, r) = (edge(i, -1.0), edge(i, 1.0));
                l + (r - l) * k as f32 / strips as f32
            };
            let (a0, c0, a1, c1) = (lat(i, k), lat(i, k + 1), lat(j, k), lat(j, k + 1));
            b_quad(&mut dirt, t, i, j, [a0, c0, c1, a1], 0.04, 5.0, (v0, v1), [
                tint(i, a0),
                tint(i, c0),
                tint(j, c1),
                tint(j, a1),
            ]);
        }
    }
    let mut verge = Builder::new();
    for side in [-1.0f32, 1.0] {
        textured(
            &mut verge,
            t,
            side * (HALF + RUNOFF - 0.05),
            side * (HALF + RUNOFF + 1.5),
            0.015,
            3.0,
            |_| [0.92, 0.9, 0.86, 1.0],
        );
    }
    let mut paint = Builder::new();
    start_line(&mut paint, t);
    let mut dirt = dirt.finish();
    drape(&mut dirt, t);
    dirt.material = 1;
    let mut gravel = gravel.finish();
    drape(&mut gravel, t);
    gravel.material = 0;
    let mut paint = paint.finish();
    paint.material = 2;
    let mut verge = verge.finish();
    drape(&mut verge, t);
    verge.material = 3;
    let mut marks = ruts(t);
    drape(&mut marks, t);
    marks.material = 4;
    model(
        vec![gravel, dirt, paint, verge, marks],
        vec![
            textured_material(0, 0.97),
            textured_material(1, 0.95),
            MaterialData {
                metallic: 0.0,
                roughness: 0.8,
                ..MaterialData::default()
            },
            textured_material(2, 0.95),
            MaterialData {
                metallic: 0.0,
                roughness: 0.95,
                alpha_mode: AlphaMode::Blend,
                ..MaterialData::default()
            },
        ],
        &["gravel.tex", "dirt.tex", "scree.tex"],
    )
}

/// One textured quad between samples `i` and `j` at lateral offsets
/// `lat` (i-left, i-right, j-right, j-left).
#[allow(clippy::too_many_arguments)]
fn b_quad(
    b: &mut Builder,
    t: &Track,
    i: usize,
    j: usize,
    lat: [f32; 4],
    y: f32,
    tile: f32,
    (v0, v1): (f32, f32),
    colors: [[f32; 4]; 4],
) {
    b.quad_uvc(
        [
            on(t, i, lat[0], y),
            on(t, i, lat[1], y),
            on(t, j, lat[2], y),
            on(t, j, lat[3], y),
        ],
        [
            Vec2::new(lat[0] / tile, v0),
            Vec2::new(lat[1] / tile, v0),
            Vec2::new(lat[2] / tile, v1),
            Vec2::new(lat[3] / tile, v1),
        ],
        colors,
    );
}

/// Grooves and slide marks in the dirt: darker tracks along the racing line
/// all the way round, and wide sweeping arcs through every slow corner where
/// the cars go sideways. Vertex alpha carries the fade.
fn ruts(t: &Track) -> MeshData {
    let mut b = Builder::new();
    let n = t.len();
    let brown = |a: f32| [0.16, 0.1, 0.05, a];
    // Grooves: a few cars' lines, each wandering a little about the racing line.
    for set in 0..3u32 {
        let shift = (set as f32 - 1.0) * 0.35;
        for tyre in [-0.8f32, 0.8] {
            for i in 0..n {
                let j = (i + 1) % n;
                let a = |i: usize| 0.14 + 0.1 * wobble(t, i, 9.0, 51 + set);
                let l = |i: usize| t.line[i] + shift + tyre + 0.3 * wobble(t, i, 13.0, 61 + set);
                let (l0, l1) = (l(i), l(j));
                b.quad_uvc(
                    [
                        on(t, i, l0 - 0.16, 0.045),
                        on(t, i, l0 + 0.16, 0.045),
                        on(t, j, l1 + 0.16, 0.045),
                        on(t, j, l1 - 0.16, 0.045),
                    ],
                    [Vec2::ZERO; 4],
                    [brown(a(i)), brown(a(i)), brown(a(j)), brown(a(j))],
                );
            }
        }
    }
    // Slide marks: through each slow corner the rear wheels swing wide of the
    // fronts, leaving broad, soft arcs on the outside.
    let reach = (24.0 / t.step) as usize;
    let apexes = (0..n).filter(|&i| {
        let v = t.speed[i];
        v < crate::track::VMAX * 0.8
            && (1..reach).all(|k| v <= t.speed[(i + k) % n] && v <= t.speed[(i + n - k) % n])
    });
    for (a, apex) in apexes.enumerate() {
        let outside = -t.bend[apex].signum();
        for set in 0..2u32 {
            let len = (18.0 / t.step) as usize;
            let start = len / 2 + (noise(a as i32, set as i32, 71) * 4.0 / t.step) as usize;
            let strength = 0.16 + 0.1 * noise(a as i32, set as i32, 72);
            for tyre in [-0.8f32, 0.8] {
                for k in 0..len {
                    let i = (apex + n - start + k) % n;
                    let j = (i + 1) % n;
                    let fade = |k: usize| {
                        let u = k as f32 / len as f32;
                        strength * smooth(0.0, 0.3, u) * (1.0 - smooth(0.6, 1.0, u))
                    };
                    let swing = |k: usize| {
                        let u = k as f32 / len as f32;
                        outside * 1.1 * math::sin(u * std::f32::consts::PI)
                    };
                    let (l0, l1) = (
                        t.line[i] + tyre + swing(k) + set as f32 * 0.4 * outside,
                        t.line[j] + tyre + swing(k + 1) + set as f32 * 0.4 * outside,
                    );
                    b.quad_uvc(
                        [
                            on(t, i, l0 - 0.3, 0.046),
                            on(t, i, l0 + 0.3, 0.046),
                            on(t, j, l1 + 0.3, 0.046),
                            on(t, j, l1 - 0.3, 0.046),
                        ],
                        [Vec2::ZERO; 4],
                        [brown(fade(k)), brown(fade(k)), brown(fade(k + 1)), brown(fade(k + 1))],
                    );
                }
            }
        }
    }
    b.finish()
}

/// A chequered start/finish line across the road.
fn start_line(paint: &mut Builder, t: &Track) {
    let white = hex(0xf1efe8);
    let cells = 12;
    for row in 0..2 {
        for k in 0..cells {
            let a = -HALF + 2. * HALF * k as f32 / cells as f32;
            let w = 2. * HALF / cells as f32;
            let f = t.tangent(0);
            let o = t.centre(0) + f * (row as f32 * 0.75 - 0.75);
            let nrm = t.normal(0);
            let p = |lat: f32, lon: f32| {
                let at = o + nrm * lat + f * lon;
                v3(at, t.height(at, 0) + 0.07)
            };
            let c = if (k + row) % 2 == 0 {
                white
            } else {
                hex(0x22252b)
            };
            paint.quad_up(p(a, 0.), p(a + w, 0.), p(a + w, 0.75), p(a, 0.75), c);
        }
    }
}

/// The smooth centreline `s` metres past the start line: a cubic between the
/// samples either side that leaves each along the road's heading, so it bends
/// with the road between them rather than cutting across. Its point and its
/// unit left normal.
fn centreline(t: &Track, s: f32) -> (Vec2, Vec2) {
    let x = s / t.step;
    let k = math::floor(x);
    let (i, j) = (t.wrap(k as i64), t.wrap(k as i64 + 1));
    let (p0, p1) = (t.centre(i), t.centre(j));
    let (m0, m1) = (t.tangent(i) * t.step, t.tangent(j) * t.step);
    let u = x - k;
    let (u2, u3) = (u * u, u * u * u);
    let at = p0 * (2. * u3 - 3. * u2 + 1.) + m0 * (u3 - 2. * u2 + u) + p1 * (3. * u2 - 2. * u3) + m1 * (u3 - u2);
    let dir = p0 * (6. * u2 - 6. * u) + m0 * (3. * u2 - 4. * u + 1.) + p1 * (6. * u - 6. * u2) + m1 * (3. * u2 - 2. * u);
    let dir = dir.normalize();
    (at, Vec2::new(-dir.y, dir.x))
}

/// A dashed line down the middle of the road, 1.2 m painted and 1.8 m clear,
/// stretched a little so the lap closes on whole dashes with the start line in
/// a gap. Each dash is cut into pieces no longer than 15 cm along the smooth
/// centreline, so it curves with a hairpin and lies on the road over a crest.
fn centre_line(paint: &mut Builder, t: &Track, color: [f32; 4]) {
    const WIDTH: f32 = 0.14;
    let dashes = (t.length() / 3.0).round().max(1.0);
    let period = t.length() / dashes;
    let dash = period * 0.4;
    let pieces = (dash / 0.15).ceil() as u32;
    let row = |s: f32| {
        let (c, left) = centreline(t, s);
        let (i, _) = t.locate(c, t.wrap((s / t.step) as i64), 2);
        let g = t.gradient(c, i);
        let up = Vec3::new(-g.x, 1.0, -g.y).normalize();
        let side = |lat: f32| {
            let p = c + left * lat;
            v3(p, t.surface(p, i) + 0.05)
        };
        (side(-WIDTH / 2.0), side(WIDTH / 2.0), up)
    };
    for d in 0..dashes as u32 {
        let from = (d as f32 + 0.5) * period - dash / 2.0;
        let rows: Vec<_> = (0..=pieces).map(|k| row(from + dash * k as f32 / pieces as f32)).collect();
        paint.strip(&rows, color);
    }
}

/// The road: clay run-off and asphalt (textured), then edge lines, kerbs and
/// the start line. A dirt track gets `dirt_road` instead.
pub fn road(t: &Track) -> Model {
    if t.dirt {
        return dirt_road(t);
    }
    let n = t.len();
    let mut clay = Builder::new();
    textured(
        &mut clay,
        t,
        -(HALF + RUNOFF),
        HALF + RUNOFF,
        0.02,
        3.0,
        |i| shade([1.0; 4], 0.9 + 0.12 * noise(i as i32, 0, 3)),
    );
    let mut tarmac = Builder::new();
    textured_strips(&mut tarmac, t, -HALF, HALF, 10, 0.04, 4.6, |i, lat| {
        // Rubbered in darker along the racing line, dusty and lighter at the edges.
        let d = (lat - t.line[i]) / 1.6;
        let rubber = 0.22 * math::exp(-d * d);
        let edge = 0.08 * ((lat.abs() - (HALF - 1.0)) / 1.0).clamp(0.0, 1.0);
        let k = (0.98 + 0.05 * noise(i as i32 / 6, 1, 5)) * (1.0 - rubber) + edge;
        // Rain darkens the tarmac; it pools where the surface dips and in the
        // ruts the tyres have worn, a blacker sheen in broad patches.
        let k = if t.wet {
            let puddle = noise(i as i32 / 9, (lat * 0.6) as i32, 31);
            k * (0.5 - 0.12 * puddle)
        } else {
            k
        };
        shade([1.0; 4], k)
    });
    let mut verge = Builder::new();
    for side in [-1.0f32, 1.0] {
        textured(
            &mut verge,
            t,
            side * (HALF + RUNOFF - 0.05),
            side * (HALF + RUNOFF + 1.5),
            0.015,
            3.0,
            |_| [1.25, 1.3, 1.05, 1.0],
        );
    }
    let mut paint = Builder::new();
    let white = hex(0xf1efe8);
    for s in [-1.0f32, 1.0] {
        ribbon(
            &mut paint,
            t,
            s * (HALF - 0.4),
            s * (HALF - 0.15),
            0.05,
            |_| Some(white),
        );
    }
    centre_line(&mut paint, t, white);
    // Kerbs on both sides of every real corner: raised red and white blocks about
    // a metre long, ramped on both edges so they catch the light like the real thing.
    let red = hex(0xd83a2e);
    // In the rain at dusk the paint is grimy and dim.
    let (red, white) = if t.wet { (shade(red, 0.55), shade(white, 0.5)) } else { (red, white) };
    let kerb = |i: usize| t.kerb[i];
    // Across the kerb: (lateral offset from the asphalt edge inward, height).
    let profile = [(-0.15, 0.045), (0.05, 0.13), (0.45, 0.13), (0.6, 0.03)];
    let red_block = |i: usize| (i as f32 * t.step) as i32 % 2 == 0;
    for s in [-1.0f32, 1.0] {
        for i in (0..n).filter(|&i| kerb(i)) {
            let j = (i + 1) % n;
            let top = if red_block(i) { red } else { white };
            for (k, w) in profile.windows(2).enumerate() {
                // Ramps a shade darker than the top, which faces the sun.
                let c = if k == 1 { top } else { shade(top, 0.8) };
                let (l0, h0) = (s * (HALF + w[0].0), w[0].1);
                let (l1, h1) = (s * (HALF + w[1].0), w[1].1);
                paint.quad_up(
                    on(t, i, l0, h0),
                    on(t, i, l1, h1),
                    on(t, j, l1, h1),
                    on(t, j, l0, h0),
                    c,
                );
            }
            // The block's leading face, where its colour changes.
            let prev = (i + n - 1) % n;
            if kerb(prev) && red_block(prev) != red_block(i) {
                for w in profile.windows(2) {
                    let (l0, h0) = (s * (HALF + w[0].0), w[0].1);
                    let (l1, h1) = (s * (HALF + w[1].0), w[1].1);
                    let back = -t.tangent(i);
                    paint.face(
                        [
                            on(t, i, l0, 0.03),
                            on(t, i, l1, 0.03),
                            on(t, i, l1, h1),
                            on(t, i, l0, h0),
                        ],
                        Vec3::new(back.x, 0.0, back.y),
                        shade(top, 0.6),
                    );
                }
            }
        }
    }
    start_line(&mut paint, t);
    // Wet, the surfaces turn glossy: the tarmac most of all, so the
    // headlights' pools shine on it.
    let (r_clay, r_tarmac, r_paint) = if t.wet { (0.55, 0.16, 0.3) } else { (0.95, 0.85, 0.7) };
    let mut clay = clay.finish();
    drape(&mut clay, t);
    clay.material = 0;
    let mut tarmac = tarmac.finish();
    drape(&mut tarmac, t);
    tarmac.material = 1;
    let mut paint = paint.finish();
    paint.material = 2;
    let mut verge = verge.finish();
    drape(&mut verge, t);
    verge.material = 3;
    let mut marks = skids(t);
    drape(&mut marks, t);
    marks.material = 4;
    model(
        vec![clay, tarmac, paint, verge, marks],
        vec![
            textured_material(0, r_clay),
            textured_material(1, r_tarmac),
            MaterialData {
                metallic: 0.0,
                roughness: r_paint,
                ..MaterialData::default()
            },
            textured_material(2, 0.95),
            MaterialData {
                metallic: 0.0,
                roughness: 0.9,
                alpha_mode: AlphaMode::Blend,
                ..MaterialData::default()
            },
        ],
        &["clay.tex", "asphalt.tex", "grass.tex"],
    )
}

/// Rubber laid down under braking: dark tyre streaks along the racing line into
/// every slow corner, a few cars' worth each, fading in and out. Vertex alpha
/// carries the fade; the material blends.
fn skids(t: &Track) -> MeshData {
    let mut b = Builder::new();
    let n = t.len();
    let reach = (24.0 / t.step) as usize;
    let apexes = (0..n).filter(|&i| {
        let v = t.speed[i];
        v < crate::track::VMAX * 0.8
            && (1..reach).all(|k| v <= t.speed[(i + k) % n] && v <= t.speed[(i + n - k) % n])
    });
    for (a, apex) in apexes.enumerate() {
        for set in 0..3u32 {
            let shift = (noise(a as i32, set as i32, 21) - 0.5) * 0.9;
            let start = reach - (noise(a as i32, set as i32, 22) * 6.0 / t.step) as usize;
            let len = start + (5.0 / t.step) as usize;
            let strength = 0.22 + 0.18 * noise(a as i32, set as i32, 23);
            for tyre in [-0.78f32, 0.78] {
                for k in 0..len {
                    let i = (apex + n - start + k) % n;
                    let j = (i + 1) % n;
                    let fade = |k: usize| {
                        let u = k as f32 / len as f32;
                        strength * smooth(0.0, 0.25, u) * (1.0 - smooth(0.7, 1.0, u))
                    };
                    let (l0, l1) = (t.line[i] + shift + tyre, t.line[j] + shift + tyre);
                    let c0 = [0.02, 0.02, 0.02, fade(k)];
                    let c1 = [0.02, 0.02, 0.02, fade(k + 1)];
                    b.quad_uvc(
                        [
                            on(t, i, l0 - 0.12, 0.045),
                            on(t, i, l0 + 0.12, 0.045),
                            on(t, j, l1 + 0.12, 0.045),
                            on(t, j, l1 - 0.12, 0.045),
                        ],
                        [Vec2::ZERO; 4],
                        [c0, c0, c1, c1],
                    );
                }
            }
        }
    }
    b.finish()
}

/// The grass: a textured grid, tinted in broad patches so the tiling never shows.
pub fn ground(t: &Track) -> Model {
    let mut b = Builder::new();
    let cell = CELL;
    let (nx, nz) = CELLS;
    let tile = 7.0;
    let (x0, z0) = (t.gx, t.gz);
    // One tint per grid corner, shared by the four cells around it, so the
    // shading is a smooth wash: bright mown grass by the road, darkening into
    // the shade under the woods, with broad soft patches across it.
    let tints: Vec<[f32; 4]> = (0..=nx)
        .flat_map(|i| (0..=nz).map(move |j| (i, j)))
        .map(|(i, j)| {
            let p = Vec2::new(x0 + i as f32 * cell, z0 + j as f32 * cell);
            let woods = smooth(WALL - 1.0, WALL + 14.0, clearance(t, p));
            let (a, b) = (i as i32, j as i32);
            let patch = 0.5 * noise(a / 3, b / 3, 11) + 0.5 * noise((a + 1) / 3, (b + 1) / 3, 11);
            let k = (1.08 - 0.4 * woods) * (0.94 + 0.12 * patch);
            // Round a dirt track the grass is parched: yellower, browner, in drier patches.
            let dry = if t.dirt { 1.0 - 0.6 * woods } else { 0.0 };
            // Moorland in the rain: a dark, muted olive, damp all over.
            if t.wet {
                return [k * 0.62, k * 0.66, k * 0.56, 1.0];
            }
            [
                k * (1.0 - 0.1 * woods) * (1.0 + 0.32 * dry),
                k * (1.0 + 0.04 * dry),
                k * (0.95 + 0.08 * woods) * (1.0 - 0.3 * dry),
                1.0,
            ]
        })
        .collect();
    let at = |i: usize, j: usize| tints[i * (nz + 1) + j];
    for i in 0..nx {
        for j in 0..nz {
            let q = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
            let uv = q.map(|(a, c)| {
                Vec2::new((x0 + a as f32 * cell) / tile, (z0 + c as f32 * cell) / tile)
            });
            b.quad_uvcn(
                q.map(|(a, c)| land_corner(t, a, c, 0.0)),
                uv,
                q.map(|(a, c)| {
                    let n = land_normal(t, a, c);
                    let tint = at(a, c);
                    if t.wet {
                        return moor_tint(t, tint, n, a, c);
                    }
                    let shade = hillshade(n);
                    [tint[0] * shade, tint[1] * shade, tint[2] * shade, tint[3]]
                }),
                q.map(|(a, c)| land_normal(t, a, c)),
            );
        }
    }
    let texture = if t.dirt { "scree.tex" } else { "grass.tex" };
    model(
        vec![b.finish()],
        vec![textured_material(0, 0.95)],
        &[texture],
    )
}

/// The moor's grass from above: its relief shaded hard, since under cloud
/// there is no sun to show it, and pale mist lying in the hollows, thinning
/// up the slopes to the dark crests, as in a valley at dusk.
fn moor_tint(t: &Track, tint: [f32; 4], n: Vec3, i: usize, j: usize) -> [f32; 4] {
    let shade = (1.0 + 2.6 * (n.dot(SUN) - SUN.y)).clamp(0.35, 1.6);
    let level = t.y.iter().sum::<f32>() / t.len().max(1) as f32;
    let low = level - t.land_node(i as i64, j as i64);
    let mist = smooth(-1.0, 6.0, low) * 0.85;
    let crest = smooth(1.0, 7.0, -low) * 0.35;
    let k = shade * (1.0 - crest);
    let haze = [1.55, 1.95, 2.0];
    let mut out = [0.0, 0.0, 0.0, tint[3]];
    for c in 0..3 {
        out[c] = tint[c] * k * (1.0 - mist) + haze[c] * tint[c].max(0.5) * mist;
    }
    out
}

/// A corner of the land grid, `lift` above it.
fn land_corner(t: &Track, i: usize, j: usize, lift: f32) -> Vec3 {
    Vec3::new(
        t.gx + i as f32 * CELL,
        t.land_node(i as i64, j as i64) + lift,
        t.gz + j as f32 * CELL,
    )
}

/// The land's smooth normal at a grid corner, from its neighbours.
fn land_normal(t: &Track, i: usize, j: usize) -> Vec3 {
    let (i, j) = (i as i64, j as i64);
    let dx = (t.land_node(i + 1, j) - t.land_node(i - 1, j)) / (2.0 * CELL);
    let dz = (t.land_node(i, j + 1) - t.land_node(i, j - 1)) / (2.0 * CELL);
    Vec3::new(-dx, 1.0, -dz).normalize()
}

pub fn centre_of(t: &Track) -> (f32, f32) {
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for i in 0..t.len() {
        lo = lo.min(t.centre(i));
        hi = hi.max(t.centre(i));
    }
    ((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5)
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn clearance(t: &Track, p: Vec2) -> f32 {
    (0..t.len())
        .map(|i| (t.centre(i) - p).length())
        .fold(f32::MAX, f32::min)
}

/// One placed Blender model: which, where, which way, and how big.
pub struct Prop {
    pub model: &'static str,
    pub at: Vec2,
    pub yaw: f32,
    pub scale: Vec3,
    /// A tree's crown: radius and height in metres, for fading it out of the way.
    pub crown: Option<(f32, f32)>,
    /// Tilt along its length (fence sections follow the ground's rise).
    pub pitch: f32,
    /// How far it is let into the ground, so it never floats on a slope.
    pub sink: f32,
}

impl Prop {
    fn new(model: &'static str, at: Vec2, yaw: f32, scale: Vec3) -> Self {
        Self {
            model,
            at,
            yaw,
            scale,
            crown: None,
            pitch: 0.0,
            sink: 0.0,
        }
    }
}

/// Fence sections along both fence lines, then trees, bushes and rocks beyond them.
pub fn props(w: &World, t: &Track, quality: u32) -> Vec<Prop> {
    let mut out = Vec::new();
    let n = t.len();
    for s in [-1.0f32, 1.0] {
        let every = (3.0 / t.step).max(1.0) as usize;
        let mut i = 0;
        while i < n {
            let j = (i + every) % n;
            let a = t.at(i, s * (t.wall[i] - 0.2));
            let c = t.at(j, s * (t.wall[j] - 0.2));
            let d = c - a;
            let rise = t.height(c, j) - t.height(a, i);
            out.push(Prop {
                pitch: math::atan(rise / d.length().max(0.1)),
                sink: 0.05,
                ..Prop::new(
                    "fence.model",
                    a,
                    math::atan2(d.y, d.x),
                    Vec3::new(d.length() / 3.0, 1.0, 1.0),
                )
            });
            // Red and white boards in front of the fence round the outside of corners.
            // A dirt track leaves the outside of its corners open.
            let room = t.wall[i].min(t.wall[j]) > HALF + RUNOFF + 3.2;
            if !t.dirt && room && t.bend[i] * s < -1.0 / 40.0 {
                let lat = s * (HALF + RUNOFF + 2.0);
                let (p, q) = (t.at(i, lat), t.at(j, lat));
                // The frame always behind the boards, away from the road.
                let (from, d) = if s > 0.0 { (q, p - q) } else { (p, q - p) };
                let rise = t.height(from + d, i) - t.height(from, i);
                out.push(Prop {
                    pitch: math::atan(rise / d.length().max(0.1)),
                    sink: 0.05,
                    ..Prop::new(
                        "barrier.model",
                        from,
                        math::atan2(d.y, d.x),
                        Vec3::new(d.length() / 3.0, 1.0, 1.0),
                    )
                });
            }
            i += every;
        }
    }
    if t.dirt {
        rocky(w, t, quality, &mut out);
        return out;
    }
    if t.wet {
        moor(w, t, quality, &mut out);
        return out;
    }
    // Dressing along the verge: ferns, poppies and stones, logs by the corners.
    if quality > 0 {
        let every = (4.0 / t.step).max(1.0) as usize;
        for s in [-1.0f32, 1.0] {
            for i in (0..n).step_by(every) {
                let lat = s * (HALF + RUNOFF + w.rand(1.9..2.8));
                let p = t.at(i, lat);
                if t.wall[i] < HALF + RUNOFF + 3.4 || clearance(t, p) < HALF + RUNOFF + 1.6 {
                    continue;
                }
                let roll = w.rand(0.0..1.0);
                let corner = t.bend[i].abs() > 1.0 / 30.0;
                let model = match roll {
                    r if r < 0.3 => "fern.model",
                    r if r < 0.4 => "poppies.model",
                    r if r < 0.46 => "rock.model",
                    r if r < 0.56 && corner => "logs.model",
                    _ => continue,
                };
                let scale = if model == "rock.model" { 0.6 } else { 1.0 };
                out.push(Prop::new(
                    model,
                    p,
                    w.rand(0.0..std::f32::consts::TAU),
                    Vec3::splat(scale * w.rand(0.8..1.15)),
                ));
            }
        }
    }
    let (cx, cz) = centre_of(t);
    let mut placed: Vec<(Vec2, f32)> = Vec::new();
    let free = |placed: &[(Vec2, f32)], p: Vec2, r: f32| {
        placed.iter().all(|(q, s)| (*q - p).length() >= r + s)
    };
    // Woods: conifers crowd up to the fences, the odd broadleaf among them.
    // Every quality has the same woods, so one baked shadow overlay fits them all;
    // low quality skips the undergrowth.
    let (trees, undergrowth) = (1800, if quality == 0 { 0 } else { 1400 });
    for _ in 0..trees {
        let p = Vec2::new(w.rand(cx - 95.0..cx + 95.0), w.rand(cz - 82.0..cz + 82.0));
        let clear = clearance(t, p);
        // Only where the camera can see: it never strays more than ~30 m from the road.
        if !(WALL + 1.6..34.0).contains(&clear) {
            continue;
        }
        let (model, radius) = *w
            .pick(&[
                ("pine-a.model", 2.3),
                ("pine-b.model", 1.9),
                ("pine-c.model", 2.7),
                ("pine-a.model", 2.3),
                ("pine-b.model", 1.9),
                ("tree-a.model", 2.6),
            ])
            .unwrap();
        let scale = w.rand(1.0..1.5);
        if !free(&placed, p, radius * scale * 0.85) {
            continue;
        }
        placed.push((p, radius * scale * 0.85));
        let height = match model {
            "pine-a.model" => 8.5,
            "pine-b.model" => 6.5,
            "pine-c.model" => 10.5,
            _ => 6.8,
        };
        out.push(Prop {
            crown: Some((radius * scale * 1.15, height * scale)),
            sink: 0.25,
            ..Prop::new(
                model,
                p,
                w.rand(0.0..std::f32::consts::TAU),
                Vec3::splat(scale),
            )
        });
        // Toadstools at the foot of the odd tree.
        if undergrowth > 0 && w.chance(0.12) {
            let a = w.rand(0.0..std::f32::consts::TAU);
            let (sa, ca) = math::sin_cos(a);
            out.push(Prop::new(
                "mushrooms.model",
                p + Vec2::new(ca, sa) * (radius * scale * 0.55),
                a,
                Vec3::splat(w.rand(1.0..1.6)),
            ));
        }
    }
    // Undergrowth on the grass between the verge and the fences, and in the infield.
    for _ in 0..undergrowth {
        let p = Vec2::new(w.rand(cx - 80.0..cx + 80.0), w.rand(cz - 70.0..cz + 70.0));
        let clear = clearance(t, p);
        if !(HALF + RUNOFF + 1.8..30.0).contains(&clear) {
            continue;
        }
        let (model, radius) = *w
            .pick(&[
                ("fern.model", 0.9),
                ("fern.model", 0.9),
                ("bush.model", 1.1),
                ("daisies.model", 0.6),
                ("poppies.model", 0.4),
                ("mushrooms.model", 0.3),
                ("stump.model", 0.4),
                ("rock.model", 0.9),
                ("logs.model", 1.0),
            ])
            .unwrap();
        // Big props keep off the strip between the road and the fence.
        if matches!(model, "logs.model" | "rock.model" | "bush.model") && clear < WALL + 0.5 {
            continue;
        }
        if !free(&placed, p, radius) {
            continue;
        }
        placed.push((p, radius));
        out.push(Prop::new(
            model,
            p,
            w.rand(0.0..std::f32::consts::TAU),
            Vec3::splat(w.rand(0.8..1.2)),
        ));
    }
    out
}

/// The open moor round a wet track: bare rolling hills, no trees, only
/// bracken and the odd grey stone in the grass, so the land's shape and the
/// mist lying in its hollows are the scenery.
fn moor(w: &World, t: &Track, quality: u32, out: &mut Vec<Prop>) {
    let (cx, cz) = centre_of(t);
    let mut placed: Vec<(Vec2, f32)> = Vec::new();
    let free = |placed: &[(Vec2, f32)], p: Vec2, r: f32| {
        placed.iter().all(|(q, s)| (*q - p).length() >= r + s)
    };
    let tufts = if quality == 0 { 120 } else { 700 };
    for _ in 0..tufts {
        let p = Vec2::new(w.rand(cx - 90.0..cx + 90.0), w.rand(cz - 78.0..cz + 78.0));
        let clear = clearance(t, p);
        if !(HALF + RUNOFF + 1.8..34.0).contains(&clear) {
            continue;
        }
        // Mostly bracken; a stone in one of every six, kept off the strip by the fence.
        let stone = w.chance(0.16);
        if stone && clear < WALL + 0.5 {
            continue;
        }
        let radius = if stone { 0.9 } else { 0.8 };
        if !free(&placed, p, radius) {
            continue;
        }
        placed.push((p, radius));
        let (model, scale) = if stone {
            ("rock.model", w.rand(0.5..1.3))
        } else {
            ("fern.model", w.rand(0.7..1.15))
        };
        out.push(Prop::new(
            model,
            p,
            w.rand(0.0..std::f32::consts::TAU),
            Vec3::splat(scale),
        ));
    }
}

/// A dirt track's surroundings: rocky scrubland with boulders, a few hardy
/// pines, bushes and stumps, and stones scattered along the verge.
fn rocky(w: &World, t: &Track, quality: u32, out: &mut Vec<Prop>) {
    let n = t.len();
    let (cx, cz) = centre_of(t);
    let mut placed: Vec<(Vec2, f32)> = Vec::new();
    let free = |placed: &[(Vec2, f32)], p: Vec2, r: f32| {
        placed.iter().all(|(q, s)| (*q - p).length() >= r + s)
    };
    // Boulders first, big ones out past the fences, in loose clusters.
    for _ in 0..900 {
        let p = Vec2::new(w.rand(cx - 95.0..cx + 95.0), w.rand(cz - 82.0..cz + 82.0));
        let clear = clearance(t, p);
        if !(WALL + 1.2..34.0).contains(&clear) {
            continue;
        }
        let size = w.rand(1.3..3.8);
        let radius = 0.9 * size;
        if !free(&placed, p, radius * 0.8) {
            continue;
        }
        placed.push((p, radius * 0.8));
        let flat = w.rand(0.55..1.15);
        out.push(Prop::new(
            "rock.model",
            p,
            w.rand(0.0..std::f32::consts::TAU),
            Vec3::new(size, size * flat, size * w.rand(0.8..1.1)),
        ));
    }
    // A few hardy pines among the rocks: far fewer than in the woods.
    for _ in 0..500 {
        let p = Vec2::new(w.rand(cx - 95.0..cx + 95.0), w.rand(cz - 82.0..cz + 82.0));
        let clear = clearance(t, p);
        if !(WALL + 2.5..34.0).contains(&clear) {
            continue;
        }
        let (model, radius, height) = *w
            .pick(&[
                ("pine-a.model", 2.3, 8.5),
                ("pine-b.model", 1.9, 6.5),
                ("pine-c.model", 2.7, 10.5),
            ])
            .unwrap();
        let scale = w.rand(0.9..1.3);
        if !free(&placed, p, radius * scale * 0.85) {
            continue;
        }
        placed.push((p, radius * scale * 0.85));
        out.push(Prop {
            crown: Some((radius * scale * 1.15, height * scale)),
            sink: 0.25,
            ..Prop::new(
                model,
                p,
                w.rand(0.0..std::f32::consts::TAU),
                Vec3::splat(scale),
            )
        });
    }
    // Stones along the verge, thrown wide by the cars.
    let every = (2.5 / t.step).max(1.0) as usize;
    for s in [-1.0f32, 1.0] {
        for i in (0..n).step_by(every) {
            let p = t.at(i, s * (HALF + RUNOFF + w.rand(1.0..2.8)));
            let size = w.rand(0.35..0.9);
            if t.wall[i] < HALF + RUNOFF + 3.4 || clearance(t, p) < HALF + RUNOFF + 0.9 || !w.chance(0.55) {
                continue;
            }
            out.push(Prop::new(
                "rock.model",
                p,
                w.rand(0.0..std::f32::consts::TAU),
                Vec3::new(size, size * w.rand(0.6..1.0), size),
            ));
        }
    }
    // Scrub: bushes, stumps and the odd log; no flowers or ferns up here.
    let scrub = if quality == 0 { 0 } else { 700 };
    for _ in 0..scrub {
        let p = Vec2::new(w.rand(cx - 80.0..cx + 80.0), w.rand(cz - 70.0..cz + 70.0));
        let clear = clearance(t, p);
        if !(HALF + RUNOFF + 2.0..30.0).contains(&clear) {
            continue;
        }
        let (model, radius) = *w
            .pick(&[
                ("bush.model", 1.1),
                ("rock.model", 0.9),
                ("rock.model", 0.9),
                ("stump.model", 0.4),
                ("logs.model", 1.0),
            ])
            .unwrap();
        if model != "rock.model" && model != "stump.model" && clear < WALL + 0.5 {
            continue;
        }
        if !free(&placed, p, radius) {
            continue;
        }
        placed.push((p, radius));
        let scale = if model == "rock.model" {
            w.rand(0.6..1.4)
        } else {
            w.rand(0.8..1.2)
        };
        out.push(Prop::new(
            model,
            p,
            w.rand(0.0..std::f32::consts::TAU),
            Vec3::splat(scale),
        ));
    }
}

/// Flowers in little clusters over the grass, as one generated mesh: white
/// daisies with yellow hearts, the odd red poppy with a dark one.
pub fn flowers(w: &World, t: &Track) -> MeshData {
    let mut b = Builder::new();
    let (cx, cz) = centre_of(t);
    let daisy = (hex(0xf6f3ea), hex(0xf2c230));
    let poppy = (hex(0xd8281e), hex(0x2a1d16));
    let pale = (hex(0xf2c6d6), hex(0xf6e7a1));
    for k in 0..260 {
        let c = Vec2::new(w.rand(cx - 75.0..cx + 75.0), w.rand(cz - 65.0..cz + 65.0));
        let clear = clearance(t, c);
        if !(HALF + RUNOFF + 1.2..WALL + 12.0).contains(&clear) {
            continue;
        }
        let (petal, heart) = match k % 9 {
            0 | 5 => poppy,
            3 => pale,
            _ => daisy,
        };
        for _ in 0..w.rand(3..8u32) {
            let p = c + Vec2::new(w.rand(-0.9..0.9), w.rand(-0.9..0.9));
            if clearance(t, p) < HALF + RUNOFF + 0.8 {
                continue;
            }
            blossom(
                &mut b,
                p,
                t.height_any(p),
                w.rand(0.11..0.17),
                w.rand(0.0..1.0),
                petal,
                heart,
            );
        }
    }
    b.finish()
}

/// One flower lying on the grass: a ring of round-tipped petals and a raised heart.
fn blossom(b: &mut Builder, p: Vec2, ground: f32, r: f32, turn: f32, petal: [f32; 4], heart: [f32; 4]) {
    let y = ground + 0.05;
    let at = |a: f32, d: f32, h: f32| {
        let (s, c) = math::sin_cos(a);
        Vec3::new(p.x + c * d, h, p.y + s * d)
    };
    let tau = std::f32::consts::TAU;
    let petals = 7;
    for k in 0..petals {
        let a = turn * tau + k as f32 / petals as f32 * tau;
        let w = tau / petals as f32 * 0.42;
        // A kite: narrow at the heart, round at the tip.
        b.quad_up(
            at(a, r * 0.25, y),
            at(a - w, r * 0.75, y),
            at(a, r, y),
            at(a + w, r * 0.75, y),
            petal,
        );
    }
    for k in 0..6 {
        let a = k as f32 / 6.0 * tau;
        let a2 = (k + 1) as f32 / 6.0 * tau;
        b.tri(
            at(0.0, 0.0, y + 0.02),
            at(a, r * 0.3, y + 0.01),
            at(a2, r * 0.3, y + 0.01),
            heart,
        );
    }
}

/// The player's marker: a downward chevron that floats over the car.
pub fn marker() -> MeshData {
    let mut b = Builder::new();
    b.pyramid(Vec3::new(0.0, 0.0, 0.0), 0.0, 0.8, 0.45, hex(0xfff1a8));
    b.pyramid(Vec3::new(0.0, 0.8, 0.0), 0.45, 0.01, 0.0, hex(0xfff1a8));
    b.finish()
}

/// Where the ground (and the baked shade over it) lies: its centre and full size.
pub fn ground_rect(t: &Track) -> (Vec2, Vec2) {
    let size = Vec2::new(CELLS.0 as f32 * CELL, CELLS.1 as f32 * CELL);
    (Vec2::new(t.gx, t.gz) + size * 0.5, size)
}

/// The baked shade: one alpha-blended quad over the whole ground carrying the
/// trees' and fences' low-sun shadows, painted offline by art/src/shade.py.
pub fn shade_overlay(t: &Track) -> Model {
    let mut b = Builder::new();
    let (nx, nz) = CELLS;
    // Draped over the land grid; under the road the land is `SINK` below it,
    // so this lies about 9 cm over the road as it always has.
    for i in 0..nx {
        for j in 0..nz {
            let q = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
            b.quad_uvcn(
                q.map(|(a, c)| land_corner(t, a, c, SINK + 0.09)),
                // Image rows run north to south: v grows with z.
                q.map(|(a, c)| Vec2::new(a as f32 / nx as f32, c as f32 / nz as f32)),
                [[1.0; 4]; 4],
                q.map(|(a, c)| land_normal(t, a, c)),
            );
        }
    }
    let texture = format!("shade-{}.tex", t.id);
    model(
        vec![b.finish()],
        vec![MaterialData {
            alpha_mode: AlphaMode::Blend,
            ..textured_material(0, 1.0)
        }],
        &[texture.as_str()],
    )
}

/// A soft dark patch under a car, stretched away from the sun.
pub fn blob() -> Model {
    let mut b = Builder::new();
    let (hx, hz) = (2.3, 1.25);
    b.quad_uv(
        [
            Vec3::new(-hx, 0.0, -hz),
            Vec3::new(hx, 0.0, -hz),
            Vec3::new(hx, 0.0, hz),
            Vec3::new(-hx, 0.0, hz),
        ],
        [
            Vec2::new(0., 0.),
            Vec2::new(1., 0.),
            Vec2::new(1., 1.),
            Vec2::new(0., 1.),
        ],
        [1.0; 4],
    );
    model(
        vec![b.finish()],
        vec![MaterialData {
            alpha_mode: AlphaMode::Blend,
            ..textured_material(0, 1.0)
        }],
        &["blob.tex"],
    )
}

/// One live skid mark: a dark, soft-edged streak a metre long along local X,
/// a tyre's width across. The race scales it to the distance slid.
pub fn skid_mark() -> Model {
    let mut b = Builder::new();
    let (hx, hz) = (0.5, 0.12);
    let dark = [0.02, 0.02, 0.02, 1.0];
    let clear = [0.02, 0.02, 0.02, 0.0];
    // Faded at both ends, so a run of marks reads as one streak.
    for (x0, x1, c0, c1) in [(-hx, -0.3, clear, dark), (-0.3, 0.3, dark, dark), (0.3, hx, dark, clear)] {
        b.quad_uvc(
            [
                Vec3::new(x0, 0.0, -hz),
                Vec3::new(x1, 0.0, -hz),
                Vec3::new(x1, 0.0, hz),
                Vec3::new(x0, 0.0, hz),
            ],
            [Vec2::ZERO; 4],
            [c0, c1, c1, c0],
        );
    }
    model(
        vec![b.finish()],
        vec![MaterialData {
            metallic: 0.0,
            roughness: 0.9,
            alpha_mode: AlphaMode::Blend,
            ..MaterialData::default()
        }],
        &[],
    )
}

/// A pair of brake lamps on a car's tail, facing back along -X with a lid
/// on top: `x` the tail's position, `y` the lamps' height and `z` their
/// distance off centre. Under them a red wash of their light on the road
/// behind, which is what shows from high above.
pub fn brake_lamps(x: f32, y: f32, z: f32) -> Model {
    let mut lamps = Builder::new();
    let white = [1.0; 4];
    for s in [-1.0f32, 1.0] {
        let (z0, z1) = (s * z - 0.16, s * z + 0.16);
        lamps.face(
            [
                Vec3::new(x, y - 0.08, z0),
                Vec3::new(x, y - 0.08, z1),
                Vec3::new(x, y + 0.08, z1),
                Vec3::new(x, y + 0.08, z0),
            ],
            Vec3::NEG_X,
            white,
        );
        lamps.quad_up(
            Vec3::new(x, y + 0.08, z0),
            Vec3::new(x + 0.14, y + 0.08, z0),
            Vec3::new(x + 0.14, y + 0.08, z1),
            Vec3::new(x, y + 0.08, z1),
            white,
        );
    }
    let mut wash = Builder::new();
    let (near, far, half) = (x + 0.1, x - 1.5, 1.05);
    let lit = [1.0, 1.0, 1.0, 0.5];
    let dark = [1.0, 1.0, 1.0, 0.0];
    wash.quad_uvc(
        [
            Vec3::new(far, 0.06, -half),
            Vec3::new(near, 0.06, -half),
            Vec3::new(near, 0.06, half),
            Vec3::new(far, 0.06, half),
        ],
        [Vec2::ZERO; 4],
        [dark, lit, lit, dark],
    );
    let mut wash = wash.finish();
    wash.material = 1;
    model(
        vec![lamps.finish(), wash],
        vec![
            MaterialData {
                base_color: [0.0, 0.0, 0.0, 1.0],
                metallic: 0.0,
                emissive: [6.0, 0.3, 0.15],
                ..MaterialData::default()
            },
            MaterialData {
                base_color: [0.9, 0.05, 0.03, 1.0],
                metallic: 0.0,
                emissive: [1.2, 0.06, 0.03],
                alpha_mode: AlphaMode::Blend,
                ..MaterialData::default()
            },
        ],
        &[],
    )
}

/// A flat chevron on the road pointing along +X, for the braking guide.
pub fn chevron() -> MeshData {
    let mut b = Builder::new();
    let white = [1.0; 4];
    let (w, d, t) = (0.55, 0.35, 0.16);
    // Two arms meeting at the tip.
    for s in [-1.0f32, 1.0] {
        b.quad_up(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(-d, 0.0, s * w),
            Vec3::new(-d - t, 0.0, s * w),
            Vec3::new(-t, 0.0, 0.0),
            white,
        );
    }
    b.finish()
}

/// An arrow lying beside the player's car, pointing forward along +X: the
/// side it has chosen to pass on.
pub fn side_arrow() -> MeshData {
    let mut b = Builder::new();
    let white = [1.0; 4];
    b.quad_up(
        Vec3::new(-0.7, 0.0, -0.11),
        Vec3::new(0.2, 0.0, -0.11),
        Vec3::new(0.2, 0.0, 0.11),
        Vec3::new(-0.7, 0.0, 0.11),
        white,
    );
    b.tri(
        Vec3::new(0.2, 0.0, -0.32),
        Vec3::new(0.75, 0.0, 0.0),
        Vec3::new(0.2, 0.0, 0.32),
        white,
    );
    b.finish()
}
