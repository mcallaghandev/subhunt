//! The whole game state and the rules that advance it.

use std::collections::BTreeMap;
use std::f32::consts::PI;

use crate::rng::SimpleRng;
use crate::{
    Contact, ContactKind, GameRules, HelmOrders, PlayerId, Submarine, Torpedo, TorpedoId, Vec2, passive_strength,
};

/// How many random spots we try when choosing where to (re)spawn a submarine.
const SPAWN_CANDIDATES: usize = 12;

/// Something notable that happened during a step.
///
/// The domain just *reports* events. Deciding what to do about them (send a
/// message, play a sound, write a log line) is somebody else's job.
#[derive(Clone, Debug, PartialEq)]
pub enum GameEvent {
    TorpedoLaunched {
        owner: PlayerId,
        torpedo: TorpedoId,
    },
    SonarPing {
        by: PlayerId,
        position: Vec2,
    },
    /// `target` was hit by someone's ping, so they know they've been spotted.
    PingDetected {
        target: PlayerId,
        bearing_to_pinger: f32,
    },
    SubmarineSunk {
        victim: PlayerId,
        attacker: PlayerId,
    },
}

/// Your own submarine's state: the one thing you can know exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnShipStatus {
    pub position: Vec2,
    pub heading: f32,
    pub speed: f32,
    pub throttle: f32,
    pub noise: f32,
    pub torpedo_cooldown: f32,
    pub ping_cooldown: f32,
}

/// One of your own torpedoes (you know where your own weapons are).
#[derive(Clone, Debug, PartialEq)]
pub struct TorpedoTrack {
    pub id: TorpedoId,
    pub position: Vec2,
    pub heading: f32,
}

/// Everything one player is allowed to know this tick.
///
/// **This is the heart of "players can't see each other".** The server builds
/// one of these per player and sends only that. Other submarines' positions
/// simply never leave the server unless your sonar earned them, so a hacked
/// client has nothing to reveal.
#[derive(Clone, Debug, PartialEq)]
pub struct SensorPicture {
    pub tick: u64,
    pub own_ship: OwnShipStatus,
    pub own_torpedoes: Vec<TorpedoTrack>,
    pub contacts: Vec<Contact>,
    pub score: u32,
}

/// The authoritative game state. Only the server owns one of these.
pub struct GameWorld {
    rules: GameRules,
    // Rust note: `BTreeMap` is a sorted map. We use it rather than `HashMap` so
    // iteration order is stable, which keeps the simulation deterministic.
    submarines: BTreeMap<PlayerId, Submarine>,
    torpedoes: Vec<Torpedo>,
    scores: BTreeMap<PlayerId, u32>,
    /// Echoes from pings sent this step, keyed by who pinged.
    ping_returns: BTreeMap<PlayerId, Vec<Contact>>,
    next_torpedo_id: u32,
    rng: SimpleRng,
    tick: u64,
}

impl GameWorld {
    pub fn new(rules: GameRules, seed: u64) -> Self {
        Self {
            rules,
            submarines: BTreeMap::new(),
            torpedoes: Vec::new(),
            scores: BTreeMap::new(),
            ping_returns: BTreeMap::new(),
            next_torpedo_id: 1,
            rng: SimpleRng::new(seed),
            tick: 0,
        }
    }

    pub fn rules(&self) -> &GameRules {
        &self.rules
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn player_ids(&self) -> Vec<PlayerId> {
        self.submarines.keys().copied().collect()
    }

    pub fn submarine(&self, id: PlayerId) -> Option<&Submarine> {
        self.submarines.get(&id)
    }

    pub fn score(&self, id: PlayerId) -> u32 {
        self.scores.get(&id).copied().unwrap_or(0)
    }

    /// Adds a player with a fresh submarine. Returns `false` if they already exist.
    pub fn add_player(&mut self, id: PlayerId) -> bool {
        if self.submarines.contains_key(&id) {
            return false;
        }
        let position = self.find_spawn_point(id);
        let heading = self.rng.range(-PI, PI);
        self.submarines.insert(id, Submarine::new(position, heading));
        self.scores.insert(id, 0);
        true
    }

    /// Removes a player and their torpedoes. Returns `false` if they weren't here.
    pub fn remove_player(&mut self, id: PlayerId) -> bool {
        self.scores.remove(&id);
        self.ping_returns.remove(&id);
        self.torpedoes.retain(|t| t.owner != id);
        self.submarines.remove(&id).is_some()
    }

    /// Moves a submarine to an exact spot. Handy for tests and scripted scenarios.
    pub fn place_submarine(&mut self, id: PlayerId, position: Vec2, heading: f32) {
        if let Some(sub) = self.submarines.get_mut(&id) {
            sub.position = position.clamp_to_square(self.rules.world_half_extent);
            sub.heading = heading;
        }
    }

    /// Records a player's latest orders (after sanitising them).
    pub fn set_orders(&mut self, id: PlayerId, orders: HelmOrders) {
        // Rust note: `if let Some(x) = ...` runs the block only when the map
        // actually contains the player. Unknown players are silently ignored.
        if let Some(sub) = self.submarines.get_mut(&id) {
            sub.orders = sub.orders.superseded_by(orders.sanitised());
        }
    }

    /// Advances the whole world by `dt` seconds and reports what happened.
    pub fn step(&mut self, dt: f32) -> Vec<GameEvent> {
        self.tick += 1;
        self.ping_returns.clear();
        let mut events = Vec::new();

        // 1. Every submarine acts on its orders and moves.
        let mut launches = Vec::new();
        let mut pingers = Vec::new();
        for (&id, sub) in &mut self.submarines {
            if let Some(launch) = sub.take_torpedo_launch(&self.rules) {
                launches.push((id, launch));
            }
            if sub.take_sonar_ping(&self.rules) {
                pingers.push(id);
            }
            sub.integrate(dt, &self.rules);
        }
        // Rust note: we collected launches/pings into lists first and act on them
        // *after* the loop. Inside the loop `self.submarines` is borrowed mutably,
        // so the borrow checker won't let us also push to other parts of `self`
        // through methods that take `&mut self`.

        // 2. New torpedoes enter the water.
        for (owner, (position, heading)) in launches {
            let id = TorpedoId(self.next_torpedo_id);
            self.next_torpedo_id += 1;
            self.torpedoes.push(Torpedo::new(id, owner, position, heading, &self.rules));
            events.push(GameEvent::TorpedoLaunched { owner, torpedo: id });
        }

        // 3. Pings bounce off everything in range.
        for pinger in pingers {
            self.resolve_ping(pinger, &mut events);
        }

        // 4. Torpedoes run and maybe hit something.
        self.advance_torpedoes(dt, &mut events);

        events
    }

    /// Builds what `id` is allowed to know. `None` if there's no such player.
    pub fn sensor_picture(&self, id: PlayerId) -> Option<SensorPicture> {
        // Rust note: the `?` returns `None` early if the player doesn't exist.
        let me = self.submarines.get(&id)?;
        let hearing = me.hearing_factor(&self.rules);
        let mut contacts = Vec::new();

        // Passive sonar: listen for other submarines...
        for (&other_id, other) in &self.submarines {
            if other_id == id {
                continue;
            }
            let noise = other.noise(&self.rules);
            if let Some(strength) = passive_strength(me.position, hearing, other.position, noise, &self.rules) {
                contacts.push(Contact::Bearing {
                    bearing: (other.position - me.position).angle(),
                    kind: ContactKind::Submarine,
                    strength,
                });
            }
        }
        // ...and for other people's torpedoes.
        for torpedo in self.torpedoes.iter().filter(|t| t.owner != id) {
            let noise = self.rules.torpedo_noise;
            if let Some(strength) = passive_strength(me.position, hearing, torpedo.position, noise, &self.rules) {
                contacts.push(Contact::Bearing {
                    bearing: (torpedo.position - me.position).angle(),
                    kind: ContactKind::Torpedo,
                    strength,
                });
            }
        }

        // Active sonar: echoes from a ping we sent this step.
        if let Some(returns) = self.ping_returns.get(&id) {
            contacts.extend(returns.iter().copied());
        }

        let own_torpedoes = self
            .torpedoes
            .iter()
            .filter(|t| t.owner == id)
            .map(|t| TorpedoTrack { id: t.id, position: t.position, heading: t.heading })
            .collect();

        Some(SensorPicture {
            tick: self.tick,
            own_ship: me.status(&self.rules),
            own_torpedoes,
            contacts,
            score: self.score(id),
        })
    }

    fn resolve_ping(&mut self, pinger: PlayerId, events: &mut Vec<GameEvent>) {
        let Some(origin) = self.submarines.get(&pinger).map(|s| s.position) else {
            return;
        };
        let range = self.rules.ping_range;
        let mut returns = Vec::new();

        for (&id, sub) in &self.submarines {
            if id != pinger && sub.position.distance(origin) <= range {
                returns.push(Contact::Fix { position: sub.position, kind: ContactKind::Submarine });
                events.push(GameEvent::PingDetected { target: id, bearing_to_pinger: (origin - sub.position).angle() });
            }
        }
        for torpedo in &self.torpedoes {
            if torpedo.owner != pinger && torpedo.position.distance(origin) <= range {
                returns.push(Contact::Fix { position: torpedo.position, kind: ContactKind::Torpedo });
            }
        }

        events.push(GameEvent::SonarPing { by: pinger, position: origin });
        self.ping_returns.insert(pinger, returns);
    }

    fn advance_torpedoes(&mut self, dt: f32, events: &mut Vec<GameEvent>) {
        // Borrow the fields we need separately so we can mutate `torpedoes`
        // while reading `rules` and `submarines`.
        let rules = &self.rules;
        let submarines = &self.submarines;
        let mut hits: Vec<(PlayerId, PlayerId)> = Vec::new(); // (victim, attacker)

        // `retain_mut` keeps the torpedoes for which the closure returns `true`.
        self.torpedoes.retain_mut(|torpedo| {
            torpedo.integrate(dt);
            if torpedo.is_spent(rules) {
                return false;
            }
            let victim = submarines
                .iter()
                .find(|(id, sub)| {
                    **id != torpedo.owner
                        && !hits.iter().any(|(already_hit, _)| already_hit == *id)
                        && sub.position.distance(torpedo.position) <= rules.torpedo_hit_radius
                })
                .map(|(id, _)| *id);
            match victim {
                Some(victim) => {
                    hits.push((victim, torpedo.owner));
                    false // the torpedo exploded
                }
                None => true,
            }
        });

        for (victim, attacker) in hits {
            self.sink(victim, attacker, events);
        }
    }

    fn sink(&mut self, victim: PlayerId, attacker: PlayerId, events: &mut Vec<GameEvent>) {
        if attacker != victim
            && let Some(score) = self.scores.get_mut(&attacker)
        {
            *score += 1;
        }
        // Respawn straight away somewhere far from everyone else.
        let position = self.find_spawn_point(victim);
        let heading = self.rng.range(-PI, PI);
        if let Some(sub) = self.submarines.get_mut(&victim) {
            *sub = Submarine::new(position, heading);
        }
        events.push(GameEvent::SubmarineSunk { victim, attacker });
    }

    /// Tries a few random spots and picks the one furthest from other submarines.
    fn find_spawn_point(&mut self, for_player: PlayerId) -> Vec2 {
        let limit = self.rules.world_half_extent * 0.9;
        let mut best = Vec2::ZERO;
        let mut best_clearance = f32::NEG_INFINITY;
        for _ in 0..SPAWN_CANDIDATES {
            let candidate = Vec2::new(self.rng.range(-limit, limit), self.rng.range(-limit, limit));
            let clearance = self
                .submarines
                .iter()
                .filter(|(id, _)| **id != for_player)
                .map(|(_, sub)| sub.position.distance(candidate))
                .fold(f32::INFINITY, f32::min);
            if clearance > best_clearance {
                best = candidate;
                best_clearance = clearance;
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 0.05;
    const A: PlayerId = PlayerId(1);
    const B: PlayerId = PlayerId(2);

    /// Two submarines facing east, `distance` metres apart, both stopped.
    fn two_subs(distance: f32) -> GameWorld {
        let mut world = GameWorld::new(GameRules::default(), 42);
        world.add_player(A);
        world.add_player(B);
        world.place_submarine(A, Vec2::new(-distance / 2.0, 0.0), 0.0);
        world.place_submarine(B, Vec2::new(distance / 2.0, 0.0), 0.0);
        world
    }

    fn contacts_of(world: &GameWorld, id: PlayerId) -> Vec<Contact> {
        world.sensor_picture(id).expect("player exists").contacts
    }

    #[test]
    fn quiet_submarine_is_not_heard_from_far_away() {
        let mut world = two_subs(1000.0);
        world.step(DT);
        assert!(contacts_of(&world, A).is_empty());
    }

    #[test]
    fn noisy_submarine_gives_a_bearing_but_not_a_position() {
        // Quiet at this range, but B sails east (away from A) at full speed.
        let mut world = two_subs(700.0);
        assert!(contacts_of(&world, A).is_empty());
        world.set_orders(B, HelmOrders { throttle: 1.0, ..Default::default() });
        for _ in 0..100 {
            world.step(DT); // five seconds to speed up
        }
        let contacts = contacts_of(&world, A);
        assert_eq!(contacts.len(), 1);
        match contacts[0] {
            Contact::Bearing { bearing, kind, .. } => {
                assert_eq!(kind, ContactKind::Submarine);
                assert!(bearing.abs() < 0.1, "B is due east of A");
            }
            Contact::Fix { .. } => panic!("passive sonar must never give an exact position"),
        }
    }

    #[test]
    fn ping_reveals_positions_and_warns_the_target() {
        let mut world = two_subs(1000.0);
        world.set_orders(A, HelmOrders { sonar_ping: true, ..Default::default() });
        let events = world.step(DT);

        assert!(events.iter().any(|e| matches!(e, GameEvent::PingDetected { target, .. } if *target == B)));
        assert!(contacts_of(&world, A).iter().any(|c| matches!(c, Contact::Fix { kind: ContactKind::Submarine, .. })));
        // B now hears A's loud ping on passive sonar.
        assert!(contacts_of(&world, B).iter().any(|c| matches!(c, Contact::Bearing { .. })));

        // Echoes only last for the step in which the ping happened.
        world.step(DT);
        assert!(!contacts_of(&world, A).iter().any(|c| matches!(c, Contact::Fix { .. })));
    }

    #[test]
    fn torpedo_sinks_target_and_scores() {
        let mut world = two_subs(300.0);
        world.set_orders(A, HelmOrders { fire_torpedo: true, ..Default::default() });

        let mut sunk = false;
        for _ in 0..100 {
            let events = world.step(DT);
            if events.contains(&GameEvent::SubmarineSunk { victim: B, attacker: A }) {
                sunk = true;
                break;
            }
        }
        assert!(sunk, "torpedo should reach B within five seconds");
        assert_eq!(world.score(A), 1);
        assert_eq!(world.score(B), 0);
    }

    #[test]
    fn torpedo_never_hits_its_owner() {
        let mut world = GameWorld::new(GameRules::default(), 7);
        world.add_player(A);
        world.set_orders(A, HelmOrders { fire_torpedo: true, ..Default::default() });
        for _ in 0..300 {
            let events = world.step(DT);
            assert!(!events.iter().any(|e| matches!(e, GameEvent::SubmarineSunk { .. })));
        }
    }

    #[test]
    fn removing_a_player_removes_their_torpedoes() {
        let mut world = two_subs(1500.0);
        world.set_orders(A, HelmOrders { fire_torpedo: true, ..Default::default() });
        world.step(DT);
        assert!(world.remove_player(A));
        assert!(world.torpedoes.is_empty());
        assert!(world.sensor_picture(A).is_none());
    }
}
