//! # SubHunt client
//!
//! The Bevy app. In hexagonal terms this crate is a bundle of *adapters*:
//!
//! * `helm`: keyboard → `HelmOrders` (an input adapter)
//! * `panel` and `panel_plugin`: the instrument panel. It shows what the
//!   session knows (output) and its levers and buttons, worked by mouse or
//!   touch, are also helm controls (input)
//! * `network` and `connect`: wire a transport to the `ClientSession` use case
//!
//! plus this file, the *composition root*, which plugs them together.
//!
//! The same code builds for the desktop (TCP) and for the web (WebAssembly
//! plus WebSockets). Only `connect.rs` and the window settings differ.
//!
//! ## Bevy in 30 seconds
//!
//! Bevy uses **ECS** (Entity Component System):
//! * **Resources** are global singletons (`Session`, `Helm`, `Network`).
//! * **Components** are data attached to entities (the panel's sprite).
//! * **Systems** are ordinary functions. Their *parameters* say what they need
//!   (`Res<Session>`, `ResMut<Helm>`, `Query<&mut Text>`), and Bevy calls them
//!   every frame with those things filled in.
//! * **Plugins** group related systems and resources together.
//!
//! Desktop: `cargo run -p subhunt-client -- 127.0.0.1:7777`
//! Web: see `scripts/build-web.sh` and the README.

mod connect;
mod helm;
mod network;
mod panel;
mod panel_plugin;
mod session;

use bevy::prelude::*;
use subhunt_application::ClientSession;

/// How often we send helm orders to the server (matches the server tick rate).
const ORDERS_PER_SECOND: f64 = 20.0;

fn main() -> AppExit {
    // Connect before opening a window, so a typo in the address fails fast.
    let network = match connect::connect() {
        Ok(network) => network,
        Err(error) => {
            eprintln!("{error}");
            return AppExit::error();
        }
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(window()), ..default() }))
        // The colour round the edges when the window isn't the panel's shape.
        .insert_resource(ClearColor(Color::srgb_u8(22, 26, 30)))
        // `FixedUpdate` systems run at this rate regardless of frame rate.
        .insert_resource(Time::<Fixed>::from_hz(ORDERS_PER_SECOND))
        .insert_resource(session::Session(ClientSession::new()))
        .insert_non_send(network)
        .add_plugins((helm::HelmPlugin, network::NetworkPlugin, panel_plugin::PanelPlugin))
        .run()
}

#[cfg(not(target_arch = "wasm32"))]
fn window() -> Window {
    Window { title: "SubHunt".into(), resolution: bevy::window::WindowResolution::new(960, 540), ..default() }
}

/// In a browser, draw into the page's `<canvas id="subhunt">` and resize with it.
#[cfg(target_arch = "wasm32")]
fn window() -> Window {
    Window { title: "SubHunt".into(), canvas: Some("#subhunt".into()), fit_canvas_to_parent: true, ..default() }
}
