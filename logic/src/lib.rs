//! Kerb Racers: a short top-down circuit race. You hold the pedals (right
//! click for gas, left click to brake); the car steers itself along the
//! racing line, and the tyres decide whether it makes the corner. A or D (or
//! a thumb dragged sideways) picks the side to pass on.
pub mod car;
pub mod fx;
pub mod scenery;
pub mod sound;
pub mod track;

use car::{Body, Car};
use exact_game::*;
use track::Track;

#[derive(Args)]
pub struct Options {
    pub seed: u64,
    /// Which circuit: 0 Pine Hollow, 1 Fern Loop, 2 Kerb Ring, 3 Gravel Pit (dirt), 4 Misty Moor (wet, at dusk).
    pub track: u32,
    pub laps: u32,
    /// Graphics: 0 low (no undergrowth), 1 medium (baked shadows, the default), 2 high (real-time sun shadows, haze and bloom).
    pub quality: u32,
    /// Driving help: 0 none, 1 the braking guide on the road (the default),
    /// 2 the guide and a throttle that lifts by itself before a corner.
    pub assist: u32,
    #[live]
    pub paused: bool,
    /// Master volume, 0 (muted) to 1.
    #[live]
    pub volume: f32,
    #[restart]
    pub restart: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            seed: 7,
            track: 0,
            laps: 3,
            quality: 1,
            assist: 1,
            paused: false,
            volume: 1.0,
            restart: false,
        }
    }
}

/// Where a car's blob shadow falls from its centre: away from the low western sun.
pub const SHADOW_OFFSET: Vec2 = Vec2::new(1.1, 0.45);
/// The direction the sun throws shadows along the ground, and how far (m) per
/// metre of height: it stands about 18° above the horizon.
pub const SUN_AWAY: Vec2 = Vec2::new(0.928, 0.371);
pub const SHADOW_REACH: f32 = 3.0;

/// Seconds of countdown before the lights go out.
pub const COUNTDOWN: f32 = 3.0;

pub struct Driver {
    pub name: &'static str,
    pub paint: u32,
    pub trim: u32,
    pub skill: f32,
    /// How late it brakes and how small a gap it goes for (0..1).
    pub aggression: f32,
    /// How rarely it overcooks a corner (0..1).
    pub consistency: f32,
    /// How hard it covers the inside against a car behind (0..1).
    pub defence: f32,
}

/// Grid order, front to back. The player starts fifth.
pub const DRIVERS: [Driver; 6] = [
    Driver {
        name: "Juno",
        paint: 0x2a2c33,
        trim: 0xf2c230,
        skill: 0.925,
        aggression: 0.6,
        consistency: 0.96,
        defence: 0.8,
    },
    Driver {
        name: "Mika",
        paint: 0x8b4fd8,
        trim: 0xf1efe8,
        skill: 0.915,
        aggression: 0.85,
        consistency: 0.88,
        defence: 0.5,
    },
    Driver {
        name: "Rook",
        paint: 0xf1efe8,
        trim: 0xe8742a,
        skill: 0.9,
        aggression: 0.4,
        consistency: 0.94,
        defence: 0.95,
    },
    Driver {
        name: "Ayla",
        paint: 0x3fa7d6,
        trim: 0xf1efe8,
        skill: 0.885,
        aggression: 0.7,
        consistency: 0.9,
        defence: 0.6,
    },
    Driver {
        name: "You",
        paint: 0xf2c230,
        trim: 0x22252b,
        skill: 0.9,
        aggression: 0.5,
        consistency: 1.0,
        defence: 0.0,
    },
    Driver {
        name: "Vex",
        paint: 0xf1efe8,
        trim: 0xd83a2e,
        skill: 0.87,
        aggression: 0.95,
        consistency: 0.85,
        defence: 0.3,
    },
];
pub const PLAYER: u32 = 4;

#[derive(Clone, Debug, Default, Resource)]
pub struct Race {
    pub laps: u32,
    /// Tick the lights go out.
    pub start: u64,
    /// Car ids in finishing order.
    pub order: Vec<u32>,
    /// Car ids as the driver list shows them: see `settle_board`.
    pub board: Vec<u32>,
    /// Where the camera looks, eased toward the player's path.
    pub focus_x: f32,
    pub focus_z: f32,
    /// The ground height the camera looks at, eased like the focus.
    pub focus_y: f32,
    /// The camera's shake after the player lands hard (m), dying away.
    pub shake: f32,
    /// The camera's height above the focus, eased with speed.
    pub lift: f32,
    /// Driving help (see `Options::assist`).
    pub assist: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Row {
    pub id: u32,
    pub place: u32,
    pub name: String,
    pub color: String,
    pub time: String,
    pub you: bool,
}

/// One car on the radar: where it is (world metres) and which way it points.
#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Blip {
    pub id: u32,
    /// `translate(x z) rotate(deg)`, ready for the SVG.
    pub at: String,
    pub fill: String,
    pub ring: String,
    pub you: bool,
}

/// The radar: a window of the circuit centred on the player, drawn the same
/// way up as the camera, so it shows who is close by rather than the whole lap.
#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Radar {
    /// The SVG viewBox, in world metres around the player.
    pub view: String,
    /// The whole centreline as SVG points; the viewBox crops it.
    pub road: String,
    pub blips: Vec<Blip>,
    /// The car one place up the order and the time to it ("Ayla 1.2s"), or "".
    pub ahead: String,
    /// The car one place down and the time it trails by, or "".
    pub behind: String,
}

/// Half the radar's width, in metres: a little past the edge of the screen.
pub const RADAR_REACH: f32 = 30.0;

#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct Hud {
    /// "grid", "race" or "done".
    pub phase: String,
    pub count: u32,
    pub go: bool,
    pub track: String,
    pub lap: u32,
    pub laps: u32,
    pub place: u32,
    pub cars: u32,
    pub time: String,
    pub best: String,
    pub last: String,
    pub speed: u32,
    pub gas: bool,
    pub brake: bool,
    /// The player's car is in the air off a jump.
    pub air: bool,
    /// The player's car is in another's slipstream.
    pub draft: bool,
    pub off: bool,
    pub standings: Vec<Row>,
    pub radar: Radar,
}

pub fn clock(ticks: u64, hz: u32) -> String {
    let cs = ticks * 100 / hz as u64;
    format!("{}:{:02}.{:02}", cs / 6000, cs / 100 % 60, cs % 100)
}

fn name_of(id: u32) -> String {
    if id == PLAYER {
        "player".into()
    } else {
        format!("rival-{}", id + 1)
    }
}

/// A tree's crown, so `present` can fade the ones standing between the camera and a car.
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct Crown {
    pub radius: f32,
    pub height: f32,
}

/// One of a car's four wheels, drawn turning with the car's `spin` and, at
/// the front, pointing along its `steer`.
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct Wheel {
    pub front: bool,
}

/// Where each car's wheels sit (axle x fore and aft, half track), and its rims,
/// as `art/src/cars.py` prints them: the three body styles in grid order.
fn wheel_layout(id: u32) -> (f32, f32, f32, &'static str) {
    match id {
        0 | 4 => (0.908, -0.862, 0.72, "fuchs"),
        1 | 5 => (0.949, -0.906, 0.75, "disc"),
        _ => (0.976, -0.944, 0.731, "disc"),
    }
}

/// The Blender models and ground textures, then the sounds (`sound::ASSETS`).
const MODELS: [&str; 35] = [
    "car-0.model",
    "car-1.model",
    "car-2.model",
    "car-3.model",
    "car-4.model",
    "car-5.model",
    "pine-a.model",
    "pine-b.model",
    "pine-c.model",
    "tree-a.model",
    "bush.model",
    "fern.model",
    "daisies.model",
    "mushrooms.model",
    "logs.model",
    "stump.model",
    "rock.model",
    "wheel-disc-l.model",
    "wheel-disc-r.model",
    "wheel-fuchs-l.model",
    "wheel-fuchs-r.model",
    "fence.model",
    "barrier.model",
    "poppies.model",
    "asphalt.tex",
    "grass.tex",
    "clay.tex",
    "dirt.tex",
    "gravel.tex",
    "scree.tex",
    "blob.tex",
    "shade-0.tex",
    "shade-1.tex",
    "shade-2.tex",
    "shade-3.tex",
];
const ASSETS: [&str; MODELS.len() + sound::ASSETS.len()] = {
    let mut all = [""; MODELS.len() + sound::ASSETS.len()];
    let mut i = 0;
    while i < all.len() {
        all[i] = if i < MODELS.len() {
            MODELS[i]
        } else {
            sound::ASSETS[i - MODELS.len()]
        };
        i += 1;
    }
    all
};

pub struct KerbRacers;
impl Game for KerbRacers {
    const ID: &'static str = "kerb-racers";
    const HZ: u32 = 120;
    /// The models, textures and sounds. Setup waits for them, so the race
    /// (and its countdown) only begins once the whole track can be drawn and
    /// heard; the loading screen in app.contract covers the wait.
    const ASSETS: &'static [&'static str] = &ASSETS;
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .button("gas", &["MouseRight", "ArrowUp", "KeyW"])
            .button_touch("gas", Region::Right)
            .button("brake", &["MouseLeft", "ArrowDown", "KeyS", "Space"])
            .button_touch("brake", Region::Left)
            // The side to pass on, from the driver's seat: A or ← for the
            // left, D or → for the right; on touch, drag the thumb that
            // holds a pedal toward the side on screen.
            .button("left", &["KeyA", "ArrowLeft"])
            .button("right", &["KeyD", "ArrowRight"])
            .stick("swipe", Stick::touch(Region::All))
    }
    fn validate(args: &Options) -> Result<(), String> {
        if args.quality > 2 {
            return Err(format!("quality is 0, 1 or 2, not {}", args.quality));
        }
        if args.assist > 2 {
            return Err(format!("assist is 0, 1 or 2, not {}", args.assist));
        }
        if !(0.0..=1.0).contains(&args.volume) {
            return Err(format!("volume is 0 to 1, not {}", args.volume));
        }
        if args.laps == 0 || args.laps > 20 {
            return Err(format!("laps must be 1 to 20, not {}", args.laps));
        }
        Ok(())
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        let t = track::build(args.track % track::LAYOUTS.len() as u32);
        light(w, args.quality, t.wet);
        let id = t.id;
        let ground = w
            .generated_model(&format!("ground-{id}.model"), scenery::ground(&t))
            .expect("ground");
        w.spawn_named("ground", (Transform::default(), ground));
        let road = w
            .generated_model(&format!("road-{id}.model"), scenery::road(&t))
            .expect("road");
        w.spawn_named("road", (Transform::default(), road));
        // No flowers on the dirt track's scree, nor up on the wet moor.
        if !t.dirt && !t.wet {
            let flowers = scenery::flowers(w, &t);
            let flowers = w
                .generated(&format!("flowers-{id}.model"), flowers)
                .expect("flowers");
            w.spawn_named("flowers", (Transform::default(), flowers, Ambient));
        }
        for prop in scenery::props(w, &t, args.quality) {
            let ground = t.height_any(prop.at) - prop.sink;
            let e = w.spawn((
                Transform {
                    position: Vec3::new(prop.at.x, ground, prop.at.y),
                    rotation: Quat::from_rotation_y(-prop.yaw) * Quat::from_rotation_z(prop.pitch),
                    scale: prop.scale,
                },
                Mesh::asset(prop.model),
                // Nothing further than this from the camera can be on screen.
                ModelLod {
                    levels: Vec::new(),
                    hide: Some(72.0),
                },
                Ambient,
            ));
            if let Some((radius, height)) = prop.crown {
                w.insert(e, Crown { radius, height });
            }
        }
        // Real-time sun shadows are the costliest thing on screen (Safari's
        // WebGPU above all), so below high quality the woods' shadows are baked
        // into an overlay and each car's blob is its sun shadow too. With
        // real-time shadows, or under the rain clouds where there is no sun,
        // the blob is only a soft contact shadow right under the car.
        let baked = args.quality < 2 || t.wet;
        if baked && !t.wet {
            let shade = w
                .generated_model(&format!("shade-{id}.model"), scenery::shade_overlay(&t))
                .expect("shade");
            w.spawn_named("shade", (Transform::default(), shade, Ambient));
        }
        let blob = w
            .generated_model("blob.model", scenery::blob())
            .expect("blob");
        let mut cars = Vec::new();
        for (k, d) in DRIVERS.iter().enumerate() {
            let k = k as u32;
            let row = k / 2;
            let behind = 5.0 + 6.0 * row as f32 + 3.0 * (k % 2) as f32;
            let idx = t.wrap(-((behind / t.step) as i64));
            let side = if k.is_multiple_of(2) { 1.9 } else { -1.9 };
            let p = t.at(idx, side);
            let f = t.tangent(idx);
            let heading = math::atan2(f.y, f.x);
            let model = Mesh::asset(format!("car-{k}.model"));
            let y = t.surface(p, idx);
            let e = w.spawn_named(
                name_of(k),
                (
                    Transform {
                        position: Vec3::new(p.x, y, p.y),
                        rotation: Quat::from_rotation_y(-heading),
                        scale: Vec3::ONE,
                    },
                    model,
                    Car {
                        id: k,
                        ai: k != PLAYER,
                        skill: d.skill,
                        aggression: d.aggression,
                        consistency: d.consistency,
                        defence: d.defence,
                        pace: 1.0,
                        heading,
                        idx: idx as u32,
                        side,
                        y,
                        load: 1.0,
                        ..Car::default()
                    },
                ),
            );
            cars.push((k, e));
            puffs(w, e, &name_of(k), t.dirt, t.wet);
            fx::setup_car(w, e, &name_of(k), k);
            if t.wet {
                headlights(w, e);
            }
            let (front, rear, track, rims) = wheel_layout(k);
            for (x, is_front) in [(front, true), (rear, false)] {
                // Blender's left (+Y) is the game's -Z.
                for (z, side) in [(-track, "l"), (track, "r")] {
                    w.spawn((
                        Transform::at(x, car::WHEEL_R, z),
                        Mesh::asset(format!("wheel-{rims}-{side}.model")),
                        Wheel { front: is_front },
                        Parent(e),
                        Ambient,
                    ));
                }
            }
            w.spawn_named(
                format!("{}-shadow", name_of(k)),
                (
                    Transform::at(p.x, y + 0.1, p.y),
                    blob.clone(),
                    Ambient,
                ),
            );
            if k == PLAYER {
                let marker = w
                    .generated("marker.model", scenery::marker())
                    .expect("marker");
                w.spawn_named(
                    "marker",
                    (
                        Transform::at(0.0, 2.6, 0.0),
                        marker,
                        Material {
                            color: [1.0, 1.0, 1.0, 1.0],
                            ..Material::glow([0.9, 0.7, 0.2])
                        },
                        Parent(e),
                    ),
                );
            }
        }
        sound::setup(w, &cars, t.dirt);
        let start = t.at(t.wrap(-12), 0.0);
        let focus_y = t.surface(start, t.wrap(-12));
        w.insert_resource(Race {
            laps: args.laps,
            start: w.tick() + (COUNTDOWN * Self::HZ as f32) as u64,
            order: Vec::new(),
            board: Vec::new(),
            focus_x: start.x,
            focus_z: start.y,
            focus_y,
            shake: 0.0,
            lift: 30.0,
            assist: args.assist,
        });
        if t.wet {
            rain(w);
        }
        w.insert_resource(t);
        w.spawn_named(
            "camera",
            (
                Transform::default(),
                Camera {
                    fov_y_degrees: 36.0,
                    ..Camera::default()
                },
            ),
        );
        fx::setup(w, args.assist);
        camera(w, true);
        hud(w);
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    /// Trees between the camera and a car thin out (a dithered fade), so the
    /// race is never hidden behind a crown.
    fn present(p: &mut Present<'_>, _: &Options) {
        let Some(cam) = p.named("camera").and_then(|e| p.global(e)) else {
            return;
        };
        let eye = Vec3::from(cam.translation);
        let mut cars = Vec::new();
        p.for_each::<Car>(|e, _| {
            if let Some(g) = p.global(e) {
                cars.push(Vec3::from(g.translation) + Vec3::Y * 0.6);
            }
        });
        let mut fade = Vec::new();
        p.for_each::<Crown>(|e, crown| {
            let Some(g) = p.global(e) else { return };
            let at = Vec3::from(g.translation);
            let hidden = cars.iter().any(|car| {
                // Where the sight line from the eye to the car passes the trunk.
                let d = *car - eye;
                let flat = Vec2::new(d.x, d.z);
                let s = (Vec2::new(at.x - eye.x, at.z - eye.z).dot(flat) / flat.length_squared())
                    .clamp(0.0, 1.0);
                let near = Vec2::new(eye.x + flat.x * s - at.x, eye.z + flat.y * s - at.z).length();
                let y = eye.y + d.y * s;
                near < crown.radius && y < at.y + crown.height
            });
            if hidden {
                fade.push(e);
            }
        });
        for e in fade {
            p.insert(e, Opacity(0.28));
        }
    }
    fn tick(w: &mut World, input: &Input, args: &Options) {
        let now = w.tick();
        let (start, laps) = {
            let r = w.resource::<Race>();
            (r.start, r.laps)
        };
        if now >= start {
            race(w, input, now, start, laps);
        } else {
            let dt = w.dt();
            let mut player = w.require_mut::<Car>("player");
            player.gas = throttle(player.gas, input.held("gas"), dt);
            player.brake = input.held("brake");
        }
        sound::tick(w, now, start, args.volume);
        camera(w, false);
        settle_board(w);
        hud(w);
    }
}

fn race(w: &mut World, input: &Input, now: u64, start: u64, laps: u32) {
    let dt = w.dt();
    let t = w.resource::<Track>().clone();
    let mut cars: Vec<(Entity, Body)> = w
        .query::<(&Transform, &Car)>()
        .iter()
        .map(|(e, (pose, car))| {
            (
                e,
                Body {
                    car: car.clone(),
                    p: Vec2::new(pose.position.x, pose.position.z),
                },
            )
        })
        .collect();
    cars.sort_by_key(|(_, b)| b.car.id);
    let (entities, mut list): (Vec<Entity>, Vec<Body>) = cars.into_iter().unzip();
    for body in list.iter_mut() {
        body.car.bump = 0.0;
    }
    let assist = w.resource::<Race>().assist;
    if let Some(player) = list.iter_mut().find(|b| b.car.id == PLAYER) {
        steer_input(&t, player, input);
    }
    let caps = car::plan(&t, &mut list, dt);
    // Rivals keep in touch: a little quicker while behind the player, a
    // little easier while ahead, at most 3% either way.
    let player_progress = list
        .iter()
        .find(|b| b.car.id == PLAYER)
        .map(|b| (b.progress(&t), b.car.finished));
    for (k, body) in list.iter_mut().enumerate() {
        if body.car.ai || body.car.finished {
            if !body.car.ai {
                // The player's car cruises home once the flag has fallen.
                body.car.skill = 0.85;
                body.car.pace = 1.0;
            } else {
                body.car.pace = match player_progress {
                    Some((p, false)) => 1.0 + ((p - body.progress(&t)) / 60.0).clamp(-1.0, 1.0) * 0.03,
                    _ => 1.0,
                };
                // Bolder drivers carry a little more into the corners.
                body.car.pace *= 1.0 + 0.012 * (body.car.aggression - 0.5);
                car::blunder(&t, &mut body.car);
            }
            car::drive(&t, body, caps[k]);
        } else {
            body.car.brake = input.held("brake");
            body.car.gas = if body.car.brake {
                0.0
            } else {
                throttle(body.car.gas, input.held("gas"), dt)
            };
            if assist >= 2 {
                lift_for_corners(&t, body);
            }
        }
        if now == start {
            body.car.lap_tick = start;
        }
        let before = body.car.crossings;
        car::step(&t, body, dt);
        if body.car.crossings > before && body.car.crossings >= 2 && !body.car.finished {
            let lap = now - body.car.lap_tick;
            body.car.last_lap = lap;
            if body.car.best_lap == 0 || lap < body.car.best_lap {
                body.car.best_lap = lap;
            }
            body.car.lap_tick = now;
            if body.car.crossings > laps {
                body.car.finished = true;
                body.car.finish_tick = now;
                let mut race = w.resource_mut::<Race>();
                race.order.push(body.car.id);
                drop(race);
                w.emit(if body.car.id == PLAYER {
                    "finished"
                } else {
                    "rival-finished"
                });
            }
        }
    }
    car::collide(&t, &mut list);
    turn_wheels(w, &entities, &list);
    let shake = list
        .iter()
        .find(|b| b.car.id == PLAYER)
        .map_or(0.0, |b| b.car.landing);
    if shake > 1.0 {
        let mut race = w.resource_mut::<Race>();
        race.shake = race.shake.max((shake * 0.045).min(0.32));
    }
    // A buzz through the view while the player rides a kerb, a jolt on a bump.
    if let Some(p) = list.iter().find(|b| b.car.id == PLAYER) {
        let speed = p.v().length();
        let kerb: f32 = if p.car.kerb && speed > 6.0 { 0.035 } else { 0.0 };
        let bump = if p.car.bump > 2.0 { (p.car.bump * 0.02).min(0.2) } else { 0.0 };
        if kerb > 0.0 || bump > 0.0 {
            let mut race = w.resource_mut::<Race>();
            race.shake = race.shake.max(kerb.max(bump));
        }
    }
    for body in list.iter_mut() {
        car::locate(&t, body);
    }
    // High quality's sun casts the cars' own shadows.
    let live_sun = w.get::<DirectionalLight>("sun").is_some_and(|sun| sun.shadows);
    for (e, mut body) in entities.iter().zip(list) {
        let name = name_of(body.car.id);
        fx::car(w, &t, &mut body, &name, dt);
        if body.car.id == PLAYER {
            fx::player(w, &t, &body, assist);
        }
        let car = &body.car;
        let mut pose = w.require_mut::<Transform>(*e);
        pose.position = Vec3::new(body.p.x, car.y + car.squash, body.p.y);
        pose.rotation = Quat::from_rotation_y(-car.heading)
            * Quat::from_rotation_z(car.pitch)
            * Quat::from_rotation_x(car.lean);
        drop(pose);
        let speed = Vec2::new(car.vx, car.vz).length();
        // The shadow lies on the ground below: in the air it slides away from
        // the low sun, shrinks and fades, which is how a jump reads from above.
        let idx = car.idx as usize;
        let up = (car.y - t.surface(body.p, idx)).max(0.0);
        // Under cloud, or beside a real-time sun shadow, it stays right below the car.
        let at = if t.wet || live_sun {
            body.p
        } else {
            body.p + SHADOW_OFFSET + SUN_AWAY * (up * SHADOW_REACH)
        };
        *w.require_mut::<Transform>(format!("{name}-shadow").as_str()) = blob_pose(&t, at, idx, car.heading, up);
        // On dirt every car trails dust; there is no rubber to smoke.
        let smoke = !t.dirt && !t.wet && car.sliding && car.surface != car::GRASS && speed > 5.0;
        // On a wet road every car throws up spray behind it.
        let dust = (t.dirt || car.surface != car::ASPHALT || (t.wet && speed > 9.0)) && speed > 3.0 && !car.air;
        // A hard landing throws up a cloud, wherever it comes down.
        if car.landing > 1.0 {
            let puff = (6.0 + car.landing * 3.0).min(26.0) as u32;
            w.require_mut::<Emitter>(format!("{name}-dust").as_str()).state.burst += puff;
        }
        for (suffix, on) in [("smoke", smoke), ("dust", dust)] {
            let target = format!("{name}-{suffix}");
            // Read first: a mutable borrow marks the row written even when nothing changes.
            if w.require::<Emitter>(target.as_str()).running != on {
                w.require_mut::<Emitter>(target.as_str()).running = on;
            }
        }
        *w.require_mut::<Car>(*e) = body.car;
    }
}

/// The gas pedal under a finger or a key: held, it comes in quickly from a
/// little over half throttle to full, so a quick dab only feathers it.
fn throttle(gas: f32, held: bool, dt: f32) -> f32 {
    if held {
        (gas.max(0.55) + dt * 1.8).min(1.0)
    } else {
        0.0
    }
}

/// The player's choice of side: A/D or the arrows from the driver's seat,
/// or a thumb dragged across the screen toward a side. It holds for a
/// couple of seconds after the input lets go.
fn steer_input(t: &Track, body: &mut Body, input: &Input) {
    const HOLD: u32 = 240;
    let car = &mut body.car;
    car.nudge_ticks = car.nudge_ticks.saturating_sub(1);
    // The road's positive side is the driver's right as the camera sees it.
    let mut chosen = 0.0;
    if input.held("left") {
        chosen = -1.0;
    } else if input.held("right") {
        chosen = 1.0;
    }
    let drag = input.stick("swipe");
    if chosen == 0.0 && drag.length() > 0.5 {
        // Screen right is +x and screen up is -z: which side of the road is that?
        let toward = Vec2::new(drag.x, -drag.y);
        let lateral = toward.dot(t.normal(car.idx as usize));
        if lateral.abs() > 0.35 * toward.length() {
            chosen = lateral.signum();
        }
    }
    if chosen != 0.0 {
        car.nudge = chosen;
        car.nudge_ticks = HOLD;
    } else if car.nudge_ticks == 0 {
        car.nudge = 0.0;
    }
}

/// The full assist: off the gas when the car is coming into a corner too
/// fast for it, and on the brakes when far too fast.
fn lift_for_corners(t: &Track, body: &mut Body) {
    let i = t.wrap(body.car.idx as i64 + (8.0 / t.step) as i64);
    let v = body.v().length();
    let plan = t.speed[i];
    if v > plan * 1.05 {
        body.car.gas = 0.0;
    }
    if v > plan * 1.14 {
        body.car.brake = true;
    }
}

/// Each wheel turns by its car's `spin` (rolling forward about the axle, which
/// runs along local Z) and the front pair also steers.
fn turn_wheels(w: &mut World, cars: &[Entity], bodies: &[Body]) {
    let wheels: Vec<(Entity, Entity, bool)> = w
        .query::<(&Parent, &Wheel)>()
        .iter()
        .map(|(e, (parent, wheel))| (e, parent.0, wheel.front))
        .collect();
    for (e, parent, front) in wheels {
        let Some(k) = cars.iter().position(|c| *c == parent) else {
            continue;
        };
        let car = &bodies[k].car;
        let steer = if front { car.steer } else { 0.0 };
        // Heading grows toward the car's local +Z, so a turn that way is -Y.
        let rotation = Quat::from_rotation_y(-steer) * Quat::from_rotation_z(-car.spin);
        // The wheels stay on the road while the body squashes, and hang a
        // little in the air.
        let y = car::WHEEL_R - car.squash - if car.air { 0.07 } else { 0.0 };
        // Read first: a mutable borrow marks the row written even when nothing changes.
        let pose = *w.require::<Transform>(e);
        if pose.rotation != rotation || pose.position.y != y {
            let mut pose = w.require_mut::<Transform>(e);
            pose.rotation = rotation;
            pose.position.y = y;
        }
    }
}

/// Two trails behind each car: white tyre smoke for a slide on the road,
/// brown dust off it. Both stay where they were born and start switched off.
/// On a dirt track the dust is a pale ochre plume, thicker and longer-lived.
fn puffs(w: &mut World, car: Entity, name: &str, dirt: bool, wet: bool) {
    let dust = if wet {
        // Spray off the wet road: pale, cool, lingering in the damp air.
        ([0.45, 0.52, 0.54, 0.16], [0.4, 0.47, 0.5, 0.0], [0.8, 3.6])
    } else if dirt {
        ([0.78, 0.64, 0.45, 0.5], [0.82, 0.72, 0.56, 0.0], [0.8, 3.2])
    } else {
        ([0.62, 0.45, 0.28, 0.42], [0.66, 0.55, 0.4, 0.0], [0.6, 2.2])
    };
    for (suffix, birth, death, size) in [
        (
            "smoke",
            [0.9, 0.9, 0.88, 0.26],
            [0.85, 0.85, 0.85, 0.0],
            [0.45, 1.5],
        ),
        ("dust", dust.0, dust.1, dust.2),
    ] {
        w.spawn_named(
            format!("{name}-{suffix}"),
            (
                Transform::at(-1.5, 0.25, 0.0),
                Emitter {
                    shape: emitter::Shape::Sphere(0.35),
                    rate: if dirt && suffix == "dust" { 26.0 } else { 18.0 },
                    lifetime: if dirt && suffix == "dust" { 1.3 } else { 0.85 },
                    speed: 0.8,
                    spread: 1.3,
                    gravity: Vec3::new(0.0, 0.7, 0.0),
                    drag: 1.8,
                    size,
                    color: [birth, death],
                    ease: emitter::Ease::Smooth,
                    seed: car.index() as u64 * 2 + (suffix == "dust") as u64,
                    bound: [-6.0, -1.0, -6.0, 6.0, 4.0, 6.0],
                    additive: false,
                    running: false,
                    ..Emitter::default()
                },
                emitter::WorldSpace::default(),
                Parent(car),
                Ambient,
            ),
        );
    }
}

/// A car's blob laid on the ground at `at`: tilted to the plane through the
/// ground at its nose, tail and sides rather than the slope at one point, and
/// lifted clear of every sample, so on a crest, in a dip or across a bank's
/// edge it neither stands up on its side nor sinks into the ground.
fn blob_pose(t: &Track, at: Vec2, idx: usize, heading: f32, up: f32) -> Transform {
    // Inside the blob's 4.6 m by 2.5 m, where its shade is darkest.
    let (reach, half) = (1.6, 0.9);
    let fwd = Vec2::new(math::cos(heading), math::sin(heading));
    let side = Vec2::new(-fwd.y, fwd.x);
    let h = |d: Vec2| t.height(at + d, idx);
    let (nose, tail) = (h(fwd * reach), h(-fwd * reach));
    let (left, right) = (h(side * half), h(-side * half));
    let slope = fwd * ((nose - tail) / (2.0 * reach)) + side * ((left - right) / (2.0 * half));
    let tilt = Quat::from_rotation_arc(Vec3::Y, Vec3::new(-slope.x, 1.0, -slope.y).normalize());
    let y = h(Vec2::ZERO).max((nose + tail) * 0.5).max((left + right) * 0.5);
    Transform {
        position: Vec3::new(at.x, y + 0.1, at.y),
        rotation: tilt * Quat::from_rotation_y(-heading),
        scale: Vec3::splat(1.0 / (1.0 + 0.35 * up)),
    }
}

/// A low, late-afternoon sun from the west: long shadows across the road,
/// warm light on everything facing it, cool sky in the shade, a golden haze.
fn light(w: &mut World, quality: u32, wet: bool) {
    if wet {
        dusk(w, quality);
        return;
    }
    w.insert_resource(Environment {
        background: Some([0.42, 0.36, 0.22].into()),
        zenith: [0.22, 0.34, 0.55],
        horizon: [0.95, 0.72, 0.45],
        ground: [0.10, 0.09, 0.05],
        // Moderate ambient and a stronger sky fill: soft shade, not black woods.
        ambient: 0.5,
        sun_disc: 0.0,
        exposure: 0.9,
        tone_mapping: ToneMapping::Aces,
        // The golden haze and bloom are full-screen passes: high quality only.
        fog: (quality >= 2).then_some(Fog {
            color: Some([0.95, 0.74, 0.48].into()),
            ..Fog::new(0.006, 0.12)
        }),
        bloom: (quality >= 2).then_some(Bloom {
            threshold: 0.85,
            intensity: 0.28,
            radius: 2.0,
        }),
    });
    // Contact shadows where trunks, fences, kerbs and cars meet the ground.
    if quality >= 1 {
        w.insert_resource(AmbientOcclusion {
            radius: 1.2,
            intensity: 0.9,
            ..AmbientOcclusion::default()
        });
    }
    w.spawn_named(
        "sun",
        (
            Transform::at(-70.0, 24.0, -28.0).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight {
                color: [1.0, 0.72, 0.45],
                illuminance: 22000.0,
                shadows: quality >= 2,
            },
        ),
    );
    // Skylight from the other side, so shadows stay blue rather than black.
    w.spawn_named(
        "sky",
        (
            Transform::at(40.0, 60.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight {
                color: [0.55, 0.68, 1.0],
                illuminance: 3800.0,
                shadows: false,
            },
        ),
    );
}

/// Misty Moor: a raincloud at dusk. A low teal sky, thick mist pooled in the
/// hollows, no sun; the cars' headlights carry the scene, so the haze and
/// bloom that are High's alone elsewhere are on at every quality here.
fn dusk(w: &mut World, quality: u32) {
    let mist = [0.15, 0.21, 0.22];
    w.insert_resource(Environment {
        background: Some(mist.into()),
        zenith: [0.08, 0.12, 0.14],
        horizon: [0.14, 0.19, 0.2],
        ground: [0.02, 0.03, 0.03],
        ambient: 0.35,
        sun_disc: 0.0,
        exposure: 0.85,
        tone_mapping: ToneMapping::Aces,
        fog: Some(Fog {
            color: Some(mist.into()),
            ..Fog::new(0.024, 0.12)
        }),
        bloom: Some(Bloom {
            threshold: 0.9,
            intensity: 0.45,
            radius: 2.2,
        }),
    });
    if quality >= 1 {
        w.insert_resource(AmbientOcclusion {
            radius: 1.2,
            intensity: 0.7,
            ..AmbientOcclusion::default()
        });
    }
    // What little daylight gets through the cloud, from the last of the
    // light low in the west: enough to model the hills.
    w.spawn_named(
        "sky",
        (
            Transform::at(-60.0, 40.0, -25.0).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight {
                color: [0.55, 0.7, 0.75],
                illuminance: 1500.0,
                shadows: false,
            },
        ),
    );
}

/// A car's two headlights, cones thrown down the road ahead, and the red glow
/// of its tail lights on the wet tarmac behind. The car's +X is forward; a
/// spot shines along its own -Z.
fn headlights(w: &mut World, car: Entity) {
    let ahead = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2) * Quat::from_rotation_x(-0.2);
    for side in [-0.56f32, 0.56] {
        w.spawn((
            Transform {
                position: Vec3::new(1.85, 0.7, side),
                rotation: ahead,
                scale: Vec3::ONE,
            },
            SpotLight {
                color: [1.0, 0.86, 0.62],
                intensity: 300000.0,
                range: 32.0,
                inner: 0.22,
                outer: 0.5,
            },
            Parent(car),
            Ambient,
        ));
    }
    w.spawn((
        Transform::at(-2.3, 0.6, 0.0),
        PointLight {
            color: [1.0, 0.12, 0.08],
            intensity: 4000.0,
            range: 7.0,
        },
        Parent(car),
        Ambient,
    ));
}

/// Rain: streaks falling through a box round the camera's focus, which the
/// camera carries along (see `camera`).
fn rain(w: &mut World) {
    let rain = w.spawn_named(
        "rain",
        (
            // Upside down, so the emitter's +Y falls.
            Transform {
                position: Vec3::ZERO,
                rotation: Quat::from_rotation_x(std::f32::consts::PI),
                scale: Vec3::ONE,
            },
            Emitter {
                shape: emitter::Shape::Box(Vec3::new(30.0, 0.5, 26.0)),
                rate: 700.0,
                lifetime: 0.75,
                speed: 22.0,
                spread: 0.06,
                gravity: Vec3::ZERO,
                drag: 0.0,
                size: [0.022, 0.022],
                color: [[0.6, 0.7, 0.74, 0.16], [0.6, 0.7, 0.74, 0.08]],
                seed: 99,
                bound: [-32.0, -1.0, -28.0, 32.0, 18.0, 28.0],
                additive: true,
                running: true,
                ..Emitter::default()
            },
            ParticleLook {
                stretch: 0.04,
                ..ParticleLook::default()
            },
            emitter::WorldSpace::default(),
            Ambient,
        ),
    );
    let _ = rain;
}

/// Ease the camera's focus toward a point ahead of the player.
fn camera(w: &mut World, snap: bool) {
    let (p, v, idx) = {
        let pose = w.require::<Transform>("player");
        let car = w.require::<Car>("player");
        (
            Vec2::new(pose.position.x, pose.position.z),
            Vec2::new(car.vx, car.vz),
            car.idx as usize,
        )
    };
    // Lead along the road, and a little toward the inside of the corner coming up.
    let (turn, normal) = {
        let t = w.resource::<Track>();
        let ahead = t.wrap(idx as i64 + (20.0 / t.step) as i64);
        (t.turn_ahead(idx), t.normal(ahead))
    };
    let speed = v.length();
    let target = p + v * 0.38 + normal * (turn * 2.5);
    let ground = w.require::<Car>("player").y;
    let dt = w.dt();
    let now = w.tick() as f32 * dt;
    let (focus, shake, lift) = {
        let mut race = w.resource_mut::<Race>();
        let k = if snap { 1.0 } else { (dt * 3.0).min(1.0) };
        race.focus_x += (target.x - race.focus_x) * k;
        race.focus_z += (target.y - race.focus_z) * k;
        // Heights ease slower, so bumps and jumps don't bob the view.
        let ky = if snap { 1.0 } else { (dt * 1.5).min(1.0) };
        race.focus_y += (ground - race.focus_y) * ky;
        race.shake *= 1.0 - (dt * 7.0).min(1.0);
        // Pull back a little at speed, to see more of what is coming.
        let lift = 30.0 + 4.0 * (speed / track::VMAX).clamp(0.0, 1.0);
        let kl = if snap { 1.0 } else { (dt * 0.8).min(1.0) };
        race.lift += (lift - race.lift) * kl;
        // Aim up-screen of the car, so it sits below the middle with more road ahead.
        (
            Vec3::new(race.focus_x, race.focus_y, race.focus_z - 3.0),
            race.shake,
            race.lift,
        )
    };
    let jolt = Vec3::new(
        math::sin(now * 53.0),
        math::sin(now * 71.0 + 1.3),
        math::sin(now * 61.0 + 2.1),
    ) * shake;
    if let Some(rain) = w.named("rain") {
        w.require_mut::<Transform>(rain).position = focus + Vec3::Y * 15.0;
    }
    let mut cam = w.require_mut::<Transform>("camera");
    // A little lower and further back than straight down, so hills and jumps
    // show as depth as well as light.
    *cam = Transform::at(focus.x, focus.y + lift, focus.z + lift * 0.9).looking_at(focus, Vec3::Y);
    cam.position += jolt;
}

/// Places: finishers in order, then everyone else by distance raced.
pub fn standings(w: &World) -> Vec<(u32, Car, f32)> {
    let t = w.resource::<Track>();
    let order = w.resource::<Race>().order.clone();
    let mut cars: Vec<(u32, Car, f32)> = w
        .query::<(&Transform, &Car)>()
        .iter()
        .map(|(_, (pose, car))| {
            let body = Body {
                car: car.clone(),
                p: Vec2::new(pose.position.x, pose.position.z),
            };
            (car.id, car.clone(), body.progress(&t))
        })
        .collect();
    cars.sort_by(|a, b| {
        let fa = order.iter().position(|&id| id == a.0);
        let fb = order.iter().position(|&id| id == b.0);
        match (fa, fb) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)),
        }
    });
    cars
}

/// How far (m) a car must be past the one above it before the driver list swaps them.
const BOARD_MARGIN: f32 = 3.0;

/// Reorder the driver list from its last order, with a margin, so two cars running
/// side by side don't swap places on screen every frame. Finishers keep their order.
fn settle_board(w: &mut World) {
    let order = standings(w);
    let mut race = w.resource_mut::<Race>();
    let finished = race.order.clone();
    let progress = |id: u32| order.iter().find(|o| o.0 == id).map_or(0.0, |o| o.2);
    let running: Vec<u32> = if race.board.len() == order.len() {
        race.board.iter().copied().filter(|id| !finished.contains(id)).collect()
    } else {
        order.iter().map(|o| o.0).filter(|id| !finished.contains(id)).collect()
    };
    let mut board = [finished.clone(), running].concat();
    let mut swapped = true;
    while swapped {
        swapped = false;
        for i in finished.len()..board.len().saturating_sub(1) {
            if progress(board[i + 1]) > progress(board[i]) + BOARD_MARGIN {
                board.swap(i, i + 1);
                swapped = true;
            }
        }
    }
    race.board = board;
}

fn hex_string(rgb: u32) -> String {
    format!("#{rgb:06x}")
}

/// The centreline every few metres, as SVG points; the last repeats the first.
fn road_points(t: &Track) -> String {
    let every = (4.0 / t.step).max(1.0) as usize;
    let mut out = String::new();
    for i in (0..t.len()).step_by(every).chain([0]) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&format!("{:.1},{:.1}", t.x[i], t.z[i]));
    }
    out
}

/// Seconds it takes to cover `metres` at the circuit's average planned speed.
fn gap(t: &Track, metres: f32) -> String {
    let pace = t.speed.iter().sum::<f32>() / t.len().max(1) as f32;
    format!("{:.1}s", metres.abs() / pace.max(1.0))
}

pub fn radar(w: &World, t: &Track, order: &[(u32, Car, f32)]) -> Radar {
    let mut centre = Vec2::ZERO;
    let mut blips = Vec::new();
    for (_, (pose, car)) in w.query::<(&Transform, &Car)>().iter() {
        let p = Vec2::new(pose.position.x, pose.position.z);
        if car.id == PLAYER {
            centre = p;
        }
        let d = &DRIVERS[car.id as usize];
        blips.push(Blip {
            id: car.id,
            at: format!(
                "translate({:.1} {:.1}) rotate({:.0})",
                p.x,
                p.y,
                car.heading.to_degrees()
            ),
            fill: hex_string(d.paint),
            ring: hex_string(d.trim),
            you: car.id == PLAYER,
        });
    }
    // The player last, so it draws on top.
    blips.sort_by_key(|b| (b.you, b.id));
    let at = order.iter().position(|(id, _, _)| *id == PLAYER);
    let player = at.map(|k| &order[k]);
    let racing = player.is_some_and(|(_, car, _)| !car.finished);
    let neighbour = |k: Option<usize>| -> String {
        match (racing, player, k.and_then(|k| order.get(k))) {
            (true, Some((_, _, mine)), Some((id, car, theirs))) if !car.finished => {
                format!("{} {}", DRIVERS[*id as usize].name, gap(t, theirs - mine))
            }
            _ => String::new(),
        }
    };
    Radar {
        view: format!(
            "{:.1} {:.1} {:.0} {:.0}",
            centre.x - RADAR_REACH,
            centre.y - RADAR_REACH,
            RADAR_REACH * 2.0,
            RADAR_REACH * 2.0
        ),
        road: road_points(t),
        blips,
        ahead: neighbour(at.and_then(|k| k.checked_sub(1))),
        behind: neighbour(at.map(|k| k + 1)),
    }
}

fn hud(w: &World) {
    let hz = w.hz();
    let now = w.tick();
    let race = w.resource::<Race>().clone();
    let t = w.resource::<Track>();
    let name = track::layout(t.id).name.to_string();
    let order = standings(w);
    let radar = radar(w, &t, &order);
    drop(t);
    let order: Vec<_> = if race.board.len() == order.len() {
        race.board
            .iter()
            .filter_map(|id| order.iter().find(|o| o.0 == *id).cloned())
            .collect()
    } else {
        order
    };
    let player = w.require::<Car>("player").clone();
    let place = order
        .iter()
        .position(|(id, _, _)| *id == PLAYER)
        .unwrap_or(0) as u32
        + 1;
    let phase = if now < race.start {
        "grid"
    } else if player.finished {
        "done"
    } else {
        "race"
    };
    let leader_finish = race.order.first().map(|id| {
        order
            .iter()
            .find(|(i, _, _)| i == id)
            .map(|(_, c, _)| c.finish_tick)
            .unwrap_or(0)
    });
    let rows = order
        .iter()
        .enumerate()
        .map(|(k, (id, car, _))| Row {
            id: *id,
            place: k as u32 + 1,
            name: DRIVERS[*id as usize].name.into(),
            color: hex_string(DRIVERS[*id as usize].paint),
            time: if car.finished {
                match (k, leader_finish) {
                    (0, _) | (_, None) => clock(car.finish_tick - race.start, hz),
                    (_, Some(lead)) => format!("+{}", clock(car.finish_tick - lead, hz)),
                }
            } else {
                format!("Lap {}", car.crossings.clamp(1, race.laps))
            },
            you: *id == PLAYER,
        })
        .collect();
    let elapsed = if now < race.start {
        0
    } else if player.finished {
        player.finish_tick - race.start
    } else {
        now - race.start
    };
    let speed = Vec2::new(player.vx, player.vz).length();
    let since_go = now.saturating_sub(race.start);
    w.publish_record(&Hud {
        phase: phase.into(),
        count: if now < race.start {
            (race.start - now).div_ceil(hz as u64) as u32
        } else {
            0
        },
        go: now >= race.start && since_go < hz as u64,
        track: name,
        lap: player.crossings.clamp(1, race.laps),
        laps: race.laps,
        place,
        cars: DRIVERS.len() as u32,
        time: clock(elapsed, hz),
        best: if player.best_lap > 0 {
            clock(player.best_lap, hz)
        } else {
            "–".into()
        },
        last: if player.last_lap > 0 {
            clock(player.last_lap, hz)
        } else {
            "–".into()
        },
        speed: (speed * 3.6).round() as u32,
        gas: player.gas > 0.0,
        brake: player.brake,
        air: player.air && player.air_ticks >= car::JUMP_TICKS,
        draft: player.draft > 0.45 && speed > 12.0,
        off: player.surface == car::GRASS,
        standings: rows,
        radar,
    });
}
