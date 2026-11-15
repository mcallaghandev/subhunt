//! Connects the network transport (a port implementation) to the session.

use bevy::prelude::*;
use subhunt_application::{ClientTransport, ConnectionStatus};

use crate::helm::Helm;
use crate::session::Session;

/// Holds whichever transport `connect.rs` chose.
///
/// Rust note: `Box<dyn ClientTransport>` is a *trait object*: "some type that
/// implements `ClientTransport`, decided at runtime". Compare with the server,
/// which uses a generic (`GameServer<T>`) decided at compile time.
///
/// Bevy note: this is a *non-send* resource (added with `insert_non_send`,
/// read with `NonSendMut`). The browser's WebSocket can't be shared between
/// threads, so Bevy keeps this resource on the main thread for us.
pub struct Network {
    transport: Box<dyn ClientTransport>,
}

impl Network {
    pub fn new(transport: impl ClientTransport + 'static) -> Self {
        Self { transport: Box::new(transport) }
    }
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (receive_from_server, advance_session).chain()).add_systems(FixedUpdate, send_orders);
    }
}

fn receive_from_server(time: Res<Time>, mut network: NonSendMut<Network>, mut session: ResMut<Session>) {
    // Only a practice match (simulated on this machine) actually uses this.
    network.transport.update(time.delta_secs());
    for inbound in network.transport.poll() {
        session.0.handle(inbound);
    }
}

fn advance_session(time: Res<Time>, mut session: ResMut<Session>) {
    session.0.advance(time.delta_secs());
}

fn send_orders(mut network: NonSendMut<Network>, mut helm: ResMut<Helm>, session: Res<Session>) {
    if session.0.status() == ConnectionStatus::Playing {
        let orders = helm.take_orders();
        network.transport.send_orders(&orders);
    }
}
