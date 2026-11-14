use bevy::prelude::*;
use subhunt_application::ClientSession;

/// Makes the application's `ClientSession` available to Bevy systems.
///
/// Rust note: we can't write `impl Resource for ClientSession` here, because
/// Rust's *orphan rule* forbids implementing a foreign trait (Bevy's) for a
/// foreign type (the application crate's). Wrapping it in our own newtype is
/// the standard workaround, and it also keeps Bevy out of the application crate.
#[derive(Resource)]
pub struct Session(pub ClientSession);
