use std::collections::HashMap;
use std::io::{self, ErrorKind};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use subhunt_application::{InboundEvent, PlayerIdSource, ServerMessage, ServerTransport};
use subhunt_domain::PlayerId;
use subhunt_protocol::{ClientToServer, ServerToClient, decode_line, encode_line};
use tungstenite::{Message, WebSocket};

use crate::static_files;

/// How long a connection thread waits for incoming data before checking
/// whether it has anything to send. Lower = less latency, more CPU.
const POLL_INTERVAL: Duration = Duration::from_millis(5);
/// How long we wait for a browser to finish sending its HTTP request headers.
const HEADER_TIMEOUT: Duration = Duration::from_secs(2);

/// Each connected player's outbox: the connection thread sends whatever arrives here.
type Outboxes = Arc<Mutex<HashMap<PlayerId, Sender<String>>>>;

pub struct WsServerTransport {
    events: Receiver<InboundEvent>,
    outboxes: Outboxes,
    local_addr: SocketAddr,
}

impl WsServerTransport {
    /// Starts listening for browsers.
    ///
    /// * `ids` hands out player ids (share it with any other transports).
    /// * `site` is the folder of web files to serve to plain HTTP requests,
    ///   or `None` to serve only WebSockets.
    pub fn bind(addr: impl ToSocketAddrs, ids: PlayerIdSource, site: Option<PathBuf>) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        let local_addr = listener.local_addr()?;
        let (sender, events) = mpsc::channel();
        let outboxes = Outboxes::default();

        let outboxes_for_thread = Arc::clone(&outboxes);
        thread::spawn(move || accept_loop(listener, ids, site, sender, outboxes_for_thread));

        Ok(Self { events, outboxes, local_addr })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

impl ServerTransport for WsServerTransport {
    fn poll(&mut self) -> Vec<InboundEvent> {
        self.events.try_iter().collect()
    }

    fn send(&mut self, to: PlayerId, message: &ServerMessage) {
        let text = encode_line(&ServerToClient::from(message));
        let outboxes = self.outboxes.lock().expect("outboxes lock poisoned");
        if let Some(outbox) = outboxes.get(&to) {
            // If the connection thread has gone, it will report the disconnect itself.
            let _ = outbox.send(text);
        }
    }
}

fn accept_loop(
    listener: TcpListener,
    ids: PlayerIdSource,
    site: Option<PathBuf>,
    events: Sender<InboundEvent>,
    outboxes: Outboxes,
) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let ids = ids.clone();
        let site = site.clone();
        let events = events.clone();
        let outboxes = Arc::clone(&outboxes);
        // Every connection gets its own thread, so a slow browser can't hold up others.
        thread::spawn(move || handle_connection(stream, ids, site, events, outboxes));
    }
}

/// Works out whether this is a browser asking for a web page or for a
/// WebSocket, and hands it to the right code.
fn handle_connection(
    stream: TcpStream,
    ids: PlayerIdSource,
    site: Option<PathBuf>,
    events: Sender<InboundEvent>,
    outboxes: Outboxes,
) {
    let Some(headers) = peek_request_headers(&stream) else { return };
    if headers.to_ascii_lowercase().contains("upgrade: websocket") {
        play(stream, ids, events, outboxes);
    } else {
        static_files::serve(stream, &headers, site.as_deref());
    }
}

/// Looks at the HTTP request headers *without consuming them*, so the
/// WebSocket library can still read the full request afterwards.
fn peek_request_headers(stream: &TcpStream) -> Option<String> {
    let mut buffer = [0u8; 8192];
    let deadline = Instant::now() + HEADER_TIMEOUT;
    loop {
        let read = stream.peek(&mut buffer).ok()?;
        if read == 0 {
            return None; // closed before sending anything
        }
        let text = String::from_utf8_lossy(&buffer[..read]);
        if let Some(end) = text.find("\r\n\r\n") {
            return Some(text[..end + 4].to_string());
        }
        if read == buffer.len() || Instant::now() > deadline {
            return None; // headers too big, or too slow
        }
        thread::sleep(Duration::from_millis(2));
    }
}

/// Runs one player's WebSocket connection until it closes.
fn play(stream: TcpStream, ids: PlayerIdSource, events: Sender<InboundEvent>, outboxes: Outboxes) {
    let _ = stream.set_nodelay(true);
    let mut socket = match tungstenite::accept(stream) {
        Ok(socket) => socket,
        Err(error) => {
            eprintln!("WebSocket handshake failed: {error}");
            return;
        }
    };
    // From now on, reads give up after POLL_INTERVAL so we can also send.
    let _ = socket.get_ref().set_read_timeout(Some(POLL_INTERVAL));

    let id = ids.next_id();
    let (outbox, outgoing) = mpsc::channel();
    outboxes.lock().expect("outboxes lock poisoned").insert(id, outbox);
    if events.send(InboundEvent::PlayerConnected(id)).is_ok() {
        run_connection(id, &mut socket, &outgoing, &events);
    }

    outboxes.lock().expect("outboxes lock poisoned").remove(&id);
    let _ = events.send(InboundEvent::PlayerDisconnected(id));
}

fn run_connection(
    id: PlayerId,
    socket: &mut WebSocket<TcpStream>,
    outgoing: &Receiver<String>,
    events: &Sender<InboundEvent>,
) {
    loop {
        // 1. Send everything the game has queued for this player.
        loop {
            match outgoing.try_recv() {
                Ok(text) => {
                    if is_fatal(socket.send(Message::text(text))) {
                        return;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        if is_fatal(socket.flush()) {
            return;
        }

        // 2. Wait briefly for a message from the browser.
        match socket.read() {
            Ok(Message::Text(text)) => match decode_line::<ClientToServer>(text.as_str()) {
                Ok(ClientToServer::Orders(orders)) => {
                    if events.send(InboundEvent::OrdersReceived(id, orders.into())).is_err() {
                        return;
                    }
                }
                Err(error) => eprintln!("{id} sent something odd: {error}"),
            },
            Ok(Message::Close(_)) => return,
            Ok(_) => {} // pings, pongs and binary frames: nothing for us to do
            Err(error) => {
                if is_fatal::<()>(Err(error)) {
                    return;
                }
            }
        }
    }
}

/// "Nothing arrived in time" and "couldn't write everything yet" are normal
/// for a polling connection. Anything else means the connection is finished.
fn is_fatal<T>(result: Result<T, tungstenite::Error>) -> bool {
    match result {
        Ok(_) => false,
        Err(tungstenite::Error::Io(error)) => !matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
        Err(_) => true,
    }
}
