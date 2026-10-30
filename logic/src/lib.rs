//! Kerb Racers: a short top-down circuit race. You hold the pedals (right
//! click for gas, left click to brake); the car steers itself along the
//! racing line, and the tyres decide whether it makes the corner.
pub mod car;
pub mod scenery;
pub mod track;

use car::{Body, Car};
use exact_game::*;
use track::Track;

#[derive(Args)]
pub struct Options {
    pub seed: u64,
    /// Which circuit: 0 Pine Hollow, 1 Fern Loop, 2 Kerb Ring.
    pub track: u32,
    pub laps: u32,
    /// Graphics: 0 low (no shadows, fewer props), 1 medium (sun shadows), 2 high (haze and bloom too).
    pub quality: u32,
    #[live]
    pub paused: bool,
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
            paused: false,
            restart: false,
        }
    }
}

/// Seconds of countdown before the lights go out.
pub const COUNTDOWN: f32 = 3.0;

pub struct Driver {
    pub name: &'static str,
    pub paint: u32,
    pub trim: u32,
    pub skill: f32,
}

/// Grid order, front to back. The player starts fifth.
pub const DRIVERS: [Driver; 6] = [
    Driver {
        name: "Juno",
        paint: 0x2a2c33,
        trim: 0xf2c230,
        skill: 0.925,
    },
    Driver {
        name: "Mika",
        paint: 0x8b4fd8,
        trim: 0xf1efe8,
        skill: 0.915,
    },
    Driver {
        name: "Rook",
        paint: 0xf1efe8,
        trim: 0xe8742a,
        skill: 0.9,
    },
    Driver {
        name: "Ayla",
        paint: 0x3fa7d6,
        trim: 0xf1efe8,
        skill: 0.885,
    },
    Driver {
        name: "You",
        paint: 0xf2c230,
        trim: 0x22252b,
        skill: 0.9,
    },
    Driver {
        name: "Vex",
        paint: 0xf1efe8,
        trim: 0xd83a2e,
        skill: 0.87,
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
    /// Where the camera looks, eased toward the player's path.
    pub focus_x: f32,
    pub focus_z: f32,
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
    pub sliding: bool,
    pub off: bool,
    pub standings: Vec<Row>,
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

pub struct KerbRacers;
impl Game for KerbRacers {
    const ID: &'static str = "kerb-racers";
    const HZ: u32 = 120;
    /// The Blender models and ground textures: shown as they land, never read by the simulation.
    const STREAMED: &'static [&'static str] = &[
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
    ];
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .button("gas", &["MouseRight", "ArrowUp", "KeyW"])
            .button_touch("gas", Region::Right)
            .button("brake", &["MouseLeft", "ArrowDown", "KeyS", "Space"])
            .button_touch("brake", Region::Left)
    }
    fn validate(args: &Options) -> Result<(), String> {
        if args.quality > 2 {
            return Err(format!("quality is 0, 1 or 2, not {}", args.quality));
        }
        if args.laps == 0 || args.laps > 20 {
            return Err(format!("laps must be 1 to 20, not {}", args.laps));
        }
        Ok(())
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        let t = track::build(args.track % track::LAYOUTS.len() as u32);
        light(w, args.quality);
        let id = t.id;
        let ground = w
            .generated_model(&format!("ground-{id}.model"), scenery::ground(&t))
            .expect("ground");
        w.spawn_named("ground", (Transform::default(), ground));
        let road = w
            .generated_model(&format!("road-{id}.model"), scenery::road(&t))
            .expect("road");
        w.spawn_named("road", (Transform::default(), road));
        let flowers = scenery::flowers(w, &t);
        let flowers = w
            .generated(&format!("flowers-{id}.model"), flowers)
            .expect("flowers");
        w.spawn_named("flowers", (Transform::default(), flowers, Ambient));
        for prop in scenery::props(w, &t, args.quality) {
            let e = w.spawn((
                Transform {
                    position: Vec3::new(prop.at.x, 0.0, prop.at.y),
                    rotation: Quat::from_rotation_y(-prop.yaw),
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
            let e = w.spawn_named(
                name_of(k),
                (
                    Transform {
                        position: Vec3::new(p.x, 0.0, p.y),
                        rotation: Quat::from_rotation_y(-heading),
                        scale: Vec3::ONE,
                    },
                    model,
                    Car {
                        id: k,
                        ai: k != PLAYER,
                        skill: d.skill,
                        heading,
                        idx: idx as u32,
                        side,
                        ..Car::default()
                    },
                ),
            );
            puffs(w, e, &name_of(k));
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
        let start = t.at(t.wrap(-12), 0.0);
        w.insert_resource(Race {
            laps: args.laps,
            start: w.tick() + (COUNTDOWN * Self::HZ as f32) as u64,
            order: Vec::new(),
            focus_x: start.x,
            focus_z: start.y,
        });
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
                near < crown.radius && y < crown.height
            });
            if hidden {
                fade.push(e);
            }
        });
        for e in fade {
            p.insert(e, Opacity(0.28));
        }
    }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        let now = w.tick();
        let (start, laps) = {
            let r = w.resource::<Race>();
            (r.start, r.laps)
        };
        if now >= start {
            race(w, input, now, start, laps);
        } else {
            let mut player = w.require_mut::<Car>("player");
            player.gas = if input.held("gas") { 1.0 } else { 0.0 };
            player.brake = input.held("brake");
        }
        camera(w, false);
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
    let caps = car::plan(&t, &mut list, dt);
    for (k, body) in list.iter_mut().enumerate() {
        if body.car.ai || body.car.finished {
            if !body.car.ai {
                // The player's car cruises home once the flag has fallen.
                body.car.skill = 0.85;
            }
            car::drive(&t, body, caps[k]);
        } else {
            body.car.brake = input.held("brake");
            body.car.gas = if input.held("gas") && !body.car.brake {
                1.0
            } else {
                0.0
            };
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
    car::collide(&mut list);
    turn_wheels(w, &entities, &list);
    for body in list.iter_mut() {
        car::locate(&t, body);
    }
    for (e, body) in entities.iter().zip(list) {
        let mut pose = w.require_mut::<Transform>(*e);
        pose.position = Vec3::new(body.p.x, 0.0, body.p.y);
        pose.rotation = Quat::from_rotation_y(-body.car.heading);
        drop(pose);
        let speed = Vec2::new(body.car.vx, body.car.vz).length();
        let name = name_of(body.car.id);
        let smoke = body.car.sliding && body.car.surface != car::GRASS && speed > 5.0;
        let dust = body.car.surface != car::ASPHALT && speed > 3.0;
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
        // Read first: a mutable borrow marks the row written even when nothing changes.
        if w.require::<Transform>(e).rotation != rotation {
            w.require_mut::<Transform>(e).rotation = rotation;
        }
    }
}

/// Two trails behind each car: white tyre smoke for a slide on the road,
/// brown dust off it. Both stay where they were born and start switched off.
fn puffs(w: &mut World, car: Entity, name: &str) {
    for (suffix, birth, death, size) in [
        (
            "smoke",
            [0.9, 0.9, 0.88, 0.26],
            [0.85, 0.85, 0.85, 0.0],
            [0.45, 1.5],
        ),
        (
            "dust",
            [0.62, 0.45, 0.28, 0.42],
            [0.66, 0.55, 0.4, 0.0],
            [0.6, 2.2],
        ),
    ] {
        w.spawn_named(
            format!("{name}-{suffix}"),
            (
                Transform::at(-1.5, 0.25, 0.0),
                Emitter {
                    shape: emitter::Shape::Sphere(0.35),
                    rate: 18.0,
                    lifetime: 0.85,
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

/// A low, late-afternoon sun from the west: long shadows across the road,
/// warm light on everything facing it, cool sky in the shade, a golden haze.
fn light(w: &mut World, quality: u32) {
    w.insert_resource(Environment {
        background: Some([0.42, 0.36, 0.22]),
        zenith: [0.22, 0.34, 0.55],
        horizon: [0.95, 0.72, 0.45],
        ground: [0.10, 0.09, 0.05],
        // Moderate ambient and a stronger sky fill: soft shade, not black woods.
        ambient: 0.5,
        sun_disc: 0.0,
        exposure: 0.9,
        // The golden haze and bloom are full-screen passes: high quality only.
        fog: (quality >= 2).then_some(Fog {
            color: Some([0.95, 0.74, 0.48]),
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
                shadows: quality >= 1,
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

/// Ease the camera's focus toward a point ahead of the player.
fn camera(w: &mut World, snap: bool) {
    let (p, v) = {
        let pose = w.require::<Transform>("player");
        let car = w.require::<Car>("player");
        (
            Vec2::new(pose.position.x, pose.position.z),
            Vec2::new(car.vx, car.vz),
        )
    };
    let target = p + v * 0.35;
    let dt = w.dt();
    let focus = {
        let mut race = w.resource_mut::<Race>();
        let k = if snap { 1.0 } else { (dt * 3.0).min(1.0) };
        race.focus_x += (target.x - race.focus_x) * k;
        race.focus_z += (target.y - race.focus_z) * k;
        Vec3::new(race.focus_x, 0.0, race.focus_z)
    };
    let mut cam = w.require_mut::<Transform>("camera");
    *cam = Transform::at(focus.x, 33.0, focus.z + 24.0).looking_at(focus, Vec3::Y);
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

fn hex_string(rgb: u32) -> String {
    format!("#{rgb:06x}")
}

fn hud(w: &World) {
    let hz = w.hz();
    let now = w.tick();
    let race = w.resource::<Race>().clone();
    let t = w.resource::<Track>();
    let name = track::layout(t.id).name.to_string();
    drop(t);
    let order = standings(w);
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
        sliding: player.sliding,
        off: player.surface == car::GRASS,
        standings: rows,
    });
}
