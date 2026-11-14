//! # SubHunt dedicated server
//!
//! This is a *composition root*: the one place where concrete pieces get
//! plugged together. We open two transports (TCP for desktop clients,
//! WebSockets for browsers), merge them into one, hand that and a fresh world
//! to the `GameServer` use case, and run a fixed-rate loop.
//!
//! Usage: `cargo run -p subhunt-server -- [--tcp ADDR] [--web ADDR] [--site DIR]`

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use subhunt_application::{CombinedTransport, GameServer, PlayerIdSource};
use subhunt_domain::{GameEvent, GameRules, GameWorld};
use subhunt_net_tcp::TcpServerTransport;
use subhunt_net_ws::WsServerTransport;

/// Simulation steps per second. Every player gets a sensor picture each tick.
const TICK_RATE_HZ: f64 = 20.0;

/// Command-line settings.
struct Settings {
    tcp: String,
    web: String,
    site: PathBuf,
}

impl Settings {
    fn from_args() -> Result<Self, String> {
        let mut settings = Settings { tcp: "0.0.0.0:7777".into(), web: "0.0.0.0:8080".into(), site: "web".into() };
        let mut args = std::env::args().skip(1);
        // Rust note: `while let` keeps looping as long as the pattern matches.
        while let Some(flag) = args.next() {
            let mut value = || args.next().ok_or(format!("{flag} needs a value"));
            match flag.as_str() {
                "--tcp" => settings.tcp = value()?,
                "--web" => settings.web = value()?,
                "--site" => settings.site = value()?.into(),
                other => {
                    return Err(format!(
                        "unknown option {other}\nusage: subhunt-server [--tcp ADDR] [--web ADDR] [--site DIR]"
                    ));
                }
            }
        }
        Ok(settings)
    }
}

fn main() -> std::io::Result<()> {
    let settings = Settings::from_args().map_err(std::io::Error::other)?;

    // Both transports draw ids from one source, so ids never clash.
    let ids = PlayerIdSource::new();
    let tcp = TcpServerTransport::bind(&settings.tcp, ids.clone())?;
    let web = WsServerTransport::bind(&settings.web, ids, Some(settings.site.clone()))?;
    println!("SubHunt server");
    println!("  desktop clients: {}", tcp.local_addr());
    println!("  browsers:        http://{} (open this in a browser)", web.local_addr());
    if !settings.site.join("pkg").exists() {
        println!("  note: the web client isn't built yet. Run scripts/build-web.sh first.");
    }

    // A different seed each run gives different spawn points.
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    let world = GameWorld::new(GameRules::default(), seed);
    let mut server = GameServer::new(world, CombinedTransport::new(tcp, web));

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
