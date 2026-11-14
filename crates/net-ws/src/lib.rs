//! # WebSocket network adapter (server side)
//!
//! Browsers can't open raw TCP sockets, but they can open **WebSockets**: a
//! message-based connection that starts life as an HTTP request. This crate
//! implements the same [`ServerTransport`] port as the TCP adapter, so the
//! game server doesn't change at all.
//!
//! As a convenience, the same port also serves the web client's files
//! (`index.html`, the `.wasm` binary and its JavaScript glue). Open
//! `http://<server>:8080` in a browser and the page connects back to the
//! same address for the game. One port, no separate web server needed.
//!
//! [`ServerTransport`]: subhunt_application::ServerTransport

mod server;
mod static_files;

pub use server::WsServerTransport;
