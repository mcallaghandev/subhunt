//! Integration tests: a real WebSocket client (standing in for a browser)
//! talking to the server adapter over localhost.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use subhunt_application::{InboundEvent, PlayerIdSource, ServerMessage, ServerTransport};
use subhunt_domain::HelmOrders;
use subhunt_net_ws::WsServerTransport;
use subhunt_protocol::{ClientToServer, ServerToClient, decode_line, encode_line};
use tungstenite::Message;

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
fn browser_style_client_can_play() {
    let mut server = WsServerTransport::bind("127.0.0.1:0", PlayerIdSource::new(), None).unwrap();
    let url = format!("ws://{}/ws", server.local_addr());
    let (mut client, _) = tungstenite::connect(url).unwrap();

    let id = wait_for(|| {
        server.poll().into_iter().find_map(|e| match e {
            InboundEvent::PlayerConnected(id) => Some(id),
            _ => None,
        })
    });

    let orders = HelmOrders { throttle: 1.0, rudder: 0.5, fire_torpedo: false, sonar_ping: true };
    client.send(Message::text(encode_line(&ClientToServer::from(&orders)))).unwrap();
    let received = wait_for(|| {
        server.poll().into_iter().find_map(|e| match e {
            InboundEvent::OrdersReceived(from, o) if from == id => Some(o),
            _ => None,
        })
    });
    assert_eq!(received, orders);

    let welcome = ServerMessage::Welcome { player_id: id, world_half_extent: 2000.0, ping_range: 1500.0 };
    server.send(id, &welcome);
    let Message::Text(text) = client.read().unwrap() else { panic!("expected a text message") };
    let decoded: ServerToClient = decode_line(text.as_str()).unwrap();
    assert_eq!(ServerMessage::from(decoded), welcome);

    client.close(None).unwrap();
    wait_for(|| server.poll().into_iter().find(|e| *e == InboundEvent::PlayerDisconnected(id)));
}

#[test]
fn plain_http_requests_get_the_web_page() {
    let site = std::env::temp_dir().join(format!("subhunt-site-{}", std::process::id()));
    std::fs::create_dir_all(&site).unwrap();
    std::fs::write(site.join("index.html"), "<h1>SubHunt</h1>").unwrap();

    let server = WsServerTransport::bind("127.0.0.1:0", PlayerIdSource::new(), Some(site.clone())).unwrap();
    let mut stream = TcpStream::connect(server.local_addr()).unwrap();
    stream.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(response.contains("text/html"));
    assert!(response.ends_with("<h1>SubHunt</h1>"));
    std::fs::remove_dir_all(site).unwrap();
}
