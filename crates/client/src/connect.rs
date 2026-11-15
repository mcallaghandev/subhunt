//! Chooses and opens a transport for this platform: a real server, or a
//! practice match against computer subs running on this machine.
//!
//! Rust note: `#[cfg(...)]` includes code only when the condition is true at
//! compile time. Only one of these two `connect` functions exists in any build.

use subhunt_application::LocalMatch;

use crate::network::Network;

/// How many computer subs join a practice match.
const PRACTICE_OPPONENTS: usize = 3;

/// Desktop:
/// * `subhunt-client` connects to a server on this machine,
/// * `subhunt-client 192.168.1.20:7777` connects to that server,
/// * `subhunt-client --practice` plays offline against computer subs.
#[cfg(not(target_arch = "wasm32"))]
pub fn connect() -> Result<Network, String> {
    const DEFAULT_SERVER: &str = "127.0.0.1:7777";
    let argument = std::env::args().nth(1);
    if argument.as_deref() == Some("--practice") {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);
        return Ok(Network::new(LocalMatch::new(LocalMatch::practice_rules(), PRACTICE_OPPONENTS, seed)));
    }
    let server = argument.unwrap_or_else(|| DEFAULT_SERVER.to_string());
    subhunt_net_tcp::TcpClientTransport::connect(&server).map(Network::new).map_err(|error| {
        format!(
            "Could not connect to {server}: {error}\n\
             Is the server running? Start it with: cargo run -p subhunt-server\n\
             Or play offline with: cargo run -p subhunt-client -- --practice"
        )
    })
}

/// Browser:
/// * `?practice` in the address plays offline straight away,
/// * otherwise connect to `?server=...`, or `window.SUBHUNT_SERVER` from the
///   page, or the server that served the page,
/// * and if that server can't be reached (always the case on GitHub Pages),
///   fall back to a practice match.
#[cfg(target_arch = "wasm32")]
pub fn connect() -> Result<Network, String> {
    use subhunt_application::PracticeFallback;
    use subhunt_net_browser::BrowserWsTransport;

    let seed = BrowserWsTransport::random_seed();
    let practice = || Network::new(LocalMatch::new(LocalMatch::practice_rules(), PRACTICE_OPPONENTS, seed));

    if BrowserWsTransport::page_has_flag("practice") {
        return Ok(practice());
    }
    let url = BrowserWsTransport::page_setting("server", "SUBHUNT_SERVER").or_else(BrowserWsTransport::same_origin_url);
    match url.map(|url| BrowserWsTransport::connect(&url)) {
        Some(Ok(online)) => Ok(Network::new(PracticeFallback::new(online, PRACTICE_OPPONENTS, seed))),
        _ => Ok(practice()),
    }
}
