use exact_game::Sim;
use kerb_racers_logic::car::{self, Car, GRASS};
use kerb_racers_logic::track::{self, Track, HALF};
use kerb_racers_logic::{radar, standings, KerbRacers, Options, Race, PLAYER, RADAR_REACH};

fn race(track: u32) -> Sim<KerbRacers> {
    let mut sim = Sim::<KerbRacers>::new(Options {
        track,
        ..Options::default()
    })
    .unwrap();
    // The baked models and textures, as a host delivers them. The road and
    // ground are generated in setup and come back as requests with no file:
    // they are already loaded, so their read failures are expected.
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/");
    if let Err(e) = sim.load_assets(|name| std::fs::read(format!("{assets}{name}"))) {
        let unexpected =
            e.to_string().contains("\"Pending\"") || e.to_string().contains("\"Failed\"");
        assert!(!unexpected, "{e}");
    }
    sim
}

#[test]
fn tracks_are_short_and_separated() {
    for id in 0..4 {
        let t = track::build(id);
        let n = t.len();
        assert!(t.length() > 240. && t.length() < 380., "{} m", t.length());
        let fastest = t.speed.iter().cloned().fold(0., f32::max);
        let slowest = t.speed.iter().cloned().fold(f32::MAX, f32::min);
        println!(
            "{}: {:.0} m, {} samples, speeds {slowest:.1}..{fastest:.1}, line {:.2}..{:.2}",
            track::layout(id).name,
            t.length(),
            n,
            t.line.iter().cloned().fold(f32::MAX, f32::min),
            t.line.iter().cloned().fold(f32::MIN, f32::max)
        );
        for i in 0..n {
            if t.speed[i] < 10.0
                && t.speed[i] <= t.speed[t.wrap(i as i64 - 1)]
                && t.speed[i] <= t.speed[(i + 1) % n]
            {
                println!(
                    "  slow {:.1} m/s at ({:.0}, {:.0})",
                    t.speed[i], t.x[i], t.z[i]
                );
            }
        }
        // Dirt has less grip, so its hairpins are slower.
        let floor = if t.dirt { 6.0 } else { 7.5 };
        assert!(slowest > floor && fastest > 25.0);
        assert!(t.wall.iter().all(|w| *w > HALF + 1.5));
    }
}

#[test]
fn rivals_finish_three_laps_on_every_track() {
    for id in 0..4 {
        let mut game = race(id);
        // The player holds gas and brakes for the planned speed: a careful human.
        for _ in 0..(120 * 150) {
            let (target, v) = {
                let w = game.world();
                let t = w.resource::<Track>();
                let car = w.require::<Car>("player");
                let i = t.wrap(car.idx as i64 + 6);
                (
                    t.speed[i] * 1.04,
                    (car.vx * car.vx + car.vz * car.vz).sqrt(),
                )
            };
            if v > target + 0.5 {
                game.key_down("KeyS");
            } else {
                game.key_up("KeyS");
                game.key_down("KeyW");
            }
            game.run(1000.0 / 120.0);
            if game.world().resource::<Race>().order.len() == 6 {
                break;
            }
        }
        let w = game.world();
        let race = w.resource::<Race>();
        let t = w.resource::<Track>();
        let mut grass = 0;
        for (_, car) in w.query::<&Car>().iter() {
            println!(
                "{} car {} finished {} at {:.2}s best {:.2}s",
                track::layout(id).name,
                car.id,
                car.finished,
                (car.finish_tick - race.start) as f32 / 120.,
                car.best_lap as f32 / 120.
            );
            if car.surface == GRASS {
                grass += 1;
            }
        }
        println!("order {:?}", race.order);
        assert_eq!(race.order.len(), 6, "every car finishes on {}", t.id);
        assert!(race.order.contains(&PLAYER));
        let _ = grass;
    }
}

#[test]
fn flat_out_leaves_the_road() {
    let mut game = race(0);
    game.key_down("KeyW");
    let mut off = 0;
    for _ in 0..(120 * 30) {
        game.run(1000.0 / 120.0);
        if game.world().require::<Car>("player").surface == GRASS {
            off += 1;
        }
    }
    println!("ticks on grass flat out: {off}");
    assert!(off > 120, "full throttle everywhere should run wide");
}

#[test]
fn saves_restore_and_continue_identically() {
    let mut game = race(1);
    game.key_down("KeyW");
    game.run(5000.0);
    let saved = game.save().unwrap();
    let mut other = race(1);
    assert!(game.world().tick() > 0);
    other.restore(&saved).unwrap();
    game.run(2000.0);
    other.run(2000.0);
    assert_eq!(game.world().hash(), other.world().hash());
}

#[test]
fn scenery_budget() {
    for id in 0..4 {
        let game = race(id);
        let w = game.world();
        let mut counts = std::collections::BTreeMap::new();
        for (_, mesh) in w.query::<&exact_game::Mesh>().iter() {
            if let exact_game::Mesh::Asset(name) = mesh {
                *counts.entry(name.clone()).or_insert(0) += 1;
            }
        }
        println!("{id}: {counts:?}");
        // Foliage overdraw is the frame's main cost: keep the woods to what the camera sees.
        let trees: u32 = [
            "pine-a.model",
            "pine-b.model",
            "pine-c.model",
            "tree-a.model",
        ]
        .iter()
        .map(|m| counts.get(*m).copied().unwrap_or(0))
        .sum();
        assert!(trees < 300, "{trees} trees on track {id}");
    }
}

#[test]
fn wheels_roll_with_the_road_and_lock_under_braking() {
    let mut game = race(0);
    game.run(3500.0);
    game.key_down("KeyW");
    game.run(2500.0);
    let car = game.world().require::<Car>("player").clone();
    let ground = (car.vx * car.vx + car.vz * car.vz).sqrt();
    assert!(ground > 10.0, "{ground} m/s");
    assert!((car.roll - ground).abs() < 1.0, "rolling at {} for {ground} m/s", car.roll);
    let before = car.spin;
    game.run(100.0);
    assert_ne!(game.world().require::<Car>("player").spin, before, "wheels turn");
    // Stamp on the brake: the wheels lock well before the car stops.
    game.key_up("KeyW");
    game.key_down("KeyS");
    game.run(400.0);
    let car = game.world().require::<Car>("player").clone();
    let ground = (car.vx * car.vx + car.vz * car.vz).sqrt();
    assert!(ground > 2.0 && car.roll == 0.0, "car {ground} m/s, wheels {}", car.roll);
    let locked = car.spin;
    game.run(100.0);
    assert_eq!(game.world().require::<Car>("player").spin, locked, "locked wheels hold still");
}

/// The woods as art/src/shade.py reads them: every tree, bush and fence section
/// that casts a baked shadow, in setup's order.
fn casters(game: &Sim<KerbRacers>) -> String {
    let w = game.world();
    let dirt = w.resource::<Track>().dirt;
    let (centre, size) = kerb_racers_logic::scenery::ground_rect(&w.resource::<Track>());
    let mut out = format!(
        "[\n  {{\"kind\": \"ground\", \"x\": {:.3}, \"z\": {:.3}, \"w\": {:.3}, \"h\": {:.3}}},\n",
        centre.x, centre.y, size.x, size.y
    );
    for (_, (pose, mesh)) in w
        .query::<(&exact_game::Transform, &exact_game::Mesh)>()
        .iter()
    {
        let exact_game::Mesh::Asset(name) = mesh else {
            continue;
        };
        let kind = name.trim_end_matches(".model");
        // The dirt track's boulders cast shadows too (the woods' rocks are pebbles).
        let rock = dirt && kind == "rock";
        if !rock && !["pine-a", "pine-b", "pine-c", "tree-a", "bush", "fence"].contains(&kind) {
            continue;
        }
        if rock {
            out += &format!(
                "  {{\"kind\": \"rock\", \"x\": {:.3}, \"z\": {:.3}, \"scale\": {:.3}, \"h\": {:.3}}},\n",
                pose.position.x, pose.position.z, pose.scale.x, pose.scale.y
            );
            continue;
        }
        let along = pose.rotation * exact_game::Vec3::X * pose.scale.x;
        out += &format!(
            "  {{\"kind\": \"{kind}\", \"x\": {:.3}, \"z\": {:.3}, \"scale\": {:.3}, \"dx\": {:.3}, \"dz\": {:.3}}},\n",
            pose.position.x, pose.position.z, pose.scale.y, along.x, along.z
        );
    }
    out.truncate(out.len() - 2);
    out + "\n]\n"
}

#[test]
fn baked_shade_matches_the_woods() {
    // KR_REBAKE=1 rewrites the lists; then run `blender -b -P art/src/shade.py`.
    for id in 0..4 {
        // Setup waits for the declared models, so deliver them as a host would.
        let game = race(id);
        let path = format!(
            "{}/../../art/src/casters-{id}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let now = casters(&game);
        if std::env::var("KR_REBAKE").is_ok() {
            std::fs::write(&path, &now).unwrap();
        }
        let baked = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            baked == now,
            "track {id}'s woods moved: rebake its shade (KR_REBAKE=1, then art/src/shade.py)"
        );
    }
}

#[test]
fn radar_follows_the_player_and_names_the_neighbours() {
    let mut game = race(2);
    game.key_down("KeyW");
    game.run(8000.0);
    let w = game.world();
    let t = w.resource::<Track>();
    let order = standings(w);
    let r = radar(w, &t, &order);
    let pose = w.require::<exact_game::Transform>("player");
    let view: Vec<f32> = r.view.split(' ').map(|n| n.parse().unwrap()).collect();
    assert!((view[0] + RADAR_REACH - pose.position.x).abs() < 0.1, "{}", r.view);
    assert!((view[1] + RADAR_REACH - pose.position.z).abs() < 0.1, "{}", r.view);
    assert_eq!(r.blips.len(), 6);
    assert!(r.blips.last().unwrap().you, "the player draws on top");
    let first = r.road.split(' ').next().unwrap();
    assert_eq!(r.road.split(' ').last().unwrap(), first, "the road closes");
    let place = order.iter().position(|(id, _, _)| *id == PLAYER).unwrap();
    println!("place {} ahead {:?} behind {:?}", place + 1, r.ahead, r.behind);
    assert_eq!(r.ahead.is_empty(), place == 0);
    assert_eq!(r.behind.is_empty(), place == 5);
    assert!(r.ahead.is_empty() || r.ahead.ends_with('s'));
}

#[test]
fn cars_drift_through_corners() {
    let mut game = race(0);
    game.key_down("KeyW");
    // Every car's slip angle on the road, in the slow corners and on the straights.
    let (mut corner, mut straight) = ((0.0f32, 0), (0.0f32, 0));
    for _ in 0..(120 * 60) {
        game.run(1000.0 / 120.0);
        let w = game.world();
        let t = w.resource::<Track>();
        for (_, car) in w.query::<&Car>().iter().filter(|(_, c)| c.ai) {
            let speed = (car.vx * car.vx + car.vz * car.vz).sqrt();
            if speed < 8.0 || car.surface == GRASS {
                continue;
            }
            let slip = (car.heading - car.vz.atan2(car.vx)).sin_cos();
            let slip = slip.0.atan2(slip.1).abs().to_degrees();
            let planned = t.speed[car.idx as usize];
            let bucket = if planned < 18.0 {
                &mut corner
            } else if planned > 26.0 {
                &mut straight
            } else {
                continue;
            };
            bucket.0 += slip;
            bucket.1 += 1;
        }
    }
    let (corner, straight) = (corner.0 / corner.1 as f32, straight.0 / straight.1 as f32);
    println!("slip: corners {corner:.1}°, straights {straight:.1}°");
    assert!(corner > 6.0, "the tail hangs out in corners: {corner:.1}°");
    assert!(straight < corner / 2.0, "and straightens up after: {straight:.1}°");
}

/// Walk the ground across the whole road and verge in 10 cm steps along
/// continuous paths: no seams where the road's height model hands over between
/// samples, corners or the land.
#[test]
fn the_ground_has_no_seams() {
    for id in 0..4 {
        let t = track::build(id);
        let mut worst = (0f32, 0f32, 0f32);
        for k in -76..=76 {
            let lat = k as f32 * 0.1;
            let mut prev = None;
            for step in 0..(t.length() * 10.0) as i32 {
                let s = step as f32 * 0.1;
                let i = ((s / t.step) as usize).min(t.len() - 1);
                let u = s / t.step - i as f32;
                let p = t.at(i, lat) * (1.0 - u) + t.at(t.wrap(i as i64 + 1), lat) * u;
                let h = t.height(p, i);
                if let Some(q) = prev {
                    let d: f32 = h - q;
                    if d.abs() > worst.0 {
                        worst = (d.abs(), s, lat);
                    }
                }
                prev = Some(h);
            }
        }
        println!("{}: steepest 10 cm step {:.3} m at {:.1} m, {:.1} m across", track::layout(id).name, worst.0, worst.1, worst.2);
        assert!(worst.0 < 0.1, "a seam on {}: {worst:?}", track::layout(id).name);
    }
}

/// Coast for half a second from the racing line `s` metres into the lap.
fn coast(t: &Track, s: f32) -> f32 {
    let i = (s / t.step) as usize;
    let p = t.at(i, t.line[i]);
    let f = t.tangent(i);
    let mut body = car::Body {
        car: Car {
            idx: i as u32,
            heading: f.y.atan2(f.x),
            vx: f.x * 20.0,
            vz: f.y * 20.0,
            y: t.surface(p, i),
            vy: t.grade[i] * 20.0,
            load: 1.0,
            skill: 1.0,
            ..Car::default()
        },
        p,
    };
    car::locate(t, &mut body);
    for _ in 0..60 {
        car::step(t, &mut body, 1.0 / 120.0);
    }
    body.v().length()
}

#[test]
fn hills_slow_the_climb_and_speed_the_descent() {
    // Pine Hollow climbs to its hairpin from 108 m and runs down from 178 m;
    // the start straight is level.
    let t = track::build(0);
    let (flat, up, down) = (coast(&t, 330.0), coast(&t, 126.0), coast(&t, 203.0));
    println!("after coasting: level {flat:.2}, uphill {up:.2}, downhill {down:.2} m/s");
    assert!(up < flat - 0.4 && down > flat + 0.4);
}

/// Every car's longest flight, in ticks, over `seconds` of racing.
fn flights(track: u32, seconds: u32) -> Vec<u32> {
    let mut game = race(track);
    game.key_down("KeyW");
    let mut longest = vec![0; 6];
    for _ in 0..(120 * seconds) {
        game.run(1000.0 / 120.0);
        for (_, car) in game.world().query::<&Car>().iter() {
            let k = car.id as usize;
            longest[k] = longest[k].max(car.air_ticks);
        }
    }
    longest
}

#[test]
fn cars_fly_off_crests_and_come_down() {
    let gravel = flights(3, 30);
    println!("Gravel Pit's longest flights (ticks): {gravel:?}");
    assert!(gravel.iter().any(|t| *t >= 24), "someone gets real air");
    assert!(gravel.iter().all(|t| *t < 120), "and nobody flies for a second");
    let ring = flights(2, 20);
    assert!(
        ring.iter().all(|t| *t < car::JUMP_TICKS),
        "Kerb Ring has nothing to fly off: {ring:?}"
    );
}

#[test]
fn a_save_in_the_air_restores_in_the_air() {
    let mut game = race(3);
    game.key_down("KeyW");
    for _ in 0..(120 * 20) {
        game.run(1000.0 / 120.0);
        let car = game.world().require::<Car>("player").clone();
        if car.air && car.air_ticks > 4 {
            break;
        }
    }
    assert!(game.world().require::<Car>("player").air, "the player leaves the ground");
    let saved = game.save().unwrap();
    let mut other = race(3);
    other.restore(&saved).unwrap();
    game.run(2000.0);
    other.run(2000.0);
    assert_eq!(game.world().hash(), other.world().hash());
}
