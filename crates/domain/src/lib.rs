//! # SubHunt domain
//!
//! This crate is the *centre* of the hexagon: the rules of the game.
//!
//! It knows about submarines, torpedoes and sonar. It does **not** know about
//! Bevy, windows, keyboards, TCP or JSON. That makes it:
//!
//! * easy to test (plain `cargo test`, no window or server needed),
//! * reusable (the headless server and the Bevy client both use it),
//! * stable (swapping the network library never touches game rules).
//!
//! Rust note: `mod foo;` tells the compiler "there is a module in `foo.rs`".
//! Modules are private by default; `pub use` re-exports the bits we want other
//! crates to see, so they can write `subhunt_domain::Submarine` instead of
//! `subhunt_domain::submarine::Submarine`.

mod controls;
mod ids;
mod math;
mod rng;
mod rules;
mod sonar;
mod submarine;
mod torpedo;
mod world;

pub use controls::HelmOrders;
pub use ids::{PlayerId, TorpedoId};
pub use math::{Vec2, compass_degrees, wrap_angle};
pub use rules::GameRules;
pub use sonar::{Contact, ContactKind, passive_strength};
pub use submarine::Submarine;
pub use torpedo::Torpedo;
pub use world::{GameEvent, GameWorld, OwnShipStatus, SensorPicture, TorpedoTrack};
