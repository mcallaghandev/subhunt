//! # SubHunt client
//!
//! The Bevy app. In hexagonal terms this crate is a bundle of *adapters*:
//!
//! * `helm`: keyboard → `HelmOrders` (an input adapter)
//! * `network`: wires the TCP transport to the `ClientSession` use case
//! * `sonar_display` and `hud`: draw what the session knows (output adapters)
//!
//! plus this file, the *composition root*, which plugs them together.
//!
//! ## Bevy in 30 seconds
//!
//! Bevy uses **ECS** (Entity Component System):
//! * **Resources** are global singletons (`Session`, `Helm`, `Network`).
//! * **Components** are data attached to entities (the HUD text node).
//! * **Systems** are ordinary functions. Their *parameters* say what they need
//!   (`Res<Session>`, `ResMut<Helm>`, `Query<&mut Text>`), and Bevy calls them
//!   every frame with those things filled in.
//! * **Plugins** group related systems and resources together.
//!
//! Run with: `cargo run -p subhunt-client -- 127.0.0.1:7777`

mod helm;
mod hud;
mod network;
mod session;
mod sonar_display;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use subhunt_application::ClientSession;
use subhunt_net_tcp::TcpClientTransport;

const DEFAULT_SERVER: &str = "127.0.0.1:7777";
/// How often we send helm orders to the server (matches the server tick rate).
const ORDERS_PER_SECOND: f64 = 20.0;

fn main() -> AppExit {
    let server = std::env::args().nth(1).unwrap_or_else(|| DEFAULT_SERVER.to_string());

    // Connect before opening a window, so a typo in the address fails fast.
    let transport = match TcpClientTransport::connect(&server) {
        Ok(transport) => transport,
        Err(error) => {
            eprintln!("Could not connect to {server}: {error}");
            eprintln!("Is the server running? Start it with: cargo run -p subhunt-server");
            return AppExit::error();
        }
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "SubHunt".into(),
                resolution: WindowResolution::new(900, 900),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.01, 0.03, 0.02)))
        // `FixedUpdate` systems run at this rate regardless of frame rate.
        .insert_resource(Time::<Fixed>::from_hz(ORDERS_PER_SECOND))
        .insert_resource(session::Session(ClientSession::new()))
        .insert_resource(network::Network::new(transport))
        .add_plugins((helm::HelmPlugin, network::NetworkPlugin, sonar_display::SonarDisplayPlugin, hud::HudPlugin))
        .run()
}
