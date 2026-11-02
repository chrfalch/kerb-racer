//! The race's sound, all synthesized: the player's engine, its tyres, the
//! kerbs and the slipstream, the pack of rivals around it, bumps, landings
//! and the start lights.
//!
//! A looping sound plays at one pitch, so the engine is a ladder of loops a
//! few semitones apart: its revs crossfade between the two nearest rungs.
use crate::car::{self, Car};
use exact_game::audio::{self, AudioListener, AudioSource, Spatial, Synth};
use exact_game::*;

/// Rungs of the engine's ladder, from idle to the limiter.
pub const RUNGS: usize = 9;
const IDLE_HZ: f32 = 44.0;
/// Each rung this much higher than the last (about three semitones).
const RUNG_STEP: f32 = 1.19;
/// The tops of the gears (m/s of wheel speed).
const GEARS: [f32; 5] = [8.5, 15.0, 21.5, 28.0, 40.0];
/// Gains move in steps this size, so a loop only changes when it is heard to.
const STEP: f32 = 0.05;

/// The engine's revs (0 idle .. 1 the limiter), eased.
#[derive(Clone, Debug, Default, Resource)]
pub struct Engine {
    pub revs: f32,
}

fn rung_hz(k: usize) -> f32 {
    (0..k).fold(IDLE_HZ, |hz, _| hz * RUNG_STEP)
}

/// Name the sounds and hang the loops on the player's car, silent until the
/// first tick sets them. The ears ride with the camera; the loops don't fade
/// with its height.
pub fn setup(w: &mut World) {
    let mut sounds: Vec<(String, Synth)> = (0..RUNGS)
        .map(|k| {
            let hz = rung_hz(k);
            (
                format!("engine-{k}"),
                Synth::saw(hz)
                    .sustain(1.)
                    .lowpass_hz(hz * 7.0)
                    .seconds(1.)
                    .gain(0.16)
                    .layer(Synth::square(hz * 0.5).sustain(1.).lowpass_hz(320.).seconds(1.).gain(0.1))
                    .layer(Synth::saw(hz * 2.0).sustain(1.).lowpass_hz(hz * 9.0).seconds(1.).gain(0.04))
                    .looped(),
            )
        })
        .collect();
    sounds.extend([
        (
            "rivals".to_string(),
            Synth::saw(61.)
                .sustain(1.)
                .lowpass_hz(420.)
                .vibrato_hz(5.)
                .vibrato_depth(0.4)
                .seconds(1.)
                .gain(0.14)
                .layer(Synth::saw(83.).sustain(1.).lowpass_hz(500.).vibrato_hz(3.).vibrato_depth(0.5).seconds(1.).gain(0.1))
                .looped(),
        ),
        (
            "squeal".to_string(),
            Synth::triangle(860.)
                .sustain(1.)
                .vibrato_hz(9.)
                .vibrato_depth(0.7)
                .seconds(1.)
                .gain(0.09)
                .layer(Synth::noise().sustain(1.).highpass_hz(1800.).lowpass_hz(4200.).seconds(1.).gain(0.05))
                .looped(),
        ),
        (
            "gravel".to_string(),
            Synth::noise().sustain(1.).highpass_hz(150.).lowpass_hz(1100.).seconds(1.).gain(0.22).looped(),
        ),
        (
            "rumble".to_string(),
            Synth::square(34.).sustain(1.).lowpass_hz(220.).seconds(1.).gain(0.3).looped(),
        ),
        (
            "wind".to_string(),
            Synth::noise().sustain(1.).highpass_hz(500.).lowpass_hz(2400.).seconds(1.).gain(0.14).looped(),
        ),
        (
            "thump".to_string(),
            Synth::noise()
                .lowpass_hz(320.)
                .decay(0.16)
                .seconds(0.24)
                .gain(0.8)
                .layer(Synth::sine(75.).slide(-24.).decay(0.2).seconds(0.24).gain(0.6)),
        ),
        (
            "beep".to_string(),
            Synth::sine(660.).decay(0.25).seconds(0.26).gain(0.3),
        ),
        (
            "go".to_string(),
            Synth::sine(990.)
                .sustain(0.8)
                .release(0.2)
                .seconds(0.6)
                .gain(0.3)
                .layer(Synth::sine(1485.).sustain(0.5).release(0.2).seconds(0.6).gain(0.1)),
        ),
    ]);
    w.sounds(sounds);
    let player = w.named("player").expect("player");
    let flat = Spatial {
        rolloff_factor: 0.0,
        ..Spatial::default()
    };
    let loops = (0..RUNGS)
        .map(|k| format!("engine-{k}"))
        .chain(["rivals", "squeal", "gravel", "rumble", "wind"].map(String::from));
    for sound in loops {
        w.spawn_named(
            format!("sfx-{sound}"),
            (
                Transform::default(),
                AudioSource::new(sound).gain(0.0),
                flat,
                Parent(player),
                Ambient,
            ),
        );
    }
    w.insert_resource(Engine { revs: 0.2 });
    let camera = w.named("camera").expect("camera");
    w.insert(camera, AudioListener);
}

/// Set a loop's gain, in steps, only when it changes.
fn level(w: &mut World, sound: &str, gain: f32) {
    let gain = (gain.clamp(0.0, 2.0) / STEP).round() * STEP;
    let name = format!("sfx-{sound}");
    if w.require::<AudioSource>(name.as_str()).gain != gain {
        w.require_mut::<AudioSource>(name.as_str()).gain = gain;
    }
}

/// Where the engine wants to be: through the gears by wheel speed, or
/// revving on the spot with the throttle.
fn revs_for(car: &Car) -> f32 {
    let wheel = car.roll.max(0.0);
    if wheel < 1.0 {
        return 0.15 + 0.55 * car.gas;
    }
    let mut lo = 0.0;
    for top in GEARS {
        if wheel < top {
            let frac = (wheel - lo) / (top - lo);
            let floor = if lo == 0.0 { 0.15 } else { 0.42 };
            return floor + (1.0 - floor) * frac;
        }
        lo = top;
    }
    1.0
}

/// This tick's sound: call after the race has moved the cars.
pub fn step(w: &mut World, nearest_rival: f32, loose: bool) {
    let dt = w.dt();
    let car = w.require::<Car>("player").clone();
    let speed = Vec2::new(car.vx, car.vz).length();
    let want = revs_for(&car);
    // Up fast under throttle, down a little slower; a gear change drops at once.
    let revs = {
        let mut engine = w.resource_mut::<Engine>();
        let rate = if want > engine.revs { 9.0 } else { 6.0 };
        if want < engine.revs - 0.3 {
            engine.revs = want;
        } else {
            engine.revs += (want - engine.revs) * (dt * rate).min(1.0);
        }
        engine.revs
    };
    // Louder under load.
    let volume = 0.5 + 0.5 * car.gas;
    let x = revs.clamp(0.0, 1.0) * (RUNGS - 1) as f32;
    let low = (math::floor(x) as usize).min(RUNGS - 2);
    let frac = x - low as f32;
    for k in 0..RUNGS {
        let gain = if k == low {
            1.0 - frac
        } else if k == low + 1 {
            frac
        } else {
            0.0
        };
        level(w, &format!("engine-{k}"), gain * volume);
    }
    let ground = !car.air;
    let slide = if ground && (car.sliding || car.locked) && speed > 4.0 {
        (Vec2::new(car.vx, car.vz).length() / 20.0).clamp(0.4, 1.0)
    } else {
        0.0
    };
    level(w, "squeal", if loose { 0.0 } else { slide });
    let off = car.surface != car::ASPHALT;
    let gravel = if ground && (loose || off) && speed > 2.0 {
        (speed / 25.0).clamp(0.2, 1.0) * if loose && !off { 0.6 } else { 1.0 }
    } else {
        0.0
    };
    level(w, "gravel", gravel);
    level(w, "rumble", if car.kerb && speed > 4.0 { 1.0 } else { 0.0 });
    level(w, "wind", if speed > 14.0 { car.draft * 1.2 } else { 0.0 });
    level(w, "rivals", ((22.0 - nearest_rival) / 18.0).clamp(0.0, 1.0) * 0.9);
    if car.bump > 2.0 {
        w.play("thump").ui().gain((car.bump / 8.0).clamp(0.25, 1.0)).start();
    }
    if car.landing > 1.0 {
        w.play("thump").ui().gain((car.landing / 6.0).clamp(0.2, 0.8)).pitch(0.8).start();
    }
    audio::step(w);
}

/// The start lights: a beep on each count, a higher one on go.
pub fn lights(w: &mut World, now: u64, start: u64) {
    let hz = w.hz() as u64;
    if now == start {
        w.play("go").ui().start();
    } else if now < start && (start - now).is_multiple_of(hz) {
        w.play("beep").ui().start();
    }
}
