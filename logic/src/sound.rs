//! The race's sound: every car's engine, the player's tyres and kerbs, knocks
//! between cars and off the fence, the countdown, the lap and flag stings, and
//! the music. The recordings come from infinite-driver's banks
//! (`art/src/sounds.py`); the music and stings are composed in `art/src/music.py`.
//!
//! An engine is a bank of recorded loops of a real V8 held at steady rpm, on
//! load (pulling) and off load (coasting). A voice here plays at one fixed rate,
//! so the bank is a close grid of rpms and the engine crossfades the two either
//! side of its rpm (equal power), as infinite-driver's sample engine does, and
//! the on and off sets by its load. Each car carries two slots per set: the
//! loops alternate between them up the grid, so the slot that changes loop is
//! always the one that has just faded out.
use crate::car::{self, Car};
use crate::track::{Track, HALF};
use exact_game::audio::{self, AudioListener, AudioSource, Sample, Spatial};
use exact_game::*;

/// The recorded loops' rpms (`art/src/sounds.py` prints these).
pub const ENGINE_RPMS: [u32; 26] = [
    800, 1000, 1250, 1500, 1600, 1720, 1840, 1970, 2100, 2250, 2410, 2580, 2760, 2950, 3160, 3380,
    3610, 3870, 4140, 4430, 4740, 5070, 5420, 5800, 6210, 6600,
];

/// Every baked sound setup waits for.
pub const ASSETS: [&str; 72] = [
    "engine-off-1000.sound",
    "engine-off-1250.sound",
    "engine-off-1500.sound",
    "engine-off-1600.sound",
    "engine-off-1720.sound",
    "engine-off-1840.sound",
    "engine-off-1970.sound",
    "engine-off-2100.sound",
    "engine-off-2250.sound",
    "engine-off-2410.sound",
    "engine-off-2580.sound",
    "engine-off-2760.sound",
    "engine-off-2950.sound",
    "engine-off-3160.sound",
    "engine-off-3380.sound",
    "engine-off-3610.sound",
    "engine-off-3870.sound",
    "engine-off-4140.sound",
    "engine-off-4430.sound",
    "engine-off-4740.sound",
    "engine-off-5070.sound",
    "engine-off-5420.sound",
    "engine-off-5800.sound",
    "engine-off-6210.sound",
    "engine-off-6600.sound",
    "engine-off-800.sound",
    "engine-on-1000.sound",
    "engine-on-1250.sound",
    "engine-on-1500.sound",
    "engine-on-1600.sound",
    "engine-on-1720.sound",
    "engine-on-1840.sound",
    "engine-on-1970.sound",
    "engine-on-2100.sound",
    "engine-on-2250.sound",
    "engine-on-2410.sound",
    "engine-on-2580.sound",
    "engine-on-2760.sound",
    "engine-on-2950.sound",
    "engine-on-3160.sound",
    "engine-on-3380.sound",
    "engine-on-3610.sound",
    "engine-on-3870.sound",
    "engine-on-4140.sound",
    "engine-on-4430.sound",
    "engine-on-4740.sound",
    "engine-on-5070.sound",
    "engine-on-5420.sound",
    "engine-on-5800.sound",
    "engine-on-6210.sound",
    "engine-on-6600.sound",
    "engine-on-800.sound",
    "hit-0.sound",
    "hit-1.sound",
    "hit-2.sound",
    "hit-3.sound",
    "hit-4.sound",
    "hit-5.sound",
    "kerb-0.sound",
    "kerb-1.sound",
    "kerb-2.sound",
    "kerb-3.sound",
    "music-race.sound",
    "roll-dirt.sound",
    "roll-gravel.sound",
    "scrape.sound",
    "slide.sound",
    "squeal.sound",
    "sting-count.sound",
    "sting-finish.sound",
    "sting-go.sound",
    "sting-lap.sound",
];

/// The loops that last while something does.
const LOOPS: [&str; 5] = ["squeal", "slide", "roll-gravel", "roll-dirt", "scrape"];
/// One-shots.
const SHOTS: [&str; 14] = [
    "hit-0",
    "hit-1",
    "hit-2",
    "hit-3",
    "hit-4",
    "hit-5",
    "kerb-0",
    "kerb-1",
    "kerb-2",
    "kerb-3",
    "sting-count",
    "sting-go",
    "sting-lap",
    "sting-finish",
];

pub const IDLE: f32 = 900.0;
/// The limiter.
pub const REDLINE: f32 = 6400.0;
/// Road speed (m/s) at the limiter in each gear: top gear just covers `car::TOP`.
pub const GEARS: [f32; 4] = [11.5, 19.0, 27.0, 36.0];
const UPSHIFT: f32 = 6100.0;
/// Low enough that the gear below lands under UPSHIFT, so it never hunts.
const DOWNSHIFT: f32 = 3000.0;
/// The ignition cut while a gear goes in (s).
const SHIFT: f32 = 0.11;

/// Levels.
const MUSIC: f32 = 0.32;
const ENGINE_PLAYER: f32 = 0.55;
const ENGINE_RIVAL: f32 = 0.5;
const STING: f32 = 0.5;

/// Rivals fade with distance from the player (the listener rides with them).
const SPATIAL: Spatial = Spatial {
    distance_model: audio::DistanceModel::Inverse,
    ref_distance: 7.0,
    max_distance: 10000.0,
    rolloff_factor: 1.0,
};

/// A car's engine and what its sounds last heard.
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct Engine {
    pub gear: u32,
    pub rpm: f32,
    /// 0 coasting to 1 pulling, smoothed.
    pub load: f32,
    /// Seconds of ignition cut left in a gear change.
    pub cut: f32,
    /// The last kerb block a wheel crossed.
    pub kerb: i32,
    pub last_hit: u64,
    pub crossings: u32,
    pub finished: bool,
}

/// One of a car's engine voices: its set (on or off load) and its parity in the grid.
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct Slot {
    pub car: u32,
    pub on: bool,
    pub odd: bool,
}

/// One of the player's tyre and body loops.
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct Foley {
    pub kind: u32,
}
const SQUEAL: u32 = 0;
const SLIDE: u32 = 1;
const ROLL: u32 = 2;
const SCRAPE: u32 = 3;

fn engine_name(on: bool, rpm: u32) -> String {
    format!("engine-{}-{rpm}", if on { "on" } else { "off" })
}

/// Name every sound, start the music, and give each car its engine.
pub fn setup(w: &mut World, cars: &[(u32, Entity)], dirt: bool) {
    let mut defs: Vec<(String, Sample)> = Vec::new();
    for on in [true, false] {
        for rpm in ENGINE_RPMS {
            let name = engine_name(on, rpm);
            defs.push((name.clone(), Sample::new(format!("{name}.sound")).looped()));
        }
    }
    for name in LOOPS {
        defs.push((name.into(), Sample::new(format!("{name}.sound")).looped()));
    }
    for name in SHOTS {
        defs.push((name.into(), Sample::new(format!("{name}.sound"))));
    }
    defs.push((
        "music".into(),
        Sample::new("music-race.sound").looped().gain(MUSIC),
    ));
    w.sounds(defs);
    w.play("music").ui().fade_in(1.5).start();
    w.spawn_named("ears", (Transform::default(), AudioListener));
    for &(id, e) in cars {
        w.insert(
            e,
            Engine {
                rpm: IDLE,
                ..Engine::default()
            },
        );
        // Hits play at the car, and fade as its engine does.
        w.insert(e, SPATIAL);
        // Rivals pull nearly all the time: their off-load set would be wasted voices.
        let sets: &[bool] = if id == crate::PLAYER {
            &[true, false]
        } else {
            &[true]
        };
        for &on in sets {
            for odd in [false, true] {
                let first = ENGINE_RPMS[odd as usize];
                w.spawn((
                    Transform::default(),
                    AudioSource {
                        playing: false,
                        ..AudioSource::new(engine_name(on, first)).gain(0.0)
                    },
                    SPATIAL,
                    Slot { car: id, on, odd },
                    Parent(e),
                    Ambient,
                ));
            }
        }
        if id == crate::PLAYER {
            for (kind, name) in [
                (SQUEAL, "squeal"),
                (SLIDE, "slide"),
                (ROLL, if dirt { "roll-dirt" } else { "roll-gravel" }),
                (SCRAPE, "scrape"),
            ] {
                w.spawn((
                    Transform::default(),
                    AudioSource {
                        playing: false,
                        ..AudioSource::new(name).gain(0.0)
                    },
                    SPATIAL,
                    Foley { kind },
                    Parent(e),
                    Ambient,
                ));
            }
        }
    }
}

/// Move the engine one tick: blipped on the grid, through the gears in the race.
pub fn rev(e: &mut Engine, car: &Car, racing: bool, dt: f32) {
    let gas = if car.brake { 0.0 } else { car.gas };
    if !racing {
        // On the grid: the clutch is in and the throttle is a blip.
        let target = if gas > 0.0 { 5600.0 } else { IDLE };
        let rate = if target > e.rpm { 9000.0 } else { 4500.0 };
        e.rpm += (target - e.rpm).clamp(-rate * dt, rate * dt);
        e.gear = 0;
        e.load += (gas - e.load) * (1.0 - math::exp(-dt / 0.05));
        return;
    }
    // The tyres turn the engine: wheelspin revs it, a locked wheel stalls it to idle.
    let road = car.roll.abs();
    let at = |g: u32| road / GEARS[g as usize] * REDLINE;
    let top = GEARS.len() as u32 - 1;
    if e.gear < top && (at(e.gear) > REDLINE || gas > 0.0 && at(e.gear) > UPSHIFT) {
        e.gear += 1;
        e.cut = SHIFT;
    } else if e.gear > 0 && at(e.gear) < DOWNSHIFT {
        e.gear -= 1;
    }
    let mut target = at(e.gear);
    if e.gear == 0 {
        // Pulling away the clutch slips, so the engine runs ahead of the wheels.
        target = target.max(if gas > 0.0 { 4200.0 } else { IDLE });
    }
    let target = target.clamp(IDLE, REDLINE);
    e.rpm += (target - e.rpm) * (1.0 - math::exp(-dt / 0.045));
    e.cut = (e.cut - dt).max(0.0);
    let load = if e.cut > 0.0 { 0.0 } else { gas };
    e.load += (load - e.load) * (1.0 - math::exp(-dt / 0.06));
}

/// The two loops either side of `rpm` and their equal-power gains:
/// (index of the lower, its gain, the upper's gain).
pub fn crossfade(rpm: f32) -> (usize, f32, f32) {
    let n = ENGINE_RPMS.len();
    let lo = ENGINE_RPMS[0] as f32;
    let hi = ENGINE_RPMS[n - 1] as f32;
    let rpm = rpm.clamp(lo, hi);
    let i = ENGINE_RPMS
        .iter()
        .rposition(|&r| r as f32 <= rpm)
        .unwrap_or(0)
        .min(n - 2);
    let (a, b) = (ENGINE_RPMS[i] as f32, ENGINE_RPMS[i + 1] as f32);
    let x = (math::ln(rpm / a) / math::ln(b / a)).clamp(0.0, 1.0);
    let q = x * std::f32::consts::FRAC_PI_2;
    (i, math::cos(q), math::sin(q))
}

/// What a slot should play: its loop and gain for this rpm and load.
fn slot_target(slot: &Slot, e: &Engine, level: f32) -> (u32, f32) {
    let (i, ga, gb) = crossfade(e.rpm);
    let (k, g) = if (i % 2 == 1) == slot.odd {
        (i, ga)
    } else {
        (i + 1, gb)
    };
    let q = e.load.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    let set = if slot.car != crate::PLAYER {
        // A rival's single set: a little quieter off the throttle.
        0.7 + 0.3 * e.load
    } else if slot.on {
        math::sin(q)
    } else {
        math::cos(q)
    };
    (ENGINE_RPMS[k], g * set * level)
}

/// Set a source, touching it only when something audible changes.
fn set_source(w: &mut World, e: Entity, sound: &str, gain: f32) {
    let gain = (gain * 256.0).round() / 256.0;
    let playing = gain > 0.0;
    let same = {
        let s = w.require::<AudioSource>(e);
        s.playing == playing && (!playing || s.sound == sound && s.gain == gain)
    };
    if !same {
        let mut s = w.require_mut::<AudioSource>(e);
        if playing {
            s.sound = sound.into();
            s.gain = gain;
        }
        s.playing = playing;
    }
}

use math::smoothstep;

/// A pitch near 1 for a hit, varied by when and where it is: the world's
/// random stream stays the gameplay's alone.
fn jitter(now: u64, salt: u32, spread: f32) -> f32 {
    let mut h =
        now.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (salt as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    h ^= h >> 29;
    let unit = (h % 10_000) as f32 / 10_000.0;
    1.0 + spread * (unit * 2.0 - 1.0)
}

/// Corners have kerbs (as `scenery::road` lays them); the dirt track has none.
fn kerbed(t: &Track, i: usize) -> bool {
    let n = t.len();
    !t.dirt && (0..9).any(|k| t.bend[(i + n + k - 4) % n].abs() > 1.0 / 24.0)
}

/// After gameplay: engines, the player's tyres, knocks and stings, then `audio::step`.
pub fn tick(w: &mut World, now: u64, start: u64, volume: f32) {
    if w.resource::<audio::Audio>().master != volume {
        w.resource_mut::<audio::Audio>().master = volume;
    }
    let dt = w.dt();
    let hz = w.hz() as u64;
    let racing = now >= start;
    // The countdown: a beep as each count shows, a higher one for go.
    if !racing && (start - now + 1).is_multiple_of(hz) {
        w.play("sting-count").ui().gain(STING).start();
    }
    if now == start {
        w.play("sting-go").ui().gain(STING).start();
    }
    let t = w.resource::<Track>().clone();
    let cars: Vec<(Entity, Car, Vec3)> = w
        .query::<(&Transform, &Car)>()
        .iter()
        .map(|(e, (pose, car))| (e, car.clone(), pose.position))
        .collect();
    let mut engines = [Engine::default(); crate::DRIVERS.len()];
    for (e, car, pos) in &cars {
        let mut engine = *w.require::<Engine>(*e);
        rev(&mut engine, car, racing, dt);
        let speed = Vec2::new(car.vx, car.vz).length();
        // A knock: another car, the fence, or coming down off a crest; light to heavy.
        let knock = car.knock.max(car.landing * 1.5);
        if knock > 1.8 && now - engine.last_hit > hz / 4 {
            engine.last_hit = now;
            let take = ((knock - 1.8) / 2.0).clamp(0.0, 5.0) as u32;
            let gain = (0.3 + knock / 10.0).min(1.0);
            let pitch = jitter(now, car.id, 0.08);
            w.play(&format!("hit-{take}"))
                .at(*e)
                .gain(gain)
                .pitch(pitch)
                .start();
        }
        if car.id == crate::PLAYER {
            // Over a kerb: a knock for every red block under a wheel.
            let i = car.idx as usize;
            let side = car.side.abs();
            let on_kerb = kerbed(&t, i) && side + 0.8 > HALF - 0.15 && side - 0.8 < HALF + 0.6;
            let block = (i as f32 * t.step) as i32;
            if block != engine.kerb {
                engine.kerb = block;
                if on_kerb && block % 2 == 0 && speed > 4.0 {
                    let pitch = jitter(now, 100 + block as u32, 0.12);
                    let take = block.rem_euclid(8) / 2;
                    w.play(&format!("kerb-{take}"))
                        .at(*e)
                        .gain(0.25 + 0.4 * smoothstep(4.0, 30.0, speed))
                        .pitch(pitch)
                        .start();
                }
            }
            if car.crossings > engine.crossings && car.crossings >= 2 && !car.finished {
                w.play("sting-lap").ui().gain(STING).start();
            }
            if car.finished && !engine.finished {
                w.play("sting-finish").ui().gain(STING).start();
            }
            let mut ears = w.require_mut::<Transform>("ears");
            ears.position = *pos;
        }
        engine.crossings = car.crossings;
        engine.finished = car.finished;
        *w.require_mut::<Engine>(*e) = engine;
        engines[car.id as usize] = engine;
    }
    // The engine slots.
    let slots: Vec<(Entity, Slot)> = w.query::<&Slot>().iter().map(|(e, s)| (e, *s)).collect();
    for (e, slot) in slots {
        let level = if slot.car == crate::PLAYER {
            ENGINE_PLAYER
        } else {
            ENGINE_RIVAL
        };
        let (rpm, gain) = slot_target(&slot, &engines[slot.car as usize], level);
        set_source(w, e, &engine_name(slot.on, rpm), gain);
    }
    // The player's tyres and body.
    if let Some((_, player, _)) = cars.iter().find(|(_, c, _)| c.id == crate::PLAYER) {
        let speed = Vec2::new(player.vx, player.vz).length();
        let (s, c) = math::sin_cos(player.heading);
        let sideways = Vec2::new(player.vx, player.vz)
            .perp_dot(Vec2::new(c, s))
            .abs();
        let road = player.surface == car::ASPHALT;
        let slide = smoothstep(car::SLIDE * 0.6, car::SLIDE + 6.0, sideways);
        let foley: Vec<(Entity, Foley, String)> = w
            .query::<(&Foley, &AudioSource)>()
            .iter()
            .map(|(e, (f, s))| (e, *f, s.sound.clone()))
            .collect();
        for (e, f, sound) in foley {
            let gain = match f.kind {
                // Rubber on tarmac squeals; on dirt and gravel it scrubs.
                SQUEAL => slide * if road && !t.dirt { 0.45 } else { 0.0 },
                SLIDE => slide * if road && !t.dirt { 0.15 } else { 0.5 },
                ROLL => smoothstep(1.0, 18.0, speed) * if t.dirt || !road { 0.45 } else { 0.0 },
                SCRAPE => {
                    if player.walled && speed > 2.0 {
                        0.3 + 0.4 * smoothstep(2.0, 20.0, speed)
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            };
            set_source(w, e, &sound, gain);
        }
    }
    audio::step(w);
}
