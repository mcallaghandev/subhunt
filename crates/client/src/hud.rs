//! Output adapter: the text readouts.

use std::fmt::Write;

use bevy::prelude::*;
use subhunt_application::{ConnectionStatus, Notice};
use subhunt_domain::{Contact, compass_degrees};

use crate::session::Session;

const HUD_GREEN: Color = Color::srgb(0.3, 1.0, 0.5);

/// A *marker component*: no data, it just lets us find the HUD entity in a query.
#[derive(Component)]
struct StatusText;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud).add_systems(Update, update_status);
    }
}

fn spawn_hud(mut commands: Commands) {
    // Rust note: `spawn` takes a tuple of components; together they make one entity.
    commands.spawn((
        Text::new("Connecting..."),
        TextFont { font_size: FontSize::Px(16.0), ..default() },
        TextColor(HUD_GREEN),
        Node { position_type: PositionType::Absolute, top: Val::Px(12.0), left: Val::Px(12.0), ..default() },
        StatusText,
    ));
    commands.spawn((
        Text::new("W/S throttle   A/D rudder   X all stop\nSPACE sonar ping   F fire torpedo"),
        TextFont { font_size: FontSize::Px(14.0), ..default() },
        TextColor(HUD_GREEN.with_alpha(0.6)),
        Node { position_type: PositionType::Absolute, bottom: Val::Px(12.0), left: Val::Px(12.0), ..default() },
    ));
}

fn update_status(session: Res<Session>, mut query: Query<&mut Text, With<StatusText>>) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    text.0 = status_report(&session);
}

/// Builds the HUD text. Kept as a plain function so it's easy to read and test.
fn status_report(session: &Session) -> String {
    let session = &session.0;
    match session.status() {
        ConnectionStatus::AwaitingWelcome => return "Connecting...".into(),
        ConnectionStatus::Lost => return "CONNECTION LOST\nRestart the client to rejoin.".into(),
        ConnectionStatus::Playing => {}
    }
    let Some(picture) = session.picture() else {
        return "Waiting for sonar...".into();
    };
    let ship = &picture.own_ship;
    let bearings = picture.contacts.iter().filter(|c| matches!(c, Contact::Bearing { .. })).count();

    // `write!` into a String works like `format!` but appends. It returns a
    // `Result` we can safely ignore, because writing to a String can't fail.
    let mut out = String::new();
    if let Some(id) = session.player_id() {
        let _ = writeln!(out, "{id}    SCORE {}", picture.score);
    }
    let _ = writeln!(out, "HDG {:03.0}   SPD {:5.1}", compass_degrees(ship.heading), ship.speed);
    let _ = writeln!(out, "THROTTLE {}", throttle_bar(ship.throttle));
    let _ = writeln!(out, "NOISE {:.2}", ship.noise);
    let _ = writeln!(out, "TORPEDO {}", readiness(ship.torpedo_cooldown));
    let _ = writeln!(out, "PING    {}", readiness(ship.ping_cooldown));
    let _ = writeln!(out, "PASSIVE CONTACTS {bearings}");
    for timed in session.notices() {
        let _ = writeln!(out, "> {}", describe(timed.notice));
    }
    out
}

fn throttle_bar(throttle: f32) -> String {
    let filled = (throttle.abs() * 10.0).round() as usize;
    let direction = if throttle < 0.0 { "ASTERN" } else { "AHEAD" };
    format!("[{}{}] {:3.0}% {direction}", "#".repeat(filled), "-".repeat(10 - filled), throttle.abs() * 100.0)
}

fn readiness(cooldown: f32) -> String {
    if cooldown <= 0.0 { "READY".into() } else { format!("{cooldown:.1}s") }
}

fn describe(notice: Notice) -> String {
    match notice {
        Notice::Sunk { by } => format!("SUNK by {by}! New boat assigned."),
        Notice::Kill { victim } => format!("Hit! {victim} destroyed."),
        Notice::Pinged { bearing } => {
            format!("PINGED from bearing {:03.0}: you've been spotted", compass_degrees(bearing))
        }
        Notice::PlayerJoined(id) => format!("{id} entered the area"),
        Notice::PlayerLeft(id) => format!("{id} left the area"),
    }
}
