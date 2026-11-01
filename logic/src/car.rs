//! Car dynamics, the automatic steering every car shares, and the rivals'
//! pedals. All cars step together on plain copies so they can push each other.
use crate::track::{Track, G, HALF, RUNOFF};
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
/// On a dirt track: packed dirt, the loose gravel beside it, and the grass.
pub const GRIP_DIRT: f32 = 11.0;
pub const GRIP_GRAVEL: f32 = 8.0;
pub const GRIP_DIRT_GRASS: f32 = 6.5;
/// The most the body may point away from where it is going.
pub const MAX_SLIP: f32 = 0.42;
/// On dirt the tail swings further out before the body catches it.
pub const MAX_SLIP_DIRT: f32 = 0.62;
/// How fast (1/s) the tyres straighten a sideways slide while under their
/// grip. Lower lets the tail hang out further in a corner: the nose turns in
/// and the car drifts through at a steady angle of about yaw / DRIFT.
pub const DRIFT: f32 = 7.0;
/// On dirt the tyres straighten a slide much more lazily: every corner is a drift.
pub const DRIFT_DIRT: f32 = 3.2;
/// Braking on loose dirt (m/s²).
pub const BRAKE_DIRT: f32 = 17.0;
/// Sideways speed (m/s) past which the tyres smoke and scrub.
pub const SLIDE: f32 = 3.0;
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
    /// Front wheels' steering angle (radians, left positive).
    pub steer: f32,
    /// The tyres' surface speed (m/s): ground speed while they grip, less when
    /// the brakes lock them, more while they spin up off the line.
    pub roll: f32,
    /// How far the wheels have turned (radians, wrapped).
    pub spin: f32,
    /// Height of the car's base above the datum and its vertical speed.
    pub y: f32,
    pub vy: f32,
    /// In the air: off a crest, nothing but gravity until it comes down.
    pub air: bool,
    /// Ticks since the wheels left the ground (0 on it).
    pub air_ticks: u32,
    /// How hard (m/s) it came down, on the tick it lands; 0 otherwise.
    pub landing: f32,
    /// Weight on the tyres: 1 on the level, less over a crest, more in a dip.
    pub load: f32,
    /// A hard landing unsettles the car: grip is down while this fades (0..1).
    pub shaken: f32,
    /// The body's tilt (nose up, and about its length), its suspension's
    /// travel (m, negative is squashed) and that travel's speed.
    pub pitch: f32,
    pub lean: f32,
    pub squash: f32,
    pub squash_v: f32,
}

/// Wheel radius of every car (m).
pub const WHEEL_R: f32 = 0.36;
/// Distance between the axles (m), for the steering angle.
pub const WHEELBASE: f32 = 1.85;
/// How hard the brakes slow the wheels themselves (m/s² at the tread): far
/// more than the road can slow the car, so a stamp on the pedal locks them.
pub const WHEEL_BRAKE: f32 = 70.0;
/// The road has to fall away this much faster (m/s) than gravity can pull
/// the car down before its wheels leave the ground.
pub const LIFT_OFF: f32 = 0.05;
/// The most upward acceleration (m/s²) the ground can give the body; the
/// suspension takes the rest.
pub const SOAK: f32 = 40.0;
/// Ticks in the air before it counts as a jump (for the HUD and the camera).
pub const JUMP_TICKS: u32 = 12;

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
/// The wheels against the road: braking slows the tyres faster than the car,
/// so they lock and the car slides on; off the brake they grip and roll at
/// ground speed again; pulling away they spin up a little.
fn wheels(car: &mut Car, ground: f32, dt: f32) {
    if car.brake {
        car.roll = (car.roll - WHEEL_BRAKE * dt).max(0.0);
    } else {
        let wheelspin = car.gas * (9.0 - ground).max(0.0) * 0.45;
        car.roll += (ground + wheelspin - car.roll) * (dt * 16.0).min(1.0);
    }
    car.roll = car.roll.max(0.0);
    car.spin = wrap_angle(car.spin + car.roll / WHEEL_R * dt);
}

pub fn step(track: &Track, body: &mut Body, dt: f32) {
    let speed = body.v().length();
    let air = body.car.air;
    let slope = track.gradient(body.p, body.car.idx as usize);
    // Steering: pure pursuit toward the aim point, limited by the wheels.
    let ahead = 4.5 + 0.32 * speed;
    let target = aim(track, &body.car, ahead) - body.p;
    // Aim along where the car is going, not where its nose points, so a
    // drift doesn't fool the steering into running wide.
    let f = if speed > 1.0 { body.v() / speed } else { body.forward() };
    let along = target.dot(f);
    let across = f.perp_dot(target);
    let alpha = math::atan2(across, along.max(0.01));
    let curvature = 2.0 * math::sin(alpha) / target.length().max(1.0);
    let mut yaw = speed * curvature;
    let max_yaw = (speed / 4.0).min(3.6);
    yaw = yaw.clamp(-max_yaw, max_yaw);
    // In the air the tyres have nothing to turn against.
    if air {
        yaw = 0.0;
    }
    body.car.heading = wrap_angle(body.car.heading + yaw * dt);
    // The front wheels point along the arc the car is turning on.
    let steer = math::atan(WHEELBASE * yaw / speed.max(1.0)).clamp(-0.55, 0.55);
    body.car.steer += (steer - body.car.steer) * (dt * 14.0).min(1.0);

    let surface = body.car.surface;
    let grip = match (track.dirt, surface) {
        (false, ASPHALT) => GRIP,
        (false, RUNOFF_SURFACE) => GRIP_RUNOFF,
        (false, _) => GRIP_GRASS,
        (true, ASPHALT) => GRIP_DIRT,
        (true, RUNOFF_SURFACE) => GRIP_GRAVEL,
        (true, _) => GRIP_DIRT_GRASS,
    };
    let grip = grip * body.car.load.clamp(0.3, 1.5) * (1.0 - 0.55 * body.car.shaken);
    let (drift, brake, max_slip) = if track.dirt {
        (DRIFT_DIRT, BRAKE_DIRT, MAX_SLIP_DIRT)
    } else {
        (DRIFT, BRAKE, MAX_SLIP)
    };
    let f = body.forward();
    let l = f.perp();
    let mut vf = body.v().dot(f);
    let mut vl = body.v().dot(l);
    // Pedals; in the air the engine only revs and the brakes only stop the wheels.
    if air {
        // Only the air slows it.
        vf -= 0.0012 * vf * vf * dt;
    } else if body.car.brake {
        vf -= brake.min(vf.abs() / dt).copysign(vf) * dt;
    } else {
        let thrust = ENGINE * (1.0 - vf / TOP).max(0.0) * body.car.gas;
        let coast = COAST * (1.0 - body.car.gas);
        vf += (thrust - ROLL - coast - 0.0012 * vf * vf) * dt;
    }
    if !air {
        match surface {
            RUNOFF_SURFACE => vf -= 0.35 * vf * dt,
            GRASS => vf -= 1.3 * vf * dt,
            _ => {}
        }
        // Gravity along the ground: hills slow the climb and speed the descent,
        // and a banked or sloping road pulls the car down its fall line.
        let norm = math::sqrt(1.0 + slope.length_squared());
        vf -= G * slope.dot(f) / norm * dt;
        vl -= G * slope.dot(l) / norm * dt;
    }
    vf = vf.max(0.0);
    // Tyres pull the sideways velocity out, gently while it is small (the
    // drift) and up to their grip when it is large; past that, slide.
    let pull = if air { 0.0 } else { (vl.abs() * drift).min(grip) * dt };
    body.car.sliding = !air && vl.abs() > SLIDE;
    if body.car.sliding {
        // A sliding tyre scrubs speed.
        vf -= ((vl.abs() - SLIDE) * 0.35 * dt).min(vf);
    }
    vl -= vl.clamp(-pull, pull);
    let v = f * vf + l * vl;
    body.set_v(v);
    // The nose cannot point too far from where the car is going.
    if !air && v.length() > 1.0 {
        let travel = math::atan2(v.y, v.x);
        let slip = wrap_angle(body.car.heading - travel);
        if slip.abs() > max_slip {
            body.car.heading = wrap_angle(travel + max_slip.copysign(slip));
        }
    }
    body.p += v * dt;
    if air {
        // Nothing for the tyres to roll on: they keep turning as they were.
        let spin = body.car.roll / WHEEL_R * dt;
        body.car.spin = wrap_angle(body.car.spin + spin);
    } else {
        wheels(&mut body.car, vf, dt);
    }
    locate(track, body);
    fence(track, body);
    vertical(track, body, slope, dt);
}

/// Up and down: follow the ground while gravity can keep the car on it, fly
/// when the road falls away faster than that (over a sharp enough crest at
/// speed), and come down again with a thump that costs speed and grip.
fn vertical(track: &Track, body: &mut Body, slope: Vec2, dt: f32) {
    let ground = track.surface(body.p, body.car.idx as usize);
    let v = body.v();
    let car = &mut body.car;
    car.landing = 0.0;
    if !car.air {
        let follow = (ground - car.y) / dt;
        let fall = car.vy - G * dt;
        if follow < fall - LIFT_OFF && v.length() > 4.0 {
            car.air = true;
            car.air_ticks = 1;
            car.vy = fall;
            car.y += car.vy * dt;
        } else {
            // What it takes to stay on the road is the weight on the tyres.
            let lift = (follow - car.vy) / dt;
            let load = ((G + lift) / G).clamp(0.3, 1.6);
            car.load += (load - car.load) * (dt * 14.0).min(1.0);
            // The suspension soaks up a sudden ramp (a bank, a kerb of land):
            // the body only gains upward speed so fast, so it isn't flung.
            car.vy = follow.min(car.vy + SOAK * dt);
            car.y = ground;
        }
    } else {
        car.vy -= G * dt;
        car.y += car.vy * dt;
        car.air_ticks += 1;
        if car.y <= ground {
            // How fast it meets the ground, which may itself be falling away.
            let under = slope.dot(v);
            let impact = (under - car.vy).max(0.0);
            car.y = ground;
            car.vy = under;
            car.air = false;
            car.air_ticks = 0;
            car.landing = impact.max(0.01);
            car.load = 1.0;
            // Loose ground and grass give way: a rougher landing.
            let rough = if track.dirt || car.surface != ASPHALT { 1.5 } else { 1.0 };
            let scrub = ((0.015 + 0.02 * impact) * rough).min(0.3);
            car.vx *= 1.0 - scrub;
            car.vz *= 1.0 - scrub;
            car.shaken = car.shaken.max((impact * rough / 7.0).min(1.0));
            car.squash_v -= impact * 0.4;
        }
    }
    car.shaken = (car.shaken - dt * 1.6).max(0.0);
    pose(car, slope, dt);
}

/// The body's tilt and suspension, for drawing: lie along the ground, lift
/// the nose a little with the flight path, squash on landings.
fn pose(car: &mut Car, slope: Vec2, dt: f32) {
    let (s, c) = math::sin_cos(car.heading);
    let (f, l) = (Vec2::new(c, s), Vec2::new(-s, c));
    let speed = Vec2::new(car.vx, car.vz).length();
    let (pitch, lean, rate) = if car.air {
        (math::atan(car.vy / speed.max(4.0)) * 0.5, 0.0, 3.0)
    } else {
        (math::atan(slope.dot(f)), -math::atan(slope.dot(l)), 14.0)
    };
    let k = (dt * rate).min(1.0);
    car.pitch += (pitch - car.pitch) * k;
    car.lean += (lean - car.lean) * k;
    // A spring settling where the load puts it.
    let rest = -(car.load - 1.0) * 0.06;
    car.squash_v += (-180.0 * (car.squash - rest) - 13.0 * car.squash_v) * dt;
    car.squash = (car.squash + car.squash_v * dt).clamp(-0.2, 0.12);
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
    shove(track, body, -out * (side.abs() - limit));
    body.car.side = limit.copysign(side);
    let v = body.v();
    let into = v.dot(out);
    if into > 0.0 {
        body.set_v((v - out * into * 1.35) * 0.7);
    }
}

/// Move a car without it travelling there: a fence or another car pushing it
/// aside. On the ground it stays on the ground, rather than the step up or
/// down reading as a ramp it drove over at speed.
fn shove(track: &Track, body: &mut Body, by: Vec2) {
    let i = body.car.idx as usize;
    let before = track.height(body.p, i);
    body.p += by;
    if !body.car.air {
        body.car.y += track.height(body.p, i) - before;
    }
}

/// Push overlapping cars apart and trade the closing speed between them.
pub fn collide(track: &Track, bodies: &mut [Body]) {
    for a in 0..bodies.len() {
        for b in a + 1..bodies.len() {
            // A car in the air sails over one on the ground.
            if (bodies[a].car.y - bodies[b].car.y).abs() > 0.9 {
                continue;
            }
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
                    shove(track, &mut bodies[a], -n * overlap * 0.5);
                    shove(track, &mut bodies[b], n * overlap * 0.5);
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
