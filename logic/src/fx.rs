//! What a race shows besides the cars themselves: live skid marks, brake
//! lamps, slipstream streaks, the braking guide painted ahead of the player
//! and the arrow beside the player's car for the side it will pass on.
use crate::car::{self, Body};
use crate::scenery;
use crate::track::Track;
use exact_game::*;

/// Skid marks on the road at once; the oldest is reused for the next.
pub const SKIDS: usize = 240;
/// A mark is laid every this many metres of a lock-up or slide.
pub const SKID_EVERY: f32 = 0.9;
/// The braking guide: this many chevrons, from this far ahead, this far apart (m).
pub const GUIDES: usize = 10;
pub const GUIDE_FROM: f32 = 6.0;
pub const GUIDE_EVERY: f32 = 3.2;

/// The ring of skid marks: which one goes down next.
#[derive(Clone, Debug, Default, Resource)]
pub struct Marks {
    pub next: u32,
}

/// Where each body style's tail lamps sit (x aft, height, off centre), as
/// `art/src/cars.py` builds them, in grid order.
fn tail(id: u32) -> (f32, f32, f32) {
    match id {
        0 | 4 => (-1.6, 0.6, 0.52),
        1 | 5 => (-1.62, 0.66, 0.61),
        _ => (-1.62, 0.56, 0.58),
    }
}

/// A car's brake lamps (lit while it brakes) and its slipstream streaks.
pub fn setup_car(w: &mut World, car: Entity, name: &str, id: u32) {
    let (x, y, z) = tail(id);
    let lamps = w
        .generated_model(&format!("lamps-{id}.model"), scenery::brake_lamps(x - 0.07, y, z))
        .expect("lamps");
    w.spawn_named(
        format!("{name}-lamps"),
        (
            Transform::default(),
            lamps,
            Visible(false),
            Parent(car),
            Ambient,
        ),
    );
    w.spawn_named(
        format!("{name}-draft"),
        (
            Transform::at(0.4, 0.7, 0.0),
            Emitter {
                shape: emitter::Shape::Box(Vec3::new(1.6, 0.4, 1.1)),
                rate: 40.0,
                lifetime: 0.32,
                speed: 0.3,
                spread: 0.3,
                gravity: Vec3::ZERO,
                drag: 0.0,
                size: [0.05, 0.02],
                color: [[0.92, 0.95, 1.0, 0.3], [0.92, 0.95, 1.0, 0.0]],
                seed: 400 + id as u64,
                bound: [-8.0, -1.0, -8.0, 8.0, 3.0, 8.0],
                additive: false,
                running: false,
                ..Emitter::default()
            },
            emitter::WorldSpace::default(),
            Parent(car),
            Ambient,
        ),
    );
}

/// The skid marks (hidden until laid), the braking guide and the side arrow.
pub fn setup(w: &mut World, assist: u32) {
    let mark = w
        .generated_model("skid.model", scenery::skid_mark())
        .expect("skid");
    for k in 0..SKIDS {
        w.spawn_named(
            format!("skid-{k}"),
            (Transform::default(), mark.clone(), Visible(false), Ambient),
        );
    }
    w.insert_resource(Marks::default());
    let chevron = w.generated("chevron.model", scenery::chevron()).expect("chevron");
    for k in 0..GUIDES {
        w.spawn_named(
            format!("guide-{k}"),
            (
                Transform::default(),
                chevron.clone(),
                Material::glow(GREEN),
                Visible(assist > 0),
                Ambient,
            ),
        );
    }
    let arrow = w.generated("arrow.model", scenery::side_arrow()).expect("arrow");
    let player = w.named("player").expect("player");
    w.spawn_named(
        "side-arrow",
        (
            Transform::at(0.0, 0.12, 1.6),
            arrow,
            Material::glow([1.8, 1.25, 0.2]),
            Visible(false),
            Parent(player),
            Ambient,
        ),
    );
}

const GREEN: [f32; 3] = [0.15, 1.1, 0.35];
const AMBER: [f32; 3] = [1.4, 0.8, 0.1];
const RED: [f32; 3] = [1.6, 0.15, 0.1];

/// Set a toggle only when it changes: a mutable borrow marks the row written.
fn show(w: &mut World, name: &str, on: bool) {
    if w.require::<Visible>(name).0 != on {
        w.require_mut::<Visible>(name).0 = on;
    }
}

/// One car's lamps, streaks and skid marks, after it has moved this tick.
pub fn car(w: &mut World, t: &Track, body: &mut Body, name: &str, dt: f32) {
    let car = body.car.clone();
    let speed = body.v().length();
    show(w, &format!("{name}-lamps"), car.brake && speed > 0.5);
    let streaks = car.draft > 0.35 && speed > 12.0;
    let draft = format!("{name}-draft");
    if w.require::<Emitter>(draft.as_str()).running != streaks {
        w.require_mut::<Emitter>(draft.as_str()).running = streaks;
    }
    // Rubber on the road: wheels locked on asphalt, or sliding on it. A dirt
    // car is always sliding, and water washes a slide off a wet road, so
    // there only a lock-up leaves a mark.
    let rubber = !car.air
        && car.surface == car::ASPHALT
        && speed > 4.0
        && (car.locked || (car.sliding && !t.dirt && !t.wet));
    if !rubber {
        body.car.skid_run = 0.0;
        return;
    }
    body.car.skid_run += speed * dt;
    if body.car.skid_run < SKID_EVERY {
        return;
    }
    let run = body.car.skid_run;
    body.car.skid_run = 0.0;
    let travel = body.v() / speed;
    let yaw = math::atan2(travel.y, travel.x);
    let f = body.forward();
    let l = f.perp();
    let idx = car.idx as usize;
    // The rear tyres, and the middle of the stretch they just covered.
    let rear = body.p - f * 0.9 - travel * (run * 0.5);
    let slope = t.gradient(rear, idx);
    let tilt = Quat::from_rotation_arc(Vec3::Y, Vec3::new(-slope.x, 1.0, -slope.y).normalize());
    for side in [-0.78f32, 0.78] {
        let at = rear + l * side;
        let k = {
            let mut marks = w.resource_mut::<Marks>();
            let k = marks.next;
            marks.next = (k + 1) % SKIDS as u32;
            k
        };
        let mark = format!("skid-{k}");
        *w.require_mut::<Transform>(mark.as_str()) = Transform {
            position: Vec3::new(at.x, t.height(at, idx) + 0.05 + k as f32 * 0.00005, at.y),
            rotation: tilt * Quat::from_rotation_y(-yaw),
            scale: Vec3::new(run + 0.15, 1.0, 1.0),
        };
        show(w, &mark, true);
    }
}

/// What a guide chevron tells the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Green,
    Amber,
    Red,
}

/// The cue for a chevron where the car must slow at `need` (m/s²) to make
/// the planned speed there, measured against a lift off the gas: the
/// player's only brake. Red means let go now, amber that it is coming.
pub fn guide_cue(t: &Track, need: f32) -> Cue {
    let brake = if t.dirt { car::BRAKE_DIRT } else { car::BRAKE };
    let lift = brake * car::LIFT * if t.wet { 0.8 } else { 0.85 };
    if need > 0.7 * lift {
        Cue::Red
    } else if need > 0.3 * lift {
        Cue::Amber
    } else {
        Cue::Green
    }
}

/// The player's guide and arrow: chevrons painted down the line ahead,
/// green where the car is fine as it is, amber where it must slow soon, red
/// where it has to be off the gas now; and the arrow on the side it will pass.
pub fn player(w: &mut World, t: &Track, body: &Body, assist: u32) {
    let car = &body.car;
    let side = if car.pass != 0 { car.pass_side } else { 0.0 };
    show(w, "side-arrow", side != 0.0 && !car.finished);
    if side != 0.0 {
        // The car's local +Z is the road's positive side.
        let z = 1.55 * side;
        if w.require::<Transform>("side-arrow").position.z != z {
            w.require_mut::<Transform>("side-arrow").position.z = z;
        }
    }
    let on = assist > 0 && !car.finished;
    let speed = body.v().length();
    let here = car.idx as f32 * t.step;
    let first = math::ceil((here + GUIDE_FROM) / GUIDE_EVERY) * GUIDE_EVERY;
    for k in 0..GUIDES {
        let name = format!("guide-{k}");
        show(w, &name, on);
        if !on {
            continue;
        }
        let s = first + k as f32 * GUIDE_EVERY;
        let j = t.wrap((s / t.step).round() as i64);
        let ahead = (s - here).max(1.0);
        // Deceleration needed to be at the planned speed there.
        let target = t.speed[j];
        let need = (speed * speed - target * target) / (2.0 * ahead);
        let glow = match guide_cue(t, need) {
            Cue::Red => RED,
            Cue::Amber => AMBER,
            Cue::Green => GREEN,
        };
        if w.require::<Material>(name.as_str()).emissive != glow {
            w.require_mut::<Material>(name.as_str()).emissive = glow;
        }
        let lat = (t.line[j] + car.lane).clamp(-t.edge[j], t.edge[j]);
        let p = t.at(j, lat);
        let tan = t.tangent(j);
        let yaw = math::atan2(tan.y, tan.x);
        let pose = Transform {
            position: Vec3::new(p.x, t.surface_at(j, lat) + 0.07, p.y),
            rotation: Quat::from_rotation_y(-yaw)
                * Quat::from_rotation_z(math::atan(t.grade[j]))
                * Quat::from_rotation_x(-math::atan(t.bank[j])),
            scale: Vec3::ONE,
        };
        if *w.require::<Transform>(name.as_str()) != pose {
            *w.require_mut::<Transform>(name.as_str()) = pose;
        }
    }
}
