//! Sonar: how submarines find each other without seeing each other.
//!
//! * **Passive sonar** (listening) tells you the *direction* of something
//!   noisy, but not how far away it is. Loud things are heard from further away.
//! * **Active sonar** (pinging) tells you exactly *where* things are, but the
//!   ping is very loud, so everyone nearby now hears you too.

use crate::{GameRules, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactKind {
    Submarine,
    Torpedo,
}

/// Something your sonar has picked up.
///
/// Rust note: an `enum` in Rust can carry different data in each variant.
/// You read it with `match`, and the compiler makes sure you handle every case.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Contact {
    /// Passive sonar: "there's something over *there*".
    Bearing {
        /// World angle from the listener towards the noise, in radians.
        bearing: f32,
        kind: ContactKind,
        /// 0..1, how clearly it's heard (1 = very close for its loudness).
        strength: f32,
    },
    /// Active sonar echo: an exact position at the moment of the ping.
    Fix { position: Vec2, kind: ContactKind },
}

/// Can a listener hear a noise? Returns `Some(strength)` if so, `None` if not.
///
/// Rust note: `Option<T>` is Rust's replacement for null. It's either `Some(value)`
/// or `None`, and you can't use the value without checking which.
pub fn passive_strength(
    listener: Vec2,
    hearing_factor: f32,
    target: Vec2,
    target_noise: f32,
    rules: &GameRules,
) -> Option<f32> {
    let range = target_noise * rules.passive_range_per_noise * hearing_factor;
    if range <= 0.0 {
        return None;
    }
    let distance = listener.distance(target);
    if distance > range {
        return None;
    }
    Some((1.0 - distance / range).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn louder_targets_are_heard_further_away() {
        let rules = GameRules::default();
        let listener = Vec2::ZERO;
        let target = Vec2::new(500.0, 0.0);
        assert!(passive_strength(listener, 1.0, target, 0.2, &rules).is_none());
        assert!(passive_strength(listener, 1.0, target, 1.2, &rules).is_some());
    }
}
