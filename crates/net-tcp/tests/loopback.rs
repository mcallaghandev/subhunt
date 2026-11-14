//! Integration test: a real client and server talking over localhost.
//!
//! Files in `tests/` are compiled as separate crates that can only use the
//! public API, just like a real user of this crate would.

use std::time::{Duration, Instant};

use subhunt_application::{ClientInbound, ClientTransport, InboundEvent, ServerMessage, ServerTransport};
use subhunt_domain::{HelmOrders, PlayerId};
use subhunt_net_tcp::{TcpClientTransport, TcpServerTransport};

/// Polls until `found` returns something, or panics after two seconds.
fn wait_for<T>(mut found: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(value) = found() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn client_and_server_exchange_messages() {
    let mut server = TcpServerTransport::bind("127.0.0.1:0").unwrap();
    let mut client = TcpClientTransport::connect(server.local_addr()).unwrap();

    let id = wait_for(|| {
        server.poll().into_iter().find_map(|e| match e {
            InboundEvent::PlayerConnected(id) => Some(id),
            _ => None,
        })
    });

    let orders = HelmOrders { throttle: 0.5, rudder: -1.0, fire_torpedo: true, sonar_ping: false };
    client.send_orders(&orders);
    let received = wait_for(|| {
        server.poll().into_iter().find_map(|e| match e {
            InboundEvent::OrdersReceived(from, o) if from == id => Some(o),
            _ => None,
        })
    });
    assert_eq!(received, orders);

    let welcome = ServerMessage::Welcome { player_id: id, world_half_extent: 2000.0, ping_range: 1500.0 };
    server.send(id, &welcome);
    let message = wait_for(|| client.poll().into_iter().next());
    assert_eq!(message, ClientInbound::Message(welcome));

    drop(client);
    wait_for(|| server.poll().into_iter().find(|e| *e == InboundEvent::PlayerDisconnected(PlayerId(id.0))));
}
