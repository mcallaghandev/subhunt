//! # SubHunt application layer
//!
//! The ring around the domain. It holds:
//!
//! * **Use cases**: the things the program *does*. [`GameServer`] runs a
//!   match; [`ClientSession`] keeps a player's view of the world up to date.
//! * **Ports**: traits ([`ServerTransport`], [`ClientTransport`]) describing
//!   what the use cases need from the outside world, *without saying how*.
//!   Adapters in other crates (TCP today, maybe UDP tomorrow) implement them.
//!
//! Rust note: a `trait` is like an interface in C# or Java. Code written
//! against a trait works with any type that implements it, which is exactly
//! what "ports and adapters" means.

mod client;
mod combined;
mod messages;
mod ports;
mod server;

pub use client::{ClientSession, ConnectionStatus, Echo, TimedNotice};
pub use combined::{CombinedTransport, PlayerIdSource};
pub use messages::{Notice, ServerMessage};
pub use ports::{ClientInbound, ClientTransport, InboundEvent, ServerTransport};
pub use server::{GameServer, TickReport};
