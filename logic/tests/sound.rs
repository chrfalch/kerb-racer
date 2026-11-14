use exact_game::audio::{AudioSource, Voices};
use exact_game::{Sim, Vec2, World};
use kerb_racers_logic::car::Car;
use kerb_racers_logic::sound::{self, Engine, Heard, Slot, ENGINE_RPMS};
use kerb_racers_logic::track::Track;
use kerb_racers_logic::{KerbRacers, Options};

fn race(track: u32) -> Sim<KerbRacers> {
    let mut sim = Sim::<KerbRacers>::new(Options {
        track,
        ..Options::default()
    })
    .unwrap();
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/");
    if let Err(e) = sim.load_assets(|name| std::fs::read(format!("{assets}{name}"))) {
        let unexpected =
            e.to_string().contains("\"Pending\"") || e.to_string().contains("\"Failed\"");
        assert!(!unexpected, "{e}");
    }
    sim
}

fn played(sim: &Sim<KerbRacers>, name: &str) -> usize {
    let line = format!("sfx {name} ");
    sim.world()
        .journal()
        .iter()
        .filter(|e| e.line.contains(&line))
        .count()
}

/// The journal's sound lines (each carries its time, so they are unique).
fn sfx(sim: &Sim<KerbRacers>) -> Vec<String> {
    sim.world()
        .journal()
        .iter()
        .filter(|e| e.line.contains("sfx "))
        .map(|e| e.line.clone())
        .collect()
}

/// Loops audible right now: sources playing, plus looping voices (the music).
fn audible(sim: &Sim<KerbRacers>) -> usize {
    let w = sim.world();
    w.count::<AudioSource>(|s| s.playing) as usize
        + w.resource::<Voices>()
            .voices
            .iter()
            .filter(|v| v.looping())
            .count()
}

#[test]
fn every_rpm_is_two_neighbouring_loops_at_equal_power() {
    for rpm in (700..7000).step_by(37) {
        let (i, a, b) = sound::crossfade(rpm as f32);
        assert!(i + 1 < ENGINE_RPMS.len());
        assert!(((a * a + b * b) - 1.0).abs() < 1e-4, "{rpm}: {a} {b}");
        let r = rpm as f32;
        if r > ENGINE_RPMS[0] as f32 && r < ENGINE_RPMS[ENGINE_RPMS.len() - 1] as f32 {
            assert!(ENGINE_RPMS[i] as f32 <= r && r <= ENGINE_RPMS[i + 1] as f32);
        }
    }
    // Exactly on a loop's rpm only that loop plays.
    let (i, a, b) = sound::crossfade(3160.0);
    assert_eq!((ENGINE_RPMS[i], a, b.abs() < 1e-6), (3160, 1.0, true));
}

#[test]
fn the_engine_revs_on_the_grid_and_climbs_the_gears() {
    let mut game = race(1);
    game.run(500.0);
    let idle = *game.world().require::<Engine>("player");
    assert!(
        (idle.rpm - sound::IDLE).abs() < 1.0,
        "idles at {}",
        idle.rpm
    );
    // A blip on the grid.
    game.key_down("KeyW");
    game.run(400.0);
    let blip = *game.world().require::<Engine>("player");
    assert!(blip.rpm > 3000.0 && blip.gear == 0, "{blip:?}");
    // Flat out after the lights: up through the gears, never past the limiter.
    let mut top = 0;
    let mut most = 0.0f32;
    for _ in 0..(120 * 12) {
        game.run(1000.0 / 120.0);
        let e = *game.world().require::<Engine>("player");
        top = top.max(e.gear);
        most = most.max(e.rpm);
    }
    println!("top gear {top}, most {most:.0} rpm");
    assert!(top >= 2, "shifts up");
    assert!(most <= sound::REDLINE + 1.0);
}

#[test]
fn the_engine_labours_uphill_and_eases_downhill() {
    let pull = |gas: f32, climb: f32| {
        let car = Car {
            gas,
            roll: 20.0,
            ..Car::default()
        };
        let mut e = Engine::default();
        for _ in 0..120 {
            sound::rev(&mut e, &car, climb, true, 1.0 / 120.0);
        }
        e.load
    };
    // Flat out as well as part throttle: most of a lap is flat out.
    for gas in [1.0, 0.5] {
        let (up, level, down) = (pull(gas, 0.15), pull(gas, 0.0), pull(gas, -0.15));
        println!("gas {gas}: load uphill {up:.2}, level {level:.2}, downhill {down:.2}");
        assert!(up > level + 0.1 && level > down + 0.1, "gas {gas}: {up} {level} {down}");
    }
    let (steep, cliff) = (pull(1.0, 0.6), pull(0.0, -0.6));
    assert!(steep < 1.5 && cliff >= 0.0, "clamped: {steep} {cliff}");
}

#[test]
fn the_race_is_heard_within_the_voice_budget() {
    let mut game = race(0);
    game.run(200.0);
    assert_eq!(played(&game, "music"), 1, "the music starts with the grid");
    game.run(3200.0);
    assert_eq!(played(&game, "sting-count"), 3, "three, two, one");
    assert_eq!(played(&game, "sting-go"), 1);
    // A careful human: gas, and brake for the planned speed, to the flag.
    let mut loudest = 0;
    let mut heard = std::collections::BTreeSet::new();
    for _ in 0..(120 * 150) {
        let (target, v, done) = {
            let w = game.world();
            let t = w.resource::<Track>();
            let car = w.require::<Car>("player");
            let i = t.wrap(car.idx as i64 + 6);
            (
                t.speed[i] * 1.04,
                (car.vx * car.vx + car.vz * car.vz).sqrt(),
                car.finished,
            )
        };
        if done {
            break;
        }
        if v > target + 0.5 {
            game.key_up("KeyW");
        } else {
            game.key_down("KeyW");
        }
        game.run(1000.0 / 120.0);
        loudest = loudest.max(audible(&game));
        // The journal keeps its last 4096 lines: collect as the race goes.
        heard.extend(sfx(&game));
    }
    game.run(500.0);
    heard.extend(sfx(&game));
    let count = |name: &str| {
        heard
            .iter()
            .filter(|l| l.contains(&format!("sfx {name} ")))
            .count()
    };
    println!(
        "most loops at once {loudest}; laps {}, hits {}, kerbs {}",
        count("sting-lap"),
        (0..6).map(|k| count(&format!("hit-{k}"))).sum::<usize>(),
        (0..4).map(|k| count(&format!("kerb-{k}"))).sum::<usize>(),
    );
    // Six engines at two loops each (the player four), the tyres and the music:
    // well inside Apple's 32 voices, with room for the one-shots.
    assert!(loudest <= 20, "{loudest} loops at once");
    assert_eq!(
        count("sting-lap"),
        2,
        "laps one and two; the third is the flag"
    );
    assert_eq!(count("sting-finish"), 1);
}

#[test]
fn the_tyres_squeal_early_in_a_slide_and_loudly_in_a_full_one() {
    assert_eq!(sound::squeal(0.5, false), 0.0, "rolling straight is silent");
    let moderate = sound::squeal(2.5, false);
    assert!(moderate > 0.05, "a moderate slide is heard: {moderate}");
    let full = sound::squeal(9.0, false);
    assert!(full >= 0.65, "a full slide squeals louder than the old 0.45: {full}");
    let locked = sound::squeal(0.0, true);
    assert!(locked > 0.3, "locked wheels squeal: {locked}");
}

#[test]
fn a_lift_down_a_straight_is_silent() {
    let mut game = race(0);
    game.run(3500.0);
    game.key_down("KeyW");
    let straight_ahead = |game: &Sim<KerbRacers>| {
        let w = game.world();
        let t = w.resource::<Track>();
        let car = w.require::<Car>("player");
        let speed = (car.vx * car.vx + car.vz * car.vz).sqrt();
        speed > 15.0 && !car.sliding && (0..15).all(|k| t.bend[t.wrap(car.idx as i64 + k)].abs() < 0.02)
    };
    for _ in 0..(120 * 20) {
        if straight_ahead(&game) {
            break;
        }
        game.run(1000.0 / 120.0);
    }
    assert!(straight_ahead(&game), "no straight to lift on");
    game.key_up("KeyW");
    for tick in 0..60 {
        game.run(1000.0 / 120.0);
        let w = game.world();
        let squealing = w.count::<AudioSource>(|s| s.sound == "squeal" && s.playing);
        assert_eq!(squealing, 0, "tick {tick} of the lift: the tyres squeal");
    }
}

#[test]
fn volume_sets_the_master() {
    let mut game = race(2);
    game.run(100.0);
    assert_eq!(
        game.world().resource::<exact_game::audio::Audio>().master,
        1.0
    );
    let mut muted = Sim::<KerbRacers>::new(Options {
        volume: 0.0,
        ..Options::default()
    })
    .unwrap();
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/");
    let _ = muted.load_assets(|name| std::fs::read(format!("{assets}{name}")));
    muted.run(100.0);
    assert_eq!(
        muted.world().resource::<exact_game::audio::Audio>().master,
        0.0
    );
}

/// At the top of the range the engines sound like dragsters.
const HIGHEST_RPM: f32 = 4500.0;

/// The rpm of an engine loop.
fn loop_rpm(sound: &str) -> Option<u32> {
    sound.strip_prefix("engine-")?.rsplit('-').next()?.parse().ok()
}

/// Every engine loop playing now: (car, sound, gain, pitch). The player's are
/// sources; a rival's are voices, which can bend in pitch.
fn engine_loops(w: &World) -> Vec<(u32, String, f32, f32)> {
    w.query::<&Slot>()
        .iter()
        .filter_map(|(e, slot)| {
            if let Some(s) = w.get::<AudioSource>(e) {
                return s.playing.then(|| (slot.car, s.sound.clone(), s.gain, 1.0));
            }
            let id = slot.voice?;
            let voices = w.resource::<Voices>();
            let v = voices.voices.iter().find(|v| v.id == id)?;
            Some((slot.car, v.sound.clone(), v.gain, v.pitch))
        })
        .collect()
}

/// The race from the grid to the flag, the player flat out where the planned
/// speed allows; `each` sees the race after every tick.
fn race_flat_out(track: u32, mut each: impl FnMut(&Sim<KerbRacers>)) {
    let mut game = race(track);
    for _ in 0..(120 * 150) {
        let (target, v, done) = {
            let w = game.world();
            let t = w.resource::<Track>();
            let car = w.require::<Car>("player");
            let i = t.wrap(car.idx as i64 + 6);
            (
                t.speed[i] * 1.04,
                (car.vx * car.vx + car.vz * car.vz).sqrt(),
                car.finished,
            )
        };
        if done {
            break;
        }
        if v > target + 0.5 {
            game.key_up("KeyW");
        } else {
            game.key_down("KeyW");
        }
        game.run(1000.0 / 120.0);
        each(&game);
    }
}

#[test]
fn no_engine_reaches_the_high_revs() {
    let (mut rpm, mut heard) = (0.0f32, 0u32);
    race_flat_out(0, |game| {
        let w = game.world();
        for (_, e) in w.query::<&Engine>().iter() {
            rpm = rpm.max(e.rpm);
        }
        for (_, sound, _, _) in engine_loops(w) {
            heard = heard.max(loop_rpm(&sound).unwrap_or(0));
        }
    });
    println!("highest rpm {rpm:.0}, highest loop heard {heard}");
    assert!(rpm <= HIGHEST_RPM, "an engine reached {rpm:.0} rpm");
    assert!(heard as f32 <= HIGHEST_RPM, "the {heard} rpm loop played");
}

#[test]
fn the_grid_blip_stays_out_of_the_high_revs() {
    let car = Car {
        gas: 1.0,
        ..Car::default()
    };
    let mut e = Engine::default();
    for _ in 0..240 {
        sound::rev(&mut e, &car, 0.0, false, 1.0 / 120.0);
    }
    assert!(e.rpm > 3000.0 && e.rpm <= HIGHEST_RPM, "blips to {:.0}", e.rpm);
}

#[test]
fn the_revs_sweep_smoothly_up_the_gears_and_stay_under_the_cap() {
    let ramp = 120 * 14;
    let mut e = Engine::default();
    let mut last = e;
    let mut rising = false;
    // Flat out from rest to past top speed (down a hill), then coasting there.
    for tick in 0..(120 * 16) {
        let car = Car {
            gas: if tick < ramp { 1.0 } else { 0.0 },
            roll: 38.0 * (tick as f32 / ramp as f32).min(1.0),
            ..Car::default()
        };
        sound::rev(&mut e, &car, 0.0, true, 1.0 / 120.0);
        assert!(e.rpm <= HIGHEST_RPM, "tick {tick}: {:.0} rpm", e.rpm);
        // A new gear settles down first; from then on the revs only climb.
        if e.gear != last.gear {
            rising = false;
        } else if e.rpm > last.rpm {
            rising = true;
        } else {
            assert!(!rising || e.rpm >= last.rpm - 1e-3, "tick {tick}: falls within gear {}", e.gear);
        }
        last = e;
    }
    assert_eq!(e.gear as usize, sound::GEARS.len() - 1, "reaches top gear");
}

#[test]
fn most_of_a_lap_is_in_the_mid_revs() {
    let bands = [3000.0, 3500.0, 4000.0, 4500.0, 5000.0];
    let mut above = [[0u32; 5]; 6];
    let mut most = [0.0f32; 6];
    let (mut ticks, mut started) = (0u32, false);
    race_flat_out(0, |game| {
        let w = game.world();
        let player = w.require::<Car>("player");
        started |= player.vx.hypot(player.vz) > 0.5;
        if !started {
            return;
        }
        ticks += 1;
        for (_, (car, e)) in w.query::<(&Car, &Engine)>().iter() {
            most[car.id as usize] = most[car.id as usize].max(e.rpm);
            for (k, &rpm) in bands.iter().enumerate() {
                above[car.id as usize][k] += (e.rpm > rpm) as u32;
            }
        }
    });
    let share = |id: usize, k: usize| above[id][k] as f32 / ticks as f32;
    for id in 0..above.len() {
        let who = if id as u32 == kerb_racers_logic::PLAYER { "player" } else { "rival" };
        let shares: Vec<String> = (0..bands.len())
            .map(|k| format!("{} {:.0}%", bands[k], 100.0 * share(id, k)))
            .collect();
        println!("car {id} ({who}) above: {}; most {:.0} rpm", shares.join(", "), most[id]);
    }
    let player = kerb_racers_logic::PLAYER as usize;
    assert!(share(player, 1) <= 0.05, "the player spends {:.0}% above 3500 rpm", 100.0 * share(player, 1));
}

#[test]
fn the_gears_never_hunt_at_a_steady_speed() {
    let dt = 1.0 / 120.0;
    let mut v = 0.5;
    while v < 38.0 {
        let mut e = Engine::default();
        for tick in 0..240 {
            let car = Car {
                gas: 1.0,
                roll: v * (tick as f32 / 120.0).min(1.0),
                ..Car::default()
            };
            sound::rev(&mut e, &car, 0.0, true, dt);
        }
        // Held at this speed, the driver feathering the throttle.
        let mut shifts = Vec::new();
        for tick in 0..240 {
            let car = Car {
                gas: if tick / 24 % 2 == 0 { 1.0 } else { 0.0 },
                roll: v,
                ..Car::default()
            };
            let gear = e.gear;
            sound::rev(&mut e, &car, 0.0, true, dt);
            if e.gear != gear {
                shifts.push((tick, e.gear > gear));
            }
        }
        let hunts = shifts.windows(2).any(|p| p[0].1 != p[1].1 && p[1].0 - p[0].0 <= 60);
        assert!(!hunts, "at {v} m/s the gears hunt: {shifts:?}");
        v += 0.25;
    }
}

/// One of the player's sets, pulling (`on`) or coasting, as one amplitude: its
/// two loops crossfade at equal power, so it is the same at every rpm.
fn engine_set(game: &Sim<KerbRacers>, on: bool) -> f32 {
    let w = game.world();
    w.query::<(&Slot, &AudioSource)>()
        .iter()
        .filter(|(_, (slot, s))| slot.car == kerb_racers_logic::PLAYER && slot.on == on && s.playing)
        .map(|(_, (_, s))| s.gain * s.gain)
        .sum::<f32>()
        .sqrt()
}

#[test]
fn the_pulling_engine_is_lighter_and_a_climb_never_lifts_it() {
    let (mut flat, mut uphill, mut labour) = (0.0f32, 0.0f32, 0.0f32);
    race_flat_out(0, |game| {
        let w = game.world();
        let e = *w.require::<Engine>("player");
        let car = w.require::<Car>("player");
        if car.gas < 1.0 || car.brake || e.cut > 0.0 {
            return;
        }
        labour = labour.max(e.load);
        if e.load > 1.02 {
            uphill = uphill.max(engine_set(game, true));
        } else if e.load > 0.98 {
            flat = flat.max(engine_set(game, true));
        }
    });
    println!("pulling set flat out {flat:.3}, uphill {uphill:.3}, most load {labour:.2}");
    assert!(labour > 1.05, "the lap has a climb flat out: load {labour}");
    // The old level: 0.55 flat out, and up to 0.72 up a hill.
    let lighter = flat / 0.55;
    assert!((0.70..=0.80).contains(&lighter), "flat out at {lighter:.2} of the old level");
    assert!(uphill <= flat + 1.0 / 256.0, "a climb lifts the pulling set: {uphill} over {flat}");
}

#[test]
fn letting_off_is_no_louder_than_flat_out() {
    let (mut pull, mut coast) = (0.0f32, 0.0f32);
    race_flat_out(0, |game| {
        let e = *game.world().require::<Engine>("player");
        if (0.98..=1.02).contains(&e.load) {
            pull = pull.max(engine_set(game, true));
        } else if e.load < 0.02 {
            coast = coast.max(engine_set(game, false));
        }
    });
    println!("coasting set off the gas {coast:.3}, pulling set flat out {pull:.3}");
    assert!(coast > 0.0 && pull > 0.0, "the lap both pulls and coasts");
    assert!(coast <= pull + 1.0 / 256.0, "coasting {coast} over pulling {pull}");
}

/// Each car's engine, by its body (`art/src/cars.py`): the flat-six sports
/// cars, the mid-engine V6 rally cars and the four-cylinder rally saloons.
const BODIES: [&str; 6] = ["flat6", "v6", "i4", "i4", "flat6", "v6"];

/// Into the race, the player flat out until its revs pass `rpm` on full throttle.
fn flat_out_past(rpm: f32) -> Sim<KerbRacers> {
    let mut game = race(0);
    game.run(3500.0);
    game.key_down("KeyW");
    for _ in 0..(120 * 10) {
        let w = game.world();
        if w.require::<Engine>("player").rpm > rpm && w.require::<Car>("player").gas >= 1.0 {
            return game;
        }
        game.run(1000.0 / 120.0);
    }
    panic!("the player never passed {rpm} rpm flat out");
}

/// Every overrun pop the journal holds: (tick, line).
fn pops(sim: &Sim<KerbRacers>) -> Vec<(u64, String)> {
    sim.world()
        .journal()
        .iter()
        .filter(|e| e.line.contains("sfx pop-"))
        .map(|e| (e.tick, e.line.clone()))
        .collect()
}

fn the_players(pops: &[(u64, String)]) -> Vec<(u64, String)> {
    pops.iter().filter(|(_, l)| l.contains(" at player ")).cloned().collect()
}

#[test]
fn every_car_sounds_like_its_body() {
    let game = flat_out_past(2500.0);
    let w = game.world();
    let mut checked = [0; 6];
    for (car, sound, _, _) in engine_loops(w) {
        let body = BODIES[car as usize];
        assert!(sound.starts_with(&format!("engine-{body}-")), "car {car} plays {sound}, not its {body}");
        checked[car as usize] += 1;
    }
    assert!(checked.iter().all(|&n| n >= 1), "every car's engine plays: {checked:?}");
}

#[test]
fn every_engine_bank_and_pop_is_an_asset_within_the_memory_cap() {
    for body in ["flat6", "v6", "i4"] {
        for load in ["on", "off"] {
            for rpm in ENGINE_RPMS {
                let name = format!("engine-{body}-{load}-{rpm}.sound");
                assert!(sound::ASSETS.contains(&name.as_str()), "{name} is not an asset");
            }
        }
        for rpm in POP_RPMS {
            for k in 0..2 {
                let name = format!("pop-{body}-{rpm}-{k}.sound");
                assert!(sound::ASSETS.contains(&name.as_str()), "{name} is not an asset");
            }
        }
    }
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/");
    let bytes: u64 = sound::ASSETS
        .iter()
        .map(|name| std::fs::metadata(format!("{assets}{name}")).unwrap_or_else(|e| panic!("{name}: {e}")).len())
        .sum();
    println!("{} sounds, {:.1} MiB", sound::ASSETS.len(), bytes as f64 / (1 << 20) as f64);
    // Exact keeps at most 32 MiB of sound resident per world: leave room under it.
    assert!(bytes <= 24 << 20, "the sounds take {bytes} bytes");
}

#[test]
fn a_lift_from_high_revs_pops_and_a_steady_throttle_does_not() {
    let mut game = flat_out_past(2800.0);
    assert!(the_players(&pops(&game)).is_empty(), "pops on a steady throttle: {:?}", the_players(&pops(&game)));
    let lift = game.world().tick();
    game.key_up("KeyW");
    game.run(600.0);
    let after: Vec<_> = the_players(&pops(&game)).into_iter().filter(|(t, _)| *t > lift).collect();
    println!("pops after the lift: {after:?}");
    assert!(!after.is_empty(), "a lift from high revs plays no pop");
    assert!(after.len() <= 3, "too many pops for one lift: {after:?}");
    assert!(after.iter().all(|(t, l)| *t <= lift + 72 && l.contains("sfx pop-flat6-")), "{after:?}");
}

#[test]
fn the_pops_are_the_same_every_run() {
    let run = || {
        let mut game = flat_out_past(2800.0);
        game.key_up("KeyW");
        game.run(1000.0);
        pops(&game)
    };
    let first = run();
    assert!(!first.is_empty());
    assert_eq!(first, run());
}

/// The rpms the pops are rendered at, from the lowest lift up to the grid blip.
const POP_RPMS: [u32; 4] = [2580, 2950, 3380, 3870];

/// A rendered sound's samples (16-bit mono WAV) and its rate.
fn wav(name: &str) -> (Vec<f32>, f32) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../art/sounds/");
    let bytes = std::fs::read(format!("{path}{name}.wav")).unwrap_or_else(|e| panic!("{name}: {e}"));
    let rate = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as f32;
    let mut at = 12;
    while &bytes[at..at + 4] != b"data" {
        at += 8 + u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
    }
    let samples = bytes[at + 8..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect();
    (samples, rate)
}

/// A sound's power every 5 Hz from 40 Hz to 2 kHz: (Hz, power).
fn spectrum(x: &[f32], rate: f32) -> Vec<(f64, f64)> {
    (40..=2000)
        .step_by(5)
        .map(|hz| {
            let step = std::f64::consts::TAU * hz as f64 / rate as f64;
            let (sin, cos) = step.sin_cos();
            let (mut re, mut im, mut c, mut s) = (0.0, 0.0, 1.0f64, 0.0f64);
            for &v in x {
                re += v as f64 * c;
                im -= v as f64 * s;
                (c, s) = (c * cos - s * sin, s * cos + c * sin);
            }
            (hz as f64, re * re + im * im)
        })
        .collect()
}

/// Where a sound's note sits: the centre of its power between 40 Hz and 1 kHz.
/// An engine's loudest harmonic hops between the firing rate and its multiples
/// as the pipes favour one, so the centre is the steadier measure.
fn note(spectrum: &[(f64, f64)]) -> f64 {
    let low = spectrum.iter().filter(|(hz, _)| *hz <= 1000.0);
    let total: f64 = low.clone().map(|(_, p)| p).sum();
    low.map(|(hz, p)| hz * p).sum::<f64>() / total
}

/// How alike two spectra are in shape: the correlation of their levels (dB)
/// in third-octave bands from 50 Hz to 2 kHz. A thump and a crack where the
/// engine has neither scores below zero.
fn likeness(a: &[(f64, f64)], b: &[(f64, f64)]) -> f64 {
    let bands = |s: &[(f64, f64)]| -> Vec<f64> {
        (0..16)
            .map(|k| {
                let centre = 50.0 * 2f64.powf(k as f64 / 3.0);
                let (lo, hi) = (centre / 2f64.powf(1.0 / 6.0), centre * 2f64.powf(1.0 / 6.0));
                let power: f64 = s.iter().filter(|(hz, _)| (lo..hi).contains(hz)).map(|(_, p)| p).sum();
                10.0 * (power + 1e-12).log10()
            })
            .collect()
    };
    let (a, b) = (bands(a), bands(b));
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let (ma, mb) = (mean(&a), mean(&b));
    let cov: f64 = a.iter().zip(&b).map(|(x, y)| (x - ma) * (y - mb)).sum();
    let var = |v: &[f64], m: f64| v.iter().map(|x| (x - m) * (x - m)).sum::<f64>();
    cov / (var(&a, ma) * var(&b, mb)).sqrt()
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |m, s| m.max(s.abs()))
}

#[test]
fn every_pop_is_the_exhaust_note_at_its_revs() {
    for body in ["flat6", "v6", "i4"] {
        for rpm in POP_RPMS {
            let (engine, rate) = wav(&format!("engine-{body}-on-{rpm}"));
            // A quarter of a second holds dozens of firings.
            let engine_spectrum = spectrum(&engine[..(rate / 4.0) as usize], rate);
            let engine_note = note(&engine_spectrum);
            for k in 0..2 {
                let name = format!("pop-{body}-{rpm}-{k}");
                let (pop, rate) = wav(&name);
                let pop_spectrum = spectrum(&pop, rate);
                let (pitch, alike) = (note(&pop_spectrum), likeness(&pop_spectrum, &engine_spectrum));
                println!("{name}: note {pitch:.0} Hz against the engine's {engine_note:.0} Hz, likeness {alike:.2}, peak {:.2} against its {:.2}, {:.0} ms", peak(&pop), peak(&engine), pop.len() as f32 / rate * 1000.0);
                assert!((pitch / engine_note - 1.0).abs() <= 0.25, "{name} sits at {pitch:.0} Hz, its engine at {engine_note:.0}");
                assert!(alike >= 0.3, "{name} is not shaped like its engine: {alike:.2}");
                assert!(peak(&pop) <= peak(&engine) * 1.05, "{name} peaks over its engine");
                assert!(pop.len() as f32 <= 0.2 * rate, "{name} rings on");
            }
        }
    }
}

#[test]
fn the_pops_stay_few_and_never_louder_than_their_engine() {
    use exact_game::audio::At;
    let (mut loops, mut most) = (0, 0);
    let mut seen = std::collections::BTreeSet::new();
    // Each pop's gain over its engine's at that moment, by body.
    let mut share: std::collections::BTreeMap<&str, f32> = Default::default();
    race_flat_out(0, |game| {
        let w = game.world();
        loops = loops.max(audible(game));
        let voices = w.resource::<Voices>();
        let popping: Vec<_> = voices.voices.iter().filter(|v| v.sound.starts_with("pop-")).collect();
        most = most.max(popping.len());
        for v in popping {
            if !seen.insert(v.id) {
                continue;
            }
            let At::Entity(e) = v.at else { panic!("{} is not at a car", v.sound) };
            // A rival's pop plays at its anchor, the player's at its car.
            let car = w.get::<Heard>(e).map_or_else(|| w.require::<Car>(e).id, |h| h.car);
            let engine = engine_loops(w)
                .iter()
                .filter(|l| l.0 == car)
                .map(|l| l.2 * l.2)
                .sum::<f32>()
                .sqrt();
            assert!(v.gain <= engine + 1.0 / 256.0, "{} at {} over its engine's {}", v.sound, v.gain, engine);
            if car != kerb_racers_logic::PLAYER {
                assert_eq!(v.pan, 0.0, "a rival's pop is placed by where the car is alone");
            }
            let body = BODIES[car as usize];
            let s = share.entry(body).or_default();
            *s = s.max(v.gain / engine);
        }
    });
    println!("{} pops, most {most} at once, {loops} loops; loudest against the engine {share:?}", seen.len());
    assert!(!seen.is_empty(), "nobody popped in a whole race");
    assert!(most <= 3, "{most} pops at once");
    assert!(loops <= 20, "{loops} loops at once");
    let rally = ["v6", "i4"].iter().filter_map(|b| share.get(b)).fold(0.0f32, |m, s| m.max(*s));
    assert!(rally > share["flat6"], "the rally cars pop no louder: {share:?}");
}

/// The player's pops from `lifts` lifts off the gas from high revs, a list per
/// lift: (tick, pan).
fn players_pops_by_lift(lifts: usize) -> Vec<Vec<(u64, f32)>> {
    use exact_game::audio::At;
    let mut game = flat_out_past(2800.0);
    let player = game.world().named("player").unwrap();
    let mut seen = std::collections::BTreeSet::new();
    let mut by_lift = Vec::new();
    for _ in 0..lifts {
        game.key_up("KeyW");
        let mut pops = Vec::new();
        for _ in 0..84 {
            game.run(1000.0 / 120.0);
            let w = game.world();
            for v in w.resource::<Voices>().voices.iter() {
                if v.sound.starts_with("pop-") && v.at == At::Entity(player) && seen.insert(v.id) {
                    pops.push((w.tick(), v.pan));
                }
            }
        }
        by_lift.push(pops);
        game.key_down("KeyW");
        for _ in 0..(120 * 10) {
            let e = *game.world().require::<Engine>("player");
            if e.rpm > 2800.0 && e.open > 0.5 {
                break;
            }
            game.run(1000.0 / 120.0);
        }
    }
    by_lift
}

#[test]
fn the_players_pops_step_left_and_right() {
    let by_lift = players_pops_by_lift(8);
    println!("the player's pops by lift (tick, pan): {by_lift:?}");
    assert!(by_lift.iter().all(|pops| !pops.is_empty()), "a lift went unpopped: {by_lift:?}");
    assert!(by_lift.iter().any(|pops| pops.len() >= 2), "no lift popped twice: {by_lift:?}");
    for pops in &by_lift {
        for &(_, pan) in pops {
            assert!((0.15..=0.35).contains(&pan.abs()), "pan {pan} out of the slight spread");
        }
        for pair in pops.windows(2) {
            assert!(pair[0].1.signum() != pair[1].1.signum(), "two pops on one side: {pops:?}");
        }
    }
    assert_eq!(by_lift, players_pops_by_lift(8), "the pans differ between runs");
}

#[test]
fn a_rival_beside_the_player_is_heard_on_that_side_and_one_ahead_in_the_middle() {
    // Screen right is +x, screen up is -z (the camera looks down the track from +z).
    let (right, left) = (sound::pan(Vec2::new(8.0, 0.0)), sound::pan(Vec2::new(-8.0, 0.0)));
    let (ahead, behind) = (sound::pan(Vec2::new(0.0, -8.0)), sound::pan(Vec2::new(0.0, 8.0)));
    let diagonal = sound::pan(Vec2::new(4.0, -8.0));
    println!("pan right {right:.2}, left {left:.2}, ahead {ahead:.2}, behind {behind:.2}, 4 right 8 ahead {diagonal:.2}");
    assert!(right >= 0.6 && left <= -0.6 && (right + left).abs() < 1e-6, "{right} {left}");
    assert!(ahead.abs() <= 0.1 && behind.abs() <= 0.1, "{ahead} {behind}");
    assert!(right <= 0.9, "never all in one ear: {right}");
    // Stronger than Exact's own x / distance, which puts that diagonal at 0.45.
    assert!(diagonal >= 0.6, "{diagonal}");
}

#[test]
fn the_pan_sweeps_smoothly_as_a_rival_passes() {
    // Passing left to right at 10 m/s, close behind, then right through the player.
    for z in [3.0, 0.0] {
        let mut last = sound::pan(Vec2::new(-15.0, z));
        let mut x = -15.0;
        while x < 15.0 {
            x += 10.0 / 120.0;
            let pan = sound::pan(Vec2::new(x, z));
            assert!((pan - last).abs() <= 0.05, "at {x:.2}, {z}: {last:.3} to {pan:.3}");
            last = pan;
        }
    }
}

#[test]
fn a_closing_rival_sounds_higher_and_a_receding_one_lower() {
    let at = Vec2::new(0.0, -20.0);
    let closing = sound::doppler(at, Vec2::new(0.0, -20.0), Vec2::ZERO, Vec2::new(0.0, -30.0));
    let receding = sound::doppler(at, Vec2::new(0.0, -30.0), Vec2::ZERO, Vec2::new(0.0, -20.0));
    let alongside = sound::doppler(at, Vec2::new(0.0, -25.0), Vec2::ZERO, Vec2::new(0.0, -25.0));
    let head_on = sound::doppler(at, Vec2::new(0.0, 60.0), Vec2::ZERO, Vec2::new(0.0, -60.0));
    println!("closing {closing:.3}, receding {receding:.3}, alongside {alongside:.3}, head on {head_on:.3}");
    assert!(closing > 1.0 && receding < 1.0, "{closing} {receding}");
    assert!((alongside - 1.0).abs() < 1e-6, "{alongside}");
    assert!((0.92..=1.08).contains(&head_on), "clamped: {head_on}");
}

#[test]
fn rivals_are_heard_where_they_are_and_bend_as_they_pass() {
    use exact_game::Transform;
    let (mut steepest, mut lowest, mut highest) = (0.0f32, 1.0f32, 1.0f32);
    let mut last = [None::<f32>; 6];
    race_flat_out(0, |game| {
        let w = game.world();
        if w.tick() == 0 {
            return;
        }
        let ears = w.require::<Transform>("ears").position;
        let mut cars = [exact_game::Vec3::ZERO; 6];
        for (_, (pose, car)) in w.query::<(&Transform, &Car)>().iter() {
            cars[car.id as usize] = pose.position;
        }
        let mut bends = [1.0f32; 6];
        for (e, heard) in w.query::<&Heard>().iter() {
            let car = heard.car as usize;
            bends[car] = heard.doppler;
            // The player has no anchor: its loops are sources, at pitch 1 below.
            assert_ne!(heard.car, kerb_racers_logic::PLAYER, "the player is heard where it is, unbent");
            // Exact pans by the anchor's offset across over its distance: that is the pan.
            let at = w.require::<Transform>(e).position - ears;
            let real = cars[car] - ears;
            if real.length() > 0.5 {
                assert!((at.x / at.length() - heard.pan).abs() < 1e-3, "car {car}: anchor pans {} not {}", at.x / at.length(), heard.pan);
                assert!((at.length() - real.length()).abs() < 1e-2, "car {car}: heard {} m off, is {} m", at.length(), real.length());
            }
            if let Some(before) = last[car] {
                steepest = steepest.max((heard.pan - before).abs());
            }
            last[car] = Some(heard.pan);
            lowest = lowest.min(heard.doppler);
            highest = highest.max(heard.doppler);
        }
        for (car, _, _, pitch) in engine_loops(w) {
            let bend = bends[car as usize];
            assert!((pitch - bend).abs() < 1e-3, "car {car}'s engine at pitch {pitch}, heard bent {bend}");
        }
    });
    println!("steepest pan step {steepest:.3}, doppler {lowest:.3}..{highest:.3}");
    assert!(steepest <= 0.05, "the pan jumps {steepest} in a tick");
    assert!(lowest < 0.995 && highest > 1.005, "nobody passed: {lowest} {highest}");
    assert!(lowest >= 0.92 && highest <= 1.08, "{lowest} {highest}");
}

#[test]
fn a_rival_changing_loop_while_heard_crossfades_instead_of_cutting() {
    let mut game = flat_out_past(2500.0);
    let rival = 1;
    let audible = |game: &Sim<KerbRacers>| -> Vec<(u64, String)> {
        let w = game.world();
        let voices = w.resource::<Voices>();
        w.query::<&Slot>()
            .iter()
            .filter(|(_, s)| s.car == rival)
            .filter_map(|(_, s)| voices.voices.iter().find(|v| Some(v.id) == s.voice && v.gain > 0.05))
            .map(|v| (v.id, v.sound.clone()))
            .collect()
    };
    let before = audible(&game);
    assert!(!before.is_empty(), "rival {rival}'s engine is silent");
    let held: Vec<u64> = game.world().query::<&Slot>().iter().filter(|(_, s)| s.car == rival).filter_map(|(_, s)| s.voice).collect();
    // A gear's worth of revs in one tick, as an upshift or downshift lands: every
    // slot moves several loops while it is still loud.
    {
        let w = game.world_mut();
        let car = w.query::<&Car>().iter().find(|(_, c)| c.id == rival).unwrap().0;
        w.require_mut::<Engine>(car).rpm *= 1.35;
    }
    game.run(1000.0 / 120.0);
    {
        let w = game.world();
        let voices = &w.resource::<Voices>().voices;
        for (id, sound) in &before {
            let old = voices.iter().find(|v| v.id == *id).unwrap_or_else(|| panic!("{sound} was cut off"));
            let fade = old.fade.unwrap_or_else(|| panic!("{sound} plays on at full gain"));
            assert_eq!(fade.end, 0.0, "{sound} does not fade out");
            let ms = (fade.to - fade.from) as f32 * 1000.0 / 120.0;
            assert!((15.0..=60.0).contains(&ms), "{sound} fades over {ms} ms");
        }
    }
    // Each slot hands over to its new loop once the old one has faded, never
    // holding two voices at once.
    let rivals_loops = |game: &Sim<KerbRacers>| {
        let w = game.world();
        let ids: Vec<_> = w.query::<&Slot>().iter().filter(|(_, s)| s.car == rival).filter_map(|(_, s)| s.voice).collect();
        let voices = w.resource::<Voices>();
        voices.voices.iter().filter(|v| ids.contains(&v.id)).cloned().collect::<Vec<_>>()
    };
    // The revs keep moving after the jump, so a slot may hand over more than once:
    // every loop it takes up fades in.
    let mut taken_up = std::collections::BTreeMap::new();
    for _ in 0..8 {
        game.run(1000.0 / 120.0);
        let loops = rivals_loops(&game);
        assert!(loops.len() <= 2, "a slot holds two voices");
        for v in loops.into_iter().filter(|v| !held.contains(&v.id)) {
            taken_up.entry(v.id).or_insert((v.sound.clone(), v.fade));
        }
    }
    assert!(!taken_up.is_empty(), "no new loop after the jump");
    for (sound, fade) in taken_up.values() {
        let fade = fade.unwrap_or_else(|| panic!("{sound} starts at full gain"));
        assert_eq!((fade.start, fade.end), (0.0, 1.0), "{sound} does not fade in");
    }
    let w = game.world();
    let voices = &w.resource::<Voices>().voices;
    assert!(before.iter().all(|(id, _)| voices.iter().all(|v| v.id != *id)), "an old loop plays on");
}
