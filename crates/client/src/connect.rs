//! Chooses and opens a network transport for this platform.
//!
//! Rust note: `#[cfg(...)]` includes code only when the condition is true at
//! compile time. Only one of these two functions exists in any given build.

use crate::network::Network;

/// Desktop: connect over TCP to the address on the command line.
#[cfg(not(target_arch = "wasm32"))]
pub fn connect() -> Result<Network, String> {
    const DEFAULT_SERVER: &str = "127.0.0.1:7777";
    let server = std::env::args().nth(1).unwrap_or_else(|| DEFAULT_SERVER.to_string());
    subhunt_net_tcp::TcpClientTransport::connect(&server).map(Network::new).map_err(|error| {
        format!(
            "Could not connect to {server}: {error}\nIs the server running? Start it with: cargo run -p subhunt-server"
        )
    })
}

/// Browser: open a WebSocket back to the server that served this page.
#[cfg(target_arch = "wasm32")]
pub fn connect() -> Result<Network, String> {
    use subhunt_net_browser::BrowserWsTransport;
    let url = BrowserWsTransport::same_origin_url().ok_or("could not read the page's address")?;
    BrowserWsTransport::connect(&url).map(Network::new)
}
