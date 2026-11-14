//! The car body tilts with the road it drives on, for the eye only.

use exact_game::{Entity, Sim, Vec2, Vec3};
use kerb_racers_logic::car::{self, Body, Car};
use kerb_racers_logic::track::{self, Track};
use kerb_racers_logic::{KerbRacers, Options, Wheel};

/// Drive the racing line for a third of a second from `s` metres into the lap,
/// coasting at the planned speed (at most 20 m/s): long enough for the body to
/// settle, short enough to stay on the same stretch.
fn settle(t: &Track, s: f32) -> Body {
    let i = (s / t.step) as usize;
    let p = t.at(i, t.line[i]);
    let f = t.tangent(i);
    let v = t.speed[i].min(20.0);
    let mut body = Body {
        car: Car {
            idx: i as u32,
            heading: f.y.atan2(f.x),
            vx: f.x * v,
            vz: f.y * v,
            y: t.surface(p, i),
            vy: t.grade[i] * v,
            load: 1.0,
            skill: 1.0,
            ..Car::default()
        },
        p,
    };
    car::locate(t, &mut body);
    for _ in 0..40 {
        car::step(t, &mut body, 1.0 / 120.0);
    }
    body
}

/// Up and down the steepest stretches the nose tilts clearly with the road:
/// more than the grade itself, so a 10–15% hill reads from the camera's height.
#[test]
fn the_body_tilts_nose_up_climbing_and_nose_down_descending() {
    // (track, metres into the lap): Pine Hollow's climb and run down, Misty Moor's.
    for (id, s) in [(0, 120.0), (0, 197.0), (4, 66.0), (4, 160.0)] {
        let t = track::build(id);
        let body = settle(&t, s);
        let grade = t.grade[body.car.idx as usize];
        let true_pitch = grade.atan();
        println!(
            "{} at {s} m: grade {grade:.3} ({:.1}°), body pitch {:.1}°",
            track::layout(id).name,
            true_pitch.to_degrees(),
            body.car.pitch.to_degrees()
        );
        assert!(grade.abs() > 0.08, "not a hill: {grade}");
        assert!(
            body.car.pitch * grade.signum() > 1.3 * true_pitch.abs(),
            "{} at {s} m: pitch {:.1}° on a {:.1}° grade",
            track::layout(id).name,
            body.car.pitch.to_degrees(),
            true_pitch.to_degrees()
        );
        assert!(body.car.pitch.abs() < 0.35, "clamped: {}", body.car.pitch);
    }
}

/// Across a banked corner the body leans with the road: down toward its low side.
#[test]
fn the_body_leans_with_a_banked_road() {
    // (track, metres into the lap): Fern Loop's banked sweeper, Gravel Pit's.
    for (id, s) in [(1, 39.0), (3, 259.0)] {
        let t = track::build(id);
        let body = settle(&t, s);
        let bank = t.bank[body.car.idx as usize];
        println!(
            "{} at {s} m: bank {bank:.3} ({:.1}°), body lean {:.1}°",
            track::layout(id).name,
            bank.atan().to_degrees(),
            body.car.lean.to_degrees()
        );
        assert!(bank.abs() > 0.06, "not banked: {bank}");
        // Higher on the left leans the body's top to the right: negative.
        assert!(
            -body.car.lean * bank.signum() > 0.8 * bank.abs().atan(),
            "{} at {s} m: lean {:.1}° on a {:.1}° bank",
            track::layout(id).name,
            body.car.lean.to_degrees(),
            bank.atan().to_degrees()
        );
    }
}

/// Over a crest, in a dip or across a bank, wherever a car is on the road, the
/// body tilted with the ground neither sinks a wheel into it nor leaves one in
/// the air: the high wheels drop to the road on their springs, within reach.
#[test]
fn the_wheels_stay_on_the_road_as_the_body_tilts() {
    // Every car's wheels on the body (forward, left; m), from `wheel_layout`.
    let wheels = [(0.908, 0.72), (-0.862, -0.72), (0.949, 0.75), (-0.906, -0.75), (0.976, -0.731), (-0.944, 0.731)];
    for id in 0..track::LAYOUTS.len() as u32 {
        let t = track::build(id);
        let name = track::layout(id).name;
        let (mut sink, mut float, mut drop) = (0f32, 0f32, 0f32);
        for k in 0..(t.length() * 2.0) as usize {
            let i = ((k as f32 * 0.5 / t.step) as usize).min(t.len() - 1);
            let f = t.tangent(i);
            let heading = f.y.atan2(f.x);
            for lat in [-3.0, 0.0, t.line[i], 3.0] {
                let p = t.at(i, lat);
                let car::Tilt { pitch, lean, .. } = car::ground_tilt(&t, p, i, heading);
                let mut body = Body {
                    car: Car { idx: i as u32, heading, pitch, lean, y: t.surface(p, i), ..Car::default() },
                    p,
                };
                body.car.rise = car::lift(&t, &body);
                let base = Vec3::new(p.x, body.car.y + body.car.rise, p.y);
                let rot = car::body_rotation(&body.car);
                for (along, across) in wheels {
                    let hang = car::wheel_drop(&t, &body, Vec2::new(along, across));
                    let contact = base + rot * Vec3::new(along, -hang, across);
                    let gap = contact.y - t.height(Vec2::new(contact.x, contact.z), i);
                    (sink, float, drop) = (sink.min(gap), float.max(gap), drop.max(hang));
                }
            }
        }
        println!("{name}: wheels at most {:.3} m into the road, {float:.3} m above it, hanging {drop:.3} m", -sink);
        assert!(sink > -0.03, "{name}: a wheel {:.3} m into the road", -sink);
        assert!(float < 0.03, "{name}: a wheel {float:.3} m above the road");
    }
}

/// A body diving and rolling as hard as it ever does, either way, out on the
/// run-off at the samples `along`: its outer wheels where the ground bends
/// from the road into the land. How deep any wheel goes into the ground and
/// how high one is left above it (m), and where (m into the lap, offset).
fn hard_corners(t: &Track, along: impl Iterator<Item = usize>) -> (f32, f32, (f32, f32)) {
    let wheels = [(0.908, 0.72), (-0.862, -0.72), (0.949, 0.75), (-0.906, -0.75), (0.976, -0.731), (-0.944, 0.731)];
    let (mut sink, mut float, mut at) = (0f32, 0f32, (0.0, 0.0));
    for i in along {
        let f = t.tangent(i);
        let heading = f.y.atan2(f.x);
        // Wherever the car itself is on the road's own plane: inside a
        // hairpin tighter than the band that ends short of the run-off.
        for lat in [-6.0f32, -5.5, -5.0, 5.0, 5.5, 6.0] {
            if lat.abs() > if lat > 0.0 { t.reach_left[i] } else { t.reach_right[i] } {
                continue;
            }
            let p = t.at(i, lat);
            let tilt = car::ground_tilt(t, p, i, heading);
            for (dive, roll) in [(-0.07, -0.08), (-0.07, 0.08), (0.03, -0.08), (0.03, 0.08)] {
                let mut body = Body {
                    car: Car { idx: i as u32, heading, pitch: tilt.pitch + dive, lean: tilt.lean + roll, y: t.surface(p, i), ..Car::default() },
                    p,
                };
                car::rest_on_ground(t, &mut body, tilt.ground);
                let base = Vec3::new(p.x, body.car.y + body.car.rise, p.y);
                let rot = car::body_rotation(&body.car);
                for (along, across) in wheels {
                    let hang = car::wheel_drop(t, &body, Vec2::new(along, across));
                    let contact = base + rot * Vec3::new(along, -hang, across);
                    let gap = contact.y - t.height(Vec2::new(contact.x, contact.z), i);
                    if gap > float {
                        at = (i as f32 * t.step, lat);
                    }
                    (sink, float) = (sink.min(gap), float.max(gap));
                }
            }
        }
    }
    (sink, float, at)
}

/// On Gravel Pit's run-off down into the pit, a body diving and rolling as
/// hard as it ever does still reaches the ground with every wheel. (Cornering
/// hard across it, the player's car once drew a wheel 5 cm up.)
#[test]
fn the_wheels_reach_the_run_off_in_a_hard_corner() {
    let t = track::build(3);
    let (sink, float, at) = hard_corners(&t, (0..48).map(|k| ((138.0 + k as f32 * 0.25) / t.step) as usize));
    println!("Gravel Pit's pit run-off: wheels at most {:.3} m into the ground, {float:.3} m above it (at {at:?})", -sink);
    assert!(float < 0.02, "a wheel {float:.3} m above the ground at {at:?}");
}

/// All round every track's run-off, no wheel is left more than 3.5 cm above
/// the ground. Before each wheel's reach was checked they hung up to 0.08 m
/// (Fern Loop), 0.09 m (Misty Moor), 0.27 m (Gravel Pit's berm) and 0.28 m
/// (Kerb Ring's off-camber descent). What is left, at most 0.033 m on Gravel
/// Pit's berm, comes from each car's own wheel places standing a little
/// beyond the corners the reach is checked at. Out there a wheel can also be
/// drawn up to 0.1 m into the ground, for the same reason; not checked here.
#[test]
fn the_wheels_reach_every_run_off_in_a_hard_corner() {
    let mut high = Vec::new();
    for id in 0..track::LAYOUTS.len() as u32 {
        let t = track::build(id);
        let samples = (0..(t.length() * 2.0) as usize).map(|k| ((k as f32 * 0.5 / t.step) as usize).min(t.len() - 1));
        let (sink, float, at) = hard_corners(&t, samples);
        println!("{}: wheels at most {:.3} m into the ground, {float:.3} m above it (at {at:?})", track::layout(id).name, -sink);
        if float >= 0.035 {
            high.push(format!("{}: a wheel {float:.3} m above the ground at {at:?}", track::layout(id).name));
        }
    }
    assert!(high.is_empty(), "{}", high.join("\n"));
}

fn race(track: u32) -> Sim<KerbRacers> {
    let mut sim = Sim::<KerbRacers>::new(Options { track, ..Options::default() }).unwrap();
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/");
    let _ = sim.load_assets(|name| std::fs::read(format!("{assets}{name}")));
    sim
}

/// Everything about a car the race reads back: the pose for drawing left out.
fn physics(game: &Sim<KerbRacers>) -> Vec<String> {
    game.world()
        .query::<&Car>()
        .iter()
        .map(|(_, c)| {
            let mut c = c.clone();
            (c.pitch, c.lean, c.rise) = (0.0, 0.0, 0.0);
            format!("{c:?}")
        })
        .collect()
}

/// The pose is for the eye only: scramble every car's tilt mid-race and the
/// race runs on exactly as it would have.
#[test]
fn the_pose_does_not_change_the_race() {
    let run = |scramble: bool| {
        let mut game = race(4);
        game.key_down("KeyW");
        game.run(5000.0);
        if scramble {
            let w = game.world_mut();
            let cars: Vec<Entity> = w.query::<&Car>().iter().map(|(e, _)| e).collect();
            for e in cars {
                let mut c = w.require_mut::<Car>(e);
                (c.pitch, c.lean, c.rise) = (0.9, -0.7, 0.5);
            }
        }
        game.run(6000.0);
        physics(&game)
    };
    assert_eq!(run(false), run(true));
}

/// What the player's body did over a lap and a half on the gas (the full
/// assist lifts for the corners): its drawn pitch on the ground, by stretch.
#[derive(Debug, Default)]
struct Ride {
    /// The steepest pitch (°, signed) up the climbs and down the descents
    /// steeper than 10%, and the mean pitch there.
    climb: f32,
    descent: f32,
    climb_mean: f32,
    descent_mean: f32,
    /// The deepest any wheel went into the road, and the highest above it (m).
    sink: f32,
    float: f32,
}

fn ride(track: u32) -> Ride {
    let mut game = Sim::<KerbRacers>::new(Options { track, assist: 2, ..Options::default() }).unwrap();
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/");
    let _ = game.load_assets(|name| std::fs::read(format!("{assets}{name}")));
    game.run(3500.0);
    game.key_down("KeyW");
    let mut ride = Ride::default();
    let (mut up, mut down) = (Vec::new(), Vec::new());
    for _ in 0..(120 * 40) {
        game.run(1000.0 / 120.0);
        let w = game.world();
        let t = w.resource::<Track>();
        let c = w.require::<Car>("player").clone();
        if c.air || c.air_ticks > 0 {
            continue;
        }
        let i = c.idx as usize;
        let (pitch, grade) = (c.pitch.to_degrees(), t.grade[i]);
        if grade > 0.1 {
            up.push(pitch);
        } else if grade < -0.1 {
            down.push(pitch);
        }
        // The wheels as drawn: under the body, each dropped to the road.
        let pose = *w.require::<exact_game::Transform>("player");
        let body = Body { car: c.clone(), p: Vec2::new(pose.position.x, pose.position.z) };
        let player = w.named("player").unwrap();
        for (_, (wheel, parent, _)) in w.query::<(&exact_game::Transform, &exact_game::Parent, &Wheel)>().iter() {
            if parent.0 != player {
                continue;
            }
            let at = Vec2::new(wheel.position.x, wheel.position.z);
            let hang = car::wheel_drop(&t, &body, at);
            let contact = pose.position + pose.rotation * Vec3::new(at.x, -c.squash - hang, at.y);
            let gap = contact.y - t.height(Vec2::new(contact.x, contact.z), i);
            (ride.sink, ride.float) = (ride.sink.min(gap), ride.float.max(gap));
        }
    }
    let peak = |v: &[f32], sign: f32| v.iter().map(|p| p * sign).fold(f32::MIN, f32::max) * sign;
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
    (ride.climb, ride.descent) = (peak(&up, 1.0), peak(&down, -1.0));
    (ride.climb_mean, ride.descent_mean) = (mean(&up), mean(&down));
    ride
}

/// Racing down Pine Hollow's and Misty Moor's steepest hills, the nose points
/// down at least as hard as it points up on the way up.
#[test]
fn descents_point_the_nose_down_as_clearly_as_climbs_point_it_up() {
    for id in [0, 4] {
        let name = track::layout(id).name;
        let r = ride(id);
        println!(
            "{name}: climbs up to {:.1}° (mean {:.1}°), descents down to {:.1}° (mean {:.1}°)",
            r.climb, r.climb_mean, r.descent, r.descent_mean
        );
        assert!(-r.descent >= r.climb, "{name}: down to {:.1}° against up to {:.1}°", r.descent, r.climb);
        assert!(-r.descent_mean >= r.climb_mean, "{name}: down {:.1}° on average against up {:.1}°", r.descent_mean, r.climb_mean);
    }
}

/// Racing over the crests, dips and jumps, the wheels as drawn stay on the
/// road: none sinks into it or hangs above it by more than 2 cm.
#[test]
fn the_wheels_stay_on_the_road_in_a_race() {
    for id in [0, 4, 3] {
        let name = track::layout(id).name;
        let r = ride(id);
        println!("{name}: wheels at most {:.3} m into the road, {:.3} m above it", -r.sink, r.float);
        assert!(r.sink > -0.02, "{name}: a wheel {:.3} m into the road", -r.sink);
        assert!(r.float < 0.02, "{name}: a wheel {:.3} m above the road", r.float);
    }
}

/// The body's steepest pitch (°, either way) on the ground over Gravel Pit's
/// whoops, coasting into them from 8 m before the line at `speed` on the
/// racing line.
fn whoops_pitch(t: &Track, speed: f32) -> f32 {
    let i = t.wrap(-(8.0 / t.step) as i64);
    let p = t.at(i, t.line[i]);
    let f = t.tangent(i);
    let mut body = Body {
        car: Car {
            idx: i as u32,
            heading: f.y.atan2(f.x),
            vx: f.x * speed,
            vz: f.y * speed,
            y: t.surface(p, i),
            vy: t.grade[i] * speed,
            load: 1.0,
            skill: 1.0,
            ..Car::default()
        },
        p,
    };
    car::locate(t, &mut body);
    let mut steepest = 0f32;
    for _ in 0..(120 * 6) {
        car::step(t, &mut body, 1.0 / 120.0);
        let s = body.car.idx as f32 * t.step;
        if (34.0..t.length() / 2.0).contains(&s) {
            break;
        }
        if (4.0..34.0).contains(&s) && !body.car.air && body.car.air_ticks == 0 {
            steepest = steepest.max(body.car.pitch.to_degrees().abs());
        }
    }
    steepest
}

/// Gravel Pit's whoops already rock the car: they must not get busier at any
/// speed they are taken at. The limits are each speed's steepest before the
/// springy crests, less than a tenth of a degree over: the racing line out of
/// the last jump's landing runs a little differently into them, which reads
/// up to 0.06° steeper. (The race that used to guard this measured traffic:
/// a lap entered faster reads steeper whatever the pose does.)
#[test]
fn the_whoops_rock_no_harder() {
    let t = track::build(3);
    for (speed, before) in [(10.0, 12.28), (12.0, 12.74), (14.0, 12.02), (16.0, 11.56), (18.0, 11.44), (20.0, 11.19)] {
        let pitch = whoops_pitch(&t, speed);
        println!("Gravel Pit's whoops at {speed} m/s: up to {pitch:.2}° (before {before:.2}°)");
        assert!(pitch <= before + 0.1, "at {speed} m/s the whoops rock {pitch:.2}°, before {before:.2}°");
    }
}
