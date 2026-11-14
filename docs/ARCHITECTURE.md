# How SubHunt is built

This guide assumes you know how to program but are new to Rust, Bevy and hexagonal architecture. Read it alongside the code: every file has comments marked **Rust note** where a new language feature first shows up.

## 1. The big idea: hexagonal architecture

Hexagonal architecture (also called *ports and adapters*, and closely related to *clean architecture*) comes down to one rule:

> **Dependencies point inwards.** Game rules know nothing about engines, networks or file formats. Those details depend on the rules, never the other way round.

```
            ┌───────────────────────────────────────────┐
            │  Adapters (outer ring)                    │
            │   client (Bevy)   net-tcp   protocol      │
            │   server (main loop)                      │
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
| `subhunt-net-tcp` | Adapter | application, protocol | Implements the transport ports with TCP sockets. |
| `subhunt-server` | Adapter + composition root | application, net-tcp | Picks TCP, builds a world, runs a 20 Hz loop. |
| `subhunt-client` | Adapter + composition root | application, net-tcp, bevy | Keyboard in, sonar scope out. |

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

`GameServer` can't tell the difference. That's why the server's tests run in milliseconds with no sockets, and why switching from TCP to UDP later means writing one new crate and changing one line in `server/src/main.rs`.

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

1. **Bevy input adapter**, `client/src/helm.rs`: `read_keyboard` sees `just_pressed(Space)` and sets `helm.ping_requested = true`.
2. **Client network system**, `client/src/network.rs`: 20 times a second, `send_orders` calls `helm.take_orders()`, which builds a `HelmOrders` and clears the latch, then calls `transport.send_orders(&orders)`.
3. **TCP adapter**, `net-tcp/src/client.rs`: converts to the `ClientToServer` DTO, encodes one JSON line, and writes it to the socket:
   `{"Orders":{"throttle":0.5,"rudder":0.0,"fire_torpedo":false,"sonar_ping":true}}`
4. **Server TCP adapter**, `net-tcp/src/server.rs`: that connection's reader thread decodes the line and sends `InboundEvent::OrdersReceived(id, orders)` down a channel.
5. **Use case**, `application/src/server.rs`: on its next tick, `GameServer::tick` polls the transport and calls `world.set_orders(id, orders)`.
6. **Domain**, `domain/src/world.rs`: `GameWorld::step` lets the sub consume the ping order (if its cooldown allows), finds everything in range, stores the echoes, and emits `GameEvent::PingDetected` for each target.
7. **Use case** again: turns `PingDetected` into `Notice::Pinged` for the target, then sends every player their `SensorPicture`. Yours includes `Contact::Fix` echoes.
8. **Client**: the reader thread decodes the picture. `receive_from_server` hands it to `ClientSession::handle`, which remembers the echoes so they can fade out.
9. **Bevy output adapter**, `client/src/sonar_display.rs`: `draw_sonar` draws a green circle for each echo, with brightness from `echo.intensity()`.

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

We draw everything with **gizmos** (`gizmos.circle_2d`, `gizmos.line_2d`). They're redrawn every frame from the session data, which is great for prototyping. Later you might replace them with sprites and shaders for a nicer look.

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
2. **Player names.** Add a `Hello { name }` client message. *(Protocol, then application, then the domain stores names.)*
3. **Depth.** Give submarines a depth. Deeper is quieter but slower, and torpedoes must match depth to hit. *(Mostly domain, plus the HUD.)*
4. **Bearing uncertainty.** Make passive bearings wobble more for faint contacts. *(Domain: `sensor_picture`.)*
5. **Islands and terrain.** Add obstacles that block movement and sonar. *(Domain for the rules; the client draws the coastline.)*
6. **Decoys.** A noisemaker that looks like a submarine on passive sonar. *(Domain plus a new key in `helm.rs`.)*
7. **Smoother movement.** Interpolate your own ship between server pictures (client-side prediction). *(Client and `ClientSession`.)*
8. **UDP networking.** Write a `net-renet` or `net-lightyear` crate implementing the same two ports, then change the composition roots to use it. Nothing else changes. That's the architecture working.
9. **Lobby and match rounds.** A new use case in the application layer that wraps `GameServer`.

## 7. Known limitations (deliberate, to keep things simple)

- **TCP and JSON.** Easy to read and debug, but not ideal for games over the internet. See item 8 above.
- **No client-side prediction.** Your ship moves when the server says so, so you'll feel any latency.
- **No reconnection.** If the connection drops, restart the client.
- **Thread per connection** on the server. Fine for a handful of players, not for hundreds.
