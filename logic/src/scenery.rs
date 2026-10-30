//! Generated geometry: the textured ground and road with its kerbs and lines,
//! flowers, the player's marker, and where the Blender models stand.
use crate::track::{Track, HALF, RUNOFF, WALL};
use exact_game::asset::{MaterialData, MeshData, Model, Node};
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
            self.m.colors.extend(color);
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
    let n = t.len();
    // The loop's length in whole tiles, so the texture closes without a seam.
    let tiles = (t.length() / tile).round().max(1.0);
    let along = |i: usize| i as f32 / n as f32 * tiles;
    for i in 0..n {
        let j = (i + 1) % n;
        let (v0, v1) = (along(i), along(i) + tiles / n as f32);
        b.quad_uv(
            [
                v3(t.at(i, from), y),
                v3(t.at(i, to), y),
                v3(t.at(j, to), y),
                v3(t.at(j, from), y),
            ],
            [
                Vec2::new(from / tile, v0),
                Vec2::new(to / tile, v0),
                Vec2::new(to / tile, v1),
                Vec2::new(from / tile, v1),
            ],
            tint(i),
        );
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
    textured(&mut tarmac, t, -HALF, HALF, 0.04, 4.6, |i| {
        // Rubbered-in darker where the cars run, lighter near the edges is in the texture.
        shade([1.0; 4], 0.96 + 0.06 * noise(i as i32 / 6, 1, 5))
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
    // Kerbs on both sides of every real corner, red and white in 1.5 m blocks.
    let red = hex(0xd83a2e);
    let kerb = |i: usize| (0..9).any(|k| t.bend[(i + n + k - 4) % n].abs() > 1.0 / 24.0);
    for s in [-1.0f32, 1.0] {
        ribbon(
            &mut paint,
            t,
            s * (HALF - 0.15),
            s * (HALF + 0.6),
            0.06,
            |i| {
                kerb(i).then(|| {
                    if ((i as f32 * t.step / 1.5) as i32) % 2 == 0 {
                        red
                    } else {
                        white
                    }
                })
            },
        );
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
    model(
        vec![clay, tarmac, paint, verge],
        vec![
            textured_material(0, 0.95),
            textured_material(1, 0.85),
            MaterialData {
                metallic: 0.0,
                roughness: 0.7,
                ..MaterialData::default()
            },
            textured_material(2, 0.95),
        ],
        &["clay.tex", "asphalt.tex", "grass.tex"],
    )
}

/// The grass: a textured grid, tinted in broad patches so the tiling never shows.
pub fn ground(t: &Track) -> Model {
    let mut b = Builder::new();
    let (cx, cz) = centre_of(t);
    let cell = 4.0;
    let (nx, nz) = (48, 42);
    let tile = 7.0;
    for i in 0..nx {
        for j in 0..nz {
            let x0 = cx - nx as f32 * cell * 0.5 + i as f32 * cell;
            let z0 = cz - nz as f32 * cell * 0.5 + j as f32 * cell;
            let k = 0.86 + 0.22 * noise(i / 3, j / 3, 11) + 0.06 * noise(i, j, 12);
            let c = [
                k,
                k * (0.98 + 0.04 * noise(i / 4, j / 4, 13)),
                k * 0.95,
                1.0,
            ];
            let q = [
                Vec3::new(x0, 0., z0),
                Vec3::new(x0 + cell, 0., z0),
                Vec3::new(x0 + cell, 0., z0 + cell),
                Vec3::new(x0, 0., z0 + cell),
            ];
            let uv = q.map(|p| Vec2::new(p.x / tile, p.z / tile));
            b.quad_uv(q, uv, c);
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
            out.push(Prop {
                model: "fence.model",
                at: a,
                yaw: math::atan2(d.y, d.x),
                scale: Vec3::new(d.length() / 3.0, 1.0, 1.0),
            });
            i += every;
        }
    }
    let (cx, cz) = centre_of(t);
    let mut placed: Vec<(Vec2, f32)> = Vec::new();
    let free = |placed: &[(Vec2, f32)], p: Vec2, r: f32| {
        placed.iter().all(|(q, s)| (*q - p).length() >= r + s)
    };
    // Woods: conifers crowd up to the fences, the odd broadleaf among them.
    // Low quality keeps the fences and a thinner wood, and skips the undergrowth.
    let (trees, undergrowth) = if quality == 0 { (700, 0) } else { (1800, 1400) };
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
        out.push(Prop {
            model,
            at: p,
            yaw: w.rand(0.0..std::f32::consts::TAU),
            scale: Vec3::splat(scale),
        });
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
                ("daisies.model", 0.6),
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
        out.push(Prop {
            model,
            at: p,
            yaw: w.rand(0.0..std::f32::consts::TAU),
            scale: Vec3::splat(w.rand(0.8..1.2)),
        });
    }
    out
}

/// Flowers dotted over the grass, as one generated mesh.
pub fn flowers(w: &World, t: &Track) -> MeshData {
    let mut b = Builder::new();
    let (cx, cz) = centre_of(t);
    let flower = [hex(0xffffff), hex(0xf6e7a1), hex(0xf2c6d6), hex(0xffffff)];
    for k in 0..900 {
        let p = Vec2::new(w.rand(cx - 75.0..cx + 75.0), w.rand(cz - 65.0..cz + 65.0));
        if clearance(t, p) < HALF + RUNOFF + 0.6 {
            continue;
        }
        let c = flower[k % 4];
        let r = 0.14;
        b.quad_up(
            Vec3::new(p.x - r, 0.03, p.y - r),
            Vec3::new(p.x + r, 0.03, p.y - r),
            Vec3::new(p.x + r, 0.03, p.y + r),
            Vec3::new(p.x - r, 0.03, p.y + r),
            c,
        );
    }
    b.finish()
}

/// The player's marker: a downward chevron that floats over the car.
pub fn marker() -> MeshData {
    let mut b = Builder::new();
    b.pyramid(Vec3::new(0.0, 0.0, 0.0), 0.0, 0.8, 0.45, hex(0xfff1a8));
    b.pyramid(Vec3::new(0.0, 0.8, 0.0), 0.45, 0.01, 0.0, hex(0xfff1a8));
    b.finish()
}
