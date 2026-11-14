//! Messages the server sends to a player, in application terms.
//!
//! These are *not* the wire format. The protocol crate decides how they become
//! bytes. Keeping them separate means we can change JSON to a binary format
//! without touching this crate.

use subhunt_domain::{PlayerId, SensorPicture};

#[derive(Clone, Debug, PartialEq)]
pub enum ServerMessage {
    /// Sent once, when a player joins.
    Welcome { player_id: PlayerId, world_half_extent: f32, ping_range: f32 },
    /// Sent every tick: what this player's sonar knows right now.
    Picture(SensorPicture),
    /// Something the player should be told about.
    Notice(Notice),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Notice {
    /// You were sunk (and have respawned).
    Sunk {
        by: PlayerId,
    },
    /// Your torpedo sank someone.
    Kill {
        victim: PlayerId,
    },
    /// Someone's ping hit you: they know where you are.
    Pinged {
        bearing: f32,
    },
    PlayerJoined(PlayerId),
    PlayerLeft(PlayerId),
}
