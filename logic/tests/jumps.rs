//! Where the cars leave the ground: every crest, dip, slope and whoops on
//! every track, and what the player and the rivals do over it in a race.
//! Over the sharpest crests the springs throw every car into a real jump;
//! everywhere else the flights stay as they were.

use std::sync::OnceLock;

use exact_game::{Sim, Transform, Vec2};
use kerb_racers_logic::car::{self, Body, Car, GRASS};
use kerb_racers_logic::track::{self, Lift, Track, G};
use kerb_racers_logic::{KerbRacers, Options, Race, PLAYER};

const HZ: f32 = 120.0;

/// One stretch of authored elevation, from `from` to `to` metres into the lap.
#[derive(Clone, Copy, Debug)]
struct Feature {
    lift: usize,
    label: &'static str,
    from: f32,
    to: f32,
}

fn features(id: u32) -> Vec<Feature> {
    track::layout(id)
        .lifts
        .iter()
        .enumerate()
        .filter_map(|(lift, l)| match *l {
            Lift::Crest { at, up, down, h } => Some(Feature {
                lift,
                label: if h > 0.0 { "crest" } else { "dip" },
                from: at,
                to: at + up + down,
            }),
            Lift::Whoops { at, len, .. } => Some(Feature { lift, label: "whoops", from: at, to: at + len }),
            Lift::Slope { at, len, .. } => Some(Feature { lift, label: "slope", from: at, to: at + len }),
            Lift::Bank { .. } => None,
        })
        .collect()
}

/// One flight: who, where it left the ground (m into the lap, and its
/// lateral offset there), how long (ticks) and how high above the ground (m),
/// how hard it came down, whether the springs threw it, and what followed in
/// the next two seconds.
#[derive(Clone, Copy, Debug, Default)]
struct Flight {
    car: u32,
    s: f32,
    side: f32,
    thrown: bool,
    ticks: u32,
    peak: f32,
    impact: f32,
    /// Ticks on the grass and against the fence in the two seconds after landing.
    grass: u32,
    walled: u32,
}

impl Flight {
    /// Took off from the road and its run-off, not from the land beside it.
    fn on_road(&self) -> bool {
        self.side.abs() <= track::BAND
    }
}

/// A car passing a feature's sharpest point (m/s).
#[derive(Clone, Copy, Debug)]
struct Pass {
    car: u32,
    feature: usize,
    speed: f32,
}

struct Survey {
    track: Track,
    flights: Vec<Flight>,
    passes: Vec<Pass>,
    /// Each car's race time (s), or `None` if it did not finish.
    finish: Vec<(u32, Option<f32>)>,
}

/// The sample of sharpest convex curvature inside `f` (a crest's top, a
/// dip's exit), and the speed at which gravity alone holds a car to it there.
fn sharpest(t: &Track, f: &Feature) -> (usize, f32) {
    let (a, b) = ((f.from / t.step) as usize, ((f.to / t.step) as usize).min(t.len() - 1));
    let k = (a..=b).min_by(|i, j| t.vbend[*i].total_cmp(&t.vbend[*j])).unwrap();
    let takeoff = if t.vbend[k] < 0.0 { (G / -t.vbend[k]).sqrt() } else { f32::INFINITY };
    (k, takeoff)
}

/// A whole race on track `id` with the gas held at `assist`, every flight and
/// every pass over a feature's sharpest point recorded.
fn survey(id: u32, assist: u32) -> Survey {
    let mut game = Sim::<KerbRacers>::new(Options { track: id, assist, ..Options::default() }).unwrap();
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/");
    let _ = game.load_assets(|name| std::fs::read(format!("{assets}{name}")));
    let t = game.world().resource::<Track>().clone();
    let points: Vec<usize> = features(id).iter().map(|f| sharpest(&t, f).0).collect();
    game.key_down("KeyW");
    let mut flying: Vec<Option<Flight>> = vec![None; 6];
    let mut landed: Vec<Flight> = Vec::new();
    // Flights still within two seconds of landing, and the ticks since.
    let mut open: Vec<(usize, u32)> = Vec::new();
    let mut passes = Vec::new();
    let mut last_idx = vec![0usize; 6];
    let mut last: Vec<Car> = vec![Car::default(); 6];
    for _ in 0..(HZ as u32 * 200) {
        game.run(1000.0 / HZ as f64);
        let w = game.world();
        let racing = w.tick() >= w.resource::<Race>().start;
        for (_, (pose, car)) in w.query::<(&Transform, &Car)>().iter() {
            let k = car.id as usize;
            let i = car.idx as usize;
            let speed = Vec2::new(car.vx, car.vz).length();
            let p = Vec2::new(pose.position.x, pose.position.z);
            for (feature, &point) in points.iter().enumerate() {
                let passed = last_idx[k] < point && i >= point && i - last_idx[k] < 8;
                if racing && passed {
                    passes.push(Pass { car: car.id, feature, speed });
                }
            }
            last_idx[k] = i;
            for &(f, _) in &open {
                let flight = &mut landed[f];
                if flight.car == car.id {
                    flight.grass += (car.surface == GRASS) as u32;
                    flight.walled += car.walled as u32;
                }
            }
            if car.air {
                let up = car.y - t.surface(p, i);
                let f = flying[k].get_or_insert(Flight {
                    car: car.id,
                    s: i as f32 * t.step,
                    side: car.side,
                    thrown: thrown(&last[k], car),
                    ..Flight::default()
                });
                f.ticks = car.air_ticks;
                f.peak = f.peak.max(up);
            } else if let Some(f) = flying[k].take() {
                if racing {
                    landed.push(Flight { impact: car.landing, ..f });
                    open.push((landed.len() - 1, 0));
                }
            }
            last[k] = car.clone();
        }
        for (_, since) in open.iter_mut() {
            *since += 1;
        }
        open.retain(|(_, since)| *since < 2 * HZ as u32);
        if w.resource::<Race>().order.len() == 6 {
            break;
        }
    }
    let w = game.world();
    let race = w.resource::<Race>();
    let finish = w
        .query::<&Car>()
        .iter()
        .map(|(_, c)| (c.id, c.finished.then(|| (c.finish_tick - race.start) as f32 / HZ)))
        .collect();
    Survey { track: t, flights: landed, passes, finish }
}

impl Survey {
    /// The feature a flight took off from: the narrowest one round its take-off.
    fn feature_of(features: &[Feature], s: f32) -> Option<usize> {
        (0..features.len())
            .filter(|&k| (features[k].from - 2.0..=features[k].to + 4.0).contains(&s))
            .min_by(|a, b| (features[*a].to - features[*a].from).total_cmp(&(features[*b].to - features[*b].from)))
    }

    /// The flights off the road at `feature`.
    fn flights_from<'a>(&'a self, features: &'a [Feature], feature: usize) -> impl Iterator<Item = &'a Flight> {
        self.flights.iter().filter(move |f| f.on_road() && Self::feature_of(features, f.s) == Some(feature))
    }

    fn table(&self, id: u32) -> String {
        let t = &self.track;
        let features = features(id);
        let mut out = format!("{}\n", track::layout(id).name);
        out += "  feature        lift  plan  jump | player: pass      air  peak | rivals: pass     air  peak jumps  thump grass fence\n";
        for (k, f) in features.iter().enumerate() {
            let (a, b) = ((f.from / t.step) as usize, ((f.to / t.step) as usize).min(t.len() - 1));
            let plan = t.speed[a..=b].iter().cloned().fold(f32::MAX, f32::min);
            let (_, jump) = sharpest(t, f);
            let pass = |rival: bool| {
                let v: Vec<f32> = self
                    .passes
                    .iter()
                    .filter(|p| p.feature == k && (p.car != PLAYER) == rival)
                    .map(|p| p.speed)
                    .collect();
                (v.iter().cloned().fold(f32::MAX, f32::min), v.iter().cloned().fold(0.0, f32::max))
            };
            let air = |rival: bool| {
                self.flights_from(&features, k)
                    .filter(|f| (f.car != PLAYER) == rival)
                    .fold((0, 0.0f32, 0), |(t, p, n), f| (t.max(f.ticks), p.max(f.peak), n + (f.ticks >= 12) as u32))
            };
            let jumps = || self.flights_from(&features, k).filter(|f| f.car != PLAYER && f.ticks >= 12);
            let thump = jumps().map(|f| f.impact).fold(0.0, f32::max);
            let (grass, fence) = jumps().fold((0, 0), |(g, w), f| (g + f.grass, w + f.walled));
            let ((plo, phi), (lo, hi)) = (pass(false), pass(true));
            let ((pt, pp, _), (rt, rp, rn)) = (air(false), air(true));
            out += &format!(
                "  {:<6} @{:<5} #{:<2} {plan:5.1} {jump:5.1} | {plo:5.1}..{phi:<5.1} {pt:4} {pp:5.2} | {lo:5.1}..{hi:<5.1} {rt:4} {rp:5.2} {rn:5} {thump:6.1} {grass:5} {fence:5}\n",
                f.label, f.from, f.lift
            );
        }
        let mut times: Vec<(u32, Option<f32>)> = self.finish.clone();
        times.sort_by_key(|(car, _)| *car);
        let times: Vec<String> = times
            .iter()
            .map(|(car, time)| {
                let who = if *car == PLAYER { "player".to_string() } else { format!("car {car}") };
                format!("{who} {}", time.map_or("dnf".into(), |s| format!("{s:.1}")))
            })
            .collect();
        out += &format!("  race times (s): {}\n", times.join(", "));
        let wide = self.flights.iter().filter(|f| !f.on_road() && f.ticks >= 12);
        let (mine, theirs) = wide.fold((0, 0), |(m, r), f| if f.car == PLAYER { (m + 1, r) } else { (m, r + 1) });
        out += &format!("  flights of 12+ ticks off the land beside the road: player {mine}, rivals {theirs}\n");
        out
    }
}

/// The measurements behind the jumps: every crest, dip, slope and whoops on
/// every track, its planned speed (`plan`, the slowest over it) and the speed
/// at which gravity alone holds a car to its sharpest point (`jump`), and over
/// a race what the player (flat out, no assist) and the rivals did there:
/// speeds passing that point, longest flight (ticks) and highest (m) off the
/// road, rival flights of 12+ ticks, their hardest landing (m/s) and their
/// ticks on the grass and at the fence in the two seconds after landing.
#[test]
fn flights_over_every_feature() {
    for id in 0..track::LAYOUTS.len() as u32 {
        println!("{}", race(id).table(id));
    }
}

/// Each track's race with the player flat out and no assist, run once for
/// every test here.
fn race(id: u32) -> &'static Survey {
    static RACES: OnceLock<Vec<Survey>> = OnceLock::new();
    &RACES.get_or_init(|| (0..track::LAYOUTS.len() as u32).map(|id| survey(id, 0)).collect())[id as usize]
}

/// The crests sharp enough for the springs to throw a car off: track, and
/// metres into the lap where the crest begins.
const SPRINGY: [(u32, f32); 4] = [(0, 56.0), (3, 214.0), (3, 276.0), (4, 282.0)];

fn crest(id: u32, at: f32) -> usize {
    features(id).iter().position(|f| f.label == "crest" && f.from == at).expect("a crest there")
}

/// A real jump: a quarter to six tenths of a second in the air, and high
/// enough to see the car clear of its shadow.
fn real(f: &Flight) -> bool {
    (30..=72).contains(&f.ticks) && f.peak >= 0.35
}

#[test]
fn every_car_jumps_the_sharpest_crests() {
    let mut missing = Vec::new();
    for (id, at) in SPRINGY {
        let s = race(id);
        let features = features(id);
        let flights: Vec<&Flight> = s.flights_from(&features, crest(id, at)).collect();
        let name = format!("{} @{at}", track::layout(id).name);
        let longest = |rival: bool| {
            flights
                .iter()
                .filter(|f| (f.car != PLAYER) == rival)
                .max_by_key(|f| f.ticks)
                .map_or("none".to_string(), |f| format!("{} ticks, {:.2} m", f.ticks, f.peak))
        };
        println!("{name}: player {}, rivals {}", longest(false), longest(true));
        if !flights.iter().any(|f| f.car == PLAYER && real(f)) {
            missing.push(format!("{name}: the player never jumps"));
        }
        if !flights.iter().any(|f| f.car != PLAYER && real(f)) {
            missing.push(format!("{name}: no rival jumps"));
        }
        if let Some(f) = flights.iter().find(|f| f.thrown && f.ticks > 72) {
            missing.push(format!("{name}: car {} flies {} ticks", f.car, f.ticks));
        }
        // Unthrown, a car coming much faster than the rivals flies farther:
        // that is the crest's own shape at flat-out speed (Gravel Pit @214
        // throws a car 95 ticks and 1.17 m at 24 m/s, springs or not; see the
        // speed sweep below). The player flat out gets there.
        if let Some(f) = flights.iter().find(|f| !f.thrown && (f.ticks > 100 || f.peak > 1.2)) {
            missing.push(format!("{name}: car {} flies {} ticks, {:.2} m on its own", f.car, f.ticks, f.peak));
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

/// Every rival lands from a springy crest with the next corner still
/// makeable: never into the fence, and at most a third of a second on the
/// grass in the two seconds after.
#[test]
fn landings_off_the_sharpest_crests_stay_on_the_road() {
    landings(|car| car != PLAYER);
}

/// Each landing from a springy crest by the cars `who` picks: no fence, and
/// at most 40 ticks on the grass in the two seconds after.
fn landings(who: impl Fn(u32) -> bool) {
    let mut wide = Vec::new();
    for (id, at) in SPRINGY {
        let s = race(id);
        let name = format!("{} @{at}", track::layout(id).name);
        for f in s.flights_from(&features(id), crest(id, at)).filter(|f| who(f.car) && f.ticks >= 12) {
            let line = format!(
                "{name}: car {} {} ticks, {:.2} m{}, then {} ticks on the grass, {} at the fence",
                f.car,
                f.ticks,
                f.peak,
                if f.thrown { " thrown" } else { "" },
                f.grass,
                f.walled
            );
            println!("{line}");
            if f.walled > 0 || f.grass > 40 {
                wide.push(line);
            }
        }
    }
    assert!(wide.is_empty(), "{}", wide.join("\n"));
}

/// Gravel Pit's whoops already send the rivals wide; the springs leave them
/// alone, and the racing line over them stays where it was. Before the
/// springs they threw a car up to 0.74 m off the road. (How long the rivals
/// spend on the grass after them swings with the traffic: 502 to 3023 ticks
/// over a race before the springs, as the player starts sooner or later.)
#[test]
fn the_whoops_get_no_wilder() {
    let t = track::build(3);
    let whoops = features(3).into_iter().find(|f| f.label == "whoops").unwrap();
    for i in (whoops.from / t.step) as usize..=(whoops.to / t.step) as usize {
        assert_eq!(t.spring[i], 0.0, "a spring under the whoops at {i}");
        assert_ne!(t.edge[i], track::JUMP_EDGE, "the line narrowed over the whoops at {i}");
    }
    let s = race(3);
    let features = features(3);
    let k = features.iter().position(|f| f.label == "whoops").unwrap();
    let peak = s.flights_from(&features, k).map(|f| f.peak).fold(0.0, f32::max);
    println!("the whoops: up to {peak:.2} m");
    assert!(peak <= 0.75, "the whoops throw a car {peak:.2} m");
}

/// Fern Loop's gentle crests and flat Kerb Ring give no jumps: before the
/// springs nothing there left the road by more than 0.14 m.
#[test]
fn gentle_tracks_stay_on_the_ground() {
    for id in [1, 2] {
        let s = race(id);
        let high = s.flights.iter().filter(|f| f.on_road()).map(|f| f.peak).fold(0.0, f32::max);
        println!("{}: highest flight {high:.2} m", track::layout(id).name);
        assert!(high < 0.2, "{}: a car {high:.2} m off the road", track::layout(id).name);
    }
}

/// In the air the body rises clear of its blob shadow, which stays on the
/// ground under it.
#[test]
fn the_shadow_stays_on_the_ground_under_a_jump() {
    let mut game = Sim::<KerbRacers>::new(Options { track: 3, ..Options::default() }).unwrap();
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/");
    let _ = game.load_assets(|name| std::fs::read(format!("{assets}{name}")));
    game.key_down("KeyW");
    let mut highest = 0f32;
    for _ in 0..(HZ as u32 * 40) {
        game.run(1000.0 / HZ as f64);
        let w = game.world();
        let t = w.resource::<Track>();
        let car = w.require::<Car>("player");
        if !car.air {
            continue;
        }
        let body = *w.require::<Transform>("player");
        let shadow = *w.require::<Transform>("player-shadow");
        let (p, at) = (Vec2::new(body.position.x, body.position.z), Vec2::new(shadow.position.x, shadow.position.z));
        // The blob lies on the highest ground under its nose, tail and sides.
        let (fwd, side) = (Vec2::new(car.heading.cos(), car.heading.sin()) * 1.6, Vec2::new(-car.heading.sin(), car.heading.cos()) * 0.9);
        let ground = [Vec2::ZERO, fwd, -fwd, side, -side]
            .map(|d| t.height(at + d, car.idx as usize))
            .into_iter()
            .fold(f32::MIN, f32::max);
        assert!(shadow.position.y - ground < 0.12, "the shadow {:.2} m off the ground", shadow.position.y - ground);
        highest = highest.max(car.y - t.surface(p, car.idx as usize));
    }
    println!("the player rose {highest:.2} m off the road");
    assert!(highest >= 0.35, "the player never gets clear of its shadow: {highest:.2} m");
}

/// Where the springy crests have their tops (m into the lap), by the same
/// rule the springs use.
fn springy_tops(id: u32) -> Vec<f32> {
    track::layout(id)
        .lifts
        .iter()
        .filter_map(|l| match *l {
            Lift::Crest { at, up, h, .. } if h > 0.0 => {
                let sharp = h * std::f32::consts::PI * std::f32::consts::PI / (2.0 * up * up);
                (car::springiness(sharp) > 0.0).then_some(at + up)
            }
            _ => None,
        })
        .collect()
}

/// A take-off the springs threw upward: faster up than gravity alone leaves it.
fn thrown(before: &Car, after: &Car) -> bool {
    !before.air && after.air && after.vy > before.vy - G / HZ + 0.1
}

/// A lone car coasting over the crest whose top is `top` m into the lap,
/// from `from` m past it (short of it when negative) at `speed` on the
/// racing line: its longest flight (ticks), its highest off the ground (m),
/// and how often it was thrown.
fn coast_over(t: &Track, top: f32, from: f32, speed: f32) -> (u32, f32, u32) {
    let i = t.wrap(((top + from) / t.step) as i64);
    let p = t.at(i, t.line[i]);
    let f = t.tangent(i);
    let mut body = Body {
        car: Car {
            idx: i as u32,
            heading: f.y.atan2(f.x),
            vx: f.x * speed,
            vz: f.y * speed,
            y: t.surface(p, i),
            vy: t.grade[i] * speed,
            load: 1.0,
            skill: 1.0,
            ..Car::default()
        },
        p,
    };
    car::locate(t, &mut body);
    let (mut longest, mut peak, mut throws) = (0, 0f32, 0);
    for _ in 0..(HZ as u32 * 4) {
        let before = body.car.clone();
        car::step(t, &mut body, 1.0 / HZ);
        throws += thrown(&before, &body.car) as u32;
        if body.car.air {
            longest = longest.max(body.car.air_ticks);
            peak = peak.max(body.car.y - t.surface(body.p, body.car.idx as usize));
        }
        if body.car.idx as f32 * t.step > top + 30.0 {
            break;
        }
    }
    (longest, peak, throws)
}

/// However fast a car comes over a springy crest, from barely moving to flat
/// out, it is thrown once at most, and a throw never makes a flight longer
/// than six tenths of a second or higher than 0.8 m; at race speed it is a
/// real jump. (Above 19 m/s Gravel Pit's jump throws a car farther than that
/// on its own, springs or not: the crest's shape, and no throw is added.)
#[test]
fn springy_jumps_stay_in_bounds_at_every_speed() {
    let mut wrong = Vec::new();
    for id in [0, 3, 4] {
        let t = track::build(id);
        for top in springy_tops(id) {
            for speed in (6..=24).map(|v| v as f32) {
                let (ticks, peak, throws) = coast_over(&t, top, -2.0, speed);
                println!("{} top {top}: {speed} m/s, {ticks} ticks, {peak:.2} m, {throws} throws", track::layout(id).name);
                // The speeds the rivals take it at: Misty Moor's gentler crest faster.
                let race = if id == 4 { 14.0..=16.0 } else { 12.0..=14.0 }.contains(&speed);
                if throws > 1 || (throws == 1 && (ticks > 72 || peak > 0.8)) || (race && (ticks < 30 || peak < 0.35)) {
                    wrong.push(format!("{} top {top} at {speed} m/s: {ticks} ticks, {peak:.2} m, {throws} throws", track::layout(id).name));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// However slowly a car comes, the springs throw it no harder than at
/// `THROW_FROM`.
#[test]
fn the_springs_throw_no_harder_when_slow() {
    let most = car::THROW * (car::THROW_AT / car::THROW_FROM).sqrt();
    for sharp in [0.0444, 0.0617, 0.0822, 0.1007] {
        for speed in (1..=40).map(|v| v as f32 * 0.75) {
            let throw = car::rebound(sharp, speed);
            assert!(throw <= most + 1e-4, "{throw:.2} m/s up at {speed} m/s off a crest of {sharp}");
        }
    }
}

/// Past the top, on the way down, nothing throws a car: one landing early on
/// the descent and lifting off again flies on gravity alone.
#[test]
fn no_throw_on_the_way_down() {
    for id in [0, 3, 4] {
        let t = track::build(id);
        for top in springy_tops(id) {
            for from in [1.5, 3.0, 4.5] {
                for speed in (8..=24).step_by(2).map(|v| v as f32) {
                    let (_, _, throws) = coast_over(&t, top, from, speed);
                    assert_eq!(throws, 0, "{} top {top}: thrown {from} m past it at {speed} m/s", track::layout(id).name);
                }
            }
        }
    }
}

/// A rival lapping alone is thrown only over the tops of the springy crests:
/// never on a descent, a dip, the whoops or a gentle crest.
#[test]
fn only_springy_tops_throw() {
    for id in 0..track::LAYOUTS.len() as u32 {
        let t = track::build(id);
        let tops = springy_tops(id);
        let i = t.wrap(-(10.0 / t.step) as i64);
        let p = t.at(i, t.line[i]);
        let f = t.tangent(i);
        let v = t.speed[i];
        let mut body = Body {
            car: Car {
                ai: true,
                idx: i as u32,
                heading: f.y.atan2(f.x),
                vx: f.x * v,
                vz: f.y * v,
                y: t.surface(p, i),
                load: 1.0,
                skill: 1.0,
                ..Car::default()
            },
            p,
        };
        car::locate(&t, &mut body);
        let mut throws = Vec::new();
        for _ in 0..(HZ as u32 * 50) {
            car::drive(&t, &mut body, f32::MAX);
            let before = body.car.clone();
            car::step(&t, &mut body, 1.0 / HZ);
            if thrown(&before, &body.car) {
                throws.push(body.car.idx as f32 * t.step);
            }
        }
        println!("{}: thrown at {throws:.0?}", track::layout(id).name);
        for s in &throws {
            assert!(tops.iter().any(|top| (s - top).abs() <= 2.0), "{}: thrown at {s:.1} m", track::layout(id).name);
        }
        assert_eq!(throws.is_empty(), tops.is_empty(), "{}: thrown at {throws:?}", track::layout(id).name);
    }
}
