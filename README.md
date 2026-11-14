# SubHunt

A 2D top-down multiplayer submarine game written in Rust with [Bevy](https://bevyengine.org/).

![Two clients side by side: one running at full speed with a torpedo in the water, the other just after a sonar ping](docs/screenshot.png)

You can't see the other players. All you have is a sonar scope:

- **Passive sonar** (always on) tells you the **direction** of anything noisy, but not how far away it is. Going fast makes you louder *and* deafens your own sonar.
- **Active sonar** (`SPACE`) pings: you get an exact **position** for everything in range, but the ping is loud. Everyone nearby now knows where you are, and anyone you pinged is told they've been spotted.
- **Torpedoes** (`F`) run straight ahead. Line up a shot using bearings and pings, then sink them before they sink you.

## Quick start

You need a recent Rust toolchain (Bevy 0.20 needs Rust 1.97.1 or newer):

```sh
rustup update stable
```

On Linux you also need a few system libraries ([Bevy's Linux dependency guide](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md)). On Ubuntu or Debian:

```sh
sudo apt install pkg-config libudev-dev libx11-dev libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev
```

Then, in three terminals:

```sh
cargo run -p subhunt-server                     # terminal 1: the server
cargo run -p subhunt-client                     # terminal 2: player one
cargo run -p subhunt-client                     # terminal 3: player two
```

To play across a network, run the server on one machine and pass its address to each client:

```sh
cargo run -p subhunt-client -- 192.168.1.20:7777
```

The first build takes a few minutes because it compiles Bevy. Later builds are quick.

### Controls

| Key | Action |
| --- | --- |
| `W` / `S` (or arrow keys) | Throttle up / down (the lever stays where you leave it) |
| `A` / `D` (or arrow keys) | Rudder left / right (only while held) |
| `X` | All stop |
| `SPACE` | Active sonar ping |
| `F` / `Enter` | Fire torpedo |

### Reading the scope

- **White triangle** in the centre: you. The scope is north-up.
- **Orange lines**: passive bearings to submarines. Brighter means clearer.
- **Red lines**: passive bearings to enemy torpedoes. Turn and run!
- **Green circles**: ping echoes (exact positions). They fade over six seconds.
- **Yellow dots**: your own torpedoes.
- **Red edge**: the boundary of the play area.

## Running the tests

```sh
cargo test --workspace
```

The game rules, server logic and client logic are all tested without opening a window or a socket. That's one of the payoffs of the architecture.

## Project layout

```
crates/
  domain/        Game rules: submarines, sonar, torpedoes. No dependencies at all.
  application/   Use cases (GameServer, ClientSession) and ports (transport traits).
  protocol/      Wire format: JSON messages shared by both network adapters.
  net-tcp/       Network adapter: implements the ports over TCP.
  server/        Headless server binary (no Bevy needed).
  client/        Bevy client binary: input, sonar display, HUD.
docs/
  ARCHITECTURE.md   How it fits together, and a Rust primer for this codebase.
```

**New to Rust or hexagonal architecture?** Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). It walks through how the layers fit together, follows one keypress all the way to the screen, and explains the Rust features as they come up.
