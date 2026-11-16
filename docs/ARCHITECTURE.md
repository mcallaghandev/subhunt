# How SubHunt is built

This guide assumes you know how to program but are new to Rust, Bevy and hexagonal architecture. Read it alongside the code: every file has comments marked **Rust note** where a new language feature first shows up.

## 1. The big idea: hexagonal architecture

Hexagonal architecture (also called *ports and adapters*, and closely related to *clean architecture*) comes down to one rule:

> **Dependencies point inwards.** Game rules know nothing about engines, networks or file formats. Those details depend on the rules, never the other way round.

```
            ┌───────────────────────────────────────────┐
            │  Adapters (outer ring)                    │
            │   client (Bevy)   net-tcp   net-ws        │
            │   server (main loop)  net-browser protocol│
            │      ┌─────────────────────────────┐      │
            │      │  Application                │      │
            │      │   GameServer, ClientSession │      │
            │      │   ports: ServerTransport,   │      │
            │      │          ClientTransport    │      │
            │      │      ┌───────────────┐      │      │
            │      │      │  Domain       │      │      │
            │      │      │  GameWorld,   │      │      │
            │      │      │  Submarine,   │      │      │
            │      │      │  sonar rules  │      │      │
            │      │      └───────────────┘      │      │
            │      └─────────────────────────────┘      │
            └───────────────────────────────────────────┘
```

Each ring is a separate **crate** (a Rust package) in a Cargo **workspace**. That matters, because Cargo enforces the rule for us: `crates/domain/Cargo.toml` has no dependencies, so the domain *cannot* use Bevy or the network even by accident.

| Crate | Ring | Depends on | Job |
| --- | --- | --- | --- |
| `subhunt-domain` | Domain | nothing | The rules. What a submarine is, how sonar works, what a torpedo hit means. |
| `subhunt-application` | Application | domain | **Use cases** (`GameServer` runs a match, `ClientSession` tracks what a player knows) and **ports** (traits describing what the use cases need from outside). |
| `subhunt-protocol` | Adapter (shared) | application, serde | How messages look as JSON. |
| `subhunt-net-tcp` | Adapter | application, protocol | Implements both transport ports with TCP sockets (desktop). |
| `subhunt-net-ws` | Adapter | application, protocol, tungstenite | Implements the server port with WebSockets, and serves the web page. |
| `subhunt-net-browser` | Adapter | application, protocol, web-sys | Implements the client port with the browser's WebSocket (WebAssembly only). |
| `subhunt-server` | Adapter + composition root | application, net-tcp, net-ws | Opens TCP and WebSocket transports, merges them, builds a world, runs a 20 Hz loop. |
| `subhunt-client` | Adapter + composition root | application, bevy, net-tcp *or* net-browser | Keyboard in, sonar scope out. |

### Ports and adapters

A **port** is a trait (Rust's version of an interface) that the application defines and owns:

```rust
// crates/application/src/ports.rs
pub trait ServerTransport {
    fn poll(&mut self) -> Vec<InboundEvent>;
    fn send(&mut self, to: PlayerId, message: &ServerMessage);
}
```

An **adapter** implements it for real technology:

```rust
// crates/net-tcp/src/server.rs
impl ServerTransport for TcpServerTransport { ... }
```

…and the tests implement it with a plain `Vec`:

```rust
// crates/application/src/server.rs (tests module)
impl ServerTransport for FakeTransport { ... }
```

`GameServer` can't tell the difference. That's why the server's tests run in milliseconds with no sockets.

### Case study: adding the web version

Making the game playable in a browser is a good example of the architecture paying off. Browsers can't open TCP sockets, so we needed WebSockets. Here's everything that changed:

- **Two new adapter crates.** `net-ws` implements `ServerTransport` with WebSockets, and `net-browser` implements `ClientTransport` with the browser's WebSocket API.
- **One small application addition.** `CombinedTransport` merges two transports into one, so TCP and browser players share a match. It's also just a `ServerTransport`. `PlayerIdSource` makes sure the two transports never hand out the same player id.
- **Composition roots.** `server/src/main.rs` now opens both transports. `client/src/connect.rs` picks TCP on desktop and WebSockets in the browser, using `#[cfg(target_arch = "wasm32")]`.

The domain, `GameServer`, `ClientSession`, and the drawing code didn't change at all.

The client is compiled to **WebAssembly** (`wasm32-unknown-unknown`), a compact binary format browsers can run. `wasm-bindgen` generates the JavaScript glue that loads it. Two Rust points come up here:

- **No threads in the browser.** The TCP adapter's background reader threads won't work there. The browser adapter is event-driven instead: the browser calls our closures when messages arrive, and they push into a queue the game drains each frame.
- **Non-`Send` types.** The browser's `WebSocket` can't move between threads, so the client stores `Network` as a Bevy *non-send* resource (`insert_non_send` / `NonSendMut`).

### Case study: practice mode

GitHub Pages can't run a server, so the published page needed a way to play with no server at all. Again, nothing in the domain changed:

- **`ComputerCaptain`** (`application/src/bot.rs`) decides `HelmOrders` from a `SensorPicture`, exactly what a human player gets. It can't cheat, because it never sees the world.
- **`InMemoryTransport`** implements `ServerTransport` with plain queues, and **`LocalMatch`** runs a real `GameServer` on it, with computer captains in the other seats. `LocalMatch` implements `ClientTransport`, so to the Bevy client it looks like any other connection.
- **`PracticeFallback`** wraps a network transport and swaps in a `LocalMatch` if the server never answers.

The only change to a port was a default `update(dt)` method on `ClientTransport`. A match simulated on your own machine needs to be told that time has passed; network transports ignore it.

## 2. Why the players can't see each other (and can't cheat)

This rule shapes the whole design, so it's worth understanding.

**The server is authoritative.** Clients never say "I'm at (x, y)". They send `HelmOrders` ("throttle 0.8, rudder hard left, fire!"). The server simulates everything and decides what happened.

**Each player only receives what their sonar earned.** Every tick, the server calls `GameWorld::sensor_picture(player)`, which builds a `SensorPicture` containing:

- your own ship's exact state,
- your own torpedoes,
- passive **bearings** (directions only) to things you can hear,
- active **fixes** (exact positions), but only in the tick when you pinged.

Other submarines' positions never leave the server unless your sonar detected them. A hacked client has nothing to reveal, because the data was never sent. (Compare games that send everyone's position and rely on the client to hide it. Those are the ones that get wallhacks.)

The server also never trusts input: `HelmOrders::sanitised()` clamps every value and replaces `NaN` with zero.

## 3. Follow a keypress through the system

Here's what happens when you press `SPACE` to ping.

1. **Bevy input adapter**, `client/src/helm.rs`: `read_keyboard` sees `just_pressed(Space)` and sets `helm.ping_requested = true`. Clicking or tapping the PING button on the panel (`client/src/panel/controls.rs`) does the same. Both write to the same `Helm`, and nothing after this step knows which one was used.
2. **Client network system**, `client/src/network.rs`: 20 times a second, `send_orders` calls `helm.take_orders()`, which builds a `HelmOrders` and clears the latch, then calls `transport.send_orders(&orders)`.
3. **TCP adapter**, `net-tcp/src/client.rs`: converts to the `ClientToServer` DTO, encodes one JSON line, and writes it to the socket:
   `{"Orders":{"throttle":0.5,"rudder":0.0,"fire_torpedo":false,"sonar_ping":true}}`
4. **Server TCP adapter**, `net-tcp/src/server.rs`: that connection's reader thread decodes the line and sends `InboundEvent::OrdersReceived(id, orders)` down a channel.
5. **Use case**, `application/src/server.rs`: on its next tick, `GameServer::tick` polls the transport and calls `world.set_orders(id, orders)`.
6. **Domain**, `domain/src/world.rs`: `GameWorld::step` lets the sub consume the ping order (if its cooldown allows), finds everything in range, stores the echoes, and emits `GameEvent::PingDetected` for each target.
7. **Use case** again: turns `PingDetected` into `Notice::Pinged` for the target, then sends every player their `SensorPicture`. Yours includes `Contact::Fix` echoes.
8. **Client**: the reader thread decodes the picture. `receive_from_server` hands it to `ClientSession::handle`, which remembers the echoes so they can fade out.
9. **Output adapter**, `client/src/panel/draw.rs`: the `sonar` function draws a green circle on the panel for each echo, brighter or dimmer depending on `echo.intensity()`.

Steps 5–7 are pure Rust you can test without a network, and step 6 contains every game rule.

## 4. Bevy in a nutshell

Bevy is built on **ECS** (Entity Component System):

- **Resources** are global, one-of-a-kind data. We use `Session`, `Helm` and `Network`.
- **Entities** are IDs that **Components** are attached to. The HUD text is an entity with `Text`, `Node` and our `StatusText` marker component.
- **Systems** are plain functions. Their parameters declare what they need, and Bevy calls them with those values filled in:

  ```rust
  fn send_orders(mut network: ResMut<Network>, mut helm: ResMut<Helm>, session: Res<Session>) { ... }
  ```

  `Res<T>` borrows a resource read-only. `ResMut<T>` borrows it mutably. Bevy uses this to run systems in parallel safely.
- **Schedules** decide when systems run. `Startup` runs once. `Update` runs every frame. `FixedUpdate` runs at a fixed rate (we set 20 Hz for sending orders).
- **Plugins** bundle systems and resources. Each client module exposes one (`HelmPlugin`, `NetworkPlugin` and so on), and `main.rs` adds them.

We barely use Bevy for drawing. The instrument panel is a **software framebuffer**: `client/src/panel/canvas.rs` keeps the pixels in a plain `Vec<u8>` and draws lines, circles and text into it with classic algorithms (Bresenham lines, midpoint circles, a hand-made 3×5 pixel font). Every frame, `panel/draw.rs` repaints the panel from the session, and `panel_plugin.rs` hands the pixels to Bevy as a texture shown by a single UI image, scaled up with "nearest" sampling so each pixel stays a crisp square.

That's how games were drawn in the 1980s, and it suits a learner well:

- **It's ordinary Rust.** Loops, slices, integer maths and no GPU concepts.
- **It's testable.** The panel tests draw whole panels and press buttons without opening a window.
- **One layout table drives both drawing and input.** `panel/layout.rs` says where each instrument is, so what you see is exactly what you can press.

Note that **the server doesn't use Bevy at all.** The game rules don't need an engine, so the server is a plain loop. That's the architecture paying off.

## 5. Rust concepts you'll meet (and where)

| Concept | What it means | Where to look |
| --- | --- | --- |
| Ownership and borrowing | Each value has one owner. `&T` lends it read-only, `&mut T` lends it with exclusive write access. The compiler checks this, so there are no data races and no use-after-free. | `Submarine::integrate(&mut self, dt, rules: &GameRules)` |
| Borrow-checker workarounds | You can't mutate a collection while also calling `&mut self` methods. Collect what you need into a list first, then act. | `GameWorld::step` (collects launches and pingers) |
| `struct` and `impl` | Data plus methods. There are no classes or inheritance. | `domain/src/submarine.rs` |
| `enum` and `match` | Enums can carry data per variant, and `match` must handle every case. | `Contact`, `ServerMessage`, `GameServer::tick` |
| `Option<T>` | Replaces null: `Some(x)` or `None`. | `GameWorld::sensor_picture` returns `Option<SensorPicture>` |
| `Result<T, E>` and `?` | Errors are values. `?` returns early with the error. | `TcpServerTransport::bind` |
| Traits | Interfaces. Also how operators work (`impl Add for Vec2`). | `ports.rs`, `math.rs` |
| Generics vs `dyn Trait` | `GameServer<T: ServerTransport>` is decided at compile time. `Box<dyn ClientTransport>` is decided at runtime. | `server.rs` vs `client/src/network.rs` |
| `derive` | Auto-generate common trait implementations. | Nearly every type |
| `From` / `Into` | The idiomatic way to convert types. | `protocol/src/mapping.rs` |
| Newtype pattern | Wrap a type to give it meaning (`PlayerId(u32)`) or to satisfy the orphan rule. | `ids.rs`, `client/src/session.rs` |
| Threads, channels, `Arc<Mutex<T>>` | Background reader threads push messages down `mpsc` channels; shared state goes behind `Arc<Mutex<…>>`. | `net-tcp/src/server.rs` |
| `Send` / `Sync` | Marker traits saying a type is safe to move or share between threads. Bevy resources need both. | `TcpClientTransport` (`Mutex<Receiver>`) |
| `Drop` | Destructors. | `TcpClientTransport` closes its socket |
| Tests | `#[cfg(test)] mod tests` sits next to the code. `tests/` holds integration tests. | Every crate |

Handy commands:

```sh
cargo check                       # fast type-check, no binary built
cargo test --workspace            # run all tests
cargo test -p subhunt-domain      # test one crate
cargo clippy --workspace          # the linter: great for learning idioms
cargo fmt                         # auto-format
cargo doc --open --no-deps        # browse the docs generated from the /// comments
```

## 6. Where to take it next

Ideas roughly in order of difficulty. Each tells you which layer to touch, which is good practice for the architecture.

1. **Tune the feel.** Every number is in `domain/src/rules.rs`. Try a smaller world or a longer ping cooldown. *(Domain only.)*
2. **Smarter or dumber bots.** Give `ComputerCaptain` difficulty levels, or teach it to slow down and listen. *(Application: `bot.rs`.)*
3. **Player names.** Add a `Hello { name }` client message. *(Protocol, then application, then the domain stores names.)*
4. **Depth.** Give submarines a depth. Deeper is quieter but slower, and torpedoes must match depth to hit. *(Mostly domain, plus the HUD.)*
5. **Bearing uncertainty.** Make passive bearings wobble more for faint contacts. *(Domain: `sensor_picture`.)*
6. **Islands and terrain.** Add obstacles that block movement and sonar. *(Domain for the rules; the client draws the coastline.)*
7. **Decoys.** A noisemaker that looks like a submarine on passive sonar. *(Domain plus a new key in `helm.rs`.)*
8. **Smoother movement.** Interpolate your own ship between server pictures (client-side prediction). *(Client and `ClientSession`.)*
9. **UDP networking.** Write a `net-renet` or `net-lightyear` crate implementing the same two ports, then change the composition roots to use it, just as the web version did with WebSockets.
10. **Lobby and match rounds.** A new use case in the application layer that wraps `GameServer`.

### Sound without sound files

`client/src/sound/` builds every effect from scratch. `synth.rs` makes samples from sine waves, noise, envelopes, a low-pass filter and a resonator, then packs them into a WAV file in memory. `cues.rs` decides when to play them by comparing what the sonar knew a moment ago with what it knows now. For example, if the torpedo reload timer jumped up, we just fired. Both are plain Rust with tests. `audio_plugin.rs` is the small Bevy adapter that plays the results, and it changes the engine loop's volume and pitch as you speed up.

### Lessons from making it work on phones

Two bugs only appeared on a real Android phone (Chrome, Qualcomm Adreno graphics), never in desktop browsers or the software renderer used for testing. Both are worth knowing about:

- **Don't give a texture that changes every frame its own sampler.** Bevy creates a new GPU sampler whenever such an image changes. Phone drivers allow only a few thousand, so Chrome shut the graphics down ("WebGL context lost"). The panel uses the app-wide default sampler instead, set to "nearest" with `ImagePlugin::default_nearest()`.
- **Bevy's sprite renderer drew the panel as one flat colour on that phone**, while the UI renderer worked. That's why the panel is a UI image.

The `?debug` page option shows Bevy's log on screen, which is how both were found. When something works on desktop but not on a phone, start there.

## 7. Known limitations (deliberate, to keep things simple)

- **TCP, WebSockets and JSON.** Easy to read and debug, but not ideal for games over the internet. See item 8 above.
- **Polling WebSocket threads.** Each browser connection's thread wakes every 5 ms to check for outgoing messages. That's simple, but uses a little CPU per player.
- **Plain HTTP only.** To host the web version publicly over HTTPS you'd put it behind a reverse proxy such as Caddy or nginx. The client already switches to `wss://` on HTTPS pages.
- **No client-side prediction.** Your ship moves when the server says so, so you'll feel any latency.
- **No reconnection.** If the connection drops, restart the client.
- **Thread per connection** on the server (for both TCP and WebSockets). Fine for a handful of players, not for hundreds.
