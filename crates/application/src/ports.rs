//! Ports: what the application needs from the outside world.

use subhunt_domain::{HelmOrders, PlayerId};

use crate::ServerMessage;

/// Something a server transport noticed, translated into application terms.
#[derive(Clone, Debug, PartialEq)]
pub enum InboundEvent {
    PlayerConnected(PlayerId),
    PlayerDisconnected(PlayerId),
    OrdersReceived(PlayerId, HelmOrders),
}

/// Server side of the network, as the game server sees it.
///
/// The game server calls these; it doesn't care whether there's TCP, UDP,
/// WebSockets or (in tests) a plain `Vec` behind them.
pub trait ServerTransport {
    /// Everything that has arrived since the last call. Must not block.
    fn poll(&mut self) -> Vec<InboundEvent>;
    /// Sends a message to one player. Silently drops it if they've gone.
    fn send(&mut self, to: PlayerId, message: &ServerMessage);
}

/// Something a client transport received.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientInbound {
    Message(ServerMessage),
    ConnectionLost,
    /// A line of text to show the player permanently, such as which mode they're in.
    Banner(String),
}

/// Client side of the network.
pub trait ClientTransport {
    /// Everything that has arrived since the last call. Must not block.
    fn poll(&mut self) -> Vec<ClientInbound>;
    fn send_orders(&mut self, orders: &HelmOrders);
    /// Lets time pass. Network transports ignore this (the real server keeps
    /// its own time), but a game simulated on this machine needs it.
    ///
    /// Rust note: a trait method with a body is a *default*. Implementors
    /// get it for free and only override it if they need to.
    fn update(&mut self, _dt: f32) {}
}
