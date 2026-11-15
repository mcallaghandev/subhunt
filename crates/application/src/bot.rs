//! A computer-controlled captain for practice games.
//!
//! The bot plays by the same rules as a human: each tick it gets *only* its
//! own `SensorPicture` and answers with `HelmOrders`. It never peeks at the
//! game world, so it can't cheat. It finds you the same way you find it: by
//! listening for noise and pinging.

use std::f32::consts::PI;

use subhunt_domain::{Contact, ContactKind, HelmOrders, SensorPicture, Vec2, wrap_angle};

/// How long an echo is trusted before the bot considers it stale.
const TARGET_MEMORY: f32 = 10.0;
/// The bot only fires at fresh echoes, and only when lined up this closely (radians).
const FIRING_ARC: f32 = 0.12;
const FIRING_RANGE: f32 = 1000.0;

pub struct ComputerCaptain {
    rng: u64,
    target: Option<Target>,
    evade_heading: f32,
    evade_timer: f32,
    ping_timer: f32,
    wander_rudder: f32,
    wander_timer: f32,
}

/// Where we last saw an enemy, and how long ago.
struct Target {
    position: Vec2,
    age: f32,
}

impl ComputerCaptain {
    pub fn new(seed: u64) -> Self {
        let mut captain = Self {
            rng: seed | 1,
            target: None,
            evade_heading: 0.0,
            evade_timer: 0.0,
            ping_timer: 0.0,
            wander_rudder: 0.0,
            wander_timer: 0.0,
        };
        // Give a new player some quiet time before the first ping.
        captain.ping_timer = 20.0 + 15.0 * captain.random();
        captain
    }

    /// Looks at the sonar picture and decides what to do next.
    pub fn decide(&mut self, picture: &SensorPicture, dt: f32, world_half_extent: f32) -> HelmOrders {
        let ship = &picture.own_ship;
        self.remember_echoes(picture, dt);
        self.ping_timer -= dt;
        self.evade_timer -= dt;
        self.wander_timer -= dt;

        // Something we can hear coming at us? Turn side-on and run.
        let incoming = strongest_bearing(picture, ContactKind::Torpedo).filter(|(_, strength)| *strength > 0.2);
        if let Some((bearing, _)) = incoming {
            self.evade_timer = 4.0;
            self.evade_heading = bearing + PI / 2.0;
        }

        let mut orders = HelmOrders::default();
        let edge = world_half_extent * 0.85;

        if self.evade_timer > 0.0 {
            orders.throttle = 1.0;
            orders.rudder = steer(ship.heading, self.evade_heading);
        } else if ship.position.x.abs() > edge || ship.position.y.abs() > edge {
            // Too close to the edge of the map: head back towards the middle.
            orders.throttle = 0.5;
            orders.rudder = steer(ship.heading, (Vec2::ZERO - ship.position).angle());
        } else if let Some(target) = &self.target {
            // We know roughly where an enemy is: close in and shoot.
            let offset = target.position - ship.position;
            let aim = offset.angle();
            orders.throttle = 0.5;
            orders.rudder = steer(ship.heading, aim);
            let lined_up = wrap_angle(aim - ship.heading).abs() < FIRING_ARC;
            if lined_up && offset.length() < FIRING_RANGE && target.age < 3.0 && ship.torpedo_cooldown <= 0.0 {
                orders.fire_torpedo = true;
            }
            if target.age > 6.0 && ship.ping_cooldown <= 0.0 {
                orders.sonar_ping = true; // refresh the stale position
            }
        } else if let Some((bearing, _)) = strongest_bearing(picture, ContactKind::Submarine) {
            // We can hear someone: head that way and ping now and then to find them.
            orders.throttle = 0.6;
            orders.rudder = steer(ship.heading, bearing);
            if self.ping_timer <= 0.0 && ship.ping_cooldown <= 0.0 {
                orders.sonar_ping = true;
                self.ping_timer = 10.0 + 8.0 * self.random();
            }
        } else {
            // Nothing around: patrol quietly with lazy turns, pinging occasionally.
            if self.wander_timer <= 0.0 {
                self.wander_rudder = self.random() - 0.5;
                self.wander_timer = 4.0 + 4.0 * self.random();
            }
            orders.throttle = 0.35;
            orders.rudder = self.wander_rudder;
            if self.ping_timer <= 0.0 && ship.ping_cooldown <= 0.0 {
                orders.sonar_ping = true;
                self.ping_timer = 25.0 + 20.0 * self.random();
            }
        }
        orders
    }

    fn remember_echoes(&mut self, picture: &SensorPicture, dt: f32) {
        if let Some(target) = &mut self.target {
            target.age += dt;
            if target.age > TARGET_MEMORY {
                self.target = None;
            }
        }
        let me = picture.own_ship.position;
        let nearest_echo = picture
            .contacts
            .iter()
            .filter_map(|contact| match *contact {
                Contact::Fix { position, kind: ContactKind::Submarine } => Some(position),
                _ => None,
            })
            .min_by(|a, b| a.distance(me).total_cmp(&b.distance(me)));
        if let Some(position) = nearest_echo {
            self.target = Some(Target { position, age: 0.0 });
        }
    }

    /// A number in `[0, 1)` from a small xorshift generator.
    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Rudder needed to turn from `heading` towards `desired` (positive turns left).
fn steer(heading: f32, desired: f32) -> f32 {
    (wrap_angle(desired - heading) * 2.0).clamp(-1.0, 1.0)
}

fn strongest_bearing(picture: &SensorPicture, wanted: ContactKind) -> Option<(f32, f32)> {
    picture
        .contacts
        .iter()
        .filter_map(|contact| match *contact {
            Contact::Bearing { bearing, kind, strength } if kind == wanted => Some((bearing, strength)),
            _ => None,
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

#[cfg(test)]
mod tests {
    use subhunt_domain::OwnShipStatus;

    use super::*;

    fn picture(contacts: Vec<Contact>) -> SensorPicture {
        SensorPicture {
            tick: 1,
            own_ship: OwnShipStatus {
                position: Vec2::ZERO,
                heading: 0.0,
                speed: 20.0,
                throttle: 0.3,
                noise: 0.5,
                torpedo_cooldown: 0.0,
                ping_cooldown: 0.0,
            },
            own_torpedoes: vec![],
            contacts,
            score: 0,
        }
    }

    #[test]
    fn fires_at_a_fresh_echo_dead_ahead() {
        let mut bot = ComputerCaptain::new(1);
        let echo = Contact::Fix { position: Vec2::new(500.0, 0.0), kind: ContactKind::Submarine };
        let orders = bot.decide(&picture(vec![echo]), 0.05, 2000.0);
        assert!(orders.fire_torpedo);
    }

    #[test]
    fn turns_towards_an_echo_that_is_not_ahead() {
        let mut bot = ComputerCaptain::new(1);
        let echo = Contact::Fix { position: Vec2::new(0.0, 500.0), kind: ContactKind::Submarine };
        let orders = bot.decide(&picture(vec![echo]), 0.05, 2000.0);
        assert!(!orders.fire_torpedo);
        assert!(orders.rudder > 0.0, "the echo is to port (left)");
    }

    #[test]
    fn runs_from_torpedoes() {
        let mut bot = ComputerCaptain::new(1);
        let torpedo = Contact::Bearing { bearing: 0.0, kind: ContactKind::Torpedo, strength: 0.9 };
        let orders = bot.decide(&picture(vec![torpedo]), 0.05, 2000.0);
        assert_eq!(orders.throttle, 1.0);
    }
}
