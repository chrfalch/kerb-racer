//! Hills you can read while driving: a dashed centre line draws the road's
//! shape, and the camera lets the car climb on screen.

use exact_game::{Sim, Vec2, Vec3};
use kerb_racers_logic::car::Car;
use kerb_racers_logic::scenery;
use kerb_racers_logic::track::{self, Track};
use kerb_racers_logic::{KerbRacers, Options, Race};

fn race(track: u32) -> Sim<KerbRacers> {
    // The full assist lifts before corners, so a held throttle keeps the car on the road.
    let mut sim = Sim::<KerbRacers>::new(Options {
        track,
        assist: 2,
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

/// A paint vertex near the road's middle: how far along the lap (m), how far
/// left of the centreline, how high above the road, and where it is.
#[derive(Clone, Copy, Debug)]
struct Mark {
    s: f32,
    lat: f32,
    above: f32,
    p: Vec3,
}

/// The smooth centreline between samples `i` and `i + 1`: a cubic through
/// both, leaving each along the road's heading there. Its point at `u` (0..1)
/// and its direction.
fn centreline(t: &Track, i: usize, u: f32) -> (Vec2, Vec2) {
    let j = t.wrap(i as i64 + 1);
    let (p0, p1) = (t.centre(i), t.centre(j));
    let (m0, m1) = (t.tangent(i) * t.step, t.tangent(j) * t.step);
    let (u2, u3) = (u * u, u * u * u);
    let at = p0 * (2. * u3 - 3. * u2 + 1.) + m0 * (u3 - 2. * u2 + u) + p1 * (3. * u2 - 2. * u3) + m1 * (u3 - u2);
    let dir = p0 * (6. * u2 - 6. * u) + m0 * (3. * u2 - 4. * u + 1.) + p1 * (6. * u - 6. * u2) + m1 * (3. * u2 - 2. * u);
    (at, dir.normalize())
}

/// How far `p` lies left of the smooth centreline near sample `i`, by the
/// nearest of its points a centimetre apart.
fn off_centre(t: &Track, p: Vec2, i: usize) -> f32 {
    let mut best = (f32::MAX, 0.0);
    for a in [t.wrap(i as i64 - 1), i] {
        for k in 0..=100 {
            let (q, dir) = centreline(t, a, k as f32 / 100.0);
            let d = (p - q).length();
            if d < best.0 {
                best = (d, (p - q).dot(Vec2::new(-dir.y, dir.x)));
            }
        }
    }
    best.1
}

/// The paint down the road's middle, one list per dash, in order along the lap.
fn centre_dashes(t: &Track) -> Vec<Vec<Mark>> {
    let paint = &scenery::road(t).meshes[2];
    let mut marks = Vec::new();
    let mut hint = 0;
    for p in paint.positions.chunks(3) {
        let at = Vec2::new(p[0], p[2]);
        let (i, _) = t.locate(at, hint, t.len() as i64 / 2);
        hint = i;
        let (a, _, u, _) = t.frame(at, i);
        let s = ((a as f32 + u) * t.step).rem_euclid(t.length());
        let lat = off_centre(t, at, i);
        let above = p[1] - t.surface(at, i);
        // The chequered start line crosses the middle within 0.75 m of it.
        let start_line = s < 0.8 || s > t.length() - 0.8;
        if lat.abs() < 0.5 && (-0.1..0.3).contains(&above) && !start_line {
            marks.push(Mark { s, lat, above, p: Vec3::new(p[0], p[1], p[2]) });
        }
    }
    marks.sort_by(|a, b| a.s.total_cmp(&b.s));
    let mut dashes: Vec<Vec<Mark>> = Vec::new();
    for m in marks {
        match dashes.last_mut() {
            Some(dash) if m.s - dash.last().unwrap().s < 1.0 => dash.push(m),
            _ => dashes.push(vec![m]),
        }
    }
    dashes
}

fn length(dash: &[Mark]) -> f32 {
    dash.last().unwrap().s - dash[0].s
}

#[test]
fn asphalt_has_a_dashed_centre_line_and_dirt_none() {
    for id in 0..track::LAYOUTS.len() as u32 {
        let t = track::build(id);
        let name = track::layout(id).name;
        let found = centre_dashes(&t);
        let dashes: Vec<f32> = found.iter().map(|d| length(d)).collect();
        if t.dirt {
            assert!(dashes.is_empty(), "{name} is dirt: no paint down the middle");
            continue;
        }
        // About 1.2 m painted and 1.8 m clear.
        let expected = t.length() / 3.0;
        let vertices: usize = found.iter().map(Vec::len).sum();
        println!("{name}: {} dashes for {expected:.1}, {vertices} vertices, {dashes:?} m", dashes.len());
        assert!(
            (dashes.len() as f32 - expected).abs() <= 2.0,
            "{name}: {} dashes, about {expected:.0} expected",
            dashes.len()
        );
        assert!(dashes.iter().all(|d| (1.0..=1.5).contains(d)), "{name}: dashes {dashes:?} m");
    }
}

/// Through each asphalt track's tightest corner the dashes bend with the road
/// rather than kinking at its samples, and lie on its middle and its surface.
#[test]
fn dashes_follow_the_curve_of_the_road() {
    for id in 0..track::LAYOUTS.len() as u32 {
        let t = track::build(id);
        if t.dirt {
            continue;
        }
        let name = track::layout(id).name;
        let tightest = (0..t.len()).max_by(|&a, &b| t.bend[a].abs().total_cmp(&t.bend[b].abs())).unwrap();
        let apex = tightest as f32 * t.step;
        let corner: Vec<Vec<Mark>> = centre_dashes(&t)
            .into_iter()
            .filter(|d| d.iter().any(|m| (m.s - apex).abs() < 4.0))
            .collect();
        assert!(!corner.is_empty(), "{name}: no dash through the corner at {apex:.0} m");
        let (mut sharpest, mut widest, mut lifted) = (0f32, 0f32, 0f32);
        for dash in &corner {
            for m in dash {
                widest = widest.max(m.lat.abs());
                lifted = lifted.max((m.above - 0.05).abs());
            }
            // One edge, each corner once: the dash's outline down that side.
            let mut edge: Vec<Vec3> = Vec::new();
            for m in dash.iter().filter(|m| m.lat > 0.0) {
                if edge.last().is_none_or(|q| (m.p - *q).length() > 1e-3) {
                    edge.push(m.p);
                }
            }
            for w in edge.windows(3) {
                let (a, b) = ((w[1] - w[0]).normalize(), (w[2] - w[1]).normalize());
                sharpest = sharpest.max(a.dot(b).clamp(-1.0, 1.0).acos().to_degrees());
            }
        }
        println!(
            "{name}: tightest corner {:.1} m radius at {apex:.0} m: sharpest turn within a dash {sharpest:.2}°, \
             paint out to {widest:.3} m from the centreline, {lifted:.3} m off its height",
            1.0 / t.bend[tightest].abs()
        );
        assert!(sharpest < 3.0, "{name}: a dash kinks {sharpest:.1}° in the tightest corner");
        // Half the line's width, and a couple of centimetres.
        assert!(widest < 0.1, "{name}: paint {widest:.3} m from the centreline");
        assert!(lifted < 0.02, "{name}: paint {lifted:.3} m off its 5 cm above the road");
    }
}

/// Drive `seconds` flat out: how far the car rose above the camera's focus at
/// most, and the focus height's largest change of speed in one tick.
fn ride(track: u32, seconds: u32) -> (f32, f32) {
    let mut game = race(track);
    game.run(3500.0);
    game.key_down("KeyW");
    let mut highest = 0f32;
    let mut focus = Vec::new();
    for _ in 0..(120 * seconds) {
        game.run(1000.0 / 120.0);
        let w = game.world();
        let y = w.require::<Car>("player").y;
        let r = w.resource::<Race>();
        highest = highest.max(y - r.focus_y);
        focus.push(r.focus_y);
    }
    let jerk = focus
        .windows(3)
        .map(|f| (f[2] - 2.0 * f[1] + f[0]).abs())
        .fold(0f32, f32::max);
    (highest, jerk)
}

/// Up Misty Moor's 6 m climb the car rises on screen: from 30 m up the view,
/// 2 m above the focus is a twentieth of the screen. (Pine Hollow climbs 2 m
/// and then 3 m, too little.) Over Gravel Pit's whoops and landings the view
/// holds steady at 120 Hz.
#[test]
fn the_car_rises_on_screen_up_a_climb_and_the_view_holds_steady() {
    for id in 0..track::LAYOUTS.len() as u32 {
        let name = track::layout(id).name;
        let (highest, jerk) = ride(id, 30);
        println!("{name}: the car up to {highest:.2} m above the focus, focus jerk {jerk:.5} m/tick²");
        assert!(jerk < 0.001, "{name}: the view bobs, {jerk} m/tick²");
        if id == 4 {
            assert!(highest >= 2.0, "{name}: the car rises only {highest:.2} m above the focus");
        }
    }
}
