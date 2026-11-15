//! Brake lamps hold a moment after the brake lets go, so a rival riding its
//! target speed (brake on, off, on, tick by tick) shows a steady lamp.

use kerb_racers_logic::fx::{brake_lamp, LAMP_HOLD};

const DT: f32 = 1.0 / 60.0;

#[test]
fn a_flickering_brake_shows_a_steady_lamp() {
    let mut hold = 0.0;
    let lit: Vec<bool> = (0..60)
        .map(|i| brake_lamp(&mut hold, i % 3 == 0, DT))
        .collect();
    assert!(lit.iter().all(|&on| on), "lamp flickered: {lit:?}");
}

#[test]
fn the_lamp_goes_out_after_the_hold() {
    let mut hold = 0.0;
    assert!(brake_lamp(&mut hold, true, DT));
    let ticks = (LAMP_HOLD / DT).ceil() as usize + 1;
    let after: Vec<bool> = (0..ticks).map(|_| brake_lamp(&mut hold, false, DT)).collect();
    assert!(after[0], "lamp went out at once");
    assert!(!after[ticks - 1], "lamp stayed lit past the hold");
}

#[test]
fn no_brake_no_lamp() {
    let mut hold = 0.0;
    assert!((0..30).all(|_| !brake_lamp(&mut hold, false, DT)));
}
