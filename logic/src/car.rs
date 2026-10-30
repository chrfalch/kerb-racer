//! Car dynamics, the automatic steering every car shares, and the rivals'
//! pedals. All cars step together on plain copies so they can push each other.
use crate::track::{Track, HALF, RUNOFF};
use exact_game::*;

/// Engine thrust at rest (m/s²); it fades linearly to nothing at VMAX.
pub const ENGINE: f32 = 17.0;
/// Where the engine runs out of pull (m/s); drag holds the car a little under it.
pub const TOP: f32 = 34.0;
pub const BRAKE: f32 = 24.0;
/// Rolling resistance and the extra engine braking with no pedal.
pub const ROLL: f32 = 0.6;
pub const COAST: f32 = 1.4;
/// Lateral grip (m/s²) on asphalt, kerbs, run-off and grass.
pub const GRIP: f32 = 15.5;
pub const GRIP_RUNOFF: f32 = 12.0;
pub const GRIP_GRASS: f32 = 7.5;
/// The most the body may point away from where it is going.
pub const MAX_SLIP: f32 = 0.42;
/// Collision circles: two per car, fore and aft.
pub const BODY_R: f32 = 0.86;
pub const BODY_OFFSET: f32 = 0.72;

#[derive(Clone, Debug, Default, Component)]
pub struct Car {
    pub id: u32,
    pub ai: bool,
    /// Fraction of the planned corner speed a rival dares.
    pub skill: f32,
    pub heading: f32,
    pub vx: f32,
    pub vz: f32,
    /// Nearest track sample and the lateral offset there.
    pub idx: u32,
    pub side: f32,
    /// Extra lateral offset over the racing line, for passing.
    pub lane: f32,
    pub lane_target: f32,
    /// Start-line crossings; lap 1 begins at the first.
    pub crossings: u32,
    pub lap_tick: u64,
    pub best_lap: u64,
    pub last_lap: u64,
    pub finished: bool,
    pub finish_tick: u64,
    pub gas: f32,
    pub brake: bool,
    pub sliding: bool,
    pub surface: u32,
}

/// Surfaces under a car.
pub const ASPHALT: u32 = 0;
pub const RUNOFF_SURFACE: u32 = 1;
pub const GRASS: u32 = 2;

pub struct Body {
    pub car: Car,
    pub p: Vec2,
}

impl Body {
    pub fn v(&self) -> Vec2 {
        Vec2::new(self.car.vx, self.car.vz)
    }
    fn set_v(&mut self, v: Vec2) {
        self.car.vx = v.x;
        self.car.vz = v.y;
    }
    pub fn forward(&self) -> Vec2 {
        let (s, c) = math::sin_cos(self.car.heading);
        Vec2::new(c, s)
    }
    /// Distance raced: completed crossings and how far past the last one.
    pub fn progress(&self, track: &Track) -> f32 {
        let s = self.car.idx as f32 * track.step;
        let behind_line = self.car.idx as usize > track.len() / 2 && self.car.crossings == 0;
        if behind_line {
            s - track.length()
        } else {
            self.car.crossings.saturating_sub(1) as f32 * track.length() + s
        }
    }
}

fn wrap_angle(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let mut a = a % tau;
    if a > std::f32::consts::PI {
        a -= tau;
    } else if a < -std::f32::consts::PI {
        a += tau;
    }
    a
}

/// Where the automatic steering aims: the racing line plus this car's lane,
/// `ahead` metres down the road.
fn aim(track: &Track, car: &Car, ahead: f32) -> Vec2 {
    let i = track.wrap(car.idx as i64 + (ahead / track.step) as i64);
    let offset = (track.line[i] + car.lane).clamp(-(HALF - 1.0), HALF - 1.0);
    track.at(i, offset)
}

/// The speed a rival wants now: the plan a little ahead, scaled by its nerve.
pub fn target_speed(track: &Track, car: &Car) -> f32 {
    let i = track.wrap(car.idx as i64 + (3. / track.step) as i64);
    track.speed[i] * car.skill
}

/// Choose each car's lane: pull out to pass a slower car close ahead, drift
/// back to the racing line otherwise. Rivals also lift behind a car they cannot pass.
pub fn plan(track: &Track, bodies: &mut [Body], dt: f32) -> Vec<f32> {
    let length = track.length();
    let progress: Vec<f32> = bodies.iter().map(|b| b.progress(track)).collect();
    let mut caps = vec![f32::MAX; bodies.len()];
    for a in 0..bodies.len() {
        let mut target = 0.0;
        let me = &bodies[a];
        let my_side = me.car.side;
        let mut blocker: Option<(f32, f32, f32)> = None;
        for (b, other) in bodies.iter().enumerate() {
            if a == b {
                continue;
            }
            let mut gap = progress[b] - progress[a];
            gap -= (gap / length).round() * length;
            if gap > 0.5 && gap < 11.0 && (other.car.side - my_side).abs() < 2.4 {
                let closer = blocker.is_none_or(|(g, _, _)| gap < g);
                if closer {
                    blocker = Some((gap, other.car.side, other.v().length()));
                }
            }
        }
        if let Some((gap, side, speed)) = blocker {
            if me.v().length() + 0.5 > speed {
                let i = me.car.idx as usize;
                let line = track.line[i];
                let room_left = (HALF - 1.0) - side;
                let room_right = side + (HALF - 1.0);
                let pass = if room_left > room_right {
                    side + 2.5
                } else {
                    side - 2.5
                };
                target = (pass - line).clamp(-(HALF - 1.0) - line, (HALF - 1.0) - line);
                if room_left.max(room_right) < 2.6 && gap < 5.0 {
                    caps[a] = speed;
                }
            }
        }
        let car = &mut bodies[a].car;
        car.lane_target = target;
        let step = 2.2 * dt;
        car.lane += (car.lane_target - car.lane).clamp(-step, step);
    }
    caps
}

/// The rival's pedals for this tick.
pub fn drive(track: &Track, body: &mut Body, cap: f32) {
    let v = body.v().dot(body.forward());
    let target = target_speed(track, &body.car).min(cap);
    if v > target + 0.6 {
        body.car.gas = 0.0;
        body.car.brake = true;
    } else {
        body.car.brake = false;
        body.car.gas = ((target - v) * 1.5 + 0.35).clamp(0.0, 1.0);
    }
}

/// One fixed step of one car on its own: steering, pedals, grip, fences.
pub fn step(track: &Track, body: &mut Body, dt: f32) {
    let speed = body.v().length();
    // Steering: pure pursuit toward the aim point, limited by the wheels.
    let ahead = 4.5 + 0.32 * speed;
    let target = aim(track, &body.car, ahead) - body.p;
    let f = body.forward();
    let along = target.dot(f);
    let across = f.perp_dot(target);
    let alpha = math::atan2(across, along.max(0.01));
    let curvature = 2.0 * math::sin(alpha) / target.length().max(1.0);
    let mut yaw = speed * curvature;
    let max_yaw = (speed / 4.0).min(3.6);
    yaw = yaw.clamp(-max_yaw, max_yaw);
    body.car.heading = wrap_angle(body.car.heading + yaw * dt);

    let surface = body.car.surface;
    let grip = match surface {
        ASPHALT => GRIP,
        RUNOFF_SURFACE => GRIP_RUNOFF,
        _ => GRIP_GRASS,
    };
    let f = body.forward();
    let l = f.perp();
    let mut vf = body.v().dot(f);
    let mut vl = body.v().dot(l);
    // Pedals.
    if body.car.brake {
        vf -= BRAKE.min(vf.abs() / dt).copysign(vf) * dt;
    } else {
        let thrust = ENGINE * (1.0 - vf / TOP).max(0.0) * body.car.gas;
        let coast = COAST * (1.0 - body.car.gas);
        vf += (thrust - ROLL - coast - 0.0012 * vf * vf) * dt;
    }
    match surface {
        RUNOFF_SURFACE => vf -= 0.35 * vf * dt,
        GRASS => vf -= 1.3 * vf * dt,
        _ => {}
    }
    vf = vf.max(0.0);
    // Tyres pull the sideways velocity out up to their grip; past it, slide.
    let pull = grip * dt;
    body.car.sliding = vl.abs() > pull * 6.0;
    if body.car.sliding {
        // A sliding tyre scrubs speed.
        vf -= (vl.abs() * 0.35 * dt).min(vf);
    }
    vl -= vl.clamp(-pull, pull);
    let v = f * vf + l * vl;
    body.set_v(v);
    // The nose cannot point too far from where the car is going.
    if v.length() > 1.0 {
        let travel = math::atan2(v.y, v.x);
        let slip = wrap_angle(body.car.heading - travel);
        if slip.abs() > MAX_SLIP {
            body.car.heading = wrap_angle(travel + MAX_SLIP.copysign(slip));
        }
    }
    body.p += v * dt;
    locate(track, body);
    fence(track, body);
}

/// Update the car's nearest sample, offset and surface, and count crossings.
pub fn locate(track: &Track, body: &mut Body) -> bool {
    let before = body.car.idx as usize;
    let (i, side) = track.locate(body.p, before, 14);
    body.car.idx = i as u32;
    body.car.side = side;
    let off = side.abs();
    body.car.surface = if off <= HALF + 0.2 {
        ASPHALT
    } else if off <= HALF + RUNOFF {
        RUNOFF_SURFACE
    } else {
        GRASS
    };
    let n = track.len();
    if before > n * 3 / 4 && i < n / 4 {
        body.car.crossings += 1;
        return true;
    }
    if before < n / 4 && i > n * 3 / 4 {
        body.car.crossings = body.car.crossings.saturating_sub(1);
    }
    false
}

/// Bounce off the fence: lose most of the speed into it and some of the rest.
fn fence(track: &Track, body: &mut Body) {
    let i = body.car.idx as usize;
    let limit = track.wall[i] - 1.0;
    let side = body.car.side;
    if side.abs() <= limit {
        return;
    }
    let out = track.normal(i) * side.signum();
    body.p -= out * (side.abs() - limit);
    body.car.side = limit.copysign(side);
    let v = body.v();
    let into = v.dot(out);
    if into > 0.0 {
        body.set_v((v - out * into * 1.35) * 0.7);
    }
}

/// Push overlapping cars apart and trade the closing speed between them.
pub fn collide(bodies: &mut [Body]) {
    for a in 0..bodies.len() {
        for b in a + 1..bodies.len() {
            for sa in [-1.0f32, 1.0] {
                for sb in [-1.0f32, 1.0] {
                    let pa = bodies[a].p + bodies[a].forward() * BODY_OFFSET * sa;
                    let pb = bodies[b].p + bodies[b].forward() * BODY_OFFSET * sb;
                    let d = pb - pa;
                    let dist = d.length();
                    if !(1e-4..2.0 * BODY_R).contains(&dist) {
                        continue;
                    }
                    let n = d / dist;
                    let overlap = 2.0 * BODY_R - dist;
                    bodies[a].p -= n * overlap * 0.5;
                    bodies[b].p += n * overlap * 0.5;
                    let closing = (bodies[b].v() - bodies[a].v()).dot(n);
                    if closing < 0.0 {
                        let j = -closing * 0.6;
                        let (va, vb) = (bodies[a].v() - n * j, bodies[b].v() + n * j);
                        bodies[a].set_v(va);
                        bodies[b].set_v(vb);
                    }
                }
            }
        }
    }
}
