//! The player's one control: hold anything for gas, let go to brake. The car
//! picks its own side to pass on.
use exact_game::{Game, InputEvent, PointerPhase, Sim};
use kerb_racers_logic::car::{self, Car};
use kerb_racers_logic::track::Track;
use kerb_racers_logic::{KerbRacers, Options, Race};

const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 600.0;

/// A race on Pine Hollow, its clock in host milliseconds kept beside it so
/// pointer events can be stamped as a host would.
struct Host {
    sim: Sim<KerbRacers>,
    ms: f64,
}

impl Host {
    fn new() -> Self {
        let mut sim = Sim::<KerbRacers>::new(Options::default()).unwrap();
        let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/");
        if let Err(e) = sim.load_assets(|name| std::fs::read(format!("{assets}{name}"))) {
            let unexpected =
                e.to_string().contains("\"Pending\"") || e.to_string().contains("\"Failed\"");
            assert!(!unexpected, "{e}");
        }
        sim.viewport(WIDTH, HEIGHT);
        Self { sim, ms: 0.0 }
    }
    fn run(&mut self, ms: f64) {
        self.sim.run(ms);
        self.ms += ms;
    }
    fn to_the_lights(&mut self) {
        while self.sim.world().tick() <= self.sim.world().resource::<Race>().start {
            self.run(100.0);
        }
    }
    /// A pointer at `x` across the canvas: a mouse when `buttons` is set,
    /// a finger when it is 0.
    fn pointer(&mut self, phase: PointerPhase, x: f32, buttons: u32) {
        self.sim.input(InputEvent::Pointer {
            id: 1,
            phase,
            x,
            y: HEIGHT / 2.0,
            dx: 0.0,
            dy: 0.0,
            buttons: if phase == PointerPhase::Up { 0 } else { buttons },
            at_ms: self.ms,
            player: 0,
        });
    }
    fn player(&self) -> Car {
        self.sim.world().require::<Car>("player").clone()
    }
}

fn assert_gas(car: &Car, how: &str) {
    assert!(car.gas > 0.0 && !car.brake, "{how}: gas {} brake {}", car.gas, car.brake);
}

fn assert_brake(car: &Car, how: &str) {
    assert!(car.gas == 0.0 && car.brake, "{how}: gas {} brake {}", car.gas, car.brake);
}

#[test]
fn every_gas_key_drives_and_letting_go_brakes() {
    let mut game = Host::new();
    game.to_the_lights();
    for key in ["KeyW", "ArrowUp", "Space"] {
        game.sim.key_down(key);
        game.run(200.0);
        assert_gas(&game.player(), key);
        game.sim.key_up(key);
        game.run(100.0);
        assert_brake(&game.player(), &format!("{key} let go"));
    }
}

#[test]
fn either_mouse_button_drives_and_letting_go_brakes() {
    let mut game = Host::new();
    game.to_the_lights();
    for (button, bits) in [("MouseLeft", 1), ("MouseRight", 2)] {
        game.pointer(PointerPhase::Down, WIDTH / 2.0, bits);
        game.run(200.0);
        assert_gas(&game.player(), button);
        game.pointer(PointerPhase::Up, WIDTH / 2.0, bits);
        game.run(100.0);
        assert_brake(&game.player(), &format!("{button} let go"));
    }
}

#[test]
fn a_finger_anywhere_drives_and_lifting_it_brakes() {
    let mut game = Host::new();
    game.to_the_lights();
    for x in [WIDTH * 0.1, WIDTH * 0.5, WIDTH * 0.9] {
        game.pointer(PointerPhase::Down, x, 0);
        game.run(200.0);
        assert_gas(&game.player(), &format!("finger at x {x}"));
        game.pointer(PointerPhase::Up, x, 0);
        game.run(100.0);
        assert_brake(&game.player(), &format!("finger at x {x} lifted"));
    }
}

#[test]
fn no_input_brakes_in_the_race() {
    let mut game = Host::new();
    game.to_the_lights();
    game.run(500.0);
    assert_brake(&game.player(), "no input in the race");
}

#[test]
fn no_input_brakes_on_the_grid() {
    let mut game = Host::new();
    game.run(500.0);
    assert!(game.sim.world().tick() < game.sim.world().resource::<Race>().start);
    assert_brake(&game.player(), "no input on the grid");
    game.sim.key_down("KeyW");
    game.run(200.0);
    assert_gas(&game.player(), "a blip on the grid");
}

#[test]
fn letting_go_brakes_gently_without_locking_the_wheels() {
    let mut game = Host::new();
    game.to_the_lights();
    game.sim.key_down("KeyW");
    let speed = |car: &Car| (car.vx * car.vx + car.vz * car.vz).sqrt();
    // Let go at speed with a straight ahead, so a slide in a corner does not
    // add its scrub to the lift.
    let straight_ahead = |game: &Host| {
        let t = game.sim.world().resource::<Track>();
        let car = game.player();
        (0..15).all(|k| t.bend[t.wrap(car.idx as i64 + k)].abs() < 0.02)
    };
    while speed(&game.player()) < 15.0 || game.player().sliding || !straight_ahead(&game) {
        assert!(game.ms < 20_000.0, "no straight to lift on");
        game.run(1000.0 / 120.0);
    }
    let flat_out = speed(&game.player());
    game.sim.key_up("KeyW");
    for _ in 0..5 {
        game.run(100.0);
        assert!(!game.player().locked, "the wheels lock on a lift");
        assert!(!game.player().sliding, "the car slides during the lift");
    }
    let shed = (flat_out - speed(&game.player())) / 0.5;
    println!("a lift from {flat_out:.1} m/s sheds {shed:.1} m/s²");
    assert!(
        shed > 2.0 && shed < car::BRAKE * 0.5,
        "a lift sheds {shed} m/s², a full brake {}",
        car::BRAKE
    );
}

#[test]
fn gas_is_the_only_control() {
    let actions = KerbRacers::actions();
    let mut keys: Vec<(&str, &str)> = actions.keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            ("ArrowUp", "gas"),
            ("KeyW", "gas"),
            ("MouseLeft", "gas"),
            ("MouseRight", "gas"),
            ("Space", "gas"),
        ]
    );
    // The touch stick binds no key, so look for it by name.
    let declared = format!("{actions:?}");
    for gone in ["brake", "left", "right", "swipe"] {
        assert!(!declared.contains(&format!("\"{gone}\"")), "`{gone}` is still declared: {declared}");
    }
}
