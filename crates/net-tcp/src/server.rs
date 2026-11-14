use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use subhunt_application::{InboundEvent, PlayerIdSource, ServerMessage, ServerTransport};
use subhunt_domain::PlayerId;
use subhunt_protocol::{ClientToServer, ServerToClient, decode_line, encode_line};

/// A slow client must not freeze the whole server, so writes give up quickly.
const WRITE_TIMEOUT: Duration = Duration::from_millis(250);

/// The write half of every connected player's socket.
///
/// Rust note: `Arc` (atomically reference counted) lets several threads own the
/// same value; `Mutex` makes sure only one of them touches it at a time. You'll
/// see `Arc<Mutex<T>>` a lot in multithreaded Rust.
type Writers = Arc<Mutex<HashMap<PlayerId, TcpStream>>>;

pub struct TcpServerTransport {
    events: Receiver<InboundEvent>,
    writers: Writers,
    local_addr: SocketAddr,
}

impl TcpServerTransport {
    /// Starts listening. Use port 0 to let the OS pick a free port (handy in tests).
    /// New players get ids from `ids`, which may be shared with other transports.
    pub fn bind(addr: impl ToSocketAddrs, ids: PlayerIdSource) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?; // `?` returns the error to our caller
        let local_addr = listener.local_addr()?;
        let (sender, events) = mpsc::channel();
        let writers = Writers::default();

        let writers_for_thread = Arc::clone(&writers);
        // Rust note: `move` hands ownership of the captured variables to the new thread.
        thread::spawn(move || accept_loop(listener, ids, sender, writers_for_thread));

        Ok(Self { events, writers, local_addr })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

impl ServerTransport for TcpServerTransport {
    fn poll(&mut self) -> Vec<InboundEvent> {
        self.events.try_iter().collect()
    }

    fn send(&mut self, to: PlayerId, message: &ServerMessage) {
        let line = encode_line(&ServerToClient::from(message));
        let mut writers = self.writers.lock().expect("writers lock poisoned");
        if let Some(stream) = writers.get_mut(&to)
            && stream.write_all(line.as_bytes()).is_err()
        {
            // The reader thread will notice the dead socket and report the disconnect.
            writers.remove(&to);
        }
    }
}

fn accept_loop(listener: TcpListener, ids: PlayerIdSource, events: Sender<InboundEvent>, writers: Writers) {
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("failed to accept connection: {error}");
                continue;
            }
        };
        let id = ids.next_id();

        // Send small packets immediately instead of batching them (lower latency).
        let _ = stream.set_nodelay(true);
        let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
        let writer = match stream.try_clone() {
            Ok(writer) => writer,
            Err(error) => {
                eprintln!("failed to set up connection: {error}");
                continue;
            }
        };
        writers.lock().expect("writers lock poisoned").insert(id, writer);

        if events.send(InboundEvent::PlayerConnected(id)).is_err() {
            return; // the game server has shut down
        }
        let events = events.clone();
        let writers = Arc::clone(&writers);
        thread::spawn(move || read_loop(id, stream, events, writers));
    }
}

fn read_loop(id: PlayerId, stream: TcpStream, events: Sender<InboundEvent>, writers: Writers) {
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { break };
        match decode_line::<ClientToServer>(&line) {
            Ok(ClientToServer::Orders(orders)) => {
                if events.send(InboundEvent::OrdersReceived(id, orders.into())).is_err() {
                    return;
                }
            }
            Err(error) => eprintln!("{id} sent something odd: {error}"),
        }
    }
    writers.lock().expect("writers lock poisoned").remove(&id);
    let _ = events.send(InboundEvent::PlayerDisconnected(id));
}
