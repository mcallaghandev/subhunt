//! Deciding when to play sounds.
//!
//! The client never gets an "a torpedo was fired" message. Instead it notices
//! changes in what it knows: if the torpedo reload timer suddenly jumped up,
//! we must have just fired. Working from the reload timers (which stay high
//! for seconds) rather than one-off messages means a sound can't be missed if
//! two updates arrive in the same frame.

use subhunt_domain::{OwnShipStatus, Vec2};

/// Speed of sound in sea water, in metres per second.
const SPEED_OF_SOUND: f32 = 1500.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    PingSent,
    TorpedoLaunched,
    /// A ping echo, arriving after `delay` seconds, at `loudness` from 0 to 1.
    Echo {
        delay: f32,
        loudness: f32,
    },
}

/// Sounds caused by our own boat, found by comparing two moments.
pub fn own_ship_cues(before: &OwnShipStatus, now: &OwnShipStatus) -> Vec<Cue> {
    let mut cues = Vec::new();
    // A reload timer only ever counts down, so a jump up means we just used it.
    if now.ping_cooldown > before.ping_cooldown + 0.5 {
        cues.push(Cue::PingSent);
    }
    if now.torpedo_cooldown > before.torpedo_cooldown + 0.5 {
        cues.push(Cue::TorpedoLaunched);
    }
    cues
}

/// The sound of a ping bouncing back off something at `echo_position`.
///
/// The ping travels there and back, so the echo arrives after twice the
/// distance at the speed of sound. Distant echoes are quieter.
pub fn echo_cue(me: Vec2, echo_position: Vec2, range: f32) -> Cue {
    let distance = me.distance(echo_position);
    Cue::Echo { delay: 2.0 * distance / SPEED_OF_SOUND, loudness: (1.0 - distance / range.max(1.0)).clamp(0.25, 1.0) }
}

/// How loud and how fast (higher pitched) the engine should sound, for a
/// speed from 0 (stopped) to 1 (flat out).
pub fn engine_mix(speed_fraction: f32) -> (f32, f32) {
    let speed = speed_fraction.clamp(0.0, 1.0);
    (0.05 + 0.4 * speed, 0.75 + 0.6 * speed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ship(ping_cooldown: f32, torpedo_cooldown: f32) -> OwnShipStatus {
        OwnShipStatus {
            position: Vec2::ZERO,
            heading: 0.0,
            speed: 0.0,
            throttle: 0.0,
            noise: 0.2,
            torpedo_cooldown,
            ping_cooldown,
        }
    }

    #[test]
    fn using_something_restarts_its_timer() {
        assert_eq!(own_ship_cues(&ship(0.0, 0.0), &ship(8.0, 0.0)), vec![Cue::PingSent]);
        assert_eq!(own_ship_cues(&ship(0.0, 1.0), &ship(0.0, 4.0)), vec![Cue::TorpedoLaunched]);
        // Timers counting down normally make no sound.
        assert!(own_ship_cues(&ship(5.0, 3.0), &ship(4.95, 2.95)).is_empty());
    }

    #[test]
    fn far_echoes_come_back_later_and_quieter() {
        let near = echo_cue(Vec2::ZERO, Vec2::new(150.0, 0.0), 1500.0);
        let far = echo_cue(Vec2::ZERO, Vec2::new(1200.0, 0.0), 1500.0);
        let (Cue::Echo { delay: near_delay, loudness: near_loud }, Cue::Echo { delay: far_delay, loudness: far_loud }) =
            (near, far)
        else {
            panic!("expected echoes");
        };
        assert!((near_delay - 0.2).abs() < 1e-5); // 300 m round trip at 1500 m/s
        assert!(far_delay > near_delay && far_loud < near_loud);
    }

    #[test]
    fn engine_is_louder_and_higher_at_speed() {
        let (quiet, low) = engine_mix(0.0);
        let (loud, high) = engine_mix(1.0);
        assert!(loud > quiet && high > low);
    }
}
