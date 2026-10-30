//! Generated geometry: the textured ground and road with its kerbs and lines,
//! flowers, the player's marker, and where the Blender models stand.
use crate::track::{Track, HALF, RUNOFF, WALL};
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
        let up = (q[1] - q[0]).cross(q[2] - q[0]).y >= 0.0;
        let order = if up {
            [0, 1, 2, 0, 2, 3]
        } else {
            [0, 2, 1, 0, 3, 2]
        };
        let base = (self.m.positions.len() / 3) as u32;
        for k in 0..4 {
            let p = q[k];
            self.m.positions.extend([p.x, p.y, p.z]);
            self.m.normals.extend([0.0, 1.0, 0.0]);
            self.m.uvs.extend([uv[k].x, uv[k].y]);
            self.m.colors.extend(colors[k]);
            self.lo = self.lo.min(p);
            self.hi = self.hi.max(p);
        }
        self.m.indices.extend(order.map(|k| base + k));
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
                v3(t.at(i, from), y),
                v3(t.at(i, to), y),
                v3(t.at(j, to), y),
                v3(t.at(j, from), y),
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
    textured_strips(b, t, from, to, 1, y, tile, |i, _| tint(i));
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
                    v3(t.at(i, a), y),
                    v3(t.at(i, c), y),
                    v3(t.at(j, c), y),
                    v3(t.at(j, a), y),
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

/// The road: clay run-off and asphalt (textured), then edge lines, kerbs and the start line.
pub fn road(t: &Track) -> Model {
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
        shade(
            [1.0; 4],
            (0.98 + 0.05 * noise(i as i32 / 6, 1, 5)) * (1.0 - rubber) + edge,
        )
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
    // Kerbs on both sides of every real corner: raised red and white blocks about
    // a metre long, ramped on both edges so they catch the light like the real thing.
    let red = hex(0xd83a2e);
    let kerb = |i: usize| (0..9).any(|k| t.bend[(i + n + k - 4) % n].abs() > 1.0 / 24.0);
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
                    v3(t.at(i, l0), h0),
                    v3(t.at(i, l1), h1),
                    v3(t.at(j, l1), h1),
                    v3(t.at(j, l0), h0),
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
                            v3(t.at(i, l0), 0.03),
                            v3(t.at(i, l1), 0.03),
                            v3(t.at(i, l1), h1),
                            v3(t.at(i, l0), h0),
                        ],
                        Vec3::new(back.x, 0.0, back.y),
                        shade(top, 0.6),
                    );
                }
            }
        }
    }
    // A chequered start/finish line.
    let cells = 12;
    for row in 0..2 {
        for k in 0..cells {
            let a = -HALF + 2. * HALF * k as f32 / cells as f32;
            let w = 2. * HALF / cells as f32;
            let f = t.tangent(0);
            let o = t.centre(0) + f * (row as f32 * 0.75 - 0.75);
            let nrm = t.normal(0);
            let p = |lat: f32, lon: f32| v3(o + nrm * lat + f * lon, 0.07);
            let c = if (k + row) % 2 == 0 {
                white
            } else {
                hex(0x22252b)
            };
            paint.quad_up(p(a, 0.), p(a + w, 0.), p(a + w, 0.75), p(a, 0.75), c);
        }
    }
    let mut clay = clay.finish();
    clay.material = 0;
    let mut tarmac = tarmac.finish();
    tarmac.material = 1;
    let mut paint = paint.finish();
    paint.material = 2;
    let mut verge = verge.finish();
    verge.material = 3;
    let mut marks = skids(t);
    marks.material = 4;
    model(
        vec![clay, tarmac, paint, verge, marks],
        vec![
            textured_material(0, 0.95),
            textured_material(1, 0.85),
            MaterialData {
                metallic: 0.0,
                roughness: 0.7,
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
                            v3(t.at(i, l0 - 0.12), 0.045),
                            v3(t.at(i, l0 + 0.12), 0.045),
                            v3(t.at(j, l1 + 0.12), 0.045),
                            v3(t.at(j, l1 - 0.12), 0.045),
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
    let (cx, cz) = centre_of(t);
    let cell = 4.0;
    let (nx, nz) = (48usize, 42usize);
    let tile = 7.0;
    let x0 = cx - nx as f32 * cell * 0.5;
    let z0 = cz - nz as f32 * cell * 0.5;
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
            [k * (1.0 - 0.1 * woods), k, k * (0.95 + 0.08 * woods), 1.0]
        })
        .collect();
    let at = |i: usize, j: usize| tints[i * (nz + 1) + j];
    for i in 0..nx {
        for j in 0..nz {
            let (xa, za) = (x0 + i as f32 * cell, z0 + j as f32 * cell);
            let q = [
                Vec3::new(xa, 0., za),
                Vec3::new(xa + cell, 0., za),
                Vec3::new(xa + cell, 0., za + cell),
                Vec3::new(xa, 0., za + cell),
            ];
            let uv = q.map(|p| Vec2::new(p.x / tile, p.z / tile));
            b.quad_uvc(
                q,
                uv,
                [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)],
            );
        }
    }
    model(
        vec![b.finish()],
        vec![textured_material(0, 0.95)],
        &["grass.tex"],
    )
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
}

impl Prop {
    fn new(model: &'static str, at: Vec2, yaw: f32, scale: Vec3) -> Self {
        Self {
            model,
            at,
            yaw,
            scale,
            crown: None,
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
            out.push(Prop::new(
                "fence.model",
                a,
                math::atan2(d.y, d.x),
                Vec3::new(d.length() / 3.0, 1.0, 1.0),
            ));
            // Red and white boards in front of the fence round the outside of corners.
            let room = t.wall[i].min(t.wall[j]) > HALF + RUNOFF + 3.2;
            if room && t.bend[i] * s < -1.0 / 40.0 {
                let lat = s * (HALF + RUNOFF + 2.0);
                let (p, q) = (t.at(i, lat), t.at(j, lat));
                // The frame always behind the boards, away from the road.
                let (from, d) = if s > 0.0 { (q, p - q) } else { (p, q - p) };
                out.push(Prop::new(
                    "barrier.model",
                    from,
                    math::atan2(d.y, d.x),
                    Vec3::new(d.length() / 3.0, 1.0, 1.0),
                ));
            }
            i += every;
        }
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
fn blossom(b: &mut Builder, p: Vec2, r: f32, turn: f32, petal: [f32; 4], heart: [f32; 4]) {
    let y = 0.05;
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
    let (cx, cz) = centre_of(t);
    (Vec2::new(cx, cz), Vec2::new(48.0 * 4.0, 42.0 * 4.0))
}

/// The baked shade: one alpha-blended quad over the whole ground carrying the
/// trees' and fences' low-sun shadows, painted offline by art/src/shade.py.
pub fn shade_overlay(t: &Track) -> Model {
    let (c, size) = ground_rect(t);
    let mut b = Builder::new();
    let (x0, z0) = (c.x - size.x / 2.0, c.y - size.y / 2.0);
    let (x1, z1) = (c.x + size.x / 2.0, c.y + size.y / 2.0);
    b.quad_uv(
        [
            Vec3::new(x0, 0.09, z0),
            Vec3::new(x1, 0.09, z0),
            Vec3::new(x1, 0.09, z1),
            Vec3::new(x0, 0.09, z1),
        ],
        // Image rows run north to south: v grows with z.
        [
            Vec2::new(0., 0.),
            Vec2::new(1., 0.),
            Vec2::new(1., 1.),
            Vec2::new(0., 1.),
        ],
        [1.0; 4],
    );
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
