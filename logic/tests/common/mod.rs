use exact_game::{Entity, Sim};
use kerb_racers_logic::car::Car;
use kerb_racers_logic::KerbRacers;

/// Take the rivals off the road: their cars stay where setup put them, but
/// nothing drives them or runs into them.
pub fn without_rivals(game: &mut Sim<KerbRacers>) {
    let w = game.world_mut();
    let rivals: Vec<Entity> = w.query::<&Car>().iter().filter(|(_, c)| c.ai).map(|(e, _)| e).collect();
    assert!(!rivals.is_empty(), "no rivals on the grid");
    for e in rivals {
        w.remove::<Car>(e);
    }
}
