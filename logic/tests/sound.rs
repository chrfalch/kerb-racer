use exact_game::audio::{AudioSource, Voices};
use exact_game::Sim;
use kerb_racers_logic::car::Car;
use kerb_racers_logic::sound::{self, Engine, ENGINE_RPMS};
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
            game.key_down("KeyS");
        } else {
            game.key_up("KeyS");
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
