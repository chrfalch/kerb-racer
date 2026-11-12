//! Car dynamics, the automatic steering every car shares, and the rivals'
//! pedals. All cars step together on plain copies so they can push each other.
use crate::track::{Track, G, HALF, KERB_IN, RUNOFF, VMAX};
use exact_game::*;

/// Engine thrust at rest (m/s²); it fades linearly to nothing at VMAX.
pub const ENGINE: f32 = 17.0;
/// Where the engine runs out of pull (m/s); drag holds the car a little under it.
pub const TOP: f32 = 34.0;
pub const BRAKE: f32 = 24.0;
/// Letting go of the gas brakes this much of the pedal's force, too little to
/// lock the wheels: the player's only way to slow, it has to feel smooth.
pub const LIFT: f32 = 0.26;
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
/// Half a car's width to its wheels' outer edge (m), for the kerbs.
pub const HALF_TRACK: f32 = 0.88;
/// How far beside another car a pass aims (m between their centres).
pub const PASS_GAP: f32 = 2.6;
/// The closest two cars side by side may be steered (m between centres).
pub const SIDE_GAP: f32 = 2.05;
/// A pass holds its side until the car passed is this far behind (m).
pub const PASS_CLEAR: f32 = 4.0;
/// Slipstream: a car in another's wake, from this close to this far behind
/// (m), within this much of its line, loses up to half its drag and gains top speed.
pub const DRAFT_NEAR: f32 = 2.5;
pub const DRAFT_FAR: f32 = 15.0;
pub const DRAFT_WIDTH: f32 = 1.6;
pub const DRAFT_TOP: f32 = 0.06;
/// Kerbs: a little more bite on the raised paint.
pub const KERB_GRIP: f32 = 1.05;

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
    /// The brake comes from a lift off the gas: gentle, see [`LIFT`].
    pub lift: bool,
    pub sliding: bool,
    pub surface: u32,
    /// Front wheels' steering angle (radians, left positive).
    pub steer: f32,
    /// The tyres' surface speed (m/s): ground speed while they grip, less when
    /// the brakes lock them, more while they spin up off the line.
    pub roll: f32,
    /// How far the wheels have turned (radians, wrapped).
    pub spin: f32,
    /// The hardest knock this tick (m/s closing speed), off a car or the fence.
    pub knock: f32,
    /// Against the fence this tick.
    pub walled: bool,
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
    /// The car being passed (its id + 1; 0 for none), and which side of it
    /// this car goes by (1 left, -1 right), held until it is clear.
    pub pass: u32,
    pub pass_side: f32,
    /// A rival covering the inside of the next corner (its side, 0 for none).
    pub defend: f32,
    /// How deep in another car's slipstream (0..1).
    pub draft: f32,
    /// Riding a kerb, and the kerb block under it last tick (for the rumble).
    pub kerb: bool,
    pub kerb_block: i32,
    /// Forward and sideways acceleration (m/s²), smoothed, for the body's
    /// dive, squat and roll.
    pub accel: f32,
    pub lateral: f32,
    /// A rival's character: how late it dares to brake and fight (0..1), how
    /// rarely it makes a mistake (0..1), how hard it defends (0..1).
    pub aggression: f32,
    pub consistency: f32,
    pub defence: f32,
    /// The share of its skill a rival drives at now (its pace keeping it in
    /// touch with the player), and an overcooked corner: how far too fast and
    /// for how many more ticks.
    pub pace: f32,
    pub mistake: f32,
    pub mistake_ticks: u32,
    /// The last 32-metre stretch a mistake was considered for.
    pub mistake_block: u32,
    /// Wheels locked under braking (no roll at speed).
    pub locked: bool,
    /// Metres travelled since this car last laid a skid mark.
    pub skid_run: f32,
    /// How hard (m/s of closing speed) it hit another car this tick; 0 if not.
    pub bump: f32,
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
    let offset = (track.line[i] + car.lane).clamp(-track.edge[i], track.edge[i]);
    track.at(i, offset)
}

/// The speed a rival wants now: the plan a little ahead, scaled by its nerve
/// and its pace, too fast while it is overcooking a corner, and a little more
/// down a straight in someone's slipstream.
pub fn target_speed(track: &Track, car: &Car) -> f32 {
    let i = track.wrap(car.idx as i64 + (3. / track.step) as i64);
    let pace = if car.pace > 0.0 { car.pace } else { 1.0 };
    let mut v = track.speed[i] * car.skill * pace * (1.0 + car.mistake);
    if track.speed[i] > VMAX * 0.97 {
        v += car.draft * 2.5;
    }
    v
}

/// Lateral offset `PASS_GAP` beside a car at `side`, on `pass_side`.
fn beside(side: f32, pass_side: f32) -> f32 {
    side + PASS_GAP * pass_side
}

/// Choose each car's lane and fill in its slipstream.
///
/// A car that closes on a slower one picks a side and commits to it: the
/// player's chosen side if there is room, else the inside of the next corner,
/// else whichever side has more road. It holds that side until the car it is
/// passing is `PASS_CLEAR` behind, so a pass never turns back into the car
/// beside it. Nobody steers toward a car alongside. Rivals also cover the
/// inside when someone closes from behind, lift behind a car they cannot
/// pass, and yield a corner to the car on its inside. Returns each car's
/// speed cap for `drive`.
pub fn plan(track: &Track, bodies: &mut [Body], dt: f32) -> Vec<f32> {
    let n = bodies.len();
    let length = track.length();
    let progress: Vec<f32> = bodies.iter().map(|b| b.progress(track)).collect();
    let speed: Vec<f32> = bodies.iter().map(|b| b.v().length()).collect();
    let side: Vec<f32> = bodies.iter().map(|b| b.car.side).collect();
    let air: Vec<bool> = bodies.iter().map(|b| b.car.air).collect();
    // How far `b` is up the road from `a` (negative: behind).
    let gap = |a: usize, b: usize| {
        let g = progress[b] - progress[a];
        g - (g / length).round() * length
    };
    let mut caps = vec![f32::MAX; n];
    for a in 0..n {
        let car = &bodies[a].car;
        let i = car.idx as usize;
        let line = track.line[i];
        let turn = track.turn_ahead(i);
        let mine = side[a];
        let edge = track.edge[i];
        let room = |s: f32, other: f32| {
            if s > 0.0 {
                edge - other
            } else {
                other + edge
            }
        };

        // Slipstream: in the wake of a car close ahead on much the same line.
        let mut wake = 0.0f32;
        for b in 0..n {
            let g = gap(a, b);
            if b != a && (DRAFT_NEAR..DRAFT_FAR).contains(&g) && (side[b] - mine).abs() < DRAFT_WIDTH && !air[b] {
                wake = wake.max(1.0 - (g - DRAFT_NEAR) / (DRAFT_FAR - DRAFT_NEAR));
            }
        }

        // Carry on with a pass under way, or give it up once clear (or dropped back).
        let (mut pass, mut pass_side) = (car.pass, car.pass_side);
        let mut want: Option<f32> = None;
        if pass != 0 {
            let b = (pass - 1) as usize;
            if b >= n || !(-PASS_CLEAR..14.0).contains(&gap(a, b)) {
                pass = 0;
            } else {
                want = Some(beside(side[b], pass_side));
            }
        }
        if pass == 0 {
            let mut blocker: Option<(f32, usize)> = None;
            for b in 0..n {
                let g = gap(a, b);
                if b != a && g > 0.5 && g < 11.0 && (side[b] - mine).abs() < 2.4 && blocker.is_none_or(|(c, _)| g < c) {
                    blocker = Some((g, b));
                }
            }
            if let Some((g, b)) = blocker {
                if speed[a] + 0.5 > speed[b] {
                    let other = side[b];
                    let prefer = if turn != 0.0 {
                        turn
                    } else if room(1.0, other) > room(-1.0, other) {
                        1.0
                    } else {
                        -1.0
                    };
                    let chosen = if room(prefer, other) >= PASS_GAP {
                        prefer
                    } else if room(-prefer, other) >= PASS_GAP {
                        -prefer
                    } else {
                        0.0
                    };
                    if chosen != 0.0 {
                        pass = b as u32 + 1;
                        pass_side = chosen;
                        want = Some(beside(other, chosen));
                    } else if car.ai && g < 6.0 {
                        // No way by: follow a couple of car lengths back.
                        caps[a] = speed[b] + (g - 3.0) * 0.8;
                    }
                }
            }
        }

        // A rival covers the inside once, against a car closing from behind.
        let mut defend = car.defend;
        if car.ai && pass == 0 {
            let threat = (0..n).any(|b| {
                let g = gap(a, b);
                b != a && (-9.0..-1.2).contains(&g) && speed[b] > speed[a] + 0.3
            });
            if !threat {
                defend = 0.0;
            } else if defend == 0.0 && turn != 0.0 && car.defence > 0.35 {
                defend = turn;
            }
            if defend != 0.0 {
                want = Some(defend * (edge - 0.4) * car.defence.min(1.0));
            }
        } else {
            defend = 0.0;
        }

        // Never steer into a car alongside: keep its width plus a little away.
        let mut lo = -edge - line;
        let mut hi = edge - line;
        for b in 0..n {
            if b == a || gap(a, b).abs() > 3.4 || air[b] != air[a] {
                continue;
            }
            if side[b] > mine + 0.25 {
                hi = hi.min(side[b] - SIDE_GAP - line);
            } else if side[b] < mine - 0.25 {
                lo = lo.max(side[b] + SIDE_GAP - line);
            }
            // Side by side into a corner: the car on the outside gives way.
            let alongside = gap(a, b) > -1.6;
            let outside = turn != 0.0 && (mine - side[b]) * turn < -0.5;
            let braking = track.speed[track.wrap(i as i64 + (20.0 / track.step) as i64)] < speed[a] - 3.0;
            if car.ai && alongside && outside && braking && pass as usize != b + 1 {
                caps[a] = caps[a].min(speed[b] - 0.6);
            }
        }
        let target = want.map_or(0.0, |s| s - line);
        let target = if lo <= hi {
            target.clamp(lo, hi)
        } else {
            // Squeezed on both sides: hold the lane.
            car.lane
        };

        let car = &mut bodies[a].car;
        car.pass = pass;
        car.pass_side = pass_side;
        car.defend = defend;
        car.lane_target = target;
        car.draft += (wake - car.draft) * (dt * 2.5).min(1.0);
        let rate = if pass != 0 { 3.0 } else { 2.2 };
        let step = rate * dt;
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

/// A rival's slips: now and then, entering a braking zone, it carries too
/// much speed into the corner. The chance falls with its consistency and is
/// fixed by the car, the lap and the stretch of road, so a race replays.
pub fn blunder(track: &Track, car: &mut Car) {
    car.mistake_ticks = car.mistake_ticks.saturating_sub(1);
    if car.mistake_ticks == 0 {
        car.mistake = 0.0;
    }
    let block = car.idx / 32;
    let mark = car.crossings * 1000 + block + 1;
    if car.mistake_block == mark {
        return;
    }
    let i = car.idx as usize;
    let ahead = track.speed[track.wrap(i as i64 + (18.0 / track.step) as i64)];
    if ahead > track.speed[i] - 4.0 {
        return;
    }
    car.mistake_block = mark;
    let roll = hash01(car.id, car.crossings, block);
    if roll < (1.0 - car.consistency) * 0.32 {
        car.mistake = 0.07 + 0.05 * hash01(car.id + 17, car.crossings, block);
        car.mistake_ticks = 300;
    }
}

/// A well-mixed number in 0..1 from three small integers.
pub fn hash01(a: u32, b: u32, c: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9e37_79b1) ^ b.wrapping_mul(0x85eb_ca77) ^ c.wrapping_mul(0xc2b2_ae3d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// One fixed step of one car on its own: steering, pedals, grip, fences.
/// The wheels against the road: braking slows the tyres faster than the car,
/// so they lock and the car slides on; off the brake they grip and roll at
/// ground speed again; pulling away they spin up a little.
fn wheels(car: &mut Car, ground: f32, dt: f32) {
    if car.brake && !car.lift {
        car.roll = (car.roll - WHEEL_BRAKE * dt).max(0.0);
    } else {
        let wheelspin = car.gas * (9.0 - ground).max(0.0) * 0.45;
        car.roll += (ground + wheelspin - car.roll) * (dt * 16.0).min(1.0);
    }
    car.roll = car.roll.max(0.0);
    car.spin = wrap_angle(car.spin + car.roll / WHEEL_R * dt);
}

pub fn step(track: &Track, body: &mut Body, dt: f32) {
    body.car.knock = 0.0;
    body.car.walled = false;
    let speed = body.v().length();
    let air = body.car.air;
    let slope = track.gradient(body.p, body.car.idx as usize);
    let idx = body.car.idx as usize;
    // How far over the planned speed: too fast and the auto-steer can't hold
    // the line. It looks further and turns less, running wide toward the kerb
    // and the run-off, which reads as the driver's own doing.
    let plan = track.speed[track.wrap(idx as i64 + (6.0 / track.step) as i64)].max(4.0);
    let over = (speed / plan - 1.04).max(0.0);
    // Trail-braking: on the brake while turning in, the weight on the nose
    // tightens the line.
    let trail = body.car.brake && !body.car.locked && speed > 8.0;
    // Steering: pure pursuit toward the aim point, limited by the wheels.
    let ahead = (4.5 + 0.32 * speed) * (1.0 + over * 1.5);
    let target = aim(track, &body.car, ahead) - body.p;
    // Aim along where the car is going, not where its nose points, so a
    // drift doesn't fool the steering into running wide.
    let f = if speed > 1.0 {
        body.v() / speed
    } else {
        body.forward()
    };
    let along = target.dot(f);
    let across = f.perp_dot(target);
    let alpha = math::atan2(across, along.max(0.01));
    let curvature = 2.0 * math::sin(alpha) / target.length().max(1.0);
    let bite = (1.0 / (1.0 + 3.0 * over)) * if trail { 1.18 } else { 1.0 };
    let mut yaw = speed * curvature * bite;
    let max_yaw = (speed / 4.0).min(3.6) * if trail { 1.12 } else { 1.0 };
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
    let grip = if track.wet { grip * crate::track::WET } else { grip };
    let grip = if body.car.kerb { grip * KERB_GRIP } else { grip };
    let grip = grip * body.car.load.clamp(0.3, 1.5) * (1.0 - 0.55 * body.car.shaken);
    let (mut drift, brake, mut max_slip) = if track.dirt {
        (DRIFT_DIRT, BRAKE_DIRT, MAX_SLIP_DIRT)
    } else {
        (DRIFT, BRAKE, MAX_SLIP)
    };
    // On loose or wet ground the throttle steers the tail: gas held through a
    // slide keeps it hanging out, a lift lets the tyres straighten it.
    let loose = track.dirt || track.wet;
    if loose && body.car.sliding && body.car.gas > 0.5 {
        drift *= 0.6;
        max_slip += 0.12;
    }
    // A trail-braked entry lets the tail step out a little to point the nose.
    if trail {
        drift *= 0.85;
    }
    let f = body.forward();
    let l = f.perp();
    let mut vf = body.v().dot(f);
    let before = vf;
    let mut vl = body.v().dot(l);
    // In someone's slipstream the air holds the car back less.
    let draft = body.car.draft;
    let drag = 0.0012 * (1.0 - 0.5 * draft);
    // Pedals; in the air the engine only revs and the brakes only stop the wheels.
    if air {
        // Only the air slows it.
        vf -= drag * vf * vf * dt;
    } else if body.car.brake {
        let brake = if body.car.lift { brake * LIFT } else { brake };
        vf -= brake.min(vf.abs() / dt).copysign(vf) * dt;
    } else {
        let top = TOP * (1.0 + DRAFT_TOP * draft);
        let thrust = ENGINE * (1.0 - vf / top).max(0.0) * body.car.gas;
        let coast = COAST * (1.0 - body.car.gas);
        vf += (thrust - ROLL - coast - drag * vf * vf) * dt;
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
    // What the body feels: braking and accelerating along it, cornering across it.
    let k = (dt * 8.0).min(1.0);
    let accel = if air { 0.0 } else { (vf - before) / dt };
    let lateral = if air { 0.0 } else { vf * yaw };
    body.car.accel += (accel - body.car.accel) * k;
    body.car.lateral += (lateral - body.car.lateral) * k;
    body.p += v * dt;
    if air {
        // Nothing for the tyres to roll on: they keep turning as they were.
        let spin = body.car.roll / WHEEL_R * dt;
        body.car.spin = wrap_angle(body.car.spin + spin);
    } else {
        wheels(&mut body.car, vf, dt);
    }
    body.car.locked = !air && body.car.brake && body.car.roll == 0.0 && vf > 3.0;
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

/// Body pitch (radians) per m/s² of forward acceleration, and roll per m/s²
/// sideways: about 3° of dive braking hard and 3° of roll in a fast corner.
pub const PITCH_PER_G: f32 = 0.0022;
pub const ROLL_PER_G: f32 = 0.0036;

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
    // Weight transfer on the springs: the nose dives under braking and lifts
    // under power, the body rolls toward the outside of a corner.
    let (pitch, lean) = if car.air {
        (pitch, lean)
    } else {
        (
            pitch + (car.accel * PITCH_PER_G).clamp(-0.07, 0.03),
            lean - (car.lateral * ROLL_PER_G).clamp(-0.08, 0.08),
        )
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
    // On a kerb when the outside wheels are up on its paint.
    let kerb = track.kerb[i] && !body.car.air && off + HALF_TRACK > HALF - KERB_IN && off < HALF + 0.9;
    if kerb {
        // Each red or white block is a metre long; crossing from one to the
        // next jolts the suspension (the rumble).
        let block = (i as f32 * track.step) as i32;
        if body.car.kerb && block != body.car.kerb_block {
            let speed = body.v().length();
            body.car.squash_v -= (0.12 + speed * 0.012).min(0.5);
        }
        body.car.kerb_block = block;
    }
    body.car.kerb = kerb;
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
    body.car.walled = true;
    let v = body.v();
    let into = v.dot(out);
    if into > 0.0 {
        body.car.knock = body.car.knock.max(into);
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
                        for k in [a, b] {
                            bodies[k].car.knock = bodies[k].car.knock.max(-closing);
                        }
                        let j = -closing * 0.6;
                        let (va, vb) = (bodies[a].v() - n * j, bodies[b].v() + n * j);
                        bodies[a].set_v(va);
                        bodies[b].set_v(vb);
                        // Running into the back of a car costs the one behind:
                        // it scrubs more speed, and the car it hit is unsettled.
                        for k in [a, b] {
                            bodies[k].car.bump = bodies[k].car.bump.max(-closing);
                        }
                        let hit = (-closing - 1.0).max(0.0);
                        if hit > 0.0 {
                            let a_behind = bodies[a].forward().dot(n) > 0.5;
                            let b_behind = bodies[b].forward().dot(-n) > 0.5;
                            for (rear, front) in [(a, b), (b, a)] {
                                let behind = if rear == a { a_behind } else { b_behind };
                                if behind {
                                    let v = bodies[rear].v();
                                    bodies[rear].set_v(v * (1.0 - (0.04 * hit).min(0.25)));
                                    let car = &mut bodies[front].car;
                                    car.shaken = car.shaken.max((hit * 0.12).min(0.6));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
