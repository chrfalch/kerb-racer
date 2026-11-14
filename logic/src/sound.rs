//! The race's sound: every car's engine and its overrun pops, the player's
//! tyres and kerbs, knocks between cars and off the fence, the countdown, the
//! lap and flag stings, and the music. The engines are synthesized in
//! `art/src/engines.py`, the foley comes from infinite-driver's banks
//! (`art/src/sounds.py`), and the music and stings are composed in
//! `art/src/music.py`.
//!
//! Each body has its own engine ([`Layout`]): a bank of loops held at steady
//! rpm, on load (pulling) and off load (coasting). A voice here plays at one
//! fixed rate, so the bank is a close grid of rpms and the engine crossfades
//! the two either side of its rpm (equal power), as infinite-driver's sample
//! engine does, and the on and off sets by its load. Each car carries two slots
//! per set: the loops alternate between them up the grid, so the slot that
//! changes loop is always the one that has just faded out.
use crate::car::{self, Car};
use crate::track::{Track, HALF};
use exact_game::audio::{self, AudioListener, AudioSource, Sample, Spatial};
use exact_game::*;

/// Calls `$then!` with the bank's rpms (`art/src/engines.py` prints them), so
/// every list built from them agrees.
macro_rules! rpms {
    ($then:ident!($($arg:tt)*)) => {
        $then!($($arg)* 800 1000 1250 1500 1600 1720 1840 1970 2100 2250 2410 2580 2760 2950 3160 3380 3610 3870 4140 4430)
    };
}
macro_rules! array {
    ($($rpm:literal)*) => { [$($rpm),*] };
}
macro_rules! bank {
    ($layout:literal $($rpm:literal)*) => {
        [$(
            concat!("engine-", $layout, "-on-", $rpm, ".sound"),
            concat!("engine-", $layout, "-off-", $rpm, ".sound"),
        )*]
    };
}

/// Calls `$then!` with the pops' rpms (`art/src/engines.py` prints them).
macro_rules! pop_rpms {
    ($then:ident!($($arg:tt)*)) => {
        $then!($($arg)* 2580 2950 3380 3870)
    };
}
macro_rules! pops {
    ($layout:literal $($rpm:literal)*) => {
        [$(
            concat!("pop-", $layout, "-", $rpm, "-0.sound"),
            concat!("pop-", $layout, "-", $rpm, "-1.sound"),
        )*]
    };
}

pub const ENGINE_RPMS: [u32; 20] = rpms!(array!());
/// Each engine's pops are rendered at these rpms, in its own note there;
/// the nearest is bent to the engine's rpm as it plays.
pub const POP_RPMS: [u32; 4] = pop_rpms!(array!());
const POP_TAKES: u32 = 2;

/// A body's engine, which names its bank of loops and its pops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// The 70s sports cars' air-cooled flat six.
    Flat6,
    /// The mid-engine rally cars' V6.
    V6,
    /// The rally saloons' four.
    I4,
}

impl Layout {
    pub const ALL: [Layout; 3] = [Layout::Flat6, Layout::V6, Layout::I4];

    /// Each car's engine, by its body (`art/src/cars.py`).
    pub fn of(car: u32) -> Layout {
        match car {
            0 | 4 => Layout::Flat6,
            1 | 5 => Layout::V6,
            _ => Layout::I4,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Layout::Flat6 => "flat6",
            Layout::V6 => "v6",
            Layout::I4 => "i4",
        }
    }

    /// How many pops a lift sets off (fewest, most), and how loud as a share
    /// of the engine's own level: the rally engines bark, the sports car
    /// burbles. With `pop`'s 10% variation a pop never outplays its engine.
    fn overrun(self) -> (u32, u32, f32) {
        match self {
            Layout::Flat6 => (1, 2, 0.55),
            Layout::V6 => (1, 3, 0.8),
            Layout::I4 => (2, 3, 0.85),
        }
    }
}

const FOLEY_ASSETS: [&str; 20] = [
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

/// Every baked sound setup waits for.
pub const ASSETS: [&str;
    Layout::ALL.len() * (2 * ENGINE_RPMS.len() + POP_TAKES as usize * POP_RPMS.len())
        + FOLEY_ASSETS.len()] = join(&[
    &rpms!(bank!("flat6")),
    &rpms!(bank!("v6")),
    &rpms!(bank!("i4")),
    &pop_rpms!(pops!("flat6")),
    &pop_rpms!(pops!("v6")),
    &pop_rpms!(pops!("i4")),
    &FOLEY_ASSETS,
]);

/// `lists` end to end, which must hold exactly `N` names.
const fn join<const N: usize>(lists: &[&[&'static str]]) -> [&'static str; N] {
    let mut all = [""; N];
    let (mut i, mut l) = (0, 0);
    while l < lists.len() {
        let mut k = 0;
        while k < lists[l].len() {
            all[i] = lists[l][k];
            i += 1;
            k += 1;
        }
        l += 1;
    }
    assert!(i == N, "the asset lists do not add up");
    all
}

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
/// The limiter: the top loop of every bank.
pub const REDLINE: f32 = 4430.0;
/// Road speed (m/s) at the limiter in each gear: top gear puts `car::TOP` near 4000 rpm.
pub const GEARS: [f32; 5] = [14.0, 18.0, 23.0, 29.5, 37.5];
/// Well under the limiter, so the engine spends the race in its mid revs.
const UPSHIFT: f32 = 3300.0;
/// Under where an upshift lands (2570 at the widest step), and low enough that
/// the gear below lands under UPSHIFT, so it never hunts.
const DOWNSHIFT: f32 = 2000.0;
/// The revs with the clutch in: a blip on the grid, and pulling away.
const BLIP: f32 = 3800.0;
const CLUTCH: f32 = 2800.0;
/// The ignition cut while a gear goes in (s).
const SHIFT: f32 = 0.11;

/// Levels.
const MUSIC: f32 = 0.32;
/// Kept low: any louder and the engines read too heavy at full throttle.
const ENGINE_PLAYER: f32 = 0.4125;
const ENGINE_RIVAL: f32 = 0.375;
const STING: f32 = 0.5;
/// The player's tyres squeal from this sideways speed (m/s), well before the
/// slide that lays skid marks (`car::SLIDE`), up to their loudest here.
const SQUEAL_FROM: f32 = 1.5;
const SQUEAL_FULL: f32 = 6.0;
const SQUEAL_LEVEL: f32 = 0.7;

/// Overrun: lifting off from over `POP_FROM` rpm, after the throttle has been
/// open at least `POP_HELD` seconds (so feathering never crackles), pops the
/// exhaust within `POP_WINDOW` seconds; never more than `POPS_AT_ONCE` sound together.
const POP_FROM: f32 = 2500.0;
const POP_HELD: f32 = 0.3;
const POP_WINDOW: f32 = 0.6;
const POPS_AT_ONCE: usize = 3;

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
    /// 0 coasting to 1 pulling, smoothed; past 1 labouring up a hill.
    pub load: f32,
    /// Seconds of ignition cut left in a gear change.
    pub cut: f32,
    /// The last kerb block a wheel crossed.
    pub kerb: i32,
    pub last_hit: u64,
    pub crossings: u32,
    pub finished: bool,
    /// Seconds the throttle has been open.
    pub open: f32,
    /// The tick of the last lift from high revs, whose pops follow.
    pub lift: Option<u64>,
}

/// One of a car's engine voices: its set (on or off load) and its parity in the grid.
/// The player's is an `AudioSource` on the slot; a rival's is a looping voice,
/// which can bend in pitch, held while it is audible.
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct Slot {
    pub car: u32,
    pub on: bool,
    pub odd: bool,
    pub voice: Option<audio::VoiceId>,
}

/// Where a car's engine and pops are heard from: an anchor at the car's true
/// distance from the ears, turned so that Exact's pan (the offset across over
/// the distance) comes out at `pan`; and the Doppler bend of its pitch.
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct Heard {
    pub car: u32,
    pub pan: f32,
    pub doppler: f32,
}

/// The pan curve: full strength `PAN_MOST`, the offset along the screen
/// counted at `PAN_DEPTH` of the offset across, and `PAN_NEAR` metres that
/// keep a car passing right over the player from snapping side to side.
const PAN_MOST: f32 = 0.85;
const PAN_DEPTH: f32 = 0.4;
const PAN_NEAR: f32 = 2.0;
/// The speed of sound for the Doppler bend (m/s), under half the real one so
/// a pass is heard, and the most it bends either way (about a semitone).
const SOUND_SPEED: f32 = 150.0;
const DOPPLER_MOST: f32 = 0.08;
/// Seconds the bend takes to follow, so it glides rather than steps.
const DOPPLER_EASE: f32 = 0.15;

/// The pan (-1 left to 1 right) of a car `offset` from the player on the
/// ground, x across the screen and y down it: the camera looks straight down
/// the track, so world x is screen x. Exact's own pan, x over the distance,
/// leaves a car a little ahead nearly central; this one gives the side more weight.
pub fn pan(offset: Vec2) -> f32 {
    let depth = PAN_DEPTH * offset.y;
    PAN_MOST * offset.x / (offset.x * offset.x + depth * depth + PAN_NEAR * PAN_NEAR).sqrt()
}

/// How much a car's pitch bends, heard by a listener: above 1 closing, below
/// 1 drawing apart, clamped to `DOPPLER_MOST`.
pub fn doppler(source: Vec2, source_v: Vec2, listener: Vec2, listener_v: Vec2) -> f32 {
    let towards = (listener - source).normalize_or_zero();
    let bend = (SOUND_SPEED - listener_v.dot(towards)) / (SOUND_SPEED - source_v.dot(towards));
    bend.clamp(1.0 - DOPPLER_MOST, 1.0 + DOPPLER_MOST)
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

fn engine_name(layout: Layout, on: bool, rpm: u32) -> String {
    format!(
        "engine-{}-{}-{rpm}",
        layout.name(),
        if on { "on" } else { "off" }
    )
}

/// Name every sound, start the music, and give each car its engine.
pub fn setup(w: &mut World, cars: &[(u32, Entity)], dirt: bool) {
    let mut defs: Vec<(String, Sample)> = Vec::new();
    for layout in Layout::ALL {
        for on in [true, false] {
            for rpm in ENGINE_RPMS {
                let name = engine_name(layout, on, rpm);
                defs.push((name.clone(), Sample::new(format!("{name}.sound")).looped()));
            }
        }
        for rpm in POP_RPMS {
            for take in 0..POP_TAKES {
                let name = format!("pop-{}-{rpm}-{take}", layout.name());
                defs.push((name.clone(), Sample::new(format!("{name}.sound"))));
            }
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
        if id == crate::PLAYER {
            for on in [true, false] {
                for odd in [false, true] {
                    let first = ENGINE_RPMS[odd as usize];
                    w.spawn((
                        Transform::default(),
                        AudioSource {
                            playing: false,
                            ..AudioSource::new(engine_name(Layout::of(id), on, first)).gain(0.0)
                        },
                        SPATIAL,
                        Slot {
                            car: id,
                            on,
                            odd,
                            voice: None,
                        },
                        Parent(e),
                        Ambient,
                    ));
                }
            }
        } else {
            w.spawn((
                Transform::default(),
                SPATIAL,
                Heard {
                    car: id,
                    pan: 0.0,
                    doppler: 1.0,
                },
                Ambient,
            ));
            // Rivals pull nearly all the time: their off-load set would be wasted voices.
            for odd in [false, true] {
                w.spawn((
                    Slot {
                        car: id,
                        on: true,
                        odd,
                        voice: None,
                    },
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

/// How much a climb raises a pulling engine's load per unit of grade, and the
/// most it raises or lowers it, as shares of the throttle: flat out up a hill
/// the engine labours past full load.
const CLIMB_LOAD: f32 = 1.6;
const CLIMB_MOST: f32 = 0.3;

/// Move the engine one tick: blipped on the grid, through the gears in the
/// race, labouring up a `climb` (the road's rise per metre along the car's
/// heading) and easing down one.
pub fn rev(e: &mut Engine, car: &Car, climb: f32, racing: bool, dt: f32) {
    let gas = if car.brake { 0.0 } else { car.gas };
    if !racing {
        // On the grid: the clutch is in and the throttle is a blip.
        let target = if gas > 0.0 { BLIP } else { IDLE };
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
        target = target.max(if gas > 0.0 { CLUTCH } else { IDLE });
    }
    let target = target.clamp(IDLE, REDLINE);
    e.rpm += (target - e.rpm) * (1.0 - math::exp(-dt / 0.045));
    e.cut = (e.cut - dt).max(0.0);
    // Off the throttle a hill changes nothing: only a pulling engine feels it.
    let load = if e.cut > 0.0 {
        0.0
    } else {
        gas * (1.0 + (climb * CLIMB_LOAD).clamp(-CLIMB_MOST, CLIMB_MOST))
    };
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

fn level(car: u32) -> f32 {
    if car == crate::PLAYER {
        ENGINE_PLAYER
    } else {
        ENGINE_RIVAL
    }
}

/// A rival's single set: a little quieter off the throttle.
fn rival_set(load: f32) -> f32 {
    0.7 + 0.3 * load
}

/// How loud a car's engine plays as a whole: its slots' gains add up to this
/// in power, since loops and the player's two sets crossfade at equal power.
fn engine_level(car: u32, load: f32) -> f32 {
    level(car)
        * if car == crate::PLAYER {
            1.0
        } else {
            rival_set(load.clamp(0.0, 1.0))
        }
}

/// What a slot should play: its loop and gain for this rpm and load.
fn slot_target(slot: &Slot, e: &Engine, level: f32) -> (u32, f32) {
    let (i, ga, gb) = crossfade(e.rpm);
    let (k, g) = if (i % 2 == 1) == slot.odd {
        (i, ga)
    } else {
        (i + 1, gb)
    };
    let load = e.load.clamp(0.0, 1.0);
    let q = load * std::f32::consts::FRAC_PI_2;
    let set = if slot.car != crate::PLAYER {
        rival_set(load)
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

/// Seconds an audible rival slot takes to fade out of one loop, and then into the next.
const LOOP_SWAP: f32 = 0.02;
/// Quieter than this a slot just stops: the cut is inaudible.
const HANDOVER_FROM: f32 = 0.02;

/// Keep a rival's slot voice playing `sound` at `gain` and `pitch` from its
/// anchor, started as it becomes audible. A gear change can move a slot to a
/// new loop while it is still loud: its voice then fades out over `LOOP_SWAP`
/// and the new loop fades in from the boundary where it ends, so a slot never
/// holds two voices and the loops stay within budget. A quiet one just stops.
fn set_voice(w: &mut World, e: Entity, anchor: Entity, sound: &str, gain: f32, pitch: f32) {
    let gain = (gain * 256.0).round() / 256.0;
    let mut slot = *w.require::<Slot>(e);
    // A voice authored this tick begins at the next boundary, where one whose
    // fade ends there is dropped: the next loop takes over without a gap.
    let boundary = w.tick() + 1;
    let playing = slot.voice.map(|id| {
        let voices = w.resource::<audio::Voices>();
        let v = voices
            .voices
            .iter()
            .find(|v| v.id == id && v.ends > boundary)?;
        let fading = v.fade.is_some_and(|f| f.end == 0.0);
        Some((id, v.sound == sound, v.gain, v.pitch, fading))
    });
    let handed_over = match playing {
        // Fading out, or still on its loop: it follows the slot's gain and bend.
        Some(Some((id, same, was, bent, fading))) if fading || same && gain > 0.0 => {
            if was != gain {
                let mut voices = w.resource_mut::<audio::Voices>();
                if let Some(v) = voices.voices.iter_mut().find(|v| v.id == id) {
                    v.gain = gain;
                }
            }
            if (bent - pitch).abs() > 1e-4 {
                audio::pitch(w, id, pitch);
            }
            return;
        }
        Some(Some((id, _, was, _, _))) if was >= HANDOVER_FROM => {
            audio::pitch(w, id, pitch);
            return audio::fade(w, id, LOOP_SWAP);
        }
        Some(Some((id, ..))) => {
            audio::stop(w, id);
            false
        }
        // The voice it was fading out has gone, or goes at this boundary.
        Some(None) => true,
        None => false,
    };
    slot.voice = (gain > 0.0).then(|| {
        let play = w.play(sound).at(anchor).gain(gain).pitch(pitch);
        if handed_over {
            play.fade_in(LOOP_SWAP).start()
        } else {
            play.start()
        }
    });
    *w.require_mut::<Slot>(e) = slot;
}

/// Move each rival's anchor to where it is heard from and ease its Doppler bend.
fn hear(w: &mut World, cars: &[(Entity, Car, Vec3)], ears: Vec3, ears_v: Vec2, dt: f32) {
    let ease = 1.0 - math::exp(-dt / DOPPLER_EASE);
    let anchors: Vec<(Entity, Heard)> = w.query::<&Heard>().iter().map(|(e, h)| (e, *h)).collect();
    for (anchor, mut heard) in anchors {
        let Some((_, car, at)) = cars.iter().find(|(_, c, _)| c.id == heard.car) else {
            continue;
        };
        let offset = *at - ears;
        let bend = doppler(
            Vec2::new(at.x, at.z),
            Vec2::new(car.vx, car.vz),
            Vec2::new(ears.x, ears.z),
            ears_v,
        );
        heard.pan = pan(Vec2::new(offset.x, offset.z));
        heard.doppler += (bend - heard.doppler) * ease;
        // The same distance, turned so the offset across over it is the pan.
        let d = offset.length();
        w.require_mut::<Transform>(anchor).position =
            ears + Vec3::new(heard.pan * d, 0.0, (1.0 - heard.pan * heard.pan).sqrt() * d);
        *w.require_mut::<Heard>(anchor) = heard;
    }
}

use math::smoothstep;

/// A number in 0..1 hashed from when and where a sound is: the world's random
/// stream stays the gameplay's alone.
fn unit(now: u64, salt: u32) -> f32 {
    let mut h =
        now.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (salt as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    h ^= h >> 29;
    (h % 10_000) as f32 / 10_000.0
}

/// A pitch (or gain) near 1, varied as `unit` varies it.
fn jitter(now: u64, salt: u32, spread: f32) -> f32 {
    1.0 + spread * (unit(now, salt) * 2.0 - 1.0)
}

/// Keep `e`'s throttle in mind, and note a lift off it from high revs.
fn listen_for_lift(e: &mut Engine, car: &Car, now: u64, dt: f32) {
    let gas = if car.brake { 0.0 } else { car.gas };
    if gas > 0.0 {
        e.open += dt;
        return;
    }
    if e.open >= POP_HELD && e.rpm > POP_FROM {
        e.lift = Some(now);
    }
    e.open = 0.0;
}

/// The ticks at which a lift at `lift` pops `car`'s exhaust: the first as the
/// unburnt fuel reaches the pipe, the rest spaced out, all within POP_WINDOW
/// (three at the widest spacing end at 0.92 of it). A rival pops once, so a
/// pack braking for a corner does not crackle on and on.
fn pop_ticks(layout: Layout, car: u32, lift: u64, hz: f32) -> impl Iterator<Item = u64> {
    let (fewest, most, _) = layout.overrun();
    let salt = 1000 + 8 * car;
    let count = if car == crate::PLAYER {
        fewest + (unit(lift, salt) * (most - fewest + 1) as f32) as u32
    } else {
        1
    };
    let mut at = 0.08 + 0.17 * unit(lift, salt + 1);
    (0..count).map(move |k| {
        let tick = lift + (at * POP_WINDOW * hz).round() as u64;
        at += 0.13 + 0.2 * unit(lift, salt + 2 + k);
        tick
    })
}

/// Play `car`'s overrun pop if one is due `now`, unless enough already sound:
/// the take rendered nearest its rpm, bent to it, under the engine's own level.
fn pop(w: &World, at: Entity, car: u32, doppler: f32, engine: &Engine, now: u64) {
    let layout = Layout::of(car);
    let Some(lift) = engine.lift else { return };
    let Some(nth) = pop_ticks(layout, car, lift, w.hz() as f32).position(|t| t == now) else {
        return;
    };
    let popping = w
        .resource::<audio::Voices>()
        .voices
        .iter()
        .filter(|v| v.sound.starts_with("pop-"))
        .count();
    if popping >= POPS_AT_ONCE {
        return;
    }
    let distance = |r: u32| math::ln(engine.rpm / r as f32).abs();
    let rendered = POP_RPMS
        .into_iter()
        .min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
        .unwrap();
    let salt = 2000 + 4 * car;
    let take = (unit(now, salt) * POP_TAKES as f32) as u32;
    let (_, _, share) = layout.overrun();
    w.play(&format!("pop-{}-{rendered}-{take}", layout.name()))
        .at(at)
        .gain(share * jitter(now, salt + 1, 0.1) * engine_level(car, engine.load))
        .pitch((engine.rpm / rendered as f32).clamp(0.8, 1.25) * doppler)
        .pan(pop_pan(car, lift, nth, now))
        .start();
}

/// The player's own pops sit on the listener, dead centre: step each one a
/// little to a side, alternating through a lift's pops like twin tailpipes.
/// Exact applies pan as a balance after spatial gains, so on a rival it would
/// pull the pop off the car it comes from: theirs stay at 0.
fn pop_pan(car: u32, lift: u64, nth: usize, now: u64) -> f32 {
    if car != crate::PLAYER {
        return 0.0;
    }
    let salt = 3000 + 4 * car;
    let first = if unit(lift, salt) < 0.5 { -1.0 } else { 1.0 };
    let side = if nth % 2 == 0 { first } else { -first };
    side * (0.15 + 0.2 * unit(now, salt + 1))
}

/// Corners have kerbs (as `scenery::road` lays them); the dirt track has none.
fn kerbed(t: &Track, i: usize) -> bool {
    let n = t.len();
    !t.dirt && (0..9).any(|k| t.bend[(i + n + k - 4) % n].abs() > 1.0 / 24.0)
}

/// How loud the tyres squeal on tarmac, sliding `sideways` (m/s) or with the
/// wheels `locked` under braking.
pub fn squeal(sideways: f32, locked: bool) -> f32 {
    let slide = smoothstep(SQUEAL_FROM, SQUEAL_FULL, sideways);
    SQUEAL_LEVEL * if locked { slide.max(0.6) } else { slide }
}

/// After gameplay: engines and their pops, the player's tyres, knocks and
/// stings, then `audio::step`.
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
    if let Some((_, player, at)) = cars.iter().find(|(_, c, _)| c.id == crate::PLAYER) {
        w.require_mut::<Transform>("ears").position = *at;
        hear(w, &cars, *at, Vec2::new(player.vx, player.vz), dt);
    }
    let anchors: Vec<(Entity, Heard)> = w.query::<&Heard>().iter().map(|(e, h)| (e, *h)).collect();
    let anchor = |car: u32| anchors.iter().find(|(_, h)| h.car == car).copied();
    let mut engines = [Engine::default(); crate::DRIVERS.len()];
    for (e, car, _) in &cars {
        let mut engine = *w.require::<Engine>(*e);
        let i = car.idx as usize;
        let (s, c) = math::sin_cos(car.heading);
        let climb = t.grade[i] * t.tangent(i).dot(Vec2::new(c, s));
        listen_for_lift(&mut engine, car, now, dt);
        rev(&mut engine, car, climb, racing, dt);
        // The player's own pops play at its car, unbent.
        let (at, doppler) = anchor(car.id).map_or((*e, 1.0), |(a, h)| (a, h.doppler));
        pop(w, at, car.id, doppler, &engine, now);
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
        }
        engine.crossings = car.crossings;
        engine.finished = car.finished;
        *w.require_mut::<Engine>(*e) = engine;
        engines[car.id as usize] = engine;
    }
    // The engine slots.
    let slots: Vec<(Entity, Slot)> = w.query::<&Slot>().iter().map(|(e, s)| (e, *s)).collect();
    for (e, slot) in slots {
        let (rpm, gain) = slot_target(&slot, &engines[slot.car as usize], level(slot.car));
        let sound = engine_name(Layout::of(slot.car), slot.on, rpm);
        if slot.car == crate::PLAYER {
            set_source(w, e, &sound, gain);
        } else {
            let (at, heard) = anchor(slot.car).expect("every rival has an anchor");
            set_voice(w, e, at, &sound, gain, heard.doppler);
        }
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
                SQUEAL => {
                    if road && !t.dirt {
                        squeal(sideways, player.locked)
                    } else {
                        0.0
                    }
                }
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
