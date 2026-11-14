//! Running several transports at once (for example TCP for desktop players and
//! WebSockets for browser players) so everyone ends up in the same match.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use subhunt_domain::PlayerId;

use crate::{InboundEvent, ServerMessage, ServerTransport};

/// Hands out unique player ids. Clone it and give a copy to every transport,
/// so a TCP player and a browser player can never both be "player 1".
///
/// Rust note: `AtomicU32` is a number several threads can update safely without
/// a lock, and `Arc` lets all the clones share the same counter.
#[derive(Clone, Debug)]
pub struct PlayerIdSource(Arc<AtomicU32>);

impl PlayerIdSource {
    pub fn new() -> Self {
        Self(Arc::new(AtomicU32::new(1)))
    }

    pub fn next_id(&self) -> PlayerId {
        PlayerId(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for PlayerIdSource {
    fn default() -> Self {
        Self::new()
    }
}

/// Two transports that look like one to the game server.
///
/// This is an adapter that only plugs other adapters together, so it lives
/// here and does no I/O itself. `GameServer` neither knows nor cares that
/// there are two networks behind it.
pub struct CombinedTransport<A, B> {
    first: A,
    second: B,
    /// Which transport each player came in on, so replies go back the same way.
    routes: HashMap<PlayerId, Route>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    First,
    Second,
}

impl<A: ServerTransport, B: ServerTransport> CombinedTransport<A, B> {
    pub fn new(first: A, second: B) -> Self {
        Self { first, second, routes: HashMap::new() }
    }

    fn track(&mut self, route: Route, events: Vec<InboundEvent>) -> Vec<InboundEvent> {
        for event in &events {
            match event {
                InboundEvent::PlayerConnected(id) => {
                    self.routes.insert(*id, route);
                }
                InboundEvent::PlayerDisconnected(id) => {
                    self.routes.remove(id);
                }
                InboundEvent::OrdersReceived(..) => {}
            }
        }
        events
    }
}

impl<A: ServerTransport, B: ServerTransport> ServerTransport for CombinedTransport<A, B> {
    fn poll(&mut self) -> Vec<InboundEvent> {
        let first = self.first.poll();
        let second = self.second.poll();
        let mut events = self.track(Route::First, first);
        events.extend(self.track(Route::Second, second));
        events
    }

    fn send(&mut self, to: PlayerId, message: &ServerMessage) {
        match self.routes.get(&to) {
            Some(Route::First) => self.first.send(to, message),
            Some(Route::Second) => self.second.send(to, message),
            None => {} // unknown or already gone
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        inbox: Vec<InboundEvent>,
        sent: Vec<PlayerId>,
    }

    impl ServerTransport for Fake {
        fn poll(&mut self) -> Vec<InboundEvent> {
            std::mem::take(&mut self.inbox)
        }
        fn send(&mut self, to: PlayerId, _: &ServerMessage) {
            self.sent.push(to);
        }
    }

    #[test]
    fn ids_are_unique_across_clones() {
        let ids = PlayerIdSource::new();
        let other = ids.clone();
        assert_ne!(ids.next_id(), other.next_id());
    }

    #[test]
    fn replies_go_back_through_the_transport_the_player_used() {
        let mut combined = CombinedTransport::new(Fake::default(), Fake::default());
        combined.first.inbox.push(InboundEvent::PlayerConnected(PlayerId(1)));
        combined.second.inbox.push(InboundEvent::PlayerConnected(PlayerId(2)));
        assert_eq!(combined.poll().len(), 2);

        let notice = ServerMessage::Notice(crate::Notice::PlayerLeft(PlayerId(9)));
        combined.send(PlayerId(1), &notice);
        combined.send(PlayerId(2), &notice);
        combined.send(PlayerId(3), &notice);
        assert_eq!(combined.first.sent, vec![PlayerId(1)]);
        assert_eq!(combined.second.sent, vec![PlayerId(2)]);
    }
}
