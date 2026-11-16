//! Conversions between application/domain types and DTOs.
//!
//! Rust note: implementing `From<A> for B` lets you write `B::from(a)` or
//! `let b: B = a.into();`. It's the idiomatic way to convert between types.
//! Converting *from a reference* (`From<&A>`) avoids taking ownership of the
//! original, so the caller can keep using it.

use subhunt_application::{Notice, ServerMessage};
use subhunt_domain::{
    Contact, ContactKind, HelmOrders, OwnShipStatus, PlayerId, SensorPicture, TorpedoId, TorpedoTrack, Vec2,
};

use crate::dto::*;

// ----- Orders -----

impl From<&HelmOrders> for ClientToServer {
    fn from(o: &HelmOrders) -> Self {
        ClientToServer::Orders(OrdersDto {
            throttle: o.throttle,
            rudder: o.rudder,
            fire_torpedo: o.fire_torpedo,
            sonar_ping: o.sonar_ping,
        })
    }
}

impl From<OrdersDto> for HelmOrders {
    fn from(o: OrdersDto) -> Self {
        HelmOrders { throttle: o.throttle, rudder: o.rudder, fire_torpedo: o.fire_torpedo, sonar_ping: o.sonar_ping }
    }
}

// ----- Server messages -----

impl From<&ServerMessage> for ServerToClient {
    fn from(message: &ServerMessage) -> Self {
        match message {
            ServerMessage::Welcome { player_id, world_half_extent, ping_range } => ServerToClient::Welcome {
                player_id: player_id.0,
                world_half_extent: *world_half_extent,
                ping_range: *ping_range,
            },
            ServerMessage::Picture(picture) => ServerToClient::Picture(picture.into()),
            ServerMessage::Notice(notice) => ServerToClient::Notice((*notice).into()),
        }
    }
}

impl From<ServerToClient> for ServerMessage {
    fn from(message: ServerToClient) -> Self {
        match message {
            ServerToClient::Welcome { player_id, world_half_extent, ping_range } => {
                ServerMessage::Welcome { player_id: PlayerId(player_id), world_half_extent, ping_range }
            }
            ServerToClient::Picture(picture) => ServerMessage::Picture(picture.into()),
            ServerToClient::Notice(notice) => ServerMessage::Notice(notice.into()),
        }
    }
}

// ----- Building blocks -----

impl From<Vec2> for Vec2Dto {
    fn from(v: Vec2) -> Self {
        Vec2Dto { x: v.x, y: v.y }
    }
}

impl From<Vec2Dto> for Vec2 {
    fn from(v: Vec2Dto) -> Self {
        Vec2::new(v.x, v.y)
    }
}

impl From<&SensorPicture> for PictureDto {
    fn from(p: &SensorPicture) -> Self {
        PictureDto {
            tick: p.tick,
            own_ship: (&p.own_ship).into(),
            own_torpedoes: p.own_torpedoes.iter().map(TorpedoTrackDto::from).collect(),
            contacts: p.contacts.iter().map(|c| ContactDto::from(*c)).collect(),
            score: p.score,
        }
    }
}

impl From<PictureDto> for SensorPicture {
    fn from(p: PictureDto) -> Self {
        SensorPicture {
            tick: p.tick,
            own_ship: p.own_ship.into(),
            own_torpedoes: p.own_torpedoes.into_iter().map(TorpedoTrack::from).collect(),
            contacts: p.contacts.into_iter().map(Contact::from).collect(),
            score: p.score,
        }
    }
}

impl From<&OwnShipStatus> for OwnShipDto {
    fn from(s: &OwnShipStatus) -> Self {
        OwnShipDto {
            position: s.position.into(),
            heading: s.heading,
            speed: s.speed,
            throttle: s.throttle,
            noise: s.noise,
            torpedo_cooldown: s.torpedo_cooldown,
            ping_cooldown: s.ping_cooldown,
        }
    }
}

impl From<OwnShipDto> for OwnShipStatus {
    fn from(s: OwnShipDto) -> Self {
        OwnShipStatus {
            position: s.position.into(),
            heading: s.heading,
            speed: s.speed,
            throttle: s.throttle,
            noise: s.noise,
            torpedo_cooldown: s.torpedo_cooldown,
            ping_cooldown: s.ping_cooldown,
        }
    }
}

impl From<&TorpedoTrack> for TorpedoTrackDto {
    fn from(t: &TorpedoTrack) -> Self {
        TorpedoTrackDto { id: t.id.0, position: t.position.into(), heading: t.heading }
    }
}

impl From<TorpedoTrackDto> for TorpedoTrack {
    fn from(t: TorpedoTrackDto) -> Self {
        TorpedoTrack { id: TorpedoId(t.id), position: t.position.into(), heading: t.heading }
    }
}

impl From<ContactKind> for ContactKindDto {
    fn from(k: ContactKind) -> Self {
        match k {
            ContactKind::Submarine => ContactKindDto::Submarine,
            ContactKind::Torpedo => ContactKindDto::Torpedo,
        }
    }
}

impl From<ContactKindDto> for ContactKind {
    fn from(k: ContactKindDto) -> Self {
        match k {
            ContactKindDto::Submarine => ContactKind::Submarine,
            ContactKindDto::Torpedo => ContactKind::Torpedo,
        }
    }
}

impl From<Contact> for ContactDto {
    fn from(c: Contact) -> Self {
        match c {
            Contact::Bearing { bearing, kind, strength } => {
                ContactDto::Bearing { bearing, kind: kind.into(), strength }
            }
            Contact::Fix { position, kind } => ContactDto::Fix { position: position.into(), kind: kind.into() },
        }
    }
}

impl From<ContactDto> for Contact {
    fn from(c: ContactDto) -> Self {
        match c {
            ContactDto::Bearing { bearing, kind, strength } => {
                Contact::Bearing { bearing, kind: kind.into(), strength }
            }
            ContactDto::Fix { position, kind } => Contact::Fix { position: position.into(), kind: kind.into() },
        }
    }
}

impl From<Notice> for NoticeDto {
    fn from(n: Notice) -> Self {
        match n {
            Notice::Sunk { by } => NoticeDto::Sunk { by: by.0 },
            Notice::Kill { victim, position } => NoticeDto::Kill { victim: victim.0, position: position.into() },
            Notice::Pinged { bearing } => NoticeDto::Pinged { bearing },
            Notice::PlayerJoined(p) => NoticeDto::PlayerJoined { player: p.0 },
            Notice::PlayerLeft(p) => NoticeDto::PlayerLeft { player: p.0 },
        }
    }
}

impl From<NoticeDto> for Notice {
    fn from(n: NoticeDto) -> Self {
        match n {
            NoticeDto::Sunk { by } => Notice::Sunk { by: PlayerId(by) },
            NoticeDto::Kill { victim, position } => {
                Notice::Kill { victim: PlayerId(victim), position: position.into() }
            }
            NoticeDto::Pinged { bearing } => Notice::Pinged { bearing },
            NoticeDto::PlayerJoined { player } => Notice::PlayerJoined(PlayerId(player)),
            NoticeDto::PlayerLeft { player } => Notice::PlayerLeft(PlayerId(player)),
        }
    }
}
