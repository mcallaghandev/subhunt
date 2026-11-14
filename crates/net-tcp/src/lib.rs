//! # TCP network adapter
//!
//! Implements the application's [`ServerTransport`] and [`ClientTransport`]
//! ports using plain `std::net` TCP sockets and background threads.
//!
//! ## Why TCP (for now)?
//!
//! It's in the standard library and it's reliable and ordered, which makes it
//! simple. The downside for games is *head-of-line blocking*: one lost packet
//! delays every packet behind it. Fine on a LAN; noticeable over a bad link.
//!
//! Because the game only talks to the ports, moving to UDP later (for example
//! with the `renet` or `lightyear` crates) means writing a new adapter crate
//! like this one. The domain and application don't change at all.
//!
//! ## How it works
//!
//! Reading from a socket *blocks* (waits) until data arrives. We can't let the
//! game loop wait, so each connection gets a reader thread that turns lines into
//! events and pushes them down a channel. The game polls the channel each tick.
//!
//! [`ServerTransport`]: subhunt_application::ServerTransport
//! [`ClientTransport`]: subhunt_application::ClientTransport

mod client;
mod server;

pub use client::TcpClientTransport;
pub use server::TcpServerTransport;
