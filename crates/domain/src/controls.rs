/// What a player is asking their submarine to do.
///
/// This is the *only* thing a client is allowed to send to the server. The
/// server decides what actually happens: clients never say "I am at (x, y)",
/// they say "full ahead, hard left". That's what stops cheating.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HelmOrders {
    /// -1.0 (full astern) to 1.0 (full ahead).
    pub throttle: f32,
    /// -1.0 (hard right / clockwise) to 1.0 (hard left / counter-clockwise).
    pub rudder: f32,
    /// One-shot request to fire a torpedo.
    pub fire_torpedo: bool,
    /// One-shot request to send an active sonar ping.
    pub sonar_ping: bool,
}

impl HelmOrders {
    /// Never trust input from the network: clamp values into range and replace
    /// NaN/infinity (which a hacked client could send) with zero.
    pub fn sanitised(self) -> Self {
        Self {
            throttle: clean_axis(self.throttle),
            rudder: clean_axis(self.rudder),
            ..self // Rust note: "copy every other field from `self`"
        }
    }

    /// Combines these (older, not yet acted on) orders with newer ones.
    ///
    /// Throttle and rudder take the newest value, but a fire or ping request is
    /// kept until the simulation consumes it. Without this, a "fire!" message
    /// followed quickly by a "don't fire" message could be lost entirely if both
    /// arrived between two server ticks.
    pub fn superseded_by(self, newer: Self) -> Self {
        Self {
            throttle: newer.throttle,
            rudder: newer.rudder,
            fire_torpedo: self.fire_torpedo || newer.fire_torpedo,
            sonar_ping: self.sonar_ping || newer.sonar_ping,
        }
    }
}

fn clean_axis(value: f32) -> f32 {
    if value.is_finite() { value.clamp(-1.0, 1.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitising_clamps_and_removes_nan() {
        let orders = HelmOrders { throttle: 5.0, rudder: f32::NAN, ..Default::default() };
        let clean = orders.sanitised();
        assert_eq!(clean.throttle, 1.0);
        assert_eq!(clean.rudder, 0.0);
    }

    #[test]
    fn one_shot_requests_survive_until_consumed() {
        let fire = HelmOrders { fire_torpedo: true, ..Default::default() };
        let later = HelmOrders { throttle: 0.5, ..Default::default() };
        let merged = fire.superseded_by(later);
        assert!(merged.fire_torpedo);
        assert_eq!(merged.throttle, 0.5);
    }
}
