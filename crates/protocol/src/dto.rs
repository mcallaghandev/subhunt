//! Serialisable mirrors of the application's messages.
//!
//! `#[derive(Serialize, Deserialize)]` comes from the `serde` crate and
//! generates the JSON conversion code for us.

use serde::{Deserialize, Serialize};

// ----- Client -> server -----

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientToServer {
    Orders(OrdersDto),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrdersDto {
    pub throttle: f32,
    pub rudder: f32,
    pub fire_torpedo: bool,
    pub sonar_ping: bool,
}

// ----- Server -> client -----

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerToClient {
    Welcome { player_id: u32, world_half_extent: f32, ping_range: f32 },
    Picture(PictureDto),
    Notice(NoticeDto),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vec2Dto {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PictureDto {
    pub tick: u64,
    pub own_ship: OwnShipDto,
    pub own_torpedoes: Vec<TorpedoTrackDto>,
    pub contacts: Vec<ContactDto>,
    pub score: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnShipDto {
    pub position: Vec2Dto,
    pub heading: f32,
    pub speed: f32,
    pub throttle: f32,
    pub noise: f32,
    pub torpedo_cooldown: f32,
    pub ping_cooldown: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TorpedoTrackDto {
    pub id: u32,
    pub position: Vec2Dto,
    pub heading: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ContactKindDto {
    Submarine,
    Torpedo,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ContactDto {
    Bearing { bearing: f32, kind: ContactKindDto, strength: f32 },
    Fix { position: Vec2Dto, kind: ContactKindDto },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum NoticeDto {
    Sunk { by: u32 },
    Kill { victim: u32, position: Vec2Dto },
    Pinged { bearing: f32 },
    PlayerJoined { player: u32 },
    PlayerLeft { player: u32 },
}
