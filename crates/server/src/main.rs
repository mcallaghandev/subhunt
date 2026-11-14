//! # SubHunt dedicated server
//!
//! This is a *composition root*: the one place where concrete pieces get
//! plugged together. We pick the TCP adapter, build a world, hand both to the
//! `GameServer` use case, and run a fixed-rate loop.
//!
//! Run it with: `cargo run -p subhunt-server -- 0.0.0.0:7777`

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use subhunt_application::GameServer;
use subhunt_domain::{GameEvent, GameRules, GameWorld};
use subhunt_net_tcp::TcpServerTransport;

const DEFAULT_ADDRESS: &str = "0.0.0.0:7777";
/// Simulation steps per second. Every player gets a sensor picture each tick.
const TICK_RATE_HZ: f64 = 20.0;

fn main() -> std::io::Result<()> {
    let address = std::env::args().nth(1).unwrap_or_else(|| DEFAULT_ADDRESS.to_string());

    let transport = TcpServerTransport::bind(&address)?;
    println!("SubHunt server listening on {}", transport.local_addr());

    // A different seed each run gives different spawn points.
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    let world = GameWorld::new(GameRules::default(), seed);
    let mut server = GameServer::new(world, transport);

    // A fixed timestep keeps the simulation identical regardless of how fast
    // the machine is. We sleep until the next tick is due.
    let tick = Duration::from_secs_f64(1.0 / TICK_RATE_HZ);
    let mut next_tick = Instant::now();
    loop {
        let report = server.tick(tick.as_secs_f32());

        for id in report.joined {
            println!("{id} joined");
        }
        for id in report.left {
            println!("{id} left");
        }
        for event in report.events {
            match event {
                GameEvent::SubmarineSunk { victim, attacker } => println!("{attacker} sank {victim}"),
                GameEvent::SonarPing { by, .. } => println!("{by} pinged"),
                _ => {}
            }
        }

        next_tick += tick;
        let now = Instant::now();
        if next_tick > now {
            std::thread::sleep(next_tick - now);
        } else {
            next_tick = now; // we fell behind; don't try to catch up in a burst
        }
    }
}
