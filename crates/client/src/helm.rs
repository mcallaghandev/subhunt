//! Input adapter: keyboard → helm orders.

use bevy::prelude::*;
use subhunt_domain::HelmOrders;

/// How fast holding W/S moves the throttle lever (full range in ~3 seconds).
pub const THROTTLE_LEVER_SPEED: f32 = 0.6;

/// The local helm: what the player is currently asking for.
///
/// Throttle is a lever that stays where you leave it. Rudder is only applied
/// while a key is held. Fire and ping are latched until the next send, so a
/// quick tap between two network sends isn't lost.
#[derive(Resource, Default)]
pub struct Helm {
    pub throttle: f32,
    pub rudder: f32,
    pub fire_requested: bool,
    pub ping_requested: bool,
}

impl Helm {
    /// Builds orders to send and clears the one-shot requests.
    pub fn take_orders(&mut self) -> HelmOrders {
        HelmOrders {
            throttle: self.throttle,
            rudder: self.rudder,
            fire_torpedo: std::mem::take(&mut self.fire_requested),
            sonar_ping: std::mem::take(&mut self.ping_requested),
        }
    }
}

pub struct HelmPlugin;

impl Plugin for HelmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Helm>().add_systems(Update, read_keyboard);
    }
}

pub fn read_keyboard(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut helm: ResMut<Helm>) {
    let dt = time.delta_secs();
    let held = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);

    if held(KeyCode::KeyW, KeyCode::ArrowUp) {
        helm.throttle += THROTTLE_LEVER_SPEED * dt;
    }
    if held(KeyCode::KeyS, KeyCode::ArrowDown) {
        helm.throttle -= THROTTLE_LEVER_SPEED * dt;
    }
    if keys.just_pressed(KeyCode::KeyX) {
        helm.throttle = 0.0; // all stop
    }
    helm.throttle = helm.throttle.clamp(-1.0, 1.0);

    // Rust note: matching on a tuple is a neat way to handle combinations.
    helm.rudder = match (held(KeyCode::KeyA, KeyCode::ArrowLeft), held(KeyCode::KeyD, KeyCode::ArrowRight)) {
        (true, false) => 1.0,  // port (left, counter-clockwise)
        (false, true) => -1.0, // starboard (right, clockwise)
        _ => 0.0,
    };

    if keys.just_pressed(KeyCode::Space) {
        helm.ping_requested = true;
    }
    if keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::Enter) {
        helm.fire_requested = true;
    }
}
