use exact_game::Sim;
use kerb_racers_logic::car::{Car, GRASS};
use kerb_racers_logic::track::{self, Track, HALF};
use kerb_racers_logic::{KerbRacers, Options, Race, PLAYER};

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
    for id in 0..3 {
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
        assert!(slowest > 7.5 && fastest > 25.0);
        assert!(t.wall.iter().all(|w| *w > HALF + 1.5));
    }
}

#[test]
fn rivals_finish_three_laps_on_every_track() {
    for id in 0..3 {
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
    for id in 0..3 {
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

/// The woods as art/src/shade.py reads them: every tree, bush and fence section
/// that casts a baked shadow, in setup's order.
fn casters(game: &Sim<KerbRacers>) -> String {
    let w = game.world();
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
        if !["pine-a", "pine-b", "pine-c", "tree-a", "bush", "fence"].contains(&kind) {
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
    for id in 0..3 {
        // Setup places the woods before any asset arrives: no delivery needed.
        let game = Sim::<KerbRacers>::new(Options {
            track: id,
            ..Options::default()
        })
        .unwrap();
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
