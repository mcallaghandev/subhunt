use std::io::{self, BufRead, BufReader, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use subhunt_application::{ClientInbound, ClientTransport};
use subhunt_domain::HelmOrders;
use subhunt_protocol::{ClientToServer, ServerToClient, decode_line, encode_line};

pub struct TcpClientTransport {
    stream: TcpStream,
    // Rust note: Bevy resources must be `Sync` (safe to share between threads).
    // A channel `Receiver` isn't, but a `Mutex<Receiver>` is.
    inbound: Mutex<Receiver<ClientInbound>>,
}

impl TcpClientTransport {
    pub fn connect(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        let reader = stream.try_clone()?;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || read_loop(reader, sender));
        Ok(Self { stream, inbound: Mutex::new(receiver) })
    }
}

impl ClientTransport for TcpClientTransport {
    fn poll(&mut self) -> Vec<ClientInbound> {
        // `get_mut` skips locking: having `&mut self` already proves nobody else
        // is using it.
        let receiver = self.inbound.get_mut().expect("inbound lock poisoned");
        receiver.try_iter().collect()
    }

    fn send_orders(&mut self, orders: &HelmOrders) {
        let line = encode_line(&ClientToServer::from(orders));
        // Errors are ignored here; the reader thread reports the lost connection.
        let _ = self.stream.write_all(line.as_bytes());
    }
}

/// Rust note: `Drop` is Rust's destructor; it runs when the value goes away.
/// The reader thread holds a clone of the socket, so simply dropping our copy
/// would leave the connection open. Shutting it down tells the server we've gone.
impl Drop for TcpClientTransport {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

fn read_loop(stream: TcpStream, inbound: Sender<ClientInbound>) {
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { break };
        match decode_line::<ServerToClient>(&line) {
            Ok(message) => {
                if inbound.send(ClientInbound::Message(message.into())).is_err() {
                    return; // the game has closed
                }
            }
            Err(error) => eprintln!("server sent something odd: {error}"),
        }
    }
    let _ = inbound.send(ClientInbound::ConnectionLost);
}
