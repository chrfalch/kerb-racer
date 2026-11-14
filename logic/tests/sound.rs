use exact_game::audio::{AudioSource, Voices};
use exact_game::Sim;
use kerb_racers_logic::car::Car;
use kerb_racers_logic::sound::{self, Engine, Slot, ENGINE_RPMS};
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

/// At the top of its recorded range the V8 sounds like a dragster.
const HIGHEST_RPM: f32 = 4500.0;

/// The rpm of the engine loop a source plays.
fn loop_rpm(s: &AudioSource) -> Option<u32> {
    s.sound.strip_prefix("engine-")?.rsplit('-').next()?.parse().ok()
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
        for (_, s) in w.query::<&AudioSource>().iter() {
            if s.playing {
                heard = heard.max(loop_rpm(s).unwrap_or(0));
            }
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
