//! Racing between cars: passes, slipstreams, the rivals' characters, and
//! what the player sees of it.
mod common;

use common::without_rivals;
use exact_game::{Sim, Vec2, Visible};
use kerb_racers_logic::car::{self, Body, Car, GRASS, PASS_GAP, SIDE_GAP};
use kerb_racers_logic::track::{self, Track};
use kerb_racers_logic::fx::Cue;
use kerb_racers_logic::{fx, player_pedals, standings, KerbRacers, Options, Race, PLAYER};

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

/// The player's car flat out for `seconds` with the given assist and the
/// road to itself: ticks on the grass.
fn grass_flat_out(assist: u32, seconds: u32) -> u32 {
    let mut game = race_with(0, assist);
    without_rivals(&mut game);
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
    let (none, full) = (grass_flat_out(0, 30), grass_flat_out(2, 30));
    println!("on the grass flat out: {none} ticks with no assist, {full} with the full assist");
    assert!(none > 120 && full * 4 < none, "{full} vs {none}");
}

#[test]
fn a_lift_slows_the_car_firmly_down_a_straight() {
    let t = track::build(0);
    let i = straight(&t);
    let mut b = body(&t, PLAYER, i, t.line[i], 25.0);
    (b.car.brake, b.car.lift) = (true, true);
    for _ in 0..60 {
        car::step(&t, &mut b, 1.0 / 120.0);
        assert!(!b.car.locked, "a lift locks the wheels");
    }
    let shed = (25.0 - b.v().length()) / 0.5;
    assert!(shed >= 9.5, "a lift sheds only {shed:.1} m/s²");
}

#[test]
fn the_guide_turns_red_while_a_lift_can_still_make_the_corner() {
    for id in [0, 3, 4] {
        let t = track::build(id);
        let lift = if t.dirt { car::BRAKE_DIRT } else { car::BRAKE } * car::LIFT;
        let name = track::layout(id).name;
        assert_eq!(fx::guide_cue(&t, lift * 0.8), Cue::Red, "{name}: nearly all the lift has");
        assert_eq!(fx::guide_cue(&t, lift * 0.4), Cue::Amber, "{name}: under half the lift");
        assert_eq!(fx::guide_cue(&t, lift * 0.15), Cue::Green, "{name}: a coast does it");
    }
}

/// A sample on Pine Hollow 8 m before the road is planned well under top
/// speed, and the planned speed there.
fn corner_entry(t: &Track) -> (usize, f32) {
    let ahead = |i: usize| t.speed[t.wrap((i as f32 + 8.0 / t.step) as i64)];
    let i = (0..t.len()).find(|&i| ahead(i) < track::VMAX * 0.7).expect("a corner");
    (i, ahead(i))
}

#[test]
fn the_full_assist_lifts_for_a_corner_taken_too_fast() {
    let t = track::build(0);
    let (i, plan) = corner_entry(&t);
    let gas_held = |assist: u32, over: f32| {
        let mut b = body(&t, PLAYER, i, t.line[i], plan * over);
        b.car.ai = false;
        player_pedals(&t, &mut b, true, f32::MAX, assist, 1.0 / 120.0);
        b.car
    };
    for (assist, over) in [(0, 1.1), (1, 1.1), (1, 1.3), (2, 1.0)] {
        let car = gas_held(assist, over);
        assert!(car.gas > 0.0 && !car.brake, "assist {assist} at {over}× the plan leaves the gas on");
    }
    let lift = gas_held(2, 1.1);
    assert!(
        lift.gas == 0.0 && lift.brake && lift.lift,
        "1.1× the plan: gas {} brake {} lift {}",
        lift.gas,
        lift.brake,
        lift.lift
    );
    let full = gas_held(2, 1.3);
    assert!(full.brake && !full.lift, "the full assist brakes hard when far too fast");
}

#[test]
fn the_assist_leaves_the_gas_on_down_a_straight() {
    let t = track::build(0);
    let ahead = |i: usize| t.speed[t.wrap((i as f32 + 8.0 / t.step) as i64)];
    let i = (0..t.len()).find(|&i| ahead(i) >= track::VMAX).expect("a flat-out stretch");
    for assist in [1, 2] {
        let mut b = body(&t, PLAYER, i, t.line[i], 33.0);
        b.car.ai = false;
        player_pedals(&t, &mut b, true, f32::MAX, assist, 1.0 / 120.0);
        assert!(
            b.car.gas > 0.0 && !b.car.brake,
            "assist {assist} at 33 m/s on a straight: gas {} brake {}",
            b.car.gas,
            b.car.brake
        );
    }
}

/// One tick of the player held on gas behind a slower car.
struct Behind {
    player: Car,
    /// The speed of the car ahead (m/s).
    ahead: f32,
    /// The hardest knock between the player and the car ahead (m/s).
    contact: f32,
    /// How far the car ahead is up the road (m; negative once passed).
    gap: f32,
}

/// The car ahead: held at a steady speed (m/s), or a rival driving at this
/// share of the planned speed.
#[derive(Clone, Copy)]
enum Ahead {
    Steady(f32),
    Rival(f32),
}

/// The player on gas at 24 m/s on Pine Hollow's first straight with the
/// given assist, 9 m behind a car on the racing line; `hemmed` keeps a car
/// level with the player on either side, so it has no way by.
fn on_gas_behind(ahead: Ahead, hemmed: bool, assist: u32, ticks: usize) -> Vec<Behind> {
    let t = track::build(0);
    let i = straight(&t);
    let j = i + (9.0 / t.step) as usize;
    let line = t.line[i];
    let dt = 1.0 / 120.0;
    let start = match ahead {
        Ahead::Steady(v) => v,
        Ahead::Rival(skill) => t.speed[j] * skill,
    };
    let mut cars = vec![body(&t, PLAYER, i, line, 24.0), body(&t, 1, j, t.line[j], start)];
    if let Ahead::Rival(skill) = ahead {
        cars[1].car.skill = skill;
    }
    if hemmed {
        cars.push(body(&t, 2, i, line + SIDE_GAP, 24.0));
        cars.push(body(&t, 3, i, line - SIDE_GAP, 24.0));
    }
    cars[0].car.ai = false;
    // On the ground and following it down the slope, not dropped onto it.
    for c in cars.iter_mut() {
        let i = c.car.idx as usize;
        c.car.y = t.surface(c.p, i);
        c.car.vy = t.gradient(c.p, i).dot(c.v());
    }
    let mut trace = Vec::with_capacity(ticks);
    for _ in 0..ticks {
        for c in cars.iter_mut() {
            c.car.bump = 0.0;
        }
        let caps = car::plan(&t, &mut cars, dt);
        player_pedals(&t, &mut cars[0], true, caps[0], assist, dt);
        if let Ahead::Rival(_) = ahead {
            car::drive(&t, &mut cars[1], caps[1]);
        }
        for c in cars[..2].iter_mut() {
            car::step(&t, c, dt);
        }
        if let Ahead::Steady(v) = ahead {
            let f = cars[1].forward() * v;
            (cars[1].car.vx, cars[1].car.vz) = (f.x, f.y);
        }
        car::collide(&t, &mut cars);
        for c in cars.iter_mut() {
            car::locate(&t, c);
        }
        let me = cars[0].car.clone();
        for (k, by) in [(2, SIDE_GAP), (3, -SIDE_GAP)] {
            if let Some(c) = cars.get_mut(k) {
                c.p = t.at(me.idx as usize, me.side + by);
                (c.car.idx, c.car.side, c.car.heading) = (me.idx, me.side + by, me.heading);
                (c.car.vx, c.car.vz, c.car.y) = (me.vx, me.vz, me.y);
            }
        }
        trace.push(Behind {
            ahead: cars[1].v().length(),
            contact: cars[1].car.bump,
            gap: cars[1].progress(&t) - cars[0].progress(&t),
            player: me,
        });
    }
    trace
}

fn assert_no_contact(trace: &[Behind]) {
    if let Some((tick, b)) = trace.iter().enumerate().find(|(_, b)| b.contact > 0.0) {
        panic!("tick {tick}: the player runs into the car ahead at {:.1} m/s", b.contact);
    }
}

#[test]
fn the_full_assist_follows_a_car_it_cannot_pass() {
    let trace = on_gas_behind(Ahead::Rival(0.6), true, 2, 360);
    assert_no_contact(&trace);
    let end = trace.last().unwrap();
    let speed = Vec2::new(end.player.vx, end.player.vz).length();
    println!("following {:.1} m back at {speed:.1} m/s, the car ahead at {:.1}", end.gap, end.ahead);
    assert!(
        end.gap < 9.0 && (speed - end.ahead).abs() < 2.0,
        "it follows close behind: {:.1} m at {speed:.1} m/s",
        end.gap
    );
}

#[test]
fn following_holds_a_steady_throttle() {
    let trace = on_gas_behind(Ahead::Steady(15.0), true, 2, 360);
    assert_no_contact(&trace);
    let toggles = trace.windows(2).filter(|w| w[0].player.lift != w[1].player.lift).count();
    println!("{toggles} switches between gas and lift in 3 s");
    assert!(toggles <= 6, "the pedal flickers: {toggles} switches in 3 s");
}

#[test]
fn the_full_assist_passes_a_slower_car_with_room_beside_it() {
    let trace = on_gas_behind(Ahead::Rival(0.75), false, 2, 600);
    assert_no_contact(&trace);
    let passed = trace.iter().position(|b| b.gap < -car::PASS_CLEAR);
    println!("clear past the slower car after {passed:?} ticks");
    assert!(passed.is_some(), "not past within 5 s");
}

#[test]
fn the_guide_leaves_following_to_the_player() {
    let trace = on_gas_behind(Ahead::Steady(15.0), true, 1, 360);
    assert!(
        trace.iter().any(|b| b.contact > 0.0),
        "gas held with the guide only still holds back behind a car it cannot pass"
    );
}

#[test]
fn a_fence_hit_keeps_most_of_the_speed() {
    let t = track::build(0);
    let i = straight(&t);
    let limit = t.wall[i] - 1.0;
    for ai in [false, true] {
        let mut b = body(&t, PLAYER, i, limit + 0.05, 0.0);
        b.car.ai = ai;
        let v = t.tangent(i) * 20.0 + t.normal(i) * 2.0;
        (b.car.vx, b.car.vz) = (v.x, v.y);
        car::step(&t, &mut b, 1.0 / 120.0);
        assert!(b.car.walled, "it reaches the fence");
        let kept = b.v().length() / v.length();
        assert!((0.8..0.88).contains(&kept), "ai {ai}: keeps {kept:.2} of its speed");
    }
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

