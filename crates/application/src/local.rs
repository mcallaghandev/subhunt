//! Use case: a practice match played entirely on this machine.
//!
//! This runs the *real* `GameServer` (same rules, same code path as online
//! play) with an in-memory transport instead of a network, and fills the
//! other seats with computer captains. To the Bevy client it looks just like
//! a network connection, because it implements the same `ClientTransport` port.
//!
//! It needs no threads and no clock of its own, which is why it also works in
//! a web page with no server at all (for example on GitHub Pages).

use std::collections::{HashMap, VecDeque};

use subhunt_domain::{GameRules, GameWorld, HelmOrders, PlayerId, SensorPicture};

use crate::bot::ComputerCaptain;
use crate::{ClientInbound, ClientTransport, GameServer, InboundEvent, ServerMessage, ServerTransport};

/// Must match the dedicated server's tick rate so the game feels the same.
const TICK_SECONDS: f32 = 0.05;
/// After a long pause (for example a background browser tab), don't try to
/// simulate minutes of missed time in one go.
const MAX_CATCH_UP_SECONDS: f32 = 0.25;

/// A "network" that is really just a few queues in memory.
#[derive(Default)]
pub struct InMemoryTransport {
    inbox: Vec<InboundEvent>,
    outboxes: HashMap<PlayerId, VecDeque<ServerMessage>>,
}

impl InMemoryTransport {
    fn take_messages(&mut self, id: PlayerId) -> VecDeque<ServerMessage> {
        self.outboxes.remove(&id).unwrap_or_default()
    }
}

impl ServerTransport for InMemoryTransport {
    fn poll(&mut self) -> Vec<InboundEvent> {
        std::mem::take(&mut self.inbox)
    }

    fn send(&mut self, to: PlayerId, message: &ServerMessage) {
        self.outboxes.entry(to).or_default().push_back(message.clone());
    }
}

struct ComputerPlayer {
    id: PlayerId,
    captain: ComputerCaptain,
    latest: Option<SensorPicture>,
}

pub struct LocalMatch {
    server: GameServer<InMemoryTransport>,
    human: PlayerId,
    computers: Vec<ComputerPlayer>,
    accumulator: f32,
    banner: Option<String>,
}

impl LocalMatch {
    /// Rules tuned for practice: a smaller sea, so you find the bots sooner.
    pub fn practice_rules() -> GameRules {
        GameRules { world_half_extent: 1200.0, ..GameRules::default() }
    }

    pub fn new(rules: GameRules, computer_count: usize, seed: u64) -> Self {
        let mut server = GameServer::new(GameWorld::new(rules, seed), InMemoryTransport::default());
        let human = PlayerId(1);
        let computers: Vec<ComputerPlayer> = (0..computer_count)
            .map(|i| ComputerPlayer {
                id: PlayerId(i as u32 + 2),
                captain: ComputerCaptain::new(seed.wrapping_add(i as u64 * 7919).wrapping_mul(31)),
                latest: None,
            })
            .collect();

        let transport = server.transport_mut();
        transport.inbox.push(InboundEvent::PlayerConnected(human));
        for computer in &computers {
            transport.inbox.push(InboundEvent::PlayerConnected(computer.id));
        }

        let banner = format!("Practice vs {computer_count} bots");
        Self { server, human, computers, accumulator: 0.0, banner: Some(banner) }
    }

    fn step(&mut self) {
        let extent = self.server.world().rules().world_half_extent;
        let transport = self.server.transport_mut();
        for computer in &mut self.computers {
            for message in transport.take_messages(computer.id) {
                if let ServerMessage::Picture(picture) = message {
                    computer.latest = Some(picture);
                }
            }
            if let Some(picture) = &computer.latest {
                let orders = computer.captain.decide(picture, TICK_SECONDS, extent);
                transport.inbox.push(InboundEvent::OrdersReceived(computer.id, orders));
            }
        }
        self.server.tick(TICK_SECONDS);
    }
}

impl ClientTransport for LocalMatch {
    fn poll(&mut self) -> Vec<ClientInbound> {
        let mut inbound: Vec<ClientInbound> = self.banner.take().map(ClientInbound::Banner).into_iter().collect();
        let messages = self.server.transport_mut().take_messages(self.human);
        inbound.extend(messages.into_iter().map(ClientInbound::Message));
        inbound
    }

    fn send_orders(&mut self, orders: &HelmOrders) {
        self.server.transport_mut().inbox.push(InboundEvent::OrdersReceived(self.human, *orders));
    }

    fn update(&mut self, dt: f32) {
        self.accumulator += dt.clamp(0.0, MAX_CATCH_UP_SECONDS);
        while self.accumulator >= TICK_SECONDS {
            self.accumulator -= TICK_SECONDS;
            self.step();
        }
    }
}

/// Tries a real server first, and quietly switches to a practice match if it
/// can't be reached. Once a real server has answered, a lost connection is
/// reported normally instead.
pub struct PracticeFallback<P: ClientTransport> {
    online: Option<P>,
    reached_server: bool,
    practice: Option<LocalMatch>,
    computer_count: usize,
    seed: u64,
}

impl<P: ClientTransport> PracticeFallback<P> {
    pub fn new(online: P, computer_count: usize, seed: u64) -> Self {
        Self { online: Some(online), reached_server: false, practice: None, computer_count, seed }
    }
}

impl<P: ClientTransport> ClientTransport for PracticeFallback<P> {
    fn poll(&mut self) -> Vec<ClientInbound> {
        if let Some(practice) = &mut self.practice {
            return practice.poll();
        }
        let Some(online) = &mut self.online else { return Vec::new() };
        let inbound = online.poll();
        if !self.reached_server {
            if inbound.iter().any(|i| matches!(i, ClientInbound::Message(_))) {
                self.reached_server = true;
            } else if inbound.contains(&ClientInbound::ConnectionLost) {
                // Never got through: play offline instead.
                self.online = None;
                let mut practice = LocalMatch::new(LocalMatch::practice_rules(), self.computer_count, self.seed);
                let mut switched = vec![ClientInbound::Banner("No server: practice".to_string())];
                // Skip the practice match's own banner; ours says more.
                switched.extend(practice.poll().into_iter().filter(|i| !matches!(i, ClientInbound::Banner(_))));
                self.practice = Some(practice);
                return switched;
            }
        }
        inbound
    }

    fn send_orders(&mut self, orders: &HelmOrders) {
        match (&mut self.practice, &mut self.online) {
            (Some(practice), _) => practice.send_orders(orders),
            (None, Some(online)) => online.send_orders(orders),
            (None, None) => {}
        }
    }

    fn update(&mut self, dt: f32) {
        match (&mut self.practice, &mut self.online) {
            (Some(practice), _) => practice.update(dt),
            (None, Some(online)) => online.update(dt),
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClientSession, ConnectionStatus};

    #[test]
    fn practice_match_runs_without_any_network() {
        let mut game = LocalMatch::new(LocalMatch::practice_rules(), 3, 42);
        let mut session = ClientSession::new();
        // Two minutes of game time at 60 frames per second.
        for _ in 0..(60 * 120) {
            game.update(1.0 / 60.0);
            game.send_orders(&HelmOrders { throttle: 0.5, rudder: 0.2, ..Default::default() });
            for inbound in game.poll() {
                session.handle(inbound);
            }
        }
        assert_eq!(session.status(), ConnectionStatus::Playing);
        assert_eq!(session.banner(), Some("Practice vs 3 bots"));
        assert!(session.picture().unwrap().own_ship.speed > 0.0);
        assert_eq!(game.server.world().player_ids().len(), 4);
    }

    /// A pretend network connection that fails straight away.
    struct Unreachable;
    impl ClientTransport for Unreachable {
        fn poll(&mut self) -> Vec<ClientInbound> {
            vec![ClientInbound::ConnectionLost]
        }
        fn send_orders(&mut self, _: &HelmOrders) {}
    }

    #[test]
    fn falls_back_to_practice_when_no_server_answers() {
        let mut transport = PracticeFallback::new(Unreachable, 2, 7);
        let mut session = ClientSession::new();
        for _ in 0..10 {
            transport.update(0.05);
            for inbound in transport.poll() {
                session.handle(inbound);
            }
        }
        assert_eq!(session.status(), ConnectionStatus::Playing);
        assert_eq!(session.banner(), Some("No server: practice"));
    }
}
