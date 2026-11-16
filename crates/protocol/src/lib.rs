//! # SubHunt wire protocol
//!
//! How messages look on the wire. Shared by the client and server network
//! adapters so they always agree.
//!
//! Format: one JSON object per line. JSON is not the most compact choice, but
//! you can read it, which helps a lot while learning. To debug, connect with
//! `nc 127.0.0.1 7777` and watch the server talk.
//!
//! The types here are *DTOs* (data transfer objects): plain structs that mirror
//! the application's messages but carry serde's `Serialize`/`Deserialize`.
//! We keep them separate so serialisation concerns never leak into the domain,
//! and so renaming a domain field can't silently break old clients.

mod dto;
mod mapping;

pub use dto::*;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Turns a message into one line of text (including the trailing newline).
pub fn encode_line<T: Serialize>(message: &T) -> String {
    // Rust note: `expect` crashes with this message if serialisation fails.
    // That's acceptable here because our DTOs contain nothing that can fail.
    let mut line = serde_json::to_string(message).expect("protocol messages always serialise");
    line.push('\n');
    line
}

/// Parses one line of text back into a message.
///
/// Rust note: `Result<T, E>` is either `Ok(value)` or `Err(error)`. Rust has no
/// exceptions; errors are ordinary values you must handle.
pub fn decode_line<T: DeserializeOwned>(line: &str) -> Result<T, DecodeError> {
    serde_json::from_str(line.trim_end()).map_err(DecodeError)
}

/// A line couldn't be understood (bad JSON, unknown message, wrong fields).
#[derive(Debug)]
pub struct DecodeError(serde_json::Error);

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "could not decode message: {}", self.0)
    }
}

impl std::error::Error for DecodeError {}

#[cfg(test)]
mod tests {
    use subhunt_application::{Notice, ServerMessage};
    use subhunt_domain::{
        Contact, ContactKind, HelmOrders, OwnShipStatus, PlayerId, SensorPicture, TorpedoId, TorpedoTrack, Vec2,
    };

    use super::*;

    #[test]
    fn sensor_picture_survives_a_round_trip() {
        let original = ServerMessage::Picture(SensorPicture {
            tick: 99,
            own_ship: OwnShipStatus {
                position: Vec2::new(1.5, -2.25),
                heading: 0.7,
                speed: 12.0,
                throttle: 0.4,
                noise: 0.6,
                torpedo_cooldown: 1.0,
                ping_cooldown: 0.0,
            },
            own_torpedoes: vec![TorpedoTrack { id: TorpedoId(4), position: Vec2::new(3.0, 4.0), heading: 1.0 }],
            contacts: vec![
                Contact::Bearing { bearing: 0.3, kind: ContactKind::Torpedo, strength: 0.8 },
                Contact::Fix { position: Vec2::new(-5.0, 6.0), kind: ContactKind::Submarine },
            ],
            score: 2,
        });

        let line = encode_line(&ServerToClient::from(&original));
        assert!(line.ends_with('\n'));
        let decoded: ServerToClient = decode_line(&line).unwrap();
        assert_eq!(ServerMessage::from(decoded), original);
    }

    #[test]
    fn notices_and_orders_round_trip() {
        for notice in
            [Notice::Sunk { by: PlayerId(7) }, Notice::Kill { victim: PlayerId(2), position: Vec2::new(10.5, -3.0) }]
        {
            let message = ServerMessage::Notice(notice);
            let decoded: ServerToClient = decode_line(&encode_line(&ServerToClient::from(&message))).unwrap();
            assert_eq!(ServerMessage::from(decoded), message);
        }

        let orders = HelmOrders { throttle: -0.5, rudder: 1.0, fire_torpedo: true, sonar_ping: false };
        let decoded: ClientToServer = decode_line(&encode_line(&ClientToServer::from(&orders))).unwrap();
        let ClientToServer::Orders(dto) = decoded;
        assert_eq!(HelmOrders::from(dto), orders);
    }

    #[test]
    fn garbage_is_an_error_not_a_crash() {
        assert!(decode_line::<ClientToServer>("{not json").is_err());
        assert!(decode_line::<ClientToServer>(r#"{"Teleport":{"x":0}}"#).is_err());
    }
}
