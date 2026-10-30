//! Generated geometry, flat-shaded with vertex colours: the ground, the road
//! with its run-off, kerbs and lines, the fences, the woods and the cars.
use crate::track::{Track, HALF, RUNOFF};
use exact_game::asset::MeshData;
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
    pub fn quad_up(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, color: [f32; 4]) {
        self.tri(a, b, c, color);
        self.tri(a, c, d, color);
    }
    /// An oriented box: centre, half extents along `x` (unit, horizontal) and its perpendicular.
    pub fn block(&mut self, centre: Vec3, half: Vec3, x: Vec2, color: [f32; 4]) {
        let ax = Vec3::new(x.x, 0.0, x.y) * half.x;
        let az = Vec3::new(-x.y, 0.0, x.x) * half.z;
        let ay = Vec3::Y * half.y;
        let corner = |sx: f32, sy: f32, sz: f32| centre + ax * sx + ay * sy + az * sz;
        let top = shade(color, 1.0);
        let sides = shade(color, 0.8);
        self.face(
            [
                corner(-1., 1., -1.),
                corner(1., 1., -1.),
                corner(1., 1., 1.),
                corner(-1., 1., 1.),
            ],
            Vec3::Y,
            top,
        );
        for (out, s) in [(ax, 1.0f32), (-ax, -1.0)] {
            self.face(
                [
                    corner(s, -1., -1.),
                    corner(s, 1., -1.),
                    corner(s, 1., 1.),
                    corner(s, -1., 1.),
                ],
                out,
                sides,
            );
        }
        for (out, s) in [(az, 1.0f32), (-az, -1.0)] {
            self.face(
                [
                    corner(-1., -1., s),
                    corner(1., -1., s),
                    corner(1., 1., s),
                    corner(-1., 1., s),
                ],
                out,
                sides,
            );
        }
    }
    /// A flat-shaded low-poly ball.
    pub fn ball(&mut self, c: Vec3, r: Vec3, color: [f32; 4], seed: u32) {
        let (rings, segs) = (4, 7);
        let p = |i: u32, j: u32| {
            let lat = std::f32::consts::PI * (i as f32 / rings as f32) - std::f32::consts::FRAC_PI_2;
            let lon = std::f32::consts::TAU * (j as f32 + 0.5 * (i % 2) as f32) / segs as f32;
            let (sl, cl) = math::sin_cos(lat);
            let (so, co) = math::sin_cos(lon);
            c + Vec3::new(cl * co * r.x, sl * r.y, cl * so * r.z)
        };
        for i in 0..rings {
            for j in 0..segs {
                let k = 0.82 + 0.3 * (i as f32 / rings as f32) + 0.1 * noise(i as i32, j as i32, seed);
                let col = shade(color, k);
                let (a, b) = (p(i, j), p(i, j + 1));
                let (cc, d) = (p(i + 1, j), p(i + 1, j + 1));
                self.face_any(a, b, d, c, col);
                self.face_any(a, d, cc, c, col);
            }
        }
    }
    /// A triangle facing away from `inside`.
    fn face_any(&mut self, a: Vec3, b: Vec3, c: Vec3, inside: Vec3, color: [f32; 4]) {
        let n = (b - a).cross(c - a);
        let (b, c) = if n.dot(a - inside) < 0.0 { (c, b) } else { (b, c) };
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
    /// A cone (or frustum) around Y, closed at the base.
    pub fn cone(&mut self, base: Vec3, r0: f32, h: f32, r1: f32, sides: u32, color: [f32; 4], twist: f32) {
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
        self.m.bounds = [self.lo.x, self.lo.y, self.lo.z, self.hi.x, self.hi.y, self.hi.z];
        self.m
    }
}

fn v3(p: Vec2, y: f32) -> Vec3 {
    Vec3::new(p.x, y, p.y)
}

/// A ribbon between two lateral offsets along the whole loop, coloured per segment.
fn ribbon(b: &mut Builder, t: &Track, from: f32, to: f32, y: f32, color: impl Fn(usize) -> Option<[f32; 4]>) {
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

/// The road: run-off, asphalt, edge lines, kerbs and the start line.
pub fn road(t: &Track) -> MeshData {
    let mut b = Builder::new();
    let n = t.len();
    let clay = hex(0xa4472f);
    ribbon(&mut b, t, -(HALF + RUNOFF), HALF + RUNOFF, 0.02, |i| {
        Some(shade(clay, 0.92 + 0.12 * noise(i as i32, 0, 3)))
    });
    let asphalt = hex(0x5d5f66);
    ribbon(&mut b, t, -HALF, HALF, 0.04, |i| {
        Some(shade(asphalt, 0.95 + 0.07 * noise(i as i32, 1, 5)))
    });
    let white = hex(0xf1efe8);
    for s in [-1.0f32, 1.0] {
        ribbon(&mut b, t, s * (HALF - 0.45), s * (HALF - 0.2), 0.05, |_| Some(white));
    }
    // Kerbs on both sides of every real corner, red and white in 1.5 m blocks.
    let red = hex(0xd83a2e);
    let kerb = |i: usize| (0..9).any(|k| t.bend[(i + n + k - 4) % n].abs() > 1.0 / 24.0);
    for s in [-1.0f32, 1.0] {
        ribbon(&mut b, t, s * (HALF - 0.2), s * (HALF + 0.9), 0.06, |i| {
            kerb(i).then(|| {
                if ((i as f32 * t.step / 1.5) as i32) % 2 == 0 {
                    red
                } else {
                    white
                }
            })
        });
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
            let c = if (k + row) % 2 == 0 { white } else { hex(0x22252b) };
            b.quad_up(p(a, 0.), p(a + w, 0.), p(a + w, 0.75), p(a, 0.75), c);
        }
    }
    b.finish()
}

/// The grass: a coarse grid with mottled colour, a little darker far out.
pub fn ground(t: &Track) -> MeshData {
    let mut b = Builder::new();
    let (cx, cz) = centre_of(t);
    let cell = 4.0;
    let (nx, nz) = (44, 38);
    let grass = hex(0x5b8a36);
    for i in 0..nx {
        for j in 0..nz {
            let x0 = cx - nx as f32 * cell * 0.5 + i as f32 * cell;
            let z0 = cz - nz as f32 * cell * 0.5 + j as f32 * cell;
            let k = 0.86 + 0.2 * noise(i, j, 11);
            let c = shade(grass, k);
            b.quad_up(
                Vec3::new(x0, 0., z0),
                Vec3::new(x0 + cell, 0., z0),
                Vec3::new(x0 + cell, 0., z0 + cell),
                Vec3::new(x0, 0., z0 + cell),
                c,
            );
        }
    }
    b.finish()
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

/// Fences, trees, bushes and flowers, in one mesh.
pub fn woods(w: &World, t: &Track) -> MeshData {
    let mut b = Builder::new();
    let n = t.len();
    let wood = hex(0x8a5530);
    let dark_wood = hex(0x6b3f22);
    // Fence posts every three metres and two rails, at the wall line.
    for s in [-1.0f32, 1.0] {
        let every = (3.0 / t.step).max(1.0) as usize;
        let mut i = 0;
        while i < n {
            let j = (i + every) % n;
            let a = t.at(i, s * (t.wall[i] - 0.2));
            let c = t.at(j, s * (t.wall[j] - 0.2));
            let along = (c - a).normalize_or_zero();
            b.block(v3(a, 0.55), Vec3::new(0.13, 0.55, 0.13), along, dark_wood);
            let mid = (a + c) * 0.5;
            let half_len = (c - a).length() * 0.5;
            for y in [0.45, 0.9] {
                b.block(v3(mid, y), Vec3::new(half_len, 0.07, 0.05), along, wood);
            }
            i += every;
        }
    }
    // Trees beyond the fences, denser further out.
    let (cx, cz) = centre_of(t);
    let pine = [hex(0x2a5e33), hex(0x356d36), hex(0x234f2e)];
    let leafy = [hex(0x4a7f34), hex(0x5f8c38), hex(0x3a6f33)];
    let mut placed: Vec<Vec2> = Vec::new();
    for _ in 0..1400 {
        let p = Vec2::new(
            w.rand(cx - 92.0..cx + 92.0),
            w.rand(cz - 78.0..cz + 78.0),
        );
        let clear = clearance(t, p);
        if clear < HALF + 8.0 || placed.iter().any(|q| (*q - p).length() < 3.2) {
            continue;
        }
        let near_edge = clear < HALF + 16.0;
        if near_edge && !w.chance(0.55) {
            continue;
        }
        placed.push(p);
        let scale = w.rand(0.8..1.35);
        let twist = w.rand(0.0..1.0);
        if w.chance(0.55) {
            let c = pine[placed.len() % 3];
            b.cone(v3(p, 0.0), 0.25 * scale, 1.6 * scale, 0.2 * scale, 5, hex(0x5a3a22), twist);
            b.cone(v3(p, 1.0 * scale), 2.2 * scale, 3.2 * scale, 0.0, 7, c, twist);
            b.cone(v3(p, 2.6 * scale), 1.6 * scale, 2.8 * scale, 0.0, 7, shade(c, 1.12), twist);
        } else {
            let c = leafy[placed.len() % 3];
            b.cone(v3(p, 0.0), 0.3 * scale, 2.0 * scale, 0.22 * scale, 5, hex(0x5a3a22), twist);
            b.ball(v3(p, 3.0 * scale), Vec3::splat(2.2 * scale), c, placed.len() as u32);
            let off = Vec2::new(1.1, 0.6) * scale;
            b.ball(v3(p + off, 2.5 * scale), Vec3::splat(1.5 * scale), shade(c, 1.1), placed.len() as u32 + 7);
        }
    }
    // Bushes and flowers on the grass between road and fence, and in the infield.
    let flower = [hex(0xffffff), hex(0xf6e7a1), hex(0xf2c6d6)];
    for k in 0..900 {
        let p = Vec2::new(
            w.rand(cx - 70.0..cx + 70.0),
            w.rand(cz - 60.0..cz + 60.0),
        );
        let clear = clearance(t, p);
        if clear < HALF + RUNOFF + 0.6 {
            continue;
        }
        if k % 9 == 0 && clear > HALF + RUNOFF + 2.5 {
            b.ball(v3(p, 0.35), Vec3::new(0.9, 0.6, 0.9), hex(0x4c8a34), k);
        } else {
            let c = flower[k as usize % 3];
            let r = 0.18;
            b.quad_up(
                Vec3::new(p.x - r, 0.03, p.y - r),
                Vec3::new(p.x + r, 0.03, p.y - r),
                Vec3::new(p.x + r, 0.03, p.y + r),
                Vec3::new(p.x - r, 0.03, p.y + r),
                c,
            );
        }
    }
    b.finish()
}

/// A chunky little car, nose along +X. `paint` is the body, `trim` the stripes and roof.
pub fn car(paint: u32, trim: u32) -> MeshData {
    let mut b = Builder::new();
    let x = Vec2::X;
    let body = hex(paint);
    let accent = hex(trim);
    let tyre = hex(0x1b1c20);
    let glass = hex(0x1f2a36);
    // Wheels.
    for (wx, wz) in [(0.95, 0.74), (0.95, -0.74), (-0.95, 0.74), (-0.95, -0.74)] {
        b.block(Vec3::new(wx, 0.31, wz), Vec3::new(0.32, 0.31, 0.17), x, tyre);
    }
    // Lower body and the sloped nose.
    b.block(Vec3::new(0.0, 0.5, 0.0), Vec3::new(1.5, 0.22, 0.76), x, body);
    b.block(Vec3::new(1.2, 0.45, 0.0), Vec3::new(0.38, 0.12, 0.72), x, shade(body, 0.9));
    // Cabin: glass with a painted roof.
    b.block(Vec3::new(-0.25, 0.86, 0.0), Vec3::new(0.62, 0.16, 0.62), x, glass);
    b.block(Vec3::new(-0.3, 1.04, 0.0), Vec3::new(0.5, 0.03, 0.56), x, accent);
    // Racing stripes over the bonnet and boot.
    for z in [-0.18, 0.18] {
        b.block(Vec3::new(0.85, 0.73, z), Vec3::new(0.62, 0.012, 0.09), x, accent);
        b.block(Vec3::new(-1.15, 0.73, z), Vec3::new(0.32, 0.012, 0.09), x, accent);
    }
    // Spoiler.
    b.block(Vec3::new(-1.38, 0.98, 0.0), Vec3::new(0.14, 0.03, 0.74), x, accent);
    for z in [-0.55, 0.55] {
        b.block(Vec3::new(-1.38, 0.84, z), Vec3::new(0.06, 0.12, 0.04), x, tyre);
    }
    // Headlights.
    for z in [-0.48, 0.48] {
        b.block(Vec3::new(1.5, 0.52, z), Vec3::new(0.03, 0.06, 0.13), x, hex(0xfff3c4));
    }
    b.finish()
}

/// The player's marker: a downward chevron that floats over the car.
pub fn marker() -> MeshData {
    let mut b = Builder::new();
    b.cone(Vec3::new(0.0, 0.0, 0.0), 0.0, 1.1, 0.7, 4, hex(0xfff1a8), 0.785);
    b.cone(Vec3::new(0.0, 1.1, 0.0), 0.7, 0.01, 0.0, 4, hex(0xfff1a8), 0.785);
    b.finish()
}
