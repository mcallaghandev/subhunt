//! # Browser WebSocket adapter (client side)
//!
//! When the client is compiled to WebAssembly and runs in a web page, it
//! can't use `std::net` or threads. Instead we use the browser's own
//! `WebSocket` object through `web-sys`, the Rust bindings for browser APIs.
//!
//! The browser calls us back when a message arrives (an event-driven model,
//! like JavaScript). Our callbacks push messages into a queue, and the game
//! drains the queue each frame through the same [`ClientTransport`] port the
//! desktop TCP adapter implements.
//!
//! [`ClientTransport`]: subhunt_application::ClientTransport

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use subhunt_application::{ClientInbound, ClientTransport};
use subhunt_domain::HelmOrders;
use subhunt_protocol::{ClientToServer, ServerToClient, decode_line, encode_line};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Event, MessageEvent, WebSocket};

/// Messages waiting for the game to pick them up.
///
/// Rust note: `Rc<RefCell<T>>` is the single-threaded cousin of
/// `Arc<Mutex<T>>`: shared ownership (`Rc`) plus the ability to mutate
/// through a shared reference (`RefCell`). Web pages are single-threaded,
/// so we don't need the thread-safe versions.
type Inbox = Rc<RefCell<VecDeque<ClientInbound>>>;

pub struct BrowserWsTransport {
    socket: WebSocket,
    inbox: Inbox,
    // The browser holds pointers to these callbacks, so we must keep them
    // alive for as long as the socket exists. Dropping them would free them.
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_close: Closure<dyn FnMut(Event)>,
}

impl BrowserWsTransport {
    /// The game server's address, worked out from the page's own address.
    /// The page and the game are served from the same place, so if the page
    /// is `http://host:8080/`, the game is at `ws://host:8080/ws`.
    pub fn same_origin_url() -> Option<String> {
        let location = web_sys::window()?.location();
        let scheme = if location.protocol().ok()? == "https:" { "wss" } else { "ws" };
        Some(format!("{scheme}://{}/ws", location.host().ok()?))
    }

    /// Reads a setting for the game from the page, in priority order:
    /// a URL query parameter (`?server=...`), then a global variable set by
    /// the page's own script (`window.SUBHUNT_SERVER = "..."`).
    /// Empty values count as "not set".
    pub fn page_setting(query_name: &str, global_name: &str) -> Option<String> {
        let window = web_sys::window()?;
        let from_query = window
            .location()
            .href()
            .ok()
            .and_then(|href| web_sys::Url::new(&href).ok())
            .and_then(|url| url.search_params().get(query_name));
        let from_global =
            || js_sys::Reflect::get(&window, &global_name.into()).ok().and_then(|value| value.as_string());
        from_query.or_else(from_global).filter(|value| !value.trim().is_empty())
    }

    /// Whether the page's address has a query parameter, such as `?practice`.
    pub fn page_has_flag(name: &str) -> bool {
        web_sys::window()
            .and_then(|window| window.location().href().ok())
            .and_then(|href| web_sys::Url::new(&href).ok())
            .is_some_and(|url| url.search_params().has(name))
    }

    /// A random seed from the browser (there's no system clock to use in WebAssembly).
    pub fn random_seed() -> u64 {
        (js_sys::Math::random() * u64::MAX as f64) as u64
    }

    /// Starts connecting. The connection opens in the background; if it
    /// fails, `poll` reports `ConnectionLost`.
    pub fn connect(url: &str) -> Result<Self, String> {
        let socket = WebSocket::new(url).map_err(|e| format!("{e:?}"))?;
        let inbox = Inbox::default();

        let messages = Rc::clone(&inbox);
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let Some(text) = event.data().as_string() else { return };
            if let Ok(message) = decode_line::<ServerToClient>(&text) {
                messages.borrow_mut().push_back(ClientInbound::Message(message.into()));
            }
        });

        let closes = Rc::clone(&inbox);
        let on_close = Closure::<dyn FnMut(Event)>::new(move |_event: Event| {
            closes.borrow_mut().push_back(ClientInbound::ConnectionLost);
        });

        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        // A failed connection fires `error` then `close`; listening for
        // `close` alone covers both.
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        Ok(Self { socket, inbox, _on_message: on_message, _on_close: on_close })
    }
}

impl ClientTransport for BrowserWsTransport {
    fn poll(&mut self) -> Vec<ClientInbound> {
        self.inbox.borrow_mut().drain(..).collect()
    }

    fn send_orders(&mut self, orders: &HelmOrders) {
        if self.socket.ready_state() == WebSocket::OPEN {
            let _ = self.socket.send_with_str(&encode_line(&ClientToServer::from(orders)));
        }
    }
}

impl Drop for BrowserWsTransport {
    fn drop(&mut self) {
        self.socket.set_onmessage(None);
        self.socket.set_onclose(None);
        let _ = self.socket.close();
    }
}
