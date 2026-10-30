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

pub struct Layout {
    pub name: &'static str,
    /// Metres from the first point to the start line, so the grid sits on a straight.
    pub start: f32,
    pub points: &'static [(f32, f32)],
}

pub const LAYOUTS: [Layout; 3] = [
    Layout {
        name: "Pine Hollow",
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
    },
    Layout {
        name: "Fern Loop",
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
    },
    Layout {
        name: "Kerb Ring",
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
    let mut speed: Vec<f32> = (0..n)
        .map(|i| {
            let r = radius(path[(i + n - 5) % n], path[i], path[(i + 5) % n]).abs();
            math::sqrt(PLAN_GRIP * r).min(VMAX)
        })
        .collect();
    for _ in 0..2 {
        for i in (0..n).rev() {
            let next = speed[(i + 1) % n];
            let reachable = math::sqrt(next * next + 2. * PLAN_BRAKE * step);
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

    Track {
        id,
        x: centre.iter().map(|p| p.x).collect(),
        z: centre.iter().map(|p| p.y).collect(),
        nx: normals.iter().map(|p| p.x).collect(),
        nz: normals.iter().map(|p| p.y).collect(),
        line,
        speed,
        bend,
        wall,
        step,
    }
}
