//! Racing between cars: passes, slipstreams, the rivals' characters, and
//! what the player sees of it.
use exact_game::{Sim, Vec2, Visible};
use kerb_racers_logic::car::{self, Body, Car, GRASS, PASS_GAP, SIDE_GAP};
use kerb_racers_logic::track::{self, Track};
use kerb_racers_logic::{fx, standings, KerbRacers, Options, Race, PLAYER};

fn race_with(track: u32, assist: u32) -> Sim<KerbRacers> {
    let mut sim = Sim::<KerbRacers>::new(Options {
        track,
        assist,
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

const TICK: f64 = 1000.0 / 120.0;

/// A car on the track at sample `idx`, `side` metres off the centreline,
/// heading down the road at `speed`.
fn body(t: &Track, id: u32, idx: usize, side: f32, speed: f32) -> Body {
    let f = t.tangent(idx);
    Body {
        car: Car {
            id,
            ai: true,
            skill: 0.9,
            pace: 1.0,
            idx: idx as u32,
            side,
            crossings: 1,
            heading: f.y.atan2(f.x),
            vx: f.x * speed,
            vz: f.y * speed,
            load: 1.0,
            defence: 0.0,
            ..Car::default()
        },
        p: t.at(idx, side),
    }
}

/// A straight stretch of Pine Hollow: the start straight, well clear of corners.
fn straight(t: &Track) -> usize {
    (0..t.len())
        .find(|&i| (0..30).all(|k| t.bend[(i + k) % t.len()].abs() < 0.02) && t.turn_ahead(i) == 0.0)
        .expect("a straight")
}

#[test]
fn a_pass_holds_its_side_until_it_is_clear() {
    let t = track::build(0);
    let i = straight(&t);
    let line = t.line[i];
    // A faster car closing on a slower one on the racing line.
    let mut cars = vec![body(&t, 0, i, line, 25.0), body(&t, 1, i + 6, line, 19.0)];
    car::plan(&t, &mut cars, 1.0 / 120.0);
    assert_eq!(cars[0].car.pass, 2, "it starts a pass");
    let side = cars[0].car.pass_side;
    assert!((cars[0].car.lane_target - PASS_GAP * side).abs() < 0.01);
    // Alongside, a little ahead: it does not turn back into the car beside it.
    let beside = line + PASS_GAP * side;
    let mut cars = vec![body(&t, 0, i + 7, beside, 25.0), body(&t, 1, i + 6, line, 19.0)];
    cars[0].car.pass = 2;
    cars[0].car.pass_side = side;
    car::plan(&t, &mut cars, 1.0 / 120.0);
    assert_eq!(cars[0].car.pass, 2, "the pass holds alongside");
    let offset = line + cars[0].car.lane_target;
    assert!((offset - line).abs() >= SIDE_GAP - 0.01, "kept clear: {offset:.2} by {line:.2}");
    // Clear ahead: it is done and may take the line again.
    let mut cars = vec![body(&t, 0, i + 12, beside, 25.0), body(&t, 1, i + 6, line, 19.0)];
    cars[0].car.pass = 2;
    cars[0].car.pass_side = side;
    car::plan(&t, &mut cars, 1.0 / 120.0);
    assert_eq!(cars[0].car.pass, 0, "the pass is over once it is clear");
}

#[test]
fn nobody_steers_into_a_car_alongside() {
    let t = track::build(0);
    let i = straight(&t);
    let line = t.line[i];
    // A car level with this one, between it and the racing line or beyond it.
    for s in [-1.0f32, 1.0] {
        let mut cars = vec![body(&t, 0, i, -1.9 * s, 22.0), body(&t, 1, i, 0.3 * s, 22.0)];
        car::plan(&t, &mut cars, 1.0 / 120.0);
        let want = line + cars[0].car.lane_target;
        assert!(
            (want - cars[1].car.side).abs() >= SIDE_GAP - 0.01,
            "steers to {want:.2}, beside a car at {:.2}",
            cars[1].car.side
        );
    }
}

#[test]
fn a_slipstream_pulls_a_car_along() {
    let t = track::build(0);
    let i = straight(&t);
    let top = |draft: f32| {
        let mut b = body(&t, 0, i, t.line[i], 24.0);
        b.car.gas = 1.0;
        b.car.draft = draft;
        for _ in 0..120 {
            car::step(&t, &mut b, 1.0 / 120.0);
        }
        b.v().length()
    };
    let (clean, tow) = (top(0.0), top(1.0));
    println!("after a second flat out: {clean:.2} m/s alone, {tow:.2} m/s in the tow");
    assert!(tow > clean + 0.3);
    // And it is the car behind that gets it.
    let mut cars = vec![body(&t, 0, i, t.line[i], 25.0), body(&t, 1, i + 6, t.line[i], 25.0)];
    for _ in 0..240 {
        car::plan(&t, &mut cars, 1.0 / 120.0);
    }
    assert!(cars[0].car.draft > 0.5 && cars[1].car.draft < 0.05);
}

#[test]
fn a_lift_dips_the_nose_a_little() {
    let mut game = race_with(0, 1);
    game.key_down("KeyW");
    game.run(5000.0);
    game.key_up("KeyW");
    let mut dive = 0.0f32;
    for _ in 0..30 {
        game.run(TICK);
        dive = dive.min(game.world().require::<Car>("player").pitch);
    }
    println!("nose down {:.1}°", -dive.to_degrees());
    assert!(dive < -0.002, "a lift dips the nose: {dive}");
}

#[test]
fn a_lift_leaves_no_skid_marks() {
    let mut game = race_with(0, 1);
    game.key_down("KeyW");
    game.run(5000.0);
    game.key_up("KeyW");
    game.run(600.0);
    assert_eq!(skid_marks(&game), 0);
}

#[test]
fn the_full_assist_locks_the_wheels_and_lays_skid_marks() {
    let mut game = race_with(0, 2);
    game.key_down("KeyW");
    game.run(5000.0);
    assert_eq!(skid_marks(&game), 0, "none before anyone brakes hard");
    game.run(6000.0);
    assert!(skid_marks(&game) >= 4, "two streaks behind a locked car: {}", skid_marks(&game));
}

fn skid_marks(game: &Sim<KerbRacers>) -> usize {
    (0..fx::SKIDS)
        .filter(|k| game.world().require::<Visible>(format!("skid-{k}").as_str()).0)
        .count()
}

#[test]
fn rivals_make_mistakes_now_and_then() {
    // Over a race on every track, someone overcooks a corner, but not often.
    let mut blunders = 0;
    for id in 0..track::LAYOUTS.len() as u32 {
        let mut game = race_with(id, 1);
        game.key_down("KeyW");
        let mut was = [false; 6];
        for _ in 0..(120 * 70) {
            game.run(TICK);
            for (_, c) in game.world().query::<&Car>().iter() {
                let now = c.mistake > 0.0;
                if now && !was[c.id as usize] {
                    blunders += 1;
                }
                was[c.id as usize] = now;
            }
        }
    }
    println!("{blunders} mistakes in five races");
    assert!((2..=40).contains(&blunders), "{blunders}");
}

/// The player's car flat out for `seconds` with the given assist: ticks on the grass.
fn grass_flat_out(assist: u32, seconds: u32) -> u32 {
    let mut game = race_with(0, assist);
    game.key_down("KeyW");
    let mut off = 0;
    for _ in 0..(120 * seconds) {
        game.run(TICK);
        if game.world().require::<Car>("player").surface == GRASS {
            off += 1;
        }
    }
    off
}

#[test]
fn the_full_assist_lifts_for_the_corners() {
    let (guide, full) = (grass_flat_out(1, 30), grass_flat_out(2, 30));
    println!("on the grass flat out: {guide} ticks with the guide, {full} with the full assist");
    assert!(full * 4 < guide.max(1), "{full} vs {guide}");
}

/// A careful player braking for `k` times the planned speed, over a race:
/// passes that stick (held a second), and ticks spent side by side with
/// another car with no pass under way.
fn overtakes(track: u32, k: f32) -> (i32, u32, u32) {
    let mut game = race_with(track, 1);
    let (mut held, mut since, mut gained) = (usize::MAX, 0, 0i32);
    let (mut squeezed, mut ticks) = (0, 0);
    for _ in 0..(120 * 150) {
        let (target, v) = {
            let w = game.world();
            let t = w.resource::<Track>();
            let car = w.require::<Car>("player");
            let i = t.wrap(car.idx as i64 + 6);
            (t.speed[i] * k, Vec2::new(car.vx, car.vz).length())
        };
        if v > target + 0.5 {
            game.key_up("KeyW");
        } else {
            game.key_down("KeyW");
        }
        game.run(TICK);
        let w = game.world();
        let race = w.resource::<Race>();
        if race.order.len() == 6 {
            break;
        }
        if w.tick() < race.start {
            continue;
        }
        ticks += 1;
        let order = standings(w);
        let place = order.iter().position(|(id, _, _)| *id == PLAYER).unwrap();
        if held == usize::MAX {
            held = place;
        }
        if place != held {
            since += 1;
            if since >= 120 {
                gained += held as i32 - place as i32;
                held = place;
                since = 0;
            }
        } else {
            since = 0;
        }
        let (_, me, mine) = order.iter().find(|(id, _, _)| *id == PLAYER).unwrap().clone();
        let beside = order.iter().any(|(id, c, p)| {
            *id != PLAYER && (p - mine).abs() < 1.5 && (c.side - me.side).abs() < 2.6
        });
        if beside && me.pass == 0 && me.lane_target == 0.0 {
            squeezed += 1;
        }
    }
    (gained, squeezed, ticks)
}

#[test]
fn a_faster_driver_makes_passes_stick() {
    let mut total = 0;
    for id in [0, 1, 2, 4] {
        let (gained, squeezed, ticks) = overtakes(id, 1.08);
        let share = squeezed as f32 / ticks as f32;
        println!(
            "{}: {gained} places gained and held, {:.1}% alongside without a pass",
            track::layout(id).name,
            share * 100.0
        );
        total += gained;
        assert!(share < 0.03, "a pass never turns back into the car beside it");
    }
    assert!(total >= 6, "a quicker driver works through the field: {total}");
}

