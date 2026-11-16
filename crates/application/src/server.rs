//! Use case: run an authoritative game server.

use subhunt_domain::{GameEvent, GameWorld, PlayerId};

use crate::{InboundEvent, Notice, ServerMessage, ServerTransport};

/// What happened during one tick, so the caller can log it.
///
/// The application never prints anything itself; printing is I/O, and I/O
/// lives at the edges (here, the server binary's `main`).
#[derive(Debug, Default)]
pub struct TickReport {
    pub joined: Vec<PlayerId>,
    pub left: Vec<PlayerId>,
    pub events: Vec<GameEvent>,
}

/// Owns the game world and talks to players through *some* transport.
///
/// Rust note: `<T: ServerTransport>` is a *generic*. `GameServer<TcpServerTransport>`
/// and `GameServer<FakeTransport>` are two different concrete types, and the
/// compiler generates code for each. No runtime cost, and the test version
/// can't accidentally open a socket.
pub struct GameServer<T: ServerTransport> {
    world: GameWorld,
    transport: T,
}

impl<T: ServerTransport> GameServer<T> {
    pub fn new(world: GameWorld, transport: T) -> Self {
        Self { world, transport }
    }

    pub fn world(&self) -> &GameWorld {
        &self.world
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// One server tick: read the network, step the world, send each player
    /// their own sensor picture.
    pub fn tick(&mut self, dt: f32) -> TickReport {
        let mut report = TickReport::default();

        for event in self.transport.poll() {
            match event {
                InboundEvent::PlayerConnected(id) => {
                    if self.world.add_player(id) {
                        let rules = self.world.rules();
                        let welcome = ServerMessage::Welcome {
                            player_id: id,
                            world_half_extent: rules.world_half_extent,
                            ping_range: rules.ping_range,
                        };
                        self.transport.send(id, &welcome);
                        self.broadcast_except(id, Notice::PlayerJoined(id));
                        report.joined.push(id);
                    }
                }
                InboundEvent::PlayerDisconnected(id) => {
                    if self.world.remove_player(id) {
                        self.broadcast_except(id, Notice::PlayerLeft(id));
                        report.left.push(id);
                    }
                }
                InboundEvent::OrdersReceived(id, orders) => self.world.set_orders(id, orders),
            }
        }

        let events = self.world.step(dt);
        for event in &events {
            self.notify(event);
        }

        for id in self.world.player_ids() {
            if let Some(picture) = self.world.sensor_picture(id) {
                self.transport.send(id, &ServerMessage::Picture(picture));
            }
        }

        report.events = events;
        report
    }

    /// Turns game events into personal notices for the players involved.
    fn notify(&mut self, event: &GameEvent) {
        match *event {
            GameEvent::SubmarineSunk { victim, attacker, position } => {
                self.transport.send(victim, &ServerMessage::Notice(Notice::Sunk { by: attacker }));
                if attacker != victim {
                    self.transport.send(attacker, &ServerMessage::Notice(Notice::Kill { victim, position }));
                }
            }
            GameEvent::PingDetected { target, bearing_to_pinger } => {
                self.transport.send(target, &ServerMessage::Notice(Notice::Pinged { bearing: bearing_to_pinger }));
            }
            // Rust note: `_` matches "anything else". Launches and pings are
            // already visible through sonar, so there's nothing extra to say.
            _ => {}
        }
    }

    fn broadcast_except(&mut self, except: PlayerId, notice: Notice) {
        for id in self.world.player_ids() {
            if id != except {
                self.transport.send(id, &ServerMessage::Notice(notice));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use subhunt_domain::{Contact, GameRules, HelmOrders};

    use super::*;

    /// A pretend network: we push events in and read sent messages out.
    /// This is the big payoff of having a port: the whole server use case is
    /// testable without sockets, threads or timing.
    #[derive(Default)]
    struct FakeTransport {
        inbox: Vec<InboundEvent>,
        sent: Vec<(PlayerId, ServerMessage)>,
    }

    impl ServerTransport for FakeTransport {
        fn poll(&mut self) -> Vec<InboundEvent> {
            std::mem::take(&mut self.inbox)
        }
        fn send(&mut self, to: PlayerId, message: &ServerMessage) {
            self.sent.push((to, message.clone()));
        }
    }

    const ALICE: PlayerId = PlayerId(1);
    const BOB: PlayerId = PlayerId(2);

    fn server_with_two_players() -> GameServer<FakeTransport> {
        let mut server = GameServer::new(GameWorld::new(GameRules::default(), 1), FakeTransport::default());
        server.transport_mut().inbox.extend([InboundEvent::PlayerConnected(ALICE), InboundEvent::PlayerConnected(BOB)]);
        server.tick(0.05);
        server
    }

    fn messages_to(server: &GameServer<FakeTransport>, id: PlayerId) -> Vec<&ServerMessage> {
        server.transport.sent.iter().filter(|(to, _)| *to == id).map(|(_, m)| m).collect()
    }

    #[test]
    fn joining_player_is_welcomed_before_their_first_picture() {
        let server = server_with_two_players();
        let to_alice = messages_to(&server, ALICE);
        assert!(matches!(to_alice[0], ServerMessage::Welcome { player_id, .. } if *player_id == ALICE));
        assert!(to_alice.iter().any(|m| matches!(m, ServerMessage::Picture(_))));
        assert!(to_alice.iter().any(|m| matches!(m, ServerMessage::Notice(Notice::PlayerJoined(p)) if *p == BOB)));
    }

    #[test]
    fn players_never_receive_enemy_positions_without_pinging() {
        let mut server = server_with_two_players();
        let full_ahead = HelmOrders { throttle: 1.0, ..Default::default() };
        for _ in 0..200 {
            server.transport_mut().inbox.extend([
                InboundEvent::OrdersReceived(ALICE, full_ahead),
                InboundEvent::OrdersReceived(BOB, full_ahead),
            ]);
            server.tick(0.05);
        }
        for (_, message) in &server.transport.sent {
            if let ServerMessage::Picture(picture) = message {
                assert!(!picture.contacts.iter().any(|c| matches!(c, Contact::Fix { .. })));
            }
        }
    }

    #[test]
    fn orders_reach_the_right_submarine() {
        let mut server = server_with_two_players();
        server
            .transport_mut()
            .inbox
            .push(InboundEvent::OrdersReceived(ALICE, HelmOrders { throttle: 1.0, ..Default::default() }));
        for _ in 0..10 {
            server.tick(0.05);
        }
        assert!(server.world().submarine(ALICE).unwrap().speed > 0.0);
        assert_eq!(server.world().submarine(BOB).unwrap().speed, 0.0);
    }

    #[test]
    fn disconnecting_player_is_removed_and_others_told() {
        let mut server = server_with_two_players();
        server.transport_mut().inbox.push(InboundEvent::PlayerDisconnected(BOB));
        let report = server.tick(0.05);
        assert_eq!(report.left, vec![BOB]);
        assert!(server.world().submarine(BOB).is_none());
        assert!(
            messages_to(&server, ALICE)
                .iter()
                .any(|m| matches!(m, ServerMessage::Notice(Notice::PlayerLeft(p)) if *p == BOB))
        );
    }
}
