use crate::{GameRules, HelmOrders, OwnShipStatus, Vec2, wrap_angle};

/// One player's submarine.
#[derive(Clone, Debug, PartialEq)]
pub struct Submarine {
    pub position: Vec2,
    /// Radians, counter-clockwise from east.
    pub heading: f32,
    /// Metres/second; negative means going astern.
    pub speed: f32,
    /// The orders the simulation will act on next step.
    pub orders: HelmOrders,
    pub torpedo_cooldown: f32,
    pub ping_cooldown: f32,
    // Short bursts of extra noise (from pinging or firing). Private: other code
    // asks for `noise()` rather than poking at these directly.
    transient_noise: f32,
    transient_noise_timer: f32,
}

impl Submarine {
    pub fn new(position: Vec2, heading: f32) -> Self {
        Self {
            position,
            heading,
            speed: 0.0,
            orders: HelmOrders::default(),
            torpedo_cooldown: 0.0,
            ping_cooldown: 0.0,
            transient_noise: 0.0,
            transient_noise_timer: 0.0,
        }
    }

    /// The speed the current throttle setting is asking for.
    pub fn target_speed(&self, rules: &GameRules) -> f32 {
        let throttle = self.orders.throttle;
        if throttle >= 0.0 { throttle * rules.max_forward_speed } else { throttle * rules.max_reverse_speed }
    }

    /// 0..1, how fast we're going compared with top speed.
    pub fn speed_fraction(&self, rules: &GameRules) -> f32 {
        (self.speed.abs() / rules.max_forward_speed).clamp(0.0, 1.0)
    }

    /// How loud we are. Faster = louder. Pinging/firing = much louder for a while.
    pub fn noise(&self, rules: &GameRules) -> f32 {
        let transient = if self.transient_noise_timer > 0.0 { self.transient_noise } else { 0.0 };
        rules.base_noise + self.speed_fraction(rules) + transient
    }

    /// 0..1 multiplier on how far we can hear. Going fast makes you deaf.
    pub fn hearing_factor(&self, rules: &GameRules) -> f32 {
        1.0 - rules.flow_noise_deafness * self.speed_fraction(rules)
    }

    /// Moves the submarine forward in time by `dt` seconds.
    ///
    /// Rust note: `&mut self` means "this method changes the submarine".
    /// `&GameRules` means "I only need to read the rules, I won't change them".
    pub fn integrate(&mut self, dt: f32, rules: &GameRules) {
        // Accelerate towards the target speed, but no faster than `acceleration`.
        let max_change = rules.acceleration * dt;
        self.speed += (self.target_speed(rules) - self.speed).clamp(-max_change, max_change);

        // Turn. Rudders need water flowing past them, so we turn faster at speed.
        let steering = rules.min_turn_factor + (1.0 - rules.min_turn_factor) * self.speed_fraction(rules);
        self.heading = wrap_angle(self.heading + self.orders.rudder * rules.max_turn_rate * steering * dt);

        // Move, staying inside the play area.
        let velocity = Vec2::from_angle(self.heading) * self.speed;
        self.position = (self.position + velocity * dt).clamp_to_square(rules.world_half_extent);

        self.torpedo_cooldown = (self.torpedo_cooldown - dt).max(0.0);
        self.ping_cooldown = (self.ping_cooldown - dt).max(0.0);
        self.transient_noise_timer = (self.transient_noise_timer - dt).max(0.0);
    }

    /// If a torpedo was ordered and is ready, consumes the order and returns
    /// where the torpedo should start and which way it should go.
    pub(crate) fn take_torpedo_launch(&mut self, rules: &GameRules) -> Option<(Vec2, f32)> {
        // Rust note: `std::mem::take` returns the value and leaves the default
        // (`false`) behind, so the order is consumed whether or not we fire.
        if !std::mem::take(&mut self.orders.fire_torpedo) || self.torpedo_cooldown > 0.0 {
            return None;
        }
        self.torpedo_cooldown = rules.torpedo_cooldown;
        self.make_noise(rules.launch_noise, rules.launch_noise_duration);
        let start = self.position + Vec2::from_angle(self.heading) * rules.torpedo_launch_offset;
        Some((start, self.heading))
    }

    /// If a ping was ordered and is ready, consumes the order and returns `true`.
    pub(crate) fn take_sonar_ping(&mut self, rules: &GameRules) -> bool {
        if !std::mem::take(&mut self.orders.sonar_ping) || self.ping_cooldown > 0.0 {
            return false;
        }
        self.ping_cooldown = rules.ping_cooldown;
        self.make_noise(rules.ping_noise, rules.ping_noise_duration);
        true
    }

    fn make_noise(&mut self, level: f32, duration: f32) {
        let current = if self.transient_noise_timer > 0.0 { self.transient_noise } else { 0.0 };
        self.transient_noise = current.max(level);
        self.transient_noise_timer = self.transient_noise_timer.max(duration);
    }

    /// What the owner of this submarine is allowed to know about it.
    pub fn status(&self, rules: &GameRules) -> OwnShipStatus {
        OwnShipStatus {
            position: self.position,
            heading: self.heading,
            speed: self.speed,
            throttle: self.orders.throttle,
            noise: self.noise(rules),
            torpedo_cooldown: self.torpedo_cooldown,
            ping_cooldown: self.ping_cooldown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accelerates_gradually_towards_throttle() {
        let rules = GameRules::default();
        let mut sub = Submarine::new(Vec2::ZERO, 0.0);
        sub.orders.throttle = 1.0;
        sub.integrate(1.0, &rules);
        assert_eq!(sub.speed, rules.acceleration); // limited by acceleration
        for _ in 0..20 {
            sub.integrate(1.0, &rules);
        }
        assert_eq!(sub.speed, rules.max_forward_speed);
        assert!(sub.position.x > 0.0, "heading 0 means moving east");
    }

    #[test]
    fn cannot_leave_the_world() {
        let rules = GameRules::default();
        let mut sub = Submarine::new(Vec2::new(rules.world_half_extent - 1.0, 0.0), 0.0);
        sub.speed = rules.max_forward_speed;
        sub.orders.throttle = 1.0;
        sub.integrate(10.0, &rules);
        assert_eq!(sub.position.x, rules.world_half_extent);
    }

    #[test]
    fn going_fast_is_loud_and_deafening() {
        let rules = GameRules::default();
        let quiet = Submarine::new(Vec2::ZERO, 0.0);
        let mut fast = quiet.clone();
        fast.speed = rules.max_forward_speed;
        assert!(fast.noise(&rules) > quiet.noise(&rules));
        assert!(fast.hearing_factor(&rules) < quiet.hearing_factor(&rules));
    }

    #[test]
    fn ping_has_a_cooldown() {
        let rules = GameRules::default();
        let mut sub = Submarine::new(Vec2::ZERO, 0.0);
        sub.orders.sonar_ping = true;
        assert!(sub.take_sonar_ping(&rules));
        sub.orders.sonar_ping = true;
        assert!(!sub.take_sonar_ping(&rules), "still cooling down");
    }
}
