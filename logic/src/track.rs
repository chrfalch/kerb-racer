//! The circuits: a closed Catmull-Rom spline resampled every metre or so, its
//! racing line (the shortest-curvature path inside the kerbs), the speed that
//! line allows at every sample, and where the fences stand.
use exact_game::*;

/// Asphalt half-width in metres.
pub const HALF: f32 = 4.6;
/// Red run-off beyond the asphalt edge.
pub const RUNOFF: f32 = 1.6;
/// Fence distance from the centreline where nothing else is near.
pub const WALL: f32 = HALF + 5.4;
/// Lateral grip the planner assumes on asphalt (m/s²); the cars have a little more.
pub const PLAN_GRIP: f32 = 14.5;
/// Deceleration the planner allows itself when braking for a corner.
pub const PLAN_BRAKE: f32 = 17.0;
/// Top speed on the straight (m/s).
pub const VMAX: f32 = 31.0;
/// On dirt the planner expects this share of its asphalt grip and braking.
pub const PLAN_DIRT: f32 = 0.7;
/// Gravity (m/s²): one and a half times the real thing, so jumps are short,
/// snappy hops and hills pull hard.
pub const G: f32 = 9.81 * 1.5;
/// Out to here from the centreline the ground is the road's own surface: the
/// asphalt or dirt and the run-off, banked as one plane.
pub const BAND: f32 = HALF + RUNOFF;
/// Beyond the band the surface eases into the land over this many metres.
pub const BLEND: f32 = 1.5;
/// The land mesh lies this far under the road, so the road never sinks into it.
pub const SINK: f32 = 0.05;
/// The land's grid spacing (m).
pub const CELL: f32 = 2.0;
/// The land's extent in cells, centred on the circuit.
pub const CELLS: (usize, usize) = (96, 84);

/// One authored piece of elevation along the lap, placed by distance from the
/// start line (`at`, metres) so it stays with the corner it belongs to.
#[derive(Clone, Copy, Debug)]
pub enum Lift {
    /// A smooth climb (or descent, when `rise` is negative) over `len` metres;
    /// the road stays at the new level after it. A lap's rises must sum to zero.
    Slope { at: f32, len: f32, rise: f32 },
    /// A crest (`h` > 0) or a dip (`h` < 0): up over `up` metres, down over
    /// `down`. A short, steep `down` after a crest is a kicker the cars fly off.
    Crest { at: f32, up: f32, down: f32, h: f32 },
    /// A run of bumps `wave` metres apart and `h` high, easing in and out.
    Whoops { at: f32, len: f32, wave: f32, h: f32 },
    /// Banking into the corner (`slope` > 0, rise per metre toward the outside)
    /// or off-camber (< 0), easing in and out over the first and last quarter.
    Bank { at: f32, len: f32, slope: f32 },
}

pub struct Layout {
    pub name: &'static str,
    /// A loose dirt road with gravel run-off instead of asphalt and kerbs.
    pub dirt: bool,
    /// Metres from the first point to the start line, so the grid sits on a straight.
    pub start: f32,
    pub points: &'static [(f32, f32)],
    pub lifts: &'static [Lift],
    /// Height (m) of the low rolling hills in the land around the circuit.
    pub hills: f32,
}

pub const LAYOUTS: [Layout; 4] = [
    Layout {
        name: "Pine Hollow",
        dirt: false,
        start: 42.,
        points: &[
            (-34., 22.),
            (0., 23.),
            (24., 20.),
            (37., 10.),
            (35., -3.),
            (20., -7.),
            (8., -3.),
            (-3., -10.),
            (0., -24.),
            (22., -28.),
            (40., -36.),
            (34., -54.),
            (0., -54.),
            (-28., -48.),
            (-40., -32.),
            (-36., -14.),
            (-44., 4.),
        ],
        // Placeholder elevation to exercise the physics; the real profiles come next.
        lifts: &[
            Lift::Slope { at: 108., len: 42., rise: 5.0 },
            Lift::Slope { at: 182., len: 54., rise: -5.0 },
            Lift::Crest { at: 318., up: 14., down: 15., h: 1.2 },
        ],
        hills: 3.0,
    },
    Layout {
        name: "Fern Loop",
        dirt: false,
        start: 44.,
        points: &[
            (-38., 14.),
            (-6., 22.),
            (24., 20.),
            (38., 4.),
            (33., -14.),
            (14., -17.),
            (8., -29.),
            (22., -40.),
            (24., -54.),
            (4., -62.),
            (-24., -50.),
            (-30., -26.),
            (-44., -8.),
        ],
        lifts: &[],
        hills: 0.0,
    },
    Layout {
        name: "Kerb Ring",
        dirt: false,
        start: 40.,
        points: &[
            (-30., 18.),
            (8., 18.),
            (30., 10.),
            (32., -6.),
            (14., -11.),
            (0., -2.),
            (-12., -12.),
            (-8., -28.),
            (18., -30.),
            (31., -42.),
            (22., -56.),
            (0., -60.),
            (-26., -50.),
            (-38., -28.),
            (-40., 0.),
        ],
        lifts: &[],
        hills: 0.0,
    },
    Layout {
        name: "Gravel Pit",
        dirt: true,
        start: 24.,
        points: &[
            (-31., 24.),
            (-33., 4.),
            (-31., -20.),
            (-26., -44.),
            (-14., -58.),
            (2., -54.),
            (4., -40.),
            (16., -32.),
            (14., -18.),
            (26., -10.),
            (42., -8.),
            (46., 8.),
            (34., 22.),
            (18., 20.),
            (8., 32.),
            (-8., 40.),
            (-24., 40.),
        ],
        // Placeholder elevation to exercise the physics; the real profiles come next.
        lifts: &[
            Lift::Whoops { at: 2., len: 30., wave: 7.5, h: 0.4 },
            Lift::Slope { at: 126., len: 26., rise: -3.5 },
            Lift::Slope { at: 162., len: 28., rise: 3.5 },
            Lift::Crest { at: 214., up: 7., down: 12., h: 1.0 },
            Lift::Bank { at: 246., len: 26., slope: 0.14 },
        ],
        hills: 3.5,
    },
];

pub fn layout(track: u32) -> &'static Layout {
    &LAYOUTS[track as usize % LAYOUTS.len()]
}

/// The sampled circuit. Index 0 is the start/finish line; samples run in the
/// racing direction.
#[derive(Clone, Debug, Default, Resource)]
pub struct Track {
    pub id: u32,
    /// Loose dirt and gravel rather than asphalt (see `Layout::dirt`).
    pub dirt: bool,
    pub x: Vec<f32>,
    pub z: Vec<f32>,
    /// Unit left normal at each sample.
    pub nx: Vec<f32>,
    pub nz: Vec<f32>,
    /// Racing line: lateral offset along the normal.
    pub line: Vec<f32>,
    /// Fastest speed the racing line allows here, braking for what follows.
    pub speed: Vec<f32>,
    /// Signed curvature of the centreline (left positive), for kerbs.
    pub bend: Vec<f32>,
    /// Lateral distance to the fence.
    pub wall: Vec<f32>,
    /// Arc length between samples.
    pub step: f32,
    /// Centreline height (m), its slope along the road (dy/ds) and its
    /// vertical curvature (d²y/ds²: negative over a crest, positive in a dip).
    pub y: Vec<f32>,
    pub grade: Vec<f32>,
    pub vbend: Vec<f32>,
    /// Cross-slope: height gained per metre to the left.
    pub bank: Vec<f32>,
    /// How far to the left and to the right the road's own plane reaches: on
    /// the inside of a hairpin tighter than `BAND` the sample frames cross over
    /// each other, so past this the land (smooth there) carries the road.
    pub reach_left: Vec<f32>,
    pub reach_right: Vec<f32>,
    /// The land: heights at the corners of a `CELL`-metre grid from `(gx, gz)`,
    /// `CELLS.1 + 1` per column. Under the road it is the road's height less `SINK`.
    pub gx: f32,
    pub gz: f32,
    pub land: Vec<f32>,
}

impl Track {
    pub fn len(&self) -> usize {
        self.x.len()
    }
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }
    pub fn length(&self) -> f32 {
        self.step * self.len() as f32
    }
    pub fn wrap(&self, i: i64) -> usize {
        i.rem_euclid(self.len() as i64) as usize
    }
    pub fn centre(&self, i: usize) -> Vec2 {
        Vec2::new(self.x[i], self.z[i])
    }
    pub fn normal(&self, i: usize) -> Vec2 {
        Vec2::new(self.nx[i], self.nz[i])
    }
    pub fn tangent(&self, i: usize) -> Vec2 {
        Vec2::new(self.nz[i], -self.nx[i])
    }
    /// The point `offset` metres left of the centreline at `i`.
    pub fn at(&self, i: usize, offset: f32) -> Vec2 {
        self.centre(i) + self.normal(i) * offset
    }
    /// The nearest sample within `reach` samples of `hint`, and the signed lateral offset there.
    pub fn locate(&self, p: Vec2, hint: usize, reach: i64) -> (usize, f32) {
        let mut best = (hint, f32::MAX);
        for k in -reach..=reach {
            let i = self.wrap(hint as i64 + k);
            let d = (self.centre(i) - p).length_squared();
            if d < best.1 {
                best = (i, d);
            }
        }
        let i = best.0;
        (i, (p - self.centre(i)).dot(self.normal(i)))
    }
    /// Nearest sample anywhere (setup and recovery only).
    pub fn locate_all(&self, p: Vec2) -> (usize, f32) {
        let n = self.len() as i64;
        self.locate(p, 0, n / 2)
    }

    /// Where `p` lies along the road near sample `i`: the segment it is on
    /// (`a`, `a + 1`), how far along it (0..1), and its lateral offset. The
    /// sample frames are blended, so this is continuous as `p` moves, even
    /// out at the fences in a tight corner.
    pub fn frame(&self, p: Vec2, i: usize) -> (usize, usize, f32, f32) {
        let along = |k: usize| (p - self.centre(k)).dot(self.tangent(k));
        // The segment whose two normals straddle `p`. Inside a tight corner the
        // nearest sample isn't always one of them, so walk to it.
        let mut a = i;
        if along(a) >= 0.0 {
            for _ in 0..12 {
                let next = self.wrap(a as i64 + 1);
                if along(next) < 0.0 {
                    break;
                }
                a = next;
            }
        } else {
            for _ in 0..12 {
                a = self.wrap(a as i64 - 1);
                if along(a) >= 0.0 {
                    break;
                }
            }
        }
        let b = self.wrap(a as i64 + 1);
        let (da, db) = (along(a), along(b));
        let u = if da - db > 1e-6 {
            (da / (da - db)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let la = (p - self.centre(a)).dot(self.normal(a));
        let lb = (p - self.centre(b)).dot(self.normal(b));
        (a, b, u, la + (lb - la) * u)
    }

    /// The road's own surface at `p` near sample `i`, its plane carried on
    /// past the band: the centreline's height between samples (a cubic that
    /// keeps the slope continuous), tilted by the bank.
    fn road_height(&self, a: usize, b: usize, u: f32, lat: f32) -> f32 {
        let (u2, u3) = (u * u, u * u * u);
        let (ma, mb) = (self.grade[a] * self.step, self.grade[b] * self.step);
        let y = (2. * u3 - 3. * u2 + 1.) * self.y[a]
            + (u3 - 2. * u2 + u) * ma
            + (-2. * u3 + 3. * u2) * self.y[b]
            + (u3 - u2) * mb;
        y + (self.bank[a] + (self.bank[b] - self.bank[a]) * u) * lat
    }

    /// The ground the cars drive on at `p`, given its nearest sample `i`: the
    /// road out to `BAND`, easing into the land beyond.
    pub fn surface(&self, p: Vec2, i: usize) -> f32 {
        let (a, b, u, lat) = self.frame(p, i);
        let road = self.road_height(a, b, u, lat);
        let off = lat.abs();
        let reach = self.reach_at(a, b, u, lat);
        if off <= reach || self.land.is_empty() {
            return road;
        }
        let k = smooth(reach, reach + BLEND, off);
        // Under the road the land lies `SINK` low; the road's surface doesn't.
        let land = self.land_at(p) + SINK * (1.0 - smooth(BAND, BAND + BLEND, off));
        road + (land - road) * k
    }

    /// The road plane's reach on `lat`'s side, between samples `a` and `b`.
    fn reach_at(&self, a: usize, b: usize, u: f32, lat: f32) -> f32 {
        let side = if lat > 0.0 { &self.reach_left } else { &self.reach_right };
        side[a] + (side[b] - side[a]) * u
    }

    /// The surface right at lateral offset `lat` from sample `i` (for meshes).
    pub fn surface_at(&self, i: usize, lat: f32) -> f32 {
        self.surface(self.at(i, lat), i)
    }

    /// The ground's height at `p`, searching `hint`'s neighbourhood for the road.
    pub fn height(&self, p: Vec2, hint: usize) -> f32 {
        let (i, _) = self.locate(p, hint, 6);
        self.surface(p, i)
    }

    /// The ground's height anywhere (setup only: it searches the whole lap).
    pub fn height_any(&self, p: Vec2) -> f32 {
        let (i, _) = self.locate_all(p);
        self.surface(p, i)
    }

    /// The ground's slope at `p` (dy/dx, dy/dz), by central differences.
    pub fn gradient(&self, p: Vec2, hint: usize) -> Vec2 {
        let e = 0.4;
        let h = |d: Vec2| self.height(p + d, hint);
        Vec2::new(
            h(Vec2::new(e, 0.)) - h(Vec2::new(-e, 0.)),
            h(Vec2::new(0., e)) - h(Vec2::new(0., -e)),
        ) / (2. * e)
    }

    /// One corner of the land grid, clamped to its edge.
    pub fn land_node(&self, i: i64, j: i64) -> f32 {
        let i = i.clamp(0, CELLS.0 as i64) as usize;
        let j = j.clamp(0, CELLS.1 as i64) as usize;
        self.land[i * (CELLS.1 + 1) + j]
    }

    /// The land between its grid corners: Catmull-Rom both ways, so its slope
    /// is continuous and a car rolling over it doesn't kick at every cell edge.
    pub fn land_at(&self, p: Vec2) -> f32 {
        let fx = (p.x - self.gx) / CELL;
        let fz = (p.y - self.gz) / CELL;
        let (i, j) = (math::floor(fx), math::floor(fz));
        let (u, v) = (fx - i, fz - j);
        let (i, j) = (i as i64, j as i64);
        let cubic = |p0: f32, p1: f32, p2: f32, p3: f32, t: f32| {
            p1 + 0.5
                * t
                * (p2 - p0 + t * (2. * p0 - 5. * p1 + 4. * p2 - p3 + t * (3. * (p1 - p2) + p3 - p0)))
        };
        let row = |k: i64| {
            cubic(
                self.land_node(i - 1, j + k),
                self.land_node(i, j + k),
                self.land_node(i + 1, j + k),
                self.land_node(i + 2, j + k),
                u,
            )
        };
        cubic(row(-1), row(0), row(1), row(2), v)
    }
}

pub fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 0 → 1 along a half cosine; flat at both ends.
fn ease(u: f32) -> f32 {
    0.5 - 0.5 * math::cos(std::f32::consts::PI * u.clamp(0.0, 1.0))
}

/// The authored height of the centreline `s` metres past the start line.
pub fn lift_height(lifts: &[Lift], s: f32) -> f32 {
    let mut y = 0.0;
    for lift in lifts {
        y += match *lift {
            Lift::Slope { at, len, rise } => rise * ease((s - at) / len),
            Lift::Crest { at, up, down, h } => {
                if s < at || s > at + up + down {
                    0.0
                } else if s < at + up {
                    h * ease((s - at) / up)
                } else {
                    h * (1.0 - ease((s - at - up) / down))
                }
            }
            Lift::Whoops { at, len, wave, h } => {
                if s < at || s > at + len {
                    0.0
                } else {
                    let u = (s - at) / len;
                    let envelope = ease(u * 4.0) * ease((1.0 - u) * 4.0);
                    let bumps = 0.5 - 0.5 * math::cos(std::f32::consts::TAU * (s - at) / wave);
                    h * envelope * bumps
                }
            }
            Lift::Bank { .. } => 0.0,
        };
    }
    y
}

/// The authored banking `s` metres past the start line (rise per metre toward the outside).
fn lift_bank(lifts: &[Lift], s: f32) -> f32 {
    let mut bank = 0.0;
    for lift in lifts {
        if let Lift::Bank { at, len, slope } = *lift {
            if s >= at && s <= at + len {
                let u = (s - at) / len;
                bank += slope * ease(u * 4.0) * ease((1.0 - u) * 4.0);
            }
        }
    }
    bank
}

/// Smooth value noise in about -1..1, for the land's rolling hills.
fn hills(p: Vec2, every: f32, salt: u32) -> f32 {
    let hash = |a: i32, b: i32| {
        let mut h = (a as u32).wrapping_mul(0x8da6_b343) ^ (b as u32).wrapping_mul(0xd816_3841) ^ salt.wrapping_mul(0xcb1a_b31f);
        h ^= h >> 13;
        h = h.wrapping_mul(0x5bd1_e995);
        h ^= h >> 15;
        (h & 0xffff) as f32 / 65535.0 * 2.0 - 1.0
    };
    let (x, z) = (p.x / every, p.y / every);
    let (i, j) = (math::floor(x), math::floor(z));
    let (u, v) = (smooth(0.0, 1.0, x - i), smooth(0.0, 1.0, z - j));
    let (i, j) = (i as i32, j as i32);
    let a = hash(i, j) + (hash(i + 1, j) - hash(i, j)) * u;
    let b = hash(i, j + 1) + (hash(i + 1, j + 1) - hash(i, j + 1)) * u;
    a + (b - a) * v
}

/// The land round the circuit. Under the road and its run-off the grid takes
/// the road's height (less `SINK`); everywhere else it relaxes between the
/// pieces of road near it, settling into the low hills farther out, so the
/// ground between two parts of the track at different heights is a smooth
/// bank. Gauss-Seidel sweeps of a screened Laplace equation do the relaxing.
fn land(t: &Track, rolling: f32) -> (f32, f32, Vec<f32>) {
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for i in 0..t.len() {
        lo = lo.min(t.centre(i));
        hi = hi.max(t.centre(i));
    }
    let c = (lo + hi) * 0.5;
    let (nx, nz) = CELLS;
    let (gx, gz) = (c.x - nx as f32 * CELL * 0.5, c.y - nz as f32 * CELL * 0.5);
    let level = t.y.iter().sum::<f32>() / t.len().max(1) as f32;
    let rows = nz + 1;
    let mut h = vec![0f32; (nx + 1) * rows];
    let mut fixed = vec![false; h.len()];
    let mut rest = vec![0f32; h.len()];
    for i in 0..=nx {
        for j in 0..=nz {
            let p = Vec2::new(gx + i as f32 * CELL, gz + j as f32 * CELL);
            let k = i * rows + j;
            rest[k] = level + rolling * (0.7 * hills(p, 38.0, 1) + 0.3 * hills(p, 17.0, 2));
            let (near, _) = t.locate_all(p);
            let (a, b, u, lat) = t.frame(p, near);
            // Pin under the road plane wherever it holds (a little past the
            // run-off). On the inside of a tight corner, where it doesn't, pin
            // to the nearest sample's height: the grid smooths the steps.
            let reach = t.reach_at(a, b, u, lat);
            let off = (p - t.centre(near)).length();
            if lat.abs() <= reach.min(BAND + 0.6) {
                fixed[k] = true;
                h[k] = t.road_height(a, b, u, lat) - SINK;
            } else if off <= BAND + 0.6 {
                fixed[k] = true;
                h[k] = t.y[near] - SINK;
            } else {
                h[k] = rest[k];
            }
        }
    }
    // How strongly the land pulls back to its own hills: about 15 m from the
    // road it is mostly its own, so the banks down from a raised road (or up
    // from a sunken one) stay gentle enough to drive over.
    let pull = 0.02;
    for _ in 0..400 {
        for i in 0..=nx {
            for j in 0..=nz {
                let k = i * rows + j;
                if fixed[k] {
                    continue;
                }
                let at = |a: usize, b: usize| h[a * rows + b];
                let sum = at(i.saturating_sub(1), j)
                    + at((i + 1).min(nx), j)
                    + at(i, j.saturating_sub(1))
                    + at(i, (j + 1).min(nz));
                h[k] = (sum + pull * rest[k]) / (4.0 + pull);
            }
        }
    }
    (gx, gz, h)
}

fn catmull(points: &[(f32, f32)], per: u32) -> Vec<Vec2> {
    let n = points.len();
    let p = |i: usize| Vec2::new(points[i % n].0, points[i % n].1);
    let mut out = Vec::new();
    for i in 0..n {
        let (p0, p1, p2, p3) = (p(i + n - 1), p(i), p(i + 1), p(i + 2));
        for k in 0..per {
            let t = k as f32 / per as f32;
            let (t2, t3) = (t * t, t * t * t);
            out.push(
                0.5 * (2. * p1
                    + (p2 - p0) * t
                    + (2. * p0 - 5. * p1 + 4. * p2 - p3) * t2
                    + (3. * p1 - p0 - 3. * p2 + p3) * t3),
            );
        }
    }
    out
}

/// Radius of the circle through three points; infinite when they line up.
pub fn radius(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    let cross = (b - a).perp_dot(c - a);
    if cross.abs() < 1e-6 {
        return f32::INFINITY;
    }
    (b - a).length() * (c - b).length() * (c - a).length() / (2. * cross)
}

pub fn build(id: u32) -> Track {
    let poly = catmull(layout(id).points, 32);
    // Resample by arc length to about one metre.
    let mut cumulative = vec![0f32];
    for i in 0..poly.len() {
        let next = poly[(i + 1) % poly.len()];
        cumulative.push(cumulative[i] + (next - poly[i]).length());
    }
    let total = *cumulative.last().unwrap();
    let n = (total / 1.0).round() as usize;
    let step = total / n as f32;
    let mut centre = Vec::with_capacity(n);
    let mut j = 0;
    for i in 0..n {
        let s = i as f32 * step;
        while cumulative[j + 1] < s {
            j += 1;
        }
        let t = (s - cumulative[j]) / (cumulative[j + 1] - cumulative[j]);
        let (a, b) = (poly[j], poly[(j + 1) % poly.len()]);
        centre.push(a + (b - a) * t);
    }
    centre.rotate_left((layout(id).start / step).round() as usize % n);
    // Left of travel: the tangent turned a quarter anticlockwise in (x, z).
    let normals: Vec<Vec2> = (0..n)
        .map(|i| {
            (centre[(i + 1) % n] - centre[(i + n - 1) % n])
                .normalize()
                .perp()
        })
        .collect();
    let bend: Vec<f32> = (0..n)
        .map(|i| {
            let r = radius(centre[(i + n - 4) % n], centre[i], centre[(i + 4) % n]);
            if r.is_finite() {
                1. / r
            } else {
                0.
            }
        })
        .collect();

    // Racing line: relax each point toward the midpoint of its neighbours,
    // projected on its normal and clamped inside the kerbs.
    let limit = HALF - 1.3;
    let mut line = vec![0f32; n];
    for pass in 0..900 {
        let reach = if pass < 600 { 6 } else { 2 };
        for i in 0..n {
            let p = |k: usize, line: &[f32]| centre[k] + normals[k] * line[k];
            let (a, b) = (p((i + n - reach) % n, &line), p((i + reach) % n, &line));
            let here = p(i, &line);
            let target = (a + b) * 0.5;
            line[i] = (line[i] + (target - here).dot(normals[i]) * 0.5).clamp(-limit, limit);
        }
    }
    let path: Vec<Vec2> = (0..n).map(|i| centre[i] + normals[i] * line[i]).collect();
    let dirt = layout(id).dirt;

    // Elevation, from the layout's lifts by distance past the start line.
    let lifts = layout(id).lifts;
    let length = step * n as f32;
    for lift in lifts {
        let (at, len) = match *lift {
            Lift::Slope { at, len, .. } | Lift::Whoops { at, len, .. } | Lift::Bank { at, len, .. } => {
                (at, len)
            }
            Lift::Crest { at, up, down, .. } => (at, up + down),
        };
        assert!(
            at >= 0.0 && at + len <= length,
            "{}: a lift at {at} m runs past the start line ({length:.0} m)",
            layout(id).name
        );
    }
    let height = |s: f32| lift_height(lifts, s);
    assert!(
        (height(length) - height(0.0)).abs() < 1e-3,
        "{}: the slopes don't add up to zero round the lap",
        layout(id).name
    );
    let y: Vec<f32> = (0..n).map(|i| height(i as f32 * step)).collect();
    let grade: Vec<f32> = (0..n)
        .map(|i| {
            let s = i as f32 * step;
            (height(s + 0.25) - height(s - 0.25)) / 0.5
        })
        .collect();
    let vbend: Vec<f32> = (0..n)
        .map(|i| {
            let s = i as f32 * step;
            height(s + 1.0) - 2.0 * height(s) + height(s - 1.0)
        })
        .collect();
    // Banking tilts toward the inside of the corner, whichever way it turns.
    let bank: Vec<f32> = (0..n)
        .map(|i| {
            let turn: f32 = (0..21).map(|k| bend[(i + n + k - 10) % n]).sum();
            -lift_bank(lifts, i as f32 * step) * turn.signum()
        })
        .collect();

    // The inside of a corner is only the road's own plane short of where the
    // normals of the samples round it meet (the corner's tightest radius).
    // Left turns bend positive, so their inside is the left.
    let reach = |sign: f32| -> Vec<f32> {
        (0..n)
            .map(|i| {
                let tight = (0..17)
                    .map(|k| bend[(i + n + k - 8) % n] * sign)
                    .fold(0.0, f32::max);
                (1.0 / tight.max(1e-3) - BLEND - 2.0).clamp(1.0, BAND)
            })
            .collect()
    };
    let (reach_left, reach_right) = (reach(1.0), reach(-1.0));

    let feel = if dirt { PLAN_DIRT } else { 1.0 };
    // Over a crest the tyres go light and grip less; in a dip they bite harder.
    let mut speed: Vec<f32> = (0..n)
        .map(|i| {
            let r = radius(path[(i + n - 5) % n], path[i], path[(i + 5) % n]).abs();
            let grip = PLAN_GRIP * feel * r;
            let vb = (0..5).map(|k| vbend[(i + n + k - 2) % n]).sum::<f32>() / 5.0;
            // (Only where the corner needs the grip: flat out over a crest on
            // a straight just flies, which the jump check below allows for.)
            let denom = if r > 120.0 { 1.0 } else { 1.0 - grip * vb / G };
            let v2 = if denom > 1.0 / 1.4 { grip / denom } else { grip * 1.4 };
            math::sqrt(v2).min(VMAX)
        })
        .collect();
    // A crest the cars would fly off into a corner: lift so the wheels stay down.
    let lookahead = (30.0 / step) as usize;
    let corners = speed.clone();
    for i in 0..n {
        if vbend[i] >= -1e-4 {
            continue;
        }
        let takeoff = math::sqrt(G / -vbend[i]);
        let ahead = (1..lookahead)
            .map(|k| corners[(i + k) % n])
            .fold(f32::MAX, f32::min);
        if ahead < takeoff * 0.9 {
            speed[i] = speed[i].min(takeoff * 0.97);
        }
    }
    // Braking takes longer downhill and less up.
    for _ in 0..2 {
        for i in (0..n).rev() {
            let next = speed[(i + 1) % n];
            let decel = (PLAN_BRAKE * feel + G * grade[i]).max(2.0);
            let reachable = math::sqrt(next * next + 2. * decel * step);
            speed[i] = speed[i].min(reachable);
        }
    }

    // Fences stand at WALL unless another part of the track is closer: then
    // they share the middle between the two.
    let window = (30. / step) as usize;
    let raw: Vec<f32> = (0..n)
        .map(|i| {
            let mut nearest = f32::MAX;
            for j in 0..n {
                let gap = i.abs_diff(j).min(n - i.abs_diff(j));
                if gap > window {
                    nearest = nearest.min((centre[j] - centre[i]).length());
                }
            }
            (nearest * 0.5).clamp(HALF + RUNOFF + 0.8, WALL)
        })
        .collect();
    let wall: Vec<f32> = (0..n)
        .map(|i| {
            (0..9)
                .map(|k| raw[(i + n + k - 4) % n])
                .fold(f32::MAX, f32::min)
        })
        .collect();

    let t = Track {
        id,
        dirt,
        x: centre.iter().map(|p| p.x).collect(),
        z: centre.iter().map(|p| p.y).collect(),
        nx: normals.iter().map(|p| p.x).collect(),
        nz: normals.iter().map(|p| p.y).collect(),
        line,
        speed,
        bend,
        wall,
        step,
        y,
        grade,
        vbend,
        bank,
        reach_left,
        reach_right,
        ..Track::default()
    };
    let (gx, gz, land) = land(&t, layout(id).hills);
    Track { gx, gz, land, ..t }
}
