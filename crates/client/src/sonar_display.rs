//! Output adapter: draws the sonar scope.
//!
//! Everything is drawn with *gizmos*: Bevy's immediate-mode line drawing.
//! We redraw the whole scope every frame from the session's data, so there are
//! no sprites or entities to keep in sync. Great for prototyping.
//!
//! The scope is "north up" with your submarine always in the centre.

use bevy::prelude::*;
use subhunt_domain::{Contact, ContactKind, Vec2 as WorldVec2};

use crate::session::Session;

/// Radius of the scope in screen pixels.
const SCOPE_RADIUS: f32 = 380.0;
/// Radians per second the sweep line turns.
const SWEEP_SPEED: f32 = 1.2;

const SCOPE_GREEN: Color = Color::srgb(0.15, 0.85, 0.35);
const DIM_GREEN: Color = Color::srgba(0.15, 0.85, 0.35, 0.25);
const SUB_BEARING: Color = Color::srgb(1.0, 0.65, 0.2);
const TORPEDO_RED: Color = Color::srgb(1.0, 0.2, 0.2);
const OWN_TORPEDO: Color = Color::srgb(1.0, 1.0, 0.4);

pub struct SonarDisplayPlugin;

impl Plugin for SonarDisplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera).add_systems(Update, draw_sonar);
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn draw_sonar(mut gizmos: Gizmos, session: Res<Session>, time: Res<Time>) {
    draw_scope(&mut gizmos, time.elapsed_secs());

    let session = &session.0;
    let Some(picture) = session.picture() else {
        return; // nothing to show until the first picture arrives
    };

    // World metres → screen pixels, relative to our own submarine.
    let scale = SCOPE_RADIUS / session.display_range();
    let me = picture.own_ship.position;
    let to_screen = |p: WorldVec2| {
        let offset = p - me;
        Vec2::new(offset.x, offset.y) * scale
    };

    draw_world_edge(&mut gizmos, session.world_half_extent(), &to_screen);

    // Passive contacts: a line pointing towards the noise. Brighter = clearer.
    for contact in &picture.contacts {
        if let Contact::Bearing { bearing, kind, strength } = *contact {
            let colour = match kind {
                ContactKind::Submarine => SUB_BEARING,
                ContactKind::Torpedo => TORPEDO_RED,
            };
            let direction = Vec2::from_angle(bearing);
            gizmos.line_2d(direction * 30.0, direction * SCOPE_RADIUS, colour.with_alpha(0.25 + 0.75 * strength));
        }
    }

    // Ping echoes: exact positions that fade out over a few seconds.
    for echo in session.echoes() {
        let position = to_screen(echo.position);
        if position.length() <= SCOPE_RADIUS {
            let colour = match echo.kind {
                ContactKind::Submarine => SCOPE_GREEN,
                ContactKind::Torpedo => TORPEDO_RED,
            };
            gizmos.circle_2d(position, 8.0, colour.with_alpha(echo.intensity()));
        }
    }

    for torpedo in &picture.own_torpedoes {
        let position = to_screen(torpedo.position);
        if position.length() <= SCOPE_RADIUS {
            gizmos.circle_2d(position, 3.0, OWN_TORPEDO);
            gizmos.line_2d(position, position + Vec2::from_angle(torpedo.heading) * 10.0, OWN_TORPEDO);
        }
    }

    draw_own_ship(&mut gizmos, picture.own_ship.heading);
}

fn draw_scope(gizmos: &mut Gizmos, elapsed: f32) {
    gizmos.circle_2d(Vec2::ZERO, SCOPE_RADIUS, SCOPE_GREEN).resolution(128);
    for ring in 1..=2 {
        gizmos.circle_2d(Vec2::ZERO, SCOPE_RADIUS * ring as f32 / 3.0, DIM_GREEN).resolution(96);
    }
    gizmos.line_2d(Vec2::new(-SCOPE_RADIUS, 0.0), Vec2::new(SCOPE_RADIUS, 0.0), DIM_GREEN);
    gizmos.line_2d(Vec2::new(0.0, -SCOPE_RADIUS), Vec2::new(0.0, SCOPE_RADIUS), DIM_GREEN);

    // The rotating sweep, with a fading tail. Purely decorative: real sonar data
    // arrives whenever the server sends it, not when the sweep passes.
    let sweep = -elapsed * SWEEP_SPEED;
    for i in 0..16 {
        let angle = sweep + i as f32 * 0.02;
        let alpha = 0.6 * (1.0 - i as f32 / 16.0);
        gizmos.line_2d(Vec2::ZERO, Vec2::from_angle(angle) * SCOPE_RADIUS, SCOPE_GREEN.with_alpha(alpha));
    }
}

fn draw_own_ship(gizmos: &mut Gizmos, heading: f32) {
    let nose = Vec2::from_angle(heading) * 14.0;
    let left = Vec2::from_angle(heading + 2.5) * 10.0;
    let right = Vec2::from_angle(heading - 2.5) * 10.0;
    gizmos.linestrip_2d([nose, left, right, nose], Color::WHITE);
}

/// Draws the edge of the play area, clipped to the scope.
fn draw_world_edge(gizmos: &mut Gizmos, half_extent: f32, to_screen: &impl Fn(WorldVec2) -> Vec2) {
    let corners = [
        WorldVec2::new(-half_extent, -half_extent),
        WorldVec2::new(half_extent, -half_extent),
        WorldVec2::new(half_extent, half_extent),
        WorldVec2::new(-half_extent, half_extent),
    ]
    .map(to_screen);
    for i in 0..4 {
        if let Some((a, b)) = clip_to_circle(corners[i], corners[(i + 1) % 4], SCOPE_RADIUS) {
            gizmos.line_2d(a, b, TORPEDO_RED.with_alpha(0.6));
        }
    }
}

/// The part of the segment a→b that lies inside a circle at the origin, if any.
fn clip_to_circle(a: Vec2, b: Vec2, radius: f32) -> Option<(Vec2, Vec2)> {
    // Solve |a + t(b - a)| = radius for t (a quadratic), then keep t in 0..1.
    let d = b - a;
    let qa = d.dot(d);
    if qa == 0.0 {
        return None;
    }
    let qb = 2.0 * a.dot(d);
    let qc = a.dot(a) - radius * radius;
    let discriminant = qb * qb - 4.0 * qa * qc;
    if discriminant <= 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let t0 = ((-qb - root) / (2.0 * qa)).max(0.0);
    let t1 = ((-qb + root) / (2.0 * qa)).min(1.0);
    (t0 < t1).then(|| (a + d * t0, a + d * t1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipping_keeps_only_the_inside_part() {
        let (a, b) = clip_to_circle(Vec2::new(-20.0, 0.0), Vec2::new(20.0, 0.0), 10.0).unwrap();
        assert!((a.x + 10.0).abs() < 1e-4 && (b.x - 10.0).abs() < 1e-4);
        assert!(clip_to_circle(Vec2::new(-20.0, 50.0), Vec2::new(20.0, 50.0), 10.0).is_none());
    }
}
